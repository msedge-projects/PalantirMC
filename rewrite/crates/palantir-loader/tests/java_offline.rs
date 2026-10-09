//! The Java runtime documents, against the real ones.
//!
//! Both fixtures are Mojang's own (see `THIRD_PARTY_NOTICES.md`): the
//! product index for every platform and a complete `jre-legacy` manifest
//! for linux. The samples fix the shape and the values; the path guard is
//! pinned synthetically because no real manifest contains a hostile name.

use std::path::{Path, PathBuf};
use std::time::Duration;

use palantir_core::rules::{Arch, Os, Platform};
use palantir_loader::java::{
    Placement, RuntimeChoice, RuntimeManifest, fetch_runtime, mojang_platform, parse_index,
    placements, select,
};
use palantir_net::client::Http;
use palantir_net::download::{DownloadOptions, sha1_bytes};
use test_support::{MockServer, Route};

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

fn choice() -> RuntimeChoice<'static> {
    RuntimeChoice {
        platform: "linux",
        component: "jre-legacy",
    }
}

#[test]
fn the_real_index_selects_the_real_manifest() {
    let index = parse_index(&fixture("java-runtime-all.json")).unwrap();
    let entry = select(&index, &choice()).unwrap();
    // The manifest as published on 2026-10-09.
    assert_eq!(
        entry.manifest.sha1,
        "c529f68a6febc042e71835a579f44d55a3717c46"
    );
    assert_eq!(entry.manifest.size, 127111);
    assert_eq!(entry.version.name, "8u202");
    assert!(
        entry.manifest.url.starts_with("https://"),
        "{}",
        entry.manifest.url
    );

    // A family the index does not list fails naming it, not silently.
    let error = select(
        &index,
        &RuntimeChoice {
            platform: "linux",
            component: "java-runtime-zeta",
        },
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("java-runtime-zeta"), "{error}");
}

#[test]
fn platform_names_are_mojangs_own() {
    let named = |os, arch| mojang_platform(&Platform::new(os, "1.0", arch)).unwrap();
    assert_eq!(named(Os::Windows, Arch::X86_64), "windows-x64");
    assert_eq!(named(Os::Windows, Arch::X86), "windows-x86");
    assert_eq!(named(Os::Linux, Arch::X86_64), "linux");
    assert_eq!(named(Os::Linux, Arch::X86), "linux-i386");
    assert_eq!(named(Os::MacOs, Arch::X86_64), "mac-os");
    assert_eq!(named(Os::MacOs, Arch::Aarch64), "mac-os-arm64");
    // A platform Mojang ships nothing for says so instead of guessing.
    assert!(mojang_platform(&Platform::new(Os::Linux, "1.0", Arch::Aarch64)).is_err());
}

#[test]
fn the_real_manifest_places_every_entry() {
    let manifest = RuntimeManifest::parse(&fixture("java-runtime-jre-legacy-linux.json")).unwrap();
    assert_eq!(manifest.files.len(), 391);
    let placed = placements(&manifest, Path::new("/runtime")).unwrap();
    assert_eq!(placed.len(), 391);

    // The launcher itself: executable, and both packagings offered.
    let mut java = None;
    let mut link = None;
    let mut any_directory = false;
    for placement in &placed {
        match placement {
            Placement::File {
                path,
                download,
                executable,
            } if path == Path::new("/runtime/bin/java") => {
                java = Some((download, *executable));
            }
            Placement::Link { path, target } if path == Path::new("/runtime/bin/ControlPanel") => {
                link = Some(target.clone());
            }
            Placement::Directory { .. } => any_directory = true,
            _ => {}
        }
    }
    let (download, executable) = java.expect("bin/java placed");
    assert!(executable, "the java launcher must be executable");
    assert_eq!(download.sha1, "3d20560fb5d1a49cb689c2226972e92e06d27ba6");
    assert_eq!(download.size, 8464);
    assert_eq!(link.as_deref(), Some("jcontrol"));
    assert!(any_directory, "the manifest is not files alone");
}

