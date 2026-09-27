//! The wire: the one way this launcher's own code reaches the network.
//!
//! The engine in `palantir-net` owns the rules -- one client under one ceiling,
//! one retry policy, one cache, one queue -- and this is the desktop's handle on
//! it. It exists because the launch path had a second way out of its own: an
//! HTTP fetcher with a timeout and a thread pool of its own, which meant a
//! launch's downloads drew a ceiling that the interface's requests knew nothing
//! about, restarted from zero when a connection dropped, could not be stopped
//! from the window that started them, and were checked by a digest comparison
//! written here rather than by the transfer that wrote the file.
//!
//! Cheap to clone -- an `Arc`, a cache directory, a policy -- and it has to be:
//! the launch worker owns one for the length of a run.
//!
//! ## What a test passes
//!
//! [`Wire::over`] takes the engine's fetch seam, and the engine's own
//! `MapFetch` is the double to hand it. A launch is then exercised against the
//! queue, the retry policy and the digest check themselves, rather than against
//! stubs of them -- which is the difference between testing an install and
//! testing a description of one.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use palantir_net::engine::{
    next_event, Backoff, Cancel, Digest, Download, Event, Fetch, HttpPool, Job, JobId, LoaderMeta,
    MetadataCache, PistonMeta, Scheduler, DEFAULT_LIMIT, DEFAULT_TTL, DEFAULT_TIMEOUT,
};

/// One file to fetch, and the digest that says it arrived.
///
/// The digest is hex as its publisher wrote it, and of whichever of the three
/// kinds that publisher uses: Mojang addresses every library and asset object by
/// its `sha1`, Modrinth publishes a `sha256`. An empty digest is a file taken on
/// trust, which the engine still resumes -- it simply cannot be checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileJob {
    /// Where it comes from.
    pub url: String,
    /// Where it goes.
    pub dest: PathBuf,
    /// The hex digest it must have, or empty for a file taken on trust.
    pub digest: String,
}

impl FileJob {
    /// A file with the digest its publisher states.
    pub fn new(
        url: impl Into<String>,
        dest: impl Into<PathBuf>,
        digest: impl Into<String>,
    ) -> FileJob {
        FileJob {
            url: url.into(),
            dest: dest.into(),
            digest: digest.into().trim().to_ascii_lowercase(),
        }
    }
}

/// The engine, as the rest of this crate uses it.
///
/// No `Debug`: the fetch seam behind it is a trait object, and a derived
/// `Debug` would print the address of the pool rather than anything about it.
#[derive(Clone)]
pub struct Wire {
    /// The metadata cache, and the directory a test can read its answers from.
    cache: MetadataCache,
    /// The one client, under the process-wide ceiling.
    fetch: Arc<dyn Fetch>,
    /// The one retry policy.
    backoff: Backoff,
}

impl Wire {
    /// The launcher's own wire: one pool, and the cache directory the launcher
    /// already keeps its metadata in.
    ///
    /// The directory is the caller's rather than one chosen here, because a
    /// second cache is a second set of stale answers nobody knows about.
    pub fn new(cache_dir: impl Into<PathBuf>) -> Wire {
        Wire::over(cache_dir, Arc::new(HttpPool::new(DEFAULT_LIMIT, DEFAULT_TIMEOUT)))
    }

    /// The same wire over an explicit client and directory.
    pub fn over(cache_dir: impl Into<PathBuf>, fetch: Arc<dyn Fetch>) -> Wire {
        Wire {
            cache: MetadataCache::new(cache_dir, DEFAULT_TTL),
            fetch,
            backoff: Backoff::default(),
        }
    }

    /// The loaders' own metadata over this wire's cache and client.
    ///
    /// A handle rather than a thing owned: the launcher has one client and one
    /// cache directory, and the engine's sources are views of them. A caller that
    /// built its own would be a second client under a second ceiling, which is the
    /// thing the wire exists to stop.
    pub fn loaders(&self) -> LoaderMeta {
        LoaderMeta::new(self.cache.clone(), self.fetch.clone())
    }

    /// Mojang's own metadata over the same cache and client, for the same reason.
    pub fn piston(&self) -> PistonMeta {
        PistonMeta::new(self.cache.clone(), self.fetch.clone())
    }

    /// One document, through the cache: a request only when it has to.
    ///
    /// A `String` rather than bytes, because every caller of this parses JSON
    /// and a caller that has to remember to check for UTF-8 is a caller that
    /// will forget it once. The failure is a line for a log rather than an error
    /// type, for the same reason: it is shown to a user, not matched on.
    pub fn document(&self, url: &str) -> Result<String, String> {
        let cached = self
            .cache
            .get(url, &*self.fetch, &Cancel::new(), &self.backoff)
            .map_err(|error| format!("{error}"))?;
        String::from_utf8(cached.body).map_err(|_| format!("{url} is not text"))
    }

