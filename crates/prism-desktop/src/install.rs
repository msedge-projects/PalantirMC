//! Installing what a launch needs: libraries, the client jar, assets and
//! natives.
//!
//! Before this module existed, `launch.rs` could only *report* what was
//! missing: it resolved a pack against a metadata cache that had to already be
//! populated and refused to start when a single classpath jar or the main jar
//! was absent, which meant a freshly created instance could never launch. The
//! missing half is here, and it is deliberately plain filesystem work over
//! `prism-net`'s downloader:
//!
//! * **Metadata first.** [`prism_net::OnlineMetaStore`] fetches the version
//!   files into `cache/meta/` in exactly the layout the offline store reads, so
//!   the second launch — and Prism itself — reads what this one wrote.
//! * **Libraries and the client jar** come from the merged launch profile. Each
//!   file's URL and digest are taken from the library's own `downloads` block
//!   when it has one (that is every Mojang library and the client jar) and from
//!   its Maven `url` otherwise (Fabric, Quilt, Forge and NeoForge all publish
//!   that way). Sizes and SHA-1s are only ever *checked when known*: a Maven
//!   download publishes no digest, and inventing one by trusting the file that
//!   was just written would be theatre.
//! * **Assets** are the index plus its objects. The index is one file; the
//!   objects are addressed by hash under `assets/objects/<xx>/<hash>`, which is
//!   also how Prism stores them, so both launchers share one copy.
//! * **Natives** are extracted flat into `<instance>/natives/` after the jars
//!   land, because that is the directory `-Djava.library.path` points at.
//!
//! Everything here is testable without a network: [`plan`] and [`run`] take a
//! [`Fetcher`], so tests hand them a map of canned bodies and assert on what
//! ended up on disk.

use std::path::{Path, PathBuf};

use prism_core::assets::{object_relative_path, AssetIndex};
use prism_core::paths::PrismPaths;
use prism_core::version::{LaunchProfile, Library, RuntimeContext};
use prism_net::meta::Fetcher;
use prism_net::download_many;

/// Where Mojang serves asset objects from (hash-addressed, first two hex
/// characters as the directory).
pub const ASSET_OBJECT_BASE_URL: &str = "https://resources.download.minecraft.net";

/// Parallel downloads used for the bulk phases.
///
/// Eight is what the Modrinth and Prism clients both settle on for many small
/// files: enough to hide latency without turning a cold install into a
/// denial-of-service against Mojang's CDN.
pub const DEFAULT_THREADS: usize = 8;

/// One file that has to exist on disk before the game can start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadJob {
    /// Absolute or scheme-qualified source URL.
    pub url: String,
    /// Where it belongs.
    pub dest: PathBuf,
    /// Expected SHA-1 (lowercase hex); empty when the source publishes none.
    pub sha1: String,
    /// Expected size in bytes; `0` when the source publishes none.
    pub size: i64,
    /// Short description for the log ("net.fabricmc:fabric-loader:0.15.0").
    pub label: String,
}

impl DownloadJob {
    /// Whether the destination is already there and plausibly complete.
    ///
    /// Existence is the primary test (an interrupted download leaves no file at
    /// all: `prism-net` writes `.part` and renames). A published size is only
    /// used to reject a file that cannot be right; a *smaller* cached file is
    /// re-downloaded, a larger one is left alone because the size in the
    /// metadata is a hint rather than a contract.
    pub fn satisfied(&self) -> bool {
        match std::fs::metadata(&self.dest) {
            Ok(meta) if meta.is_file() => {
                if self.size > 0 && meta.len() < self.size as u64 {
                    return false;
                }
                true
            }
            _ => false,
        }
    }
}

/// The asset half of an install: one index and the objects it lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetPlan {
    /// Asset index id (`"17"`, `"legacy"`, ...).
    pub id: String,
    /// `assets/indexes/<id>.json`.
    pub index_path: PathBuf,
    /// `assets/` — objects live under `objects/<xx>/<hash>`.
    pub assets_dir: PathBuf,
    /// Legacy indexes are also laid out by logical name: in the instance's
    /// `resources/` folder for `legacy`, under `assets/virtual/<id>/` for
    /// `pre-1.6`. Both mirror Prism's `ReconstructAssets` step, and both are
    /// absent for a modern index.
    pub reconstruct: Vec<PathBuf>,
}

/// Everything one install has to do.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InstallPlan {
    /// Files that are missing (or too small to trust) and must be fetched.
    pub jobs: Vec<DownloadJob>,
    /// Native jars to extract once they exist.
    pub natives: Vec<PathBuf>,
    /// Directory the natives are extracted into.
    pub natives_dir: PathBuf,
    /// The asset index, when the profile names one.
    pub assets: Option<AssetPlan>,
    /// Files that were already on disk.
    pub present: usize,
    /// Bytes the `jobs` add up to, where sizes are known.
    pub total_bytes: i64,
    /// Things that could not be planned at all, for the log.
    pub problems: Vec<String>,
}

