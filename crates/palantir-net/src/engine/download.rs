//! One file, fetched: continued if it can be, restarted if it must be, verified
//! before it is called done.
//!
//! The bulk downloader writes `<dest>.part` and renames it on success, which is
//! the right shape and stops one step short of what a launcher needs: a phase
//! interrupted at file 900 of 1200 restarts at file 1, and a file interrupted
//! half way restarts from zero. Two thirds of a first install is 300 MB over
//! somebody's line, and the failure that costs it -- a closed laptop lid -- is
//! the common one.
//!
//! So this module keeps the part file and asks the server to continue from its
//! length. Four rules make that safe rather than a way to build corrupt jars:
//!
//! 1. **Only a 206 is a continuation.** A server that answers 200 to a `Range`
//!    request sent the body from zero, and [`Outcome::Ignored`] means nothing
//!    was written, so the file is truncated and fetched again rather than
//!    appended to. This is the case a naive implementation gets wrong, and the
//!    cost of getting it wrong is a jar that only fails at launch.
//! 2. **A part file is always a prefix.** Chunks are written whole, so whatever
//!    length the file has is a length of the body -- which is what makes
//!    "resume from `metadata().len()`" a correct thing to do rather than a
//!    hopeful one.
//! 3. **The hash decides, not the transfer.** A file with a known digest that
//!    does not match it is deleted and fetched again; "it downloaded" and "it is
//!    the right bytes" are different claims and only the second one matters.
//! 4. **A file that is already right is not fetched at all.** The digest is a
//!    content address: a verified file at the destination ends the job with no
//!    request and no bytes, which is what makes a re-run of a phase cheap.
//!
//! Cancellation keeps the part file, deliberately: a cancelled download is one
//! the user may want to resume, and the `.part` beside the target is exactly
//! that. A *failure* keeps it too, for the retry.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::download::{part_path, verify_sha256};
use crate::engine::cancel::Cancel;
use crate::engine::request::{Fetch, Outcome, Request};
use crate::engine::retry::{is_retryable, Backoff};
use crate::Error;

/// One file to fetch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    /// Where it comes from.
    pub url: String,
    /// Where it goes.
    pub dest: PathBuf,
    /// The `sha256` it must have, when the source says.
    ///
    /// Not optional in the sense of "nice to have": a download with a digest can
    /// be resumed, skipped when it is already there, and refused when it is
    /// wrong, and one without can do none of those things. Most of this
    /// launcher's downloads come from a publisher that states one.
    pub sha256: Option<String>,
}

impl Download {
    /// A file to fetch, to be taken on trust.
    pub fn new(url: impl Into<String>, dest: impl Into<PathBuf>) -> Download {
        Download { url: url.into(), dest: dest.into(), sha256: None }
    }

    /// The same file, with the digest it must have.
    pub fn verified(
        url: impl Into<String>,
        dest: impl Into<PathBuf>,
        sha256: impl Into<String>,
    ) -> Download {
        let sha256 = sha256.into().trim().to_ascii_lowercase();
        Download { sha256: (!sha256.is_empty()).then_some(sha256), ..Download::new(url, dest) }
    }

    /// The part file this download works in.
    pub fn part(&self) -> PathBuf {
        part_path(&self.dest)
    }

    /// Whether the destination already holds the right bytes.
    ///
    /// False without a digest, because there is nothing to compare: a launcher
    /// that skipped a file it could not check would be one that launches the
    /// wrong jar.
    pub fn satisfied(&self) -> bool {
        let Some(expected) = &self.sha256 else {
            return false;
        };
        self.dest.is_file() && verify_sha256(&self.dest, expected).is_ok()
    }
}

/// How a download finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Downloaded {
    /// The destination already held these bytes; no request was made.
    AlreadyThere,
    /// Fetched from the start.
    Fetched(u64),
    /// Fetched from where an interrupted attempt stopped.
    Resumed(u64),
}

impl Downloaded {
    /// How many bytes came off the network.
    pub fn bytes(self) -> u64 {
        match self {
            Downloaded::AlreadyThere => 0,
            Downloaded::Fetched(bytes) | Downloaded::Resumed(bytes) => bytes,
        }
    }

    /// Whether this attempt continued a partial file.
    pub fn resumed(self) -> bool {
        matches!(self, Downloaded::Resumed(_))
    }
}

