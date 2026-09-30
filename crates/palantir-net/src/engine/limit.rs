//! One ceiling for every request the process makes.
//!
//! The reference client's scheduler has a global concurrency limit: it is the
//! one number that decides how much of the line the launcher is allowed to use,
//! and it belongs to the process rather than to a phase. This launcher has had
//! that number twice over already, with two different meanings -- a bulk install
//! passed `threads` to a download pool, and nothing capped the metadata requests
//! made at the same time -- which is how a launcher ends up with eight jars and
//! a dozen metadata fetches in flight at once.
//!
//! [`Limit`] is that one number. It is a counting gate rather than a thread
//! pool: the caller still decides which thread does the work, and what is
//! bounded is how many of them may be *inside* a request. A [`Permit`] is the
//! slot, and dropping it gives the slot back, including when a transfer returns
//! early or panics.
//!
//! Nothing here is a `Semaphore` from another crate: this tree has no async
//! runtime and no room for one, and a `Mutex<usize>` with a `Condvar` is the
//! whole of what is needed.

use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

/// The gate itself, shared by every permit handed out.
#[derive(Debug)]
struct Gate {
    /// How many slots are free right now.
    free: Mutex<usize>,
    /// Signalled whenever a slot comes back.
    ready: Condvar,
}

/// A ceiling on how many callers may be inside a request at once.
///
/// `Clone` shares the ceiling, which is what makes it *global*: a pool cloned
/// into a second phase does not get a second allowance.
#[derive(Debug, Clone)]
pub struct Limit {
    capacity: usize,
    gate: std::sync::Arc<Gate>,
}

impl Limit {
    /// A ceiling of `capacity`, at least 1.
    ///
    /// Zero is clamped rather than honoured: a limit of zero is a deadlock, and
    /// a caller that computed it from a setting should get a working download
    /// rather than a hang. The clamp is here rather than at the call site so
    /// that no call site has to remember it.
    pub fn new(capacity: usize) -> Limit {
        Limit {
            capacity: capacity.max(1),
            gate: std::sync::Arc::new(Gate {
                free: Mutex::new(capacity.max(1)),
                ready: Condvar::new(),
            }),
        }
    }

    /// The number of slots this ceiling was built with.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// How many slots are free right now.
    ///
    /// A snapshot, and useful for a status line rather than for a decision: by
    /// the time a caller acts on it another thread may have taken one.
    pub fn available(&self) -> usize {
        *lock(&self.gate.free)
    }

    /// Wait for a slot, taking it and returning the permit that holds it.
    ///
    /// Blocks forever if the ceiling is full and no permit is ever dropped,
    /// which is what a bounded resource does; [`Limit::acquire_timeout`] is the
    /// version a caller can give up on.
    pub fn acquire(&self) -> Permit<'_> {
        let mut free = lock(&self.gate.free);
        while *free == 0 {
            free = self
                .gate
                .ready
                .wait(free)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        *free -= 1;
        Permit { gate: &self.gate }
    }

    /// Wait up to `wait` for a slot.
    ///
    /// `None` means the ceiling stayed full for the whole wait. A caller that
    /// gets `None` has not taken a slot and has nothing to give back.
    pub fn acquire_timeout(&self, wait: Duration) -> Option<Permit<'_>> {
        let deadline = Instant::now() + wait;
        let mut free = lock(&self.gate.free);
        while *free == 0 {
            let left = deadline.checked_duration_since(Instant::now())?;
            let (guard, timeout) = self
                .gate
                .ready
                .wait_timeout(free, left)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            free = guard;
            if timeout.timed_out() && *free == 0 {
                return None;
            }
        }
        *free -= 1;
        Some(Permit { gate: &self.gate })
    }
}

/// A slot in a [`Limit`], held until it is dropped.
///
/// The permit borrows the gate rather than the `Limit`, so a permit cannot
/// outlive the ceiling it came from -- which is the one way this could leak a
/// slot for good.
#[derive(Debug)]
pub struct Permit<'a> {
    gate: &'a Gate,
}

impl Permit<'_> {
    /// Give the slot back early.
    ///
    /// Dropping the permit does the same thing; this exists for the caller that
    /// wants to release before the end of a scope, which is what makes a permit
    /// usable as "this section of the work is the request".
    pub fn release(self) {
        // `Drop` is the only implementation of the release; letting the value
        // fall here is the release.
    }
}

