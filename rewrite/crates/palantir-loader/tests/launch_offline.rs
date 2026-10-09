//! The launch slice, offline: natives extraction and Forge processor runs.
//!
//! The processor half is tested against the real Forge install profile's
//! plan shape with a mock Java: a script named as the java binary that
//! writes the artifact the plan promises (or exits badly). The receipts the
//! install profile defines are the whole skip mechanism, so they are checked
//! in both directions -- a matching hash must skip the run entirely, a
//! promise left unkept after a "successful" run is a failure.

use std::path::{Path, PathBuf};

use palantir_loader::installer::PlannedProcessor;
use palantir_loader::launch::{extract_natives, processor_command, run_processors};

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("palantirmc-launch-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A mock java: a script that logs its arguments and runs the given body.
/// Cross-platform because CI runs windows-latest.
fn mock_java(dir: &Path, body: &str) -> PathBuf {
    #[cfg(windows)]
    let (name, script) = (
        "java-mock.bat",
        format!("@echo off\r\n{body}\r\nexit /b %errorlevel%\r\n"),
    );
    #[cfg(not(windows))]
    let (name, script) = ("java-mock.sh", format!("#!/bin/sh\n{body}\nexit $?\n"));

    let path = dir.join(name);
    std::fs::write(&path, script).unwrap();
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    path
}

/// A jar is a zip; the runner reads only the manifest out of it.
fn jar_with_manifest(path: &Path, manifest: &str) {
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("META-INF/MANIFEST.MF", options).unwrap();
    use std::io::Write;
    zip.write_all(manifest.as_bytes()).unwrap();
    zip.finish().unwrap();
}

fn sha1_of(bytes: &[u8]) -> String {
    palantir_net::download::sha1_bytes(bytes)
}

// ------------------------------------------------------------ natives

#[test]
fn natives_extract_skipping_the_excluded_paths() {
    let dir = temp_dir("natives");
    let jar = dir.join("lwjgl-platform.jar");
    {
        let file = std::fs::File::create(&jar).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("lwjgl.dll", options).unwrap();
        zip.start_file("META-INF/MANIFEST.MF", options).unwrap();
        zip.start_file("ok/keep.txt", options).unwrap();
        zip.finish().unwrap();
    }

    let dest = dir.join("natives");
    let out = extract_natives(&jar, &dest, &["META-INF".to_string()]).unwrap();
    assert!(out.join("lwjgl.dll").is_file());
    assert!(out.join("ok/keep.txt").is_file());
    assert!(
        !out.join("META-INF/MANIFEST.MF").exists(),
        "an excluded path was extracted"
    );
}

// ---------------------------------------------------------- processors

fn planned(outputs: Vec<(PathBuf, String)>) -> PlannedProcessor {
    PlannedProcessor {
        jar: "net.minecraftforge:installertools:1.4.1".to_string(),
        classpath: vec!["com.google.code.gson:gson:2.10.1".to_string()],
        args: vec!["--task".to_string(), "BINPATCH".to_string()],
        outputs,
    }
}

#[test]
fn processor_command_puts_the_jar_first_and_uses_its_main_class() {
    let dir = temp_dir("command");
    let libraries = dir.join("libraries");
    let jar = libraries.join(
        palantir_core::maven::MavenCoord::parse("net.minecraftforge:installertools:1.4.1")
            .unwrap()
            .rel_path(),
    );
    std::fs::create_dir_all(jar.parent().unwrap()).unwrap();
    jar_with_manifest(
        &jar,
        "Manifest-Version: 1.0\r\nMain-Class: cpw.mods.bootstraplauncher.BootstrapLauncher\r\n",
    );

    let command = processor_command(&PathBuf::from("java"), &libraries, &planned(vec![])).unwrap();
    let argv: Vec<String> = command
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();

    assert_eq!(argv[0], "-cp");
    assert!(argv[1].contains("installertools-1.4.1.jar"), "{argv:?}");
    assert!(argv[1].contains("gson-2.10.1.jar"), "{argv:?}");
    assert_eq!(argv[2], "cpw.mods.bootstraplauncher.BootstrapLauncher");
    assert_eq!(&argv[3..], ["--task", "BINPATCH"]);
}

#[test]
fn manifest_continuation_lines_are_joined() {
    let dir = temp_dir("manifest");
    let libraries = dir.join("libraries");
    let jar = libraries.join(
        palantir_core::maven::MavenCoord::parse("net.minecraftforge:installertools:1.4.1")
            .unwrap()
            .rel_path(),
    );
    std::fs::create_dir_all(jar.parent().unwrap()).unwrap();
    // A long Main-Class wrapped the way jars wrap it: continuation lines
    // start with a single space and carry no newline.
    jar_with_manifest(
        &jar,
        "Manifest-Version: 1.0\r\nMain-Class: cpw.mods.bootstraplauncher.Bootstrap\r\n Launcher\r\n",
    );

    let command = processor_command(&PathBuf::from("java"), &libraries, &planned(vec![])).unwrap();
    let argv: Vec<String> = command
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert_eq!(argv[2], "cpw.mods.bootstraplauncher.BootstrapLauncher");
}

#[test]
fn a_matching_receipt_skips_the_run_entirely() {
    let dir = temp_dir("skip");
    let libraries = dir.join("libraries");
    let jar = libraries.join(
        palantir_core::maven::MavenCoord::parse("net.minecraftforge:installertools:1.4.1")
            .unwrap()
            .rel_path(),
    );
    std::fs::create_dir_all(jar.parent().unwrap()).unwrap();
    jar_with_manifest(&jar, "Main-Class: mock.Main\r\n");

    // The promised artifact is already there at its promised hash.
    let artifact = dir.join("client-slim.jar");
    let content = b"already patched";
    std::fs::write(&artifact, content).unwrap();
    let java = mock_java(&dir, "exit 1"); // would fail loudly if it ran

    let plan = vec![planned(vec![(artifact, sha1_of(content))])];
    let report = run_processors(&java, &libraries, &plan).unwrap();
    assert!(report.ran.is_empty(), "ran a processor it should skip");
    assert_eq!(report.skipped.len(), 1);
}

#[test]
fn a_run_that_keeps_its_promise_is_a_success() {
    let dir = temp_dir("runs");
    let libraries = dir.join("libraries");
    let jar = libraries.join(
        palantir_core::maven::MavenCoord::parse("net.minecraftforge:installertools:1.4.1")
            .unwrap()
            .rel_path(),
    );
    std::fs::create_dir_all(jar.parent().unwrap()).unwrap();
    jar_with_manifest(&jar, "Main-Class: mock.Main\r\n");

    let artifact = dir.join("client-slim.jar");
    let content = b"patched by the mock";
    let body = if cfg!(windows) {
        // `set /p` with a nul stdin writes the text with no trailing newline,
        // matching what `printf` does on the other platforms.
        format!(
            "<nul set /p \"=patched by the mock\" > \"{}\"",
            artifact.display()
        )
    } else {
        format!("printf 'patched by the mock' > '{}'", artifact.display())
    };
    let java = mock_java(&dir, &body);

    let plan = vec![planned(vec![(artifact, sha1_of(content))])];
    let report = run_processors(&java, &libraries, &plan).unwrap();
    assert_eq!(report.ran.len(), 1);
    assert!(report.skipped.is_empty());
}

#[test]
fn a_broken_run_is_an_error_naming_the_jar() {
    let dir = temp_dir("failing");
    let libraries = dir.join("libraries");
    let jar = libraries.join(
        palantir_core::maven::MavenCoord::parse("net.minecraftforge:installertools:1.4.1")
            .unwrap()
            .rel_path(),
    );
    std::fs::create_dir_all(jar.parent().unwrap()).unwrap();
    jar_with_manifest(&jar, "Main-Class: mock.Main\r\n");

    let java = mock_java(&dir, "exit 3");
    let error = run_processors(&java, &libraries, &[planned(vec![])])
        .unwrap_err()
        .to_string();
    assert!(error.contains("installertools"), "{error}");
    assert!(error.contains('3'), "{error}");
}

#[test]
fn a_run_that_breaks_its_promise_is_an_error() {
    let dir = temp_dir("promise");
    let libraries = dir.join("libraries");
    let jar = libraries.join(
        palantir_core::maven::MavenCoord::parse("net.minecraftforge:installertools:1.4.1")
            .unwrap()
            .rel_path(),
    );
    std::fs::create_dir_all(jar.parent().unwrap()).unwrap();
    jar_with_manifest(&jar, "Main-Class: mock.Main\r\n");

    // The mock exits successfully but writes nothing: the outputs promise
    // is the only thing that catches that.
    let java = mock_java(&dir, "exit 0");
    let artifact = dir.join("client-slim.jar");
    let plan = vec![planned(vec![(artifact, sha1_of(b"never written"))])];
    let error = run_processors(&java, &libraries, &plan)
        .unwrap_err()
        .to_string();
    assert!(error.contains("installertools"), "{error}");
    assert!(error.contains("promised"), "{error}");
}
