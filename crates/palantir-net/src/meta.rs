//! Online metadata store over `meta.prismlauncher.org`.
//!
//! [`OnlineMetaStore`] implements [`palantir_core::resolve::MetaStore`] with a
//! disk write-through cache in the exact
//! [`palantir_core::resolve::OfflineMetaStore`] layout:
//!
//! * `<cache_dir>/<uid>.json` — version list
//! * `<cache_dir>/<uid>/<version>.json` — version file
//!
//! On a cache hit the file is read from disk; on a miss the URL is fetched
//! with a timeout, written atomically, then parsed with
//! [`palantir_core::version::VersionFile::parse`] (meta files use
//! `require_order = value.get("order").is_some()`, mirroring the offline
//! store).
//!
//! Testability: URL-to-body mapping sits behind the [`Fetcher`] trait.
//! Production uses [`BlockingHttpFetcher`] (`reqwest::blocking`); tests use
//! [`MapFetcher`].

use palantir_core::pack::Require;
use palantir_core::resolve::{MetaStore, VersionEntry};
use palantir_core::version::VersionFile;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Default Prism metadata base URL (`.../v1` prefix included).
pub const DEFAULT_META_BASE_URL: &str = "https://meta.prismlauncher.org/v1";

/// Default per-request timeout used by [`OnlineMetaStore::new`].
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Abstract URL-to-bytes mapping so metadata fetches are unit-testable
/// offline.
///
/// `Sync` is a supertrait so a single fetcher can be shared across the
/// scoped worker threads used by [`crate::download::download_many`];
/// [`BlockingHttpFetcher`] and [`MapFetcher`] are both `Sync`.
pub trait Fetcher: Sync {
    /// Fetch the full response body for `url`.
    ///
    /// Implementations must apply their own timeout and must return
    /// [`crate::Error::Http`] (or [`crate::Error::Io`] for cache-adjacent
    /// failures) on any transport problem.
    fn fetch(&self, url: &str) -> Result<Vec<u8>, crate::Error>;
}

/// Blocking HTTP fetcher backed by `reqwest`.
///
/// Prism parity: Prism's `Meta` index uses a single shared `QNetworkAccessManager`
/// with a per-request timeout; here one `reqwest::blocking::Client` is shared
/// and [`std::time::Duration`] is applied per request.
#[derive(Debug, Clone)]
pub struct BlockingHttpFetcher {
    client: reqwest::blocking::Client,
    timeout: Duration,
}

impl BlockingHttpFetcher {
    /// Create a fetcher with the given per-request `timeout`.
    ///
    /// The underlying client is `reqwest::blocking::Client::new()` (infallible);
    /// `timeout` is enforced on every request via
    /// `RequestBuilder::timeout`.
    pub fn new(timeout: Duration) -> Self {
        BlockingHttpFetcher { client: reqwest::blocking::Client::new(), timeout }
    }

    /// Return the per-request timeout this fetcher enforces.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    fn from_client(client: reqwest::blocking::Client, timeout: Duration) -> Self {
        BlockingHttpFetcher { client, timeout }
    }
}

impl Fetcher for BlockingHttpFetcher {
    fn fetch(&self, url: &str) -> Result<Vec<u8>, crate::Error> {
        let response = self
            .client
            .get(url)
            .timeout(self.timeout)
            .send()
            .map_err(|e| crate::Error::http(url, e.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(crate::Error::http(url, format!("http status {status}")));
        }
        response.bytes().map(|b| b.to_vec()).map_err(|e| crate::Error::http(url, e.to_string()))
    }
}

/// In-memory URL-to-body map for offline unit tests.
///
/// Missing URLs yield [`crate::Error::Http`], mirroring a 404/transport
/// failure without touching the network.
#[derive(Debug, Clone, Default)]
pub struct MapFetcher {
    map: HashMap<String, Vec<u8>>,
}

impl MapFetcher {
    /// Create an empty map fetcher.
    pub fn new() -> Self {
        MapFetcher { map: HashMap::new() }
    }

