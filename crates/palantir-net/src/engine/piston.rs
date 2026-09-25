//! Mojang's own metadata: the version manifest, and a version file per version.
//!
//! This is the source the launcher has never used directly. The shell it
//! replaces reads `meta.prismlauncher.org`, which is a *mirror*: Prism fetches
//! piston, rewrites it into its own shape (adding an `order` key, filling in
//! defaults, dropping what it does not model) and serves that. A mirror is the
//! right answer for a launcher that wants Prism's patches; it is the wrong answer
//! for this one, because every field it drops is a field this launcher has to
//! guess at, and because a mirror that is a day behind is a launcher that does
//! not know a version was released.
//!
//! So two URLs, and they are the two Mojang publishes:
//!
//! * the **manifest** — `.../mc/game/version_manifest_v2.json`, a list of every
//!   version with its id, type, release time, URL and `sha1`. A list, so it is
//!   believed for [`crate::engine::cache::DEFAULT_TTL`] and revalidated rather
//!   than re-downloaded.
//! * a **version file** — `.../v1/packages/<sha1>/<id>.json`, the document the
//!   launcher actually works from: main class, libraries, asset index, arguments.
//!   Describing something released, so it is believed for [`IMMUTABLE_TTL`].
//!
//! ## Every version file is checked against the manifest's own digest
//!
//! The manifest publishes a `sha1` per version file, which is a check nothing
//! else in this launcher's metadata path has: it says the bytes that arrived are
//! the bytes Mojang meant, rather than merely that *something* arrived and parsed.
//! A version file is what decides which libraries are downloaded and which main
//! class is launched, so a body that parses but came from a truncated response or
//! a hostile mirror is exactly the failure worth catching before it reaches a
//! classpath. A mismatch forgets the cache entry rather than only reporting it, so
//! the next call refetches instead of being served the same bad bytes.
//!
//! ## Two handles over one cache directory
//!
//! A list and a version file do not age alike, and [`MetadataCache`] carries one
//! TTL, so this holds two handles over the same directory -- `cache` for the
//! manifest and `files` for the version files. Nothing else differs between them:
//! one directory, whose entries are told apart by URL, and two beliefs about how
//! long each kind stays true.

use std::path::PathBuf;
use std::sync::Arc;

use palantir_core::version::VersionFile;

use crate::engine::cache::{MetadataCache, IMMUTABLE_TTL};
use crate::engine::cancel::Cancel;
use crate::engine::content::Digest;
use crate::engine::request::Fetch;
use crate::engine::retry::Backoff;
use crate::Error;

/// Where Mojang publishes the list of every version.
pub const PISTON_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

/// One version, as the manifest describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestVersion {
    /// The id a user types and a folder is named after: `1.21.4`, `25w02a`.
    pub id: String,
    /// `release`, `snapshot`, `old_beta` or `old_alpha`, as published.
    pub type_: String,
    /// Where the version file is.
    pub url: String,
    /// The digest of that file, when the manifest gave one that can be read as a
    /// digest at all. `None` is not a licence to skip the download -- it is the
    /// absence of a check this launcher would rather have.
    pub sha1: Option<Digest>,
    /// When the version was released, as the manifest states it.
    pub release_time: String,
}

impl ManifestVersion {
    /// Whether this is a full release rather than a snapshot or an old build.
    pub fn is_release(&self) -> bool {
        self.type_ == "release"
    }
}

/// The version manifest, as the launcher needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// The newest full release, per Mojang. Not the same as the first release in
    /// `versions`: the list is ordered by release time but it is not this
    /// launcher's job to re-derive Mojang's answer from it.
    pub latest_release: String,
    /// The newest snapshot, per Mojang.
    pub latest_snapshot: String,
    /// Every version, in the order published.
    pub versions: Vec<ManifestVersion>,
}

