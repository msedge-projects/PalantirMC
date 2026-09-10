//! Blocking file downloads with atomic writes and `sha256` verification.
//!
//! Prism parity: Prism's `NetJob`/`Download` pipeline writes to a temporary
//! file and renames over the target only on success (mirroring `QSaveFile`);
//! here downloads land in `<dest>.part` and are renamed to `dest`.
//!
//! [`download_bytes`] is the unit-testable core (it takes any [`Fetcher`],
//! so tests inject [`MapFetcher`] bytes); [`download_file`] wraps it with a
//! [`BlockingHttpFetcher`] timeout.

use crate::meta::{BlockingHttpFetcher, Fetcher};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Compute the lowercase hex `sha256` digest of `data`.
///
/// Uses the `sha2` crate (pure Rust) and formats via `LowerHex`, so no manual
/// hex table is needed.
pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::Digest;
    let hash = sha2::Sha256::digest(data);
    format!("{hash:x}")
}

/// Verify that the file at `path` matches the expected hex `sha256` digest.
///
/// Comparison is ASCII case-insensitive and surrounding whitespace on
/// `expected_hex` is ignored. Returns [`crate::Error::HashMismatch`] on a
/// digest mismatch and [`crate::Error::Format`] when `expected_hex` is not a
/// 64-character hex string.
pub fn verify_sha256(path: &Path, expected_hex: &str) -> Result<(), crate::Error> {
    let expected = expected_hex.trim().to_ascii_lowercase();
    if expected.len() != 64 || !expected.bytes().all(is_hex_digit) {
        return Err(crate::Error::format(path, format!("invalid sha256 hex '{expected_hex}'")));
    }
    let bytes =
        std::fs::read(path).map_err(|e| crate::Error::io(path, e))?;
    let actual = sha256_hex(&bytes);
    if actual != expected {
        return Err(crate::Error::hash_mismatch(path, expected, actual));
    }
    Ok(())
}

/// Download `url` via `fetcher` into `dest`, returning the byte count.
///
/// The body is written to `<dest>.part` first and renamed over `dest` only on
/// success; a leftover `.part` file is removed when the write or rename fails.
/// Parent directories are created as needed.
pub fn download_bytes(
    fetcher: &dyn Fetcher,
    url: &str,
    dest: &Path,
) -> Result<u64, crate::Error> {
    let bytes = fetcher.fetch(url)?;
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| crate::Error::io(parent, e))?;
        }
    }
    let part = part_path(dest);
    let write_result = (|| -> std::io::Result<()> {
        std::fs::write(&part, &bytes)?;
        Ok(())
    })();
    if let Err(e) = write_result {
        let _ = std::fs::remove_file(&part);
        return Err(crate::Error::io(&part, e));
    }
    std::fs::rename(&part, dest).map_err(|e| {
        let _ = std::fs::remove_file(&part);
        crate::Error::io(dest, e)
    })?;
    u64_try_from_usize(bytes.len(), dest)
}

/// Blocking download of `url` into `dest` with a per-request `timeout`,
/// returning the byte count.
///
/// Thin wrapper over [`download_bytes`] with a [`BlockingHttpFetcher`]; the
/// atomic `.part` + rename semantics are identical.
pub fn download_file(url: &str, dest: &Path, timeout: Duration) -> Result<u64, crate::Error> {
    let fetcher = BlockingHttpFetcher::new(timeout);
    download_bytes(&fetcher, url, dest)
}

/// Return the sibling temporary path `<dest>.part` for atomic downloads.
fn part_path(dest: &Path) -> PathBuf {
    let mut os: OsString = dest.as_os_str().to_owned();
    os.push(".part");
    PathBuf::from(os)
}

/// Convert a `usize` byte length to `u64` without panicking on exotic targets.
fn u64_try_from_usize(len: usize, dest: &Path) -> Result<u64, crate::Error> {
    u64::try_from(len)
        .map_err(|_| crate::Error::format(dest, format!("body too large ({len} bytes)")))
}

