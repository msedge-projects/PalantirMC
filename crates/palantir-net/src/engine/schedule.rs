//! A queue of work, a fixed set of workers, and a report for every job.
//!
//! The existing bulk downloader already spreads jobs over threads, and it does it
//! well: a shared counter, independent failures, a progress callback. What it
//! cannot do is anything a *user* can ask for. It runs to completion, on the
//! calling thread's terms, and a phase of 1200 files with no way to stop it is a
//! progress bar with no cancel button beside it -- which is the difference
//! between a launcher and a script.
//!
//! So this is the shape the interface needs:
//!
//! * **Jobs are submitted, not run.** [`Scheduler::submit`] returns an id and the
//!   work happens on a worker, so the caller is never the thread doing the I/O.
//! * **Every job reports.** [`Event`] carries `Started`, then exactly one of
//!   `Finished`, `Failed` or `Cancelled` -- one terminal event per job, including
//!   for the jobs still in the queue when the scheduler is shut down, which is
//!   what lets a page's progress list be a fold over the stream rather than a
//!   bookkeeping problem.
//! * **A job can be stopped.** [`Scheduler::cancel`] flips that job's
//!   [`Cancel`] token, and the transfer notices between chunks.
//! * **The queue knows when it is empty.** [`Event::Idle`] is sent once, when the
//!   last outstanding job reports, which is what a caller waits for instead of
//!   polling a counter.
//!
//! The workers do not own the ceiling: [`crate::engine::limit::Limit`] lives in
//! the pool and is shared with everything else the process is doing, so a phase
//! of downloads and a metadata fetch made at the same time draw from one budget
//! rather than two. What `workers` controls is how much of the engine this
//! scheduler is willing to occupy.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::engine::cancel::Cancel;
use crate::engine::download::{fetch_to_file, Download, Downloaded};
use crate::engine::request::Fetch;
use crate::engine::retry::Backoff;
use crate::Error;

/// Which job an event is about.
///
/// Assigned by the scheduler and counted up, rather than derived from the URL:
/// the same URL can be downloaded to two destinations (an instance's copy and a
/// shared one), and a caller looking jobs up by URL would see one of them.
pub type JobId = u64;

/// How many workers a scheduler starts when the caller does not say.
///
/// Four, and deliberately below the pool's own ceiling of eight: a scheduler is
/// not the only thing making requests, and a phase that occupied every slot
/// would leave the interface's own metadata waiting behind a bulk install.
pub const DEFAULT_WORKERS: usize = 4;

/// Something that happened to a job.
///
/// One `Started` and then exactly one of the three terminal events, per job. A
/// caller can therefore keep a map of id to state and delete an entry when it
/// ends, without ever wondering whether a job is still going to be reported.
#[derive(Debug, Clone)]
pub enum Event {
    /// A worker picked the job up and is about to ask for it.
    Started {
        /// Which job.
        id: JobId,
        /// What it is fetching.
        url: String,
        /// Where it is going.
        dest: PathBuf,
        /// How many bytes the part file already holds, so a progress line starts
        /// from what a previous run left rather than from zero.
        resumed_from: u64,
    },
    /// The job finished, and the file is in place.
    Finished {
        /// Which job.
        id: JobId,
        /// How it finished: fresh, continued, or already there.
        downloaded: Downloaded,
    },
    /// The job failed, after the policy's attempts.
    Failed {
        /// Which job.
        id: JobId,
        /// Why, as the reader will be shown it.
        reason: String,
    },
    /// The job was cancelled: by [`Scheduler::cancel`], by
    /// [`Scheduler::cancel_all`], or by the scheduler shutting down.
    Cancelled {
        /// Which job.
        id: JobId,
    },
    /// Every job submitted so far has reported. Sent once per quiet moment: a
    /// batch submitted from inside the handling of this event produces another,
    /// which is what makes installing a modpack's own dependencies off the back
    /// of it work.
    Idle,
}

