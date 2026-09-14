//! Getting the Java a version asks for, when the machine does not have it.
//!
//! A version file says which Java it will run on — `compatibleJavaMajors`
//! (`[21]` for 1.21.1, `[8]` for 1.12.2) and `compatibleJavaName`
//! (`java-runtime-delta`, `jre-legacy`) — and on a machine with no runtime at an
//! accepted major the alternative to fetching one is a JVM refusing the game
//! with `UnsupportedClassVersionError`, which reads as "this launcher is broken"
//! rather than "install a Java". So the metadata service's own runtime is
//! fetched and used, exactly as Prism does.
//!
//! The metadata shapes live in [`palantir_net::java`], which is pure parsing and
//! is therefore what the live tests ask the real service about. This module is
//! the half that needs a data root: it decides whether a runtime is already
//! unpacked, downloads the files a manifest names, and verifies every one of
//! them. It also carries [`JavaPrefs`] — the paths the Java tab writes, which
//! nothing read on the launch path until it existed.
//!
//! Everything here takes a [`Fetcher`], so the whole chain — list, version file,
//! manifest, files — is exercised by tests with canned bodies in a temp
//! directory, and none of them touch the network.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use palantir_core::paths::PalantirPaths;
use palantir_net::java::{
    parse_manifest, parse_runtimes, pick_runtime, runtime_file_url, runtime_list_url,
    runtime_version, RuntimeFile, MANAGED_DIR, JAVA_RUNTIMES_UID,
};
use palantir_net::meta::Fetcher;

use crate::install;

/// The host's runtime tag, re-exported so a caller choosing a Java does not have
/// to know which crate the metadata shapes live in.
pub use palantir_net::java::host_runtime_os;

/// Downloads in flight while installing a runtime.
///
/// Lower than [`install::DEFAULT_THREADS`] on purpose: a JRE is a few hundred
/// small files from one host, and the point of this phase is to finish rather
/// than to saturate the connection while the user waits for a game to start.
pub const DOWNLOAD_THREADS: usize = 6;

/// Which Java this launcher's own settings say to use.
///
/// The Java tab writes two things: a binary per major version — and a major is
/// what a version file asks for — and one binary to use otherwise. Both were
/// written and never read on the launch path until this struct existed, which
/// made the tab a promise the launcher did not keep: set Java 21 there, and the
/// game ran on whatever the scan happened to find.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JavaPrefs {
    /// The "Java executable" path, used when nothing more specific applies.
    pub default_path: String,
    /// A binary per major version, keyed by the major.
    pub by_major: BTreeMap<i64, String>,
}

impl JavaPrefs {
    /// Read the Java tab out of this launcher's own preferences.
    ///
    /// A key that is not a number and a path that is blank are both dropped:
    /// the first is a file from a newer build or a hand-edit, and the second is
    /// how the tab records "nothing chosen" — neither is a Java to run with.
    pub fn from_prefs(prefs: &crate::prefs::Prefs) -> JavaPrefs {
        let by_major = prefs
            .java_paths
            .iter()
            .filter_map(|(major, path)| {
                let major = major.trim().parse::<i64>().ok()?;
                let path = path.trim();
                (!path.is_empty()).then(|| (major, path.to_string()))
            })
            .collect();
        JavaPrefs {
            default_path: prefs
                .default_java_path
                .as_deref()
                .unwrap_or_default()
                .trim()
                .to_string(),
            by_major,
        }
    }

    /// The configured binaries that answer `majors`, in the order the version
    /// lists its majors rather than in key order: the first is the one the
    /// version would prefer, and that is the one to try first.
    pub fn for_majors<'a>(&'a self, majors: &[i64]) -> Vec<(i64, &'a str)> {
        majors
            .iter()
            .filter_map(|major| self.by_major.get(major).map(|path| (*major, path.as_str())))
            .collect()
    }

    /// The configured default, when one is set.
    pub fn default_binary(&self) -> Option<&str> {
        (!self.default_path.is_empty()).then_some(self.default_path.as_str())
    }
}

/// Where a managed runtime lives: `<data root>/java/<name>`.
pub fn runtime_dir(paths: &PalantirPaths, name: &str) -> PathBuf {
    paths.root.join(MANAGED_DIR).join(name)
}

