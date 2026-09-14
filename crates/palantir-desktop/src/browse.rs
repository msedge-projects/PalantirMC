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

use std::path::{Path, PathBuf};
use std::time::Duration;

use palantir_net::modrinth::{search_url_with_project_type, version_url, ModrinthProjectVersion};
use serde::Deserialize;
use sha1::Digest;

use crate::catalog::LoaderKind;

/// User agent identifying this launcher, as Modrinth's API guidelines ask.
pub const USER_AGENT: &str = concat!("PalantirMC/", env!("CARGO_PKG_VERSION"));

/// How long a browse request may take.
pub const HTTP_TIMEOUT: Duration = Duration::from_secs(25);

/// How many search hits the UI keeps (the API caps at 100).
pub const SEARCH_LIMIT: usize = 20;

/// Content tabs exposed by Modrinth's public project types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ContentType {
    /// Java/Fabric/Forge/Quilt mods.
    #[default]
    Mods,
    /// Client-side resource packs.
    ResourcePacks,
    /// World/data packs.
    DataPacks,
    /// Shader packs.
    Shaders,
    /// Modrinth modpacks.
    Modpacks,
}

impl ContentType {
    /// Label shown in the Browse tab strip.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Mods => "Mods",
            Self::ResourcePacks => "Resource packs",
            Self::DataPacks => "Data packs",
            Self::Shaders => "Shaders",
            Self::Modpacks => "Modpacks",
        }
    }

    /// Modrinth's `project_type` facet value.
    pub const fn api_value(self) -> &'static str {
        match self {
            Self::Mods => "mod",
            Self::ResourcePacks => "resourcepack",
            Self::DataPacks => "datapack",
            Self::Shaders => "shader",
            Self::Modpacks => "modpack",
        }
    }

    /// All supported tabs in product order.
    pub const fn all() -> [Self; 5] {
        [Self::Mods, Self::ResourcePacks, Self::DataPacks, Self::Shaders, Self::Modpacks]
    }

    /// Target folder inside an instance.
    pub const fn target_folder(self) -> &'static str {
        match self {
            Self::Mods => "mods",
            Self::ResourcePacks => "resourcepacks",
            Self::DataPacks => "datapacks",
            Self::Shaders => "shaderpacks",
            Self::Modpacks => "mods",
        }
    }

    /// Whether the target folder is a loader-specific mod folder.
    pub const fn needs_loader(self) -> bool {
        matches!(self, Self::Mods)
    }
}

/// One search result (a superset of what the UI paints).
#[derive(Debug, Clone, PartialEq, Deserialize)]
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

impl Hit {
    /// Stable identifier for follow-up calls.
    pub fn project_ref(&self) -> &str {
        if self.project_id.is_empty() {
            &self.slug
        } else {
            &self.project_id
        }
    }

    /// `1.2M downloads` / `12,345 downloads`, compact for cards.
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
struct SearchEnvelope {
    #[serde(default)]
    hits: Vec<Hit>,
}

/// Parse a `GET /v2/search` body (pure; used by the tests).
pub fn parse_search(body: &str) -> Result<Vec<Hit>, String> {
    let envelope: SearchEnvelope =
        serde_json::from_str(body).map_err(|error| format!("unexpected search response: {error}"))?;
    Ok(envelope.hits)
}

/// HTTP client used by every browse call.
pub fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| format!("cannot build an HTTP client: {error}"))
}

fn get_text(client: &reqwest::blocking::Client, url: &str) -> Result<String, String> {
    let response = client
        .get(url)
        .header("Accept", "application/json")
        .send()
        .map_err(|error| format!("request to {url} failed: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("{url} returned HTTP {status}"));
    }
    response.text().map_err(|error| format!("reading {url} failed: {error}"))
}

/// Search Modrinth for one of the launcher's content tabs.
pub fn search_typed(
    client: &reqwest::blocking::Client,
    query: &str,
    content_type: ContentType,
) -> Result<Vec<Hit>, String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let body = get_text(
        client,
        &search_url_with_project_type(trimmed, content_type.api_value()),
    )?;
    let mut hits = parse_search(&body)?;
    hits.retain(|hit| hit.project_type.eq_ignore_ascii_case(content_type.api_value()));
    hits.truncate(SEARCH_LIMIT);
    Ok(hits)
}