impl InstallPlan {
    /// Whether there is nothing left to fetch.
    pub fn is_noop(&self) -> bool {
        self.jobs.is_empty()
    }

    /// A one-line summary for the status strip.
    pub fn summary(&self) -> String {
        if self.is_noop() {
            return format!("{} file(s) already installed", self.present);
        }
        let megabytes = self.total_bytes as f64 / (1024.0 * 1024.0);
        if self.total_bytes > 0 {
            format!(
                "{} file(s) to download ({megabytes:.1} MB), {} already present",
                self.jobs.len(),
                self.present
            )
        } else {
            format!(
                "{} file(s) to download, {} already present",
                self.jobs.len(),
                self.present
            )
        }
    }
}

/// What [`run`] did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InstallReport {
    /// Files fetched.
    pub downloaded: usize,
    /// Bytes fetched.
    pub bytes: u64,
    /// Files already present and left alone.
    pub present: usize,
    /// Native library files extracted.
    pub natives_extracted: usize,
    /// Asset objects fetched.
    pub objects_downloaded: usize,
    /// Failures, as `"<label>: <reason>"`.
    pub failed: Vec<String>,
    /// Problems discovered while planning (missing URLs, unusable metadata).
    pub problems: Vec<String>,
}

impl InstallReport {
    /// Whether every file the plan asked for is on disk.
    pub fn is_complete(&self) -> bool {
        self.failed.is_empty()
    }

    /// A one-line outcome for the console.
    pub fn summary(&self) -> String {
        let megabytes = self.bytes as f64 / (1024.0 * 1024.0);
        let mut out = format!(
            "installed {} file(s) ({megabytes:.1} MB), {} already present",
            self.downloaded, self.present
        );
        if self.objects_downloaded > 0 {
            out.push_str(&format!(", {} asset object(s)", self.objects_downloaded));
        }
        if self.natives_extracted > 0 {
            out.push_str(&format!(", {} native file(s) extracted", self.natives_extracted));
        }
        if !self.failed.is_empty() {
            out.push_str(&format!(", {} failure(s)", self.failed.len()));
        }
        out
    }
}

// ---- planning -------------------------------------------------------------

/// Build the list of files `profile` needs on disk, skipping what is already
/// there.
///
/// `instance_root` is the instance folder (its `natives/` receives the native
/// jars); `local_libraries` is the instance's own `libraries/` folder, which
/// overrides the shared cache for `MMC-hint: local` entries.
pub fn plan(
    paths: &PrismPaths,
    instance_root: &Path,
    profile: &LaunchProfile,
    ctx: &RuntimeContext,
) -> InstallPlan {
    let mut plan = InstallPlan {
        natives_dir: instance_root.join("natives"),
        ..InstallPlan::default()
    };
    let local_libraries = instance_root.join("libraries");
    let local_libraries = if local_libraries.is_dir() { Some(local_libraries) } else { None };

    for library in profile
        .libraries
        .iter()
        .chain(profile.native_libraries.iter())
        .chain(profile.main_jar.iter())
    {
        if !library.is_active(ctx) {
            continue;
        }
        // A `local` library lives inside the instance and is never downloaded.
        if library.is_local() {
            continue;
        }
        let files = library.applicable_files(ctx, local_libraries.as_deref());
        // Jars first, then the three native buckets. `arch` follows the bucket,
        // because the metadata writes `${arch}` into both the path and the URL
        // of the 32/64-bit variants.
        let mut queued: Vec<(String, &str)> = Vec::new();
        for rel in &files.jar {
            queued.push((rel.clone(), ""));
        }
        for rel in &files.native {
            queued.push((rel.clone(), ""));
        }
        for rel in &files.native32 {
            queued.push((rel.clone(), "32"));
        }
        for rel in &files.native64 {
            queued.push((rel.clone(), "64"));
        }
        for (rel, arch) in queued {
            // Instance-local overrides resolve outside the shared cache; they
            // are the instance's own files and are never fetched.
            if !rel.starts_with("libraries/") {
                continue;
            }
            let dest = paths.root.join(&rel);
            let is_jar = files.jar.iter().any(|jar| jar == &rel);
            if !is_jar {
                // Every native bucket is a candidate for extraction; only the
                // ones this JVM actually loads get extracted, which the
                // 32/64-bit selection has already decided.
                plan.natives.push(dest.clone());
            }
            match file_facts(library, &rel, arch) {
                Some((url, sha1, size)) => {
                    let job = DownloadJob {
                        url,
                        dest,
                        sha1,
                        size,
                        label: library_label(library),
                    };
                    if job.satisfied() {
                        plan.present += 1;
                    } else {
                        plan.total_bytes += job.size.max(0);
                        plan.jobs.push(job);
                    }
                }
                None => {
                    plan.problems.push(format!(
                        "no download source for {} ({rel})",
                        library_label(library)
                    ));
                }
            }
        }
    }

    let (assets, index_jobs, index_present) = plan_assets(paths, instance_root, profile);
    plan.assets = assets;
    for job in index_jobs {
        if job.satisfied() {
            plan.present += 1;
        } else {
            plan.total_bytes += job.size.max(0);
            plan.jobs.push(job);
        }
    }
    plan.present += index_present;
    plan
}

