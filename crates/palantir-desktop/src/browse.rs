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

use palantir_loader::{PackFile, PackPlan};
use palantir_net::download_many_with_progress;
use palantir_net::meta::{BlockingHttpFetcher, Fetcher};
use palantir_net::modrinth::{search_url_with_project_type, version_url, ModrinthProjectVersion};
use serde::Deserialize;
use sha1::Digest;

use crate::catalog::LoaderKind;
use crate::install::{self, Progress};

/// User agent identifying this launcher, as Modrinth's API guidelines ask.
pub const USER_AGENT: &str = concat!("PalantirMC/", env!("CARGO_PKG_VERSION"));

/// How long a browse request may take.
pub const HTTP_TIMEOUT: Duration = Duration::from_secs(25);

/// How many search hits the UI keeps (the API caps at 100).
pub const SEARCH_LIMIT: usize = 20;

/// Content tabs exposed by Modrinth's public project types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
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

impl ContentType {
    /// Label shown in the Browse tab strip.
    ///
    /// "Packs" is capitalised in both entries because that is how the reference
    /// sets them; read off its tab strip, where OCR returned "Data Packs" whole
    /// and merged "ResourcePacks" out of the same line.
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

    /// Folder inside an instance this content goes into.
    ///
    /// `None` for modpacks, and that is the point of the option: a pack is
    /// installed as a *new instance*, not into an existing one. The old
    /// `"mods"` answer for it is what made Browse drop a `.mrpack` into the
    /// selected instance's mod folder, where the game read it as a broken mod
    /// and none of the pack was present.
    pub const fn target_folder(self) -> Option<&'static str> {
        match self {
            Self::Mods => Some("mods"),
            Self::ResourcePacks => Some("resourcepacks"),
            Self::DataPacks => Some("datapacks"),
            Self::Shaders => Some("shaderpacks"),
            Self::Modpacks => None,
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

/// Choose a project's newest downloadable version, whatever it targets.
///
/// Used for modpacks, and only for modpacks: a pack carries its own Minecraft
/// version and loader in its index, so filtering one against the *selected*
/// instance's version is how you end up refusing to install a 1.20.1 pack
/// because 1.21 happens to be selected. Prism's list, in preference order:
/// `release`, then `beta`, then `alpha`, in the API's publish-date order.
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

/// `sha1` of a file on disk, lowercase hex.
///
/// Streamed rather than read whole: a modpack's files include 40 MB mod jars,
/// and the digest is the only thing about them this launcher wants in memory.
/// The 64 KiB window is the same one the downloader writes through.
pub fn sha1_file(path: &Path) -> Result<String, String> {
    use std::io::Read as _;
    let mut file = std::fs::File::open(path)
        .map_err(|error| format!("reading '{}' failed: {error}", path.display()))?;
    let mut hasher = sha1::Sha1::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("reading '{}' failed: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
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

/// How many files one content install may write, dependencies included.
///
/// A bound rather than a policy: Modrinth's graph is user-generated and a cycle
/// between two projects that both require each other is entirely possible. The
/// `seen` set already stops a repeat; this stops a chain long enough to look
/// like a hang. Sixteen covers a mod and its practical dependency set — Fabric
/// API, a library, a config library, an API — several times over.
pub const MAX_CONTENT_FILES: usize = 16;

/// Content installed into an instance, dependencies included.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InstalledContent {
    /// Files written, the requested one first.
    pub files: Vec<InstalledFile>,
    /// Lines worth telling the user: dependencies that were skipped, and why.
    pub notes: Vec<String>,
}

impl InstalledContent {
    /// `3 files (2.1 MiB)` for a status line.
    pub fn summary(&self) -> String {
        let bytes: usize = self.files.iter().map(|file| file.bytes).sum();
        let verified = self.files.iter().filter(|file| file.verified).count();
        let note = if verified == self.files.len() {
            "sha1 verified"
        } else if verified == 0 {
            "size checked"
        } else {
            "sha1 verified where published"
        };
        format!("{} file(s) ({}, {note})", self.files.len(), human_bytes(bytes as u64))
    }
}

/// Install a version and the dependencies it requires.
///
/// Modrinth publishes a version's `dependencies` ([`ModrinthDependency`]) and
/// this is the only thing that reads them. A `required` dependency that names a
/// Modrinth project is resolved with the same rules as the requested file and
/// installed too, and the walk continues into that dependency's own list. What does *not*
/// happen is also deliberate: `optional` and `embedded` dependencies are left
/// alone (installing an optional dependency changes what the user asked for),
/// a dependency hosted off Modrinth is reported rather than guessed at, and a
/// dependency that fails to download does not take the requested file down with
/// it.
///
/// `seen` is keyed by project id, so two mods that both require Fabric API
/// install it once, and a cycle terminates.
pub fn install_with_dependencies(
    client: &reqwest::blocking::Client,
    target_dir: &Path,
    version: &ModrinthProjectVersion,
    game: &str,
    loader: LoaderKind,
    progress: &mut dyn FnMut(Progress),
) -> Result<InstalledContent, String> {
    let mut installed = InstalledContent::default();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut queue: Vec<(String, ModrinthProjectVersion)> =
        vec![(version.project_id.clone(), version.clone())];
    while let Some((project, version)) = queue.pop() {
        let key = if project.is_empty() { version.id.clone() } else { project };
        if !seen.insert(key) {
            continue;
        }
        if installed.files.len() >= MAX_CONTENT_FILES {
            installed.notes.push(format!(
                "stopped after {MAX_CONTENT_FILES} files — install the remaining dependencies by name"
            ));
            break;
        }
        let requested = installed.files.is_empty();
        progress(Progress::starting(format!("installing {}", version.name)));
        match install_version(client, target_dir, &version) {
            Ok(file) => installed.files.push(file),
            // The file the user clicked is the reason for the install: its
            // failure is the install's failure. A dependency's is not.
            Err(error) if requested => return Err(error),
            Err(error) => {
                installed.notes.push(format!("{}: {error}", version.name));
                continue;
            }
        }
        for dependency in version.dependencies.iter().filter(|d| d.is_required()) {
            let Some(project) = dependency.project() else {
                installed.notes.push(format!(
                    "{} also needs {} — it is not on Modrinth, so fetch it from where its author publishes it",
                    version.name,
                    dependency.file_name.as_deref().unwrap_or("a file")
                ));
                continue;
            };
            if seen.contains(project) {
                continue;
            }
            let found = project_versions(client, project).and_then(|versions| {
                pick_version(&versions, game, loader)
                    .cloned()
                    .ok_or_else(|| {
                        format!(
                            "no build of {project} matches {} {game}",
                            loader.label()
                        )
                    })
            });
            match found {
                Ok(found) => queue.push((project.to_string(), found)),
                Err(error) => installed.notes.push(format!("{} needs {project}: {error}", version.name)),
            }
        }
    }
    Ok(installed)
}

/// What fetching a pack's listed files came to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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

impl PackFetch {
    /// `18 files (121.4 MiB), 2 already present` for a status line.
    pub fn summary(&self) -> String {
        let mut out = format!("{} file(s)", self.fetched);
        if self.bytes > 0 {
            out.push_str(&format!(" ({})", human_bytes(self.bytes)));
        }
        if self.present > 0 {
            out.push_str(&format!(", {} already present", self.present));
        }
        out
    }
}

/// Human byte size (binary units), for a status line.
pub fn human_bytes(bytes: u64) -> String {
    let value = bytes as f64;
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GiB", value / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", value / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.0} KiB", value / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

/// Download the files a pack's index lists into the instance it was imported
/// into.
///
/// The pack's own `path` decides where each file lands, under the instance
/// root; [`PackFile::relative_path`] has already refused anything that would
/// escape it, and the refusal was reported at planning time rather than
/// silently rewritten here. Every file is checked against the `sha1` the pack
/// published, and a mismatch is deleted rather than kept: a corrupted mod jar
/// is a crash at startup, while a missing one is a line the user can act on.
///
/// A file the pack lists with several URLs gets its mirrors tried in order when
/// the first one fails — that is what the list is for.
pub fn fetch_pack_files(
    fetcher: &(dyn Fetcher + Sync),
    root: &Path,
    files: &[PackFile],
    threads: usize,
    progress: &mut dyn FnMut(Progress),
) -> PackFetch {
    let mut fetch = PackFetch::default();
    let mut jobs: Vec<(String, PathBuf)> = Vec::new();
    // Parallel to `jobs`: the file each job came from and where it is going, for
    // the digest and the mirrors after the download.
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
        jobs.push((file.downloads[0].clone(), dest.clone()));
        planned.push((file, dest));
    }
    if jobs.is_empty() {
        return fetch;
    }
    let total = jobs.len();
    let results = download_many_with_progress(fetcher, &jobs, threads.max(1), &mut |done, bytes| {
        progress(Progress::new("pack files", done, total, bytes));
    });
    for (index, (_, result)) in results.into_iter().enumerate() {
        let (file, dest) = &planned[index];
        let outcome = match result {
            Ok(bytes) => Ok(bytes),
            // The first URL failed, so the mirrors get their turn before this
            // file is written off. Sequential and rare: a pack lists mirrors
            // for the file the primary host would not serve, not for balance.
            Err(error) => {
                let mut last = error.to_string();
                let mut recovered = None;
                for mirror in file.downloads.iter().skip(1) {
                    match fetch_one(fetcher, mirror, dest) {
                        Ok(bytes) => {
                            recovered = Some(bytes);
                            break;
                        }
                        Err(error) => last = error.to_string(),
                    }
                }
                recovered.ok_or(last)
            }
        };
        let bytes = match outcome {
            Ok(bytes) => bytes,
            Err(reason) => {
                fetch.failed.push(format!("{}: {reason}", file.path));
                continue;
            }
        };
        match verify_pack_file(dest, file) {
            Ok(()) => {
                fetch.fetched += 1;
                fetch.bytes += bytes;
            }
            Err(reason) => {
                // Worse than missing: the next launch would trust it.
                let _ = std::fs::remove_file(dest);
                fetch.failed.push(format!("{}: {reason}", file.path));
            }
        }
    }
    fetch
}

/// Fetch one URL with `fetcher`, writing it into `dest`.
fn fetch_one(fetcher: &(dyn Fetcher + Sync), url: &str, dest: &Path) -> Result<u64, String> {
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            palantir_core::util::ensure_dir(parent)
                .map_err(|error| format!("creating {} failed: {error}", parent.display()))?;
        }
    }
    // `.part` then rename, so an interrupted mirror attempt never leaves a
    // half file where a later check would trust it.
    let part = dest.with_extension("part");
    let mut sink = std::fs::File::create(&part)
        .map_err(|error| format!("writing {} failed: {error}", part.display()))?;
    let written = fetcher
        .fetch_to(url, &mut sink)
        .map_err(|error| error.to_string())?;
    drop(sink);
    std::fs::rename(&part, dest)
        .map_err(|error| format!("finishing {} failed: {error}", dest.display()))?;
    Ok(written)
}