/// Work handed to a scheduler.
#[derive(Debug, Clone)]
pub struct Job {
    /// The file to fetch.
    pub download: Download,
}

impl Job {
    /// A job fetching `download`.
    pub fn new(download: Download) -> Job {
        Job { download }
    }
}

/// What is on the queue: the job and the id it will be reported under.
///
/// The id travels *with* the job rather than being derived from the queue's
/// order. The order would work for a plain queue and stops working the moment
/// anything is retried or requeued, and a wrong id is a cancel that stops the
/// wrong file.
type Queued = (JobId, Job);

/// The scheduler's state, shared with its workers.
struct Shared {
    /// One token per outstanding job, so a caller can stop one.
    cancels: Mutex<HashMap<JobId, Cancel>>,
    /// Submitted and not yet reported.
    outstanding: AtomicUsize,
    /// The next id to hand out.
    next_id: AtomicU64,
    /// The receiver the caller reads, cloned into each worker.
    events: Sender<Event>,
    /// The policy every job is fetched under.
    backoff: Backoff,
}

impl Shared {
    /// Report a job as done: forget its token, and say when the queue is quiet.
    ///
    /// The count going to zero is the only thing that sends [`Event::Idle`], and
    /// it is checked after the decrement so that exactly one worker sends it.
    fn release(&self, id: JobId) {
        if let Ok(mut cancels) = self.cancels.lock() {
            cancels.remove(&id);
        }
        if self.outstanding.fetch_sub(1, Ordering::SeqCst) == 1 {
            let _ = self.events.send(Event::Idle);
        }
    }
}

/// A queue of fetches over a fixed set of workers.
///
/// Dropping the scheduler stops it: every outstanding job is cancelled, the
/// queue's sender is dropped, and the workers drain what is left -- which reports
/// the remaining jobs as cancelled without a request, because the first thing an
/// attempt does is look at its token. [`Scheduler::shutdown`] does the same thing
/// and waits, which is what a caller about to exit a process wants.
pub struct Scheduler {
    jobs: Option<Sender<Queued>>,
    shared: Arc<Shared>,
    workers: Vec<JoinHandle<()>>,
}

impl Scheduler {
    /// Start `workers` workers over `fetch`, reporting on the returned receiver.
    ///
    /// The receiver is the caller's; when it is dropped the workers keep going
    /// until the scheduler is dropped, because a send to a closed channel is not
    /// a reason to abandon a download that is half way through -- the file would
    /// be left as a part file with nobody to finish it. `workers` is clamped to
    /// at least one, for the same reason the pool clamps its ceiling: zero is a
    /// queue that never moves.
    pub fn new(
        fetch: Arc<dyn Fetch>,
        workers: usize,
        backoff: Backoff,
    ) -> (Scheduler, Receiver<Event>) {
        let (jobs_tx, jobs_rx) = mpsc::channel::<Queued>();
        let (events_tx, events_rx) = mpsc::channel::<Event>();
        let shared = Arc::new(Shared {
            cancels: Mutex::new(HashMap::new()),
            outstanding: AtomicUsize::new(0),
            next_id: AtomicU64::new(1),
            events: events_tx,
            backoff,
        });
        let queue = Arc::new(Mutex::new(jobs_rx));
        let handles = (0..workers.max(1))
            .map(|_| {
                let shared = Arc::clone(&shared);
                let queue = Arc::clone(&queue);
                let fetch = Arc::clone(&fetch);
                thread::spawn(move || run_worker(&shared, &queue, fetch.as_ref()))
            })
            .collect();
        (Scheduler { jobs: Some(jobs_tx), shared, workers: handles }, events_rx)
    }