fn temp_dir(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir =
        std::env::temp_dir().join(format!("palantirmc-java-{tag}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_runtime_lands_from_the_wire() {
    // A tiny runtime shaped like the real one's file part: a directory
    // and an executable with both packagings offered (only `raw` is
    // fetched). The `lzma` URL points at nothing: asking for it is a 404
    // and a test failure. Links are their own test below -- they are a
    // unix-only step and no Windows runtime contains one.
    let bytes: Vec<u8> = (0..100u8).collect();
    let server = MockServer::start(vec![("/java", Route::new(bytes.clone()))]);
    let manifest = RuntimeManifest::parse(&format!(
        r#"{{"files": {{
          "bin": {{"type": "directory"}},
          "bin/java": {{"type": "file", "executable": true,
            "downloads": {{
              "raw": {{"sha1": "{}", "size": {}, "url": "{}/java"}},
              "lzma": {{"sha1": "nope", "size": 1, "url": "{}/java.lzma"}}}}}}
        }}}}"#,
        sha1_bytes(&bytes),
        bytes.len(),
        server.base,
        server.base,
    ))
    .unwrap();
    let dir = temp_dir("fetch");
    let runtime = dir.join("jre");
    let http = Http::new().unwrap();
    let options = DownloadOptions {
        retries: 2,
        backoff: Duration::from_millis(1),
    };

    let report = fetch_runtime(&http, &manifest, &runtime, options).unwrap();
    assert_eq!(report.fetched, 1);
    assert_eq!(report.reused, 0);
    assert_eq!(report.bytes, bytes.len() as u64);
    assert_eq!(std::fs::read(runtime.join("bin/java")).unwrap(), bytes);
    assert!(runtime.join("bin").is_dir());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(runtime.join("bin/java"))
            .unwrap()
            .permissions()
            .mode();
        assert_ne!(mode & 0o111, 0, "bin/java must be executable");
    }

    // Only the raw packaging moved, once.
    assert_eq!(server.hits(), vec!["/java".to_string()]);

    // And an existing, correct runtime costs no requests at all.
    let again = fetch_runtime(&http, &manifest, &runtime, options).unwrap();
    assert_eq!(again.fetched, 0);
    assert_eq!(again.reused, 1);
    assert_eq!(server.hits(), vec!["/java".to_string()]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn links_are_made_on_unix_and_refused_elsewhere() {
    // Every runtime Mojang ships for unix has links; none for Windows
    // does. The step therefore makes them where the platform has them and
    // fails loudly where it does not -- never a silent skip.
    let manifest =
        RuntimeManifest::parse(r#"{"files": {"bin/runner": {"type": "link", "target": "java"}}}"#)
            .unwrap();
    let dir = temp_dir("links");
    let runtime = dir.join("jre");
    let http = Http::new().unwrap();
    let options = DownloadOptions {
        retries: 2,
        backoff: Duration::from_millis(1),
    };

    #[cfg(unix)]
    {
        fetch_runtime(&http, &manifest, &runtime, options).unwrap();
        assert_eq!(
            std::fs::read_link(runtime.join("bin/runner")).unwrap(),
            Path::new("java")
        );
    }
    #[cfg(not(unix))]
    {
        let error = fetch_runtime(&http, &manifest, &runtime, options)
            .unwrap_err()
            .to_string();
        assert!(error.contains("symbolic link"), "{error}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_manifest_path_that_escapes_is_refused() {
    // No real manifest contains one; a fetched document's names are
    // joined, never trusted.
    let manifest = RuntimeManifest::parse(
        r#"{"files": {"../evil": {
            "type": "file", "executable": false,
            "downloads": {"raw": {"sha1": "abc", "size": 1, "url": "https://x/evil"}}}}}"#,
    )
    .unwrap();
    let error = placements(&manifest, Path::new("/runtime"))
        .unwrap_err()
        .to_string();
    assert!(error.contains("../evil"), "{error}");
}
