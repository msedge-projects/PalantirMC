//! Loader installs against the mock server: the listing picks the right
//! release, and the profile lands *resolved* over the game document.

use std::path::PathBuf;
use std::time::Duration;

use palantir_core::paths::DataRoot;
use palantir_core::rules::{Arch, Os, Platform};
use palantir_core::version::Version;
use palantir_loader::profiles::{
    LoaderInstall, LoaderKind, LoaderRelease, LoaderSources, install_loader, list_loaders,
    pick_loader,
};
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
        "palantirmc-profiles-{tag}-{}-{n}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn body(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

fn game_document(base: &str, id: &str, client: &[u8]) -> String {
    format!(
        r#"{{"id": "{id}", "type": "release", "mainClass": "game.Main",
          "time": "t", "releaseTime": "r",
          "downloads": {{"client": {{"sha1": "{}", "size": {}, "url": "{base}/client.jar"}}}},
          "libraries": [
            {{"name": "com.example:plain:1.0",
              "downloads": {{"artifact": {{"path": "com/example/plain/1.0/plain-1.0.jar",
                "sha1": "{}", "size": 1, "url": "{base}/plain-1.0.jar"}}}}}}
          ]}}"#,
        sha1_bytes(client),
        client.len(),
        sha1_bytes(b"x"),
    )
}

/// A Fabric listing: newest first, most releases unmarked, one stable.
const LISTING: &str = r#"[
  {"loader": {"version": "0.20.0-beta.9", "stable": false, "maven": "net.fabricmc:fabric-loader:0.20.0-beta.9"}},
  {"loader": {"version": "0.19.5", "stable": true, "maven": "net.fabricmc:fabric-loader:0.19.5"}},
  {"loader": {"version": "0.19.4", "stable": false, "maven": "net.fabricmc:fabric-loader:0.19.4"}}
]"#;

/// A Quilt listing: no stability marks anywhere.
const QUILT_LISTING: &str = r#"[
  {"loader": {"version": "0.20.0-beta.9", "maven": "org.quiltmc:quilt-loader:0.20.0-beta.9"}},
  {"loader": {"version": "0.17.0-beta.1", "maven": "org.quiltmc:quilt-loader:0.17.0-beta.1"}}
]"#;

fn release(version: &str, stable: Option<bool>) -> LoaderRelease {
    LoaderRelease {
        version: version.to_string(),
        stable,
        extra: Default::default(),
    }
}

#[test]
fn the_pick_is_the_newest_stable_not_the_newest() {
    let listings = [
        release("0.20.0-beta.9", Some(false)),
        release("0.19.5", Some(true)),
        release("0.19.4", Some(false)),
    ];
    assert_eq!(pick_loader(&listings).unwrap().version, "0.19.5");

    // A listing that marks nothing (Quilt's shape) yields its newest.
    let unmarked = [
        release("0.20.0-beta.9", None),
        release("0.17.0-beta.1", None),
    ];
    assert_eq!(pick_loader(&unmarked).unwrap().version, "0.20.0-beta.9");
    assert!(pick_loader(&[]).is_none());
}