    /// Queue a job, returning the id its events will carry.
    ///
    /// The count goes up before the job is queued rather than when a worker picks
    /// it up, so a burst of submits cannot race [`Event::Idle`]: the count is
    /// already non-zero by the time anything could report.
    pub fn submit(&self, job: Job) -> JobId {
        let id = self.shared.next_id.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut cancels) = self.shared.cancels.lock() {
            cancels.insert(id, Cancel::new());
        }
        self.shared.outstanding.fetch_add(1, Ordering::SeqCst);
        match &self.jobs {
            Some(jobs) => {
                if jobs.send((id, job)).is_err() {
                    self.refuse(id, "the scheduler is not running");
                }
            }
            None => self.refuse(id, "the scheduler has been shut down"),
        }
        id
    }

    /// Report a job that could not be queued at all.
    fn refuse(&self, id: JobId, reason: &str) {
        let _ = self.shared.events.send(Event::Failed { id, reason: reason.to_string() });
        self.shared.release(id);
    }

    /// Ask one job to stop.
    ///
    /// Returns whether the job was still known: `false` means it had already
    /// finished, failed or been cancelled. A job that has not been picked up yet
    /// is cancelled too -- its token is set before the worker sees it, and the
    /// first thing an attempt does is look -- which is what makes cancelling a
    /// queue of a thousand files take as long as the one in flight.
    pub fn cancel(&self, id: JobId) -> bool {
        let Ok(cancels) = self.shared.cancels.lock() else {
            return false;
        };
        match cancels.get(&id) {
            Some(token) => {
                token.cancel();
                true
            }
            None => false,
        }
    }

    /// Ask every outstanding job to stop.
    pub fn cancel_all(&self) {
        if let Ok(cancels) = self.shared.cancels.lock() {
            for token in cancels.values() {
                token.cancel();
            }
        }
    }

    /// How many jobs have been submitted and not yet reported.
    pub fn outstanding(&self) -> usize {
        self.shared.outstanding.load(Ordering::SeqCst)
    }

    /// How many workers this scheduler started with.
    pub fn workers(&self) -> usize {
        self.workers.len()
    }

    /// Stop accepting jobs, cancel what is outstanding and wait for the workers.
    ///
    /// Cancelling on the way out is deliberate: a shutdown that waited for a
    /// 300 MB download would be a window that will not close. What is already on
    /// disk stays, which is what makes the next run resume it, and every queued
    /// job still reports -- as cancelled -- so a caller folding events into a
    /// progress list is not left with entries that never end.
    pub fn shutdown(&mut self) {
        self.cancel_all();
        // Closing the queue is what ends the workers' loop, and the buffer is
        // drained before the `recv` fails: those jobs are taken, see a cancelled
        // token and report without a request.
        self.jobs = None;
        for handle in self.workers.drain(..) {
            let _ = handle.join();
        }
    }
}

impl Drop for Scheduler {
    fn drop(&mut self) {
        if !self.workers.is_empty() {
            self.shutdown();
        }
    }
}

/// Take jobs until the queue closes.
fn run_worker(shared: &Arc<Shared>, queue: &Arc<Mutex<Receiver<Queued>>>, fetch: &dyn Fetch) {
    loop {
        // The lock is held only for the `recv`, which is where a worker waits;
        // the work happens outside it. That serialises the *take* rather than the
        // work, which is the same trade `download_many_with_progress` makes with
        // its shared counter.
        let taken = {
            let Ok(queue) = queue.lock() else {
                return;
            };
            queue.recv()
        };
        let Ok((id, job)) = taken else {
            // Every sender is gone and the buffer is drained: nothing left.
            return;
        };
        let cancel = token_for(shared, id);
        // Reported before the attempt rather than after it starts, so a phase's
        // progress line exists from the first moment and can start from what a
        // previous run left on disk.
        let resumed_from =
            std::fs::metadata(job.download.part()).map(|meta| meta.len()).unwrap_or(0);
        let _ = shared.events.send(Event::Started {
            id,
            url: job.download.url.clone(),
            dest: job.download.dest.clone(),
            resumed_from,
        });
        let mut sleep = |wait: Duration| thread::sleep(wait);
        let report =
            match fetch_to_file(fetch, &job.download, &cancel, &shared.backoff, &mut sleep) {
                Ok(downloaded) => Event::Finished { id, downloaded },
                Err(Error::Cancelled) => Event::Cancelled { id },
                Err(error) => Event::Failed { id, reason: error.to_string() },
            };
        let _ = shared.events.send(report);
        shared.release(id);
    }
}