/// The asset index file (if the profile names one that is downloadable).
fn plan_assets(
    paths: &PrismPaths,
    instance_root: &Path,
    profile: &LaunchProfile,
) -> (Option<AssetPlan>, Vec<DownloadJob>, usize) {
    let Some(index) = profile.minecraft_assets.clone() else {
        return (None, Vec::new(), 0);
    };
    if index.id.trim().is_empty() {
        return (None, Vec::new(), 0);
    }
    let assets_dir = paths.assets_dir();
    let index_path = assets_dir.join("indexes").join(format!("{}.json", index.id));
    // A legacy index predates the hash-addressed layout, so it is also
    // reconstructed by logical name. `prism-core::assets::game_assets_dir` is
    // the authority on where `${game_assets}` points; the same two ids decide
    // what has to be materialised.
    let reconstruct = match index.id.as_str() {
        "legacy" => vec![instance_root.join("minecraft").join("resources")],
        "pre-1.6" => vec![assets_dir.join("virtual").join(&index.id)],
        _ => Vec::new(),
    };
    let plan = AssetPlan {
        id: index.id.clone(),
        index_path: index_path.clone(),
        assets_dir,
        reconstruct,
    };
    // A bare id (`"assets": "17"` with no `assetIndex` object) carries no URL:
    // the index has to come from somewhere else, and saying so beats guessing.
    if index.url.trim().is_empty() {
        return (
            Some(plan),
            Vec::new(),
            usize::from(index_path.is_file()),
        );
    }
    let job = DownloadJob {
        url: index.url.clone(),
        dest: index_path,
        sha1: index.sha1.clone(),
        size: index.size,
        label: format!("asset index {}", index.id),
    };
    (Some(plan), vec![job], 0)
}

/// URL, SHA-1 and size for one planned file of `library`.
///
/// Order of precedence mirrors Prism: an explicit absolute URL wins, then the
/// library's own `downloads` block (artifact for a jar, classifier for a
/// native, both with `${arch}` substituted as requested), and finally its Maven
/// repository joined with the file's path.
fn file_facts(library: &Library, rel: &str, arch: &str) -> Option<(String, String, i64)> {
    if !library.absolute_url.is_empty() {
        return Some((library.absolute_url.clone(), String::new(), 0));
    }
    if let Some(downloads) = &library.mojang_downloads {
        if !library.is_native() {
            if let Some(artifact) = &downloads.artifact {
                if !artifact.url.is_empty() {
                    return Some((artifact.url.clone(), artifact.sha1.clone(), artifact.size));
                }
            }
        }
        for info in downloads.classifiers.values() {
            if info.url.is_empty() {
                continue;
            }
            let Some(path) = info.path.as_deref().filter(|path| !path.is_empty()) else {
                continue;
            };
            if format!("libraries/{}", path.replace("${arch}", arch)) == rel {
                return Some((
                    info.url.replace("${arch}", arch),
                    info.sha1.clone(),
                    info.size,
                ));
            }
        }
    }
    if !library.repository_url.is_empty() {
        let suffix = rel.strip_prefix("libraries/").unwrap_or(rel);
        let base = library.repository_url.trim_end_matches('/');
        return Some((format!("{base}/{suffix}"), String::new(), 0));
    }
    None
}

/// `group:artifact:version` (classifier-qualified when it has one) for logs.
fn library_label(library: &Library) -> String {
    let name = &library.name;
    if name.classifier().is_empty() {
        format!("{}:{}:{}", name.group(), name.artifact(), name.version())
    } else {
        format!(
            "{}:{}:{}:{}",
            name.group(),
            name.artifact(),
            name.version(),
            name.classifier()
        )
    }
}

// ---- running --------------------------------------------------------------

