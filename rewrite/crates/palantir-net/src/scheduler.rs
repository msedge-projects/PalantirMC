//! A global concurrency limit for transfers.
//!
//! An install is thousands of small files; a service that sees thousands of
//! parallel connections sees a flood and answers with throttling or a ban.
//! The limit is global rather than per-host because the work is one host at
//! a time (the download service, the library service) and the point is the
//! number of sockets in flight, wherever they go.
//!
//! Workers are plain threads: transfers are blocking (one client, pooled
//! connections), and the job set is known before the first byte moves.

use std::collections::VecDeque;
use std::sync::Mutex;

/// Eight in flight. High enough to cover a round trip's idle time on a
/// broadband link, low enough to read as polite traffic to the services;
/// if a service ever throttles us, this is the number to lower.
pub const DEFAULT_CONCURRENCY: usize = 8;

/// Runs jobs with a ceiling on how many run at once.
pub struct Scheduler {
    concurrency: usize,
}

impl Scheduler {
    /// A scheduler running at most `concurrency` jobs at once (at least 1).
    pub fn new(concurrency: usize) -> Self {
        Self {
            concurrency: concurrency.max(1),
        }
    }

    /// The default scheduler: [`DEFAULT_CONCURRENCY`] workers.
    pub fn with_default_limit() -> Self {
        Self::new(DEFAULT_CONCURRENCY)
    }

    /// How many jobs this scheduler will run at once.
    pub fn concurrency(&self) -> usize {
        self.concurrency
    }

    /// Run `work` over every item, at most `concurrency` at a time, with
    /// `results[i]` answering `items[i]` regardless of finish order.
    pub fn run<T, R, F>(&self, items: Vec<T>, work: F) -> Vec<R>
    where
        T: Send,
        R: Send,
        F: Fn(T) -> R + Send + Sync,
    {
        let queue = Mutex::new(VecDeque::from_iter(items.into_iter().enumerate()));
        let done: Mutex<Vec<(usize, R)>> = Mutex::new(Vec::new());

        std::thread::scope(|scope| {
            for _ in 0..self.concurrency {
                scope.spawn(|| {
                    loop {
                        let next = queue.lock().ok().and_then(|mut q| q.pop_front());
                        let Some((index, item)) = next else {
                            break;
                        };
                        let value = work(item);
                        if let Ok(mut done) = done.lock() {
                            done.push((index, value));
                        }
                    }
                });
            }
        });

        // Every item ran exactly once, so sorting by arrival index restores
        // the caller's order no matter which worker finished first.
        let mut done = done.into_inner().unwrap_or_default();
        done.sort_by_key(|(index, _)| *index);
        done.into_iter().map(|(_, value)| value).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn results_keep_item_order() {
        let scheduler = Scheduler::new(4);
        let out = scheduler.run(vec![3, 1, 2], |n| n * 10);
        assert_eq!(out, vec![30, 10, 20]);
    }

    #[test]
    fn never_more_than_the_limit_at_once() {
        let scheduler = Scheduler::new(3);
        let live = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let items: Vec<usize> = (0..24).collect();
        scheduler.run(items, |_| {
            let now = live.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(now, Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(5));
            live.fetch_sub(1, Ordering::SeqCst);
        });
        assert!(peak.load(Ordering::SeqCst) <= 3);
    }

    #[test]
    fn one_worker_runs_everything() {
        let scheduler = Scheduler::new(0); // clamped to 1
        assert_eq!(scheduler.concurrency(), 1);
        assert_eq!(scheduler.run(vec![1, 2, 3], |n| n + 1), vec![2, 3, 4]);
    }
}
