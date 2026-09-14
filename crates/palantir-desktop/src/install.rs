//! Installing what a launch needs: libraries, the client jar, assets and
//! natives.
//!
//! Before this module existed, `launch.rs` could only *report* what was
//! missing: it resolved a pack against a metadata cache that had to already be
//! populated and refused to start when a single classpath jar or the main jar
//! was absent, which meant a freshly created instance could never launch. The
//! missing half is here, and it is deliberately plain filesystem work over
//! `palantir-net`'s downloader:
//!
//! * **Metadata first.** [`palantir_net::OnlineMetaStore`] fetches the version
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
//!   also how Prism stores them, so both launchers share one copy — but the
//!   *URL* they come from is a different path with the same hash in it
//!   ([`palantir_core::assets::object_cdn_path`]). The storage layout has an
//!   `objects/` segment and the CDN layout does not, and the first version of
//!   this module sent every asset request to the storage one, which is a 404 on
//!   every object (see `NEXT_STEPS.md` §19).
//! * **Natives** are extracted flat into `<instance>/natives/` after the jars
//!   land, because that is the directory `-Djava.library.path` points at.
//!
//! Everything here is testable without a network: [`plan`] and [`run`] take a
//! [`Fetcher`], so tests hand them a map of canned bodies and assert on what
//! ended up on disk.
//!
//! **Progress** reaches a caller through a [`Reporter`], which carries the two
//! kinds of report a phase has: [`Reporter::log`] for the few lines worth
//! keeping, and [`Reporter::report`] for the running count a window draws as a
//! bar.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use palantir_core::assets::{object_cdn_path, object_relative_path, AssetIndex};
use palantir_core::paths::PalantirPaths;
use palantir_core::version::{LaunchProfile, Library, RuntimeContext};
use palantir_net::download_many_with_progress;
use palantir_net::meta::Fetcher;

/// Where Mojang serves asset objects from (hash-addressed, first two hex
/// characters as the directory).
pub const ASSET_OBJECT_BASE_URL: &str = "https://resources.download.minecraft.net";

/// Parallel downloads used for the bulk phases.
///
/// Eight used to be the number here, on the Modrinth/Prism argument that it is
/// enough to hide latency without becoming a denial-of-service against Mojang's
/// CDN. Measured against the real object CDN with one keep-alive connection per
/// worker, eight is worth 8.9 MB/s and twenty-four is worth 10.3 MB/s — so the
/// ceiling belongs to the connection, not to the server, and there is 16% above
/// eight for the taking. Sixteen is the compromise: most of that gain, half the
/// sockets, and still well inside what this CDN serves to a single client. The
/// line itself saturates near 9-10 MB/s here, so raising this further would buy
/// nothing.
pub const DEFAULT_THREADS: usize = 16;

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
    /// all: `palantir-net` writes `.part` and renames). A published size is only
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
    paths: &PalantirPaths,
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
        // `${arch}` natives are two *alternatives*, not two files. Prism appends
        // the 32- or the 64-bit jar according to the Java architecture
        // (`LaunchProfile::getLibraryFiles`), and it has to: both jars hold the
        // same file names, so extracting both leaves a 64-bit JVM loading
        // 32-bit libraries. A Java architecture that is neither gets neither,
        // which is what Prism does with it too.
        let arch = match ctx.java_architecture.as_str() {
            "32" => Some("32"),
            "64" => Some("64"),
            _ => None,
        };
        for rel in &files.native32 {
            if arch == Some("32") {
                queued.push((rel.clone(), "32"));
            }
        }
        for rel in &files.native64 {
            if arch == Some("64") {
                queued.push((rel.clone(), "64"));
            }
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
    paths: &PalantirPaths,
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
    // reconstructed by logical name. `palantir-core::assets::game_assets_dir` is
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

// ---- progress -------------------------------------------------------------

/// What a phase is working on, as a *level* rather than a line of text.
///
/// The console gets a handful of lines per phase; a window gets a bar, and a bar
/// needs numbers, not a formatted sentence. Keeping the numbers structured is
/// also what lets the bar and the status text be one fact drawn twice instead of
/// two strings that can drift apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    /// What is being fetched, named the way the phase names itself
    /// (`files`, `asset objects`, `Java 'java-runtime-delta'`).
    pub label: String,
    /// Files finished so far.
    pub done: usize,
    /// Files this phase will fetch in total.
    pub total: usize,
    /// Bytes received so far, as the downloader counted them.
    pub bytes: u64,
}

