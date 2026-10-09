//! Cache and transfer semantics against the mock server.
//!
//! Every behaviour here is one a real service produces and a test cannot
//! otherwise schedule: the interruption, the resume, the server that
//! ignores ranges, the transient failure, the considered refusal. The
//! receipt is always the same shape: the bytes on disk are right, and the
//! server's request log says how they got there.

use std::path::PathBuf;
use std::time::Duration;

use palantir_net::Error;
use palantir_net::cache::{MetadataCache, cache_dir_for, is_under};
use palantir_net::client::Http;
use palantir_net::download::{self, DownloadOptions};
use test_support::{MockServer, Route};

fn temp_dir(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    // Unique per call: these tests run in parallel and a shared directory
    // means one test deleting another's files mid-run.
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("palantirmc-net-{tag}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn body(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

fn options() -> DownloadOptions {
    DownloadOptions {
        retries: 3,
        backoff: Duration::from_millis(1),
    }
}

#[test]
fn the_cache_fetches_once_then_serves_from_disk() {
    let server = MockServer::start(vec![(
        "/manifest",
        Route::new(b"{\"latest\": {}}".to_vec()),
    )]);
    let dir = temp_dir("cache");
    let cache = MetadataCache::new(&dir);
    let http = Http::new().unwrap();
    let url = server.url("/manifest");

    let first = cache
        .fetch_text(&http, "manifest", &url, Duration::from_secs(60))
        .unwrap();
    let second = cache
        .fetch_text(&http, "manifest", &url, Duration::from_secs(60))
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(server.hits().len(), 1, "a fresh cache must not refetch");

    // A zero TTL says "always check", and the network answers again.
    cache
        .fetch_text(&http, "manifest", &url, Duration::ZERO)
        .unwrap();
    assert_eq!(server.hits().len(), 2);

    // And the cached envelope lives where the cache says it does.
    let cached = dir.join("manifest.json");
    assert!(is_under(&dir, &cached) && cached.is_file());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cache_dir_belongs_to_the_data_root() {
    let root = palantir_core::paths::DataRoot::new("/data");
    assert_eq!(cache_dir_for(&root), PathBuf::from("/data/cache/metadata"));
}

#[test]
fn an_interrupted_transfer_resumes_and_says_so() {
    let payload = body(10_000);
    let mut route = Route::new(payload.clone());
    route.truncate_after = Some(2_000);
    let server = MockServer::start(vec![("/big", route)]);
    let dir = temp_dir("resume");
    let dest = dir.join("big.bin");
    let http = Http::new().unwrap();
    let hash = download::sha1_bytes(&payload);

    let transfer = download::download(
        &http,
        &server.url("/big"),
        &dest,
        Some(&hash),
        Some(payload.len() as u64),
        &options(),
    )
    .unwrap();

    assert_eq!(std::fs::read(&dest).unwrap(), payload);
    // The receipt: the winning attempt continued from where the bytes
    // stopped, and the server was asked exactly that.
    assert_eq!(transfer.resumed_from, 2_000);
    assert_eq!(transfer.attempts, 2);
    assert_eq!(transfer.bytes, 8_000);
    assert!(
        server.range_offsets().contains(&2_000),
        "no Range request was seen: {:?}",
        server.range_offsets()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_server_that_ignores_ranges_restarts_cleanly() {
    let payload = body(10_000);
    let mut route = Route::new(payload.clone());
    route.support_range = false;
    route.truncate_after = Some(2_000);
    let server = MockServer::start(vec![("/nore", route)]);
    let dir = temp_dir("restart");
    let dest = dir.join("big.bin");
    let http = Http::new().unwrap();
    let hash = download::sha1_bytes(&payload);

    let transfer = download::download(
        &http,
        &server.url("/nore"),
        &dest,
        Some(&hash),
        Some(payload.len() as u64),
        &options(),
    )
    .unwrap();

    // Correct bytes, and the receipt says restart, not resume: a 200
    // replaced the partial rather than continuing it.
    assert_eq!(std::fs::read(&dest).unwrap(), payload);
    assert_eq!(transfer.resumed_from, 0);
    assert!(transfer.attempts >= 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_transient_500_retries_then_succeeds() {
    let payload = body(1_000);
    let mut route = Route::new(payload.clone());
    route.fail_first = 2;
    let server = MockServer::start(vec![("/flaky", route)]);
    let dir = temp_dir("retry");
    let dest = dir.join("f.bin");
    let http = Http::new().unwrap();
    let hash = download::sha1_bytes(&payload);

    let transfer = download::download(
        &http,
        &server.url("/flaky"),
        &dest,
        Some(&hash),
        None,
        &options(),
    )
    .unwrap();
    assert_eq!(transfer.attempts, 3);
    assert_eq!(server.hits().len(), 3);
    assert_eq!(std::fs::read(&dest).unwrap(), payload);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_404_is_a_refusal_and_is_not_retried() {
    let mut route = Route::new(Vec::new());
    route.always_status = Some(404);
    let server = MockServer::start(vec![("/gone", route)]);
    let dir = temp_dir("404");
    let http = Http::new().unwrap();

    let err = download::download(
        &http,
        &server.url("/gone"),
        &dir.join("x.bin"),
        None,
        None,
        &options(),
    )
    .unwrap_err();
    assert!(
        matches!(
            err,
            Error::Http {
                status: Some(404),
                ..
            }
        ),
        "{err}"
    );
    assert_eq!(
        server.hits().len(),
        1,
        "a considered refusal is not retried"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn wrong_bytes_are_an_error_and_are_kept_nowhere() {
    let mut payload = body(1_000);
    let server = MockServer::start(vec![("/liar", Route::new(payload.clone()))]);
    let dir = temp_dir("hash");
    let dest = dir.join("x.bin");
    let http = Http::new().unwrap();
    payload[0] ^= 0xFF; // a hash that does not match what is served
    let wrong_hash = download::sha1_bytes(&payload);

    let err = download::download(
        &http,
        &server.url("/liar"),
        &dest,
        Some(&wrong_hash),
        None,
        &options(),
    )
    .unwrap_err();
    assert!(matches!(err, Error::Hash { .. }), "{err}");
    assert!(
        !dest.exists(),
        "wrong bytes must not become the destination"
    );
    assert!(
        !download::part_path(&dest).exists(),
        "wrong bytes must not seed a later resume"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_complete_partial_finalizes_without_a_body() {
    let payload = body(4_000);
    let server = MockServer::start(vec![("/whole", Route::new(payload.clone()))]);
    let dir = temp_dir("416");
    let dest = dir.join("w.bin");
    let http = Http::new().unwrap();
    let hash = download::sha1_bytes(&payload);

    // Everything already arrived in an earlier life; only the rename and
    // the verification are missing.
    std::fs::write(download::part_path(&dest), &payload).unwrap();
    let transfer = download::download(
        &http,
        &server.url("/whole"),
        &dest,
        Some(&hash),
        Some(payload.len() as u64),
        &options(),
    )
    .unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), payload);
    assert_eq!(transfer.bytes, 0);
    assert_eq!(server.hits().len(), 1, "one range probe, no body");
    let _ = std::fs::remove_dir_all(&dir);
}