impl Drop for Permit<'_> {
    fn drop(&mut self) {
        let mut free = lock(&self.gate.free);
        *free += 1;
        self.gate.ready.notify_one();
    }
}

/// Lock, ignoring poisoning.
///
/// Every critical section here is one arithmetic assignment on a `usize`: there
/// is no invariant a panicking thread could have left half-applied, so a
/// poisoned mutex is not a reason to stop a download. `unwrap` is denied in this
/// crate anyway, and `unwrap_or_else(into_inner)` is the honest spelling of
/// "carry on".
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Run `work` on `threads`, each holding a permit while it does, and report
    /// the most that were ever inside at once.
    fn peak_concurrency(limit: &Limit, threads: usize, hold: Duration) -> usize {
        let peak = Arc::new(AtomicUsize::new(0));
        let inside = Arc::new(AtomicUsize::new(0));
        std::thread::scope(|scope| {
            for _ in 0..threads {
                let peak = Arc::clone(&peak);
                let inside = Arc::clone(&inside);
                scope.spawn(move || {
                    // A generous wait: the assertion is an upper bound, and a
                    // test that failed because a loaded machine was slow would
                    // be a test that lied about the gate.
                    let Some(_permit) = limit.acquire_timeout(Duration::from_secs(30)) else {
                        return;
                    };
                    let now = inside.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    std::thread::sleep(hold);
                    inside.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });
        peak.load(Ordering::SeqCst)
    }

    #[test]
    fn no_more_than_the_capacity_are_ever_inside() {
        for capacity in [1usize, 2, 4, 8] {
            let limit = Limit::new(capacity);
            let peak = peak_concurrency(&limit, capacity * 3, Duration::from_millis(5));
            assert!(
                peak <= capacity,
                "{capacity} allowed, {peak} were inside at once"
            );
        }
    }

    #[test]
    fn the_capacity_really_is_used() {
        // The other half of the claim: a gate that let one caller through would
        // pass the test above and be a serialiser.
        let limit = Limit::new(4);
        let peak = peak_concurrency(&limit, 12, Duration::from_millis(20));
        assert_eq!(peak, 4, "four slots, {peak} used them");
    }

    #[test]
    fn a_permit_comes_back_when_it_is_dropped() {
        let limit = Limit::new(1);
        assert_eq!(limit.available(), 1);
        {
            let _first = limit.acquire();
            assert_eq!(limit.available(), 0);
            assert!(
                limit.acquire_timeout(Duration::from_millis(20)).is_none(),
                "the only slot is taken"
            );
        }
        assert_eq!(limit.available(), 1);
        let _second = limit.acquire();
    }

    #[test]
    fn a_permit_can_be_given_back_before_its_scope_ends() {
        let limit = Limit::new(1);
        let permit = limit.acquire();
        permit.release();
        assert_eq!(limit.available(), 1);
    }

    #[test]
    fn a_zero_capacity_is_clamped_rather_than_deadlocking() {
        // A limit computed from a setting is a limit that can be zero, and a
        // zero limit must be a slow download rather than a window that never
        // paints.
        let limit = Limit::new(0);
        assert_eq!(limit.capacity(), 1);
        let _permit = limit.acquire();
        assert_eq!(limit.available(), 0);
    }

    #[test]
    fn waiting_for_a_slot_gives_up() {
        let limit = Limit::new(1);
        let _held = limit.acquire();
        let started = Instant::now();
        assert!(limit.acquire_timeout(Duration::from_millis(30)).is_none());
        let waited = started.elapsed();
        assert!(waited >= Duration::from_millis(25), "gave up after {waited:?}");
        assert!(waited < Duration::from_secs(5), "waited {waited:?} to give up");
    }

    #[test]
    fn a_second_clone_is_the_same_ceiling() {
        // What makes it global rather than per-phase: cloning must not hand out
        // another allowance.
        let first = Limit::new(1);
        let second = first.clone();
        let _permit = first.acquire();
        assert_eq!(second.available(), 0);
        assert!(second.acquire_timeout(Duration::from_millis(10)).is_none());
    }
}
