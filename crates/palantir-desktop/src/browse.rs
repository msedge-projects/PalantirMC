//! Modrinth browsing: search projects and install the right file into an
//! instance.
//!
//! This is the "Browse" page's engine. It talks to the public Modrinth API
//! (`api.modrinth.com/v2`) with a proper `User-Agent`, picks the newest file
//! that actually matches the target instance's game version and loader, then
//! downloads it straight into `<instance>/mods/` after verifying the file's
//! `sha1` (or, when the API omits it, its byte size).
//!
//! URL building is shared with `palantir-net::modrinth`; parsing and selection
//! are pure so the matching rules are unit-tested without a network.

// What the binary no longer needs is marked rather than deleted: the module's
// request building and pack reading are what its tests cover, and the shell's
// own Discover page asks `palantir-net` directly since the old shell went.
//
// The pack installer's transfers joined them with the launch path's (G93): the
// files a pack lists go over the launcher's one wire, so nothing here builds a
// client, a fetcher or a thread count of its own, and the `User-Agent`
// Modrinth's guidelines ask for is the one `engine::http` sets on the client
// every request already goes through.
#[cfg(test)]
use std::path::{Path, PathBuf};
#[cfg(test)]
use serde::Deserialize;
#[cfg(test)]
use palantir_loader::{PackFile, PackPlan};
#[cfg(test)]
use palantir_net::modrinth::ModrinthProjectVersion;
#[cfg(test)]
#[cfg(test)]
use crate::install::{self, Progress};
#[cfg(test)]
use crate::wire::{FileJob, Wire};

/// Content tabs exposed by Modrinth's public project types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg(test)]
pub enum ContentType {
    /// Modrinth modpacks.
    ///
    /// First, and the default, because that is where the reference opens its
    /// Discover page: its tab strip was measured as Modpacks, Mods, Resource
    /// Packs, Data Packs, Shaders, with the Modpacks pill filled. It also has a
    /// sixth tab, Servers, which this shell does not carry -- `project_type:server`
    /// answers 0 hits through the public search API, so a tab for it could only
    /// ever be empty, and a dead tab is worse than an absent one. `REFERENCE.md`
    /// records the difference.
    #[default]
    Modpacks,
    /// Java/Fabric/Forge/Quilt mods.
    Mods,
    /// Client-side resource packs.
    ResourcePacks,
    /// World/data packs.
    DataPacks,
    /// Shader packs.
    Shaders,
}

/// The label is what `pick_list` draws in the closed control and beside each
/// entry, so the two can never disagree about what an order is called.
#[cfg(test)]
impl ContentType {
    /// Label shown in the Browse tab strip.
    ///
    /// "Packs" is capitalised in both entries because that is how the reference
    /// sets them; read off its tab strip, where OCR returned "Data Packs" whole
    /// and merged "ResourcePacks" out of the same line.
    #[cfg(test)]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Modpacks => "Modpacks",
            Self::Mods => "Mods",
            Self::ResourcePacks => "Resource Packs",
            Self::DataPacks => "Data Packs",
            Self::Shaders => "Shaders",
        }
    }

    /// Modrinth's `project_type` facet value.
    #[cfg(test)]
    pub const fn api_value(self) -> &'static str {
        match self {
            Self::Mods => "mod",
            Self::ResourcePacks => "resourcepack",
            Self::DataPacks => "datapack",
            Self::Shaders => "shader",
            Self::Modpacks => "modpack",
        }
    }

    /// All supported tabs, in the order the reference draws them.
    pub const fn all() -> [Self; 5] {
        [Self::Modpacks, Self::Mods, Self::ResourcePacks, Self::DataPacks, Self::Shaders]
    }

    /// Whether the target folder is a loader-specific mod folder.
    pub const fn needs_loader(self) -> bool {
        matches!(self, Self::Mods)
    }
}

/// One search result (a superset of what the UI paints).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[cfg(test)]
pub struct Hit {
    /// Project id (stable).
    #[serde(default)]
    pub project_id: String,
    /// URL slug.
    #[serde(default)]
    pub slug: String,
    /// Display title.
    #[serde(default)]
    pub title: String,
    /// Short description.
    #[serde(default)]
    pub description: String,
    /// Author username.
    #[serde(default)]
    pub author: String,
    /// Total downloads.
    #[serde(default)]
    pub downloads: u64,
    /// Icon URL (may be empty).
    #[serde(default)]
    pub icon_url: String,
    /// `mod`, `modpack`, `resourcepack`, ...
    #[serde(default)]
    pub project_type: String,
}