impl Progress {
    /// A report for a phase that has finished `done` of `total` files.
    pub fn new(label: impl Into<String>, done: usize, total: usize, bytes: u64) -> Progress {
        Progress { label: label.into(), done, total, bytes }
    }

    /// How full the bar is, in `0.0..=1.0`.
    ///
    /// A phase with nothing to fetch reads as full, because being over is the
    /// one thing a bar should say about it — the alternative is a phase that
    /// finished instantly leaving a bar at zero for the next one to move.
    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            return 1.0;
        }
        (self.done as f32 / self.total as f32).clamp(0.0, 1.0)
    }

    /// The same value as a whole percent, for the text beside the bar.
    pub fn percent(&self) -> u32 {
        (self.fraction() * 100.0).round() as u32
    }

    /// Bytes received so far, in megabytes.
    pub fn megabytes(&self) -> f64 {
        self.bytes as f64 / (1024.0 * 1024.0)
    }

    /// One line of text for the status bar: `asset objects 715/5057 (14%)`.
    ///
    /// Short on purpose: this is the same fact as the bar, for the pages the bar
    /// is not drawn on and for a window too narrow to hold both.
    pub fn status_line(&self) -> String {
        format!("{} {}/{} ({}%)", self.label, self.done, self.total, self.percent())
    }
}

/// Where a phase reports what it is doing.
///
/// Two channels in one object, because a phase has both kinds of report and a
/// caller that wires up one and forgets the other is a bug that only shows up on
/// a slow connection:
///
/// * **lines** ([`Reporter::log`]) — few, meaningful, and written to the
///   instance's launch log. What a phase started doing and how it ended belong
///   here.
/// * **levels** ([`Reporter::report`]) — the running count while it works.
///   Frequent, disposable, and deliberately *not* journalled: a log file that
///   grows a line every 2% of a 458 MB download is a log nobody reads.
pub struct Reporter<'a> {
    line: &'a mut dyn FnMut(String),
    /// `None` for a caller that only wants lines (the dry run, and the tests
    /// that assert on what was said rather than on how far it got).
    progress: Option<&'a mut dyn FnMut(Progress)>,
}

impl<'a> Reporter<'a> {
    /// A reporter with both channels.
    pub fn new(
        line: &'a mut dyn FnMut(String),
        progress: &'a mut dyn FnMut(Progress),
    ) -> Reporter<'a> {
        Reporter { line, progress: Some(progress) }
    }

    /// A reporter for a caller with nowhere to draw: lines still go somewhere,
    /// levels are dropped.
    pub fn lines_only(line: &'a mut dyn FnMut(String)) -> Reporter<'a> {
        Reporter { line, progress: None }
    }

    /// Report a line.
    pub fn log(&mut self, line: impl Into<String>) {
        (self.line)(line.into());
    }

    /// Report a level.
    pub fn report(&mut self, progress: Progress) {
        if let Some(sink) = self.progress.as_mut() {
            sink(progress);
        }
    }
}

// ---- running --------------------------------------------------------------

