//! Dynamic version catalog: Minecraft releases and mod-loader builds.
//!
//! The Create Instance dialog must offer *real* choices, so this module talks
//! to the same metadata service Prism Launcher uses
//! (`meta.prismlauncher.org/v1`) and caches what it downloads in Prism's own
//! layout (`<meta>/<uid>/index.json`), so a later offline run — or Prism
//! itself, when the two share a data root — reads the same files.
//!
//! Layout note (verified against the live service): a version list now lives
//! at `{base}/{uid}/index.json`; the older flat `{base}/{uid}.json` still
//! exists in caches written by older Prism builds, so the offline fallback
//! reads both.
//!
//! Loader semantics (also verified live, and encoded in
//! [`filter_loader_versions`]):
//!
//! * Forge and NeoForge publish one build set *per game version*, expressed as
//!   `requires: [{ uid: "net.minecraft", equals: "<game>" }]`.
//! * Fabric and Quilt loader builds are game-agnostic: their `requires` only
//!   mention `net.fabricmc.intermediary`, so every listed build is valid for
//!   any game version.
//! * Every list ships newest-first and marks its recommended build with
//!   `recommended: true` — that is the "Stable" choice in the dialog, while
//!   "Latest" is simply the newest entry.
//!
//! Everything except [`fetch`] is pure, so the filtering rules are unit-tested
//! without a network.

use std::path::{Path, PathBuf};

use palantir_core::pack::Require;
use palantir_core::resolve::VersionEntry;
use palantir_net::meta::Fetcher;
use serde_json::Value;

/// uid of the Minecraft component.
#[cfg(test)]
pub const MINECRAFT_UID: &str = "net.minecraft";

/// How many builds the "Other" loader-version dropdown offers.
pub const MAX_OTHER_BUILDS: usize = 60;

/// A mod loader a new instance can be created with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum LoaderKind {
    /// Plain Minecraft, no loader.
    #[default]
    Vanilla,
    /// Fabric (`net.fabricmc.fabric-loader`).
    Fabric,
    /// NeoForge (`net.neoforged`).
    NeoForge,
    /// Minecraft Forge (`net.minecraftforge`).
    Forge,
    /// Quilt (`org.quiltmc.quilt-loader`).
    Quilt,
}

impl LoaderKind {
    /// Every loader, in dialog order.
    pub fn all() -> [LoaderKind; 5] {
        [
            LoaderKind::Vanilla,
            LoaderKind::Fabric,
            LoaderKind::NeoForge,
            LoaderKind::Forge,
            LoaderKind::Quilt,
        ]
    }

    /// Label shown on the chip.
    pub fn label(self) -> &'static str {
        match self {
            LoaderKind::Vanilla => "Vanilla",
            LoaderKind::Fabric => "Fabric",
            LoaderKind::NeoForge => "NeoForge",
            LoaderKind::Forge => "Forge",
            LoaderKind::Quilt => "Quilt",
        }
    }

    /// Prism component uid, or `None` for vanilla.
    pub fn uid(self) -> Option<&'static str> {
        match self {
            LoaderKind::Vanilla => None,
            LoaderKind::Fabric => Some("net.fabricmc.fabric-loader"),
            LoaderKind::NeoForge => Some("net.neoforged"),
            LoaderKind::Forge => Some("net.minecraftforge"),
            LoaderKind::Quilt => Some("org.quiltmc.quilt-loader"),
        }
    }

    /// Loader name as used by the Modrinth API (`fabric`, `neoforge`, ...).
    pub fn modrinth_name(self) -> &'static str {
        match self {
            LoaderKind::Vanilla => "vanilla",
            LoaderKind::Fabric => "fabric",
            LoaderKind::NeoForge => "neoforge",
            LoaderKind::Forge => "forge",
            LoaderKind::Quilt => "quilt",
        }
    }

    /// Reverse of [`LoaderKind::label`], for instance summaries.
    #[cfg(test)]
    pub fn from_label(label: &str) -> Option<LoaderKind> {
        Self::all().into_iter().find(|kind| kind.label().eq_ignore_ascii_case(label))
    }

    /// Reverse of [`LoaderKind::uid`].
    pub fn from_uid(uid: &str) -> Option<LoaderKind> {
        Self::all().into_iter().find(|kind| kind.uid() == Some(uid))
    }

    /// Loaders that can actually load mods (everything but vanilla).
    pub fn loads_mods(self) -> bool {
        self.uid().is_some()
    }
}