impl Manifest {
    /// Read a manifest body.
    ///
    /// An entry without an `id` or without a `url` is skipped rather than
    /// failing the whole file: a list of a thousand versions is not worth
    /// throwing away because one of them is malformed, and a version with no URL
    /// is not a version a launcher can do anything with. A missing `latest` pair
    /// *is* a failure, because then the manifest cannot answer the one question
    /// it is asked first.
    pub fn parse(body: &str) -> Result<Manifest, Error> {
        let value: serde_json::Value = serde_json::from_str(body)
            .map_err(|error| Error::json("<piston manifest>", error.to_string()))?;
        let object = value
            .as_object()
            .ok_or_else(|| Error::format("<piston manifest>", "the root must be an object"))?;
        let latest = object
            .get("latest")
            .and_then(|latest| latest.as_object())
            .ok_or_else(|| Error::format("<piston manifest>", "no 'latest' object"))?;
        let latest_release = latest
            .get("release")
            .and_then(|id| id.as_str())
            .ok_or_else(|| Error::format("<piston manifest>", "no 'latest.release'"))?
            .to_string();
        let latest_snapshot = latest
            .get("snapshot")
            .and_then(|id| id.as_str())
            .ok_or_else(|| Error::format("<piston manifest>", "no 'latest.snapshot'"))?
            .to_string();

        let mut versions = Vec::new();
        for item in object.get("versions").and_then(|v| v.as_array()).into_iter().flatten() {
            let Some(entry) = item.as_object() else {
                continue;
            };
            let (Some(id), Some(url)) = (
                entry.get("id").and_then(|v| v.as_str()),
                entry.get("url").and_then(|v| v.as_str()),
            ) else {
                continue;
            };
            // A digest that cannot be read is kept as `None` rather than
            // dropping the version: the file is still fetchable, and the check
            // is one this launcher wants rather than one it requires.
            let sha1 = entry.get("sha1").and_then(|v| v.as_str()).and_then(|hex| Digest::parse(hex).ok());
            versions.push(ManifestVersion {
                id: id.to_string(),
                type_: entry.get("type").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                url: url.to_string(),
                sha1,
                release_time: entry
                    .get("releaseTime")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            });
        }
        if versions.is_empty() {
            return Err(Error::format("<piston manifest>", "the manifest lists no versions"));
        }
        Ok(Manifest { latest_release, latest_snapshot, versions })
    }

    /// The version with this id, if the manifest lists it.
    pub fn find(&self, id: &str) -> Option<&ManifestVersion> {
        self.versions.iter().find(|version| version.id == id)
    }

    /// The newest full release, as the manifest names it.
    pub fn newest_release(&self) -> Option<&ManifestVersion> {
        self.find(&self.latest_release)
    }

    /// The newest snapshot, as the manifest names it.
    pub fn newest_snapshot(&self) -> Option<&ManifestVersion> {
        self.find(&self.latest_snapshot)
    }

    /// Every full release, newest first as published.
    pub fn releases(&self) -> impl Iterator<Item = &ManifestVersion> {
        self.versions.iter().filter(|version| version.is_release())
    }
}

/// Mojang's metadata, read through the engine's cache.
pub struct PistonMeta {
    /// The list handle: believed for half an hour.
    cache: MetadataCache,
    /// The version-file handle, over the same directory: believed for a year.
    files: MetadataCache,
    fetch: Arc<dyn Fetch>,
    manifest_url: String,
}

impl std::fmt::Debug for PistonMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PistonMeta")
            .field("dir", &self.cache.dir())
            .field("manifest_url", &self.manifest_url)
            .finish_non_exhaustive()
    }
}

impl PistonMeta {
    /// Piston, caching under `cache`'s directory and fetching through `fetch`.
    pub fn new(cache: MetadataCache, fetch: Arc<dyn Fetch>) -> PistonMeta {
        let files = cache.clone().with_ttl(IMMUTABLE_TTL);
        PistonMeta { cache, files, fetch, manifest_url: PISTON_MANIFEST_URL.to_string() }
    }

    /// The same, with a different manifest URL: a test's own server, or a
    /// mirror this launcher is pointed at deliberately.
    pub fn with_manifest_url(mut self, url: impl Into<String>) -> PistonMeta {
        self.manifest_url = url.into();
        self
    }

    /// Where the manifest is read from.
    pub fn manifest_url(&self) -> &str {
        &self.manifest_url
    }

    /// Where the metadata is cached.
    pub fn cache_dir(&self) -> &std::path::Path {
        self.cache.dir()
    }

