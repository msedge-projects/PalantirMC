//! The end-to-end sync against the mock server: one synthetic version whose
//! shapes mirror the real ones -- client jar, a plain library, a library
//! with natives, an asset index whose objects repeat a hash -- and every
//! byte landed, verified, and deduplicated.

mod common;

use std::path::{Path, PathBuf};
use std::time::Duration;

use common::{MockServer, Route};
use palantir_core::paths::DataRoot;
use palantir_core::rules::{Arch, Os, Platform};
use palantir_core::version::Version;
use palantir_net::client::Http;
use palantir_net::download::{DownloadOptions, sha1_bytes};
use palantir_net::scheduler::Scheduler;
use palantir_net::store::ContentStore;
use palantir_net::sync::Syncer;

fn temp_dir(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    // Unique per call: these tests run in parallel and a shared directory
    // means one test deleting another's files mid-run.
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir =
        std::env::temp_dir().join(format!("palantirmc-sync-{tag}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn body(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

/// The pieces a mini version is made of, with the server that serves them.
struct Fixture {
    dir: PathBuf,
    server: MockServer,
    version: Version,
    root: DataRoot,
    client: Vec<u8>,
    one: Vec<u8>,
    h1: String,
    h2: String,
    index_body: String,
}

fn fixture(map_to_resources: bool, interrupt_client_after: Option<u64>) -> Fixture {
    let client = body(5_000);
    let plain = body(100);
    let nat = body(120);
    let natives = body(200);
    let one = body(300);
    let two = body(400);
    let (h1, h2) = (sha1_bytes(&one), sha1_bytes(&two));

    let flag = if map_to_resources {
        r#""map_to_resources": true,"#
    } else {
        ""
    };
    let index_body = format!(
        r#"{{{flag}"objects": {{
            "a/one.ogg": {{"hash": "{h1}", "size": {}}},
            "b/two.ogg": {{"hash": "{h2}", "size": {}}},
            "c/again.ogg": {{"hash": "{h1}", "size": {}}}
        }}}}"#,
        one.len(),
        two.len(),
        one.len(),
    );

    let mut client_route = Route::new(client.clone());
    client_route.truncate_after = interrupt_client_after;

    // Objects are served where the content service keeps them: hash
    // buckets, not in-game names.
    let object_one_path = format!("/{}/{}", &h1[..2], h1);
    let object_two_path = format!("/{}/{}", &h2[..2], h2);

    // The server is built before the version document, because the document
    // names URLs on it.
    let dir = temp_dir("e2e");
    let server = MockServer::start(vec![
        ("/client.jar", client_route),
        (
            "/com/example/plain/1.0/plain-1.0.jar",
            Route::new(plain.clone()),
        ),
        ("/com/example/nat/1.0/nat-1.0.jar", Route::new(nat.clone())),
        (
            "/com/example/nat/1.0/nat-1.0-natives-linux.jar",
            Route::new(natives.clone()),
        ),
        ("/index.json", Route::new(index_body.clone().into_bytes())),
        (&object_one_path, Route::new(one.clone())),
        (&object_two_path, Route::new(two.clone())),
    ]);
    let base = server.base.clone();

    let version_json = format!(
        r#"{{
          "id": "mini", "type": "release", "mainClass": "game.Main",
          "time": "t", "releaseTime": "r",
          "downloads": {{"client": {{"sha1": "{}", "size": {}, "url": "{base}/client.jar"}}}},
          "libraries": [
            {{"name": "com.example:plain:1.0",
              "downloads": {{"artifact": {{"path": "com/example/plain/1.0/plain-1.0.jar",
                "sha1": "{}", "size": {}, "url": "{base}/com/example/plain/1.0/plain-1.0.jar"}}}}}},
            {{"name": "com.example:nat:1.0",
              "natives": {{"linux": "natives-linux"}},
              "downloads": {{"artifact": {{"path": "com/example/nat/1.0/nat-1.0.jar",
                "sha1": "{}", "size": {}, "url": "{base}/com/example/nat/1.0/nat-1.0.jar"}},
                "classifiers": {{"natives-linux": {{"path": "com/example/nat/1.0/nat-1.0-natives-linux.jar",
                "sha1": "{}", "size": {}, "url": "{base}/com/example/nat/1.0/nat-1.0-natives-linux.jar"}}}}}}}}
          ],
          "assetIndex": {{"id": "mini-assets", "sha1": "{}", "size": {}, "url": "{base}/index.json"}},
          "assets": "mini-assets"
        }}"#,
        sha1_bytes(&client),
        client.len(),
        sha1_bytes(&plain),
        plain.len(),
        sha1_bytes(&nat),
        nat.len(),
        sha1_bytes(&natives),
        natives.len(),
        sha1_bytes(index_body.as_bytes()),
        index_body.len(),
    );
    let version = Version::parse(&version_json).unwrap();

    Fixture {
        root: DataRoot::new(dir.join("root")),
        dir,
        server,
        version,
        client,
        one,
        h1,
        h2,
        index_body,
    }
}