    /// Fetch every file over the engine's queue, reporting each one that lands.
    ///
    /// The results come back in the order the jobs were given, which is what
    /// lets a caller line them up against its own plan; the *reports* come in
    /// the order files finish, which is the order a progress bar should move in.
    /// `report` is handed the number of jobs over so far -- refusals included --
    /// and the bytes they carried, and it is called on this thread, so a caller
    /// needs no lock of its own.
    ///
    /// A digest that is not a digest fails the job that named it, before any
    /// request is made. A file named by a digest and then fetched without one is
    /// the single case where "it downloaded" and "it is the right file" are
    /// different sentences, and the hex here comes out of metadata that is a
    /// text file anyone can edit.
    pub fn files(
        &self,
        jobs: &[FileJob],
        workers: usize,
        report: &mut dyn FnMut(usize, u64),
    ) -> Vec<Result<u64, String>> {
        let mut results: Vec<Option<Result<u64, String>>> = vec![None; jobs.len()];
        let mut queued: Vec<(usize, Download)> = Vec::new();
        for (index, job) in jobs.iter().enumerate() {
            if job.digest.is_empty() {
                queued.push((index, Download::new(&job.url, &job.dest)));
                continue;
            }
            match Digest::parse(&job.digest) {
                Ok(digest) => queued.push((index, Download::checked(&job.url, &job.dest, digest))),
                Err(_) => results[index] = Some(Err(format!("{}: not a digest", job.dest.display()))),
            }
        }
        let refused = jobs.len() - queued.len();
        if refused > 0 {
            // Reported as over before anything starts: they are the first things
            // to finish, and a last-file report that could not fire because the
            // last job was refused would leave a bar short of its total.
            report(refused, 0);
        }
        if queued.is_empty() {
            return results.into_iter().map(|result| result.unwrap_or(Ok(0))).collect();
        }
        let (scheduler, events) =
            Scheduler::new(self.fetch.clone(), workers.max(1), self.backoff);
        let ids: Vec<(JobId, usize)> = queued
            .iter()
            .map(|(index, download)| (scheduler.submit(Job::new(download.clone())), *index))
            .collect();
        // Blocking waits are right here: the queue reports exactly once per job,
        // so a caller that needs every job's word can simply wait for it, and the
        // scheduler holds its own sender until it is dropped, so the channel
        // cannot close under this loop. A timeout would be a deadline on a
        // download, which is the one thing a launcher must not have.
        let mut over = refused;
        let mut bytes = 0u64;
        while over < jobs.len() {
            let Some(event) = next_event(&events, QUEUE_WAIT) else {
                // The queue went quiet without reporting everything: every job
                // still waiting gets that sentence rather than a hang.
                break;
            };
            let (id, result) = match event {
                Event::Finished { id, downloaded } => (id, Ok(downloaded.bytes())),
                Event::Failed { id, reason } => (id, Err(reason)),
                Event::Cancelled { id } => (id, Err("cancelled".to_string())),
                Event::Started { .. } | Event::Idle => continue,
            };
            let Some((_, index)) = ids.iter().find(|(queued_id, _)| *queued_id == id) else {
                continue;
            };
            if let Ok(carried) = &result {
                bytes += carried;
            }
            results[*index] = Some(result);
            over += 1;
            report(over, bytes);
        }
        results
            .into_iter()
            .map(|result| result.unwrap_or_else(|| Err("the queue stopped reporting".to_string())))
            .collect()
    }
}

/// How long the queue may say nothing before the caller stops believing it.
///
/// This is a hang guard rather than a deadline: the engine reports a job's start
/// and its end, so a quiet queue means either a large file arriving slowly or a
/// worker that died. Ten minutes of silence is not a slow line, it is a stop.
const QUEUE_WAIT: Duration = Duration::from_secs(600);

#[cfg(test)]
pub(crate) use script::Script;

#[cfg(test)]
mod tests {
    use std::fs;

    use palantir_net::engine::request::{MapFetch, Route};

    use super::*;