    /// The manifest, from the cache if it is fresh.
    ///
    /// A list, so [`DEFAULT_TTL`] applies: half an hour after a version is
    /// published is soon enough for a launcher to notice it, and until then the
    /// answer costs a header rather than the document.
    pub fn manifest(&self, cancel: &Cancel, backoff: &Backoff) -> Result<Manifest, Error> {
        let held = self.cache.get(&self.manifest_url, self.fetch.as_ref(), cancel, backoff)?;
        let text = String::from_utf8_lossy(&held.body).into_owned();
        Manifest::parse(&text)
    }

    /// The version file for `id`, verified against the manifest's own digest.
    ///
    /// Goes through the manifest even when the caller already has it, because the
    /// manifest is what says where the file is and what it should hash to: a
    /// caller that could name its own URL would be a caller that could skip the
    /// check. That costs nothing when the manifest is fresh, which it is for half
    /// an hour.
    pub fn version(
        &self,
        id: &str,
        cancel: &Cancel,
        backoff: &Backoff,
    ) -> Result<VersionFile, palantir_core::error::Error> {
        let manifest = self.manifest(cancel, backoff)?;
        let entry = manifest.find(id).ok_or_else(|| {
            palantir_core::error::Error::format(
                PathBuf::from(&self.manifest_url),
                format!("the manifest does not list a version called '{id}'"),
            )
        })?;
        let url = entry.url.clone();
        let expected = entry.sha1.clone();
        let held = self.files.get(&url, self.fetch.as_ref(), cancel, backoff)?;
        if let Some(expected) = &expected {
            if let Err(error) = verify(expected, &held.body, &url) {
                // Forget it as well as reporting it: the entry is wrong, and the
                // next call should fetch rather than be served it again.
                self.files.forget(&url);
                return Err(error.into());
            }
        }
        let text = String::from_utf8_lossy(&held.body).into_owned();
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|error| palantir_core::error::Error::json(&url, error.to_string()))?;
        // Piston's files carry no `order` key -- that is Prism's addition -- so a
        // missing one is the shape of the source rather than a warning.
        VersionFile::parse(&value, &PathBuf::from(&url), false)
    }

    /// The newest full release's version file.
    pub fn latest_release(
        &self,
        cancel: &Cancel,
        backoff: &Backoff,
    ) -> Result<(String, VersionFile), palantir_core::error::Error> {
        let manifest = self.manifest(cancel, backoff)?;
        let id = manifest.latest_release.clone();
        let file = self.version(&id, cancel, backoff)?;
        Ok((id, file))
    }
}