impl Fixture {
    fn sync(&self, game_dir: Option<&Path>) -> palantir_net::sync::SyncReport {
        let http = Http::new().unwrap();
        let scheduler = Scheduler::new(4);
        let store = ContentStore::new(self.root.content_dir());
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
        let (report, _client) = syncer
            .sync_version(&self.version, &platform, game_dir)
            .unwrap();
        report
    }
}

#[test]
fn a_whole_version_lands_verified_and_deduplicated() {
    let fx = fixture(false, None);
    let report = fx.sync(None);

    assert_eq!(report.libraries, 2);
    assert_eq!(report.natives, 1);
    assert_eq!(report.asset_index, 1);
    assert_eq!(report.assets, 3);
    assert_eq!(report.reused, 0);
    assert_eq!(report.resumed, 0);
    assert_eq!(report.fetched, 8);

    // The layout paths hold the right bytes.
    assert_eq!(
        std::fs::read(fx.root.version_jar("mini")).unwrap(),
        fx.client
    );
    assert!(
        fx.root
            .library_file("com/example/plain/1.0/plain-1.0.jar")
            .unwrap()
            .is_file()
    );
    assert!(
        fx.root
            .library_file("com/example/nat/1.0/nat-1.0-natives-linux.jar")
            .unwrap()
            .is_file()
    );
    assert_eq!(
        std::fs::read_to_string(fx.root.asset_index_file("mini-assets")).unwrap(),
        fx.index_body
    );

    // Two names, one hash: one store copy, two layout paths.
    let object = fx
        .root
        .assets_dir()
        .join("objects")
        .join(&fx.h1[..2])
        .join(&fx.h1);
    assert_eq!(std::fs::read(&object).unwrap(), fx.one);
    assert!(
        fx.root
            .content_dir()
            .join(&fx.h1[..2])
            .join(&fx.h1)
            .is_file()
    );
    assert!(
        fx.root
            .content_dir()
            .join(&fx.h2[..2])
            .join(&fx.h2)
            .is_file()
    );

    let _ = std::fs::remove_dir_all(&fx.dir);
}

#[test]
fn a_second_sync_asks_the_network_nothing() {
    let fx = fixture(false, None);
    fx.sync(None);
    let hits_after_first = fx.server.hits().len();
    let report = fx.sync(None);
    assert_eq!(fx.server.hits().len(), hits_after_first);
    assert_eq!(report.reused, 8, "every file should come from the store");
    assert_eq!(report.fetched, 0);
    let _ = std::fs::remove_dir_all(&fx.dir);
}

#[test]
fn a_resources_index_lands_in_the_game_directory() {
    let fx = fixture(true, None);
    let game_dir = fx.dir.join("game");
    let report = fx.sync(Some(&game_dir));
    assert_eq!(report.assets, 3);
    assert_eq!(
        std::fs::read(game_dir.join("resources/a/one.ogg")).unwrap(),
        fx.one
    );
    // Same bytes, second name: a copy in the game dir, one store entry.
    assert!(game_dir.join("resources/c/again.ogg").is_file());
    let _ = std::fs::remove_dir_all(&fx.dir);
}

#[test]
fn an_interrupted_client_jar_resumes_and_the_report_says_so() {
    let fx = fixture(false, Some(64));
    let report = fx.sync(None);
    assert_eq!(report.resumed, 1, "the client jar must have resumed");
    assert_eq!(
        std::fs::read(fx.root.version_jar("mini")).unwrap(),
        fx.client
    );
    assert!(fx.server.range_offsets().contains(&64));
    let _ = std::fs::remove_dir_all(&fx.dir);
}