/// Fetch everything the plan lists and extract the natives.
///
/// Two phases rather than one because the asset objects cannot be enumerated
/// until the index is on disk: phase one is the planned files, phase two reads
/// the index it just wrote and fetches the hashes it names.
///
/// Both phases report through `reporter` while they work (see
/// [`download_with_progress`]): a phase that fetches thousands of small files
/// says nothing between starting and finishing otherwise, and a silent screen
/// is indistinguishable from a hang — which is how a resumable install gets
/// killed by the person waiting for it.
pub fn run(
    plan: &InstallPlan,
    fetcher: &(dyn Fetcher + Sync),
    threads: usize,
    reporter: &mut Reporter,
) -> InstallReport {
    let mut report = InstallReport {
        present: plan.present,
        problems: plan.problems.clone(),
        ..InstallReport::default()
    };
    if !plan.jobs.is_empty() {
        reporter.log(format!("downloading {} file(s)…", plan.jobs.len()));
        let jobs: Vec<(String, PathBuf)> = plan
            .jobs
            .iter()
            .map(|job| (job.url.clone(), job.dest.clone()))
            .collect();
        let started = Instant::now();
        let bytes_before = report.bytes;
        let results = download_with_progress(fetcher, &jobs, threads, "files", reporter);
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
        // `report.downloaded` is this phase's count, successes only: a file
        // that failed its digest is reported on its own line right after, and a
        // phase-end line that counted it as done would be the one place the
        // launcher claimed to have installed something it removed.
        reporter.log(phase_done_line(
            "files",
            report.downloaded,
            report.bytes - bytes_before,
            started.elapsed(),
        ));
    }

    if let Some(assets) = &plan.assets {
        run_assets(assets, fetcher, threads, &mut report, reporter);
    }

    if !plan.natives.is_empty() {
        let rename_jnilib = cfg!(target_os = "macos");
        for jar in &plan.natives {
            if !jar.is_file() {
                continue;
            }
            match palantir_loader::archive::extract_zip_file_flat(
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
            reporter.log(format!(
                "extracted {} native file(s) into {}",
                report.natives_extracted,
                plan.natives_dir.display()
            ));
        }
    }
    report
}

/// Progress reports a bulk phase draws at most.
///
/// One report per ~2% of the phase, so a 5000-file asset index gets a report
/// every hundred files or so: enough to keep a bar moving without asking the
/// window to redraw for every file.
const PROGRESS_REPORTS: usize = 50;

/// The number of finished files between two progress reports.
///
/// Rounded up, so the count of reports stays near [`PROGRESS_REPORTS`] rather
/// than doubling for a phase whose size does not divide evenly.
fn progress_step(total: usize) -> usize {
    total.div_ceil(PROGRESS_REPORTS).max(1)
}

/// The line a phase leaves behind when it is over.
///
/// The per-tick lines became a bar, and a bar leaves nothing on disk. The run
/// log still has to answer the question it was read for — how long a first
/// install of hundreds of megabytes actually took — so each phase writes one
/// line when it finishes, with what it moved and how long it took to move it.
fn phase_done_line(label: &str, files: usize, bytes: u64, elapsed: Duration) -> String {
    format!(
        "{label}: done — {files} file(s), {:.1} MB in {:.1} s",
        bytes as f64 / (1024.0 * 1024.0),
        elapsed.as_secs_f64()
    )
}

/// Fetch every job in parallel, reporting progress while it works.
///
/// The downloads stay parallel inside [`download_many_with_progress`]; the
/// reporting happens on this thread, once per finished file, so the reports
/// reach `reporter` in order and need no locking. The first file and the last are
/// always reported — the first says the phase is alive, the last that it is
/// over — and the rest follow [`progress_step`].
pub(crate) fn download_with_progress(
    fetcher: &(dyn Fetcher + Sync),
    jobs: &[(String, PathBuf)],
    threads: usize,
    label: &str,
    reporter: &mut Reporter,
) -> Vec<(String, Result<u64, palantir_net::Error>)> {
    let total = jobs.len();
    let step = progress_step(total);
    let mut reported = 0usize;
    download_many_with_progress(fetcher, jobs, threads.max(1), &mut |done, bytes| {
        if done == 1 || done == total || done - reported >= step {
            reported = done;
            reporter.report(Progress::new(label, done, total, bytes));
        }
    })
}

/// Phase two: read the index and fetch the objects it names.
fn run_assets(
    assets: &AssetPlan,
    fetcher: &(dyn Fetcher + Sync),
    threads: usize,
    report: &mut InstallReport,
    reporter: &mut Reporter,
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
        // The path on disk, not the path on the CDN: separate helpers, and this
        // is the one place both are in play.
        let dest = assets.assets_dir.join(object_relative_path(&object.hash));
        if let Ok(meta) = std::fs::metadata(&dest) {
            if meta.is_file() && (object.size <= 0 || meta.len() >= object.size as u64) {
                report.present += 1;
                continue;
            }
        }
        jobs.push((asset_object_url(&object.hash), dest));
        labels.push(name.clone());
    }
    if !jobs.is_empty() {
        reporter.log(format!("downloading {} asset object(s)…", jobs.len()));
        let started = Instant::now();
        let bytes_before = report.bytes;
        let objects_before = report.objects_downloaded;
        let results = download_with_progress(fetcher, &jobs, threads, "asset objects", reporter);
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
        reporter.log(phase_done_line(
            "asset objects",
            report.objects_downloaded - objects_before,
            report.bytes - bytes_before,
            started.elapsed(),
        ));
    }
    reconstruct(assets, &index, reporter);
}