/// The executable of a managed runtime.
///
/// `java.exe` on Windows, which is the console build: this is the file that
/// says whether the runtime is complete, so it is what
/// [`install_runtime`] checks for and what the tests write. To *run* the game,
/// see [`launcher_binary`].
pub fn java_binary(paths: &PalantirPaths, name: &str) -> PathBuf {
    let exe = if cfg!(windows) { "java.exe" } else { "java" };
    runtime_dir(paths, name).join("bin").join(exe)
}

/// The executable to actually run, which is the windowless one on Windows.
///
/// A JRE ships two launchers for the same JVM: `java.exe`, a console
/// application, and `javaw.exe`, the same thing with no console. Starting
/// `java.exe` from a GUI process makes Windows allocate a console window for
/// it, and that window is what a player sees as a command prompt sitting behind
/// the game — titled with the `java.exe` path and carrying Java's own icon, so
/// the thing on the taskbar is the console rather than the game.
///
/// Windows' `CreateProcess` hides it when the child is spawned with
/// `CREATE_NO_WINDOW` ([`crate::launch`] does that too, for the cases this
/// cannot cover: a `java.exe` the user configured by hand, and the runtime
/// probes). Both are wanted, because either alone leaves a path that flashes a
/// console.
pub fn launcher_binary(paths: &PalantirPaths, name: &str) -> PathBuf {
    let console = java_binary(paths, name);
    match windowless_binary(&console) {
        Some(windowless) if windowless.is_file() => windowless,
        _ => console,
    }
}

/// `javaw.exe` beside a `java.exe`, or `None` when `console` is not one.
///
/// Keyed off the file name rather than `cfg!(windows)` so the rule is the same
/// on every platform and can be tested on any of them; on a platform whose
/// runtime is a bare `java` this matches nothing and the caller keeps what it
/// had.
fn windowless_binary(console: &Path) -> Option<PathBuf> {
    let name = console.file_name()?.to_str()?;
    if !name.eq_ignore_ascii_case("java.exe") {
        return None;
    }
    Some(console.with_file_name("javaw.exe"))
}

/// Whether a file is there and plausibly complete: existence first, then a size
/// the manifest published, which is what catches a half-written file left by an
/// interrupted run.
fn present_file(path: &Path, size: i64) -> bool {
    match std::fs::metadata(path) {
        Ok(meta) => meta.is_file() && (size <= 0 || meta.len() == size as u64),
        Err(_) => false,
    }
}

/// Mark a file executable, which every JRE needs on unix.
#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = std::fs::metadata(path) {
        let mut permissions = meta.permissions();
        permissions.set_mode(permissions.mode() | 0o111);
        let _ = std::fs::set_permissions(path, permissions);
    }
}

/// Windows has no executable bit: a file that is there is a file that runs.
#[cfg(not(unix))]
fn make_executable(_path: &Path) {}