/// Fetch everything the plan lists and extract the natives.
///
/// Two phases rather than one because the asset objects cannot be enumerated
/// until the index is on disk: phase one is the planned files, phase two reads
/// the index it just wrote and fetches the hashes it names.
pub fn run(
    plan: &InstallPlan,
    fetcher: &(dyn Fetcher + Sync),
    threads: usize,
    log: &mut dyn FnMut(String),
) -> InstallReport {
    let mut report = InstallReport {
        present: plan.present,
        problems: plan.problems.clone(),
        ..InstallReport::default()
    };
    if !plan.jobs.is_empty() {
        log(format!("downloading {} file(s)…", plan.jobs.len()));
        let jobs: Vec<(String, PathBuf)> = plan
            .jobs
            .iter()
            .map(|job| (job.url.clone(), job.dest.clone()))
            .collect();
        let results = download_many(fetcher, &jobs, threads.max(1));
        for (index, (url, result)) in results.into_iter().enumerate() {
            let job = &plan.jobs[index];
            match result {
                Ok(bytes) => match verify_download(&job.dest, &job.sha1) {
                    Ok(()) => {
                        report.downloaded += 1;
                        report.bytes += bytes;
                    }
                    Err(reason) => {
                        // A file that fails its digest is worse than no file:
                        // remove it so the next run fetches it again instead of
                        // trusting corrupt bytes.
                        let _ = std::fs::remove_file(&job.dest);
                        report.failed.push(format!("{}: {reason}", job.label));
                    }
                },
                Err(error) => report.failed.push(format!("{}: {error}", job.label)),
            }
            let _ = url;
        }
    }

    if let Some(assets) = &plan.assets {
        run_assets(assets, fetcher, threads, &mut report, log);
    }

    if !plan.natives.is_empty() {
        let rename_jnilib = cfg!(target_os = "macos");
        for jar in &plan.natives {
            if !jar.is_file() {
                continue;
            }
            match prism_loader::archive::extract_zip_file_flat(
                jar,
                &plan.natives_dir,
                rename_jnilib,
            ) {
                Ok(count) => report.natives_extracted += count,
                Err(error) => report
                    .failed
                    .push(format!("extracting {}: {error}", jar.display())),
            }
        }
        if report.natives_extracted > 0 {
            log(format!(
                "extracted {} native file(s) into {}",
                report.natives_extracted,
                plan.natives_dir.display()
            ));
        }
    }
    report
}

/// Phase two: read the index and fetch the objects it names.
fn run_assets(
    assets: &AssetPlan,
    fetcher: &(dyn Fetcher + Sync),
    threads: usize,
    report: &mut InstallReport,
    log: &mut dyn FnMut(String),
) {
    let text = match std::fs::read_to_string(&assets.index_path) {
        Ok(text) => text,
        Err(error) => {
            // A bare index id with no download URL lands here, and it is worth
            // saying plainly rather than reporting a successful install of a
            // game that cannot run.
            report.problems.push(format!(
                "asset index {} is missing ({}) and the profile carries no download URL for it: {error}",
                assets.id,
                assets.index_path.display()
            ));
            return;
        }
    };
    let index = match AssetIndex::parse(&text) {
        Ok(index) => index,
        Err(error) => {
            report.problems.push(format!("asset index {} is unreadable: {error}", assets.id));
            return;
        }
    };
    let mut jobs: Vec<(String, PathBuf)> = Vec::new();
    let mut labels: Vec<String> = Vec::new();
    for (name, object) in &index.objects {
        if object.hash.len() < 2 {
            continue;
        }
        let rel = object_relative_path(&object.hash);
        let dest = assets.assets_dir.join(&rel);
        if let Ok(meta) = std::fs::metadata(&dest) {
            if meta.is_file() && (object.size <= 0 || meta.len() >= object.size as u64) {
                report.present += 1;
                continue;
            }
        }
        jobs.push((
            format!("{ASSET_OBJECT_BASE_URL}/{rel}"),
            dest,
        ));
        labels.push(name.clone());
    }
    if !jobs.is_empty() {
        log(format!("downloading {} asset object(s)…", jobs.len()));
        let results = download_many(fetcher, &jobs, threads.max(1));
        for (index_in_jobs, (url, result)) in results.into_iter().enumerate() {
            match result {
                Ok(bytes) => {
                    report.objects_downloaded += 1;
                    report.bytes += bytes;
                }
                Err(error) => report
                    .failed
                    .push(format!("asset {}: {error}", labels[index_in_jobs])),
            }
            let _ = url;
        }
    }
    reconstruct(assets, &index, report, log);
}

/// Materialise the logical-name layout legacy indexes expect.
///
/// Only reachable for `legacy` (into the instance's `resources/`) and `pre-1.6`
/// (into `assets/virtual/<id>/`); a modern index returns immediately.
fn reconstruct(
    assets: &AssetPlan,
    index: &AssetIndex,
    report: &mut InstallReport,
    log: &mut dyn FnMut(String),
) {
    if assets.reconstruct.is_empty() {
        return;
    }
    let mut copied = 0usize;
    for target in &assets.reconstruct {
        for (name, object) in &index.objects {
            if object.hash.len() < 2 {
                continue;
            }
            let source = assets.assets_dir.join(object_relative_path(&object.hash));
            if !source.is_file() {
                continue;
            }
            let dest = target.join(name);
            if dest.is_file() {
                continue;
            }
            if let Some(parent) = dest.parent() {
                if std::fs::create_dir_all(parent).is_err() {
                    continue;
                }
            }
            if std::fs::copy(&source, &dest).is_ok() {
                copied += 1;
            }
        }
        log(format!("reconstructed {} asset(s) into {}", copied, target.display()));
    }
}