/// Fetch `download`, retrying the failures that are worth retrying.
///
/// `sleep` is the sleeper; a real caller passes `std::thread::sleep` and a test
/// passes one that records, which is how "three attempts, 250ms apart" is
/// asserted rather than waited out.
///
/// The waits are spread by URL rather than taken straight from the policy. When
/// every download in a phase fails together -- which is what a 503 does -- the
/// un-spread version brings them all back at the same millisecond, and the
/// server that asked for less traffic gets its own retries in one burst. The
/// seed is the URL, so the same file always waits in the same places and a test
/// can assert them.
pub fn fetch_to_file(
    fetch: &dyn Fetch,
    download: &Download,
    cancel: &Cancel,
    backoff: &Backoff,
    sleep: &mut dyn FnMut(std::time::Duration),
) -> Result<Downloaded, Error> {
    if download.satisfied() {
        return Ok(Downloaded::AlreadyThere);
    }
    let seed = seed_of(&download.url);
    let mut tries = 0;
    loop {
        tries += 1;
        match attempt(fetch, download, cancel) {
            Ok(done) => return Ok(done),
            Err(error) => {
                // The retryable line first: a cancellation must not reach the
                // delay, or cancelling during a backoff would park the thread
                // for the rest of the wait before admitting it was cancelled.
                if !is_retryable(&error) {
                    return Err(error);
                }
                // `tries + 1` because the delay is the wait *before* an attempt:
                // after attempt `tries` failed, the question is how long until
                // the one about to be made.
                match backoff.delay_for(tries + 1, seed) {
                    Some(wait) if !wait.is_zero() => sleep(wait),
                    Some(_) => {}
                    None => return Err(error),
                }
            }
        }
    }
}

/// A stable number for a URL.
///
/// FNV-1a: the requirement is that the same URL gives the same seed on every run
/// and every machine, and that two different URLs almost never collide. A
/// `HashMap`'s hasher would do neither, since it is seeded per process.
fn seed_of(url: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in url.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// One attempt: resume if there is a part file, then verify and rename.
fn attempt(fetch: &dyn Fetch, download: &Download, cancel: &Cancel) -> Result<Downloaded, Error> {
    cancel.check()?;
    if let Some(parent) = download.dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|error| Error::io(parent, error))?;
        }
    }
    let part = download.part();
    let offset = std::fs::metadata(&part).map(|meta| meta.len()).unwrap_or(0);
    if offset == 0 {
        return fetch_whole(fetch, download, cancel);
    }
    // Append rather than create: what is already in the part file is a prefix of
    // the body and re-sending it would corrupt the file if the server continued
    // from where the header asked.
    let mut sink = OpenOptions::new()
        .append(true)
        .open(&part)
        .map_err(|error| Error::io(&part, error))?;
    match fetch.get_to(&Request::from(download.url.clone(), offset), &mut sink, cancel)? {
        Outcome::Resumed(fetched) => {
            sink.flush().map_err(|error| Error::io(&part, error))?;
            finish(download, Downloaded::Resumed(fetched))
        }
        // The server would not continue. Nothing was written, so the file is
        // still a prefix and starting again is safe rather than merely possible.
        Outcome::Ignored => fetch_whole(fetch, download, cancel),
        Outcome::Whole(_) => {
            // The double, or a caller, answered a whole body to a ranged
            // request, which the contract says it must not. Trusting it would
            // append a second copy of the file; refusing says so.
            Err(Error::format(&download.url, "a body arrived whole for an offset"))
        }
    }
}

/// Fetch from zero into a fresh part file, then verify and rename.
fn fetch_whole(fetch: &dyn Fetch, download: &Download, cancel: &Cancel) -> Result<Downloaded, Error> {
    let part = download.part();
    let file = File::create(&part).map_err(|error| Error::io(&part, error))?;
    let mut sink = std::io::BufWriter::with_capacity(64 * 1024, file);
    let outcome = fetch.get_to(&Request::get(download.url.clone()), &mut sink, cancel)?;
    if outcome == Outcome::Ignored {
        return Err(Error::format(&download.url, "a body was ignored for a whole request"));
    }
    sink.flush().map_err(|error| Error::io(&part, error))?;
    drop(sink);
    finish(download, Downloaded::Fetched(outcome.bytes()))
}

