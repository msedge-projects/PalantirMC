//! The Forge family's install, offline: `forge::install` driven against a
//! mock vendor Maven.
//!
//! The live ignored test in `live.rs` proves the vendors' own documents
//! install and their processors run; it cannot run in a sandbox and it keeps
//! its own copy of the sequence, so nothing here would catch a change to the
//! library's. This test stands the vendor's Maven up as the mock server --
//! `VendorInstall::base` is the same seam `profiles::LoaderSources::at` is for
//! Fabric -- and walks the whole function: the jar fetched, the overlay
//! resolved over its game, the toolchain placed, and the processor plan built.
//!
//! The processors are planned but never run: running them needs a Java
//! runtime, and resolving one is the caller's job for the reason `forge.rs`
//! documents (which runtime belongs to an install is a fact about the
//! *installed* document).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use palantir_core::paths::DataRoot;
use palantir_core::rules::{Arch, Os, Platform};
use palantir_core::version::Version;
use palantir_loader::forge::{self, Vendor, VendorInstall};
use palantir_net::cache::{MetadataCache, cache_dir_for};
use palantir_net::client::Http;
use palantir_net::download::{DownloadOptions, sha1_bytes};
use palantir_net::scheduler::Scheduler;
use palantir_net::store::ContentStore;
use palantir_net::sync::Syncer;
use test_support::{MockServer, Route};

/// The vendor release under test. Forge names the game and the loader in one
/// string, which is why the overlay it installs is named after both.
const VERSION: &str = "1.20.1-47.4.26";
const GAME: &str = "1.20.1";
const INSTALLED_ID: &str = "1.20.1-forge-47.4.26";
/// The profile's own toolchain library: placed before any processor runs.
const TOOL: &str = "com/example/tool/1.0/tool-1.0.jar";
/// The overlay document's library: placed with the version's own sync.
const OVERLAY_LIB: &str = "com/example/overlaylib/1.0/overlaylib-1.0.jar";

fn temp_dir(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir =
        std::env::temp_dir().join(format!("palantirmc-forge-{tag}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn body(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

/// One artifact record in the metadata's own shape. The path is the Maven
/// layout the data root joins onto its library directory, which is a different
/// string from the URL's tail and the one the install is judged on.
fn artifact(rel_path: &str, url: &str, bytes: &[u8]) -> String {
    format!(
        r#"{{"path": "{rel_path}", "sha1": "{}", "size": {}, "url": "{url}"}}"#,
        sha1_bytes(bytes),
        bytes.len()
    )
}

/// The fragment of a URL under the mock's base: the server matches on the
/// request target, which is what a URL's path and query become on the wire.
fn path_of(url: &str, base: &str) -> String {
    url.strip_prefix(base)
        .unwrap_or_else(|| panic!("{url} is not under {base}"))
        .to_string()
}

/// A vendor installer jar in miniature, built in memory: `install_profile.json`,
/// the overlay it names, and the packaged data file its marker points at.
///
/// The data entry is the point. The vendor generated its profile inside its own
/// build tree, so the marker carries an absolute path (`/data/client.lzma`)
/// that names a file packaged *inside the jar*; planning stages it out beside
/// the installer so the argument the processor receives is a path that exists.
fn installer_jar(profile: &str, overlay: Option<&str>, packaged: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut bytes));
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("install_profile.json", options).unwrap();
        zip.write_all(profile.as_bytes()).unwrap();
        if let Some(overlay) = overlay {
            zip.start_file("version.json", options).unwrap();
            zip.write_all(overlay.as_bytes()).unwrap();
        }
        zip.start_file("data/client.lzma", options).unwrap();
        zip.write_all(packaged).unwrap();
        zip.finish().unwrap();
    }
    bytes
}

/// The install profile, with one processor whose argument names the packaged
/// data marker and whose `outputs` map is a skip receipt in the real shape:
/// the produced artifact paired with its promised hash.
fn profile_json(base: &str, tool: &[u8]) -> String {
    let tool_url = format!("{base}/{TOOL}");
    format!(
        r#"{{"spec": 1, "minecraft": "{GAME}", "version": "{INSTALLED_ID}",
           "json": "/version.json",
           "libraries": [{{"name": "com.example:tool:1.0",
             "downloads": {{"artifact": {}}}}}],
           "processors": [
             {{"jar": "com.example:patcher:1.0",
               "classpath": ["com.example:tool:1.0"],
               "args": ["{{BINPATCH}}", "--client"],
               "outputs": {{"{{ROOT}}/libraries/patched-{GAME}.jar": "{promised}"}}}}
           ],
           "data": {{"BINPATCH": {{"client": "/data/client.lzma", "server": "/data/server.lzma"}}}}}}"#,
        artifact(TOOL, &tool_url, tool),
        promised = "a".repeat(40),
    )
}