#[cfg(test)]
impl Hit {
    /// Stable identifier for follow-up calls.
    #[cfg(test)]
    pub fn project_ref(&self) -> &str {
        if self.project_id.is_empty() {
            &self.slug
        } else {
            &self.project_id
        }
    }

    /// `1.2M downloads` / `12,345 downloads`, compact for cards.
    #[cfg(test)]
    pub fn downloads_label(&self) -> String {
        let n = self.downloads;
        if n >= 1_000_000_000 {
            format!("{:.1}B downloads", n as f64 / 1_000_000_000.0)
        } else if n >= 1_000_000 {
            format!("{:.1}M downloads", n as f64 / 1_000_000.0)
        } else if n >= 1_000 {
            format!("{:.1}K downloads", n as f64 / 1_000.0)
        } else {
            format!("{n} downloads")
        }
    }

    /// `by author · 1.2M downloads`.
    pub fn byline(&self) -> String {
        if self.author.is_empty() {
            self.downloads_label()
        } else {
            format!("by {} · {}", self.author, self.downloads_label())
        }
    }
}

/// Search response envelope.
#[derive(Debug, Clone, Default, Deserialize)]
#[cfg(test)]
struct SearchEnvelope {
    #[serde(default)]
    hits: Vec<Hit>,
}

/// Parse a `GET /v2/search` body (pure; used by the tests).
#[cfg(test)]
pub fn parse_search(body: &str) -> Result<Vec<Hit>, String> {
    let envelope: SearchEnvelope =
        serde_json::from_str(body).map_err(|error| format!("unexpected search response: {error}"))?;
    Ok(envelope.hits)
}

/// Choose a project's newest downloadable version, whatever it targets.
///
/// Used for modpacks, and only for modpacks: a pack carries its own Minecraft
/// version and loader in its index, so filtering one against the *selected*
/// instance's version is how you end up refusing to install a 1.20.1 pack
/// because 1.21 happens to be selected. Prism's list, in preference order:
/// `release`, then `beta`, then `alpha`, in the API's publish-date order.
#[cfg(test)]
pub fn newest_version(versions: &[ModrinthProjectVersion]) -> Option<&ModrinthProjectVersion> {
    for kind in ["release", "beta", "alpha"] {
        if let Some(found) = versions
            .iter()
            .find(|candidate| candidate.primary_file().is_some() && candidate.version_type == kind)
        {
            return Some(found);
        }
    }
    versions.iter().find(|candidate| candidate.primary_file().is_some())
}

/// What fetching a pack's listed files came to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg(test)]
pub struct PackFetch {
    /// Files downloaded this time.
    pub fetched: usize,
    /// Files that were already on disk and correct enough to leave alone.
    pub present: usize,
    /// Bytes written.
    pub bytes: u64,
    /// One line per file that could not be installed.
    pub failed: Vec<String>,
}