// ---- Pure list helpers -------------------------------------------------

/// Newest-first ordering key: the date part of an ISO-8601 timestamp. Missing
/// timestamps sort last, so source order wins for undated entries.
#[cfg(test)]
fn date_key(entry: &VersionEntry) -> String {
    let raw = entry.release_time.trim();
    if raw.is_empty() {
        String::new()
    } else {
        raw.chars().take(10).collect()
    }
}

/// Sort a slice newest-first by [`date_key`] (stable: ties keep source order).
#[cfg(test)]
pub fn sort_newest_first(entries: &mut [VersionEntry]) {
    entries.sort_by(|a, b| date_key(b).cmp(&date_key(a)));
}

/// Copy + sort newest-first.
#[cfg(test)]
pub fn sorted_newest_first(entries: &[VersionEntry]) -> Vec<VersionEntry> {
    let mut out = entries.to_vec();
    sort_newest_first(&mut out);
    out
}

/// Whether `entry` pins `uid` to `version` through a `requires` clause.
#[cfg(test)]
pub fn pins_version(entry: &VersionEntry, uid: &str, version: &str) -> bool {
    entry
        .requires
        .iter()
        .any(|require| require.uid == uid && !require.equals_version.is_empty() && require.equals_version == version)
}

/// The set of game versions an entry pins itself to (usually zero or one).
#[cfg(test)]
pub fn pinned_games(entry: &VersionEntry) -> Vec<String> {
    entry
        .requires
        .iter()
        .filter(|require| require.uid == MINECRAFT_UID && !require.equals_version.is_empty())
        .map(|require| require.equals_version.clone())
        .collect()
}

/// Loader builds valid for `game`, newest first.
///
/// * Entries that pin `net.minecraft` to `game` always qualify.
/// * When the list is game-agnostic (no entry pins Minecraft at all — Fabric
///   and Quilt), every entry qualifies.
/// * When the list is game-specific but has nothing for `game`, the result is
///   empty and the dialog says so instead of offering a build that cannot work.
#[cfg(test)]
pub fn filter_loader_versions(entries: &[VersionEntry], game: &str) -> Vec<VersionEntry> {
    let game_specific = entries.iter().any(|entry| !pinned_games(entry).is_empty());
    let matching: Vec<VersionEntry> = entries
        .iter()
        .filter(|entry| pins_version(entry, MINECRAFT_UID, game))
        .cloned()
        .collect();
    if !matching.is_empty() {
        return sorted_newest_first(&matching);
    }
    if game_specific {
        return Vec::new();
    }
    sorted_newest_first(entries)
}

// ---- Fetching ----------------------------------------------------------

/// Where a uid's version list lives on the metadata service.
pub fn list_url(base_url: &str, uid: &str) -> String {
    format!("{}/{}/index.json", base_url.trim_end_matches('/'), uid)
}

/// Cache paths to try for `uid`, in preference order: Prism's current layout
/// first, then the flat layout older builds (and `palantir-net`) wrote.
pub fn cache_paths(meta_dir: &Path, uid: &str) -> [PathBuf; 2] {
    [meta_dir.join(uid).join("index.json"), meta_dir.join(format!("{uid}.json"))]
}