/// List a project's versions (newest first, as the API returns them).
pub fn project_versions(
    client: &reqwest::blocking::Client,
    project: &str,
) -> Result<Vec<ModrinthProjectVersion>, String> {
    let body = get_text(client, &version_url(project))?;
    serde_json::from_str(&body).map_err(|error| format!("unexpected version list: {error}"))
}

/// Choose the best version for an instance.
///
/// Rules, in order:
/// 1. the version must list `game` in `game_versions` and carry at least one
///    downloadable file;
/// 2. it must declare the target loader (`fabric`, `neoforge`, `forge`,
///    `quilt`); vanilla targets accept any loader;
/// 3. among those, the newest `release` wins, then the newest `beta`, then the
///    newest `alpha` — "newest" being API order, which is publish-date
///    descending. [`ModrinthProjectVersion`] carries no timestamp, so order is
///    the only ordering signal available.
pub fn pick_version<'a>(
    versions: &'a [ModrinthProjectVersion],
    game: &str,
    loader: LoaderKind,
) -> Option<&'a ModrinthProjectVersion> {
    let compatible = |candidate: &&ModrinthProjectVersion| {
        candidate.primary_file().is_some()
            && candidate.game_versions.iter().any(|version| version == game)
            && (loader == LoaderKind::Vanilla
                || candidate.loaders.iter().any(|name| name == loader.modrinth_name()))
    };
    for kind in ["release", "beta", "alpha"] {
        let found = versions
            .iter()
            .filter(compatible)
            .find(|candidate| candidate.version_type == kind);
        if let Some(version) = found {
            return Some(version);
        }
    }
    versions.iter().filter(compatible).next()
}

/// A file that was written into an instance.
#[derive(Debug, Clone, PartialEq)]
pub struct InstalledFile {
    /// File name as written.
    pub filename: String,
    /// Full destination path.
    pub path: PathBuf,
    /// Bytes written.
    pub bytes: usize,
    /// Whether the `sha1` the API published was checked.
    pub verified: bool,
}

/// `sha1` hex digest, lowercase.
pub fn sha1_hex(bytes: &[u8]) -> String {
    let mut hasher = sha1::Sha1::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Make an API-provided file name safe to write on Windows.
pub fn safe_file_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("the project published a file without a name".to_string());
    }
    if trimmed.contains('/') || trimmed.contains('\\') || trimmed.contains("..") {
        return Err(format!("refusing to write suspicious file name '{trimmed}'"));
    }
    let cleaned: String = trimmed
        .chars()
        .map(|c| if matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*' | '\0') { '_' } else { c })
        .collect();
    Ok(cleaned)
}

/// Verify downloaded bytes against a Modrinth `sha1` (and size when known).
pub fn verify_download(bytes: &[u8], expected_sha1: Option<&str>, expected_size: u64) -> Result<bool, String> {
    if expected_size > 0 && bytes.len() as u64 != expected_size {
        return Err(format!("size mismatch: expected {expected_size} bytes, got {}", bytes.len()));
    }
    match expected_sha1.map(str::trim).filter(|digest| !digest.is_empty()) {
        Some(expected) => {
            let actual = sha1_hex(bytes);
            if !actual.eq_ignore_ascii_case(expected) {
                return Err(format!("sha1 mismatch: expected {expected}, got {actual}"));
            }
            Ok(true)
        }
        None => Ok(false),
    }
}