    /// Insert a URL body mapping, replacing any previous entry.
    pub fn insert(&mut self, url: impl Into<String>, body: impl Into<Vec<u8>>) {
        self.map.insert(url.into(), body.into());
    }

    /// Insert a URL body from a `&str` body (convenience for JSON fixtures).
    pub fn insert_str(&mut self, url: impl Into<String>, body: &str) {
        self.map.insert(url.into(), body.as_bytes().to_vec());
    }

    /// Return how many URL mappings are stored.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Return `true` when no URL mappings are stored.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

impl Fetcher for MapFetcher {
    fn fetch(&self, url: &str) -> Result<Vec<u8>, crate::Error> {
        self.map
            .get(url)
            .cloned()
            .ok_or_else(|| crate::Error::http(url, "no mocked response for url"))
    }
}

/// Online metadata store with a disk write-through cache.
///
/// Prism parity: mirrors `Meta::Index` + `Meta::VersionList` fetch logic —
/// version lists live at `{base}/{uid}.json`, version files at
/// `{base}/{uid}/{version}.json`, and successful fetches are persisted into
/// the meta-cache layout so later runs (and
/// [`palantir_core::resolve::OfflineMetaStore`]) can read them offline.
#[derive(Debug, Clone)]
pub struct OnlineMetaStore {
    base_url: String,
    cache_dir: PathBuf,
    timeout: Duration,
    client: reqwest::blocking::Client,
}

impl OnlineMetaStore {
    /// Create a store fetching from `base_url` and caching under `cache_dir`.
    ///
    /// A trailing `/` on `base_url` is stripped; an empty `base_url` falls back
    /// to [`DEFAULT_META_BASE_URL`]. The default timeout is
    /// [`DEFAULT_TIMEOUT_SECS`] seconds (see [`OnlineMetaStore::with_timeout`]).
    pub fn new(base_url: impl Into<String>, cache_dir: impl Into<PathBuf>) -> Self {
        let raw = base_url.into();
        let trimmed = raw.trim_end_matches('/').to_string();
        let base_url = if trimmed.is_empty() { DEFAULT_META_BASE_URL.to_string() } else { trimmed };
        OnlineMetaStore {
            base_url,
            cache_dir: cache_dir.into(),
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            client: reqwest::blocking::Client::new(),
        }
    }

    /// Set the per-request network timeout, returning the updated store.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Return the configured metadata base URL (no trailing slash).
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Return the disk cache directory.
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Return the per-request network timeout.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Build the version-file URL for a uid/version pair.
    pub fn version_file_url(&self, uid: &str, version: &str) -> String {
        format!("{}/{}/{}.json", self.base_url, uid, version)
    }

    /// Build the version-list URL for a uid.
    pub fn version_list_url(&self, uid: &str) -> String {
        format!("{}/{}.json", self.base_url, uid)
    }

    /// Load a version file using `fetcher` (cache hit reads disk, miss fetches
    /// then writes through). Unit-testable offline with [`MapFetcher`].
    pub fn version_file_with(
        &self,
        uid: &str,
        version: &str,
        fetcher: &dyn Fetcher,
    ) -> Result<VersionFile, palantir_core::error::Error> {
        let url = self.version_file_url(uid, version);
        let cache_path = self.version_cache_path(uid, version);
        let bytes = self.load_or_fetch(&url, &cache_path, fetcher)?;
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| palantir_core::error::Error::json(&cache_path, e.to_string()))?;
        VersionFile::parse(&value, &cache_path, value.get("order").is_some())
    }

    /// Load a version list using `fetcher` (cache hit reads disk, miss fetches
    /// then writes through). Unit-testable offline with [`MapFetcher`].
    pub fn version_list_with(
        &self,
        uid: &str,
        fetcher: &dyn Fetcher,
    ) -> Result<Vec<VersionEntry>, palantir_core::error::Error> {
        let url = self.version_list_url(uid);
        let cache_path = self.list_cache_path(uid);
        let bytes = self.load_or_fetch(&url, &cache_path, fetcher)?;
        parse_version_list(&bytes, &cache_path, uid)
    }

