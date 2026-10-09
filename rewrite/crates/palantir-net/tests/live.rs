//! The live proof: a real game version downloaded end to end, with a
//! transfer that genuinely resumes.
//!
//! These tests are `#[ignore]`d like every network test in this project:
//! the offline suite is the deterministic gate, and this one asks the real
//! services and costs real bandwidth (~60 MB: 1.5.2 is the smallest complete
//! version -- client jar, libraries, and the pre-1.6 asset set). A failure
//! here is a failure: either a service moved or a protocol assumption broke.
//!
//! Run with:
//! `cargo test -p palantir-net --test live --locked -- --ignored --test-threads=1`

use std::path::PathBuf;
use std::time::Duration;

use palantir_core::paths::DataRoot;
use palantir_core::rules::{Arch, Os, Platform};
use palantir_core::version::{Version, VersionManifest};
use palantir_net::cache::{MANIFEST_TTL, MetadataCache, VERSION_TTL};
use palantir_net::client::Http;
use palantir_net::download::{self, DownloadOptions};
use palantir_net::scheduler::Scheduler;
use palantir_net::store::ContentStore;
use palantir_net::sync::Syncer;

/// The smallest complete version: ~60 MB where modern ones are ~500 MB.
const VERSION_ID: &str = "1.5.2";

fn temp_root(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir =
        std::env::temp_dir().join(format!("palantirmc-live-{tag}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn platform() -> Platform {
    Platform::new(Os::host(), "1.0", Arch::host())
}

/// Manifest -> version metadata, through the TTL cache like the app will.
fn fetch_version(http: &Http, cache: &MetadataCache) -> Version {
    let manifest_text = cache
        .fetch_text(
            http,
            "version-manifest-v2",
            "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json",
            MANIFEST_TTL,
        )
        .unwrap();
    let manifest = VersionManifest::parse(&manifest_text).unwrap();
    let entry = manifest.find(VERSION_ID).unwrap();
    let version_text = cache
        .fetch_text(
            http,
            &format!("version-{VERSION_ID}"),
            &entry.url,
            VERSION_TTL,
        )
        .unwrap();
    Version::parse(&version_text).unwrap()
}

#[test]
#[ignore = "downloads ~60 MB from live services"]
fn live_syncs_a_real_version_end_to_end() {
    let dir = temp_root("sync");
    let root = DataRoot::new(dir.join("root"));
    let game_dir = dir.join("game");
    let http = Http::new().unwrap();
    let cache = MetadataCache::new(palantir_net::cache::cache_dir_for(&root));
    let version = fetch_version(&http, &cache);

    let scheduler = Scheduler::with_default_limit();
    let store = ContentStore::new(root.content_dir());
    let syncer = Syncer::new(
        &http,
        &scheduler,
        &store,
        &root,
        DownloadOptions {
            retries: 3,
            backoff: Duration::from_millis(250),
        },
    );
    let (report, _client) = syncer
        .sync_version(&version, &platform(), Some(&game_dir))
        .unwrap();

    // Every landed file passed its hash check inside the sync; what is
    // worth asserting here is that the whole shape arrived.
    assert!(report.libraries > 5, "libraries: {}", report.libraries);
    assert_eq!(report.asset_index, 1);
    // The pre-1.6 index names 749 objects; it is frozen history, but say
    // "> 700" so a mirror's re-issue cannot flake this test.
    assert!(report.assets > 700, "assets: {}", report.assets);
    assert_eq!(
        report.fetched,
        report.libraries + report.natives + report.asset_index + report.assets + 1
    );

    // Independent re-check of the one file everything boots from.
    let client = version.downloads.get("client").unwrap();
    let jar = root.version_jar(&version.id);
    assert_eq!(
        download::sha1_file(&jar).unwrap().to_ascii_lowercase(),
        client.sha1.as_deref().unwrap().to_ascii_lowercase()
    );
    // This index writes into the game directory and did. The pre-1.6 index
    // is frozen history (it has no `lang/` objects at all -- those lived in
    // the game jar), so pinning one real name is safe: `icons/` shipped with
    // the resources directory since alpha.
    assert!(game_dir.join("resources/icons/icon_16x16.png").is_file());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "interrupts and resumes a live transfer"]
fn live_resumes_an_interrupted_client_jar() {
    let dir = temp_root("resume");
    let root = DataRoot::new(dir.join("root"));
    let http = Http::new().unwrap();
    let cache = MetadataCache::new(palantir_net::cache::cache_dir_for(&root));
    let version = fetch_version(&http, &cache);
    let client = version.downloads.get("client").unwrap();
    let url = client.url.as_deref().unwrap();
    let hash = client.sha1.as_deref().unwrap();
    let store = ContentStore::new(root.content_dir());
    let store_path = store.path(hash).unwrap();

    // Stage an interruption exactly as a dropped connection leaves one: the
    // first 256 KB of the real jar in the part file, nothing else.
    let staged = 256 * 1024u64;
    let mut response = http.get(url).unwrap();
    let part = download::part_path(&store_path);
    std::fs::create_dir_all(part.parent().unwrap()).unwrap();
    let mut file = std::fs::File::create(&part).unwrap();
    let written = response.copy_to(&mut file, Some(staged)).unwrap();
    drop(file);
    assert_eq!(written, staged, "staging the interruption failed");

    let transfer = download::download(
        &http,
        url,
        &store_path,
        Some(hash),
        client.size,
        &DownloadOptions {
            retries: 3,
            backoff: Duration::from_millis(250),
        },
    )
    .unwrap();

    // The receipt: it continued from the interruption, did not restart.
    assert_eq!(transfer.resumed_from, staged, "the transfer did not resume");
    assert!(transfer.bytes > 0);
    assert_eq!(
        download::sha1_file(&store_path)
            .unwrap()
            .to_ascii_lowercase(),
        hash.to_ascii_lowercase()
    );

    let _ = std::fs::remove_dir_all(&dir);
}
