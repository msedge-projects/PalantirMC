//! Keep the machine awake for the work a reader is waiting on.
//!
//! A launcher has two slow moments, and both look like idleness to Windows: an
//! install is hundreds of megabytes arriving over a home connection, and a first
//! launch downloads its assets before any game window appears. Neither produces
//! input for minutes at a time, so a laptop on its default power plan suspends in
//! the middle of one -- and a suspended machine takes the download's process with
//! it, which is how a 900 MB install dies at 80%.
//!
//! [`Awake`] is the request that stops that. It is a guard rather than a pair of
//! calls because `SetThreadExecutionState` is **per thread** and `ES_CONTINUOUS`
//! holds the request until the same thread changes it or exits: the request
//! therefore lives in the worker thread doing the work, and dropping the guard
//! clears it. That includes the paths a hand-written `release()` at the end of
//! the function would miss -- an early return, a `?`, a panic.
//!
//! **Displays are not kept on.** `ES_DISPLAY_REQUIRED` would hold the panel lit
//! for the whole of a multi-gigabyte install, and burning a laptop's screen to
//! finish a download trades one annoyance for a worse one. The system stays up;
//! the screen is the screen's business.
//!
//! **Nesting is counted.** A launch runs its own install on the same worker
//! thread, so two guards are held at once there. `ES_CONTINUOUS` on the second
//! release would clear the first request outright, so the thread keeps a depth
//! and only the outermost guard talks to the OS.
//!
//! Off Windows there is nothing to ask -- the Linux and macOS builds here exist
//! to run tests and the build VM -- so [`Awake`] still exists and does nothing,
//! which is what lets every call site be unconditional.

use std::cell::Cell;

/// `ES_CONTINUOUS`: the request stays in force until this thread changes it.
///
/// The same value releases it, which is why [`RELEASE`] is this flag alone: it
/// asks Windows to go back to the reader's own power policy.
const ES_CONTINUOUS: u32 = 0x8000_0000;
/// `ES_SYSTEM_REQUIRED`: keep the machine out of idle sleep.
const ES_SYSTEM_REQUIRED: u32 = 0x0000_0001;
/// `ES_DISPLAY_REQUIRED`: keep the *screen* on. Named only so the tests can say
/// it is deliberately absent from [`KEEP_AWAKE`].
#[cfg(test)]
const ES_DISPLAY_REQUIRED: u32 = 0x0000_0002;

/// What an [`Awake`] asks for: continuous, and the system must stay up.
pub const KEEP_AWAKE: u32 = ES_CONTINUOUS | ES_SYSTEM_REQUIRED;

/// What dropping the last [`Awake`] asks for: this thread's request, cleared.
pub const RELEASE: u32 = ES_CONTINUOUS;

