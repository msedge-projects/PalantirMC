//! A TTL'd cache for the small metadata documents that move rarely.
//!
//! The version manifest points at everything and changes a few times a
//! week; a version's own metadata changes essentially never. Asking the
//! network for either on every window open is waste, and caching either
//! forever is how a launcher ends up telling someone a version does not
//! exist. So each document gets a time-to-live, chosen per kind and passed
//! in by the caller.
//!
//! Freshness comes from the fetch time stored *inside* the cache envelope,
//! not the file's mtime: mtimes lie after a copy, a restore, or a zip.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::client::Http;
use crate::error::{Error, Result};

/// The version manifest lists every published version and its URL, and new
/// versions move it. Fifteen minutes keeps a session's "is there an update?"
/// honest without asking more than a few times a day.
pub const MANIFEST_TTL: Duration = Duration::from_secs(15 * 60);

/// A version's own metadata is immutable in practice: released versions
/// never change, and the manifest carries a SHA-1 for the ones that might.
/// A day is the cautious choice; phase 3 can pin by hash instead.
pub const VERSION_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// The cache: one envelope file per key under one directory.
pub struct MetadataCache {
    dir: PathBuf,
}

/// What a cached document carries besides its body.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Envelope {
    /// Seconds since the Unix epoch when this body was fetched.
    fetched_at_unix: u64,
    body: String,
}

impl MetadataCache {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The cached body for `key` when it exists and is younger than `ttl`.
    pub fn read(&self, key: &str, ttl: Duration) -> Result<Option<String>> {
        let Some(envelope) = self.load(key)? else {
            return Ok(None);
        };
        let fetched = UNIX_EPOCH + Duration::from_secs(envelope.fetched_at_unix);
        let age = SystemTime::now()
            .duration_since(fetched)
            .unwrap_or(Duration::ZERO);
        if age < ttl {
            Ok(Some(envelope.body))
        } else {
            Ok(None)
        }
    }

    /// Store a body under `key`.
    pub fn write(&self, key: &str, body: &str) -> Result<()> {
        let envelope = Envelope {
            fetched_at_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_secs(),
            body: body.to_string(),
        };
        let path = self.path(key)?;
        std::fs::create_dir_all(&self.dir).map_err(|source| Error::Io {
            path: self.dir.clone(),
            source,
        })?;
        // Write-then-rename so an interrupted write cannot leave a half
        // envelope that reads as a valid, wrong document.
        let staging = path.with_extension("tmp");
        let bytes = serde_json::to_vec(&envelope).map_err(|source| Error::Io {
            path: staging.clone(),
            source: std::io::Error::other(source.to_string()),
        })?;
        std::fs::write(&staging, bytes).map_err(|source| Error::Io {
            path: staging.clone(),
            source,
        })?;
        std::fs::rename(&staging, &path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })
    }

    /// Cache-first text fetch: return the cached body if fresh, else fetch,
    /// store and return it. A network failure with a stale copy available is
    /// reported to the caller as an error -- a stale version list silently
    /// served is worse than an honest "could not check" -- so `read`'s
    /// None is the only path to the network.
    pub fn fetch_text(&self, http: &Http, key: &str, url: &str, ttl: Duration) -> Result<String> {
        if let Some(body) = self.read(key, ttl)? {
            return Ok(body);
        }
        let body = http.get(url)?.text()?;
        self.write(key, &body)?;
        Ok(body)
    }

    fn load(&self, key: &str) -> Result<Option<Envelope>> {
        let path = self.path(key)?;
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(Error::Io { path, source }),
        };
        // A corrupt envelope is a cache miss, not a failure: it is a
        // disposable copy and the network can replace it.
        Ok(serde_json::from_slice(&bytes).ok())
    }

    fn path(&self, key: &str) -> Result<PathBuf> {
        let safe = !key.is_empty()
            && key.len() <= 128
            && key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
        if !safe {
            return Err(Error::Invalid {
                what: "cache key",
                why: format!("{key:?} is not a plain file name"),
            });
        }
        Ok(self.dir.join(format!("{key}.json")))
    }
}

/// Where the cache for a data root lives. Kept here rather than in
/// `paths.rs` because the layout is the core crate's and the cache is
/// regenerable: losing it costs nothing.
pub fn cache_dir_for(root: &palantir_core::paths::DataRoot) -> PathBuf {
    root.cache_dir().join("metadata")
}

/// Test helper: is this path inside the cache directory? (Used by tests to
/// assert writes land where they should.)
pub fn is_under(dir: &Path, path: &Path) -> bool {
    path.starts_with(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_keys_are_file_names_or_refused() {
        let cache = MetadataCache::new("/tmp/x");
        assert!(cache.path("version-26.3").is_ok());
        assert!(cache.path("../../etc/passwd").is_err());
        assert!(cache.path("a/b").is_err());
        assert!(cache.path("").is_err());
        assert!(cache.path("a b").is_err());
    }

    #[test]
    fn an_envelope_reads_back_within_ttl_and_not_after() {
        let dir =
            std::env::temp_dir().join(format!("palantirmc-cache-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cache = MetadataCache::new(&dir);
        cache.write("doc", "hello").unwrap();
        assert_eq!(
            cache.read("doc", Duration::from_secs(60)).unwrap().unwrap(),
            "hello"
        );
        assert!(cache.read("doc", Duration::ZERO).unwrap().is_none());
        assert!(
            cache
                .read("missing", Duration::from_secs(60))
                .unwrap()
                .is_none()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_corrupt_envelope_is_a_miss_not_a_failure() {
        let dir =
            std::env::temp_dir().join(format!("palantirmc-cache-corrupt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cache = MetadataCache::new(&dir);
        cache.write("doc", "hello").unwrap();
        std::fs::write(cache.path("doc").unwrap(), b"not json").unwrap();
        assert!(
            cache
                .read("doc", Duration::from_secs(60))
                .unwrap()
                .is_none()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
