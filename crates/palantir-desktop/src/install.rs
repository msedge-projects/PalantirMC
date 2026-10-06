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
//! [`Wire`], so a test hands it the engine's own scripted server and asserts on
//! what ended up on disk -- and on what was *asked for*, which is the half a
//! hand-written stand-in for a downloader could never say.
//!
//! **Progress** reaches a caller through a [`Reporter`], which carries the two
//! kinds of report a phase has: [`Reporter::log`] for the few lines worth
//! keeping, and [`Reporter::report`] for the running count a window draws as a
//! bar.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use palantir_core::assets::{object_cdn_path, object_relative_path, AssetIndex};
use palantir_core::instance::Instance;
use palantir_core::paths::PalantirPaths;
use palantir_core::version::{LaunchProfile, Library, RuntimeContext};
use crate::wire::{FileJob, Wire};

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

/// A native jar to unpack, with the entries its own metadata excludes.
///
/// The list travels with the jar rather than being looked up again at extract
/// time because the library that named the jar is the only thing that knows it:
/// `extract.exclude` is a property of the component, and flattening every entry
/// regardless is what put `META-INF/MANIFEST.MF` on the JVM's library path as
/// `natives/MANIFEST.MF`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeJar {
    /// The jar in the shared library cache.
    pub path: PathBuf,
    /// `extract.exclude` prefixes from the library (`["META-INF/"]`).
    pub excludes: Vec<String>,
}

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

    /// Whether the destination is there *and* is the file the metadata names.
    ///
    /// The repair's rule, and the one a launch does not pay for: hashing a present
    /// file is a read of everything installed, while a repair is the one moment a
    /// reader is asking for exactly that. A source that publishes no digest cannot
    /// be checked, so it keeps [`DownloadJob::satisfied`]'s answer -- the same
    /// "an empty expectation means do not check" rule [`verify_download`] states.
    pub fn verified(&self) -> bool {
        self.satisfied() && verify_download(&self.dest, &self.sha1).is_ok()
    }
}