/// Verify the part file and put it where it belongs, reporting how it got here.
///
/// The finished state is passed in rather than derived from the byte count: a
/// resumed file is `Resumed` even though its total length is the whole body, and
/// a caller reading only the length would report every resume as a fresh fetch.
///
/// A digest that does not match removes the part rather than leaving it: a
/// corrupt prefix would be resumed from on the next attempt, and every attempt
/// after it would start from the same wrong bytes. Deleting is what makes the
/// retry of a hash mismatch an actual second try.
fn finish(download: &Download, done: Downloaded) -> Result<Downloaded, Error> {
    let part = download.part();
    if let Some(expected) = &download.sha256 {
        if let Err(error) = verify_sha256(&part, expected) {
            let _ = std::fs::remove_file(&part);
            return Err(error);
        }
    }
    rename(&part, &download.dest)?;
    Ok(done)
}

/// Move the part file over the destination.
///
/// A rename cannot cross a filesystem, and a launcher's cache and its instance
/// directory can be on two drives, so the fallback copies. It is rare and the
/// path is the same as the existing downloader's, which had the same problem.
fn rename(part: &Path, dest: &Path) -> Result<(), Error> {
    if std::fs::rename(part, dest).is_ok() {
        return Ok(());
    }
    std::fs::copy(part, dest).map_err(|error| Error::io(dest, error))?;
    std::fs::remove_file(part).map_err(|error| Error::io(part, error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::request::{MapFetch, Route};
    use std::time::Duration;

    const URL: &str = "https://example.invalid/a.jar";

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join("palantirmc-engine-download").join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a scratch directory");
        root
    }

    /// Fetch with a sleeper that records instead of waiting.
    fn fetch_now(
        fetch: &MapFetch,
        download: &Download,
        cancel: &Cancel,
    ) -> (Result<Downloaded, Error>, Vec<Duration>) {
        let mut waits = Vec::new();
        let result = fetch_to_file(
            fetch,
            download,
            cancel,
            &Backoff::with_attempts(3),
            &mut |wait| waits.push(wait),
        );
        (result, waits)
    }

    #[test]
    fn a_whole_file_lands_at_its_destination_and_leaves_no_part() {
        let dir = scratch("whole");
        let dest = dir.join("a.jar");
        let fetch = MapFetch::new().with_route(URL, Route::body(vec![7u8; 5000]));
        let download = Download::new(URL, &dest);
        let (result, waits) = fetch_now(&fetch, &download, &Cancel::new());
        assert_eq!(result.expect("fetched"), Downloaded::Fetched(5000));
        assert!(waits.is_empty());
        assert_eq!(std::fs::read(&dest).expect("the file").len(), 5000);
        assert!(!download.part().exists(), "the part file is gone");
        assert_eq!(fetch.count(), 1);
    }

    #[test]
    fn an_interrupted_download_continues_from_what_it_already_has() {
        // The point of the module: a part file that holds the first 300 bytes is
        // not thrown away, and the bytes that come back are the rest of the body
        // rather than the whole thing again.
        let dir = scratch("resume");
        let dest = dir.join("a.jar");
        let body: Vec<u8> = (0..1000u32).map(|n| n as u8).collect();
        let download = Download::new(URL, &dest);
        std::fs::write(download.part(), &body[..300]).expect("a partial file");

        let fetch = MapFetch::new().with_route(URL, Route::body(body.clone()));
        let (result, _) = fetch_now(&fetch, &download, &Cancel::new());
        assert_eq!(result.expect("resumed"), Downloaded::Resumed(700));
        assert_eq!(std::fs::read(&dest).expect("the file"), body);
        let requests = fetch.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].offset, Some(300), "it asked to continue from 300");
        assert_eq!(requests[0].range().as_deref(), Some("bytes=300-"));
    }

    #[test]
    fn a_server_that_will_not_continue_gets_a_second_request_from_zero() {
        // Rule 1, and the reason it is a rule: the first attempt wrote nothing,
        // so the file is replaced rather than appended to.
        let dir = scratch("restart");
        let dest = dir.join("a.jar");
        let body: Vec<u8> = (0..400u32).map(|n| n as u8).collect();
        let download = Download::new(URL, &dest);
        std::fs::write(download.part(), &body[..150]).expect("a partial file");

        let fetch = MapFetch::new().with_route(URL, Route::body(body.clone()).ignoring_range());
        let (result, _) = fetch_now(&fetch, &download, &Cancel::new());
        assert_eq!(result.expect("fetched"), Downloaded::Fetched(400), "not 550: nothing was appended");
        assert_eq!(std::fs::read(&dest).expect("the file"), body);
        let offsets: Vec<Option<u64>> = fetch.requests().iter().map(|r| r.offset).collect();
        assert_eq!(offsets, vec![Some(150), None], "ranged, then from zero");
    }

    #[test]
    fn a_part_file_longer_than_the_body_is_refetched_rather_than_appended_to() {
        // A stale part from a different build: the server cannot continue past
        // the end, so it must be treated as no part at all.
        let dir = scratch("stale-part");
        let dest = dir.join("a.jar");
        let body = vec![3u8; 100];
        let download = Download::new(URL, &dest);
        std::fs::write(download.part(), vec![0u8; 900]).expect("a stale part");

        let fetch = MapFetch::new().with_route(URL, Route::body(body.clone()));
        let (result, _) = fetch_now(&fetch, &download, &Cancel::new());
        assert_eq!(result.expect("fetched"), Downloaded::Fetched(100));
        assert_eq!(std::fs::read(&dest).expect("the file"), body);
    }

    #[test]
    fn a_file_that_is_already_right_is_not_fetched_at_all() {
        let dir = scratch("satisfied");
        let dest = dir.join("a.jar");
        let body = b"the right bytes".to_vec();
        std::fs::write(&dest, &body).expect("the file");
        let digest = crate::download::sha256_hex(&body);
        let download = Download::verified(URL, &dest, &digest);
        assert!(download.satisfied());

        let fetch = MapFetch::new().with_route(URL, Route::body(body));
        let (result, _) = fetch_now(&fetch, &download, &Cancel::new());
        let done = result.expect("already there");
        assert_eq!(done, Downloaded::AlreadyThere);
        assert_eq!(fetch.count(), 0, "no request at all");
        assert_eq!(done.bytes(), 0);
        // Without a digest there is nothing to compare, so it is fetched: the
        // same bytes, but the launcher cannot know that.
        let unverified = Download::new(URL, &dest);
        assert!(!unverified.satisfied());
        let (again, _) = fetch_now(&fetch, &unverified, &Cancel::new());
        assert_eq!(again.expect("fetched"), Downloaded::Fetched(15));
    }

    #[test]
    fn a_file_whose_bytes_are_wrong_is_refused_and_the_part_removed() {
        let dir = scratch("mismatch");
        let dest = dir.join("a.jar");
        let download = Download::verified(URL, &dest, crate::download::sha256_hex(b"expected"));
        let fetch = MapFetch::new().with_route(URL, Route::body(b"something else".to_vec()));
        let (result, waits) = fetch_now(&fetch, &download, &Cancel::new());
        let error = result.expect_err("a digest that does not match");
        assert!(matches!(error, Error::HashMismatch { .. }), "{error:?}");
        assert!(!dest.exists(), "nothing was put in place");
        assert!(!download.part().exists(), "the wrong bytes were deleted");
        // A hash mismatch is retryable -- the bytes arrived and were wrong, and
        // the only fix is to fetch them again -- so the policy gets its three
        // attempts and no more.
        assert_eq!(fetch.count(), 3, "three attempts: {:?}", waits);
        assert_eq!(waits.len(), 2);
    }

    #[test]
    fn a_failure_that_clears_is_retried_and_the_delay_is_spread() {
        let dir = scratch("retry");
        let dest = dir.join("a.jar");
        let download = Download::new(URL, &dest);
        let fetch = MapFetch::new().with_route(URL, Route::body(b"body".to_vec()).failing(2, 503));
        let (result, waits) = fetch_now(&fetch, &download, &Cancel::new());
        assert_eq!(result.expect("fetched"), Downloaded::Fetched(4));
        assert_eq!(fetch.count(), 3);
        assert_eq!(std::fs::read(&dest).expect("the file"), b"body");

        // Two waits for two failures: the first is the policy's 250ms spread
        // over its lower half, the second its 500ms, and neither is longer than
        // the interval it belongs to -- which is what keeps the median retry
        // where the policy says it is.
        assert_eq!(waits.len(), 2);
        assert!(waits[0] >= Duration::from_millis(125), "{:?}", waits[0]);
        assert!(waits[0] <= Duration::from_millis(250), "{:?}", waits[0]);
        assert!(waits[1] >= Duration::from_millis(250), "{:?}", waits[1]);
        assert!(waits[1] <= Duration::from_millis(500), "{:?}", waits[1]);
    }

    #[test]
    fn a_404_is_not_retried() {
        let dir = scratch("missing");
        let dest = dir.join("a.jar");
        let download = Download::new("https://example.invalid/gone.jar", &dest);
        let fetch = MapFetch::new();
        let (result, waits) = fetch_now(&fetch, &download, &Cancel::new());
        let error = result.expect_err("a 404");
        assert!(matches!(error, Error::Http { status: Some(404), .. }), "{error:?}");
        assert_eq!(fetch.count(), 1);
        assert!(waits.is_empty(), "a 404 does not wait");
    }

    #[test]
    fn cancelling_mid_body_stops_there_and_keeps_what_arrived() {
        // A cancelled download is one the user may want to resume, so the part
        // file stays: that is the whole difference between a cancel and a
        // failure being *reported* the same way but meaning different things.
        let dir = scratch("cancel");
        let dest = dir.join("a.jar");
        let download = Download::new(URL, &dest);
        let cancel = Cancel::new();
        let fetch = MapFetch::new().with_route(
            URL,
            Route::body(vec![9u8; 1000]).chunked(100).stopping_after(3, cancel.clone()),
        );
        let (result, waits) = fetch_now(&fetch, &download, &cancel);
        let error = result.expect_err("cancelled");
        assert!(matches!(error, Error::Cancelled), "{error:?}");
        assert!(waits.is_empty(), "a cancellation is not retried and never waits");
        assert_eq!(fetch.count(), 1);
        assert_eq!(
            std::fs::metadata(download.part()).map(|meta| meta.len()).expect("a part file"),
            300,
            "the three chunks that arrived were kept"
        );
        assert!(!dest.exists());
    }

    #[test]
    fn a_cancelled_download_resumes_where_it_stopped() {
        // The two rules together: the part file kept by a cancellation is
        // continued by the next run rather than started again.
        let dir = scratch("cancel-then-resume");
        let dest = dir.join("a.jar");
        let download = Download::new(URL, &dest);
        let body: Vec<u8> = (0..600u32).map(|n| n as u8).collect();
        let cancel = Cancel::new();
        let stopping = MapFetch::new().with_route(
            URL,
            Route::body(body.clone()).chunked(100).stopping_after(2, cancel.clone()),
        );
        let (result, _) = fetch_now(&stopping, &download, &cancel);
        assert!(result.is_err());
        assert_eq!(
            std::fs::metadata(download.part()).map(|meta| meta.len()).expect("a part file"),
            200
        );

        let resuming = MapFetch::new().with_route(URL, Route::body(body.clone()));
        let (result, _) = fetch_now(&resuming, &download, &Cancel::new());
        assert_eq!(result.expect("resumed"), Downloaded::Resumed(400));
        assert_eq!(std::fs::read(&dest).expect("the file"), body);
    }

    #[test]
    fn the_seed_is_stable_and_moves_between_urls() {
        assert_eq!(seed_of(URL), seed_of(URL));
        assert_ne!(seed_of(URL), seed_of("https://example.invalid/b.jar"));
        assert_ne!(seed_of("a"), seed_of("b"));
    }

    #[test]
    fn the_waits_of_two_different_files_do_not_all_land_together() {
        // Why the spread exists: a phase of eight downloads that failed at the
        // same moment must not come back at the same moment.
        let backoff = Backoff::with_attempts(3);
        let waits: std::collections::HashSet<u128> = (0..8)
            .filter_map(|n| {
                backoff
                    .delay_for(2, seed_of(&format!("https://example.invalid/{n}.jar")))
                    .map(|wait| wait.as_nanos())
            })
            .collect();
        assert!(waits.len() > 1, "every retry waited the same amount");
    }

    #[test]
    fn a_missing_version_of_the_destination_is_created_rather_than_refused() {
        let dir = scratch("nested");
        let dest = dir.join("libraries").join("org").join("ns").join("name").join("1.0.jar");
        let download = Download::new(URL, &dest);
        let fetch = MapFetch::new().with_route(URL, Route::body(b"jar".to_vec()));
        let (result, _) = fetch_now(&fetch, &download, &Cancel::new());
        assert_eq!(result.expect("fetched"), Downloaded::Fetched(3));
        assert_eq!(std::fs::read(&dest).expect("the file"), b"jar");
    }
}