/// The overlay document the profile names: an ordinary `inheritsFrom`
/// document, so what lands is resolved and says nothing about its parent.
fn overlay_json(base: &str, overlay_lib: &[u8]) -> String {
    let url = format!("{base}/{OVERLAY_LIB}");
    format!(
        r#"{{"id": "{INSTALLED_ID}", "type": "release",
           "mainClass": "cpw.mods.bootstraplauncher.BootstrapLauncher",
           "time": "t", "releaseTime": "r", "inheritsFrom": "{GAME}",
           "libraries": [{{"name": "com.example:overlaylib:1.0",
             "downloads": {{"artifact": {}}}}}]}}"#,
        artifact(OVERLAY_LIB, &url, overlay_lib),
    )
}

/// The game's own document, with the client jar and one library -- the parent
/// the overlay's `inheritsFrom` resolves against.
fn game_json(base: &str, client: &[u8]) -> String {
    let url = format!("{base}/client.jar");
    format!(
        r#"{{"id": "{GAME}", "type": "release", "mainClass": "game.Main",
           "time": "t", "releaseTime": "r",
           "downloads": {{"client": {}}},
           "libraries": []}}"#,
        client_download(&url, client),
    )
}

/// The client record's shape, which is not the library one: it carries no
/// `path`, because the data root decides where the client jar lives.
fn client_download(url: &str, bytes: &[u8]) -> String {
    format!(
        r#"{{"sha1": "{}", "size": {}, "url": "{url}"}}"#,
        sha1_bytes(bytes),
        bytes.len()
    )
}

fn manifest_json(base: &str) -> String {
    format!(
        r#"{{"latest": {{"release": "{GAME}", "snapshot": "{GAME}"}},
           "versions": [{{"id": "{GAME}", "type": "release", "time": "t",
             "releaseTime": "r", "url": "{base}/{GAME}.json"}}]}}"#
    )
}

struct Fixture {
    dir: PathBuf,
    root: DataRoot,
    /// Where the installer jar is staged, which is not the data root: the
    /// caller chooses it, and the packaged data files land beside it.
    into: PathBuf,
    server: MockServer,
}

impl Fixture {
    fn install(&self, server_jar: &[u8]) -> Result<forge::StagedInstall, String> {
        self.server.set_route(
            &path_of(
                &forge::installer_url(Vendor::Forge, VERSION),
                "https://maven.minecraftforge.net",
            ),
            Route::new(server_jar.to_vec()),
        );
        let http = Http::new().map_err(|e| e.to_string())?;
        let scheduler = Scheduler::new(4);
        let store = ContentStore::new(self.root.content_dir());
        let cache = MetadataCache::new(cache_dir_for(&self.root));
        let mut syncer = Syncer::new(
            &http,
            &scheduler,
            &store,
            &self.root,
            DownloadOptions {
                retries: 2,
                backoff: Duration::from_millis(1),
            },
        );
        syncer.asset_url_base = self.server.base.clone();
        let platform = Platform::new(Os::Linux, "6.8.0", Arch::X86_64);
        forge::install(
            &syncer,
            &cache,
            &self.server.url("/manifest.json"),
            VendorInstall {
                vendor: Vendor::Forge,
                version: VERSION,
                game_dir: None,
                // The mock Maven stands in for the vendor's own service, which
                // is what `None` means at every other call site.
                base: Some(self.server.base.as_str()),
            },
            &platform,
            &self.into,
        )
        .map_err(|error| error.to_string())
    }
}

/// Every shared byte on one server; the installer jar and the documents that
/// name URLs on it are added once its base exists.
fn fixture() -> (Fixture, Vec<u8>, Vec<u8>, Vec<u8>) {
    let client = body(5_000);
    let overlay_lib = body(120);
    let tool = body(200);
    let dir = temp_dir("install");
    let server = MockServer::start(vec![
        ("/client.jar", Route::new(client.clone())),
        (&format!("/{OVERLAY_LIB}"), Route::new(overlay_lib.clone())),
        (&format!("/{TOOL}"), Route::new(tool.clone())),
    ]);
    let root = DataRoot::new(dir.join("root"));
    (
        Fixture {
            into: dir.join("staging"),
            dir,
            root,
            server,
        },
        client,
        overlay_lib,
        tool,
    )
}

/// The documents the install will ask for, registered on the running server:
/// the manifest the caller passes the URL of, and the game document the
/// overlay's `inheritsFrom` resolves against.
fn serve_documents(fx: &Fixture, client: &[u8]) {
    let base = &fx.server.base;
    fx.server.set_route(
        &format!("/{GAME}.json"),
        Route::new(game_json(base, client).into_bytes()),
    );
    fx.server.set_route(
        "/manifest.json",
        Route::new(manifest_json(base).into_bytes()),
    );
}