/// Download the files a pack's index lists into the instance it was imported
/// into.
///
/// The pack's own `path` decides where each file lands, under the instance
/// root; [`PackFile::relative_path`] has already refused anything that would
/// escape it, and the refusal was reported at planning time rather than
/// silently rewritten here. Every file that states a `sha1` is checked against
/// it by the transfer itself, which never renames a part file that fails the
/// check: a corrupted mod jar in `mods/` is a crash at startup, while a missing
/// one is a line the user can act on.
///
/// A file the pack lists with several URLs gets its mirrors tried in order when
/// the first one fails — that is what the list is for.
#[cfg(test)]
pub fn fetch_pack_files(
    wire: &Wire,
    root: &Path,
    files: &[PackFile],
    workers: usize,
    progress: &mut dyn FnMut(Progress),
) -> PackFetch {
    let mut fetch = PackFetch::default();
    // The file each job came from and where it is going, for the digest, the
    // mirrors and the report after the download.
    let mut planned: Vec<(&PackFile, PathBuf)> = Vec::new();
    for file in files {
        let Some(relative) = file.relative_path() else {
            fetch
                .failed
                .push(format!("{}: refusing to write that path", file.path));
            continue;
        };
        if file.downloads.is_empty() {
            // `plan_pack` never produces this, but this is a public entry point
            // and indexing `downloads[0]` is not how a missing URL should be
            // reported.
            fetch
                .failed
                .push(format!("{}: no download URL", file.path));
            continue;
        }
        let dest = root.join(relative);
        if file.satisfied_at(&dest) {
            fetch.present += 1;
            continue;
        }
        planned.push((file, dest));
    }
    if planned.is_empty() {
        return fetch;
    }
    let total = planned.len();
    let jobs: Vec<FileJob> = planned
        .iter()
        .map(|(file, dest)| {
            FileJob::new(&file.downloads[0], dest, file.sha1.clone().unwrap_or_default())
        })
        .collect();
    let results = wire.files(&jobs, workers.max(1), &mut |done, bytes| {
        progress(Progress::new("pack files", done, total, bytes));
    });
    for ((file, dest), result) in planned.iter().zip(results) {
        let bytes = match result {
            Ok(bytes) => bytes,
            // The first URL failed, so the mirrors get their turn before this
            // file is written off. Sequential and rare: a pack lists mirrors
            // for the file the primary host would not serve, not for balance,
            // and a mirror is one more job on the same queue.
            Err(mut last) => {
                let mut recovered = None;
                for mirror in file.downloads.iter().skip(1) {
                    let jobs =
                        [FileJob::new(mirror, dest, file.sha1.clone().unwrap_or_default())];
                    let mut one = wire.files(&jobs, 1, &mut |_, _| {});
                    match one.remove(0) {
                        Ok(bytes) => {
                            recovered = Some(bytes);
                            break;
                        }
                        Err(error) => last = error,
                    }
                }
                match recovered {
                    Some(bytes) => bytes,
                    None => {
                        fetch.failed.push(format!("{}: {last}", file.path));
                        continue;
                    }
                }
            }
        };
        fetch.fetched += 1;
        fetch.bytes += bytes;
    }
    fetch
}

/// A pack installed as a new instance, files and all.
#[derive(Debug, Clone, PartialEq)]
#[cfg(test)]
pub struct InstalledPack {
    /// The new instance's id (its folder name).
    pub id: String,
    /// What the files came to.
    pub fetch: PackFetch,
    /// Entries the pack listed that were not installed, with the reason.
    pub skipped: Vec<String>,
}

/// Install a pack that is already on disk (a dropped `.mrpack` or `.zip`).
///
/// The same tail as [`install_pack`] without the download: the instance name is
/// the archive's file stem, which is the only name available and the one the
/// user would have typed.
#[cfg(test)]
pub fn install_pack_archive(
    wire: &Wire,
    paths: &palantir_core::paths::PalantirPaths,
    archive: &Path,
    progress: &mut dyn FnMut(Progress),
) -> Result<InstalledPack, String> {
    let bytes = std::fs::read(archive)
        .map_err(|error| format!("reading '{}' failed: {error}", archive.display()))?;
    let name = archive
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Imported pack".to_string());
    import_and_fetch(wire, paths, &bytes, name.as_str(), progress)
}

/// Import pack bytes and fetch everything the index lists.
#[cfg(test)]
fn import_and_fetch(
    wire: &Wire,
    paths: &palantir_core::paths::PalantirPaths,
    bytes: &[u8],
    name: &str,
    progress: &mut dyn FnMut(Progress),
) -> Result<InstalledPack, String> {
    let plan: PackPlan = palantir_loader::plan_pack(bytes)
        .map_err(|error| format!("that archive is not a readable pack: {error}"))?;
    let instances_dir = paths.configured_instances_dir();
    let root = match palantir_loader::detect_format(bytes) {
        palantir_loader::PackFormat::MrPack => {
            palantir_loader::import_mrpack(bytes, &instances_dir, name)
        }
        palantir_loader::PackFormat::CurseForge => {
            palantir_loader::import_curseforge(bytes, &instances_dir, name)
        }
        palantir_loader::PackFormat::Unknown => {
            return Err("that archive is neither a .mrpack nor a CurseForge pack".to_string())
        }
    }
    .map_err(|error| format!("importing the pack failed: {error}"))?;
    let id = root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.to_string());
    // The launcher's one wire, so a pack's files draw the same ceiling as
    // everything else it is fetching at that moment.
    let fetch = fetch_pack_files(wire, &root, &plan.files, install::DEFAULT_THREADS, progress);
    Ok(InstalledPack { id, fetch, skipped: plan.skipped })
}