/// Check a body against a digest, naming the URL in the error.
///
/// `Digest::of` is what decides which algorithm to compute: the manifest named
/// one, and a caller that switched on the kind itself would be a caller that
/// could check a `sha1` with a `sha256` and call it a mismatch.
fn verify(digest: &Digest, body: &[u8], url: &str) -> Result<(), Error> {
    if digest.matches(body) {
        return Ok(());
    }
    Err(Error::hash_mismatch(url, digest.hex(), digest.of(body).hex()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::cache::DEFAULT_TTL;
    use crate::engine::request::{MapFetch, Route};

    /// A manifest with three versions: two releases and a snapshot, one entry
    /// deliberately malformed.
    fn manifest_body(sha1_of_the_version_file: &str) -> String {
        format!(
            r#"{{
              "latest": {{ "release": "1.21.4", "snapshot": "25w02a" }},
              "versions": [
                {{ "id": "25w02a", "type": "snapshot", "url": "https://piston.invalid/25w02a.json",
                   "time": "2025-01-08T10:00:00+00:00", "releaseTime": "2025-01-08T10:00:00+00:00",
                   "sha1": "{sha1_of_the_version_file}", "complianceLevel": 1 }},
                {{ "id": "1.21.4", "type": "release", "url": "https://piston.invalid/1.21.4.json",
                   "time": "2024-12-03T10:00:00+00:00", "releaseTime": "2024-12-03T10:00:00+00:00",
                   "sha1": "{sha1_of_the_version_file}", "complianceLevel": 1 }},
                {{ "id": "1.21.3", "type": "release", "url": "https://piston.invalid/1.21.3.json",
                   "releaseTime": "2024-10-23T10:00:00+00:00", "sha1": "not a digest" }},
                {{ "type": "release", "url": "https://piston.invalid/no-id.json" }}
              ]
            }}"#
        )
    }

    /// A version file the launcher can actually work from.
    const VERSION_BODY: &str = r#"{
        "id": "1.21.4", "type": "release", "mainClass": "net.minecraft.client.main.Main",
        "assets": "17", "complianceLevel": 1,
        "arguments": { "game": [], "jvm": [] },
        "libraries": [ { "name": "org.lwjgl:lwjgl:3.3.3" } ]
    }"#;

    /// A piston over a scratch directory, with a manifest URL no real server
    /// answers so a test that forgets to script one fails instead of dialling out.
    fn meta(name: &str, ttl: std::time::Duration) -> (PistonMeta, Arc<MapFetch>) {
        let root = std::env::temp_dir().join("palantirmc-engine-piston").join(name);
        let _ = std::fs::remove_dir_all(&root);
        let fetch = Arc::new(MapFetch::new());
        let cache = MetadataCache::new(root, ttl);
        let meta = PistonMeta::new(cache, fetch.clone())
            .with_manifest_url("https://piston.invalid/version_manifest_v2.json");
        (meta, fetch)
    }

    /// A fetcher with a manifest whose digest is the real one for the body.
    fn scripted(name: &str) -> (PistonMeta, Arc<MapFetch>) {
        let (meta, fetch) = meta(name, DEFAULT_TTL);
        let digest = Digest::sha1(VERSION_BODY.as_bytes()).hex().to_string();
        fetch.set_route(
            "https://piston.invalid/version_manifest_v2.json",
            Route::text(&manifest_body(&digest)),
        );
        fetch.set_route("https://piston.invalid/1.21.4.json", Route::text(VERSION_BODY));
        (meta, fetch)
    }

    #[test]
    fn the_manifest_reads_the_latest_pair_and_every_usable_version() {
        let digest = Digest::sha1(VERSION_BODY.as_bytes()).hex().to_string();
        let manifest = Manifest::parse(&manifest_body(&digest)).expect("a manifest");
        assert_eq!(manifest.latest_release, "1.21.4");
        assert_eq!(manifest.latest_snapshot, "25w02a");
        // The entry with no id is dropped; the one with an unreadable digest is
        // kept, with no check attached to it.
        assert_eq!(manifest.versions.len(), 3);
        assert_eq!(manifest.versions[0].id, "25w02a");
        assert!(!manifest.versions[0].is_release());
        assert_eq!(manifest.find("1.21.3").expect("an entry").sha1, None);
        assert!(manifest.find("nonsense").is_none());
        assert_eq!(manifest.newest_release().expect("the newest").id, "1.21.4");
        assert_eq!(manifest.newest_snapshot().expect("the newest").id, "25w02a");
        assert_eq!(manifest.releases().count(), 2);
        assert_eq!(manifest.versions[1].sha1.as_ref().map(Digest::hex), Some(digest.as_str()));
        assert_eq!(manifest.versions[1].sha1.as_ref().map(Digest::kind), Some("sha1"));
    }

    #[test]
    fn a_manifest_that_cannot_answer_what_is_newest_is_refused() {
        for body in [
            "{}",
            r#"{"versions":[{"id":"1.21.4","url":"u"}]}"#,
            r#"{"latest":{"release":"1.21.4"},"versions":[{"id":"1.21.4","url":"u"}]}"#,
            r#"{"latest":{"release":"1.21.4","snapshot":"25w02a"},"versions":[]}"#,
            "not json",
            "[]",
        ] {
            assert!(Manifest::parse(body).is_err(), "{body} parsed");
        }
    }

    #[test]
    fn a_version_file_is_fetched_and_checked_against_the_manifests_digest() {
        let (meta, fetch) = scripted("checked");
        let cancel = Cancel::new();
        let backoff = Backoff::with_attempts(1);
        let file = meta.version("1.21.4", &cancel, &backoff).expect("the version file");
        assert_eq!(file.main_class, "net.minecraft.client.main.Main");
        assert!(!file.has_order, "piston's files carry no order key");
        assert_eq!(file.libraries.len(), 1);
        assert_eq!(fetch.count(), 2, "the manifest, then the file");
        assert!(meta.cache_dir().exists(), "the cache directory was made");
    }

    #[test]
    fn a_version_file_that_does_not_match_the_published_digest_is_refused() {
        // A truncated body, a stale mirror, or a proxy with opinions: whatever it
        // was, these are not the bytes the manifest describes, and a version file
        // decides which libraries are downloaded and which class is launched.
        let (meta, fetch) = scripted("mismatch");
        let digest = Digest::sha1(b"the bytes Mojang published").hex().to_string();
        fetch.set_route(
            "https://piston.invalid/version_manifest_v2.json",
            Route::text(&manifest_body(&digest)),
        );
        let cancel = Cancel::new();
        let backoff = Backoff::with_attempts(1);
        let error = meta
            .version("1.21.4", &cancel, &backoff)
            .expect_err("a digest that does not match");
        let message = error.to_string();
        assert!(message.contains("mismatch"), "{message}");
        assert!(message.contains("1.21.4.json"), "it names what it checked: {message}");
    }

    #[test]
    fn a_mismatched_file_is_forgotten_so_the_next_call_fetches_it() {
        let (meta, fetch) = meta("forget", DEFAULT_TTL);
        // The manifest names a digest the body will not match, so the entry that
        // gets stored has to be dropped rather than served again.
        let digest = Digest::sha1(b"something else entirely").hex().to_string();
        fetch.set_route(
            "https://piston.invalid/version_manifest_v2.json",
            Route::text(&manifest_body(&digest)),
        );
        fetch.set_route("https://piston.invalid/1.21.4.json", Route::text(VERSION_BODY));
        let cancel = Cancel::new();
        let backoff = Backoff::with_attempts(1);
        assert!(meta.version("1.21.4", &cancel, &backoff).is_err());
        assert!(
            meta.files.cached("https://piston.invalid/1.21.4.json").is_none(),
            "the wrong bytes are not left where a later call would believe them"
        );
        // And once the manifest tells the truth, the same call works.
        let digest = Digest::sha1(VERSION_BODY.as_bytes()).hex().to_string();
        fetch.set_route(
            "https://piston.invalid/version_manifest_v2.json",
            Route::text(&manifest_body(&digest)),
        );
        // The manifest itself is cached and fresh, so the corrected one has to
        // come from a new call after forgetting it -- which is what a caller
        // who has been told the data is wrong would do.
        meta.cache.forget("https://piston.invalid/version_manifest_v2.json");
        let file = meta.version("1.21.4", &cancel, &backoff).expect("the version file");
        assert_eq!(file.main_class, "net.minecraft.client.main.Main");
    }

    #[test]
    fn a_version_the_manifest_does_not_name_is_an_error_and_asks_for_nothing() {
        let (meta, fetch) = scripted("unknown");
        let cancel = Cancel::new();
        let backoff = Backoff::with_attempts(1);
        let error = meta
            .version("1.99.9", &cancel, &backoff)
            .expect_err("a version that does not exist");
        assert!(error.to_string().contains("does not list a version"), "{error}");
        assert_eq!(fetch.count(), 1, "only the manifest was asked for");
    }

    #[test]
    fn a_version_file_is_believed_for_a_year_and_the_manifest_for_half_an_hour() {
        // The two TTLs made visible: three calls, one request for the list and
        // one for the file.
        let (meta, fetch) = scripted("ttls");
        let cancel = Cancel::new();
        let backoff = Backoff::with_attempts(1);
        for _ in 0..3 {
            meta.version("1.21.4", &cancel, &backoff).expect("the version file");
        }
        let asked: Vec<String> = fetch.requests().into_iter().map(|request| request.url).collect();
        assert_eq!(
            asked.iter().filter(|url| url.ends_with("/1.21.4.json")).count(),
            1,
            "a released version file is not fetched twice: {asked:?}"
        );
        assert_eq!(
            asked.iter().filter(|url| url.ends_with("version_manifest_v2.json")).count(),
            1,
            "and the list is fresh for half an hour: {asked:?}"
        );
    }
}