/// The cancellation token for a job, or a fresh one.
///
/// A missing token means the job was already reported, which cannot happen while
/// it is being taken; a job that cannot be cancelled is still a job rather than a
/// panic.
fn token_for(shared: &Arc<Shared>, id: JobId) -> Cancel {
    match shared.cancels.lock() {
        Ok(cancels) => cancels.get(&id).cloned().unwrap_or_default(),
        Err(_) => Cancel::default(),
    }
}

/// Wait for the next event, up to `wait`.
///
/// A free function rather than a method on the receiver so that "did the queue go
/// quiet" reads the same way in a test as in the interface, and so a caller can
/// hold the receiver without holding the scheduler.
pub fn next_event(events: &Receiver<Event>, wait: Duration) -> Option<Event> {
    match events.recv_timeout(wait) {
        Ok(event) => Some(event),
        Err(RecvTimeoutError::Timeout) | Err(RecvTimeoutError::Disconnected) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::request::{MapFetch, Route};
    use std::path::PathBuf;
    use std::sync::Arc;

    /// Every event until the queue goes quiet, or the test gives up.
    ///
    /// The timeout is generous on purpose: it is what stops a hung scheduler
    /// from hanging the suite, not a deadline anything is expected to meet.
    fn drain(events: &Receiver<Event>) -> Vec<Event> {
        let mut seen = Vec::new();
        while let Some(event) = next_event(events, Duration::from_secs(20)) {
            let idle = matches!(event, Event::Idle);
            seen.push(event);
            if idle {
                break;
            }
        }
        seen
    }

    fn started(events: &[Event]) -> Vec<JobId> {
        events
            .iter()
            .filter_map(|event| match event {
                Event::Started { id, .. } => Some(*id),
                _ => None,
            })
            .collect()
    }

    /// The terminal event of each job, as `(id, kind)`.
    ///
    /// The kinds are strings rather than an enum because the promise being
    /// asserted is "exactly one of these three, per job", and a fold over strings
    /// makes an unmatched fourth variant impossible to miss.
    fn terminal(events: &[Event]) -> Vec<(JobId, &'static str)> {
        events
            .iter()
            .filter_map(|event| match event {
                Event::Finished { id, .. } => Some((*id, "finished")),
                Event::Failed { id, .. } => Some((*id, "failed")),
                Event::Cancelled { id } => Some((*id, "cancelled")),
                _ => None,
            })
            .collect()
    }

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join("palantirmc-engine-scheduler").join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a scratch directory");
        root
    }

    fn job(url: &str, dest: &std::path::Path) -> Job {
        Job::new(Download::new(url, dest))
    }

    #[test]
    fn every_job_reports_once_and_then_the_queue_is_quiet() {
        let dir = scratch("reports");
        let fetch = Arc::new(
            MapFetch::new()
                .with_route("u1", Route::text("one"))
                .with_route("u2", Route::text("two"))
                .with_route("u3", Route::text("three")),
        );
        let (scheduler, events) = Scheduler::new(fetch.clone(), 3, Backoff::with_attempts(1));
        let ids: Vec<JobId> = ["u1", "u2", "u3"]
            .iter()
            .map(|url| scheduler.submit(job(url, &dir.join(format!("{url}.bin")))))
            .collect();
        assert_eq!(scheduler.outstanding(), 3);

        let seen = drain(&events);
        assert_eq!(started(&seen).len(), 3);
        let ends = terminal(&seen);
        assert_eq!(ends.len(), 3, "one terminal event per job: {seen:?}");
        for id in &ids {
            assert_eq!(
                ends.iter().filter(|(ended, _)| ended == id).count(),
                1,
                "job {id} reported {} times",
                ends.iter().filter(|(ended, _)| ended == id).count()
            );
        }
        assert!(ends.iter().all(|(_, kind)| *kind == "finished"));
        assert!(matches!(seen.last(), Some(Event::Idle)), "{seen:?}");
        assert_eq!(scheduler.outstanding(), 0);
        for name in ["u1.bin", "u2.bin", "u3.bin"] {
            assert!(dir.join(name).is_file(), "{name} is not there");
        }
    }

    #[test]
    fn a_job_that_cannot_be_fetched_says_why_and_does_not_stop_the_others() {
        let dir = scratch("failure");
        let fetch = Arc::new(MapFetch::new().with_route("good", Route::text("ok")));
        let (scheduler, events) = Scheduler::new(fetch, 2, Backoff::with_attempts(1));
        let bad = scheduler.submit(job("https://example.invalid/gone", &dir.join("gone.bin")));
        let good = scheduler.submit(job("good", &dir.join("good.bin")));

        let seen = drain(&events);
        let ends = terminal(&seen);
        assert_eq!(ends.len(), 2);
        let (kind, reason) = seen
            .iter()
            .find_map(|event| match event {
                Event::Failed { id, reason } if *id == bad => Some(("failed", reason.clone())),
                _ => None,
            })
            .expect("the missing file failed");
        assert_eq!(kind, "failed");
        assert!(reason.contains("404"), "{reason}");
        assert!(ends.contains(&(good, "finished")));
        assert!(dir.join("good.bin").is_file());
        assert!(!dir.join("gone.bin").exists());
    }

    #[test]
    fn a_job_the_user_stops_reports_cancelled_and_leaves_its_part_file() {
        // One worker, held by a slow job, so the second is provably still in the
        // queue when it is cancelled -- which is the case that decides whether
        // cancelling a thousand queued files works or merely looks like it does.
        let dir = scratch("cancel-queued");
        let fetch = Arc::new(
            MapFetch::new()
                .with_route("slow", Route::text("slow").pausing(Duration::from_millis(200)))
                .with_route("queued", Route::body(vec![7u8; 1000])),
        );
        let (scheduler, events) = Scheduler::new(fetch.clone(), 1, Backoff::with_attempts(1));
        scheduler.submit(job("slow", &dir.join("slow.bin")));
        let waiting = scheduler.submit(job("queued", &dir.join("queued.bin")));
        assert!(scheduler.cancel(waiting), "the job is still known");
        assert!(!scheduler.cancel(waiting + 100), "an id that was never submitted");

        let seen = drain(&events);
        assert!(
            terminal(&seen).contains(&(waiting, "cancelled")),
            "the queued job was cancelled: {seen:?}"
        );
        assert!(!dir.join("queued.bin").exists());
        assert!(!dir.join("queued.bin.part").exists(), "it never started");
        // And the slow one still finished, because cancelling one job is not
        // cancelling the phase.
        assert!(dir.join("slow.bin").is_file());
    }

    #[test]
    fn cancelling_everything_stops_the_whole_phase() {
        let dir = scratch("cancel-all");
        let fetch = Arc::new(MapFetch::new().with_route(
            "u",
            Route::body(vec![1u8; 4096]).chunked(512).pausing(Duration::from_millis(50)),
        ));
        let (scheduler, events) = Scheduler::new(fetch, 2, Backoff::with_attempts(1));
        let ids: Vec<JobId> = (0..6)
            .map(|n| scheduler.submit(job("u", &dir.join(format!("{n}.bin")))))
            .collect();
        scheduler.cancel_all();

        let seen = drain(&events);
        let ends = terminal(&seen);
        assert_eq!(ends.len(), 6, "every job reported: {seen:?}");
        assert!(
            ends.iter().all(|(_, kind)| *kind == "cancelled"),
            "nothing finished: {ends:?}"
        );
        for id in ids {
            assert!(ends.iter().any(|(ended, _)| *ended == id), "job {id} never reported");
        }
        assert_eq!(scheduler.outstanding(), 0);
    }

    #[test]
    fn shutting_down_cancels_the_queue_and_still_reports_every_job() {
        // The promise a progress list depends on: a page folding these events
        // must not be left with an entry that never ends, even when the user
        // closed the window.
        let dir = scratch("shutdown");
        let fetch = Arc::new(MapFetch::new().with_route(
            "u",
            Route::body(vec![2u8; 8192]).pausing(Duration::from_millis(10)),
        ));
        let (mut scheduler, events) = Scheduler::new(fetch, 2, Backoff::with_attempts(1));
        let submitted: Vec<JobId> = (0..8)
            .map(|n| scheduler.submit(job("u", &dir.join(format!("{n}.bin")))))
            .collect();
        // The shutdown happens before the drain rather than after it, which is
        // the ordering a window that is closing produces. `shutdown` joins the
        // workers, so by the time it returns every event is already in the
        // channel's buffer -- nothing has to be read *during* it to be seen.
        scheduler.shutdown();

        let seen = drain(&events);
        let ends = terminal(&seen);
        assert_eq!(ends.len(), 8, "every job reported: {seen:?}");
        for id in &submitted {
            assert!(
                ends.iter().any(|(ended, _)| ended == id),
                "job {id} was reported as nothing at all"
            );
        }
        assert_eq!(scheduler.outstanding(), 0);
        assert!(
            seen.iter().any(|event| matches!(event, Event::Idle)),
            "the queue went quiet: {seen:?}"
        );
    }

    #[test]
    fn a_batch_submitted_after_the_queue_goes_quiet_is_reported_as_well() {
        // The reason `Idle` is sent per quiet moment rather than once: a modpack
        // install submits the pack's own dependencies from inside the handling of
        // the pack's event, and that second batch has to be waitable-for the same
        // way the first was.
        let dir = scratch("second-batch");
        let fetch = Arc::new(MapFetch::new().with_route("u", Route::text("bytes")));
        let (scheduler, events) = Scheduler::new(fetch, 2, Backoff::with_attempts(1));
        scheduler.submit(job("u", &dir.join("first.bin")));

        let first = drain(&events);
        assert_eq!(terminal(&first).len(), 1);

        let later: Vec<JobId> = (0..3)
            .map(|n| scheduler.submit(job("u", &dir.join(format!("later-{n}.bin")))))
            .collect();
        let second = drain(&events);
        let ends = terminal(&second);
        assert_eq!(ends.len(), 3, "the second batch reported: {second:?}");
        for id in later {
            assert!(ends.contains(&(id, "finished")), "job {id}");
        }
        assert!(
            matches!(second.last(), Some(Event::Idle)),
            "the second quiet moment was announced too: {second:?}"
        );
    }

    #[test]
    fn a_job_submitted_after_shutdown_is_refused_rather_than_dropped() {
        // A refusal still has to be an event. A caller that folded ids into a
        // progress list would otherwise wait forever on a job that was never
        // going to be attempted.
        let dir = scratch("refused");
        let fetch = Arc::new(MapFetch::new().with_route("u", Route::text("bytes")));
        let (mut scheduler, events) = Scheduler::new(fetch, 1, Backoff::with_attempts(1));
        scheduler.shutdown();

        let late = scheduler.submit(job("u", &dir.join("late.bin")));
        let seen = drain(&events);
        let reason = seen
            .iter()
            .find_map(|event| match event {
                Event::Failed { id, reason } if *id == late => Some(reason.clone()),
                _ => None,
            })
            .expect("the refused job was reported");
        assert!(reason.contains("shut down"), "{reason}");
        assert!(!dir.join("late.bin").exists());
        assert_eq!(scheduler.outstanding(), 0);
    }
}