#[cfg(test)]
mod tests {
    use super::*;
    use palantir_net::modrinth::ModrinthVersionFile;

    use crate::wire::Script;

    fn version(name: &str, kind: &str, games: &[&str], loaders: &[&str], with_file: bool) -> ModrinthProjectVersion {
        ModrinthProjectVersion {
            id: name.to_string(),
            project_id: "P".to_string(),
            name: name.to_string(),
            version_number: name.to_string(),
            version_type: kind.to_string(),
            downloads: 0,
            changelog: String::new(),
            game_versions: games.iter().map(|g| g.to_string()).collect(),
            loaders: loaders.iter().map(|l| l.to_string()).collect(),
            files: if with_file {
                vec![ModrinthVersionFile {
                    url: format!("https://cdn.example.invalid/{name}.jar"),
                    filename: format!("{name}.jar"),
                    primary: true,
                    size: 4,
                    hashes: std::collections::HashMap::new(),
                }]
            } else {
                Vec::new()
            },
            // No dependencies unless a test asks for them: the field exists so
            // an installer can follow Modrinth's graph, and an empty list is
            // what the overwhelming majority of versions publish.
            dependencies: Vec::new(),
        }
    }

    /// The tab strip's own rule, and the one assertion that outlived it: a pack
    /// is installed as an instance of its own rather than into a folder, which
    /// is why it is the one tab whose content has no `mods` to go in. The folder
    /// table itself is `route::ProjectType::target_folder` now -- the install
    /// path is live and this module is not.
    #[test]
    fn a_modpack_targets_an_instance_of_its_own() {
        assert!(!ContentType::Modpacks.needs_loader());
        assert_eq!(ContentType::Modpacks.api_value(), "modpack");
        assert!(ContentType::Mods.needs_loader());
    }

    /// The strip's order and its opening tab are the reference's, measured off
    /// its own Discover page (`REFERENCE.md`): Modpacks first and selected.
    #[test]
    fn tabs_lead_with_modpacks() {
        assert_eq!(ContentType::default(), ContentType::Modpacks);
        let labels: Vec<&str> = ContentType::all().iter().map(|k| k.label()).collect();
        assert_eq!(
            labels,
            ["Modpacks", "Mods", "Resource Packs", "Data Packs", "Shaders"]
        );
        // Every tab still maps to a project type the search API answers for;
        // the reference's sixth tab (Servers) is deliberately absent, because
        // `project_type:server` returns nothing.
        for kind in ContentType::all() {
            assert!(!kind.api_value().is_empty());
        }
    }

    #[test]
    fn the_newest_pack_is_chosen_without_regard_to_the_selection() {
        let versions = vec![
            version("beta-new", "beta", &["1.21.4"], &["fabric"], true),
            version("release-old", "release", &["1.20.1"], &["forge"], true),
            version("no-file", "release", &["1.20.1"], &["forge"], false),
        ];
        // Releases first, whatever game version or loader they carry: the pack
        // brings its own, and there is no selected instance to match.
        assert_eq!(newest_version(&versions).unwrap().name, "release-old");
        assert!(newest_version(&[]).is_none());
        let only_beta = vec![version("b", "beta", &["1.21"], &[], true)];
        assert_eq!(newest_version(&only_beta).unwrap().name, "b");
    }

