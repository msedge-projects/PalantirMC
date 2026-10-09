//! Installing a version: its document and everything it needs on disk.
//!
//! An install is two things and no more: resolve a version document over
//! whatever it inherits (a loader profile names its game), then land the
//! resolved version with `palantir-net`'s syncer and write the resolved
//! document to `versions/<id>/<id>.json`. From that moment the install *is*
//! a vanilla version: `build_launch_plan` reads the written document and
//! knows nothing about how it got there.
//!
//! The document to install does not have to come from the version manifest:
//! a loader's profile (fetched from its vendor's meta service) enters at
//! `install_document`, and its `inheritsFrom` is resolved against the
//! manifest the same way.

use std::path::Path;

use palantir_core::rules::Platform;
use palantir_core::version::{Version, VersionManifest};
use palantir_net::cache::{MANIFEST_TTL, MetadataCache, VERSION_TTL};
use palantir_net::client::Http;
use palantir_net::sync::{SyncReport, Syncer};

use crate::error::{Error, Result};

/// Where the game's version manifest lives: Mojang's own metadata service,
/// the origin of the format.
pub const VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

/// What an install did.
#[derive(Debug)]
pub struct InstallReport {
    /// The resolved document as written to the data root.
    pub version: Version,
    /// What its needs did on the way in.
    pub sync: SyncReport,
}

/// The full document for one version id: its own metadata, resolved over
/// everything it inherits.
pub fn resolve_version(
    http: &Http,
    cache: &MetadataCache,
    manifest_url: &str,
    id: &str,
) -> Result<Version> {
    let version = Version::parse(&document_text(http, cache, manifest_url, id)?)?;
    resolve_chain(version, &|by_id| {
        let text = document_text(http, cache, manifest_url, by_id)?;
        Ok(Version::parse(&text)?)
    })
}

/// Land `document` -- a version document, possibly an overlay naming
/// `inheritsFrom` -- and everything the resolved version needs in the data
/// root. The resolved document is written to `versions/<id>/<id>.json`, so
/// what lands is a plain vanilla install.
///
/// `game_dir` is needed only by asset indexes that write into a game
/// directory (`map_to_resources`).
pub fn install_document(
    syncer: &Syncer<'_>,
    cache: &MetadataCache,
    manifest_url: &str,
    platform: &Platform,
    document: &str,
    game_dir: Option<&Path>,
) -> Result<InstallReport> {
    let version = Version::parse(document)?;
    let version = resolve_chain(version, &|by_id| {
        let text = document_text(syncer.http, cache, manifest_url, by_id)?;
        Ok(Version::parse(&text)?)
    })?;

    // Write the resolved document first: it is the receipt of the install
    // and the one file every later phase reads back.
    let dest = syncer.root.version_json(&version.id);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|source| Error::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(&dest, version.to_json_string()?).map_err(|source| Error::Io {
        path: dest.clone(),
        source,
    })?;

    let (sync, _client) = syncer.sync_version(&version, platform, game_dir)?;
    Ok(InstallReport { version, sync })
}

/// A document chain resolved downward: whatever `version` inherits is
/// fetched and resolved first, then merged under it. The result names no
/// parent.
fn resolve_chain(version: Version, fetch: &dyn Fn(&str) -> Result<Version>) -> Result<Version> {
    let Some(parent_id) = version.inherits_from.clone() else {
        return Ok(version);
    };
    let parent = fetch(&parent_id)?;
    let parent = resolve_chain(parent, fetch)?;
    Ok(version.merged_with(&parent)?)
}

/// The document text for one id, through the TTL cache: the manifest
/// points at it, the cache decides whether to ask the network.
fn document_text(
    http: &Http,
    cache: &MetadataCache,
    manifest_url: &str,
    id: &str,
) -> Result<String> {
    let manifest_text = cache.fetch_text(http, "version-manifest", manifest_url, MANIFEST_TTL)?;
    let manifest = VersionManifest::parse(&manifest_text)?;
    let entry = manifest.find(id).ok_or_else(|| Error::Invalid {
        what: "version manifest",
        why: format!("{id} is not in it"),
    })?;
    Ok(cache.fetch_text(http, &format!("version-{id}"), &entry.url, VERSION_TTL)?)
}