/// Download a version's primary file into the selected content folder.
pub fn install_version(
    client: &reqwest::blocking::Client,
    target_dir: &Path,
    version: &ModrinthProjectVersion,
) -> Result<InstalledFile, String> {
    let file = version
        .primary_file()
        .ok_or_else(|| format!("{} has no downloadable file", version.name))?;
    if file.url.is_empty() {
        return Err(format!("{} publishes no direct download", version.name));
    }
    let filename = safe_file_name(&file.filename)?;
    let response = client
        .get(&file.url)
        .send()
        .map_err(|error| format!("downloading {filename} failed: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("downloading {filename} failed: HTTP {status}"));
    }
    let bytes = response
        .bytes()
        .map_err(|error| format!("reading {filename} failed: {error}"))?
        .to_vec();
    let verified = verify_download(&bytes, file.sha1(), file.size)?;
    palantir_core::util::ensure_dir(target_dir)
        .map_err(|error| format!("creating {} failed: {error}", target_dir.display()))?;
    let path = target_dir.join(&filename);
    palantir_core::util::atomic_write(&path, &bytes)
        .map_err(|error| format!("writing {} failed: {error}", path.display()))?;
    Ok(InstalledFile { filename, path, bytes: bytes.len(), verified })
}

/// One imported modpack.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedPack {
    /// New instance id.
    pub id: String,
    /// Pack format that was detected.
    pub format: &'static str,
}

/// Import a dropped `.mrpack` / CurseForge zip as a new instance.
///
/// Fully offline: the pack's `overrides` land in the instance and the loader is
/// registered; remote files listed in the index are *not* downloaded, and the
/// returned status says so.
pub fn import_pack(
    paths: &palantir_core::paths::PalantirPaths,
    archive: &Path,
) -> Result<ImportedPack, String> {
    let bytes = std::fs::read(archive)
        .map_err(|error| format!("reading '{}' failed: {error}", archive.display()))?;
    let instances_dir = paths.configured_instances_dir();
    let name = archive
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Imported pack".to_string());
    let (format, created) = match palantir_loader::detect_format(&bytes) {
        palantir_loader::PackFormat::MrPack => (
            "Modrinth .mrpack",
            palantir_loader::import_mrpack(&bytes, &instances_dir, &name),
        ),
        palantir_loader::PackFormat::CurseForge => (
            "CurseForge pack",
            palantir_loader::import_curseforge(&bytes, &instances_dir, &name),
        ),
        palantir_loader::PackFormat::Unknown => {
            return Err(format!(
                "'{}' is neither a .mrpack nor a CurseForge pack",
                archive.display()
            ))
        }
    };
    let root = created.map_err(|error| format!("importing the pack failed: {error}"))?;
    let id = root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.clone());
    Ok(ImportedPack { id, format })
}

#[cfg(test)]
mod tests {
    use super::*;
    use palantir_net::modrinth::ModrinthVersionFile;