/// Fetch and unpack a manifest runtime into `<data root>/java/<name>`.
///
/// Returns how many files were downloaded (`0` when it was already complete).
/// Every file that lands is checked against the digest the manifest published,
/// and a file that fails is removed rather than left to be trusted: a JRE with
/// one corrupt library fails in a way that looks like a Minecraft bug.
pub fn install_runtime(
    paths: &PalantirPaths,
    name: &str,
    files: &[RuntimeFile],
    fetcher: &(dyn Fetcher + Sync),
    threads: usize,
    reporter: &mut install::Reporter<'_>,
) -> Result<usize, String> {
    let dir = runtime_dir(paths, name);
    if let Err(error) = palantir_core::util::ensure_dir(&dir) {
        return Err(format!("{}: {error}", dir.display()));
    }
    let mut jobs: Vec<(String, PathBuf)> = Vec::new();
    let mut wanted: Vec<&RuntimeFile> = Vec::new();
    let mut already = 0usize;
    for file in files {
        let dest = dir.join(&file.path);
        if present_file(&dest, file.size) {
            already += 1;
            continue;
        }
        jobs.push((file.url.clone(), dest));
        wanted.push(file);
    }
    reporter.log(format!(
        "installing Java '{name}': {} file(s) to fetch, {already} already present",
        jobs.len()
    ));
    let mut failed: Vec<String> = Vec::new();
    if !jobs.is_empty() {
        // One result per job, in job order — and progress while it works, on
        // the same cadence as the asset phase: a JRE is a few hundred files, and
        // a phase that says nothing between starting and finishing is as easy
        // to mistake for a hang here as it is there. The label carries the
        // runtime's name, because a bar that says "files" while a JRE unpacks
        // is a bar that does not say what the 50 MB is for.
        let label = format!("Java '{name}'");
        let results = install::download_with_progress(fetcher, &jobs, threads, &label, reporter);
        for (index, (_url, result)) in results.into_iter().enumerate() {
            let file = match wanted.get(index) {
                Some(file) => *file,
                None => continue,
            };
            let dest = &jobs[index].1;
            match result {
                Ok(_) => match install::verify_download(dest, &file.sha1) {
                    Ok(()) => {
                        if file.executable {
                            make_executable(dest);
                        }
                    }
                    Err(reason) => {
                        let _ = std::fs::remove_file(dest);
                        failed.push(format!("{}: {reason}", file.path));
                    }
                },
                Err(error) => failed.push(format!("{}: {error}", file.path)),
            }
        }
    }
    if !failed.is_empty() {
        let shown = failed.iter().take(3).cloned().collect::<Vec<_>>().join("; ");
        return Err(format!(
            "{} of {} file(s) of Java '{name}' could not be installed ({shown})",
            failed.len(),
            jobs.len()
        ));
    }
    let binary = java_binary(paths, name);
    if !binary.is_file() {
        return Err(format!(
            "Java '{name}' was installed, but {} is not there",
            binary.display()
        ));
    }
    Ok(jobs.len())
}

/// What a launch wants when no installed Java fits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeRequest<'a> {
    /// Metadata service to ask.
    pub base_url: &'a str,
    /// Majors the version accepts, in the order it lists them.
    pub majors: &'a [i64],
    /// Runtime the version names (`compatibleJavaName`).
    pub name: &'a str,
    /// The tag this host needs ([`host_runtime_os`]).
    pub os: &'a str,
}