thread_local! {
    /// How many guards this thread is holding. Only the first talks to the OS.
    static DEPTH: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
thread_local! {
    /// Every request this thread made, in order -- see [`record`].
    static CALLS: std::cell::RefCell<Vec<u32>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Keeping the system awake, in force until this value is dropped.
///
/// See the module documentation for why this is a guard and why the count is
/// per thread rather than a flag on a struct somewhere.
#[derive(Debug)]
pub struct Awake {
    /// Whether this guard is the one that asked the OS, which is the only one
    /// allowed to release it again.
    holds: bool,
}

impl Awake {
    /// Ask the OS to keep the machine awake until this value is dropped.
    ///
    /// A request the OS refuses is not an error -- the machine may then sleep,
    /// which is where it was before this was called -- so the failure is silent
    /// and the guard still releases what it asked for.
    pub fn begin() -> Awake {
        let holds = DEPTH.with(|depth| {
            let held = depth.get();
            if held == 0 {
                set_state(KEEP_AWAKE);
            }
            depth.set(held + 1);
            held == 0
        });
        Awake { holds }
    }
}

impl Drop for Awake {
    fn drop(&mut self) {
        DEPTH.with(|depth| {
            depth.set(depth.get().saturating_sub(1));
            if self.holds && depth.get() == 0 {
                set_state(RELEASE);
            }
        });
    }
}

/// Ask Windows, or do nothing where there is no such request to make.
fn set_state(flags: u32) -> bool {
    #[cfg(test)]
    record(flags);
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Power::SetThreadExecutionState;
        // The call takes a bit flag and is documented as safe from any thread;
        // the `unsafe` is the FFI declaration's, not this call's.
        unsafe { SetThreadExecutionState(flags) != 0 }
    }
    #[cfg(not(windows))]
    {
        let _ = flags;
        true
    }
}

/// Every request this *thread* made, in order.
///
/// A test-only recorder rather than a trait seam: the interesting behaviour is
/// *when* the OS is asked -- once per thread, at the outermost guard -- and a
/// seam would end up testing the seam. Thread-local rather than shared because
/// the test harness runs the gates below on threads of their own, and a buffer
/// they shared would interleave the three sequences into one unreadable list.
#[cfg(test)]
fn record(flags: u32) {
    CALLS.with(|calls| calls.borrow_mut().push(flags));
}

#[cfg(test)]
fn recorded() -> Vec<u32> {
    CALLS.with(|calls| calls.borrow().clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two halves of the request, spelled out: the system is required, the
    /// display is not, and the release is the continuous flag alone.
    #[test]
    fn the_request_is_the_system_and_not_the_display() {
        assert_eq!(KEEP_AWAKE & ES_CONTINUOUS, ES_CONTINUOUS);
        assert_eq!(KEEP_AWAKE & ES_SYSTEM_REQUIRED, ES_SYSTEM_REQUIRED);
        assert_eq!(
            KEEP_AWAKE & ES_DISPLAY_REQUIRED,
            0,
            "the display flag would hold the panel lit for a whole install"
        );
        assert_eq!(RELEASE, ES_CONTINUOUS, "releasing is the continuous flag");
    }

    /// One guard asks once, and dropping it releases once.
    #[test]
    fn one_guard_asks_once_and_releases_once() {
        let before = recorded().len();
        {
            let _awake = Awake::begin();
            assert_eq!(
                recorded().as_slice(),
                &[KEEP_AWAKE],
                "nothing was asked of the OS"
            );
        }
        assert_eq!(recorded().as_slice(), &[KEEP_AWAKE, RELEASE]);
        assert_eq!(recorded().len(), before + 2);
    }

    /// A launch installs its own files while the launch's own guard is held, so
    /// the second guard must not ask twice and must not release the first
    /// request when it goes away.
    #[test]
    fn nested_guards_ask_once_and_release_after_the_last_one() {
        let before = recorded().len();
        {
            let outer = Awake::begin();
            {
                let _inner = Awake::begin();
                assert_eq!(
                    recorded().len(),
                    before + 1,
                    "the inner guard asked the OS again: {:?}",
                    recorded()
                );
            }
            assert_eq!(
                recorded().last().copied(),
                Some(KEEP_AWAKE),
                "the inner guard's drop released the outer request"
            );
            drop(outer);
        }
        assert_eq!(recorded().last().copied(), Some(RELEASE));
    }

    /// The guard only means anything where the work is, and neither of those two
    /// places can be run here: an install needs a wire and a plan, a launch needs
    /// a game. So the two function bodies are read out of the tree and held to
    /// the line that has to be in them.
    ///
    /// The body is taken from the signature to the next `pub fn` at the start of
    /// a line, which is the next item: none of these functions defines another.
    #[test]
    fn the_work_that_must_not_be_interrupted_holds_a_guard() {
        for (source, signature) in [
            (include_str!("install.rs"), "pub fn run("),
            (include_str!("launch.rs"), "pub fn run_launch_worker("),
        ] {
            let body = source
                .split(signature)
                .nth(1)
                .unwrap_or_else(|| panic!("{signature} is gone from its module"));
            let body = body.split("\npub fn ").next().unwrap_or(body);
            assert!(
                body.contains("crate::awake::Awake::begin()"),
                "{signature} no longer holds the machine awake"
            );
        }
    }
}
