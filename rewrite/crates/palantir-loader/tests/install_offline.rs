//! An install against the mock server: the document lands, the needs land,
//! and an overlay that names `inheritsFrom` installs *resolved* -- the
//! written document is the merged one and says nothing about its parent.

use std::path::PathBuf;
use std::time::Duration;

use palantir_core::paths::DataRoot;
use palantir_core::rules::{Arch, Os, Platform};
use palantir_core::version::Version;
use palantir_loader::install::install_document;
use palantir_net::cache::{MetadataCache, cache_dir_for};
use palantir_net::client::Http;
use palantir_net::download::{DownloadOptions, sha1_bytes};
use palantir_net::scheduler::Scheduler;
use palantir_net::store::ContentStore;
use palantir_net::sync::Syncer;
use test_support::{MockServer, Route};

fn temp_dir(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "palantirmc-install-{tag}-{}-{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn body(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

fn index_body(h1: &str, size: usize) -> String {
    format!(r#"{{"objects": {{"a/one.ogg": {{"hash": "{h1}", "size": {size}}}}}}}"#)
}

/// A full game document: client jar, one library, one asset object. `id`
/// gives it (and its index) their names.
fn game_document(base: &str, id: &str, client: &[u8], plain: &[u8], one: &[u8]) -> String {
    let h1 = sha1_bytes(one);
    let index = index_body(&h1, one.len());
    format!(
        r#"{{"id": "{id}", "type": "release", "mainClass": "game.Main",
          "time": "t", "releaseTime": "r",
          "downloads": {{"client": {{"sha1": "{}", "size": {}, "url": "{base}/client.jar"}}}},
          "libraries": [
            {{"name": "com.example:plain:1.0",
              "downloads": {{"artifact": {{"path": "com/example/plain/1.0/plain-1.0.jar",
                "sha1": "{}", "size": {}, "url": "{base}/com/example/plain/1.0/plain-1.0.jar"}}}}}}
          ],
          "assetIndex": {{"id": "{id}-assets", "sha1": "{}", "size": {}, "url": "{base}/index.json"}},
          "assets": "{id}-assets"}}"#,
        sha1_bytes(client),
        client.len(),
        sha1_bytes(plain),
        plain.len(),
        sha1_bytes(index.as_bytes()),
        index.len(),
    )
}

struct Fixture {
    dir: PathBuf,
    root: DataRoot,
    server: MockServer,
}

/// One server carrying every shared byte; documents that name URLs on it
/// are added with `set_route` once its base exists.
fn fixture() -> (Fixture, Vec<u8>, Vec<u8>, Vec<u8>) {
    let client = body(5_000);
    let plain = body(100);
    let one = body(300);
    let h1 = sha1_bytes(&one);
    let index = index_body(&h1, one.len());
    let dir = temp_dir("install");
    let server = MockServer::start(vec![
        ("/client.jar", Route::new(client.clone())),
        (
            "/com/example/plain/1.0/plain-1.0.jar",
            Route::new(plain.clone()),
        ),
        ("/index.json", Route::new(index.into_bytes())),
        (&format!("/{}/{}", &h1[..2], h1), Route::new(one.clone())),
    ]);
    let root = DataRoot::new(dir.join("root"));
    (Fixture { dir, root, server }, client, plain, one)
}

impl Fixture {
    fn install(
        &self,
        document: &str,
        manifest_path: &str,
    ) -> palantir_loader::install::InstallReport {
        let http = Http::new().unwrap();
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
        install_document(
            &syncer,
            &cache,
            &self.server.url(manifest_path),
            &platform,
            document,
            None,
        )
        .unwrap()
    }
}