#[test]
fn a_vendor_install_lands_resolved_and_plans_its_processors() {
    let (fx, client, overlay_lib, tool) = fixture();
    serve_documents(&fx, &client);

    let packaged = b"client patch bytes".to_vec();
    let profile = profile_json(&fx.server.base, &tool);
    let overlay = overlay_json(&fx.server.base, &overlay_lib);
    let jar = installer_jar(&profile, Some(&overlay), &packaged);

    let staged = fx.install(&jar).unwrap();

    // The receipt: the overlay landed *resolved*, under the id the profile
    // named, with nothing left saying who its parent was.
    let text = std::fs::read_to_string(fx.root.version_json(INSTALLED_ID)).unwrap();
    assert!(text.contains(INSTALLED_ID), "{text}");
    assert!(!text.contains("inheritsFrom"), "{text}");
    assert_eq!(Version::parse(&text).unwrap().id, INSTALLED_ID);
    assert_eq!(staged.report.version.id, INSTALLED_ID);
    // The game's own bytes are the client jar of the installed version: that
    // is the file the processors patch.
    assert_eq!(
        std::fs::read(fx.root.version_jar(INSTALLED_ID)).unwrap(),
        client
    );
    assert!(fx.root.version_jar(INSTALLED_ID).is_file());

    // Both libraries are on disk at their Maven paths: the overlay's through
    // the version's sync, the toolchain through the profile's own list.
    assert!(fx.root.library_file(OVERLAY_LIB).unwrap().is_file());
    assert!(fx.root.library_file(TOOL).unwrap().is_file());

    // The plan is not empty -- an install whose processors were skipped
    // silently would leave the overlay's patched artifacts unbuilt.
    assert!(staged.needs_processors());
    assert_eq!(staged.processors.len(), 1);
    let planned = &staged.processors[0];
    assert_eq!(planned.jar, "com.example:patcher:1.0");
    assert_eq!(planned.classpath, ["com.example:tool:1.0"]);

    // The packaged marker: the absolute `/data/client.lzma` the profile carries
    // names a file *inside* the jar, and the plan must point at the copy staged
    // beside the installer -- a path that exists -- not at the vendor's build
    // tree. Byte equality is what makes that copy a copy.
    let staged_file = staged
        .installer
        .parent()
        .unwrap()
        .join("installer-data")
        .join("data")
        .join("client.lzma");
    assert!(
        staged_file.is_file(),
        "the packaged file was not staged to {}",
        staged_file.display()
    );
    assert_eq!(std::fs::read(&staged_file).unwrap(), packaged);
    assert!(
        planned.args.iter().any(|arg| Path::new(arg) == staged_file),
        "the plan does not name the staged file: {:?}",
        planned.args
    );
    assert!(planned.args.contains(&"--client".to_string()));

    // The skip receipt expands too: produced artifact, promised hash.
    assert_eq!(planned.outputs.len(), 1);
    assert_eq!(planned.outputs[0].1, "a".repeat(40));

    // And the installer jar itself is still there, because two of the plan's
    // arguments name it and the staged files live beside it.
    let installer = forge::installer_path(&fx.into, Vendor::Forge, VERSION);
    assert!(installer.is_file(), "{}", installer.display());
    assert_eq!(std::fs::read(&installer).unwrap(), jar);

    // Both vendor paths were asked for, and neither is the real host.
    let hits = fx.server.hits();
    assert!(
        hits.iter()
            .any(|hit| hit.ends_with(&format!("forge-{VERSION}-installer.jar"))),
        "{hits:?}"
    );
    let _ = std::fs::remove_dir_all(&fx.dir);
}

#[test]
fn an_installer_without_its_overlay_refuses_and_installs_nothing() {
    let (fx, client, _overlay_lib, tool) = fixture();
    serve_documents(&fx, &client);

    // The profile names `/version.json`; the jar carries no such entry. The
    // vendor's own build tree would have had it, and a launcher that shrugged
    // would install a version document it invented.
    let profile = profile_json(&fx.server.base, &tool);
    let jar = installer_jar(&profile, None, b"client patch bytes");
    let error = fx.install(&jar).unwrap_err();
    assert!(error.contains("version.json"), "{error}");

    // Nothing was installed: the refusal happens before a document is written
    // or a need is fetched.
    assert!(!fx.root.version_json(INSTALLED_ID).exists());
    assert!(
        !fx.root.versions_dir().exists(),
        "a half-install was left behind"
    );
    let _ = std::fs::remove_dir_all(&fx.dir);
}