    #[test]
    fn search_parsing_reads_the_api_shape() {
        let body = r#"{"hits":[{
            "project_id":"AANobbMI","slug":"sodium","title":"Sodium",
            "description":"Fast rendering","author":"jellysquid3","downloads":12345678,
            "icon_url":"https://x/icon.png","project_type":"mod","unexpected":true
        }],"offset":0,"limit":20,"total_hits":1}"#;
        let hits = parse_search(body).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].project_ref(), "AANobbMI");
        assert_eq!(hits[0].byline(), "by jellysquid3 · 12.3M downloads");
        assert_eq!(hits[0].downloads_label(), "12.3M downloads");
        assert!(parse_search("not json").is_err());
        assert!(parse_search("{}").unwrap().is_empty());
    }

    #[test]
    fn download_labels_scale() {
        let mut hit = Hit {
            project_id: "p".into(),
            slug: "s".into(),
            title: "t".into(),
            description: String::new(),
            author: String::new(),
            downloads: 12,
            icon_url: String::new(),
            project_type: "mod".into(),
        };
        assert_eq!(hit.downloads_label(), "12 downloads");
        assert_eq!(hit.byline(), "12 downloads");
        hit.downloads = 1_234;
        assert_eq!(hit.downloads_label(), "1.2K downloads");
        hit.downloads = 2_500_000_000;
        assert_eq!(hit.downloads_label(), "2.5B downloads");
        // A hit with no project id falls back to its slug.
        hit.project_id.clear();
        assert_eq!(hit.project_ref(), "s");
    }

    #[test]
    fn pack_install_rejects_non_pack_files() {
        let dir = tempfile::tempdir().unwrap();
        let paths = palantir_core::paths::PalantirPaths::at(dir.path());
        std::fs::create_dir_all(paths.instances_dir()).unwrap();
        // An archive that is not a pack is refused before the wire is touched,
        // which is what makes an empty wire the right double for it.
        let wire = Script::new().wire();
        let bogus = dir.path().join("not-a-pack.zip");
        std::fs::write(&bogus, b"definitely not a zip archive").unwrap();
        let error = install_pack_archive(&wire, &paths, &bogus, &mut |_| {}).unwrap_err();
        assert!(error.contains("readable pack"), "error: {error}");
        assert!(install_pack_archive(&wire, &paths, &dir.path().join("missing.zip"), &mut |_| {})
            .is_err());
    }

    #[test]
    fn a_pack_file_is_fetched_verified_and_dropped_when_it_does_not_match() {
        // The fetch is driven through the launcher's own wire, so the whole
        // path is exercised without a network: one good file, one whose bytes
        // are not what the pack's `sha1` says, and one already present.
        let dir = tempfile::tempdir().unwrap();
        let good = b"good mod bytes";
        let files = vec![
            PackFile {
                path: "mods/good.jar".to_string(),
                downloads: vec!["https://cdn.example.invalid/good.jar".to_string()],
                sha1: Some(install::sha1_hex(good)),
                size: good.len() as u64,
            },
            PackFile {
                path: "mods/bad.jar".to_string(),
                downloads: vec!["https://cdn.example.invalid/bad.jar".to_string()],
                sha1: Some("0000000000000000000000000000000000000000".to_string()),
                size: 4,
            },
            PackFile {
                path: "shaderpacks/here.zip".to_string(),
                downloads: vec!["https://cdn.example.invalid/here.zip".to_string()],
                sha1: None,
                size: 0,
            },
        ];
        std::fs::create_dir_all(dir.path().join("shaderpacks")).unwrap();
        std::fs::write(dir.path().join("shaderpacks").join("here.zip"), b"x").unwrap();
        let mut script = Script::new();
        script.insert("https://cdn.example.invalid/good.jar", good.to_vec());
        script.insert("https://cdn.example.invalid/bad.jar", b"tampered".to_vec());
        let wire = script.wire();
        let mut reports = 0usize;
        let fetch = fetch_pack_files(&wire, dir.path(), &files, 2, &mut |_| reports += 1);
        assert_eq!(fetch.fetched, 1);
        assert_eq!(fetch.present, 1, "a file that is already there is left alone");
        assert_eq!(fetch.bytes, good.len() as u64);
        assert_eq!(fetch.failed.len(), 1);
        assert!(fetch.failed[0].contains("mods/bad.jar"), "{:?}", fetch.failed);
        assert!(fetch.failed[0].contains("hash mismatch"), "{:?}", fetch.failed);
        assert!(dir.path().join("mods").join("good.jar").is_file());
        assert!(
            !dir.path().join("mods").join("bad.jar").exists(),
            "a file that fails its digest is removed, not kept for the next launch to trust"
        );
        assert!(reports > 0, "the bar hears about the phase");
    }
}
