//! Transfers that survive interruption.
//!
//! Three promises, in the order they matter:
//!
//! 1. **Never call wrong bytes done.** The transfer is verified against the
//!    hash the metadata carries before it becomes its destination file, and
//!    wrong bytes are deleted rather than kept. Metadata that carries no
//!    hash is verified by size when it has one.
//! 2. **Resume, don't restart.** Interrupted bytes stay in a `.part` file
//!    beside the destination, and the next attempt continues from its
//!    length with an HTTP range request. A server that answers 200 to a
//!    range request is not resuming, and the file starts over rather than
//!    corrupting.
//! 3. **Retry what is worth retrying.** Connection trouble, 5xx and 429 are
//!    transient: they get exponential backoff (250 ms, 500 ms, 1 s) and a
//!    fresh attempt that resumes where the bytes stopped. Other 4xx are the
//!    server's considered opinion and fail at once.
//!
//! Backoff has no jitter on purpose: there is one launcher process, its
//! retries are few, and a deterministic delay is testable.

use std::path::{Path, PathBuf};
use std::time::Duration;

use sha1::{Digest, Sha1};

use crate::client::Http;
use crate::error::{Error, Result};

/// How many times to try a transient failure before giving up.
pub const DEFAULT_RETRIES: u32 = 3;

/// The first backoff pause; each retry doubles it.
pub const DEFAULT_BACKOFF: Duration = Duration::from_millis(250);

/// Knobs for one transfer.
#[derive(Debug, Clone, Copy)]
pub struct DownloadOptions {
    /// Transient-failure attempts after the first (so `3` = 4 tries total).
    pub retries: u32,
    /// First backoff pause.
    pub backoff: Duration,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            retries: DEFAULT_RETRIES,
            backoff: DEFAULT_BACKOFF,
        }
    }
}

/// What a finished transfer did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transfer {
    /// Byte offset the final successful attempt actually continued from --
    /// `> 0` is the receipt that this transfer resumed rather than
    /// restarted. A server that answered 200 to a range request means 0:
    /// the bytes were replaced, not continued.
    pub resumed_from: u64,
    /// Bytes the successful attempt wrote to the file.
    pub bytes: u64,
    /// How many attempts the transfer took.
    pub attempts: u32,
    /// The destination already held verified bytes; nothing was fetched.
    pub already_present: bool,
}

/// Fetch `url` to `dest`, resumable and verified.
///
/// `sha1` is the hash the metadata promises, hex; `expected_size` its size.
/// With neither, the transfer is unverified -- only metadata documents take
/// that path, and they come through the cache instead.
pub fn download(
    http: &Http,
    url: &str,
    dest: &Path,
    sha1: Option<&str>,
    expected_size: Option<u64>,
    opts: &DownloadOptions,
) -> Result<Transfer> {
    if dest.exists() {
        if verify(dest, sha1, expected_size).is_ok() {
            return Ok(Transfer {
                resumed_from: 0,
                bytes: 0,
                attempts: 0,
                already_present: true,
            });
        }
        // Present but wrong: stale bytes from something else. Remove rather
        // than trust a name that lied once.
        remove_if_present(dest)?;
    }

    let part = part_path(dest);
    let mut backoff = opts.backoff;
    let mut attempt = 0u32;
    loop {
        let offset = file_len(&part).unwrap_or(0);
        match try_once(http, url, &part, offset, sha1, expected_size) {
            Ok((bytes, resumed_from)) => {
                std::fs::rename(&part, dest).map_err(|source| Error::Io {
                    path: dest.to_path_buf(),
                    source,
                })?;
                return Ok(Transfer {
                    resumed_from,
                    bytes,
                    attempts: attempt + 1,
                    already_present: false,
                });
            }
            Err(err) if attempt < opts.retries && is_transient(&err) => {
                attempt += 1;
                std::thread::sleep(backoff);
                backoff *= 2;
            }
            Err(err) => {
                if is_corrupt(&err) {
                    // Wrong bytes must not seed the next resume.
                    remove_if_present(&part)?;
                }
                return Err(err);
            }
        }
    }
}