/// Check a fetched pack file against the `sha1` the pack published.
fn verify_pack_file(dest: &Path, file: &PackFile) -> Result<(), String> {
    let Some(expected) = file.sha1.as_deref().filter(|digest| !digest.is_empty()) else {
        return Ok(());
    };
    let actual = sha1_file(dest)?;
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(format!("sha1 mismatch: packed {expected}, downloaded {actual}"))
    }
}

/// A pack installed as a new instance, files and all.
#[derive(Debug, Clone, PartialEq)]
pub struct InstalledPack {
    /// The new instance's id (its folder name).
    pub id: String,
    /// What the files came to.
    pub fetch: PackFetch,
    /// Entries the pack listed that were not installed, with the reason.
    pub skipped: Vec<String>,
}

/// Install a Modrinth modpack as a new instance.
///
/// The order is the only one that works: the `.mrpack` is fetched and cached,
/// its plan is read, the archive is imported (overrides written, loader
/// registered), and only then are the files the index lists downloaded into the
/// new instance. Installing a pack by dropping its archive into `mods/` — which
/// is what this used to do — left the game reading a zip as a broken mod and
/// none of the pack present.
///
/// The archive stays in `<data root>/cache/packs/`: it is the only thing that
/// knows what the pack contains, so a failed fetch can be retried and a repair
/// has something to work from.
pub fn install_pack(
    client: &reqwest::blocking::Client,
    paths: &palantir_core::paths::PalantirPaths,
    version: &ModrinthProjectVersion,
    name: &str,
    progress: &mut dyn FnMut(Progress),
) -> Result<InstalledPack, String> {
    let file = version
        .primary_file()
        .ok_or_else(|| format!("{} has no downloadable file", version.name))?;
    if file.url.is_empty() {
        return Err(format!("{} publishes no direct download", version.name));
    }
    let filename = safe_file_name(&file.filename)?;
    let cache_dir = paths.root.join("cache").join("packs");
    palantir_core::util::ensure_dir(&cache_dir)
        .map_err(|error| format!("creating {} failed: {error}", cache_dir.display()))?;
    let archive = cache_dir.join(&filename);
    progress(Progress::starting(format!("downloading {filename}")));
    let mut response = client
        .get(&file.url)
        .send()
        .map_err(|error| format!("downloading {filename} failed: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("downloading {filename} failed: HTTP {status}"));
    }
    {
        let mut sink = std::fs::File::create(&archive)
            .map_err(|error| format!("writing {} failed: {error}", archive.display()))?;
        std::io::copy(&mut response, &mut sink)
            .map_err(|error| format!("writing {} failed: {error}", archive.display()))?;
    }
    if let Some(expected) = file.sha1().filter(|digest| !digest.is_empty()) {
        let actual = sha1_file(&archive)?;
        if !actual.eq_ignore_ascii_case(expected) {
            let _ = std::fs::remove_file(&archive);
            return Err(format!("sha1 mismatch for {filename}: packed {expected}, downloaded {actual}"));
        }
    }
    // Read once and hand the same bytes to the plan and the import: a pack's
    // overrides make it tens of megabytes, and neither call needs its own copy.
    let bytes = std::fs::read(&archive)
        .map_err(|error| format!("reading '{}' failed: {error}", archive.display()))?;
    import_and_fetch(client, paths, &bytes, name, progress)
}

/// Install a pack that is already on disk (a dropped `.mrpack` or `.zip`).
///
/// The same tail as [`install_pack`] without the download: the instance name is
/// the archive's file stem, which is the only name available and the one the
/// user would have typed.
pub fn install_pack_archive(
    client: &reqwest::blocking::Client,
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
    import_and_fetch(client, paths, &bytes, name.as_str(), progress)
}

/// Import pack bytes and fetch everything the index lists.
fn import_and_fetch(
    client: &reqwest::blocking::Client,
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
    // The Browse client is reused for the files so Modrinth gets the same
    // `User-Agent` it got for the search that led here.
    let fetcher = BlockingHttpFetcher::with_client(client.clone(), HTTP_TIMEOUT);
    let fetch = fetch_pack_files(&fetcher, &root, &plan.files, install::DEFAULT_THREADS, progress);
    Ok(InstalledPack { id, fetch, skipped: plan.skipped })
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
            // No dependencies unless a test asks for them: the field exists so
            // an installer can follow Modrinth's graph, and an empty list is
            // what the overwhelming majority of versions publish.
            dependencies: Vec::new(),
        }
    }

    #[test]
    fn a_modpack_targets_an_instance_of_its_own() {
        assert_eq!(ContentType::Mods.target_folder(), Some("mods"));
        assert_eq!(ContentType::Shaders.target_folder(), Some("shaderpacks"));
        assert_eq!(
            ContentType::Modpacks.target_folder(),
            None,
            "a pack is an instance, not a folder inside one"
        );
        assert!(!ContentType::Modpacks.needs_loader());
        assert_eq!(ContentType::Modpacks.api_value(), "modpack");
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
    fn pack_install_rejects_non_pack_files() {
        let dir = tempfile::tempdir().unwrap();
        let paths = palantir_core::paths::PalantirPaths::at(dir.path());
        std::fs::create_dir_all(paths.instances_dir()).unwrap();
        let client = client().unwrap();
        let bogus = dir.path().join("not-a-pack.zip");
        std::fs::write(&bogus, b"definitely not a zip archive").unwrap();
        let error = install_pack_archive(&client, &paths, &bogus, &mut |_| {}).unwrap_err();
        assert!(error.contains("readable pack"), "error: {error}");
        assert!(install_pack_archive(&client, &paths, &dir.path().join("missing.zip"), &mut |_| {})
            .is_err());
    }

    #[test]
    fn a_pack_file_is_fetched_verified_and_dropped_when_it_does_not_match() {
        // The fetch is driven through the same `Fetcher` the install phases use,
        // so the whole path is exercised without a network: one good file, one
        // whose bytes are not what the pack's `sha1` says, and one already
        // present.
        let dir = tempfile::tempdir().unwrap();
        let good = b"good mod bytes";
        let files = vec![
            PackFile {
                path: "mods/good.jar".to_string(),
                downloads: vec!["https://cdn.example.invalid/good.jar".to_string()],
                sha1: Some(sha1_hex(good)),
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
        let mut fetcher = palantir_net::MapFetcher::new();
        fetcher.insert("https://cdn.example.invalid/good.jar", good.to_vec());
        fetcher.insert("https://cdn.example.invalid/bad.jar", b"tampered".to_vec());
        // Spelled the way `MapFetcher` stores them, so the assertion below is
        // about the pack's files rather than about the fixture.
        assert!(!fetcher.is_empty());
        let mut reports = 0usize;
        let fetch = fetch_pack_files(&fetcher, dir.path(), &files, 2, &mut |_| reports += 1);
        assert_eq!(fetch.fetched, 1);
        assert_eq!(fetch.present, 1, "a file that is already there is left alone");
        assert_eq!(fetch.bytes, good.len() as u64);
        assert_eq!(fetch.failed.len(), 1);
        assert!(fetch.failed[0].contains("mods/bad.jar"), "{:?}", fetch.failed);
        assert!(fetch.failed[0].contains("sha1 mismatch"));
        assert!(dir.path().join("mods").join("good.jar").is_file());
        assert!(
            !dir.path().join("mods").join("bad.jar").exists(),
            "a file that fails its digest is removed, not kept for the next launch to trust"
        );
        assert!(reports > 0, "the bar hears about the phase");
    }
}