#[test]
fn a_vanilla_install_lands_the_document_and_its_needs() {
    let (fx, client, plain, one) = fixture();
    let document = game_document(&fx.server.base, "mini", &client, &plain, &one);
    let report = fx.install(&document, "/manifest.json");

    // The receipt: the resolved document is on disk and says who it is.
    let text = std::fs::read_to_string(fx.root.version_json("mini")).unwrap();
    assert!(text.contains("\"mini\""), "{text}");
    assert!(!text.contains("inheritsFrom"), "{text}");
    assert_eq!(Version::parse(&text).unwrap().id, "mini");
    assert_eq!(report.version.id, "mini");

    // And every need landed, counted.
    assert_eq!(report.sync.libraries, 1);
    assert_eq!(report.sync.asset_index, 1);
    assert_eq!(report.sync.assets, 1);
    assert_eq!(report.sync.fetched, 4);
    assert_eq!(std::fs::read(fx.root.version_jar("mini")).unwrap(), client);
    assert!(
        fx.root
            .library_file("com/example/plain/1.0/plain-1.0.jar")
            .unwrap()
            .is_file()
    );

    // A document that inherits nothing must ask the manifest nothing.
    assert!(
        !fx.server.hits().contains(&"/manifest.json".to_string()),
        "the manifest was fetched for no reason"
    );
    let _ = std::fs::remove_dir_all(&fx.dir);
}

#[test]
fn an_inherited_document_installs_resolved() {
    let (fx, client, plain, one) = fixture();
    let loader = body(50);
    fx.server.set_route(
        "/com/example/loader/1.0/loader-1.0.jar",
        Route::new(loader.clone()),
    );

    // The parent is a whole game document on the same server; the child is
    // only what it changes -- exactly the loader-profile shape.
    let parent = game_document(&fx.server.base, "parent", &client, &plain, &one);
    let child = format!(
        r#"{{"id": "modded", "type": "release", "mainClass": "loader.Main",
          "time": "t", "releaseTime": "r", "inheritsFrom": "parent",
          "libraries": [
            {{"name": "com.example:loader:1.0",
              "downloads": {{"artifact": {{"path": "com/example/loader/1.0/loader-1.0.jar",
                "sha1": "{}", "size": {}, "url": "{}/com/example/loader/1.0/loader-1.0.jar"}}}}}}
          ]}}"#,
        sha1_bytes(&loader),
        loader.len(),
        fx.server.base,
    );
    let manifest = format!(
        r#"{{"latest": {{"release": "parent", "snapshot": "parent"}},
          "versions": [{{"id": "parent", "type": "release",
            "url": "{}/parent.json", "time": "t", "releaseTime": "r"}}]}}"#,
        fx.server.base
    );
    fx.server
        .set_route("/parent.json", Route::new(parent.into_bytes()));
    fx.server
        .set_route("/manifest.json", Route::new(manifest.into_bytes()));

    let report = fx.install(&child, "/manifest.json");

    // The written document is the resolved one: the child's identity over
    // the parent's body, with no trace of the inheritance.
    let text = std::fs::read_to_string(fx.root.version_json("modded")).unwrap();
    assert!(!text.contains("inheritsFrom"), "{text}");
    assert!(text.contains("\"modded\""), "{text}");
    let resolved = Version::parse(&text).unwrap();
    assert_eq!(resolved.main_class, "loader.Main");
    assert_eq!(resolved.libraries.len(), 2, "loader + parent's library");
    assert!(
        resolved.downloads.contains_key("client"),
        "the client jar record comes from the parent"
    );

    // And the needs of the *merged* version all landed.
    assert_eq!(report.sync.libraries, 2);
    assert_eq!(report.sync.asset_index, 1);
    assert_eq!(report.sync.assets, 1);
    assert_eq!(
        std::fs::read(fx.root.version_jar("modded")).unwrap(),
        client
    );

    // The chain was fetched exactly once each: manifest -> parent.
    let hits = fx.server.hits();
    assert!(hits.contains(&"/manifest.json".to_string()), "{hits:?}");
    assert!(hits.contains(&"/parent.json".to_string()), "{hits:?}");
    let _ = std::fs::remove_dir_all(&fx.dir);
}
