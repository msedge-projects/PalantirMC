//! The engine: one client, one ceiling, one policy for what to do when a
//! request fails.
//!
//! Stage 4 of the rewrite spec. Everything that touches the network goes through
//! here, and the reason it is a module rather than a habit is that the launcher
//! has had the alternative: a `Client` built per call, a thread count that meant
//! something different in each phase, a download that started again from zero,
//! retries scattered across call sites that each decided differently, and no way
//! at all to stop a transfer that was already running.
//!
//! The pieces, and what each of them is for:
//!
//! | Module | What it decides |
//! | --- | --- |
//! | [`limit`] | How many requests may be in flight at once, for the whole process |
//! | [`retry`] | Which failures get another attempt, and how long before it |
//! | [`cancel`] | How a running transfer is stopped, mid-body, in milliseconds |
//! | [`request`] | What a request is, and the seam every rule above is tested through |
//! | [`http`] | The one `reqwest` client, the `Range` header, and the honest answer when a range is ignored |
//! | [`download`] | One file: resumed if it can be, restarted if it must be, verified before it is done |
//!
//! ## Why the rules are here and not at the call sites
//!
//! A retry policy applied per call site is a policy that differs per call site,
//! and the differences are invisible until a service has a bad afternoon. The
//! same argument holds for the ceiling: the only number a service can be fair
//! about is a global one. So the engine owns both, and the call sites pass the
//! things they know (a URL, a destination, a digest) rather than the things the
//! engine decides (how many tries, how long to wait, how many at once).
//!
//! ## Testing without a network
//!
//! `reqwest` cannot be reached from a unit test, so every rule that has to hold
//! is stated against [`request::Fetch`] and exercised with
//! [`request::MapFetch`], which is a server that can be told to honour an offset
//! or ignore it, to fail twice with a 503, and to cancel a transfer three chunks
//! in. That last one is the reason the double exists at all: a cancellation that
//! lands *inside* a body is the case a pre-set flag cannot reach, and it is the
//! case the loop's check is for.
//!
//! What the double cannot say is that `reqwest` behaves the way it is assumed
//! to -- that a `Range` request really comes back 206, that `read` really
//! returns zero at the end. That is `tests/live.rs`'s job, and it is a
//! `#[ignore]`d test by design: a network that is down is not a passing test.

pub mod cancel;
pub mod download;
pub mod http;
pub mod limit;
pub mod request;
pub mod retry;

pub use cancel::Cancel;
pub use download::{fetch_to_file, Download, Downloaded};
pub use http::{HttpPool, DEFAULT_LIMIT, DEFAULT_TIMEOUT, USER_AGENT};
pub use limit::{Limit, Permit};
pub use request::{Fetch, Outcome, Request};
pub use retry::{is_retryable, Backoff};