/// Fetch a version list for `uid`: network first, cache second. Returns the
/// parsed entries and whether the cache had to be used.
pub fn fetch_list(
    base_url: &str,
    meta_dir: &Path,
    uid: &str,
    fetcher: &dyn Fetcher,
) -> Result<(Vec<VersionEntry>, bool), String> {
    let url = list_url(base_url, uid);
    let remote = fetcher.fetch(&url).and_then(|bytes| {
        let parsed = parse_version_list(&bytes, &PathBuf::from(&url), uid)
            .map_err(|detail| palantir_net::Error::format(&url, detail))?;
        Ok((bytes, parsed))
    });
    match remote {
        Ok((bytes, parsed)) if !parsed.is_empty() => {
            let cache = meta_dir.join(uid).join("index.json");
            // `atomic_write` writes beside its target, so on a fresh install —
            // where the cache folder does not exist yet — it must be created
            // first, or every list silently loses its offline copy.
            if let Some(dir) = cache.parent() {
                let _ = palantir_core::util::ensure_dir(dir);
            }
            if let Err(error) = palantir_core::util::atomic_write(&cache, &bytes) {
                // A read-only cache is not fatal: the list is already parsed.
                let _ = error;
            }
            Ok((parsed, false))
        }
        Ok((_bytes, _empty)) => Err(format!("{uid}: metadata service returned no versions")),
        Err(remote_error) => match read_cached_list(meta_dir, uid) {
            Ok(entries) => Ok((entries, true)),
            Err(cache_error) => Err(format!("{uid}: {remote_error} (cache: {cache_error})")),
        },
    }
}

/// Read a cached list from either supported layout.
pub fn read_cached_list(meta_dir: &Path, uid: &str) -> Result<Vec<VersionEntry>, String> {
    let mut last = String::from("no cached copy");
    for path in cache_paths(meta_dir, uid) {
        match std::fs::read(&path) {
            Ok(bytes) => match parse_version_list(&bytes, &path, uid) {
                Ok(entries) if !entries.is_empty() => return Ok(entries),
                Ok(_) => last = format!("{}: empty version list", path.display()),
                Err(detail) => last = detail,
            },
            Err(error) => last = format!("{}: {error}", path.display()),
        }
    }
    Err(last)
}