/// The Java binary to run with, downloading the runtime if it is not there yet.
///
/// The order is what Prism's "java switch" plus "java download" add up to, with
/// one deliberate difference: a runtime this launcher already unpacked under the
/// data root wins without a single request. A 50 MB download the machine did not
/// need is not a favour, and the caller has a fallback either way.
pub fn ensure_runtime(
    paths: &PalantirPaths,
    request: &RuntimeRequest<'_>,
    fetcher: &(dyn Fetcher + Sync),
    threads: usize,
    reporter: &mut install::Reporter<'_>,
) -> Result<String, String> {
    if request.name.trim().is_empty() {
        return Err(
            "the version does not say which Java runtime it wants (compatibleJavaName is empty)"
                .to_string(),
        );
    }
    let name = request.name.trim();
    let binary = java_binary(paths, name);
    if binary.is_file() {
        let runnable = launcher_binary(paths, name);
        reporter.log(format!("'{name}' is already unpacked at {}", runnable.display()));
        return Ok(runnable.to_string_lossy().into_owned());
    }
    if request.majors.is_empty() {
        return Err(
            "the version does not say which Java major it needs (compatibleJavaMajors is empty)"
                .to_string(),
        );
    }

    let meta_dir = paths.meta_dir();
    // `crate::catalog` reads and caches a version list the same way the create
    // dialog does, so one implementation covers both and the cached copy is at
    // the path the offline store reads.
    let (known, from_cache) =
        match crate::catalog::fetch_list(request.base_url, &meta_dir, JAVA_RUNTIMES_UID, fetcher) {
            Ok((entries, from_cache)) => (entries, from_cache),
            Err(error) => {
                return Err(format!(
                    "the Java runtime list at {} could not be read: {error}",
                    runtime_list_url(request.base_url)
                ));
            }
        };
    if from_cache {
        reporter.log("using the cached Java runtime list (the service could not be reached)");
    }

    let mut last = String::from("no runtime entry was published for this platform");
    for major in request.majors {
        let version = runtime_version(*major);
        if !known.iter().any(|entry| entry.version == version) {
            last = format!("{version}: the service publishes no runtime for Java {major}");
            continue;
        }
        let file_url = runtime_file_url(request.base_url, &version);
        let bytes = match fetcher.fetch(&file_url) {
            Ok(bytes) => {
                // Cached in the metadata layout, so the next launch — or Prism —
                // does not ask again.
                let cache = meta_dir.join(JAVA_RUNTIMES_UID).join(format!("{version}.json"));
                if let Some(parent) = cache.parent() {
                    let _ = palantir_core::util::ensure_dir(parent);
                }
                let _ = palantir_core::util::atomic_write(&cache, &bytes);
                bytes
            }
            Err(remote_error) => {
                let cache = meta_dir.join(JAVA_RUNTIMES_UID).join(format!("{version}.json"));
                match std::fs::read(&cache) {
                    Ok(bytes) => {
                        reporter.log(format!("{version}: {remote_error} — using the cached copy"));
                        bytes
                    }
                    Err(_) => {
                        last = format!("{version}: {remote_error}");
                        continue;
                    }
                }
            }
        };
        let entries = match parse_runtimes(&bytes, Path::new(&file_url)) {
            Ok(entries) => entries,
            Err(error) => {
                last = error.to_string();
                continue;
            }
        };
        let entry = match pick_runtime(&entries, request.os, name) {
            Some(entry) => entry.clone(),
            None => {
                last = format!("{version}: no runtime named '{name}' for {}", request.os);
                continue;
            }
        };
        if !entry.is_manifest() {
            last = format!(
                "{version}: the '{name}' runtime for {} is published as a '{}' download, which this build does not unpack",
                request.os, entry.kind
            );
            continue;
        }
        reporter.log(format!(
            "{version}: fetching {name} for {} from {}",
            request.os, entry.url
        ));
        let manifest = match fetcher.fetch(&entry.url) {
            Ok(bytes) => bytes,
            Err(error) => {
                last = format!("{version}: {error}");
                continue;
            }
        };
        if !entry.sha1.is_empty() {
            let published = entry.sha1.to_ascii_lowercase();
            let got = install::sha1_hex(&manifest);
            if published != got {
                last = format!(
                    "{version}: the manifest digest {got} does not match the published {published}"
                );
                continue;
            }
        }
        let files = match parse_manifest(&manifest, Path::new(&entry.url)) {
            Ok(files) => files,
            Err(error) => {
                last = error.to_string();
                continue;
            }
        };
        if files.is_empty() {
            last = format!("{version}: the manifest for '{name}' lists no files");
            continue;
        }
        match install_runtime(paths, name, &files, fetcher, threads, reporter) {
            Ok(count) => {
                let runnable = launcher_binary(paths, name);
                reporter.log(format!(
                    "Java '{name}' is ready ({count} file(s) fetched) at {}",
                    runnable.display()
                ));
                return Ok(runnable.to_string_lossy().into_owned());
            }
            Err(error) => last = format!("{version}: {error}"),
        }
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use super::*;
    use palantir_net::meta::MapFetcher;
    use serde_json::json;

    #[test]
    fn the_java_tab_reaches_the_launch_path() {
        let mut prefs = crate::prefs::Prefs::default();
        prefs.default_java_path = Some("  C:/jdk/bin/javaw.exe ".to_string());
        prefs.java_paths.insert("8".to_string(), "C:/jdk8/bin/java.exe".to_string());
        prefs.java_paths.insert("21".to_string(), "C:/jdk21/bin/java.exe".to_string());
        // Neither of these is a Java to run with.
        prefs.java_paths.insert("not-a-major".to_string(), "C:/nope/java.exe".to_string());
        prefs.java_paths.insert("17".to_string(), "   ".to_string());

        let java = JavaPrefs::from_prefs(&prefs);
        assert_eq!(java.default_binary(), Some("C:/jdk/bin/javaw.exe"));
        assert_eq!(java.by_major.len(), 2, "a bad key and a blank path are not choices");
        // The version's own order decides, not the map's. This is the case the
        // tab exists for: 1.12.2 accepts 8 and would otherwise be run on 21.
        assert_eq!(
            java.for_majors(&[8, 21]),
            vec![(8, "C:/jdk8/bin/java.exe"), (21, "C:/jdk21/bin/java.exe")]
        );
        assert_eq!(java.for_majors(&[21, 8])[0].0, 21);
        assert!(java.for_majors(&[17]).is_empty());
        assert!(JavaPrefs::default().default_binary().is_none());
    }

    fn temp_paths() -> (tempfile::TempDir, PalantirPaths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = PalantirPaths::at(dir.path());
        (dir, paths)
    }

    /// Where a JRE keeps its launcher, per the manifest of the host platform.
    ///
    /// The fixtures have to spell it the way the real manifests do, or the
    /// `java_binary` check at the end of an install would pass on one platform
    /// and fail on the other.
    fn exe_rel() -> &'static str {
        if cfg!(windows) {
            "bin/java.exe"
        } else {
            "bin/java"
        }
    }

    #[test]
    fn a_managed_runtime_lives_where_the_scanner_looks_for_one() {
        let (_dir, paths) = temp_paths();
        assert_eq!(
            java_binary(&paths, "java-runtime-delta"),
            paths.root.join("java").join("java-runtime-delta").join("bin").join(
                if cfg!(windows) { "java.exe" } else { "java" }
            )
        );
    }

    #[test]
    fn the_windowless_launcher_is_the_sibling_named_javaw() {
        assert_eq!(
            windowless_binary(Path::new("C:/jdk/bin/java.exe")),
            Some(PathBuf::from("C:/jdk/bin/javaw.exe")),
            "the pair a Windows JRE ships"
        );
        assert_eq!(
            windowless_binary(Path::new("/usr/lib/jvm/bin/java")),
            None,
            "there is no windowed build to prefer off Windows, so nothing changes"
        );
        assert_eq!(
            windowless_binary(Path::new("C:/jdk/bin/javaw.exe")),
            None,
            "only a console launcher has a windowless sibling to find"
        );
    }

    /// The point of the pair: never run the console build when the windowless
    /// one is sitting next to it, because `java.exe` puts a command prompt on
    /// screen with the game.
    #[test]
    fn a_runtime_with_javaw_beside_it_runs_without_a_console() {
        let (_dir, paths) = temp_paths();
        let console = java_binary(&paths, "java-runtime-delta");
        if let Some(parent) = console.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&console, b"console").unwrap();
        assert_eq!(
            launcher_binary(&paths, "java-runtime-delta"),
            console,
            "with no windowless build there is nothing to prefer"
        );
        let Some(windowless) = windowless_binary(&console) else {
            return;
        };
        std::fs::write(&windowless, b"windowless").unwrap();
        assert_eq!(
            launcher_binary(&paths, "java-runtime-delta"),
            windowless,
            "the JRE ships both, and the game gets the one without a console"
        );
    }

    #[test]
    fn installing_a_runtime_writes_every_file_under_the_runtime_name() {
        let (_dir, paths) = temp_paths();
        let mut fetcher = MapFetcher::new();
        fetcher.insert_str("https://objects/java", "java-binary-bytes");
        fetcher.insert_str("https://objects/lib", "library-bytes");
        let files = vec![
            RuntimeFile {
                path: exe_rel().into(),
                url: "https://objects/java".into(),
                sha1: install::sha1_hex(b"java-binary-bytes"),
                size: 17,
                executable: true,
            },
            RuntimeFile {
                path: "lib/modules".into(),
                url: "https://objects/lib".into(),
                sha1: install::sha1_hex(b"library-bytes"),
                size: 13,
                executable: false,
            },
        ];
        let mut lines: Vec<String> = Vec::new();
        let mut levels: Vec<install::Progress> = Vec::new();
        let count = install_runtime(
            &paths,
            "java-runtime-delta",
            &files,
            &fetcher,
            DOWNLOAD_THREADS,
            &mut install::Reporter::new(
                &mut |line| lines.push(line),
                &mut |level| levels.push(level),
            ),
        )
        .unwrap();
        assert_eq!(count, 2);
        assert!(java_binary(&paths, "java-runtime-delta").is_file());
        assert!(runtime_dir(&paths, "java-runtime-delta").join("lib/modules").is_file());
        assert!(lines.iter().any(|line| line.contains("java-runtime-delta")));
        // The same progress cadence the asset phase uses: a runtime install is
        // hundreds of files, so it has to show it is moving while it moves. The
        // label names the runtime, because a bar that said "files" while a JRE
        // unpacks would not say what the megabytes are for.
        let first = levels.first().expect("the runtime install reports its first file");
        assert_eq!(first.label, "Java 'java-runtime-delta'");
        assert_eq!((first.done, first.total), (1, 2));
        let last = levels.last().unwrap();
        assert_eq!((last.done, last.total), (2, 2), "and ends at the last");
        assert_eq!(last.percent(), 100);
        assert!(
            !lines.iter().any(|line| line.starts_with("Java '")),
            "a level is not a console line: {lines:?}"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(java_binary(&paths, "java-runtime-delta"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0o111, "a fetched java must be executable");
        }

        // A second run is a no-op: the files are there.
        let count = install_runtime(
            &paths,
            "java-runtime-delta",
            &files,
            &MapFetcher::new(),
            DOWNLOAD_THREADS,
            &mut install::Reporter::lines_only(&mut |_| {}),
        )
        .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn a_file_with_the_wrong_digest_is_removed_rather_than_kept() {
        let (_dir, paths) = temp_paths();
        let mut fetcher = MapFetcher::new();
        fetcher.insert_str("https://objects/java", "not what was published");
        let files = vec![RuntimeFile {
            path: exe_rel().into(),
            url: "https://objects/java".into(),
            sha1: install::sha1_hex(b"expected bytes"),
            size: 25,
            executable: true,
        }];
        let err = install_runtime(
            &paths,
            "java-runtime-delta",
            &files,
            &fetcher,
            DOWNLOAD_THREADS,
            &mut install::Reporter::lines_only(&mut |_| {}),
        )
        .unwrap_err();
        assert!(err.contains("could not be installed"), "{err}");
        assert!(!runtime_dir(&paths, "java-runtime-delta").join(exe_rel()).is_file());
    }

    #[test]
    fn an_installation_missing_its_executable_is_reported() {
        let (_dir, paths) = temp_paths();
        let mut fetcher = MapFetcher::new();
        fetcher.insert_str("https://objects/lib", "library");
        let files = vec![RuntimeFile {
            path: "lib/modules".into(),
            url: "https://objects/lib".into(),
            sha1: install::sha1_hex(b"library"),
            size: 7,
            executable: false,
        }];
        let err = install_runtime(
            &paths,
            "java-runtime-delta",
            &files,
            &fetcher,
            DOWNLOAD_THREADS,
            &mut install::Reporter::lines_only(&mut |_| {}),
        )
        .unwrap_err();
        assert!(err.contains("is not there"), "{err}");
    }

    /// A whole `net.minecraft.java` chain, canned: list, version file, manifest.
    fn runtime_fetcher() -> MapFetcher {
        let mut fetcher = MapFetcher::new();
        fetcher.insert_str(
            "https://meta.example/v1/net.minecraft.java/index.json",
            r#"{"formatVersion":1,"uid":"net.minecraft.java","versions":[
                {"version":"java8","recommended":false},
                {"version":"java21","recommended":false}]}"#,
        );
        // Built as a map rather than a `json!` literal because the executable's
        // path is the host's (`bin/java.exe` or `bin/java`).
        let mut manifest_files = serde_json::Map::new();
        manifest_files.insert("bin".to_string(), json!({"type": "directory"}));
        manifest_files.insert(
            exe_rel().to_string(),
            json!({
                "type": "file", "executable": true,
                "downloads": {"raw": {
                    "sha1": crate::install::sha1_hex(b"jre-java-exe"),
                    "size": 12,
                    "url": "https://objects/java.exe"}}
            }),
        );
        let manifest = serde_json::to_vec(&json!({"files": manifest_files})).unwrap();
        let manifest_text = String::from_utf8(manifest).unwrap();
        fetcher.insert_str("https://piston/manifest.json", &manifest_text);
        let version_file = format!(
            "{{\"formatVersion\":1,\"uid\":\"net.minecraft.java\",\"version\":\"java21\",
              \"runtimes\":[{{\"runtimeOS\":\"windows-x64\",\"name\":\"java-runtime-delta\",
                \"downloadType\":\"manifest\",\"url\":\"https://piston/manifest.json\",
                \"checksum\":{{\"type\":\"sha1\",\"hash\":\"{}\"}},
                \"version\":{{\"major\":21}}}}]}}",
            crate::install::sha1_hex(manifest_text.as_bytes())
        );
        fetcher.insert_str("https://meta.example/v1/net.minecraft.java/java21.json", &version_file);
        fetcher.insert_str("https://objects/java.exe", "jre-java-exe");
        fetcher
    }

    fn request<'a>(majors: &'a [i64], name: &'a str, os: &'a str) -> RuntimeRequest<'a> {
        RuntimeRequest { base_url: "https://meta.example/v1", majors, name, os }
    }

    #[test]
    fn the_whole_chain_from_the_list_to_the_binary_is_followed() {
        let (_dir, paths) = temp_paths();
        let fetcher = runtime_fetcher();
        let mut lines: Vec<String> = Vec::new();
        let binary = ensure_runtime(
            &paths,
            &request(&[21], "java-runtime-delta", "windows-x64"),
            &fetcher,
            DOWNLOAD_THREADS,
            &mut install::Reporter::lines_only(&mut |line| lines.push(line)),
        )
        .unwrap();
        let expected = java_binary(&paths, "java-runtime-delta");
        assert_eq!(binary, expected.to_string_lossy());
        assert!(expected.is_file());
        assert!(lines.iter().any(|line| line.contains("fetching java-runtime-delta")));
        // The version file was cached where the offline store reads it.
        assert!(paths.meta_dir().join(JAVA_RUNTIMES_UID).join("java21.json").is_file());

        // Second call: no fetch at all, because the binary is there.
        let mut lines: Vec<String> = Vec::new();
        let again = ensure_runtime(
            &paths,
            &request(&[21], "java-runtime-delta", "windows-x64"),
            &MapFetcher::new(),
            DOWNLOAD_THREADS,
            &mut install::Reporter::lines_only(&mut |line| lines.push(line)),
        )
        .unwrap();
        assert_eq!(again, binary);
        assert!(lines.iter().any(|line| line.contains("already unpacked")));
    }

    #[test]
    fn a_manifest_that_does_not_match_its_published_digest_is_refused() {
        let (_dir, paths) = temp_paths();
        let mut fetcher = runtime_fetcher();
        // Same manifest, different published digest.
        fetcher.insert_str(
            "https://meta.example/v1/net.minecraft.java/java21.json",
            &serde_json::to_string(&json!({
                "uid": "net.minecraft.java", "version": "java21",
                "runtimes": [{"runtimeOS": "windows-x64", "name": "java-runtime-delta",
                    "downloadType": "manifest", "url": "https://piston/manifest.json",
                    "checksum": {"type": "sha1", "hash": "0".repeat(40)}}]
            }))
            .unwrap(),
        );
        let err = ensure_runtime(
            &paths,
            &request(&[21], "java-runtime-delta", "windows-x64"),
            &fetcher,
            DOWNLOAD_THREADS,
            &mut install::Reporter::lines_only(&mut |_| {}),
        )
        .unwrap_err();
        assert!(err.contains("does not match the published"), "{err}");
        assert!(!java_binary(&paths, "java-runtime-delta").is_file());
    }

    #[test]
    fn an_archive_only_platform_is_reported_instead_of_half_unpacked() {
        let (_dir, paths) = temp_paths();
        let mut fetcher = MapFetcher::new();
        fetcher.insert_str(
            "https://meta.example/v1/net.minecraft.java/index.json",
            r#"{"formatVersion":1,"uid":"net.minecraft.java","versions":[{"version":"java21"}]}"#,
        );
        fetcher.insert_str(
            "https://meta.example/v1/net.minecraft.java/java21.json",
            r#"{"uid":"net.minecraft.java","version":"java21","runtimes":[
                {"runtimeOS":"linux-riscv64","name":"java-runtime-delta",
                 "downloadType":"archive","url":"https://adoptium/jdk.tar.gz",
                 "vendor":"eclipse"}]}"#,
        );
        let err = ensure_runtime(
            &paths,
            &request(&[21], "java-runtime-delta", "linux-riscv64"),
            &fetcher,
            DOWNLOAD_THREADS,
            &mut install::Reporter::lines_only(&mut |_| {}),
        )
        .unwrap_err();
        assert!(err.contains("archive"), "{err}");
    }

    #[test]
    fn a_version_that_names_no_runtime_says_so() {
        let (_dir, paths) = temp_paths();
        let err = ensure_runtime(
            &paths,
            &request(&[21], "", "windows-x64"),
            &MapFetcher::new(),
            DOWNLOAD_THREADS,
            &mut install::Reporter::lines_only(&mut |_| {}),
        )
        .unwrap_err();
        assert!(err.contains("compatibleJavaName"), "{err}");
    }

    #[test]
    fn a_version_that_names_no_major_says_so() {
        let (_dir, paths) = temp_paths();
        let err = ensure_runtime(
            &paths,
            &request(&[], "java-runtime-delta", "windows-x64"),
            &MapFetcher::new(),
            DOWNLOAD_THREADS,
            &mut install::Reporter::lines_only(&mut |_| {}),
        )
        .unwrap_err();
        assert!(err.contains("compatibleJavaMajors"), "{err}");
    }

    #[test]
    fn a_platform_the_service_does_not_build_for_is_named_in_the_reason() {
        let (_dir, paths) = temp_paths();
        let err = ensure_runtime(
            &paths,
            &request(&[21], "java-runtime-delta", "solaris-sparc"),
            &runtime_fetcher(),
            DOWNLOAD_THREADS,
            &mut install::Reporter::lines_only(&mut |_| {}),
        )
        .unwrap_err();
        assert!(
            err.contains("no runtime named 'java-runtime-delta' for solaris-sparc"),
            "{err}"
        );
        assert!(!java_binary(&paths, "java-runtime-delta").is_file());
    }

    #[test]
    fn a_later_major_is_tried_when_the_service_publishes_nothing_for_the_first() {
        let (_dir, paths) = temp_paths();
        // The version accepts 23 first, which the service's list has no entry
        // for at all; 21 is the one that answers.
        let binary = ensure_runtime(
            &paths,
            &request(&[23, 21], "java-runtime-delta", "windows-x64"),
            &runtime_fetcher(),
            DOWNLOAD_THREADS,
            &mut install::Reporter::lines_only(&mut |_| {}),
        )
        .unwrap();
        assert_eq!(binary, java_binary(&paths, "java-runtime-delta").to_string_lossy());
    }

    #[test]
    fn an_unreachable_service_reads_the_cached_version_file() {
        let (_dir, paths) = temp_paths();
        // First run caches `java21.json`; the list is a known URL that fails.
        let fetcher = runtime_fetcher();
        ensure_runtime(
            &paths,
            &request(&[21], "java-runtime-delta", "windows-x64"),
            &fetcher,
            DOWNLOAD_THREADS,
            &mut install::Reporter::lines_only(&mut |_| {}),
        )
        .unwrap();
        // Remove what was installed, keep the metadata, and serve only the
        // cached list plus nothing else: the version file has to come from disk.
        std::fs::remove_dir_all(runtime_dir(&paths, "java-runtime-delta")).unwrap();
        let mut partial = MapFetcher::new();
        partial.insert_str(
            "https://meta.example/v1/net.minecraft.java/index.json",
            r#"{"formatVersion":1,"uid":"net.minecraft.java","versions":[{"version":"java21"}]}"#,
        );
        let mut lines: Vec<String> = Vec::new();
        let err = ensure_runtime(
            &paths,
            &request(&[21], "java-runtime-delta", "windows-x64"),
            &partial,
            DOWNLOAD_THREADS,
            &mut install::Reporter::lines_only(&mut |line| lines.push(line)),
        )
        .unwrap_err();
        assert!(lines.iter().any(|line| line.contains("using the cached copy")), "{lines:?}");
        // The manifest itself is not cached, so the download is what fails — and
        // the reason names it rather than pretending the runtime was installed.
        assert!(err.contains("https://piston/manifest.json"), "{err}");
        assert!(
            !java_binary(&paths, "java-runtime-delta").is_file(),
            "a failed download must not leave a runtime behind"
        );
    }
}
