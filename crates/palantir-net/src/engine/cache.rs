//! The metadata cache: a TTL, a validator, and one file per URL.
//!
//! The launcher already caches metadata in `crate::meta`, with one rule: if the
//! file is on disk, use it. That is not a cache, it is a write-once archive.
//! `meta.prismlauncher.org` publishes a new Fabric or Quilt build whenever
//! Fabric or Quilt does, and a client that never asks again is a client that
//! offers an old loader until somebody deletes a directory by hand -- which is
//! one of the bugs this rewrite exists to remove.
//!
//! So an entry here has an **age**, and what happens when the age passes the TTL
//! is not "download it again" but "ask whether it is still current":
//!
//! * within the TTL -- the bytes on disk are the answer, and no request is made;
//! * past the TTL with a validator -- `If-None-Match` goes out, and a `304` moves
//!   the stamp without moving a byte;
//! * past the TTL without one -- the body comes down again, because a service
//!   that sends no `ETag` has left the cache nothing to ask with.
//!
//! ## Why the stamp is a separate file
//!
//! `<url-hash>.body` is the payload and `<url-hash>.stamp` says when it was last
//! confirmed and what validator it came with. Keeping the two apart means the
//! body is exactly what the service sent -- readable by anything, comparable
//! byte for byte in a test -- instead of a payload wrapped in a bookkeeping
//! envelope that every reader would have to unwrap. The stamp is written second,
//! so an interruption between the two leaves a body nobody claims, which is
//! re-fetched; the other order would leave a stamp claiming freshness for bytes
//! that never arrived.
//!
//! ## What this is not
//!
//! It is not a resolver and it does not know what a version is. It maps a URL to
//! bytes with an age on them, so that the metadata layers above it -- Prism's
//! index, Mojang's piston meta, Modrinth's API -- share one TTL, one validator
//! scheme and one place to be wrong. It is also not a second retry policy: a
//! lookup runs under the [`Backoff`] it is handed, which is the engine's.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::engine::cancel::Cancel;
use crate::engine::request::{Fetch, Request};
use crate::engine::retry::{retry, Backoff};
use crate::Error;

/// How long a metadata *list* is believed without asking.
///
/// Thirty minutes. A version list changes when someone publishes a build, which
/// is a handful of times a day, and re-asking more often than this spends a
/// request per page view on an answer that has not moved. It is short enough
/// that a user who restarts the launcher after a loader release sees it.
pub const DEFAULT_TTL: Duration = Duration::from_secs(30 * 60);

/// How long a metadata *version file* is believed.
///
/// A year, which is forever in practice. A version file describes something
/// released: `net.minecraft/1.20.4.json` is not rewritten, and its digests are
/// what the client verifies every download against. Re-fetching one is pure
/// waste, but the entry is still a TTL rather than an infinity, so a year from
/// now it can be revalidated -- or the file can be deleted and the launcher can
/// forget how to ask, which an "immutable" directory never could.
pub const IMMUTABLE_TTL: Duration = Duration::from_secs(365 * 24 * 60 * 60);

/// A copy the cache is holding, with what is known about it.
#[derive(Debug, Clone)]
pub struct Cached {
    /// The URL these bytes answer.
    pub url: String,
    /// The bytes, exactly as the service sent them.
    pub body: Vec<u8>,
    /// When they were fetched, or last confirmed without being fetched.
    pub fetched: SystemTime,
    /// The validator the service gave, if it gave one.
    pub etag: Option<String>,
    /// Whether this answer came off the disk without a request.
    ///
    /// True for a hit inside the TTL and for a `304` -- in both cases the bytes
    /// that were returned are the ones that were already here. False when they
    /// were just downloaded, which is the difference a caller needs to tell
    /// "this was free" from "this cost a request".
    pub from_disk: bool,
}

impl Cached {
    /// How long ago these bytes were last confirmed.
    ///
    /// Clamped at zero: a stamp written by a clock that is ahead of this one is
    /// a copy just confirmed rather than a negative age, and the alternative --
    /// a saturating subtraction that goes the other way -- would report every
    /// entry as permanently stale on a machine whose clock was fixed backwards.
    pub fn age(&self, now: SystemTime) -> Duration {
        now.duration_since(self.fetched).unwrap_or_default()
    }

