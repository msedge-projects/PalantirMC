//! A token that stops work somebody else started.
//!
//! Every long transfer in this engine is a loop over chunks: bytes arrive from a
//! socket, and something has to decide, between one chunk and the next, whether
//! to keep going. That something is a flag one thread sets and another reads,
//! and [`Cancel`] is it -- a single `Arc<AtomicBool>` that is cheap to clone into
//! as many threads as look at it.
//!
//! Two decisions worth knowing:
//!
//! * **`Relaxed`, not `SeqCst`.** The flag guards no other data -- a cancelled
//!   transfer publishes nothing and reads nothing that another thread could see
//!   half-written -- so no ordering is needed between the store and the load.
//!   What is needed is that the store becomes visible, which `Relaxed` on the
//!   same atomic already gives.
//! * **A cancelled transfer is an error, not an `Option`.** The callers are all
//!   returning `Result` for other reasons, and a worker that treated "cancelled"
//!   as "finished with nothing" would report a half-written file as a success.
//!   [`crate::Error::Cancelled`] cannot be mistaken for a byte count.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::Error;

/// A flag one thread sets and others read.
///
/// `Clone` shares the flag rather than copying it: a token cloned into a worker
/// is the same token, which is the whole point.
#[derive(Debug, Clone, Default)]
pub struct Cancel {
    flag: Arc<AtomicBool>,
}

impl Cancel {
    /// A token that has not been cancelled.
    pub fn new() -> Cancel {
        Cancel::default()
    }

    /// Ask everything watching this token to stop.
    ///
    /// Idempotent, and irreversible: there is no "uncancel", because a transfer
    /// that stopped half way has a local file to clean up rather than a state to
    /// return to.
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::Relaxed);
    }

    /// Whether [`Cancel::cancel`] has been called on any clone of this token.
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }

    /// `Ok(())` until the token is cancelled, then [`Error::Cancelled`].
    ///
    /// The shape a transfer wants: it has other reasons to return an error, and
    /// `?` on this puts the cancellation in the same channel as the rest.
    pub fn check(&self) -> Result<(), Error> {
        if self.is_cancelled() {
            return Err(Error::Cancelled);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_token_is_not_cancelled() {
        let cancel = Cancel::new();
        assert!(!cancel.is_cancelled());
        assert!(cancel.check().is_ok());
    }

    #[test]
    fn cancelling_is_visible_through_every_clone() {
        let cancel = Cancel::new();
        let worker = cancel.clone();
        let other = worker.clone();
        cancel.cancel();
        assert!(worker.is_cancelled());
        assert!(other.is_cancelled());
        assert!(cancel.check().is_err());
        // And it stays cancelled: a second call is not a toggle.
        cancel.cancel();
        assert!(worker.is_cancelled());
    }

    #[test]
    fn a_token_set_from_another_thread_stops_this_one() {
        // The real shape: the thread doing the transfer polls, the thread that
        // owns the window sets the flag. One word of sharing, no channel.
        let cancel = Cancel::new();
        let flag = cancel.clone();
        let setter = std::thread::spawn(move || flag.cancel());
        let mut stopped = false;
        for _ in 0..10_000_000 {
            if cancel.check().is_err() {
                stopped = true;
                break;
            }
            std::hint::spin_loop();
        }
        setter.join().expect("the setter thread");
        assert!(stopped, "the poller never saw the flag");
    }

    #[test]
    fn a_cancellation_is_the_crates_own_error_and_says_nothing_else() {
        let cancel = Cancel::new();
        cancel.cancel();
        let error = cancel.check().expect_err("a cancelled token stops");
        assert!(matches!(error, Error::Cancelled));
        assert!(!crate::engine::retry::is_retryable(&error));
    }
}