/// Parse a `formatVersion` 1 version list (same rules as the metadata store:
/// the root must be an object, `versions` an array, `formatVersion` 0 or 1).
pub fn parse_version_list(bytes: &[u8], path: &Path, uid: &str) -> Result<Vec<VersionEntry>, String> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let object = value
        .as_object()
        .ok_or_else(|| format!("{}: version list root must be an object", path.display()))?;
    let format_version = object.get("formatVersion").and_then(Value::as_i64).unwrap_or(0);
    if format_version != 0 && format_version != 1 {
        return Err(format!(
            "{}: unknown metadata format version {format_version}",
            path.display()
        ));
    }
    let mut out = Vec::new();
    for item in object.get("versions").and_then(Value::as_array).into_iter().flatten() {
        let Some(entry) = item.as_object() else { continue };
        out.push(VersionEntry {
            uid: uid.to_string(),
            version: entry.get("version").and_then(Value::as_str).unwrap_or_default().to_string(),
            type_: entry.get("type").and_then(Value::as_str).unwrap_or_default().to_string(),
            recommended: entry.get("recommended").and_then(Value::as_bool).unwrap_or(false),
            volatile: entry.get("volatile").and_then(Value::as_bool).unwrap_or(false),
            release_time: entry
                .get("releaseTime")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            requires: entry
                .get("requires")
                .and_then(Value::as_array)
                .map(|items| items.iter().filter_map(Require::from_json).collect())
                .unwrap_or_default(),
            conflicts: entry
                .get("conflicts")
                .and_then(Value::as_array)
                .map(|items| items.iter().filter_map(Require::from_json).collect())
                .unwrap_or_default(),
            sha256: entry.get("sha256").and_then(Value::as_str).unwrap_or_default().to_string(),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(version: &str, type_: &str, time: &str, recommended: bool, game: Option<&str>) -> VersionEntry {
        VersionEntry {
            uid: "test".to_string(),
            version: version.to_string(),
            type_: type_.to_string(),
            recommended,
            release_time: time.to_string(),
            requires: match game {
                Some(game) => vec![Require {
                    uid: MINECRAFT_UID.to_string(),
                    equals_version: game.to_string(),
                    ..Default::default()
                }],
                None => Vec::new(),
            },
            ..Default::default()
        }
    }

    #[test]
    fn forge_style_lists_filter_by_game_version() {
        let builds = vec![
            entry("65.1.3", "", "2026-08-27T04:52:23+00:00", false, Some("26.2")),
            entry("65.1.2", "", "2026-08-20T04:52:23+00:00", false, Some("26.2")),
            entry("52.1.16", "", "2026-08-27T04:52:23+00:00", true, Some("1.21.1")),
        ];
        let filtered = filter_loader_versions(&builds, "26.2");
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].version, "65.1.3");
        // A game version nobody built for: empty, and the dialog reports it.
        assert!(filter_loader_versions(&builds, "1.20.1").is_empty());
    }

    #[test]
    fn loader_kind_round_trips_and_maps_to_modrinth_names() {
        for kind in LoaderKind::all() {
            assert_eq!(LoaderKind::from_label(kind.label()), Some(kind));
            if let Some(uid) = kind.uid() {
                assert_eq!(LoaderKind::from_uid(uid), Some(kind));
            }
            assert!(!kind.modrinth_name().is_empty());
        }
        assert!(LoaderKind::Vanilla.uid().is_none());
        assert!(!LoaderKind::Vanilla.loads_mods());
        assert!(LoaderKind::Fabric.loads_mods());
    }

    #[test]
    fn list_url_and_cache_paths_cover_both_layouts() {
        assert_eq!(
            list_url("https://meta.example.invalid/v1/", "net.minecraft"),
            "https://meta.example.invalid/v1/net.minecraft/index.json"
        );
        let paths = cache_paths(Path::new("/tmp/meta"), "net.minecraft");
        assert!(paths[0].ends_with("net.minecraft/index.json"));
        assert!(paths[1].ends_with("net.minecraft.json"));
    }

    #[test]
    fn parse_reads_prism_shape_and_rejects_junk() {
        let body = br#"{"formatVersion":1,"name":"Minecraft","uid":"net.minecraft","versions":[
            {"version":"26.2","type":"release","recommended":true,"releaseTime":"2026-06-16T12:03:33+00:00",
             "requires":[{"uid":"org.lwjgl3","suggests":"3.4.1"}]}]}"#;
        let parsed = parse_version_list(body, Path::new("index.json"), MINECRAFT_UID).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].version, "26.2");
        assert!(parsed[0].recommended);
        assert_eq!(parsed[0].uid, MINECRAFT_UID);
        // Ordinary parse problems stay ordinary: an error string, no panic.
        assert!(parse_version_list(b"{oops", Path::new("i"), "x").is_err());
        assert!(parse_version_list(b"[]", Path::new("i"), "x").is_err());
        assert!(parse_version_list(br#"{"formatVersion":9}"#, Path::new("i"), "x").is_err());
        assert!(parse_version_list(br#"{"formatVersion":1}"#, Path::new("i"), "x").unwrap().is_empty());
    }

    #[test]
    fn read_cached_list_accepts_the_old_flat_layout() {
        let dir = tempfile::tempdir().unwrap();
        let meta = dir.path().join("meta");
        std::fs::create_dir_all(&meta).unwrap();
        std::fs::write(
            meta.join("org.quiltmc.quilt-loader.json"),
            br#"{"formatVersion":1,"versions":[{"version":"0.31.0","type":"release"}]}"#,
        )
        .unwrap();
        let entries = read_cached_list(&meta, "org.quiltmc.quilt-loader").unwrap();
        assert_eq!(entries[0].version, "0.31.0");
        assert!(read_cached_list(&meta, "missing.uid").is_err());
    }

}