    /// Whether these bytes are still inside `ttl`.
    ///
    /// A `ttl` of zero is never fresh, which is deliberate: it is the one value
    /// that means "always ask", and it is how a test reaches the revalidation
    /// path without waiting for a clock.
    pub fn fresh_at(&self, now: SystemTime, ttl: Duration) -> bool {
        self.age(now) < ttl
    }
}

/// What a stamp file says about the body beside it.
struct Stamp {
    /// The validator, if the service sent one.
    etag: Option<String>,
    /// When the body was fetched or last confirmed.
    fetched: SystemTime,
    /// The URL, kept so a human reading the cache directory can tell what the
    /// hashed file name was for.
    url: String,
}

/// A directory of bodies and their stamps, keyed by URL hash.
#[derive(Debug, Clone)]
pub struct MetadataCache {
    dir: PathBuf,
    ttl: Duration,
}

impl MetadataCache {
    /// A cache in `dir`, believing entries for `ttl`.
    ///
    /// The directory is created on the first store rather than here, so
    /// constructing a cache is not a filesystem operation that can fail -- and
    /// so a launcher that never fetches anything never creates the directory
    /// either.
    pub fn new(dir: impl Into<PathBuf>, ttl: Duration) -> MetadataCache {
        MetadataCache { dir: dir.into(), ttl }
    }

    /// The same cache with a different TTL.
    pub fn with_ttl(mut self, ttl: Duration) -> MetadataCache {
        self.ttl = ttl;
        self
    }

    /// Where the bodies live.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// How long an entry is believed.
    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    /// What is on disk for `url`, fresh or not.
    ///
    /// Both files have to be there and the stamp has to parse: an entry with no
    /// age is not a stale answer, it is an answer with nothing known about it,
    /// and serving it would be the write-once archive this module replaced. Such
    /// a body is left where it is and overwritten by the next store.
    pub fn cached(&self, url: &str) -> Option<Cached> {
        let stamp = read_stamp(&self.stamp_path(url))?;
        let body = std::fs::read(self.body_path(url)).ok()?;
        Some(Cached {
            url: url.to_string(),
            body,
            fetched: stamp.fetched,
            etag: stamp.etag,
            from_disk: true,
        })
    }

    /// What is on disk for `url`, only while it is inside the TTL.
    ///
    /// The question a page asks before it decides whether to wait: `Some` means
    /// the answer is already here and no request is needed.
    pub fn fresh(&self, url: &str) -> Option<Cached> {
        let held = self.cached(url)?;
        held.fresh_at(SystemTime::now(), self.ttl).then_some(held)
    }

    /// The bytes for `url`, from disk if they are fresh, from the service
    /// otherwise, retried under `backoff`.
    ///
    /// Blocking, like everything else in the engine: the caller decides which
    /// thread to pay for. An expired entry is revalidated rather than replaced
    /// when the service gave a validator, and a `304` is answered with the copy
    /// that is already on disk and a stamp that has moved.
    ///
    /// A failure is returned as it came, and the copy on disk is left alone: an
    /// install that is offline still has yesterday's version list, and the
    /// caller that wants it asks [`MetadataCache::cached`] for it rather than
    /// being handed stale bytes with no way to tell.
    pub fn get(
        &self,
        url: &str,
        fetch: &dyn Fetch,
        cancel: &Cancel,
        backoff: &Backoff,
    ) -> Result<Cached, Error> {
        let mut sleep = |wait: Duration| std::thread::sleep(wait);
        self.get_with_sleep(url, fetch, cancel, backoff, &mut sleep)
    }

    /// [`MetadataCache::get`] with the waiting handed to `sleep`.
    ///
    /// `sleep` is taken rather than called because a test asserts the backoff by
    /// the waits it asked for, and a test that waited 750 ms to prove a retry
    /// policy is a test that is slow on purpose. `download.rs` takes the same
    /// parameter for the same reason.
    pub fn get_with_sleep(
        &self,
        url: &str,
        fetch: &dyn Fetch,
        cancel: &Cancel,
        backoff: &Backoff,
        sleep: &mut dyn FnMut(Duration),
    ) -> Result<Cached, Error> {
        let Some(held) = self.cached(url) else {
            let response = retry(backoff, |_| fetch.get_with(&Request::get(url), cancel), sleep)?;
            // A service that withholds a body the caller has never seen has
            // nothing to withhold it *for*: storing the empty answer would be a
            // cache entry that is permanently wrong, so this is a failure. It
            // means a broken server or a proxy answering for one.
            if response.not_modified {
                return Err(Error::format(url, "not modified, with nothing stored to be current"));
            }
            return self.put(url, &response.body, response.etag.as_deref());
        };
        if held.fresh_at(SystemTime::now(), self.ttl) {
            return Ok(held);
        }
        self.revalidate(url, held, fetch, cancel, backoff, sleep)
    }