/// Where one asset object is fetched from.
///
/// The hash is the same one the file is stored under, but the *path* is not:
/// the CDN serves `/<xx>/<hash>` and this launcher stores
/// `objects/<xx>/<hash>` under its data root. Building the URL from the storage
/// path is what sent all 5057 object requests of a first install to a 404
/// (`NEXT_STEPS.md` §19), so the two are separate helpers with separate tests.
pub fn asset_object_url(hash: &str) -> String {
    format!("{ASSET_OBJECT_BASE_URL}/{}", object_cdn_path(hash))
}

/// Materialise the logical-name layout legacy indexes expect.
///
/// Only reachable for `legacy` (into the instance's `resources/`) and `pre-1.6`
/// (into `assets/virtual/<id>/`); a modern index returns immediately.
fn reconstruct(assets: &AssetPlan, index: &AssetIndex, reporter: &mut Reporter) {
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
        reporter.log(format!("reconstructed {} asset(s) into {}", copied, target.display()));
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
    use palantir_core::version::{LaunchProfile, Library};
    use palantir_net::MapFetcher;
    use serde_json::json;

    fn library_from(value: serde_json::Value) -> Library {
        let mut problems = Vec::new();
        Library::from_json(&value, &mut problems).unwrap()
    }

    fn test_paths() -> (tempfile::TempDir, PalantirPaths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = PalantirPaths::at(dir.path());
        std::fs::create_dir_all(paths.instances_dir()).unwrap();
        (dir, paths)
    }

    /// The URL one asset object is served from, spelled out here rather than
    /// built with the production helper.
    ///
    /// A fixture that derives its expected URL the same way the code derives the
    /// real one agrees with a mistake, which is exactly how every asset request
    /// went to `/objects/<xx>/<hash>` for a 404 without a single test noticing
    /// (`NEXT_STEPS.md` §19). This is the literal shape the resource CDN serves.
    fn cdn_url(hash: &str) -> String {
        let prefix = hash.get(0..2).unwrap_or("");
        format!("{ASSET_OBJECT_BASE_URL}/{prefix}/{hash}")
    }

    /// Run a plan and keep every line it wrote (the levels go nowhere).
    fn run_lines(
        plan: &InstallPlan,
        fetcher: &(dyn Fetcher + Sync),
        threads: usize,
    ) -> (InstallReport, Vec<String>) {
        let mut lines: Vec<String> = Vec::new();
        // Each tap is bound inside the block that uses it, so the borrows end
        // with the block and the vectors can be read (and returned) afterwards.
        let report = {
            let mut collect = |line: String| lines.push(line);
            let mut reporter = Reporter::lines_only(&mut collect);
            run(plan, fetcher, threads, &mut reporter)
        };
        (report, lines)
    }

    /// Run a plan and keep both kinds of report, which is what a test that
    /// asserts on the bar and on the log at once needs.
    fn run_both(
        plan: &InstallPlan,
        fetcher: &(dyn Fetcher + Sync),
        threads: usize,
    ) -> (InstallReport, Vec<String>, Vec<Progress>) {
        let mut lines: Vec<String> = Vec::new();
        let mut levels: Vec<Progress> = Vec::new();
        let report = {
            let mut collect_line = |line: String| lines.push(line);
            let mut collect_level = |level: Progress| levels.push(level);
            let mut reporter = Reporter::new(&mut collect_line, &mut collect_level);
            run(plan, fetcher, threads, &mut reporter)
        };
        (report, lines, levels)
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
        // Shaped like Mojang's LWJGL native: the `natives` map is keyed by OS
        // and its value is the classifier, so only this host's jar is planned.
        let classifier = format!("natives-{}", ctx.system);
        let native_path = format!("org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-{classifier}.jar");
        profile
            .libraries
            .push(library_from(native_library(&ctx, &classifier, &native_path, "def", 5)));
        let instance_root = paths.root.join("instances").join("Natives");
        let plan = plan(&paths, &instance_root, &profile, &ctx);
        assert_eq!(plan.jobs.len(), 1, "jobs: {:?}", plan.jobs);
        assert_eq!(plan.natives.len(), 1);
        assert_eq!(plan.natives[0], paths.root.join("libraries").join(&native_path));
        assert_eq!(plan.natives_dir, instance_root.join("natives"));
        assert_eq!(plan.jobs[0].sha1, "def");
    }

    #[test]
    fn a_placeholder_arch_native_plans_only_the_javas_own_width() {
        // `natives-windows-${arch}` libraries ship a 32- and a 64-bit jar, and
        // both contain the same file names. Prism appends exactly one of them,
        // chosen by the *Java* architecture; planning both would extract 32-bit
        // libraries over the 64-bit ones and leave the JVM loading them.
        let (_dir, paths) = test_paths();
        let mut ctx = RuntimeContext::current_host();
        let mut profile = LaunchProfile::default();
        // Both widths are present in the metadata, keyed the way Mojang keys
        // them, and only the `natives` map value carries `${arch}`.
        let mut natives = serde_json::Map::new();
        for key in [ctx.system.clone(), ctx.classifier()] {
            natives.insert(
                key,
                serde_json::Value::String(format!("natives-{}-${{arch}}", ctx.system)),
            );
        }
        let mut classifiers = serde_json::Map::new();
        for width in ["32", "64"] {
            let path = format!(
                "org/lwjgl/lwjgl/lwjgl-platform/2.9.2/lwjgl-platform-2.9.2-natives-{}-{width}.jar",
                ctx.system
            );
            classifiers.insert(
                format!("natives-{}-{width}", ctx.system),
                json!({
                    "path": path,
                    "sha1": "abc",
                    "size": 4,
                    "url": format!("https://libraries.minecraft.net/{path}")
                }),
            );
        }
        profile.libraries.push(library_from(json!({
            "name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.2",
            "natives": serde_json::Value::Object(natives),
            "downloads": { "classifiers": serde_json::Value::Object(classifiers) }
        })));

        ctx.java_architecture = "64".into();
        let sixty_four = plan(&paths, &paths.root, &profile, &ctx);
        assert_eq!(
            sixty_four.natives.len(),
            1,
            "one width, not both: {:?}",
            sixty_four.natives
        );
        assert_eq!(sixty_four.jobs.len(), 1, "jobs: {:?}", sixty_four.jobs);
        assert!(
            sixty_four.natives[0].to_string_lossy().ends_with("-64.jar"),
            "a 64-bit JVM gets the 64-bit jar: {:?}",
            sixty_four.natives[0]
        );

        ctx.java_architecture = "32".into();
        let thirty_two = plan(&paths, &paths.root, &profile, &ctx);
        assert_eq!(thirty_two.natives.len(), 1, "natives: {:?}", thirty_two.natives);
        assert!(
            thirty_two.natives[0].to_string_lossy().ends_with("-32.jar"),
            "a 32-bit JVM gets the 32-bit jar: {:?}",
            thirty_two.natives[0]
        );
    }

    /// A native library JSON value, shaped the way a version file shapes one.
    ///
    /// The `natives` map is keyed by *OS* (`windows`/`linux`/`osx`) and its value
    /// is the classifier — which is what makes the file name
    /// `lwjgl-3.3.1-natives-windows.jar` — while `downloads.classifiers` is keyed
    /// by that same classifier. Keying the map by classifier instead, as this
    /// fixture first did, describes a shape no real version file has: the planner
    /// then builds a path from the classifier and finds no matching download.
    ///
    /// Built with maps rather than `json!` because the keys are computed from the
    /// host, and a computed key cannot be written as a macro literal.
    fn native_library(
        ctx: &RuntimeContext,
        classifier: &str,
        path: &str,
        sha1: &str,
        size: usize,
    ) -> serde_json::Value {
        let mut natives = serde_json::Map::new();
        // The OS key is what Mojang writes; the precise `<system>-<arch>` key is
        // what some pack metadata writes, and it is the only one a non-x86_64
        // host consults — `compatible_native` falls back to the OS key for the
        // legacy architectures only. Both keeps this fixture true everywhere.
        for key in [ctx.system.clone(), ctx.classifier()] {
            natives.insert(key, serde_json::Value::String(classifier.to_string()));
        }
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
        let classifier = format!("natives-{}", ctx.system);
        let native_path = format!("org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-{classifier}.jar");
        let native_bytes = zip_with("lwjgl.dll", b"native");
        let digest = sha1_hex(&native_bytes);

        let mut profile = LaunchProfile::default();
        profile.libraries.push(library_from(native_library(
            &ctx,
            &classifier,
            &native_path,
            &digest,
            native_bytes.len(),
        )));
        let plan = plan(&paths, &instance_root, &profile, &ctx);
        assert_eq!(plan.jobs.len(), 1);

        let mut fetcher = MapFetcher::new();
        fetcher.insert(&plan.jobs[0].url, native_bytes.clone());
        let (report, lines) = run_lines(&plan, &fetcher, 2);

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

        let (report, _) = run_lines(&plan, &fetcher, 1);
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
        profile.minecraft_assets = Some(palantir_core::version::AssetIndexInfo {
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
        fetcher.insert(&cdn_url("aa11"), b"abcd".to_vec());
        fetcher.insert(&cdn_url("bb22"), b"abc".to_vec());
        let (report, lines) = run_lines(&plan, &fetcher, 2);

        assert!(report.is_complete(), "failures: {:?}", report.failed);
        assert_eq!(report.downloaded, 1, "the index");
        assert_eq!(report.objects_downloaded, 2);
        assert_eq!(report.present, 0, "nothing was on disk before this run");
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
        profile.minecraft_assets = Some(palantir_core::version::AssetIndexInfo {
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
        fetcher.insert(&cdn_url("aa11"), b"abcd".to_vec());

        let first = plan(&paths, &paths.root, &profile, &ctx);
        run_lines(&first, &fetcher, 1);
        let second = plan(&paths, &paths.root, &profile, &ctx);
        assert!(second.is_noop(), "jobs: {:?}", second.jobs);
        assert!(second.summary().contains("already installed"));
        let (report, _) = run_lines(&second, &MapFetcher::new(), 1);
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
        profile.minecraft_assets = Some(palantir_core::version::AssetIndexInfo::bare("17"));
        let plan = plan(&paths, &paths.root, &profile, &ctx);
        assert!(plan.jobs.is_empty());
        let (report, _) = run_lines(&plan, &MapFetcher::new(), 1);
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
        profile.minecraft_assets = Some(palantir_core::version::AssetIndexInfo {
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
        fetcher.insert(&cdn_url("cc33"), b"hi".to_vec());
        let (report, lines) = run_lines(&plan, &fetcher, 1);

        assert!(report.is_complete(), "failures: {:?}", report.failed);
        let reconstructed = instance_root.join("minecraft").join("resources").join("lang/en_US.lang");
        assert!(reconstructed.is_file(), "legacy assets land under resources/");
        assert_eq!(std::fs::read(&reconstructed).unwrap(), b"hi");
        assert!(lines.iter().any(|line| line.contains("reconstructed")));
    }

    /// The whole URL, spelled out as the literal Mojang's CDN serves it.
    ///
    /// The fixture helpers build their URLs from the hash, so they can only
    /// disagree with production about the *path rule*; this is the one place that
    /// writes down the finished string, which is what makes a change to the base
    /// URL or to the path a disagreement with a recorded fact instead of with
    /// another copy of the same computation.
    #[test]
    fn the_asset_object_url_is_the_cdn_layout() {
        assert_eq!(
            asset_object_url("9ea1b80ddb116f0355d5f9107ba8c6c4d20b44f5"),
            "https://resources.download.minecraft.net/9e/9ea1b80ddb116f0355d5f9107ba8c6c4d20b44f5"
        );
        assert_eq!(
            asset_object_url("9ea1b80ddb116f0355d5f9107ba8c6c4d20b44f5")
                .strip_prefix(ASSET_OBJECT_BASE_URL)
                .unwrap(),
            "/9e/9ea1b80ddb116f0355d5f9107ba8c6c4d20b44f5",
            "the CDN path has no `objects/` segment; the storage path does"
        );
        // The storage path is still what the file is called on disk, and the two
        // must stay different functions: the bug was one helper serving both.
        assert!(object_relative_path("aabb").starts_with("objects/"));
        assert!(!object_cdn_path("aabb").starts_with("objects/"));
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

    /// A phase of thousands of small files has to show it is moving without
    /// printing a line per file: the first file says the phase started before
    /// any real delay, the last says it finished, and the ones between are
    /// spaced by [`progress_step`]. None of them is a console line — they are
    /// the levels a window draws as a bar.
    #[test]
    fn a_bulk_phase_reports_progress_without_narrating_every_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut fetcher = MapFetcher::new();
        let mut jobs: Vec<(String, PathBuf)> = Vec::new();
        const FILES: usize = 1000;
        const EACH: usize = 4096;
        for index in 0..FILES {
            let url = format!("https://cdn.invalid/object-{index:04}");
            fetcher.insert(&url, vec![b'x'; EACH]);
            jobs.push((url, dir.path().join(format!("object-{index:04}"))));
        }

        let mut lines: Vec<String> = Vec::new();
        let mut levels: Vec<Progress> = Vec::new();
        let results = {
            let mut collect_line = |line: String| lines.push(line);
            let mut collect_level = |level: Progress| levels.push(level);
            let mut reporter = Reporter::new(&mut collect_line, &mut collect_level);
            download_with_progress(&fetcher, &jobs, 4, "files", &mut reporter)
        };

        assert_eq!(results.len(), FILES);
        assert!(
            results.iter().all(|(_, result)| result.is_ok()),
            "every file arrived"
        );
        assert!(
            lines.is_empty(),
            "a bulk phase puts nothing on the console between starting and \
             finishing — the count is a level, not a line: {lines:?}"
        );
        let first = levels.first().expect("the phase reports its first file");
        assert_eq!((first.done, first.total), (1, FILES), "the phase is alive at once");
        let last = levels.last().unwrap();
        assert_eq!(last.done, FILES, "and says when it is over");
        assert_eq!(last.percent(), 100);
        assert_eq!(last.fraction(), 1.0);
        assert_eq!(last.bytes, (FILES * EACH) as u64, "with the bytes it carried");
        assert!(
            levels.len() <= PROGRESS_REPORTS + 2,
            "a report per file would be {FILES} redraws, got {}: {levels:?}",
            levels.len()
        );
    }

    /// The bar and the status text are one fact, and a phase with nothing left
    /// to fetch reads as full rather than as an empty bar for the next phase to
    /// move.
    #[test]
    fn a_progress_report_reads_as_a_fraction_and_a_line() {
        let empty = Progress::new("files", 0, 0, 0);
        assert_eq!(empty.fraction(), 1.0, "a phase with no work is over");
        assert_eq!(empty.percent(), 100);
        assert_eq!(empty.status_line(), "files 0/0 (100%)");

        let half = Progress::new("asset objects", 715, 5057, 65_000_000);
        assert!((half.fraction() - 0.141_39).abs() < 0.0001, "got {}", half.fraction());
        assert_eq!(half.percent(), 14);
        assert_eq!(half.status_line(), "asset objects 715/5057 (14%)");
        assert!((half.megabytes() - 61.99).abs() < 0.02, "got {}", half.megabytes());

        // A report that outran its total (a phase whose size changed under it)
        // still has to stay inside the bar.
        let over = Progress::new("files", 9, 4, 0);
        assert_eq!(over.fraction(), 1.0);
        assert_eq!(over.percent(), 100);
    }

    #[test]
    fn the_progress_cadence_is_a_step_not_a_report_per_file() {
        assert_eq!(progress_step(0), 1, "an empty phase still finishes in one step");
        assert_eq!(progress_step(1), 1);
        assert_eq!(progress_step(PROGRESS_REPORTS), 1);
        assert_eq!(progress_step(5057), 102, "a report every ~2% of 5057 files");
        assert_eq!(progress_step(120), 3, "rounded up, so 120 files get ~40 reports");
        // What a phase leaves on disk when the levels it drew are gone.
        assert_eq!(
            phase_done_line("asset objects", 5057, 480 * 1024 * 1024, Duration::from_secs(91)),
            "asset objects: done — 5057 file(s), 480.0 MB in 91.0 s"
        );
        assert_eq!(
            phase_done_line("files", 0, 0, Duration::from_millis(7)),
            "files: done — 0 file(s), 0.0 MB in 0.0 s"
        );
    }

    /// The phase that made the launcher look dead is the asset one, so both of
    /// its reports — the level the bar draws and the line the log keeps — have
    /// to come out of a real [`run`] and not just the helper.
    #[test]
    fn the_asset_phase_reports_progress_while_it_downloads() {
        let (_dir, paths) = test_paths();
        let ctx = RuntimeContext::current_host();
        const OBJECTS: usize = 120;
        let mut objects = serde_json::Map::new();
        let mut bodies: Vec<(String, String)> = Vec::new();
        for index in 0..OBJECTS {
            let body = format!("object-{index}");
            let hash = sha1_hex(body.as_bytes());
            objects.insert(
                format!("file-{index}.bin"),
                json!({ "hash": hash, "size": body.len() }),
            );
            bodies.push((hash, body));
        }
        let index_body = json!({ "objects": objects }).to_string();
        let mut profile = LaunchProfile::default();
        profile.minecraft_assets = Some(palantir_core::version::AssetIndexInfo {
            path: None,
            sha1: sha1_hex(index_body.as_bytes()),
            size: index_body.len() as i64,
            url: "https://piston-meta.mojang.com/120.json".into(),
            total_size: 0,
            id: "120".into(),
            known: true,
        });
        let mut fetcher = MapFetcher::new();
        fetcher.insert(
            "https://piston-meta.mojang.com/120.json",
            index_body.into_bytes(),
        );
        for (hash, body) in bodies {
            fetcher.insert(&cdn_url(&hash), body.into_bytes());
        }

        let plan = plan(&paths, &paths.root, &profile, &ctx);
        let (report, lines, levels) = run_both(&plan, &fetcher, 4);

        assert!(report.is_complete(), "failures: {:?}", report.failed);
        assert_eq!(report.objects_downloaded, OBJECTS);
        assert!(
            lines
                .iter()
                .any(|line| line.contains("downloading 120 asset object(s)")),
            "the phase is announced: {lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("asset objects: done — 120 file(s)")),
            "and its outcome is one line, with what it moved: {lines:?}"
        );
        assert!(
            !lines.iter().any(|line| line.contains("/120 (")),
            "no tick line survives on the console: {lines:?}"
        );
        // Two phases fetch here: the index in the file phase, then the objects.
        // Both report, and each names itself — a bar that said "files" while the
        // 458 MB moved would not say what the megabytes are for.
        assert!(
            levels.first().is_some_and(|level| level.label == "files"),
            "the index is fetched in the file phase: {levels:?}"
        );
        let objects: Vec<&Progress> = levels
            .iter()
            .filter(|level| level.label == "asset objects")
            .collect();
        assert_eq!(
            objects.len() + 1,
            levels.len(),
            "every level is either the index or an object: {levels:?}"
        );
        let first = objects.first().expect("the phase reports its first object");
        assert_eq!((first.done, first.total), (1, OBJECTS));
        let last = objects.last().unwrap();
        assert_eq!(last.done, OBJECTS, "the last object is reported");
        assert_eq!(last.percent(), 100);
        assert!(
            objects.len() > 1 && objects.len() < OBJECTS / 2,
            "a few reports, not one per object: {}",
            objects.len()
        );
    }
}