#[test]
fn both_listing_shapes_parse_through_one_parser() {
    // Fabric's `loader` objects carry `stable`, Quilt's do not -- one
    // shape must read both, and each must pick as its marks say.
    let dir = temp_dir("listings");
    let root = DataRoot::new(dir.join("root"));
    let server = MockServer::start(vec![]);
    server.set_route(
        "/v2/versions/loader/1.20.1",
        Route::new(LISTING.as_bytes().to_vec()),
    );
    server.set_route(
        "/v3/versions/loader/1.20.1",
        Route::new(QUILT_LISTING.as_bytes().to_vec()),
    );
    let http = Http::new().unwrap();
    let cache = MetadataCache::new(cache_dir_for(&root));

    let fabric = list_loaders(
        &http,
        &cache,
        &LoaderSources::at(LoaderKind::Fabric, server.base.clone()),
        "1.20.1",
    )
    .unwrap();
    assert_eq!(fabric.len(), 3);
    assert_eq!(pick_loader(&fabric).unwrap().version, "0.19.5");

    let quilt = list_loaders(
        &http,
        &cache,
        &LoaderSources::at(LoaderKind::Quilt, server.base.clone()),
        "1.20.1",
    )
    .unwrap();
    assert_eq!(quilt.len(), 2);
    assert!(quilt.iter().all(|release| release.stable.is_none()));
    assert_eq!(pick_loader(&quilt).unwrap().version, "0.20.0-beta.9");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_loader_installs_resolved_over_the_game() {
    let client = body(5_000);
    let dir = temp_dir("e2e");
    let root = DataRoot::new(dir.join("root"));
    let server = MockServer::start(vec![
        ("/client.jar", Route::new(client.clone())),
        ("/plain-1.0.jar", Route::new(b"x".to_vec())),
    ]);
    let base = server.base.clone();
    let game = game_document(&base, "1.20.1", &client);
    let profile = format!(
        r#"{{"id": "fabric-loader-0.19.5-1.20.1", "type": "release",
          "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
          "time": "t", "releaseTime": "r", "inheritsFrom": "1.20.1",
          "libraries": [
            {{"name": "net.fabricmc:fabric-loader:0.19.5",
              "downloads": {{"artifact": {{"path": "net/fabricmc/fabric-loader/0.19.5/fabric-loader-0.19.5.jar",
                "sha1": "{}", "size": 1, "url": "{base}/loader-0.19.5.jar"}}}}}}
          ]}}"#,
        sha1_bytes(b"l"),
    );
    let manifest = format!(
        r#"{{"latest": {{"release": "1.20.1", "snapshot": "1.20.1"}},
          "versions": [{{"id": "1.20.1", "type": "release",
            "url": "{base}/game.json", "time": "t", "releaseTime": "r"}}]}}"#,
    );
    server.set_route("/game.json", Route::new(game.into_bytes()));
    server.set_route(
        "/v2/versions/loader/1.20.1",
        Route::new(LISTING.to_string().into_bytes()),
    );
    server.set_route(
        "/v2/versions/loader/1.20.1/0.19.5/profile/json",
        Route::new(profile.into_bytes()),
    );
    server.set_route("/loader-0.19.5.jar", Route::new(b"l".to_vec()));
    server.set_route("/manifest.json", Route::new(manifest.into_bytes()));

    let http = Http::new().unwrap();
    let scheduler = Scheduler::new(4);
    let store = ContentStore::new(root.content_dir());
    let cache = MetadataCache::new(cache_dir_for(&root));
    let mut syncer = Syncer::new(
        &http,
        &scheduler,
        &store,
        &root,
        DownloadOptions {
            retries: 2,
            backoff: Duration::from_millis(1),
        },
    );
    syncer.asset_url_base = base.clone();

    // The loader is not pinned: the listing's stable choice (0.19.5, not
    // the newer beta) is the one installed -- the URLs on the server name
    // it, and any other pick 404s.
    let platform = Platform::new(Os::Linux, "6.8.0", Arch::X86_64);
    let report = install_loader(
        &syncer,
        &cache,
        &server.url("/manifest.json"),
        LoaderInstall {
            sources: &LoaderSources::at(LoaderKind::Fabric, server.base.clone()),
            game: "1.20.1",
            loader: None,
            game_dir: None,
        },
        &platform,
    )
    .unwrap();

    assert_eq!(report.version.id, "fabric-loader-0.19.5-1.20.1");
    assert_eq!(
        report.version.main_class,
        "net.fabricmc.loader.impl.launch.knot.KnotClient"
    );
    let text = std::fs::read_to_string(root.version_json("fabric-loader-0.19.5-1.20.1")).unwrap();
    assert!(!text.contains("inheritsFrom"), "{text}");
    let resolved = Version::parse(&text).unwrap();
    assert_eq!(resolved.libraries.len(), 2, "loader + game's library");
    assert!(resolved.downloads.contains_key("client"));

    // The chain it walked: the listing, the profile it chose, the game.
    let hits = server.hits();
    assert!(
        hits.contains(&"/v2/versions/loader/1.20.1".to_string()),
        "{hits:?}"
    );
    assert!(
        hits.contains(&"/v2/versions/loader/1.20.1/0.19.5/profile/json".to_string()),
        "{hits:?}"
    );
    assert!(hits.contains(&"/game.json".to_string()), "{hits:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