/// Where an interrupted transfer keeps its bytes. Public because a caller
/// that wants to *stage* an interruption (a test, a paused install) writes
/// here exactly as a dropped connection would.
pub fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.as_os_str().to_owned();
    name.push(".part");
    PathBuf::from(name)
}

/// The SHA-1 of a file's bytes, hex.
pub fn sha1_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut hasher = Sha1::new();
    std::io::copy(&mut file, &mut hasher).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(hex(&hasher.finalize()))
}

/// The SHA-1 of bytes in memory, hex.
pub fn sha1_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    hex(&hasher.finalize())
}

/// One attempt: fetch from `offset`, append to `part`, verify. Returns the
/// bytes written and the offset the response really continued from.
fn try_once(
    http: &Http,
    url: &str,
    part: &Path,
    offset: u64,
    sha1: Option<&str>,
    expected_size: Option<u64>,
) -> Result<(u64, u64)> {
    let mut response = if offset > 0 {
        http.get_range(url, offset)?
    } else {
        http.get(url)?
    };
    let status = response.status();

    let (append, resumed_from) = match status {
        200 => {
            // The whole body, whether or not we asked for a range: any
            // bytes already in the part are from an attempt this response
            // is not continuing.
            truncate(part)?;
            (false, 0)
        }
        206 => {
            if offset == 0 {
                return Err(Error::Invalid {
                    what: "transfer",
                    why: format!("{url} answered 206 to a whole-file request"),
                });
            }
            (true, offset)
        }
        416 if offset > 0 => {
            // "Range not satisfiable": the part is likely already the whole
            // file. Verify it as the completed transfer it claims to be.
            verify(part, sha1, expected_size)?;
            return Ok((0, offset));
        }
        408 | 425 | 429 | 500..=599 => {
            return Err(Error::Http {
                url: url.to_string(),
                status: Some(status),
                why: "transient; will retry".to_string(),
            });
        }
        _ => {
            return Err(Error::Http {
                url: url.to_string(),
                status: Some(status),
                why: "not a success".to_string(),
            });
        }
    };

    let mut file = open_part(part, append)?;
    let copied = response.copy_to(&mut file, None)?;
    drop(file);

    verify(part, sha1, expected_size)?;
    Ok((copied, resumed_from))
}

/// Is this failure worth another attempt?
fn is_transient(err: &Error) -> bool {
    match err {
        Error::Io { .. } => true, // a dropped connection surfaces here mid-copy
        Error::Http { status: None, .. } => true, // never got an answer
        Error::Http {
            status: Some(status),
            ..
        } => matches!(status, 408 | 425 | 429 | 500..=599),
        _ => false,
    }
}

/// Is this failure a wrong-bytes failure?
fn is_corrupt(err: &Error) -> bool {
    matches!(err, Error::Hash { .. })
}

/// Check what a file claims to be before it is called done.
fn verify(path: &Path, sha1: Option<&str>, expected_size: Option<u64>) -> Result<()> {
    if let Some(expected) = sha1 {
        let actual = sha1_file(path)?;
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(Error::Hash {
                path: path.to_path_buf(),
                expected: expected.to_string(),
                actual,
            });
        }
    }
    if let Some(expected) = expected_size {
        let actual = file_len(path).unwrap_or(0);
        if actual != expected {
            return Err(Error::Invalid {
                what: "transfer",
                why: format!(
                    "{} is {actual} bytes; the metadata promised {expected}",
                    path.display()
                ),
            });
        }
    }
    Ok(())
}

fn open_part(part: &Path, append: bool) -> Result<std::fs::File> {
    if let Some(parent) = part.parent() {
        std::fs::create_dir_all(parent).map_err(|source| Error::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(append)
        .truncate(!append)
        .open(part)
        .map_err(|source| Error::Io {
            path: part.to_path_buf(),
            source,
        })
}

fn truncate(part: &Path) -> Result<()> {
    if part.exists() {
        std::fs::remove_file(part).map_err(|source| Error::Io {
            path: part.to_path_buf(),
            source,
        })?;
    }
    Ok(())
}

fn remove_if_present(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(Error::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn file_len(path: &Path) -> Option<u64> {
    std::fs::metadata(path).ok().map(|m| m.len())
}

/// SHA-1 digest bytes as lowercase hex.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