    /// Ask whether a stored copy is still current, and take the new one if not.
    fn revalidate(
        &self,
        url: &str,
        held: Cached,
        fetch: &dyn Fetch,
        cancel: &Cancel,
        backoff: &Backoff,
        sleep: &mut dyn FnMut(Duration),
    ) -> Result<Cached, Error> {
        let request = match &held.etag {
            // A validator goes back exactly as it came: an `ETag` is opaque and
            // some services are strict about the quotes.
            Some(etag) => Request::get(url).header("If-None-Match", etag.clone()),
            // Without one the request is unconditional, and the body comes down
            // again. Saying so is the honest part: the alternative is to serve
            // the stored copy forever and never learn that the list moved.
            None => Request::get(url),
        };
        let response = retry(backoff, |_| fetch.get_with(&request, cancel), sleep)?;
        if !response.not_modified {
            return self.put(url, &response.body, response.etag.as_deref());
        }
        // The bytes stay where they are; only the age resets. This is the whole
        // point of keeping the validator.
        let now = SystemTime::now();
        self.write_stamp(url, held.etag.as_deref(), now)?;
        Ok(Cached { fetched: now, from_disk: true, ..held })
    }

    /// Store `body` for `url`, with `etag` if the service gave one.
    pub fn put(&self, url: &str, body: &[u8], etag: Option<&str>) -> Result<Cached, Error> {
        let now = SystemTime::now();
        write_atomic(&self.body_path(url), body)?;
        // Second, and never first: a body with no stamp is re-fetched, and a
        // stamp with no body is an entry that is believed and cannot be read.
        self.write_stamp(url, etag, now)?;
        Ok(Cached {
            url: url.to_string(),
            body: body.to_vec(),
            fetched: now,
            etag: etag.map(str::to_string),
            from_disk: false,
        })
    }

    /// Drop what is held for `url`, returning whether anything was.
    ///
    /// The caller's way out of a bad answer: a digest that came back wrong, a
    /// version list that is missing a build the user can see on the website.
    pub fn forget(&self, url: &str) -> bool {
        let body = std::fs::remove_file(self.body_path(url)).is_ok();
        let _ = std::fs::remove_file(self.stamp_path(url));
        body
    }

    /// Every entry the cache holds, as `(url, fetched)`, sorted by URL.
    ///
    /// Read from the stamps, because a stamp is what makes a body an entry; a
    /// body whose stamp did not survive is counted by neither this nor
    /// [`MetadataCache::len`], and is overwritten the next time the URL is
    /// stored.
    pub fn entries(&self) -> Vec<(String, SystemTime)> {
        let mut found = Vec::new();
        let Ok(dir) = std::fs::read_dir(&self.dir) else {
            return found;
        };
        for entry in dir.flatten() {
            let path = entry.path();
            if path.extension().and_then(|kind| kind.to_str()) != Some("stamp") {
                continue;
            }
            if let Some(stamp) = read_stamp(&path) {
                found.push((stamp.url, stamp.fetched));
            }
        }
        found.sort();
        found
    }

    /// How many entries the cache holds.
    pub fn len(&self) -> usize {
        self.entries().len()
    }

    /// Whether the cache holds nothing.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Where a URL's body goes: the hash of the URL, so the name is a fixed
    /// length whatever the URL contains and cannot escape the directory.
    fn body_path(&self, url: &str) -> PathBuf {
        self.dir.join(format!("{}.body", key_for(url)))
    }

    /// Where a URL's stamp goes.
    fn stamp_path(&self, url: &str) -> PathBuf {
        self.dir.join(format!("{}.stamp", key_for(url)))
    }

