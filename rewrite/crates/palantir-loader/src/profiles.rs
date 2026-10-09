//! Loader profiles from the loaders' own meta services.
//!
//! Fabric and Quilt both publish, per game version, a *listing* of loader
//! releases and a *launcher profile* for each: the overlay document that
//! names `inheritsFrom` and the loader's libraries. Installing one is that
//! document plus `install_document` -- the merge rules are the format's,
//! in `palantir-core`, and this module only knows where the documents are.
//!
//! Picking a release: the newest one the listing calls stable. Quilt's
//! listing marks no release stable at all (verified 2026-10-09), so there
//! the newest outright is taken -- the vendor's own ordering is the
//! strongest signal it gives. Callers who care pin the version themselves;
//! every function here accepts one.
//!
//! The URLs live in `LoaderSources`: the default is the vendor's own meta
//! service, and a mirror or an offline test substitutes the base while
//! keeping the vendor's paths.

use std::collections::BTreeMap;
use std::path::Path;

use palantir_core::rules::Platform;
use palantir_net::cache::{MANIFEST_TTL, MetadataCache, VERSION_TTL};
use palantir_net::client::Http;
use palantir_net::sync::Syncer;
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::install::{InstallReport, install_document};

/// A loader vendor that publishes profiles this way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoaderKind {
    Fabric,
    Quilt,
}

/// One vendor's meta service, at one base URL.
#[derive(Debug, Clone)]
pub struct LoaderSources {
    pub kind: LoaderKind,
    pub base: String,
}

impl LoaderSources {
    /// The vendor's own published service.
    pub fn vendor(kind: LoaderKind) -> Self {
        let base = match kind {
            LoaderKind::Fabric => "https://meta.fabricmc.net",
            LoaderKind::Quilt => "https://meta.quiltmc.org",
        };
        Self {
            kind,
            base: base.to_string(),
        }
    }

    /// The same service paths under another base: a mirror, or the mock
    /// server in tests.
    pub fn at(kind: LoaderKind, base: impl Into<String>) -> Self {
        Self {
            kind,
            base: base.into(),
        }
    }

    /// The listing of loader releases for one game version.
    pub fn listing_url(&self, game: &str) -> String {
        let path = match self.kind {
            LoaderKind::Fabric => format!("v2/versions/loader/{game}"),
            LoaderKind::Quilt => format!("v3/versions/loader/{game}"),
        };
        format!("{}/{}", self.base.trim_end_matches('/'), path)
    }

    /// One release's launcher profile for one game version.
    pub fn profile_url(&self, game: &str, loader: &str) -> String {
        let path = match self.kind {
            LoaderKind::Fabric => format!("v2/versions/loader/{game}/{loader}/profile/json"),
            LoaderKind::Quilt => format!("v3/versions/loader/{game}/{loader}/profile/json"),
        };
        format!("{}/{}", self.base.trim_end_matches('/'), path)
    }

    /// Cache keys name the vendor and the version; versions carry only
    /// characters that are safe file names.
    fn key(&self, what: &str, parts: &[&str]) -> String {
        let vendor = match self.kind {
            LoaderKind::Fabric => "fabric",
            LoaderKind::Quilt => "quilt",
        };
        format!("{vendor}-{what}-{}", parts.join("-"))
    }
}

/// One release in a listing. The listing entries wrap it under `loader`;
/// the other keys (`intermediary`, `launcherMeta`, ...) ride along
/// unmodelled.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LoaderRelease {
    pub version: String,
    /// Quilt's listing has no such field; Fabric's does.
    #[serde(default)]
    pub stable: Option<bool>,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
struct ListingEntry {
    loader: LoaderRelease,
}

/// Every release a vendor publishes for one game version, newest first as
/// the services order them.
pub fn list_loaders(
    http: &Http,
    cache: &MetadataCache,
    sources: &LoaderSources,
    game: &str,
) -> Result<Vec<LoaderRelease>> {
    let key = sources.key("loader-list", &[game]);
    let text = cache.fetch_text(http, &key, &sources.listing_url(game), MANIFEST_TTL)?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let entries: Vec<ListingEntry> =
        serde_json::from_str(text).map_err(|source| Error::Invalid {
            what: "loader listing",
            why: source.to_string(),
        })?;
    Ok(entries.into_iter().map(|entry| entry.loader).collect())
}

/// The newest release the listing calls stable; a listing that marks none
/// (Quilt's) yields its newest outright.
pub fn pick_loader(listings: &[LoaderRelease]) -> Option<&LoaderRelease> {
    listings
        .iter()
        .find(|release| release.stable == Some(true))
        .or_else(|| listings.first())
}

/// One release's launcher profile document, as the vendor published it.
pub fn fetch_profile(
    http: &Http,
    cache: &MetadataCache,
    sources: &LoaderSources,
    game: &str,
    loader: &str,
) -> Result<String> {
    let key = sources.key("loader-profile", &[game, loader]);
    Ok(cache.fetch_text(http, &key, &sources.profile_url(game, loader), VERSION_TTL)?)
}

/// What to install: which vendor's service, over which game, and (when a
/// modpack pins one) which loader release.
#[derive(Debug, Clone, Copy)]
pub struct LoaderInstall<'a> {
    pub sources: &'a LoaderSources,
    pub game: &'a str,
    /// A pinned release; `None` takes `pick_loader`'s choice.
    pub loader: Option<&'a str>,
    /// Only for indexes that write into a game directory.
    pub game_dir: Option<&'a Path>,
}

/// Install a loader over its game: the profile fetched and installed
/// resolved, so what lands is one vanilla-shaped version named after the
/// loader.
pub fn install_loader(
    syncer: &Syncer<'_>,
    cache: &MetadataCache,
    manifest_url: &str,
    request: LoaderInstall<'_>,
    platform: &Platform,
) -> Result<InstallReport> {
    let sources = request.sources;
    let game = request.game;
    let picked_version;
    let loader = match request.loader {
        Some(loader) => loader,
        None => {
            let listings = list_loaders(syncer.http, cache, sources, game)?;
            let picked = pick_loader(&listings).ok_or_else(|| Error::Invalid {
                what: "loader listing",
                why: format!("{game} has no loader releases at all"),
            })?;
            picked_version = picked.version.clone();
            &picked_version
        }
    };
    let profile = fetch_profile(syncer.http, cache, sources, game, loader)?;
    install_document(
        syncer,
        cache,
        manifest_url,
        platform,
        &profile,
        request.game_dir,
    )
}