    fn version_cache_path(&self, uid: &str, version: &str) -> PathBuf {
        self.cache_dir.join(uid).join(format!("{version}.json"))
    }

    fn list_cache_path(&self, uid: &str) -> PathBuf {
        self.cache_dir.join(format!("{uid}.json"))
    }

    fn load_or_fetch(
        &self,
        url: &str,
        cache_path: &Path,
        fetcher: &dyn Fetcher,
    ) -> Result<Vec<u8>, palantir_core::error::Error> {
        if cache_path.exists() {
            match std::fs::read(cache_path) {
                Ok(bytes) => return Ok(bytes),
                Err(e) => return Err(palantir_core::error::Error::io(cache_path, e)),
            }
        }
        let bytes: Vec<u8> =
            fetcher.fetch(url).map_err(|e| -> palantir_core::error::Error { e.into() })?;
        if let Some(parent) = cache_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| palantir_core::error::Error::io(parent, e))?;
            }
        }
        palantir_core::util::atomic_write(cache_path, &bytes)?;
        Ok(bytes)
    }
}

impl MetaStore for OnlineMetaStore {
    fn version_file(
        &mut self,
        uid: &str,
        version: &str,
    ) -> Result<VersionFile, palantir_core::error::Error> {
        let fetcher = BlockingHttpFetcher::from_client(self.client.clone(), self.timeout);
        self.version_file_with(uid, version, &fetcher)
    }

    fn version_list(
        &mut self,
        uid: &str,
    ) -> Result<Vec<VersionEntry>, palantir_core::error::Error> {
        let fetcher = BlockingHttpFetcher::from_client(self.client.clone(), self.timeout);
        self.version_list_with(uid, &fetcher)
    }
}