    /// A directory of this test's own, emptied first.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("palantirmc-wire-tests").join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        dir
    }

    #[test]
    fn a_file_that_is_already_there_is_not_asked_for_again() {
        // "Download only what is missing", through the desktop's own handle. The
        // digest on the job is what makes the answer possible at all: without one
        // there is nothing to compare the file against, so the engine fetches it
        // again rather than trusting whatever is on disk.
        let dir = scratch("already-there");
        let dest = dir.join("library.jar");
        fs::write(&dest, b"the bytes").expect("the file");
        let fetch = Arc::new(MapFetch::new());
        let wire = Wire::over(dir.join("cache"), fetch.clone());
        let jobs = [FileJob::new(
            "https://libraries.invalid/library.jar",
            &dest,
            crate::install::sha1_hex(b"the bytes"),
        )];
        let mut reports = Vec::new();
        let results = wire.files(&jobs, 2, &mut |done, bytes| reports.push((done, bytes)));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], Ok(0), "nothing was transferred");
        assert_eq!(fetch.count(), 0, "and nothing was asked for");
        assert_eq!(reports, vec![(1, 0)], "the job is still reported as over");
    }

    #[test]
    fn an_interrupted_file_is_continued_rather_than_fetched_again() {
        // The half of the engine's queue this crate never had before it: the part
        // file a killed launch left behind is a prefix of the body, so the next
        // attempt asks for the rest of it and the bytes that come off the network
        // are the remainder -- not the whole file a second time.
        let dir = scratch("resume");
        let dest = dir.join("asset");
        let body = vec![b'a'; 4096];
        let part = Download::new("https://cdn.invalid/asset", &dest).part();
        fs::write(&part, &body[..2048]).expect("the part file");
        let fetch = Arc::new(
            MapFetch::new().with_route("https://cdn.invalid/asset", Route::body(body.clone())),
        );
        let wire = Wire::over(dir.join("cache"), fetch.clone());
        let jobs = [FileJob::new("https://cdn.invalid/asset", &dest, "")];
        let results = wire.files(&jobs, 1, &mut |_, _| {});
        assert_eq!(results[0], Ok(2048), "the other half came off the network");
        assert_eq!(fs::read(&dest).expect("the file"), body, "and the file is whole");
        let requests = fetch.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].offset, Some(2048), "from where the part file stopped");
    }

    #[test]
    fn a_digest_that_is_not_a_digest_is_refused_without_a_request() {
        // The hex comes out of metadata anyone can edit, and a file that is named
        // by a digest and then fetched without one is the single case where "it
        // downloaded" and "it is the right file" are different sentences.
        let dir = scratch("bad-digest");
        let dest = dir.join("mod.jar");
        let fetch = Arc::new(MapFetch::new());
        let wire = Wire::over(dir.join("cache"), fetch.clone());
        let jobs = [FileJob::new("https://cdn.invalid/mod.jar", &dest, "not-a-digest")];
        let results = wire.files(&jobs, 1, &mut |_, _| {});
        assert!(results[0].is_err(), "{:?}", results[0]);
        assert_eq!(fetch.count(), 0, "the request was never made");
        assert!(!dest.exists(), "and no file was left behind");
    }
}

#[cfg(test)]
pub(crate) mod script {
    //! A scripted service for the tests that drive a launch.
    //!
    //! The engine's own [`MapFetch`] underneath -- so a phase runs against the
    //! real queue, the real retry policy and the real digest check rather than
    //! against stubs of them -- with the one convenience every caller here needs:
    //! a list of bodies to hand back, keyed by URL.
    //!
    //! [`MapFetch`]: palantir_net::engine::request::MapFetch

    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use palantir_net::engine::request::{MapFetch, Route};

    use super::Wire;

    /// Bodies by URL, and the wire that serves them.
    #[derive(Default)]
    pub(crate) struct Script {
        pages: Vec<(String, Vec<u8>)>,
    }

    impl Script {
        /// A service with nothing on it.
        pub(crate) fn new() -> Script {
            Script::default()
        }

        /// Publish `body` at `url`.
        pub(crate) fn insert(&mut self, url: impl Into<String>, body: impl Into<Vec<u8>>) {
            self.pages.push((url.into(), body.into()));
        }

        /// Publish `body` at `url`, for the callers that hold text.
        pub(crate) fn insert_str(&mut self, url: impl Into<String>, body: &str) {
            self.insert(url, body.as_bytes().to_vec());
        }

        /// The double itself, for a test that wants to ask what was requested.
        pub(crate) fn fetch(&self) -> MapFetch {
            let mut fetch = MapFetch::new();
            for (url, body) in &self.pages {
                fetch = fetch.with_route(url, Route::body(body.clone()));
            }
            fetch
        }

        /// A wire over this service, in a cache directory of its own.
        ///
        /// A directory per call rather than one shared by the process: the
        /// engine's cache is the thing being tested in half of these, and two
        /// tests writing one cache file would be testing each other.
        pub(crate) fn wire(&self) -> Wire {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let dir = std::env::temp_dir()
                .join("palantirmc-script-wire")
                .join(NEXT.fetch_add(1, Ordering::SeqCst).to_string());
            // Emptied first, and this is not tidiness: the index is a per-*run*
            // counter, so the nth call of one run lands on the directory the nth
            // call of the *previous* run left behind, and the engine's cache is a
            // directory of hash-named answers with a TTL measured in hours. Two
            // runs with a different test order therefore read each other's
            // answers -- measured: 20 of these directories held one URL's body,
            // two different bodies among them, and the java runtime digest gate
            // read the fixture from a run where the digests had matched.
            let _ = std::fs::remove_dir_all(&dir);
            Wire::over(dir, Arc::new(self.fetch()))
        }
    }
}