/// Return `true` for ASCII hex digits (`0-9`, `a-f`, `A-F`).
fn is_hex_digit(b: u8) -> bool {
    matches!(b, b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::MapFetcher;

    #[test]
    fn sha256_hex_matches_known_vectors() {
        // echo -n "" | sha256sum
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        // echo -n "abc" | sha256sum
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sha256_hex_is_lowercase_hex() {
        let h = sha256_hex(b"prism");
        assert_eq!(h.len(), 64);
        assert!(h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')));
    }

    #[test]
    fn verify_sha256_accepts_correct_digest() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a.bin");
        std::fs::write(&path, b"abc").unwrap();
        verify_sha256(&path, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
            .unwrap();
    }

    #[test]
    fn verify_sha256_accepts_uppercase_and_whitespace() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a.bin");
        std::fs::write(&path, b"abc").unwrap();
        verify_sha256(&path, "  BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD\n")
            .unwrap();
    }

    #[test]
    fn verify_sha256_rejects_mismatch() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a.bin");
        std::fs::write(&path, b"abc").unwrap();
        let err = verify_sha256(
            &path,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        )
        .unwrap_err();
        match err {
            crate::Error::HashMismatch { expected, actual, .. } => {
                assert!(expected.starts_with("e3b0"));
                assert_eq!(actual, sha256_hex(b"abc"));
            }
            _ => panic!("expected HashMismatch"),
        }
    }

    #[test]
    fn verify_sha256_rejects_invalid_hex() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a.bin");
        std::fs::write(&path, b"abc").unwrap();
        let err = verify_sha256(&path, "not-hex").unwrap_err();
        match err {
            crate::Error::Format { .. } => {}
            _ => panic!("expected Format"),
        }
    }

    #[test]
    fn verify_sha256_missing_file_is_io_error() {
        let tmp = tempfile::tempdir().unwrap();
        let err = verify_sha256(
            &tmp.path().join("missing.bin"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        )
        .unwrap_err();
        match err {
            crate::Error::Io { .. } => {}
            _ => panic!("expected Io"),
        }
    }

    #[test]
    fn download_bytes_writes_atomically_and_returns_count() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("libs").join("a.jar");
        let mut f = MapFetcher::new();
        f.insert("https://x/a.jar", b"payload".to_vec());
        let n = download_bytes(&f, "https://x/a.jar", &dest).unwrap();
        assert_eq!(n, 7);
        assert_eq!(std::fs::read(&dest).unwrap(), b"payload");
        // no .part leftovers
        assert!(!part_path(&dest).exists());
    }

    #[test]
    fn download_bytes_overwrites_existing_target() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("a.jar");
        std::fs::write(&dest, b"old").unwrap();
        let mut f = MapFetcher::new();
        f.insert("https://x/a.jar", b"new-bytes".to_vec());
        let n = download_bytes(&f, "https://x/a.jar", &dest).unwrap();
        assert_eq!(n, 9);
        assert_eq!(std::fs::read(&dest).unwrap(), b"new-bytes");
    }

    #[test]
    fn download_bytes_propagates_fetcher_error_and_writes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("a.jar");
        let f = MapFetcher::new();
        let err = download_bytes(&f, "https://x/missing.jar", &dest).unwrap_err();
        match err {
            crate::Error::Http { .. } => {}
            _ => panic!("expected Http"),
        }
        assert!(!dest.exists());
        assert!(!part_path(&dest).exists());
    }

    #[test]
    fn download_file_reports_http_error_offline() {
        // Unroutable address + short timeout: must fail fast without network.
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("a.jar");
        let err = download_file(
            "http://192.0.2.1/a.jar",
            &dest,
            Duration::from_millis(800),
        )
        .unwrap_err();
        match err {
            crate::Error::Http { url, .. } => assert_eq!(url, "http://192.0.2.1/a.jar"),
            _ => panic!("expected Http"),
        }
        assert!(!dest.exists());
    }
}