    /// Write the age and validator of the body beside it.
    fn write_stamp(&self, url: &str, etag: Option<&str>, fetched: SystemTime) -> Result<(), Error> {
        let millis = fetched.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
        let millis = u64::try_from(millis).unwrap_or(u64::MAX);
        // Three lines, and no escaping: a header value cannot contain a line
        // break, and the URL is the last line so a newline in it -- which no URL
        // we build has -- could only ever affect the line that is not read back.
        let text = format!("{}\n{}\n{}\n", etag.unwrap_or_default(), millis, url);
        write_atomic(&self.stamp_path(url), text.as_bytes())
    }
}

/// The cache key for a URL.
///
/// A digest rather than the URL itself, because a URL is not a file name: it
/// carries `:`, `?`, `/` and, on Windows, characters that are illegal in a
/// name. Hashing fixes the length too, so a query string cannot produce a path
/// the filesystem refuses.
fn key_for(url: &str) -> String {
    crate::download::sha256_hex(url.as_bytes())
}

/// Read a stamp, or `None` when it is missing, empty or unreadable.
///
/// Every failure is the same failure -- an entry that cannot be aged is not an
/// entry -- so they are not distinguished, and the caller re-fetches.
fn read_stamp(path: &Path) -> Option<Stamp> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let etag = lines.next()?.to_string();
    let millis: u64 = lines.next()?.trim().parse().ok()?;
    let url = lines.next().unwrap_or_default().to_string();
    Some(Stamp {
        etag: (!etag.is_empty()).then_some(etag),
        fetched: UNIX_EPOCH + Duration::from_millis(millis),
        url,
    })
}

/// Write `bytes` to `path` through a sibling temporary file.
///
/// `palantir_core::util::atomic_write` does the same thing, but it reports
/// through `palantir_core`'s error type and this crate's errors are its own; a
/// second conversion layer for eight lines of filesystem work would cost more
/// than it explains. The name carries the process id so two launchers sharing a
/// cache directory cannot overwrite each other's half-written file.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|error| Error::io(parent, error))?;
        }
    }
    let temp = temp_path(path);
    std::fs::write(&temp, bytes).map_err(|error| Error::io(&temp, error))?;
    if let Err(error) = std::fs::rename(&temp, path) {
        let _ = std::fs::remove_file(&temp);
        return Err(Error::io(path, error));
    }
    Ok(())
}