/// Whether a plan trusts the files that are already on disk.
///
/// A named rule rather than a `bool` at the call site, because the two answers
/// are different questions and the difference is the whole of what a repair is:
/// [`Existing::Trust`] is "is something of the right size there", which is what a
/// launch asks on its way to the game, and [`Existing::Verify`] is "is the file
/// the one the metadata names", which is what a reader pressing *Repair* asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Existing {
    /// Leave a file that is present and plausibly complete alone.
    #[default]
    Trust,
    /// Hash every present file against the digest its metadata publishes, and plan
    /// the ones that do not match -- `repair`, in the reference's own word.
    Verify,
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
    pub natives: Vec<NativeJar>,
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
    /// The question the plan's file checks answered ([`Existing`]).
    ///
    /// Carried on the plan rather than asked at the call site again, because
    /// [`run`] asks it of files the plan does not list: the asset objects are
    /// enumerated from the index at run time, and a repair that verified its
    /// libraries and then trusted every asset object would be checking half of
    /// what the reader pressed the button for.
    pub existing: Existing,
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
        self.failed.is_empty() && self.problems.is_empty()
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
        if !self.problems.is_empty() {
            out.push_str(&format!(", {} problem(s)", self.problems.len()));
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
    existing: Existing,
) -> InstallPlan {
    let mut plan = InstallPlan {
        natives_dir: instance_root.join("natives"),
        existing,
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
                plan.natives.push(NativeJar {
                    path: dest.clone(),
                    excludes: library.extract_excludes.clone(),
                });
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
                    if keeps(&job, existing) {
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
        if keeps(&job, existing) {
            plan.present += 1;
        } else {
            plan.total_bytes += job.size.max(0);
            plan.jobs.push(job);
        }
    }
    plan.present += index_present;
    plan
}

/// Whether a planned file can be left where it is, by [`Existing`]'s rule.
fn keeps(job: &DownloadJob, existing: Existing) -> bool {
    match existing {
        Existing::Trust => job.satisfied(),
        Existing::Verify => job.verified(),
    }
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
        if let Some(artifact) = &downloads.artifact {
            // An ordinary library *is* its artifact. A native has to be the file
            // the metadata named for it: the classic shape names it in
            // `downloads.classifiers`, and a native that is its own entry
            // (`org.lwjgl:lwjgl:3.3.3:natives-windows`) names it as that entry's
            // artifact, which is why the path is compared rather than assumed.
            let is_this_file = !library.is_native()
                || artifact.path.as_deref()
                    .map(|path| format!("libraries/{path}") == rel)
                    .unwrap_or(false);
            if is_this_file && !artifact.url.is_empty() {
                return Some((artifact.url.clone(), artifact.sha1.clone(), artifact.size));
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
    /// Whether this level has no total to be a fraction of.
    ///
    /// A separate flag rather than a reading of `total`, because `total`
    /// already means something else: a phase with nothing to fetch is *over*,
    /// and a bar that said "still working" for it would be wrong in the other
    /// direction.
    pub starting: bool,
}

impl Progress {
    /// A report for a phase that has finished `done` of `total` files.
    pub fn new(label: impl Into<String>, done: usize, total: usize, bytes: u64) -> Progress {
        Progress { label: label.into(), done, total, bytes, starting: false }
    }

    /// A report for work whose length is not known yet.
    ///
    /// The launch is what needs this: signing in, resolving a version,
    /// installing a Java runtime and unpacking natives have no countable total
    /// until they are over, and a bar that invented a percentage for them would
    /// be the one place this launcher's progress lied. The views answer
    /// [`Progress::is_indeterminate`] with a segment sliding along the bar
    /// instead of a fill, which says "working" without saying "this much".
    pub fn starting(label: impl Into<String>) -> Progress {
        Progress { label: label.into(), done: 0, total: 0, bytes: 0, starting: true }
    }

    /// Whether this level is a report of *activity* rather than of progress.
    pub fn is_indeterminate(&self) -> bool {
        self.starting
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
    #[cfg(test)]
    pub fn megabytes(&self) -> f64 {
        self.bytes as f64 / (1024.0 * 1024.0)
    }

    /// One line of text for the status bar: `asset objects 715/5057 (14%)`.
    ///
    /// Short on purpose: this is the same fact as the bar, for the pages the bar
    /// is not drawn on and for a window too narrow to hold both. An
    /// indeterminate level has no numbers to print, so it prints what it is
    /// doing and nothing else.
    pub fn status_line(&self) -> String {
        if self.is_indeterminate() {
            return format!("{}…", self.label);
        }
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
    ///
    /// Test-only: every caller in the shell has a bar to fill, so only the
    /// tests that collect lines need a reporter with no progress channel.
    #[cfg(test)]
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

    /// Run `f` with both taps, borrowed separately.
    ///
    /// For a helper that takes a line sink and a level sink as they are rather
    /// than through this type: the launch's Java search reaches `pick_java` that
    /// way, and its last answer can be a runtime fetched from the metadata
    /// service -- a transfer that belongs on the bar like every other one here. A
    /// caller holding no level channel gets a sink that drops them, rather than a
    /// closure of its own that would report a fetch into nothing.
    pub fn with_taps<R>(
        &mut self,
        f: impl FnOnce(&mut dyn FnMut(String), &mut dyn FnMut(Progress)) -> R,
    ) -> R {
        match self.progress.as_mut() {
            Some(sink) => f(&mut *self.line, &mut **sink),
            None => {
                let mut nowhere = |_: Progress| {};
                f(&mut *self.line, &mut nowhere)
            }
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
pub fn run(plan: &InstallPlan, wire: &Wire, threads: usize, reporter: &mut Reporter) -> InstallReport {
    let mut report = InstallReport {
        present: plan.present,
        problems: plan.problems.clone(),
        ..InstallReport::default()
    };
    if !plan.jobs.is_empty() {
        reporter.log(format!("downloading {} file(s)…", plan.jobs.len()));
        let jobs: Vec<FileJob> = plan
            .jobs
            .iter()
            .map(|job| FileJob::new(&job.url, &job.dest, &job.sha1))
            .collect();
        let started = Instant::now();
        let bytes_before = report.bytes;
        let results = download_with_progress(wire, &jobs, threads, "files", reporter);
        for (index, result) in results.into_iter().enumerate() {
            let job = &plan.jobs[index];
            // No digest check here any more: the transfer that wrote the file is
            // the one that checks it, against whatever kind of digest the
            // publisher stated, and it removes a mismatch *before* the rename.
            // A file that reached `dest` at all is therefore the file the
            // metadata described, and checking again here would read every
            // downloaded byte a second time to answer a question that is settled.
            match result {
                Ok(bytes) => {
                    report.downloaded += 1;
                    report.bytes += bytes;
                }
                Err(error) => report.failed.push(format!("{}: {error}", job.label)),
            }
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
        run_assets(assets, wire, threads, plan.existing, &mut report, reporter);
    }

    if !plan.natives.is_empty() {
        let rename_jnilib = cfg!(target_os = "macos");
        for jar in &plan.natives {
            if !jar.path.is_file() {
                continue;
            }
            match palantir_loader::archive::extract_zip_file_flat(
                &jar.path,
                &plan.natives_dir,
                rename_jnilib,
                &jar.excludes,
            ) {
                Ok(count) => report.natives_extracted += count,
                Err(error) => report
                    .failed
                    .push(format!("extracting {}: {error}", jar.path.display())),
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

/// Fetch every job over the engine's queue, reporting progress while it works.
///
/// The parallelism is the engine's -- a fixed set of workers over one client,
/// under the ceiling the whole process shares -- rather than a thread count
/// chosen here. The reporting happens on this thread, once per finished file, so
/// the reports reach `reporter` in order and need no locking. The first file and
/// the last are always reported — the first says the phase is alive, the last
/// that it is over — and the rest follow [`progress_step`].
pub(crate) fn download_with_progress(
    wire: &Wire,
    jobs: &[FileJob],
    threads: usize,
    label: &str,
    reporter: &mut Reporter,
) -> Vec<Result<u64, String>> {
    let total = jobs.len();
    let step = progress_step(total);
    let mut reported = 0usize;
    wire.files(jobs, threads, &mut |done, bytes| {
        if done == 1 || done == total || done - reported >= step {
            reported = done;
            reporter.report(Progress::new(label, done, total, bytes));
        }
    })
}

/// Phase two: read the index and fetch the objects it names.
///
/// `existing` is the plan's own question, asked here too: an object that is
/// already there is checked by its size on a launch and hashed on a repair (see
/// [`Existing`]), and the plan cannot answer for the objects because it has not
/// read the index when it is built.
fn run_assets(
    assets: &AssetPlan,
    wire: &Wire,
    threads: usize,
    existing: Existing,
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
    let mut jobs: Vec<FileJob> = Vec::new();
    let mut labels: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    // An asset object is addressed *by* its digest, so the check that catches a
    // corrupted or half-written object costs nothing to state: the hash in the
    // URL is the hash the file has to have, and the transfer verifies it before
    // the file is renamed out of its part name.
    for (name, object) in &index.objects {
        if object.hash.len() < 2 {
            continue;
        }
        // Logical names can alias one physical object. Queue it once so
        // workers never write the same part file concurrently; reconstruction
        // still visits every logical name below.
        if !seen.insert(&object.hash) {
            continue;
        }
        // The path on disk, not the path on the CDN: separate helpers, and this
        // is the one place both are in play.
        let dest = assets.assets_dir.join(object_relative_path(&object.hash));
        let job = DownloadJob {
            url: asset_object_url(&object.hash),
            dest,
            sha1: object.hash.clone(),
            size: object.size,
            label: name.clone(),
        };
        if keeps(&job, existing) {
            report.present += 1;
            continue;
        }
        jobs.push(FileJob::new(&job.url, job.dest, &job.sha1));
        labels.push(name.clone());
    }
    if !jobs.is_empty() {
        reporter.log(format!("downloading {} asset object(s)…", jobs.len()));
        let started = Instant::now();
        let bytes_before = report.bytes;
        let objects_before = report.objects_downloaded;
        let results = download_with_progress(wire, &jobs, threads, "asset objects", reporter);
        for (index_in_jobs, result) in results.into_iter().enumerate() {
            // A digest mismatch is the transfer's own failure now: the object is
            // named by its hash, the download was given that hash, and a file
            // that did not hash to it never reached its destination. What is
            // left for this loop is the bookkeeping.
            match result {
                Ok(bytes) => {
                    report.objects_downloaded += 1;
                    report.bytes += bytes;
                }
                Err(error) => report
                    .failed
                    .push(format!("asset {}: {error}", labels[index_in_jobs])),
            }
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
/// Digest check for a file already on disk.
///
/// Test-only since the wire landed: the transfer verifies what it wrote, against
/// whichever kind of digest the publisher stated, so nothing in the launcher
/// reads a downloaded file back to check it any more. It stays because the tests
/// below are how the rule itself -- an empty expectation means "do not check" --
/// is written down, and a test is a reader.
///
/// It was `#[cfg(test)]` until the repair half needed it
/// ([`DownloadJob::verified`]), which is the file's own rule turned on it: a rule
/// in a `cfg(test)` module is a rule the binary does not have, and "is this file
/// the one the metadata names" is now a question this launcher asks a reader's
/// install.
pub fn verify_download(path: &Path, expected_sha1: &str) -> Result<(), String> {
    let expected = expected_sha1.trim().to_ascii_lowercase();
    if expected.is_empty() {
        return Ok(());
    }
    // Repairs hash installed jars too: reuse the engine's bounded-memory
    // verifier rather than allocating a buffer as large as each file.
    palantir_net::engine::Digest::Sha1(expected)
        .verify_file(path)
        .map_err(|error| error.to_string())
}

/// Lowercase hex SHA-1 of `bytes` (Mojang publishes SHA-1, not SHA-256).
pub fn sha1_hex(bytes: &[u8]) -> String {
    use sha1::Digest;
    let mut hasher = sha1::Sha1::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

// ---- content: one project's version into an instance ----------------------
//
// The other half of installing: a launch's files come from metadata this
// launcher already resolves, and a *project*'s file comes from Modrinth. The two
// rules were the old shell's (`browse.rs`); they live here now that a page asks
// for them, because a rule in a `cfg(test)` module is a rule the binary does not
// have.

/// A file that was written into an instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledFile {
    /// File name as written.
    pub filename: String,
    /// Full destination path.
    pub path: PathBuf,
    /// Bytes on disk after the transfer.
    pub bytes: u64,
    /// Whether the `sha1` the API published was checked.
    pub verified: bool,
}

/// Make an API-provided file name safe to write on Windows.
///
/// A published name is a stranger's string: it is the second thing after the
/// URL that an attacker on Modrinth's CDN could choose, and it is joined onto a
/// directory under the instance root. A separator or a `..` is refused rather
/// than rewritten -- a file whose name needs rewriting is a file this launcher
/// does not know how to place -- while Windows' own reserved characters are
/// replaced, because the published names really do carry them (a shader pack
/// called `Complementary: Reimagined` is not an attack).
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

/// Choose the version to install into an instance.
///
/// The reference's rule (`content-install.ts`'s `findPreferredVersion`), and it
/// is deliberately *not* a ranking by release type: `/v2/project/{id}/version`
/// answers publish-date descending -- measured on Sodium's 256 versions, all
/// `date_published` strictly ordered -- so the first match is the newest one and
/// the type is a label rather than a preference. The rule this replaces
/// (`browse::pick_version`) ranked `release`, then `beta`, then `alpha`, which
/// would install an older release over the newer beta the reader was looking at.
///
/// 1. the first version that lists the instance's game version, and -- for a mod
///    -- the instance's loader;
/// 2. else the first that also accepts a version flagged `datapack`, which is
///    the reference's fallback for a mod its author published as both;
/// 3. else nothing, which the caller turns into a sentence naming the target.
///
/// Anything that is not a mod is matched on the game version alone: a resource
/// pack or a shader has no loader to be wrong about. A version with no file is
/// skipped in both passes -- the one thing this rule adds to the reference's,
/// because a version that cannot be downloaded is not an answer to "install
/// this", and the next one down the list usually is.
pub fn preferred_version<'a>(
    versions: &'a [palantir_net::modrinth::ModrinthProjectVersion],
    project_type: &str,
    game: &str,
    loader: &str,
) -> Option<&'a palantir_net::modrinth::ModrinthProjectVersion> {
    let is_mod = project_type == "mod";
    let matches = |candidate: &palantir_net::modrinth::ModrinthProjectVersion,
                   allow_datapack: bool| {
        candidate.primary_file().is_some()
            && candidate.game_versions.iter().any(|listed| listed == game)
            && (!is_mod
                || candidate
                    .loaders
                    .iter()
                    .any(|name| name == loader || (allow_datapack && name == "datapack")))
    };
    versions
        .iter()
        .find(|candidate| matches(candidate, false))
        .or_else(|| versions.iter().find(|candidate| matches(candidate, true)))
}

/// Download one version's primary file into `target_dir`.
///
/// One file over the launcher's one queue: the transfer resumes a part file it
/// finds, is checked against the `sha1` the API published before the rename, and
/// draws the process-wide ceiling while it runs. The published *size* is still
/// checked here, because it is the only check a file with no digest has -- and
/// because the length is the caller's to know, the transfer's is not.
///
/// Measured where it landed rather than counted off the wire: a file that is
/// already there and already right transfers nothing at all.
pub fn install_file(
    wire: &Wire,
    target_dir: &Path,
    version: &palantir_net::modrinth::ModrinthProjectVersion,
) -> Result<InstalledFile, String> {
    let file = version
        .primary_file()
        .ok_or_else(|| format!("{} has no downloadable file", version.name))?;
    if file.url.is_empty() {
        return Err(format!("{} publishes no direct download", version.name));
    }
    let filename = safe_file_name(&file.filename)?;
    palantir_core::util::ensure_dir(target_dir)
        .map_err(|error| format!("creating {} failed: {error}", target_dir.display()))?;
    let path = target_dir.join(&filename);
    let jobs = [FileJob::new(&file.url, &path, file.sha1().unwrap_or_default())];
    wire.files(&jobs, 1, &mut |_, _| {})
        .remove(0)
        .map_err(|reason| format!("downloading {filename} failed: {reason}"))?;
    let bytes = std::fs::metadata(&path)
        .map(|meta| meta.len())
        .map_err(|error| format!("reading {} failed: {error}", path.display()))?;
    if file.size > 0 && bytes != file.size {
        let _ = std::fs::remove_file(&path);
        return Err(format!(
            "downloading {filename} failed: size mismatch: expected {} bytes, got {bytes}",
            file.size
        ));
    }
    Ok(InstalledFile {
        filename,
        path,
        bytes,
        verified: file.sha1().is_some(),
    })
}

// ---- content: a pack becomes an instance of its own -----------------------
//
// The other shape of install. A pack is not a file that goes *in* an instance,
// it *is* one: its index names the Minecraft version and the loaders, its
// `overrides/` tree is written over the new instance root, and the files it lists
// are fetched into their own paths. These rules were the old shell's too, beside
// the single-file ones, and they are here for the same reason: a rule in a
// `#[cfg(test)]` module is a rule the binary does not have.

/// Choose the version of a pack to install.
///
/// Releases first, then betas, then alphas, and within a kind the API's own
/// publish-date order -- the reference's list, and the rule the old shell's
/// `newest_version` held. A game version is deliberately *not* consulted, which is
/// where this differs from [`preferred_version`]: a pack carries its own Minecraft
/// version and loader in its index, so matching one against whatever instance
/// happens to be selected is how a launcher ends up refusing to install a 1.20.1
/// pack because 1.21 is the one on screen.
pub fn newest_pack_version(
    versions: &[palantir_net::modrinth::ModrinthProjectVersion],
) -> Option<&palantir_net::modrinth::ModrinthProjectVersion> {
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
/// The pack's own `path` decides where each file lands, under the instance root;
/// [`palantir_loader::PackFile::relative_path`] has already refused anything that
/// would escape it, and the refusal is reported rather than silently rewritten
/// here. Every file that states a `sha1` is checked against it by the transfer
/// itself, which never renames a part file that fails the check: a corrupted mod
/// jar in `mods/` is a crash at startup, while a missing one is a line the user
/// can act on.
///
/// A file the pack lists with several URLs gets its mirrors tried in order when
/// the first one fails -- that is what the list is for.
pub fn fetch_pack_files(
    wire: &Wire,
    root: &Path,
    files: &[palantir_loader::PackFile],
    workers: usize,
    progress: &mut dyn FnMut(Progress),
) -> PackFetch {
    let mut fetch = PackFetch::default();
    // The file each job came from and where it is going, for the digest, the
    // mirrors and the report after the download.
    let mut planned: Vec<(&palantir_loader::PackFile, PathBuf)> = Vec::new();
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
            fetch.failed.push(format!("{}: no download URL", file.path));
            continue;
        }
        let dest = root.join(relative);
        // Reapplying a pack is a repair too: size alone cannot distinguish a
        // corrupt jar or an older version published under the same filename.
        let correct_size = file.size == 0
            || std::fs::metadata(&dest).map(|meta| meta.len() == file.size).unwrap_or(false);
        if file.satisfied_at(&dest) && correct_size
            && verify_download(&dest, file.sha1.as_deref().unwrap_or_default()).is_ok()
        {
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
        let bytes = match result.and_then(|bytes| check_pack_file_size(file, dest, bytes)) {
            Ok(bytes) => bytes,
            // The first URL failed, so the mirrors get their turn before this
            // file is written off. Sequential and rare: a pack lists mirrors for
            // the file the primary host would not serve, not for balance, and a
            // mirror is one more job on the same queue.
            Err(mut last) => {
                let mut recovered = None;
                for mirror in file.downloads.iter().skip(1) {
                    let jobs = [FileJob::new(mirror, dest, file.sha1.clone().unwrap_or_default())];
                    let mut one = wire.files(&jobs, 1, &mut |_, _| {});
                    match one.remove(0).and_then(|bytes| check_pack_file_size(file, dest, bytes)) {
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

/// A successful HTTP response is not enough for a digest-less pack file:
/// check its published length before accepting it or abandoning its mirrors.
fn check_pack_file_size(
    file: &palantir_loader::PackFile,
    dest: &Path,
    transferred: u64,
) -> Result<u64, String> {
    if file.size == 0 {
        return Ok(transferred);
    }
    let actual = std::fs::metadata(dest)
        .map_err(|error| format!("reading {}: {error}", dest.display()))?
        .len();
    if actual != file.size {
        // A mirror must not mistake this bad file for a completed download.
        std::fs::remove_file(dest)
            .map_err(|error| format!("removing incomplete {}: {error}", dest.display()))?;
        return Err(format!("size mismatch: expected {} bytes, got {actual}", file.size));
    }
    Ok(transferred)
}

/// A pack installed as an instance of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledPack {
    /// The new instance's id (its folder name).
    pub id: String,
    /// What the files came to.
    pub fetch: PackFetch,
    /// Entries the pack listed that were not installed, with the reason.
    pub skipped: Vec<String>,
}

impl InstalledPack {
    /// The line the page draws after a pack is installed.
    ///
    /// It names the instance, because that is the thing that did not exist
    /// before and the thing the reader now has to find in the library -- and it
    /// says how many files came with it, so a pack that installed its index and
    /// nothing else is visible rather than silent.
    pub fn summary(&self, project: &str) -> String {
        let files = self.fetch.fetched + self.fetch.present;
        let noun = if files == 1 { "file" } else { "files" };
        let mut line = format!("Installed {project} as {}, {files} {noun}", self.id);
        if !self.fetch.failed.is_empty() {
            line.push_str(&format!(", {} could not be fetched", self.fetch.failed.len()));
        }
        if !self.skipped.is_empty() {
            line.push_str(&format!(", {} entries are not installable", self.skipped.len()));
        }
        line
    }
}

/// What laying a pack over an instance that already exists came to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackReapply {
    /// What the pack's own files came to.
    pub fetch: PackFetch,
    /// Entries the pack listed that were not installed, with the reason.
    pub skipped: Vec<String>,
}

impl PackReapply {
    /// The sentence the card draws once the pack has been laid over the instance.
    ///
    /// A repair's shape ([`crate::launch::repair_instance`]'s line), because the
    /// reader asked the same kind of question: how many files the pack brought,
    /// how many of them this run had to fetch, and the bytes that was. A failure
    /// names the first file it happened to, for a pack with one dead URL.
    pub fn summary(&self, project: &str, version: &str) -> String {
        let files = self.fetch.fetched + self.fetch.present;
        let megabytes = self.fetch.bytes as f64 / (1024.0 * 1024.0);
        let mut line = format!(
            "Re-applied {project} {version}: {files} file(s), {} fetched again ({megabytes:.1} MB)",
            self.fetch.fetched
        );
        if let Some(first) = self.fetch.failed.first() {
            line.push_str(&format!(
                "; {} could not be fetched, starting with: {first}",
                self.fetch.failed.len()
            ));
        }
        if !self.skipped.is_empty() {
            line.push_str(&format!(", {} entries are not installable", self.skipped.len()));
        }
        line
    }
}

/// Lay a pack archive over an instance that already exists, and fetch what it
/// lists.
///
/// The other shape of a pack install: [`install_pack_archive`] makes an instance
/// *out of* an archive, while this one lays an archive *over* an instance that is
/// already there -- the same index read, the same `overrides/` tree, the same
/// digest-checked transfers, with the instance root as the destination instead of
/// one that was just created. It is what the installation tab's *Re-install
/// modpack* and *Change version* both do.
///
/// What it is not is the reference's own reset: nothing here deletes, so a file
/// the previous version of the pack wrote and this one does not list stays where
/// it is -- which is why the card's sentence is this launcher's own rather than
/// the reference's `reinstall-modpack.description`. The reason is
/// [`crate::launch::repair_instance`]'s, said once more where it applies: a
/// reader's worlds, their configs and the mods they added are theirs, and a button
/// that can take them away is a button that has to be sure (G133).
///
/// **Blocking**, like every other install: the shell runs it off the frame thread.
pub fn reapply_pack(
    wire: &Wire,
    instance: &Instance,
    archive: &Path,
    progress: &mut dyn FnMut(Progress),
) -> Result<PackReapply, String> {
    let bytes = std::fs::read(archive)
        .map_err(|error| format!("reading '{}' failed: {error}", archive.display()))?;
    let plan = palantir_loader::apply_mrpack(bytes.as_slice(), instance)
        .map_err(|error| format!("that archive is not a readable pack: {error}"))?;
    let fetch = fetch_pack_files(wire, instance.root(), &plan.files, DEFAULT_THREADS, progress);
    Ok(PackReapply { fetch, skipped: plan.skipped })
}

/// The versions of a pack that fit the instance they would be laid over.
///
/// The rule the reference's own *Change version* opens with: its
/// `ContentUpdaterModal` is handed the instance's `current-game-version` and
/// `current-loader` and lists the versions of the linked project that name them,
/// so a version of another game version or loader is not offered. Laying one over
/// would change what the instance *is*, which is the installation form's own job
/// rather than this button's.
///
/// The API's order is kept, which is publish-date descending, so the newest
/// fitting version is the first row. A version with no file is dropped for
/// [`preferred_version`]'s reason: it cannot be laid over anything.
pub fn versions_for<'a>(
    versions: &'a [palantir_net::modrinth::ModrinthProjectVersion],
    game: &str,
    loader: &str,
) -> Vec<&'a palantir_net::modrinth::ModrinthProjectVersion> {
    // A plain instance has no loader to match, and its API name is `vanilla`
    // rather than a name any pack publishes -- so both spell the same question:
    // the game version alone decides.
    let plain = loader.is_empty() || loader == "vanilla";
    versions
        .iter()
        .filter(|candidate| {
            candidate.primary_file().is_some()
                && candidate.game_versions.iter().any(|listed| listed == game)
                && (plain || candidate.loaders.iter().any(|name| name == loader))
        })
        .collect()
}

/// Fetch a pack's archive into the launcher's cache and answer where it landed.
///
/// The archive *is* the pack's primary file, so this is [`install_file`] with a
/// cache directory for a target -- the same digest check, the same resume, the
/// same ceiling. Keeping it in `cache/meta/packs/` rather than in memory is what
/// makes a second install of the same pack version cost nothing, and it is also
/// what keeps a 300 MB pack out of the frame thread's stack.
pub fn fetch_pack_archive(
    wire: &Wire,
    cache_dir: &Path,
    version: &palantir_net::modrinth::ModrinthProjectVersion,
) -> Result<PathBuf, String> {
    Ok(install_file(wire, cache_dir, version)?.path)
}

/// Install a pack that is already on disk as a new instance named `name`.
///
/// The archive's own file stem is *not* the name: a project page knows what the
/// project is called, and `Cobblemon [Fabric] 1.6.1.mrpack` is not what a reader
/// wants in their library.
pub fn install_pack_archive(
    wire: &Wire,
    paths: &PalantirPaths,
    archive: &Path,
    name: &str,
    progress: &mut dyn FnMut(Progress),
) -> Result<InstalledPack, String> {
    let bytes = std::fs::read(archive)
        .map_err(|error| format!("reading '{}' failed: {error}", archive.display()))?;
    import_and_fetch(wire, paths, &bytes, name, progress)
}

/// Import pack bytes and fetch everything the index lists.
fn import_and_fetch(
    wire: &Wire,
    paths: &PalantirPaths,
    bytes: &[u8],
    name: &str,
    progress: &mut dyn FnMut(Progress),
) -> Result<InstalledPack, String> {
    let plan = palantir_loader::plan_pack(bytes)
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
    let fetch = fetch_pack_files(wire, &root, &plan.files, DEFAULT_THREADS, progress);
    Ok(InstalledPack { id, fetch, skipped: plan.skipped })
}

#[cfg(test)]
mod tests {
    use super::*;
    use palantir_core::version::{LaunchProfile, Library};
    use palantir_net::modrinth::{ModrinthProjectVersion, ModrinthVersionFile};
    use crate::wire::Script;
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Arc;

    // ---- content: the install rules, and the tests that came with them -----

    /// One version, as the API publishes it: newest first in a test's list, the
    /// way the service orders one.
    fn published(
        name: &str,
        games: &[&str],
        loaders: &[&str],
        with_file: bool,
    ) -> ModrinthProjectVersion {
        ModrinthProjectVersion {
            id: name.to_string(),
            project_id: "P".to_string(),
            name: name.to_string(),
            version_number: name.to_string(),
            version_type: "release".to_string(),
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
                    hashes: HashMap::new(),
                }]
            } else {
                Vec::new()
            },
            dependencies: Vec::new(),
        }
    }

    #[test]
    fn the_newest_matching_version_wins_whatever_its_type() {
        // The list is in the service's order, which is publish-date descending
        // (measured on Sodium's 256 versions): the first match is the newest, and
        // `version_type` is not consulted. The rule this replaced ranked
        // releases over betas and would have chosen `release-old` here.
        let versions = vec![
            published("beta-new", &["1.21.4"], &["fabric"], true),
            published("release-old", &["1.21.4"], &["fabric"], true),
            published("other-game", &["1.21.1"], &["fabric"], true),
            published("forge-only", &["1.21.4"], &["forge"], true),
            published("no-file", &["1.21.4"], &["fabric"], false),
        ];
        assert_eq!(
            preferred_version(&versions, "mod", "1.21.4", "fabric").unwrap().name,
            "beta-new"
        );
        assert_eq!(
            preferred_version(&versions, "mod", "1.21.4", "forge").unwrap().name,
            "forge-only"
        );
        assert!(preferred_version(&versions, "mod", "1.21.4", "quilt").is_none());
        assert!(preferred_version(&versions, "mod", "1.0", "fabric").is_none());
        // A version with nothing to download is skipped rather than chosen and
        // then refused, which is the one thing this rule adds to the reference's:
        // with `no-file` the only candidate there is no answer at all.
        assert!(
            preferred_version(&versions[4..], "mod", "1.21.4", "fabric").is_none(),
            "a version with no file is not a version this can install"
        );
        assert!(preferred_version(&[], "mod", "1.21.4", "fabric").is_none());
    }

    #[test]
    fn a_mod_published_as_a_data_pack_too_is_the_second_pass() {
        let versions = vec![
            published("datapack-first", &["1.21.4"], &["datapack"], true),
            published("fabric-second", &["1.21.4"], &["fabric"], true),
        ];
        assert_eq!(
            preferred_version(&versions, "mod", "1.21.4", "fabric").unwrap().name,
            "fabric-second",
            "the loader the instance runs wins over the fallback"
        );
        let only_pack = vec![published("datapack-first", &["1.21.4"], &["datapack"], true)];
        assert_eq!(
            preferred_version(&only_pack, "mod", "1.21.4", "fabric").unwrap().name,
            "datapack-first",
            "`isVersionCompatible`'s datapack allowance"
        );
        // Anything that is not a mod is matched on the game version alone: a
        // resource pack has no loader to be wrong about.
        assert_eq!(
            preferred_version(&only_pack, "resourcepack", "1.21.4", "vanilla").unwrap().name,
            "datapack-first"
        );
        assert!(preferred_version(&only_pack, "resourcepack", "1.0", "vanilla").is_none());
    }

    #[test]
    fn file_names_are_sanitized_and_paths_refused() {
        assert_eq!(
            safe_file_name("sodium-fabric-0.6.0+mc1.21.1.jar").unwrap(),
            "sodium-fabric-0.6.0+mc1.21.1.jar"
        );
        assert_eq!(safe_file_name("we:ird?.jar").unwrap(), "we_ird_.jar");
        assert!(safe_file_name("").is_err());
        assert!(safe_file_name("nested/evil.jar").is_err());
        assert!(safe_file_name("..\\evil.jar").is_err());
    }

    #[test]
    fn a_version_file_is_fetched_over_the_wire_and_measured_where_it_lands() {
        // The whole path without a network: the engine's queue fetches the body
        // and checks the `sha1` the API published before it renames the part
        // file, and the caller measures what is on disk afterwards -- which is
        // the file's length rather than what came off the wire, because a file
        // that is already here and already right transfers nothing.
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        let body = b"a mod jar";
        let mut candidate = published("sodium", &["26.2"], &["fabric"], true);
        candidate.files[0].hashes.insert("sha1".to_string(), sha1_hex(body));
        candidate.files[0].size = body.len() as u64;
        let mut script = Script::new();
        script.insert(&candidate.files[0].url, body.to_vec());
        let fetch = Arc::new(script.fetch());
        let wire = Wire::over(dir.path().join("wire"), fetch.clone());

        let installed = install_file(&wire, &mods, &candidate).unwrap();
        assert_eq!(installed.filename, "sodium.jar");
        assert_eq!(installed.bytes, body.len() as u64);
        assert!(installed.verified, "the published sha1 was checked");
        assert_eq!(std::fs::read(&installed.path).unwrap(), body);
        assert_eq!(fetch.count(), 1, "one file, one request");

        let again = install_file(&wire, &mods, &candidate).unwrap();
        assert_eq!(again.bytes, body.len() as u64);
        assert_eq!(fetch.count(), 1, "a file that is already here is not asked for twice");
    }

    #[test]
    fn a_file_whose_length_is_not_the_published_one_is_dropped() {
        // The length is the check the caller still owns, and for a file the API
        // publishes no digest for it is the only one there is.
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        let body = b"a mod jar";
        let mut candidate = published("sodium", &["26.2"], &["fabric"], true);
        candidate.files[0].size = body.len() as u64 + 1;
        let mut script = Script::new();
        script.insert(&candidate.files[0].url, body.to_vec());
        let wire = script.wire();

        let error = install_file(&wire, &mods, &candidate).unwrap_err();
        assert!(error.contains("size mismatch"), "error: {error}");
        assert!(
            !mods.join("sodium.jar").exists(),
            "a file of the wrong length is not left where the game would load it"
        );
    }

    #[test]
    fn installing_refuses_versions_without_files_and_leaves_no_trace() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        let mut candidate = published("sodium", &["26.2"], &["fabric"], true);
        candidate.files[0].hashes.insert("sha1".to_string(), sha1_hex(b"jar bytes"));
        candidate.files[0].size = 9;
        let script = Script::new();
        let wire = script.wire();

        // No files at all: refused before any request.
        let empty = ModrinthProjectVersion { files: Vec::new(), ..candidate.clone() };
        assert!(install_file(&wire, &mods, &empty).is_err());
        // No direct URL: also refused before any request.
        let no_url = ModrinthProjectVersion {
            files: vec![ModrinthVersionFile { url: String::new(), ..candidate.files[0].clone() }],
            ..candidate.clone()
        };
        let error = install_file(&wire, &mods, &no_url).unwrap_err();
        assert!(error.contains("no direct download"), "error: {error}");
        assert!(!mods.join("sodium.jar").exists());
        assert_eq!(script.fetch().count(), 0, "neither refusal made a request");
    }

    /// One version of a pack, with whatever game version and loader an index
    /// would carry -- neither of which this rule reads.
    fn pack_version(name: &str, kind: &str, with_file: bool) -> ModrinthProjectVersion {
        ModrinthProjectVersion {
            version_type: kind.to_string(),
            ..published(name, &["1.21.4"], &["fabric"], with_file)
        }
    }

    #[test]
    fn the_newest_pack_is_chosen_without_regard_to_the_selection() {
        let versions = vec![
            pack_version("beta-new", "beta", true),
            pack_version("release-old", "release", true),
            pack_version("no-file", "release", false),
        ];
        // Releases first, whatever game version or loader they carry: the pack
        // brings its own, and there is no selected instance to match.
        assert_eq!(newest_pack_version(&versions).unwrap().name, "release-old");
        assert!(newest_pack_version(&[]).is_none());
        let only_beta = vec![pack_version("b", "beta", true)];
        assert_eq!(newest_pack_version(&only_beta).unwrap().name, "b");
        // And a list of nothing but fileless versions has no answer at all.
        assert!(newest_pack_version(&[pack_version("x", "release", false)]).is_none());
    }

    #[test]
    fn pack_install_rejects_non_pack_files() {
        let (dir, paths) = test_paths();
        // An archive that is not a pack is refused before the wire is touched,
        // which is what makes an empty wire the right double for it.
        let wire = Script::new().wire();
        let bogus = dir.path().join("not-a-pack.zip");
        std::fs::write(&bogus, b"definitely not a zip archive").unwrap();
        let error =
            install_pack_archive(&wire, &paths, &bogus, "Bogus", &mut |_| {}).unwrap_err();
        assert!(error.contains("readable pack"), "error: {error}");
        assert!(install_pack_archive(
            &wire,
            &paths,
            &dir.path().join("missing.zip"),
            "Missing",
            &mut |_| {}
        )
        .is_err());
    }

    #[test]
    fn a_pack_file_is_fetched_verified_and_dropped_when_it_does_not_match() {
        // The fetch is driven through the launcher's own wire, so the whole path
        // is exercised without a network: one good file, one whose bytes are not
        // what the pack's `sha1` says, and one already present.
        let dir = tempfile::tempdir().unwrap();
        let good = b"good mod bytes";
        let files = vec![
            palantir_loader::PackFile {
                path: "mods/good.jar".to_string(),
                downloads: vec!["https://cdn.example.invalid/good.jar".to_string()],
                sha1: Some(sha1_hex(good)),
                size: good.len() as u64,
            },
            palantir_loader::PackFile {
                path: "mods/bad.jar".to_string(),
                downloads: vec!["https://cdn.example.invalid/bad.jar".to_string()],
                sha1: Some("0000000000000000000000000000000000000000".to_string()),
                size: 4,
            },
            palantir_loader::PackFile {
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

    #[test]
    fn pack_repair_replaces_a_same_size_corrupt_mod() {
        let dir = tempfile::tempdir().unwrap();
        let good = b"good jar";
        let dest = dir.path().join("mods/test.jar");
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::write(&dest, b"bad! jar").unwrap();
        let files = [palantir_loader::PackFile {
            path: "mods/test.jar".into(),
            downloads: vec!["https://cdn.example.invalid/test.jar".into()],
            sha1: Some(sha1_hex(good)),
            size: good.len() as u64,
        }];
        let mut script = Script::new();
        script.insert(&files[0].downloads[0], good.to_vec());
        let result = fetch_pack_files(&script.wire(), dir.path(), &files, 1, &mut |_| {});
        assert!(result.failed.is_empty(), "{result:?}");
        assert_eq!(result.fetched, 1);
        assert_eq!(std::fs::read(&dest).unwrap(), good);
    }

    #[test]
    fn a_pack_file_without_a_digest_still_checks_size_and_tries_mirrors() {
        let dir = tempfile::tempdir().unwrap();
        let good = b"complete jar";
        let files = [palantir_loader::PackFile {
            path: "mods/test.jar".into(),
            downloads: vec!["https://cdn.example.invalid/short.jar".into(),
                            "https://cdn.example.invalid/good.jar".into()],
            sha1: None,
            size: good.len() as u64,
        }];
        let mut script = Script::new();
        script.insert(&files[0].downloads[0], b"short".to_vec());
        script.insert(&files[0].downloads[1], good.to_vec());
        let result = fetch_pack_files(&script.wire(), dir.path(), &files, 1, &mut |_| {});
        assert!(result.failed.is_empty(), "{result:?}");
        assert_eq!(std::fs::read(dir.path().join(&files[0].path)).unwrap(), good);

        let missing = tempfile::tempdir().unwrap();
        let mut no_mirror = files[0].clone();
        no_mirror.downloads.truncate(1);
        let result = fetch_pack_files(&script.wire(), missing.path(), &[no_mirror], 1, &mut |_| {});
        assert_eq!(result.failed.len(), 1);
        assert!(result.failed[0].contains("size mismatch"), "{result:?}");
        assert!(!missing.path().join(&files[0].path).exists());
    }

    #[test]
    fn a_pack_installs_as_an_instance_of_its_own_and_says_what_it_came_to() {
        // The whole tail without a network: an index that names a Minecraft
        // version and a loader, one overrides file, and one remote mod.
        let (dir, paths) = test_paths();
        let mod_bytes = b"mod jar";
        let index = serde_json::json!({
            "formatVersion": 1,
            "game": "minecraft",
            "versionId": "1.6.1",
            "name": "Cobblemon",
            "files": [{
                "path": "mods/cobblemon.jar",
                "hashes": { "sha1": sha1_hex(mod_bytes) },
                "downloads": ["https://cdn.example.invalid/cobblemon.jar"],
                "fileSize": mod_bytes.len(),
            }],
            "dependencies": { "minecraft": "1.21.4", "fabric-loader": "0.16.9" },
        });
        let archive = pack_zip(&index, "overrides/config/cobblemon.json", b"{}");
        // The entry's name is the whole path inside the zip, `overrides/` and
        // all: the loader strips that prefix when it writes the tree, which is
        // why the assertion below is not looking under `overrides/`.
        let mut script = Script::new();
        script.insert("https://cdn.example.invalid/cobblemon.jar", mod_bytes.to_vec());
        let wire = script.wire();
        let installed =
            install_pack_archive(&wire, &paths, &write_archive(&dir, &archive), "Cobblemon", &mut |_| {})
                .expect("the pack installs");
        assert_eq!(installed.id, "Cobblemon");
        assert_eq!(installed.fetch.fetched, 1);
        assert!(installed.skipped.is_empty());
        assert_eq!(
            installed.summary("Cobblemon"),
            "Installed Cobblemon as Cobblemon, 1 file"
        );
        let root = paths.instances_dir().join("Cobblemon");
        assert!(root.join("mods").join("cobblemon.jar").is_file());
        assert_eq!(
            std::fs::read_to_string(root.join("config").join("cobblemon.json"))
                .unwrap_or_default(),
            "{}",
            "the pack's overrides land in the instance root, with `overrides/` stripped"
        );
        let pack = palantir_core::util::read_text(&root.join("mmc-pack.json")).unwrap_or_default();
        assert!(pack.contains("net.fabricmc.fabric-loader"), "pack: {pack}");
        assert!(pack.contains("1.21.4"), "pack: {pack}");
    }

    #[test]
    fn re_applying_a_pack_lays_its_files_over_an_instance_and_deletes_nothing() {
        // The G133 tail without a network: an instance that already holds the
        // reader's own files, a pack laid over it that names one remote file and
        // one override, and the promise the card's sentence makes -- the pack's
        // files are back, the reader's are untouched, and a second press fetches
        // nothing because what it lists is already right.
        let (dir, paths) = test_paths();
        let mod_bytes = b"mod jar";
        let index = serde_json::json!({
            "formatVersion": 1,
            "game": "minecraft",
            "versionId": "1.6.1",
            "name": "Cobblemon",
            "files": [{
                "path": "mods/cobblemon.jar",
                "hashes": { "sha1": sha1_hex(mod_bytes) },
                "downloads": ["https://cdn.example.invalid/cobblemon.jar"],
                "fileSize": mod_bytes.len(),
            }],
            "dependencies": { "minecraft": "1.21.4", "fabric-loader": "0.16.9" },
        });
        let archive = pack_zip(&index, "overrides/config/cobblemon.json", b"{}");
        let instance =
            Instance::create(&paths.instances_dir(), "Cobblemon", "1.21.4").expect("an instance");
        let root = instance.root().to_path_buf();
        std::fs::create_dir_all(root.join("mods")).unwrap();
        std::fs::create_dir_all(root.join("saves").join("world")).unwrap();
        std::fs::create_dir_all(root.join("config")).unwrap();
        std::fs::write(root.join("mods").join("manual.jar"), b"my mod").unwrap();
        std::fs::write(root.join("saves").join("world").join("level.dat"), b"my world")
            .unwrap();
        std::fs::write(root.join("config").join("left-behind.cfg"), b"mine").unwrap();
        // What the pack's previous version wrote under the same name.
        std::fs::write(root.join("config").join("cobblemon.json"), b"old").unwrap();

        let mut script = Script::new();
        script.insert("https://cdn.example.invalid/cobblemon.jar", mod_bytes.to_vec());
        let wire = script.wire();
        let mut reports = 0usize;
        let reapply =
            reapply_pack(&wire, &instance, &write_archive(&dir, &archive), &mut |_| reports += 1)
                .expect("the pack is laid over the instance");
        assert_eq!(reapply.fetch.fetched, 1);
        assert_eq!(reapply.fetch.failed.len(), 0);
        assert!(reapply.skipped.is_empty(), "{:?}", reapply.skipped);
        assert_eq!(
            reapply.summary("Cobblemon", "1.6.1"),
            "Re-applied Cobblemon 1.6.1: 1 file(s), 1 fetched again (0.0 MB)"
        );
        assert!(reports > 0, "the bar hears about the phase");
        assert_eq!(
            std::fs::read(root.join("mods").join("cobblemon.jar")).unwrap(),
            mod_bytes
        );
        assert_eq!(
            std::fs::read(root.join("config").join("cobblemon.json")).unwrap(),
            b"{}",
            "an override of the same name is replaced"
        );
        assert!(root.join("mods").join("manual.jar").is_file());
        assert!(root.join("saves").join("world").join("level.dat").is_file());
        assert!(root.join("config").join("left-behind.cfg").is_file());
        let pack = palantir_core::util::read_text(&root.join("mmc-pack.json")).unwrap_or_default();
        assert!(pack.contains("net.fabricmc.fabric-loader"), "pack: {pack}");

        // A second press: the file is where it belongs and the right size, so
        // nothing is transferred and the sentence says so.
        let again =
            reapply_pack(&wire, &instance, &write_archive(&dir, &archive), &mut |_| {}).unwrap();
        assert_eq!(again.fetch.fetched, 0);
        assert_eq!(again.fetch.present, 1);
        assert_eq!(
            again.summary("Cobblemon", "1.6.1"),
            "Re-applied Cobblemon 1.6.1: 1 file(s), 0 fetched again (0.0 MB)"
        );
    }

    #[test]
    fn only_the_pack_versions_that_fit_the_instance_are_offered() {
        // The reference's `ContentUpdaterModal` rule, which is what the
        // *Change version* list draws: the pack's versions that name this game
        // version and this loader, in the API's publish-date order, and never one
        // with no file to lay over.
        let versions = vec![
            published("newest", &["1.21.4"], &["fabric"], true),
            published("other-game", &["1.21.1"], &["fabric"], true),
            published("other-loader", &["1.21.4"], &["neoforge"], true),
            published("no-file", &["1.21.4"], &["fabric"], false),
            published("older", &["1.21.4"], &["fabric"], true),
        ];
        let fitting: Vec<&str> = versions_for(&versions, "1.21.4", "fabric")
            .iter()
            .map(|version| version.name.as_str())
            .collect();
        assert_eq!(fitting, vec!["newest", "older"], "the API's order is kept");

        // A plain instance has no loader to match, and `vanilla` is the API's
        // own name for it: the game version alone decides.
        let plain: Vec<&str> = versions_for(&versions, "1.21.4", "vanilla")
            .iter()
            .map(|version| version.name.as_str())
            .collect();
        assert_eq!(plain, vec!["newest", "other-loader", "older"]);
        assert_eq!(
            versions_for(&versions, "1.21.4", "").len(),
            plain.len(),
            "an empty loader is the same question as vanilla"
        );
        assert!(
            versions_for(&versions, "1.20.1", "fabric").is_empty(),
            "a version list for another game version offers nothing"
        );
    }

    /// A `.mrpack` in memory: the index, one overrides file, nothing else.
    ///
    /// `entry` is the name inside the zip, so it carries the pack's own
    /// `overrides/` prefix -- the layout the loader strips.
    fn pack_zip(index: &serde_json::Value, entry: &str, body: &[u8]) -> Vec<u8> {
        use std::io::Write as _;
        let buf = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(buf);
        let options =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        writer.start_file("modrinth.index.json", options).unwrap();
        writer.write_all(index.to_string().as_bytes()).unwrap();
        writer.start_file(entry, options).unwrap();
        writer.write_all(body).unwrap();
        writer.finish().unwrap().into_inner()
    }

    /// Write an archive into a temp dir and answer where it is.
    fn write_archive(dir: &tempfile::TempDir, bytes: &[u8]) -> PathBuf {
        let path = dir.path().join("Cobblemon [Fabric].mrpack");
        std::fs::write(&path, bytes).unwrap();
        path
    }

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
    fn run_lines(plan: &InstallPlan, wire: &Wire, threads: usize) -> (InstallReport, Vec<String>) {
        let mut lines: Vec<String> = Vec::new();
        // Each tap is bound inside the block that uses it, so the borrows end
        // with the block and the vectors can be read (and returned) afterwards.
        let report = {
            let mut collect = |line: String| lines.push(line);
            let mut reporter = Reporter::lines_only(&mut collect);
            run(plan, wire, threads, &mut reporter)
        };
        (report, lines)
    }

    /// Run a plan and keep both kinds of report, which is what a test that
    /// asserts on the bar and on the log at once needs.
    fn run_both(
        plan: &InstallPlan,
        wire: &Wire,
        threads: usize,
    ) -> (InstallReport, Vec<String>, Vec<Progress>) {
        let mut lines: Vec<String> = Vec::new();
        let mut levels: Vec<Progress> = Vec::new();
        let report = {
            let mut collect_line = |line: String| lines.push(line);
            let mut collect_level = |level: Progress| levels.push(level);
            let mut reporter = Reporter::new(&mut collect_line, &mut collect_level);
            run(plan, wire, threads, &mut reporter)
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
        let plan = plan(&paths, &paths.root.join("instances").join("Test"), &profile(), &ctx, Existing::Trust);
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

        let plan = plan(&paths, &paths.root, &profile(), &ctx, Existing::Trust);
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
        let plan = plan(&paths, &paths.root, &profile(), &ctx, Existing::Trust);
        assert_eq!(plan.present, 0);
        assert!(plan.jobs.iter().any(|job| job.dest == dest));
    }

    #[test]
    fn a_repair_hashes_what_is_on_disk_and_a_launch_does_not() {
        // The whole difference between the two questions a plan can be built
        // with. The brigadier file below is the right *size* and the wrong *file*:
        // a launch leaves it (`satisfied`), a repair plans it again (`verified`).
        // The digest in the fixture is `"abc"`, which 77 zero bytes do not hash
        // to, so the check fails for the honest reason rather than a short file --
        // and "abc" is not even hex, which a launcher that trusted the shape of
        // the field would have read as "no digest" and passed everything.
        let (_dir, paths) = test_paths();
        let ctx = RuntimeContext::current_host();
        let dest = paths
            .root
            .join("libraries")
            .join("com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar");
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::write(&dest, vec![0u8; 77]).unwrap();
        // And the Maven library, which publishes no digest at all: a repair has
        // nothing to check it against, so it keeps it. That is the second half of
        // the rule -- an empty expectation means "do not check" -- and a repair
        // that deleted it would be a repair that broke an instance.
        let maven = paths
            .root
            .join("libraries")
            .join("net/fabricmc/intermediary/1.21.1/intermediary-1.21.1.jar");
        std::fs::create_dir_all(maven.parent().unwrap()).unwrap();
        std::fs::write(&maven, b"whatever this file is").unwrap();

        let trusted = plan(&paths, &paths.root, &profile(), &ctx, Existing::Trust);
        assert_eq!(trusted.present, 2, "a launch trusts the sizes it can see");
        assert!(trusted.jobs.is_empty(), "jobs: {:?}", trusted.jobs);

        let repaired = plan(&paths, &paths.root, &profile(), &ctx, Existing::Verify);
        assert_eq!(repaired.present, 1, "the file with no digest is kept");
        assert_eq!(repaired.jobs.len(), 1, "jobs: {:?}", repaired.jobs);
        assert_eq!(repaired.jobs[0].dest, dest, "and the wrong one is planned again");
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
        let plan = plan(&paths, &instance_root, &profile, &ctx, Existing::Trust);
        assert_eq!(plan.jobs.len(), 1, "jobs: {:?}", plan.jobs);
        assert_eq!(plan.natives.len(), 1);
        assert_eq!(plan.natives[0].path, paths.root.join("libraries").join(&native_path));
        assert_eq!(plan.natives_dir, instance_root.join("natives"));
        assert_eq!(plan.jobs[0].sha1, "def");
        // The library's own `extract.exclude` travels with the jar, because
        // nothing else at extract time knows which entries are not natives.
        assert_eq!(plan.natives[0].excludes, vec!["META-INF/".to_string()]);
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
        let sixty_four = plan(&paths, &paths.root, &profile, &ctx, Existing::Trust);
        assert_eq!(
            sixty_four.natives.len(),
            1,
            "one width, not both: {:?}",
            sixty_four.natives
        );
        assert_eq!(sixty_four.jobs.len(), 1, "jobs: {:?}", sixty_four.jobs);
        assert!(
            sixty_four.natives[0].path.to_string_lossy().ends_with("-64.jar"),
            "a 64-bit JVM gets the 64-bit jar: {:?}",
            sixty_four.natives[0]
        );

        ctx.java_architecture = "32".into();
        let thirty_two = plan(&paths, &paths.root, &profile, &ctx, Existing::Trust);
        assert_eq!(thirty_two.natives.len(), 1, "natives: {:?}", thirty_two.natives);
        assert!(
            thirty_two.natives[0].path.to_string_lossy().ends_with("-32.jar"),
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
            "downloads": { "classifiers": serde_json::Value::Object(classifiers) },
            // Mojang's native libraries all carry this, and it is the reason the
            // extractor takes an exclude list at all.
            "extract": { "exclude": ["META-INF/"] }
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
        let plan = plan(&paths, &instance_root, &profile, &ctx, Existing::Trust);
        assert_eq!(plan.jobs.len(), 1);

        let mut fetcher = Script::new();
        fetcher.insert(&plan.jobs[0].url, native_bytes.clone());
        let (report, lines) = run_lines(&plan, &fetcher.wire(), 2);

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
        let plan = plan(&paths, &paths.root, &profile, &ctx, Existing::Trust);
        assert_eq!(plan.jobs.len(), 1);
        let mut fetcher = Script::new();
        fetcher.insert(&plan.jobs[0].url, b"a different file".to_vec());

        let (report, _) = run_lines(&plan, &fetcher.wire(), 1);
        assert!(!report.is_complete());
        // The words are the engine's now, and they are the better sentence: the
        // transfer that wrote the part file names it, the digest it expected and
        // the one it got.
        assert!(report.failed[0].contains("hash mismatch"), "{:?}", report.failed);
        assert!(
            report.failed[0].contains(&sha1_hex(b"the real file")),
            "and names the digest it expected: {:?}",
            report.failed
        );
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
        let plan = plan(&paths, &paths.root, &profile, &ctx, Existing::Trust);
        assert!(plan.jobs.is_empty());
        assert_eq!(plan.problems.len(), 1);
        assert!(plan.problems[0].contains("no download source"), "{:?}", plan.problems);
        let (report, _) = run_lines(&plan, &Script::new().wire(), 1);
        assert!(!report.is_complete(), "a missing library source blocks completion");
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
        let plan = plan(&paths, &paths.root, &profile, &ctx, Existing::Trust);
        assert!(plan.jobs.is_empty(), "jobs: {:?}", plan.jobs);
        assert!(plan.problems.is_empty(), "problems: {:?}", plan.problems);
    }

    #[test]
    fn the_asset_index_and_its_objects_are_installed() {
        let (_dir, paths) = test_paths();
        let ctx = RuntimeContext::current_host();
        // The bodies come first and the hashes are their real digests: an asset
        // object is *named* after its own sha1, so a fixture with invented
        // hashes is a fixture the downloader is right to reject.
        let (click, meta) = (&b"abcd"[..], &b"abc"[..]);
        let (click_hash, meta_hash) = (sha1_hex(click), sha1_hex(meta));
        let index_body = json!({
            "objects": {
                "minecraft/sounds/click.ogg": { "hash": click_hash, "size": click.len() },
                "pack.mcmeta": { "hash": meta_hash, "size": meta.len() }
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
        let plan = plan(&paths, &instance_root, &profile, &ctx, Existing::Trust);
        assert_eq!(plan.jobs.len(), 1, "just the index: {:?}", plan.jobs);
        assert!(plan.assets.is_some());
        assert!(plan.summary().contains("to download"));

        let mut fetcher = Script::new();
        fetcher.insert(&plan.jobs[0].url, index_body.clone().into_bytes());
        fetcher.insert(cdn_url(&click_hash), click.to_vec());
        fetcher.insert(cdn_url(&meta_hash), meta.to_vec());
        let (report, lines) = run_lines(&plan, &fetcher.wire(), 2);

        assert!(report.is_complete(), "failures: {:?}", report.failed);
        assert_eq!(report.downloaded, 1, "the index");
        assert_eq!(report.objects_downloaded, 2);
        assert_eq!(report.present, 0, "nothing was on disk before this run");
        assert!(paths.assets_dir().join("indexes").join("17.json").is_file());
        assert!(paths
            .assets_dir()
            .join(object_relative_path(&click_hash))
            .is_file());
        assert!(lines.iter().any(|line| line.contains("2 asset object(s)")));
    }

    #[test]
    fn an_asset_object_that_is_not_what_its_name_says_is_thrown_away() {
        // The object is addressed by its own digest, so bytes that do not hash
        // to the name they are stored under are not the object. Keeping them
        // would be permanent: the present path checks size, not content, so
        // every later launch would trust a corrupt file.
        let (_dir, paths) = test_paths();
        let ctx = RuntimeContext::current_host();
        let real = &b"abcd"[..];
        let hash = sha1_hex(real);
        let index_body = json!({
            "objects": { "pack.mcmeta": { "hash": hash, "size": real.len() } }
        })
        .to_string();
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
        let plan = plan(&paths, &paths.root, &profile, &ctx, Existing::Trust);
        let mut fetcher = Script::new();
        fetcher.insert("https://piston-meta.mojang.com/17.json", index_body.into_bytes());
        // Same length, different bytes: the case a size check cannot see.
        fetcher.insert(cdn_url(&hash), b"wxyz".to_vec());
        let (report, _) = run_lines(&plan, &fetcher.wire(), 1);

        assert!(!report.is_complete());
        assert_eq!(report.objects_downloaded, 0);
        assert!(
            report.failed.iter().any(|line| line.contains("hash mismatch")),
            "failures: {:?}",
            report.failed
        );
        assert!(!paths.assets_dir().join(object_relative_path(&hash)).exists());
    }

    #[test]
    fn a_second_install_finds_everything_present() {
        let (_dir, paths) = test_paths();
        let ctx = RuntimeContext::current_host();
        let body = &b"abcd"[..];
        let hash = sha1_hex(body);
        let index_body =
            json!({ "objects": { "a": { "hash": hash, "size": body.len() } } }).to_string();
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
        let mut fetcher = Script::new();
        fetcher.insert("https://piston-meta.mojang.com/17.json", index_body.into_bytes());
        fetcher.insert(cdn_url(&hash), body.to_vec());

        let first = plan(&paths, &paths.root, &profile, &ctx, Existing::Trust);
        run_lines(&first, &fetcher.wire(), 1);
        let second = plan(&paths, &paths.root, &profile, &ctx, Existing::Trust);
        assert!(second.is_noop(), "jobs: {:?}", second.jobs);
        assert!(second.summary().contains("already installed"));
        let (report, _) = run_lines(&second, &Script::new().wire(), 1);
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
        let plan = plan(&paths, &paths.root, &profile, &ctx, Existing::Trust);
        assert!(plan.jobs.is_empty());
        let (report, _) = run_lines(&plan, &Script::new().wire(), 1);
        assert!(report.problems.iter().any(|p| p.contains("no download URL")));
        assert!(!report.is_complete(), "a missing asset index blocks completion");
    }

    #[test]
    fn an_unreadable_asset_index_blocks_completion() {
        let (_dir, paths) = test_paths();
        let mut profile = LaunchProfile::default();
        profile.minecraft_assets = Some(palantir_core::version::AssetIndexInfo::bare("17"));
        let index_path = paths.assets_dir().join("indexes/17.json");
        std::fs::create_dir_all(index_path.parent().unwrap()).unwrap();
        std::fs::write(&index_path, b"not JSON").unwrap();
        let plan = plan(&paths, &paths.root, &profile, &RuntimeContext::current_host(), Existing::Trust);
        let (report, _) = run_lines(&plan, &Script::new().wire(), 1);
        assert!(!report.is_complete());
        assert!(report.problems.iter().any(|p| p.contains("unreadable")));
    }

    #[test]
    fn asset_aliases_download_one_object_and_reconstruct_every_name() {
        let (_dir, paths) = test_paths();
        let body = b"shared asset";
        let hash = sha1_hex(body);
        let index_path = paths.assets_dir().join("indexes/legacy.json");
        std::fs::create_dir_all(index_path.parent().unwrap()).unwrap();
        std::fs::write(&index_path, json!({
            "objects": {
                "first.txt": { "hash": hash, "size": body.len() },
                "second.txt": { "hash": hash, "size": body.len() }
            }
        }).to_string()).unwrap();
        let target = paths.root.join("resources");
        let plan = InstallPlan {
            assets: Some(AssetPlan {
                id: "legacy".into(),
                index_path,
                assets_dir: paths.assets_dir(),
                reconstruct: vec![target.clone()],
            }),
            ..InstallPlan::default()
        };
        let mut fetcher = Script::new();
        fetcher.insert(cdn_url(&hash), body.to_vec());
        // One worker makes the duplicate-job regression deterministic: the
        // second job finds the first's file but still counts it as downloaded.
        let (report, _) = run_lines(&plan, &fetcher.wire(), 1);
        assert!(report.is_complete(), "{report:?}");
        assert_eq!(report.objects_downloaded, 1);
        assert_eq!(report.bytes, body.len() as u64);
        for name in ["first.txt", "second.txt"] {
            assert_eq!(std::fs::read(target.join(name)).unwrap(), body);
        }
        let (again, _) = run_lines(&plan, &Script::new().wire(), 1);
        assert!(again.is_complete());
        assert_eq!(again.present, 1, "count physical objects, not aliases");
    }

    #[test]
    fn legacy_indexes_are_reconstructed_by_logical_name() {
        let (_dir, paths) = test_paths();
        let ctx = RuntimeContext::current_host();
        let body = &b"hi"[..];
        let hash = sha1_hex(body);
        let index_body = json!({
            "objects": { "lang/en_US.lang": { "hash": hash, "size": body.len() } }
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

        let plan = plan(&paths, &instance_root, &profile, &ctx, Existing::Trust);
        let mut fetcher = Script::new();
        fetcher.insert("https://piston-meta.mojang.com/legacy.json", index_body.into_bytes());
        fetcher.insert(cdn_url(&hash), body.to_vec());
        let (report, lines) = run_lines(&plan, &fetcher.wire(), 1);

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
    fn repair_verification_reads_files_larger_than_a_hash_chunk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("large.jar");
        let bytes: Vec<u8> = (0..(3 * 64 * 1024 + 17)).map(|i| (i % 251) as u8).collect();
        std::fs::write(&path, &bytes).unwrap();
        assert!(verify_download(&path, &sha1_hex(&bytes)).is_ok());
        assert!(verify_download(&path, &sha1_hex(&bytes[..bytes.len() - 1])).is_err());
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
        let plan = plan(&paths, &paths.root, &profile, &ctx, Existing::Trust);
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
        let mut fetcher = Script::new();
        // No digest: these are a phase's worth of files, and what this test is
        // about is the reports, not the checking.
        let mut jobs: Vec<FileJob> = Vec::new();
        const FILES: usize = 1000;
        const EACH: usize = 4096;
        for index in 0..FILES {
            let url = format!("https://cdn.invalid/object-{index:04}");
            fetcher.insert(&url, vec![b'x'; EACH]);
            jobs.push(FileJob::new(url, dir.path().join(format!("object-{index:04}")), ""));
        }

        let mut lines: Vec<String> = Vec::new();
        let mut levels: Vec<Progress> = Vec::new();
        let results = {
            let mut collect_line = |line: String| lines.push(line);
            let mut collect_level = |level: Progress| levels.push(level);
            let mut reporter = Reporter::new(&mut collect_line, &mut collect_level);
            download_with_progress(&fetcher.wire(), &jobs, 4, "files", &mut reporter)
        };

        assert_eq!(results.len(), FILES);
        assert!(results.iter().all(Result::is_ok), "every file arrived");
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
        let mut fetcher = Script::new();
        fetcher.insert(
            "https://piston-meta.mojang.com/120.json",
            index_body.into_bytes(),
        );
        for (hash, body) in bodies {
            fetcher.insert(cdn_url(&hash), body.into_bytes());
        }

        let plan = plan(&paths, &paths.root, &profile, &ctx, Existing::Trust);
        let (report, lines, levels) = run_both(&plan, &fetcher.wire(), 4);

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
