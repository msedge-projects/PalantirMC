//! The authenticated session: a credential that has to outlive the window.
//!
//! A sign-in is worth nothing if it dies with the process. The launcher that
//! forgets it makes the player re-type a device code every launch, and the one
//! that stores it carelessly leaves a bearer token in a world-readable file —
//! the same token grants the account's Minecraft identity to anything that can
//! read it.
//!
//! So the file this module owns is written the way a credential should be: one
//! atomic write (a half-written session must never read as a valid one), mode
//! `0o600` from the moment it exists on unix, and a read that refuses a symlink
//! or a non-regular file rather than following it. Expiry is *stored*, not
//! guessed: the token's own lifetime comes from the service, and a session
//! whose expiration is unknown is treated as expired rather than trusted
//! forever.
//!
//! What is deliberately absent: re-deriving a token here, or injecting it into
//! a launch. Refreshing is [`crate::auth::refresh_session`]'s job, and the
//! caller decides what to do with the answer.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// A signed-in player's session as it survives a restart.
///
/// Every field is stored rather than recomputed: the uuid is the service's own
/// (an offline derivation would be a different identity), and `client_id` is
/// kept beside the refresh token because a refresh is addressed to the client
/// that was granted the token in the first place.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredSession {
    pub player_name: String,
    pub player_uuid: String,
    pub access_token: String,
    /// The token a refresh can be bought with, when the service issued one.
    #[serde(default)]
    pub refresh_token: Option<String>,
    /// The client id this session was granted to; a refresh names it again.
    #[serde(default)]
    pub client_id: String,
    /// Unix seconds after which `access_token` must not be used. Zero means the
    /// service named no lifetime, which [`StoredSession::is_expired`] refuses
    /// to treat as "forever".
    pub expires_at: u64,
}

impl StoredSession {
    /// When a session issued now with a service-named lifetime expires.
    ///
    /// Saturating, because the arithmetic is over a value the service chose: a
    /// nonsensical lifetime must clamp rather than wrap into a date in the past
    /// that would look expired for a reason nobody could explain.
    pub fn expires_at(now_unix: u64, expires_in: u64) -> u64 {
        now_unix.saturating_add(expires_in)
    }

    /// Whether the access token must not be used at `now_unix`.
    ///
    /// A session with no stated lifetime is expired: the alternative is
    /// trusting an unknown token for as long as the file exists, which is how a
    /// launcher ends up launching into a 401 with nothing to show the player.
    pub fn is_expired(&self, now_unix: u64) -> bool {
        self.expires_at == 0 || now_unix >= self.expires_at
    }

    /// The identity a launch injects: `(uuid, name)`, in the order the launch
    /// context takes them, so one caller cannot pass them the wrong way round.
    pub fn identity(&self) -> (String, String) {
        (self.player_uuid.clone(), self.player_name.clone())
    }
}

/// Where one data root's session file lives.
///
/// The layout belongs to `palantir-core`; this names the single file inside it
/// that holds a credential, so the writer and the reader cannot disagree about
/// which file that is.
pub fn session_file(root: &palantir_core::paths::DataRoot) -> PathBuf {
    root.root.join("session.json")
}

/// Read the stored session.
///
/// A missing file is `Ok(None)` -- nobody has signed in yet, which is a state
/// the frontend renders rather than an error. Anything else is reported: a
/// session file that is a symlink, is not a regular file, or does not parse is
/// refused loudly, because silently reporting "signed out" would hide a
/// tampered or corrupted credential behind a routine-looking screen.
pub fn load(path: &Path) -> Result<Option<StoredSession>> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(Error::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(Error::Invalid {
            what: "stored session",
            why: format!(
                "{} is not a regular file, so no session is read through it",
                path.display()
            ),
        });
    }
    let bytes = std::fs::read(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let session: StoredSession =
        serde_json::from_slice(&bytes).map_err(|source| Error::Invalid {
            what: "stored session",
            why: format!("{}: {source}", path.display()),
        })?;
    Ok(Some(session))
}

/// Write the session, atomically and privately.
///
/// The temporary file is written first and renamed over the destination, so an
/// interrupted write leaves the previous session rather than half of a new one.
/// On unix the temporary file is created at mode `0o600` from the start:
/// writing a token and then narrowing the mode would leave a window in which
/// the credential is readable by every account on the machine.
pub fn save(path: &Path, session: &StoredSession) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| Error::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let bytes = serde_json::to_vec_pretty(session).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source: std::io::Error::other(source.to_string()),
    })?;
    let staging = staging_path(path);
    write_private(&staging, &bytes)?;
    if let Err(source) = std::fs::rename(&staging, path) {
        // Windows cannot replace an existing file with a rename. Losing the
        // previous session costs one sign-in, which is why this is a remove
        // rather than the backup dance a descriptor file needs.
        if path.exists() {
            std::fs::remove_file(path).map_err(|source| Error::Io {
                path: path.to_path_buf(),
                source,
            })?;
            return std::fs::rename(&staging, path).map_err(|source| Error::Io {
                path: path.to_path_buf(),
                source,
            });
        }
        let _ = std::fs::remove_file(&staging);
        return Err(Error::Io {
            path: path.to_path_buf(),
            source,
        });
    }
    Ok(())
}