/// A sibling of `path` to write before renaming it into place.
fn temp_path(path: &Path) -> PathBuf {
    let mut name = std::ffi::OsString::from(".");
    name.push(path.file_name().unwrap_or_default());
    name.push(format!(".part-{}", std::process::id()));
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::request::{MapFetch, Outcome, Response, Route};
    use std::cell::RefCell;
    use std::io::Write;

    const URL: &str = "https://meta.example.invalid/v1/net.minecraft/index.json";

    /// A scratch directory named for the test, emptied first.
    fn dir_of(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join("palantirmc-engine-cache").join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a scratch directory");
        root
    }

    fn cache_in(root: &Path, ttl: Duration) -> MetadataCache {
        // Under a subdirectory that does not exist yet, so every test also says
        // the cache creates its own directory rather than needing one made.
        MetadataCache::new(root.join("meta"), ttl)
    }

    /// A lookup whose waits are recorded rather than spent.
    fn get(
        cache: &MetadataCache,
        url: &str,
        fetch: &dyn Fetch,
        backoff: &Backoff,
    ) -> (Result<Cached, Error>, Vec<Duration>) {
        let waits = RefCell::new(Vec::new());
        let got = {
            let mut sleep = |wait: Duration| waits.borrow_mut().push(wait);
            cache.get_with_sleep(url, fetch, &Cancel::new(), backoff, &mut sleep)
        };
        (got, waits.into_inner())
    }

    #[test]
    fn a_first_lookup_fetches_stores_and_says_it_was_not_free() {
        let root = dir_of("first");
        let cache = cache_in(&root, DEFAULT_TTL);
        let fetch = MapFetch::new().with_route(URL, Route::text("index").tagged("\"v1\""));

        let (got, waits) = get(&cache, URL, &fetch, &Backoff::with_attempts(1));
        let held = got.expect("the body");
        assert_eq!(held.body, b"index");
        assert!(!held.from_disk, "it cost a request");
        assert_eq!(held.etag.as_deref(), Some("\"v1\""));
        assert!(held.age(SystemTime::now()) < Duration::from_secs(5));
        assert!(waits.is_empty());

        // And it is on disk in both halves: a body anyone can read, and a stamp
        // that ages it.
        assert!(cache.dir().is_dir(), "the cache made its own directory");
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.entries()[0].0, URL);
        assert!(cache.cached(URL).is_some());
    }

    #[test]
    fn a_lookup_inside_the_ttl_never_reaches_the_service() {
        let root = dir_of("fresh-hit");
        let cache = cache_in(&root, DEFAULT_TTL);
        let fetch = MapFetch::new().with_route(URL, Route::text("index"));
        get(&cache, URL, &fetch, &Backoff::with_attempts(1)).0.expect("the first");

        // A second fetch with nothing configured would be a 404, so a request
        // that was made would show up as an error rather than as silence.
        let nothing = MapFetch::new();
        let (got, _) = get(&cache, URL, &nothing, &Backoff::with_attempts(1));
        assert_eq!(got.expect("the stored copy").body, b"index");
        assert_eq!(nothing.count(), 0, "no request was made");
        assert_eq!(fetch.count(), 1);
        assert!(cache.fresh(URL).is_some());
    }

    #[test]
    fn an_expired_entry_is_revalidated_and_the_stamp_moves() {
        let root = dir_of("revalidate");
        // Short enough that a test can cross it by sleeping, long enough not to
        // be crossed by accident.
        let cache = cache_in(&root, Duration::from_millis(40));
        let fetch = MapFetch::new().with_route(URL, Route::text("index").tagged("\"v7\""));
        get(&cache, URL, &fetch, &Backoff::with_attempts(1)).0.expect("the first");
        assert!(cache.fresh(URL).is_some());

        std::thread::sleep(Duration::from_millis(80));
        assert!(cache.fresh(URL).is_none(), "the TTL has passed");
        assert!(cache.cached(URL).is_some(), "but the copy is still there");

        let before = SystemTime::now();
        let second = get(&cache, URL, &fetch, &Backoff::with_attempts(1)).0.expect("the second");
        assert_eq!(second.body, b"index");
        assert!(second.from_disk, "the bytes were already here");
        assert_eq!(fetch.count(), 2, "the second lookup did ask");
        let asked = fetch.requests();
        assert_eq!(
            asked[1].header_value("If-None-Match"),
            Some("\"v7\""),
            "the validator went back exactly as it came"
        );
        assert!(
            cache.fresh(URL).is_some(),
            "a 304 refreshes the age, which is the whole point of keeping the validator"
        );
        assert!(second.fetched >= before);
    }

    #[test]
    fn a_service_that_republished_something_replaces_the_stored_copy() {
        let root = dir_of("republished");
        // Zero TTL: stale the moment it is stored, so the test reaches the
        // revalidation path without waiting for anything.
        let cache = cache_in(&root, Duration::ZERO);
        let fetch = MapFetch::new().with_route(URL, Route::text("old").tagged("\"v1\""));
        get(&cache, URL, &fetch, &Backoff::with_attempts(1)).0.expect("the first");

        // A republish is new bytes *and* a new validator; `set_body` alone would
        // leave a validator claiming the old bytes are current, which is a
        // service telling the truth about the wrong thing.
        fetch.set_route(URL, Route::text("new").tagged("\"v2\""));
        let held = get(&cache, URL, &fetch, &Backoff::with_attempts(1)).0.expect("the second");
        assert_eq!(held.body, b"new");
        assert!(!held.from_disk, "it was downloaded again");
        let stored = cache.cached(URL).expect("the stored copy");
        assert_eq!(stored.body, b"new");
        assert_eq!(stored.etag.as_deref(), Some("\"v2\""));
    }

    #[test]
    fn a_service_with_no_validator_costs_the_whole_body_again() {
        // The honest fallback: with nothing to ask, the only way to learn that a
        // list moved is to fetch it.
        let root = dir_of("no-validator");
        let cache = cache_in(&root, Duration::ZERO);
        let fetch = MapFetch::new().with_route(URL, Route::text("list"));
        let first = get(&cache, URL, &fetch, &Backoff::with_attempts(1)).0.expect("the first");
        assert_eq!(first.etag, None);

        let second = get(&cache, URL, &fetch, &Backoff::with_attempts(1)).0.expect("the second");
        assert_eq!(second.body, b"list");
        assert!(!second.from_disk);
        let asked = fetch.requests();
        assert_eq!(asked.len(), 2);
        assert_eq!(asked[1].header_value("If-None-Match"), None, "nothing to ask with");
    }

    #[test]
    fn a_failed_lookup_reports_it_and_leaves_the_copy_where_it_was() {
        let root = dir_of("failure");
        let cache = cache_in(&root, Duration::ZERO);
        let fetch = MapFetch::new().with_route(URL, Route::text("yesterday").tagged("\"v1\""));
        get(&cache, URL, &fetch, &Backoff::with_attempts(1)).0.expect("the first");
        let stamped = cache.cached(URL).expect("the first copy").fetched;

        // The service is down for the one attempt the policy allows.
        fetch.set_route(URL, Route::text("yesterday").tagged("\"v1\"").failing(1, 500));
        let (got, waits) = get(&cache, URL, &fetch, &Backoff::with_attempts(1));
        let error = got.expect_err("a 500 is a failure, not a stale answer");
        assert!(matches!(error, Error::Http { status: Some(500), .. }), "{error:?}");
        assert!(waits.is_empty(), "one attempt means no wait");

        let kept = cache.cached(URL).expect("the copy is untouched");
        assert_eq!(kept.body, b"yesterday");
        assert_eq!(kept.fetched, stamped, "and so is the age on it");
    }

    #[test]
    fn a_retryable_failure_is_tried_again_before_the_caller_hears_about_it() {
        let root = dir_of("retry");
        let cache = cache_in(&root, DEFAULT_TTL);
        let fetch = MapFetch::new().with_route(URL, Route::text("late").failing(2, 503));

        let (got, waits) = get(&cache, URL, &fetch, &Backoff::with_attempts(3));
        assert_eq!(got.expect("the third attempt").body, b"late");
        assert_eq!(fetch.count(), 3);
        assert_eq!(
            waits,
            vec![Duration::from_millis(250), Duration::from_millis(500)],
            "the policy's waits, in order"
        );
    }

    #[test]
    fn a_cancelled_lookup_stops_and_stores_nothing() {
        let root = dir_of("cancelled");
        let cache = cache_in(&root, DEFAULT_TTL);
        let cancel = Cancel::new();
        cancel.cancel();
        let fetch = MapFetch::new().with_route(URL, Route::body(vec![1u8; 64]).chunked(8));

        let mut sleep = |_: Duration| {};
        let error = cache
            .get_with_sleep(URL, &fetch, &cancel, &Backoff::with_attempts(3), &mut sleep)
            .expect_err("cancelled before the first chunk");
        assert!(matches!(error, Error::Cancelled), "{error:?}");
        assert!(cache.cached(URL).is_none(), "nothing was stored");
        assert_eq!(fetch.count(), 1, "and it was not retried");
    }

    /// A service that answers `304` to everything, including a request that
    /// carried no validator.
    struct AlwaysCurrent;

    impl Fetch for AlwaysCurrent {
        fn get(&self, request: &Request, _cancel: &Cancel) -> Result<Vec<u8>, Error> {
            Err(Error::format(&request.url, "no body to give"))
        }

        fn get_to(
            &self,
            request: &Request,
            _sink: &mut dyn Write,
            _cancel: &Cancel,
        ) -> Result<Outcome, Error> {
            Err(Error::format(&request.url, "no body to give"))
        }

        fn get_with(&self, _request: &Request, _cancel: &Cancel) -> Result<Response, Error> {
            Ok(Response {
                body: Vec::new(),
                etag: Some("\"v0\"".to_string()),
                not_modified: true,
            })
        }
    }

    #[test]
    fn a_304_with_nothing_stored_is_a_failure_rather_than_an_empty_file() {
        // A server that says "still current" about a copy the caller never had
        // is broken, and storing its empty answer would make the cache wrong
        // forever: every later lookup would be served nothing, freshly.
        let root = dir_of("bogus-304");
        let cache = cache_in(&root, DEFAULT_TTL);
        let (got, _) = get(&cache, URL, &AlwaysCurrent, &Backoff::with_attempts(1));
        let error = got.expect_err("nothing to be current with");
        assert!(matches!(error, Error::Format { .. }), "{error:?}");
        assert!(cache.is_empty());
    }

    #[test]
    fn each_url_is_its_own_entry_and_can_be_forgotten() {
        let root = dir_of("separate");
        let cache = cache_in(&root, DEFAULT_TTL);
        let fetch = MapFetch::new()
            .with_route("https://a.invalid/one.json", Route::text("one"))
            .with_route("https://b.invalid/two.json", Route::text("two"));
        get(&cache, "https://a.invalid/one.json", &fetch, &Backoff::with_attempts(1))
            .0
            .expect("the first");
        get(&cache, "https://b.invalid/two.json", &fetch, &Backoff::with_attempts(1))
            .0
            .expect("the second");

        assert_eq!(cache.len(), 2);
        assert!(!cache.is_empty());
        assert_eq!(cache.cached("https://a.invalid/one.json").expect("one").body, b"one");
        assert_eq!(cache.entries()[0].0, "https://a.invalid/one.json", "sorted by url");

        assert!(cache.forget("https://a.invalid/one.json"));
        assert!(!cache.forget("https://a.invalid/one.json"), "it was already gone");
        assert_eq!(cache.len(), 1);
        assert!(cache.cached("https://a.invalid/one.json").is_none());
        assert!(cache.fresh("https://a.invalid/one.json").is_none());
        assert_eq!(cache.cached("https://b.invalid/two.json").expect("two").body, b"two");
    }

    #[test]
    fn ttl_of_zero_is_never_fresh_and_the_year_one_always_is() {
        let now = SystemTime::now();
        let held = Cached {
            url: URL.to_string(),
            body: b"{}".to_vec(),
            fetched: now - Duration::from_secs(300 * 24 * 60 * 60),
            etag: Some("\"v1\"".to_string()),
            from_disk: true,
        };
        assert!(held.age(now) >= Duration::from_secs(300 * 24 * 60 * 60));
        assert!(
            held.fresh_at(now, IMMUTABLE_TTL),
            "a released version file does not change, so a 300-day-old one is current"
        );
        assert!(!held.fresh_at(now, DEFAULT_TTL), "a list of them does change");
        assert!(!held.fresh_at(now, Duration::ZERO), "which is what always-ask means");
        // A stamp from the future is a copy just confirmed, not a negative age.
        let ahead = Cached { fetched: now + Duration::from_secs(60), ..held };
        assert_eq!(ahead.age(now), Duration::ZERO);
        assert!(ahead.fresh_at(now, DEFAULT_TTL));
    }

    #[test]
    fn an_entry_with_no_stamp_is_not_an_entry() {
        // An interruption between the two writes leaves a body nobody claims.
        // Serving it would be a cache that cannot say how old its answer is,
        // which is the write-once archive again.
        let root = dir_of("orphan");
        let cache = MetadataCache::new(root.join("meta"), DEFAULT_TTL);
        std::fs::create_dir_all(cache.dir()).expect("the cache directory");
        std::fs::write(cache.body_path(URL), b"orphan").expect("a body with no stamp");

        assert!(cache.cached(URL).is_none());
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());

        // And the next lookup overwrites it rather than failing over it.
        let fetch = MapFetch::new().with_route(URL, Route::text("whole"));
        let held = get(&cache, URL, &fetch, &Backoff::with_attempts(1)).0.expect("the body");
        assert_eq!(held.body, b"whole");
        assert_eq!(cache.cached(URL).expect("now an entry").body, b"whole");
    }

    #[test]
    fn a_stored_body_is_the_bytes_the_service_sent() {
        // No envelope around the payload: what is in the file is what a `curl`
        // would have written, which is what lets the metadata layers above this
        // one read the cache directory directly.
        let root = dir_of("raw");
        let cache = cache_in(&root, DEFAULT_TTL);
        let body = br#"{"formatVersion":1,"versions":[]}"#;
        cache.put(URL, body, Some("\"v1\"")).expect("stored");
        let path = cache.body_path(URL);
        assert_eq!(std::fs::read(&path).expect("the body file"), body);
        assert_eq!(
            std::fs::read_to_string(cache.stamp_path(URL)).expect("the stamp").lines().count(),
            3
        );
        // And the file name is a digest, whatever punctuation the URL carried.
        assert!(!path.to_string_lossy().contains("index.json"));
        assert_eq!(cache.put(URL, b"again", None).expect("stored again").etag, None);
        assert_eq!(cache.len(), 1, "the same url is one entry, rewritten");
        assert_eq!(cache.cached(URL).expect("the newest").body, b"again");
    }
}