    fn version(name: &str, kind: &str, games: &[&str], loaders: &[&str], with_file: bool) -> ModrinthProjectVersion {
        ModrinthProjectVersion {
            id: name.to_string(),
            project_id: "P".to_string(),
            name: name.to_string(),
            version_number: name.to_string(),
            version_type: kind.to_string(),
            downloads: 0,
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
        }
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
    fn picking_a_version_respects_game_loader_and_type_order() {
        let versions = vec![
            version("alpha-new", "alpha", &["26.2"], &["fabric"], true),
            version("beta-old", "beta", &["26.2"], &["fabric"], true),
            version("release-old", "release", &["26.2"], &["fabric"], true),
            version("other-game", "release", &["1.21.1"], &["fabric"], true),
            version("forge-only", "release", &["26.2"], &["forge"], true),
            version("no-file", "release", &["26.2"], &["fabric"], false),
        ];
        let picked = pick_version(&versions, "26.2", LoaderKind::Fabric).unwrap();
        assert_eq!(picked.name, "release-old", "releases win over newer betas/alphas");
        assert_eq!(pick_version(&versions, "26.2", LoaderKind::Forge).unwrap().name, "forge-only");
        // Vanilla targets accept any loader.
        assert_eq!(pick_version(&versions, "26.2", LoaderKind::Vanilla).unwrap().name, "release-old");
        // Nothing matches an unsupported loader.
        assert!(pick_version(&versions, "26.2", LoaderKind::Quilt).is_none());
        assert!(pick_version(&versions, "1.0", LoaderKind::Fabric).is_none());
    }

    #[test]
    fn picking_a_version_falls_back_to_newest_when_no_release_exists() {
        let versions = vec![
            version("beta-new", "beta", &["26.2"], &["fabric"], true),
            version("alpha-old", "alpha", &["26.2"], &["fabric"], true),
        ];
        assert_eq!(pick_version(&versions, "26.2", LoaderKind::Fabric).unwrap().name, "beta-new");
        let only_alpha = vec![version("only", "alpha", &["26.2"], &["fabric"], true)];
        assert_eq!(pick_version(&only_alpha, "26.2", LoaderKind::Fabric).unwrap().name, "only");
    }

    #[test]
    fn file_names_are_sanitized_and_paths_refused() {
        assert_eq!(safe_file_name("sodium-fabric-0.6.0+mc1.21.1.jar").unwrap(), "sodium-fabric-0.6.0+mc1.21.1.jar");
        assert_eq!(safe_file_name("we:ird?.jar").unwrap(), "we_ird_.jar");
        assert!(safe_file_name("").is_err());
        assert!(safe_file_name("nested/evil.jar").is_err());
        assert!(safe_file_name("..\\evil.jar").is_err());
    }

    #[test]
    fn download_verification_checks_size_and_sha1() {
        let bytes = b"hello";
        let digest = sha1_hex(bytes);
        assert!(verify_download(bytes, Some(&digest), 5).unwrap());
        assert!(verify_download(bytes, Some(&digest.to_uppercase()), 5).unwrap());
        assert!(verify_download(bytes, None, 5).unwrap() == false);
        let err = verify_download(bytes, Some(&sha1_hex(b"other")), 5).unwrap_err();
        assert!(err.contains("sha1 mismatch"), "error: {err}");
        let err = verify_download(bytes, None, 6).unwrap_err();
        assert!(err.contains("size mismatch"), "error: {err}");
    }

    #[test]
    fn sha1_matches_the_known_digest_of_abc() {
        assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn installing_refuses_versions_without_files_and_leaves_no_trace() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        let mut candidate = version("sodium", "release", &["26.2"], &["fabric"], true);
        candidate.files[0].hashes.insert("sha1".to_string(), sha1_hex(b"jar bytes"));
        candidate.files[0].size = 9;
        let client = client().unwrap();

        // No files at all: refused before any request.
        let empty = ModrinthProjectVersion { files: Vec::new(), ..candidate.clone() };
        assert!(install_version(&client, &mods, &empty).is_err());
        // No direct URL: also refused before any request.
        let no_url = ModrinthProjectVersion {
            files: vec![ModrinthVersionFile { url: String::new(), ..candidate.files[0].clone() }],
            ..candidate.clone()
        };
        let error = install_version(&client, &mods, &no_url).unwrap_err();
        assert!(error.contains("no direct download"), "error: {error}");
        assert!(!mods.join("sodium.jar").exists());
    }

    #[test]
    fn pack_import_rejects_non_pack_files() {
        let dir = tempfile::tempdir().unwrap();
        let paths = palantir_core::paths::PalantirPaths::at(dir.path());
        std::fs::create_dir_all(paths.instances_dir()).unwrap();
        let bogus = dir.path().join("not-a-pack.zip");
        std::fs::write(&bogus, b"definitely not a zip archive").unwrap();
        let error = import_pack(&paths, &bogus).unwrap_err();
        assert!(error.contains("neither"), "error: {error}");
        assert!(import_pack(&paths, &dir.path().join("missing.zip")).is_err());
    }
}