/// Check a downloaded file against the digest the metadata published.
///
/// A library with no published digest (anything served from a Maven repository)
/// is accepted on the strength of having been written atomically by the
/// downloader — there is nothing to compare it against, and hashing it would
/// only prove it is not empty.
pub fn verify_download(path: &Path, expected_sha1: &str) -> Result<(), String> {
    let expected = expected_sha1.trim().to_ascii_lowercase();
    if expected.is_empty() {
        return Ok(());
    }
    let bytes = std::fs::read(path).map_err(|e| format!("reading back {}: {e}", path.display()))?;
    let actual = sha1_hex(&bytes);
    if actual != expected {
        return Err(format!("sha1 mismatch: expected {expected}, got {actual}"));
    }
    Ok(())
}

/// Lowercase hex SHA-1 of `bytes` (Mojang publishes SHA-1, not SHA-256).
pub fn sha1_hex(bytes: &[u8]) -> String {
    use sha1::Digest;
    let mut hasher = sha1::Sha1::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use prism_core::version::{LaunchProfile, Library};
    use prism_net::MapFetcher;
    use serde_json::json;

    fn library_from(value: serde_json::Value) -> Library {
        let mut problems = Vec::new();
        Library::from_json(&value, &mut problems).unwrap()
    }

    fn test_paths() -> (tempfile::TempDir, PrismPaths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = PrismPaths::at(dir.path());
        std::fs::create_dir_all(paths.instances_dir()).unwrap();
        (dir, paths)
    }

    /// A profile with one Mojang-style library (downloads block) and one
    /// Maven-style library (repository url only).
    fn profile() -> LaunchProfile {
        let mut profile = LaunchProfile::default();
        profile.libraries.push(library_from(json!({
            "name": "com.mojang:brigadier:1.0.18",
            "downloads": {
                "artifact": {
                    "path": "com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar",
                    "sha1": "abc",
                    "size": 77,
                    "url": "https://libraries.minecraft.net/com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar"
                }
            }
        })));
        profile.libraries.push(library_from(json!({
            "name": "net.fabricmc:intermediary:1.21.1",
            "url": "https://maven.fabricmc.net/"
        })));
        profile
    }

    #[test]
    fn planning_lists_missing_files_with_their_urls_and_digests() {
        let (_dir, paths) = test_paths();
        let ctx = RuntimeContext::current_host();
        let plan = plan(&paths, &paths.root.join("instances").join("Test"), &profile(), &ctx);
        assert_eq!(plan.jobs.len(), 2, "jobs: {:?}", plan.jobs);
        assert_eq!(plan.present, 0);

        let brigadier = plan
            .jobs
            .iter()
            .find(|job| job.label.starts_with("com.mojang:brigadier"))
            .expect("the Mojang library is planned");
        assert_eq!(
            brigadier.dest,
            paths
                .root
                .join("libraries")
                .join("com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar")
        );
        assert_eq!(brigadier.size, 77);
        assert_eq!(brigadier.sha1, "abc");
        assert!(brigadier.url.starts_with("https://libraries.minecraft.net/"));

        let fabric = plan
            .jobs
            .iter()
            .find(|job| job.label.starts_with("net.fabricmc"))
            .expect("the Maven library is planned");
        assert_eq!(
            fabric.url,
            "https://maven.fabricmc.net/net/fabricmc/intermediary/1.21.1/intermediary-1.21.1.jar"
        );
        assert_eq!(fabric.sha1, "", "a Maven source publishes no digest");
        assert_eq!(fabric.size, 0);
    }

    #[test]
    fn planning_skips_files_that_are_already_there() {
        let (_dir, paths) = test_paths();
        let ctx = RuntimeContext::current_host();
        let dest = paths
            .root
            .join("libraries")
            .join("com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar");
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::write(&dest, vec![0u8; 77]).unwrap();

        let plan = plan(&paths, &paths.root, &profile(), &ctx);
        assert_eq!(plan.present, 1);
        assert_eq!(plan.jobs.len(), 1, "only the missing library is planned");
        assert!(plan.summary().contains("already present"));
    }

    #[test]
    fn a_truncated_cached_file_is_re_fetched() {
        let (_dir, paths) = test_paths();
        let ctx = RuntimeContext::current_host();
        let dest = paths
            .root
            .join("libraries")
            .join("com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar");
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        // 10 bytes against a published size of 77: an interrupted copy.
        std::fs::write(&dest, vec![0u8; 10]).unwrap();
        let plan = plan(&paths, &paths.root, &profile(), &ctx);
        assert_eq!(plan.present, 0);
        assert!(plan.jobs.iter().any(|job| job.dest == dest));
    }

    #[test]
    fn natives_are_planned_from_the_classifier_and_extracted_flat() {
        let (_dir, paths) = test_paths();
        let ctx = RuntimeContext::current_host();
        let mut profile = LaunchProfile::default();
        // Shaped like a modern LWJGL native: the classifier matches this host,
        // so it is the only one planned.
        let classifier = ctx.classifier();
        let native_path = format!("org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-{classifier}.jar");
        profile
            .libraries
            .push(library_from(native_library(&classifier, &native_path, "def", 5)));
        let instance_root = paths.root.join("instances").join("Natives");
        let plan = plan(&paths, &instance_root, &profile, &ctx);
        assert_eq!(plan.jobs.len(), 1, "jobs: {:?}", plan.jobs);
        assert_eq!(plan.natives.len(), 1);
        assert_eq!(plan.natives[0], paths.root.join("libraries").join(&native_path));
        assert_eq!(plan.natives_dir, instance_root.join("natives"));
        assert_eq!(plan.jobs[0].sha1, "def");
    }

    /// A library JSON value for a native jar keyed by `classifier`.
    ///
    /// Built with maps rather than `json!` because the key *is* the host's
    /// classifier, and a computed key cannot be written as a macro literal.
    fn native_library(classifier: &str, path: &str, sha1: &str, size: usize) -> serde_json::Value {
        let mut natives = serde_json::Map::new();
        natives.insert(classifier.to_string(), serde_json::Value::String(classifier.to_string()));
        let mut classifiers = serde_json::Map::new();
        classifiers.insert(
            classifier.to_string(),
            json!({
                "path": path,
                "sha1": sha1,
                "size": size,
                "url": format!("https://libraries.minecraft.net/{path}")
            }),
        );
        json!({
            "name": "org.lwjgl:lwjgl:3.3.1",
            "natives": serde_json::Value::Object(natives),
            "downloads": { "classifiers": serde_json::Value::Object(classifiers) }
        })
    }

    /// Build a zip holding one file (used for the natives phase).
    fn zip_with(name: &str, data: &[u8]) -> Vec<u8> {
        use std::io::Write as _;
        let buf = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(buf);
        let options =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        writer.start_file(name, options).unwrap();
        writer.write_all(data).unwrap();
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn running_the_plan_downloads_verifies_and_extracts() {
        let (_dir, paths) = test_paths();
        let ctx = RuntimeContext::current_host();
        let instance_root = paths.root.join("instances").join("Run");
        let native_path = format!(
            "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-{}.jar",
            ctx.classifier()
        );
        let native_bytes = zip_with("lwjgl.dll", b"native");
        let digest = sha1_hex(&native_bytes);

        let mut profile = LaunchProfile::default();
        profile.libraries.push(library_from(native_library(
            &ctx.classifier(),
            &native_path,
            &digest,
            native_bytes.len(),
        )));
        let plan = plan(&paths, &instance_root, &profile, &ctx);
        assert_eq!(plan.jobs.len(), 1);

        let mut fetcher = MapFetcher::new();
        fetcher.insert(&plan.jobs[0].url, native_bytes.clone());
        let mut lines = Vec::new();
        let report = run(&plan, &fetcher, 2, &mut |line| lines.push(line));

        assert!(report.is_complete(), "failures: {:?}", report.failed);
        assert_eq!(report.downloaded, 1);
        assert_eq!(report.natives_extracted, 1);
        assert!(plan.jobs[0].dest.is_file());
        assert!(instance_root.join("natives").join("lwjgl.dll").is_file());
        assert!(lines.iter().any(|line| line.contains("downloading 1 file(s)")));
    }

    #[test]
    fn a_download_that_fails_its_digest_is_removed_and_reported() {
        let (_dir, paths) = test_paths();
        let ctx = RuntimeContext::current_host();
        let mut profile = LaunchProfile::default();
        profile.libraries.push(library_from(json!({
            "name": "com.mojang:brigadier:1.0.18",
            "downloads": {
                "artifact": {
                    "path": "com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar",
                    "sha1": sha1_hex(b"the real file"),
                    "size": 13,
                    "url": "https://libraries.minecraft.net/brigadier.jar"
                }
            }
        })));
        let plan = plan(&paths, &paths.root, &profile, &ctx);
        assert_eq!(plan.jobs.len(), 1);
        let mut fetcher = MapFetcher::new();
        fetcher.insert(&plan.jobs[0].url, b"a different file".to_vec());

        let report = run(&plan, &fetcher, 1, &mut |_| {});
        assert!(!report.is_complete());
        assert!(report.failed[0].contains("sha1 mismatch"), "{:?}", report.failed);
        assert!(
            !plan.jobs[0].dest.exists(),
            "a file that failed its digest must not be left behind"
        );
    }

    #[test]
    fn a_library_with_no_source_is_reported_rather_than_dropped() {
        let (_dir, paths) = test_paths();
        let ctx = RuntimeContext::current_host();
        let mut profile = LaunchProfile::default();
        // No `downloads` and no `url`: there is nowhere to fetch this from.
        profile
            .libraries
            .push(library_from(json!({ "name": "com.example:orphan:1.0" })));
        let plan = plan(&paths, &paths.root, &profile, &ctx);
        assert!(plan.jobs.is_empty());
        assert_eq!(plan.problems.len(), 1);
        assert!(plan.problems[0].contains("no download source"), "{:?}", plan.problems);
    }

    #[test]
    fn an_instance_local_library_is_never_downloaded() {
        let (_dir, paths) = test_paths();
        let ctx = RuntimeContext::current_host();
        let mut profile = LaunchProfile::default();
        profile.libraries.push(library_from(json!({
            "name": "local.patch:thing:1.0",
            "MMC-hint": "local",
            "url": "https://example.invalid/"
        })));
        let plan = plan(&paths, &paths.root, &profile, &ctx);
        assert!(plan.jobs.is_empty(), "jobs: {:?}", plan.jobs);
        assert!(plan.problems.is_empty(), "problems: {:?}", plan.problems);
    }

    #[test]
    fn the_asset_index_and_its_objects_are_installed() {
        let (_dir, paths) = test_paths();
        let ctx = RuntimeContext::current_host();
        let index_body = json!({
            "objects": {
                "minecraft/sounds/click.ogg": { "hash": "aa11", "size": 4 },
                "pack.mcmeta": { "hash": "bb22", "size": 3 }
            }
        })
        .to_string();
        let index_digest = sha1_hex(index_body.as_bytes());
        let mut profile = LaunchProfile::default();
        profile.minecraft_assets = Some(prism_core::version::AssetIndexInfo {
            path: Some("17.json".into()),
            sha1: index_digest.clone(),
            size: index_body.len() as i64,
            url: "https://piston-meta.mojang.com/v1/packages/17.json".into(),
            total_size: 7,
            id: "17".into(),
            known: true,
        });

        let instance_root = paths.root.join("instances").join("Assets");
        let plan = plan(&paths, &instance_root, &profile, &ctx);
        assert_eq!(plan.jobs.len(), 1, "just the index: {:?}", plan.jobs);
        assert!(plan.assets.is_some());
        assert!(plan.summary().contains("to download"));

        let mut fetcher = MapFetcher::new();
        fetcher.insert(&plan.jobs[0].url, index_body.clone().into_bytes());
        fetcher.insert(
            &format!("{ASSET_OBJECT_BASE_URL}/objects/aa/aa11"),
            b"abcd".to_vec(),
        );
        fetcher.insert(
            &format!("{ASSET_OBJECT_BASE_URL}/objects/bb/bb22"),
            b"abc".to_vec(),
        );
        let mut lines = Vec::new();
        let report = run(&plan, &fetcher, 2, &mut |line| lines.push(line));

        assert!(report.is_complete(), "failures: {:?}", report.failed);
        assert_eq!(report.downloaded, 1, "the index");
        assert_eq!(report.objects_downloaded, 2);
        assert_eq!(report.present, 2);
        assert!(paths.assets_dir().join("indexes").join("17.json").is_file());
        assert!(paths.assets_dir().join("objects").join("aa").join("aa11").is_file());
        assert!(lines.iter().any(|line| line.contains("2 asset object(s)")));
    }

    #[test]
    fn a_second_install_finds_everything_present() {
        let (_dir, paths) = test_paths();
        let ctx = RuntimeContext::current_host();
        let index_body = json!({ "objects": { "a": { "hash": "aa11", "size": 4 } } }).to_string();
        let mut profile = LaunchProfile::default();
        profile.minecraft_assets = Some(prism_core::version::AssetIndexInfo {
            path: None,
            sha1: sha1_hex(index_body.as_bytes()),
            size: index_body.len() as i64,
            url: "https://piston-meta.mojang.com/17.json".into(),
            total_size: 4,
            id: "17".into(),
            known: true,
        });
        let mut fetcher = MapFetcher::new();
        fetcher.insert("https://piston-meta.mojang.com/17.json", index_body.into_bytes());
        fetcher.insert(&format!("{ASSET_OBJECT_BASE_URL}/objects/aa/aa11"), b"abcd".to_vec());

        let first = plan(&paths, &paths.root, &profile, &ctx);
        run(&first, &fetcher, 1, &mut |_| {});
        let second = plan(&paths, &paths.root, &profile, &ctx);
        assert!(second.is_noop(), "jobs: {:?}", second.jobs);
        assert!(second.summary().contains("already installed"));
        let report = run(&second, &MapFetcher::new(), 1, &mut |_| {});
        assert!(report.is_complete());
        assert_eq!(report.objects_downloaded, 0);
        assert_eq!(report.present, 2, "the index and the one object");
    }

    #[test]
    fn a_bare_asset_index_id_is_reported_instead_of_faked() {
        let (_dir, paths) = test_paths();
        let ctx = RuntimeContext::current_host();
        let mut profile = LaunchProfile::default();
        // `"assets": "17"` with no `assetIndex` object: no URL to fetch.
        profile.minecraft_assets = Some(prism_core::version::AssetIndexInfo::bare("17"));
        let plan = plan(&paths, &paths.root, &profile, &ctx);
        assert!(plan.jobs.is_empty());
        let report = run(&plan, &MapFetcher::new(), 1, &mut |_| {});
        assert!(report.problems.iter().any(|p| p.contains("no download URL")));
    }

    #[test]
    fn legacy_indexes_are_reconstructed_by_logical_name() {
        let (_dir, paths) = test_paths();
        let ctx = RuntimeContext::current_host();
        let index_body = json!({
            "objects": { "lang/en_US.lang": { "hash": "cc33", "size": 2 } }
        })
        .to_string();
        let mut profile = LaunchProfile::default();
        profile.minecraft_assets = Some(prism_core::version::AssetIndexInfo {
            path: None,
            sha1: sha1_hex(index_body.as_bytes()),
            size: index_body.len() as i64,
            url: "https://piston-meta.mojang.com/legacy.json".into(),
            total_size: 2,
            id: "legacy".into(),
            known: true,
        });
        let instance_root = paths.root.join("instances").join("Old");
        std::fs::create_dir_all(instance_root.join("minecraft")).unwrap();

        let plan = plan(&paths, &instance_root, &profile, &ctx);
        let mut fetcher = MapFetcher::new();
        fetcher.insert("https://piston-meta.mojang.com/legacy.json", index_body.into_bytes());
        fetcher.insert(&format!("{ASSET_OBJECT_BASE_URL}/objects/cc/cc33"), b"hi".to_vec());
        let mut lines = Vec::new();
        let report = run(&plan, &fetcher, 1, &mut |line| lines.push(line));

        assert!(report.is_complete(), "failures: {:?}", report.failed);
        let reconstructed = instance_root.join("minecraft").join("resources").join("lang/en_US.lang");
        assert!(reconstructed.is_file(), "legacy assets land under resources/");
        assert_eq!(std::fs::read(&reconstructed).unwrap(), b"hi");
        assert!(lines.iter().any(|line| line.contains("reconstructed")));
    }

    #[test]
    fn sha1_hex_matches_the_known_digest_of_abc() {
        assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn verification_accepts_a_missing_digest_but_refuses_a_wrong_one() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.jar");
        std::fs::write(&path, b"hello").unwrap();
        assert!(verify_download(&path, "").is_ok());
        assert!(verify_download(&path, "  ").is_ok());
        assert!(verify_download(&path, &sha1_hex(b"hello")).is_ok());
        assert!(verify_download(&path, &sha1_hex(b"other")).is_err());
        assert!(verify_download(&dir.path().join("missing.jar"), &sha1_hex(b"x")).is_err());
    }

    #[test]
    fn inactive_libraries_are_left_out() {
        let (_dir, paths) = test_paths();
        let mut ctx = RuntimeContext::current_host();
        // A rule that only allows another OS: this library is not for us.
        ctx.system = "windows".to_string();
        let mut profile = LaunchProfile::default();
        profile.libraries.push(library_from(json!({
            "name": "com.example:only-linux:1.0",
            "url": "https://example.invalid/",
            "rules": [{ "action": "allow", "os": { "name": "linux" } }]
        })));
        let plan = plan(&paths, &paths.root, &profile, &ctx);
        assert!(plan.jobs.is_empty(), "jobs: {:?}", plan.jobs);
    }

    #[test]
    fn the_summary_counts_what_it_actually_did() {
        let report = InstallReport {
            downloaded: 3,
            bytes: 2 * 1024 * 1024,
            present: 9,
            natives_extracted: 5,
            objects_downloaded: 2,
            failed: Vec::new(),
            problems: Vec::new(),
        };
        let summary = report.summary();
        assert!(summary.contains("3 file(s)"), "{summary}");
        assert!(summary.contains("2.0 MB"), "{summary}");
        assert!(summary.contains("9 already present"), "{summary}");
        assert!(summary.contains("5 native file(s)"), "{summary}");
        assert!(report.is_complete());
    }
}