/// Sign out: remove the stored session.
///
/// Removing what is not there is not an error -- signing out twice is not a
/// mistake the player should be told about -- and a symlink where the session
/// belongs is refused rather than deleted through, the same way it is refused
/// on read.
pub fn clear(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(Error::Invalid {
            what: "stored session",
            why: format!(
                "{} is a symbolic link, so nothing is removed through it",
                path.display()
            ),
        }),
        Ok(_) => std::fs::remove_file(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        }),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(Error::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// The temporary file beside the session, in the same directory so the rename
/// stays within one filesystem.
fn staging_path(path: &Path) -> PathBuf {
    let mut name = match path.file_name() {
        Some(name) => name.to_os_string(),
        None => std::ffi::OsString::from("session.json"),
    };
    name.push(".tmp");
    match path.parent() {
        Some(parent) => parent.join(name),
        None => PathBuf::from(name),
    }
}

/// Create the staging file with the narrowest mode the platform offers on
/// creation, then fill it. Windows has no mode bits to set: the file inherits
/// the profile directory's ACL, which is the documented platform difference
/// rather than an oversight.
#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
    file.write_all(bytes).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    file.flush().map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
    file.write_all(bytes).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    file.flush().map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "palantirmc-session-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn a_session() -> StoredSession {
        StoredSession {
            player_name: "Quiet".into(),
            player_uuid: "11111111-2222-3333-4444-555555555555".into(),
            access_token: "a-token".into(),
            refresh_token: Some("a-refresh-token".into()),
            client_id: "client-1".into(),
            expires_at: StoredSession::expires_at(1_000, 86_400),
        }
    }

    #[test]
    fn a_saved_session_reads_back_whole() {
        let dir = temp_dir("round-trip");
        let path = dir.join("session.json");
        let session = a_session();
        save(&path, &session).unwrap();
        assert_eq!(load(&path).unwrap(), Some(session.clone()));
        assert_eq!(session.identity().0, session.player_uuid);
        assert_eq!(session.identity().1, "Quiet");
        assert!(!path.with_extension("json.tmp").exists(), "no staging left");
        clear(&path).unwrap();
        assert_eq!(load(&path).unwrap(), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_missing_session_is_signed_out_and_nothing_more() {
        let dir = temp_dir("missing");
        let path = dir.join("session.json");
        assert_eq!(load(&path).unwrap(), None);
        clear(&path).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_malformed_session_is_reported_rather_than_read_as_signed_out() {
        let dir = temp_dir("malformed");
        let path = dir.join("session.json");
        std::fs::write(&path, b"{\"player_name\":\"Quiet\"").unwrap();
        let error = load(&path).unwrap_err().to_string();
        assert!(error.contains("session.json"), "{error}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_session_with_no_stated_lifetime_is_expired() {
        let mut session = a_session();
        session.expires_at = 0;
        assert!(session.is_expired(0));
        assert!(session.is_expired(1));
        session.expires_at = StoredSession::expires_at(1_000, 60);
        assert!(!session.is_expired(1_059));
        assert!(session.is_expired(1_060), "the boundary itself is expired");
        assert!(session.is_expired(1_061));
    }

    #[test]
    fn an_absurd_lifetime_clamps_instead_of_wrapping() {
        assert_eq!(StoredSession::expires_at(10, 0), 10);
        assert_eq!(StoredSession::expires_at(10, 5), 15);
        assert_eq!(
            StoredSession::expires_at(u64::MAX - 1, 10),
            u64::MAX,
            "a wrapped lifetime would read as expired for no visible reason"
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_saved_session_is_private_to_its_owner() {
        use std::os::unix::fs::PermissionsExt;

        let dir = temp_dir("private");
        let path = dir.join("session.json");
        save(&path, &a_session()).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o077, 0, "no group or other bits: {mode:o}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_session_file_that_is_a_link_is_refused_not_followed() {
        let dir = temp_dir("symlink");
        let real = dir.join("elsewhere.json");
        std::fs::write(&real, b"{}").unwrap();
        let link = dir.join("session.json");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert!(load(&link).is_err());
        assert!(clear(&link).is_err());
        assert!(real.exists(), "nothing was removed through the link");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