/// Parse a version-list payload exactly like
/// `OfflineMetaStore::version_list` (unknown uid handled by the caller via
/// fetch errors; cached payloads follow the same `formatVersion` rules).
fn parse_version_list(
    bytes: &[u8],
    path: &Path,
    uid: &str,
) -> Result<Vec<VersionEntry>, palantir_core::error::Error> {
    let text = String::from_utf8_lossy(bytes).into_owned();
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| palantir_core::error::Error::json(path, e.to_string()))?;
    let obj = value
        .as_object()
        .ok_or_else(|| palantir_core::error::Error::json(path, "version list root must be an object"))?;
    let fv = obj.get("formatVersion").and_then(|v| v.as_i64()).unwrap_or(0);
    if fv != 0 && fv != 1 {
        return Err(palantir_core::error::Error::format(
            path,
            format!("unknown metadata format version {fv}"),
        ));
    }
    let mut out = Vec::new();
    if let Some(items) = obj.get("versions").and_then(|v| v.as_array()) {
        for item in items {
            let Some(o) = item.as_object() else { continue };
            let requires = o
                .get("requires")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(Require::from_json).collect())
                .unwrap_or_default();
            let conflicts = o
                .get("conflicts")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(Require::from_json).collect())
                .unwrap_or_default();
            out.push(VersionEntry {
                uid: uid.to_string(),
                version: o
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                type_: o.get("type").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                recommended: o.get("recommended").and_then(|v| v.as_bool()).unwrap_or(false),
                volatile: o.get("volatile").and_then(|v| v.as_bool()).unwrap_or(false),
                release_time: o
                    .get("releaseTime")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                requires,
                conflicts,
                sha256: o.get("sha256").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_in(dir: &Path) -> OnlineMetaStore {
        OnlineMetaStore::new("https://meta.example.invalid/v1", dir.join("meta"))
    }

    #[test]
    fn blocking_fetcher_new_keeps_timeout() {
        let f = BlockingHttpFetcher::new(Duration::from_secs(7));
        assert_eq!(f.timeout(), Duration::from_secs(7));
    }

    #[test]
    fn blocking_fetcher_fetch_reports_http_error_offline() {
        // Unroutable documentation address with a tiny timeout: must fail
        // without hanging, proving the timeout path maps into Error::Http.
        let f = BlockingHttpFetcher::new(Duration::from_millis(800));
        let err = f.fetch("http://192.0.2.1/meta.json").unwrap_err();
        match err {
            crate::Error::Http { url, .. } => assert_eq!(url, "http://192.0.2.1/meta.json"),
            _ => panic!("expected Http error"),
        }
    }

    #[test]
    fn map_fetcher_new_is_empty() {
        let f = MapFetcher::new();
        assert!(f.is_empty());
        assert_eq!(f.len(), 0);
    }

    #[test]
    fn map_fetcher_insert_and_fetch_roundtrip() {
        let mut f = MapFetcher::new();
        f.insert("https://x/y.json", b"hello".to_vec());
        assert_eq!(f.len(), 1);
        assert!(!f.is_empty());
        assert_eq!(f.fetch("https://x/y.json").unwrap(), b"hello");
    }

    #[test]
    fn map_fetcher_insert_str_stores_utf8() {
        let mut f = MapFetcher::new();
        f.insert_str("https://x/a.json", r#"{"a":1}"#);
        assert_eq!(f.fetch("https://x/a.json").unwrap(), br#"{"a":1}"#.to_vec());
    }

    #[test]
    fn map_fetcher_len_counts_entries() {
        let mut f = MapFetcher::new();
        f.insert_str("https://x/1", "a");
        f.insert_str("https://x/2", "b");
        assert_eq!(f.len(), 2);
    }

    #[test]
    fn map_fetcher_is_empty_after_new() {
        assert!(MapFetcher::default().is_empty());
    }

    #[test]
    fn map_fetcher_missing_url_errors() {
        let f = MapFetcher::new();
        let err = f.fetch("https://x/missing.json").unwrap_err();
        match err {
            crate::Error::Http { url, .. } => assert_eq!(url, "https://x/missing.json"),
            _ => panic!("expected Http"),
        }
    }

    #[test]
    fn store_new_trims_trailing_slash_and_sets_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let s = OnlineMetaStore::new("https://meta.example.invalid/v1///", tmp.path());
        assert_eq!(s.base_url(), "https://meta.example.invalid/v1");
        assert_eq!(s.cache_dir(), tmp.path());
        assert_eq!(s.timeout(), Duration::from_secs(DEFAULT_TIMEOUT_SECS));
    }

    #[test]
    fn store_new_empty_base_falls_back_to_default() {
        let tmp = tempfile::tempdir().unwrap();
        let s = OnlineMetaStore::new("", tmp.path());
        assert_eq!(s.base_url(), DEFAULT_META_BASE_URL);
    }

    #[test]
    fn store_with_timeout_overrides() {
        let tmp = tempfile::tempdir().unwrap();
        let s = OnlineMetaStore::new("https://m/v1", tmp.path())
            .with_timeout(Duration::from_secs(3));
        assert_eq!(s.timeout(), Duration::from_secs(3));
    }

    #[test]
    fn store_base_url_accessor() {
        let tmp = tempfile::tempdir().unwrap();
        let s = OnlineMetaStore::new("https://m/v1", tmp.path());
        assert_eq!(s.base_url(), "https://m/v1");
    }

    #[test]
    fn store_cache_dir_accessor() {
        let tmp = tempfile::tempdir().unwrap();
        let s = OnlineMetaStore::new("https://m/v1", tmp.path().join("c"));
        assert_eq!(s.cache_dir(), tmp.path().join("c").as_path());
    }

    #[test]
    fn store_timeout_accessor_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let s = OnlineMetaStore::new("https://m/v1", tmp.path());
        assert_eq!(s.timeout(), Duration::from_secs(DEFAULT_TIMEOUT_SECS));
    }

    #[test]
    fn version_file_url_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let s = OnlineMetaStore::new("https://meta.example.invalid/v1", tmp.path());
        assert_eq!(
            s.version_file_url("net.minecraft", "1.20.4"),
            "https://meta.example.invalid/v1/net.minecraft/1.20.4.json"
        );
    }

    #[test]
    fn version_list_url_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let s = OnlineMetaStore::new("https://meta.example.invalid/v1", tmp.path());
        assert_eq!(
            s.version_list_url("net.minecraft"),
            "https://meta.example.invalid/v1/net.minecraft.json"
        );
    }

    #[test]
    fn version_file_with_fetches_and_caches() {
        let tmp = tempfile::tempdir().unwrap();
        let s = store_in(tmp.path());
        let url = s.version_file_url("net.minecraft", "1.20.4");
        let mut f = MapFetcher::new();
        f.insert_str(
            &url,
            r#"{"formatVersion":1,"uid":"net.minecraft","version":"1.20.4","order":0,"mainClass":"M"}"#,
        );
        let vf = s.version_file_with("net.minecraft", "1.20.4", &f).unwrap();
        assert_eq!(vf.main_class, "M");
        assert!(vf.has_order);
        // write-through: cache file exists in OfflineMetaStore layout
        let cached = tmp.path().join("meta").join("net.minecraft").join("1.20.4.json");
        assert!(cached.exists());
        // second store with empty fetcher hits disk, not network
        let s2 = store_in(tmp.path());
        let vf2 = s2.version_file_with("net.minecraft", "1.20.4", &MapFetcher::new()).unwrap();
        assert_eq!(vf2.main_class, "M");
    }

    #[test]
    fn version_file_with_require_order_matches_offline_semantics() {
        // No "order" key -> require_order=false, parses without warning-as-error.
        let tmp = tempfile::tempdir().unwrap();
        let s = store_in(tmp.path());
        let url = s.version_file_url("org.lwjgl3", "3.3.2");
        let mut f = MapFetcher::new();
        f.insert_str(&url, r#"{"uid":"org.lwjgl3","version":"3.3.2"}"#);
        let vf = s.version_file_with("org.lwjgl3", "3.3.2", &f).unwrap();
        assert!(!vf.has_order);
    }

    #[test]
    fn version_file_with_propagates_fetcher_error_as_core_format() {
        let tmp = tempfile::tempdir().unwrap();
        let s = store_in(tmp.path());
        let err = s.version_file_with("nope", "0", &MapFetcher::new()).unwrap_err();
        match err {
            palantir_core::error::Error::Format { detail, .. } => {
                assert!(detail.contains("no mocked response"))
            }
            _ => panic!("expected core Format, got {err:?}"),
        }
    }

    #[test]
    fn version_list_with_fetches_and_caches() {
        let tmp = tempfile::tempdir().unwrap();
        let s = store_in(tmp.path());
        let url = s.version_list_url("net.minecraft");
        let mut f = MapFetcher::new();
        f.insert_str(
            &url,
            r#"{"formatVersion":1,"versions":[{"version":"1.20.4","type":"release","recommended":true}]}"#,
        );
        let list = s.version_list_with("net.minecraft", &f).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].version, "1.20.4");
        assert!(list[0].recommended);
        let cached = tmp.path().join("meta").join("net.minecraft.json");
        assert!(cached.exists());
        // offline hit
        let s2 = store_in(tmp.path());
        let list2 = s2.version_list_with("net.minecraft", &MapFetcher::new()).unwrap();
        assert_eq!(list2.len(), 1);
    }

    #[test]
    fn version_list_with_rejects_unknown_format() {
        let tmp = tempfile::tempdir().unwrap();
        let s = store_in(tmp.path());
        let url = s.version_list_url("x");
        let mut f = MapFetcher::new();
        f.insert_str(&url, r#"{"formatVersion":99,"versions":[]}"#);
        let err = s.version_list_with("x", &f).unwrap_err();
        match err {
            palantir_core::error::Error::Format { detail, .. } => {
                assert!(detail.contains("unknown metadata format version"))
            }
            _ => panic!("expected Format"),
        }
    }
}
