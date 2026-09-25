//! What to do when a request fails: the second attempt, and when not to make one.
//!
//! A retry policy is two decisions and both of them are easy to get wrong in a
//! way nobody notices until the service is having a bad day:
//!
//! * **Which failures are worth another attempt.** Retrying a 404 is 404
//!   requests of the same answer; retrying no failure at all is a launcher that
//!   gives up on a dropped connection. [`is_retryable`] is that line, and it
//!   reads the status code, which is why [`crate::Error::Http`] carries one.
//! * **How long to wait before it.** Waiting the same amount every time
//!   synchronises a fleet of clients into a thundering herd against a server
//!   that just asked them to slow down; waiting a fixed multiple of the attempt
//!   count backs off only in the limit. [`Backoff`] doubles with a cap, and
//!   spreads the result so sixteen parallel downloads do not all come back at
//!   the same millisecond.
//!
//! The waiting itself is not done here. [`retry`] takes the sleeper, and a test
//! passes one that records instead of sleeping -- which is what makes "three
//! attempts, 250ms then 500ms" a thing that can be asserted rather than
//! experienced.

use std::time::Duration;

use crate::Error;

/// The default policy: three attempts, 250ms doubling to a cap of 8s.
///
/// Chosen from what the two services this talks to do rather than from taste.
/// `meta.prismlauncher.org` and `api.modrinth.com` both answer 5xx during a
/// deployment and both recover inside seconds; three attempts covers a restart,
/// and 250ms is short enough that a user watching a progress bar does not
/// notice and long enough that a service under load is not hit again
/// immediately. Past 8s the wait is bigger than the thing being downloaded.
pub const DEFAULT_ATTEMPTS: u32 = 3;
/// The first wait.
pub const DEFAULT_BASE: Duration = Duration::from_millis(250);
/// The factor each wait is multiplied by.
pub const DEFAULT_FACTOR: f32 = 2.0;
/// The longest wait a single retry may have.
pub const DEFAULT_CAP: Duration = Duration::from_secs(8);

/// How many times to try, and how long to wait between tries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Backoff {
    attempts: u32,
    base: Duration,
    factor: f32,
    cap: Duration,
}

impl Default for Backoff {
    fn default() -> Backoff {
        Backoff {
            attempts: DEFAULT_ATTEMPTS,
            base: DEFAULT_BASE,
            factor: DEFAULT_FACTOR,
            cap: DEFAULT_CAP,
        }
    }
}

impl Backoff {
    /// The default policy, as a `const`.
    pub const fn standard() -> Backoff {
        Backoff {
            attempts: DEFAULT_ATTEMPTS,
            base: DEFAULT_BASE,
            factor: DEFAULT_FACTOR,
            cap: DEFAULT_CAP,
        }
    }

    /// A policy with `attempts` tries -- at least one -- and the default waits.
    ///
    /// One attempt is not an error: it is the policy for a request that has to
    /// be answered once, such as a cached metadata read where a failure that
    /// clear is a failure the caller wants to see.
    pub fn with_attempts(attempts: u32) -> Backoff {
        Backoff { attempts: attempts.max(1), ..Backoff::standard() }
    }

    /// A policy with every number given.
    ///
    /// `factor` below 1 is clamped: a policy that waits *less* on each retry is
    /// not a backoff, and a caller that meant it can write the sequence by hand.
    pub fn new(attempts: u32, base: Duration, factor: f32, cap: Duration) -> Backoff {
        Backoff { attempts: attempts.max(1), base, factor: factor.max(1.0), cap }
    }

    /// How many attempts a call under this policy makes, the first one included.
    pub fn attempts(&self) -> u32 {
        self.attempts
    }

    /// How long to wait before attempt number `attempt`, counting the first as 1.
    ///
    /// Read as "the wait *before* the attempt", which is why the first is zero:
    /// it is the first try and there is nothing to wait behind. The first
    /// *retry* is attempt 2 and waits `base`, then `base * factor`, capped, and
    /// `None` past the last attempt the policy allows -- which is how a caller
    /// knows it is out of tries rather than out of patience.
    ///
    /// A caller's loop is therefore `delay(tries + 1)`: after attempt `n` fails,
    /// what it wants to know is how long until attempt `n + 1`.
    ///
    /// The arithmetic is in nanoseconds rather than seconds, because the obvious
    /// `as_secs_f32() * factor` does not survive a round trip: `0.1f32 * 2.0` is
    /// not `0.2f32`, and `Duration::from_secs_f32` of both gives 200000003ns and
    /// 200000000ns. A policy whose second wait is three nanoseconds longer than
    /// it says is a policy no test can pin, and this one is pinned per step.
    pub fn delay(&self, attempt: u32) -> Option<Duration> {
        if attempt == 0 || attempt > self.attempts {
            return None;
        }
        if attempt == 1 {
            return Some(Duration::ZERO);
        }
        let steps = attempt - 2;
        let base = self.base.as_nanos() as f64;
        let cap = self.cap.as_nanos() as f64;
        let wait = base * f64::from(self.factor).powi(steps as i32);
        if !wait.is_finite() || wait < 0.0 {
            return Some(self.cap);
        }
        Some(Duration::from_nanos(wait.min(cap) as u64))
    }

    /// The same wait, spread over the interval it belongs to.
    ///
    /// The point is not randomness, it is *spread*: sixteen parallel downloads
    /// that failed together must not come back together, and a server that
    /// answered 503 does not want them to. `seed` is the caller's -- a job id, a
    /// hash of the URL -- so that the same job retrying twice waits in the same
    /// places, and the value stays honest under a test.
    ///
    /// The window is half the wait: anything more makes the *median* retry later
    /// than the policy says, which is a policy nobody can reason about.
    pub fn delay_for(&self, attempt: u32, seed: u64) -> Option<Duration> {
        let wait = self.delay(attempt)?;
        let half = wait / 2;
        if half.is_zero() {
            return Some(wait);
        }
        Some(wait - half + spread(seed, attempt, half))
    }
}

/// A deterministic value in `[0, bound)`, from two integers.
///
/// This tree has no `rand` and does not need one: what is wanted is a spread,
/// not a distribution. The mixer is SplitMix64's finaliser, which is the
/// cheapest thing that passes the small statistical checks a retry spread
/// actually needs, and folding `attempt` in means two retries of the same job do
/// not wait in the same place.
fn spread(seed: u64, attempt: u32, bound: Duration) -> Duration {
    let mut z = seed
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(u64::from(attempt));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    let nanos = bound.as_nanos().max(1) as u64;
    Duration::from_nanos(z % nanos)
}

/// Whether another attempt could produce a different answer.
///
/// The line is drawn on the status code, because that is the one place the
/// server says whether it meant it:
///
/// * a 4xx is the server telling the client it is wrong -- not retryable --
///   with three exceptions that mean "later": 408 (request timeout), 425 (too
///   early) and 429 (too many requests);
/// * a 5xx is the server failing, which is what a second attempt is for;
/// * a transport failure with no status at all (timeout, connection reset, DNS)
///   is the connection failing, not the answer, so it is retryable;
/// * a hash mismatch means the bytes arrived and were wrong, and a mismatch is
///   retryable *once* is not a claim this function can make -- the attempt count
///   is the policy's business, so it says yes and the cap decides;
/// * a cancellation is the caller's own decision, and a retry would be the
///   engine deliberately ignoring it -- never retryable;
/// * a local file that could not be read or parsed will fail the same way in a
///   tenth of a second, and retrying it is a spinner in front of a real error.
pub fn is_retryable(error: &Error) -> bool {
    match error {
        Error::Http { status: Some(status), .. } => {
            matches!(*status, 408 | 425 | 429) || (500..600).contains(status)
        }
        Error::Http { status: None, .. } | Error::HashMismatch { .. } => true,
        Error::Cancelled | Error::Io { .. } | Error::Json { .. } | Error::Format { .. } => false,
    }
}

/// Run `attempt` under `backoff`, sleeping between tries through `sleep`.
///
/// `sleep` is taken rather than called because a test asserts the waits by
/// recording them; the real caller passes `std::thread::sleep`. A retryable
/// failure inside the attempt budget is retried, anything else is returned to
/// the caller as it came, so the last error a caller sees is always the real
/// one rather than a summary of the tries.
pub fn retry<T>(
    backoff: &Backoff,
    mut attempt: impl FnMut(u32) -> Result<T, Error>,
    sleep: &mut dyn FnMut(Duration),
) -> Result<T, Error> {
    let mut tries = 0;
    loop {
        tries += 1;
        match attempt(tries) {
            Ok(value) => return Ok(value),
            Err(error) => {
                // `is_retryable` first: a cancellation must not even reach the
                // delay, or cancelling a job during a backoff would park the
                // thread for the rest of the wait before admitting it was
                // cancelled.
                if !is_retryable(&error) {
                    return Err(error);
                }
                // The wait before the attempt that is *about* to be made.
                match backoff.delay(tries + 1) {
                    Some(wait) if !wait.is_zero() => sleep(wait),
                    Some(_) => {}
                    None => return Err(error),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn http(status: Option<u16>) -> Error {
        match status {
            Some(status) => Error::status("https://example.invalid/x", status),
            None => Error::http("https://example.invalid/x", "connection reset"),
        }
    }

    #[test]
    fn the_default_waits_double_and_are_waits_before_an_attempt() {
        let backoff = Backoff::standard();
        assert_eq!(backoff.attempts(), 3);
        assert_eq!(backoff.delay(1), Some(Duration::ZERO), "the first try is not a retry");
        assert_eq!(backoff.delay(2), Some(Duration::from_millis(250)), "before the first retry");
        assert_eq!(backoff.delay(3), Some(Duration::from_millis(500)), "before the second");
        // Three attempts is two retries, and a fourth attempt is out of policy
        // rather than a fourth wait.
        assert_eq!(backoff.delay(4), None);
        let waits: Vec<Duration> = (1..=5).filter_map(|n| backoff.delay(n)).collect();
        assert_eq!(
            waits,
            vec![Duration::ZERO, Duration::from_millis(250), Duration::from_millis(500)]
        );
    }

    #[test]
    fn a_longer_policy_doubles_and_then_caps() {
        let backoff = Backoff::new(8, Duration::from_millis(100), 2.0, Duration::from_secs(1));
        assert_eq!(backoff.delay(2), Some(Duration::from_millis(100)));
        assert_eq!(backoff.delay(3), Some(Duration::from_millis(200)));
        assert_eq!(backoff.delay(4), Some(Duration::from_millis(400)));
        assert_eq!(backoff.delay(5), Some(Duration::from_millis(800)));
        assert_eq!(backoff.delay(6), Some(Duration::from_secs(1)), "capped");
        assert_eq!(backoff.delay(7), Some(Duration::from_secs(1)), "still capped");
        assert_eq!(backoff.delay(8), Some(Duration::from_secs(1)), "the last attempt, still capped");
        assert_eq!(backoff.delay(9), None, "there is no ninth attempt");
    }

    #[test]
    fn a_zero_attempt_policy_is_one_attempt() {
        // A policy that tried nothing would report every request as failed
        // without making it, which is the worst possible reading of a setting.
        let backoff = Backoff::with_attempts(0);
        assert_eq!(backoff.attempts(), 1);
        assert_eq!(backoff.delay(1), Some(Duration::ZERO));
        assert_eq!(backoff.delay(2), None, "one attempt has no retry behind it");
    }

    #[test]
    fn a_factor_below_one_is_not_a_backoff() {
        let backoff = Backoff::new(4, Duration::from_millis(100), 0.5, Duration::from_secs(1));
        assert_eq!(backoff.delay(2), Some(Duration::from_millis(100)));
        assert_eq!(backoff.delay(3), Some(Duration::from_millis(100)), "flat, not shrinking");
    }

    #[test]
    fn the_spread_stays_inside_the_wait_and_moves_between_jobs() {
        let backoff = Backoff::new(4, Duration::from_millis(100), 2.0, Duration::from_secs(10));
        let full = backoff.delay(3).expect("a wait");
        let mut seen = std::collections::HashSet::new();
        for seed in 0..64u64 {
            let spread = backoff.delay_for(3, seed).expect("a wait");
            assert!(spread <= full, "{spread:?} is longer than {full:?}");
            assert!(spread >= full / 2, "{spread:?} is less than half of {full:?}");
            seen.insert(spread.as_nanos());
        }
        // Not a distribution check -- a spread check. Sixteen parallel
        // downloads must not all wait in the same place.
        assert!(seen.len() > 16, "only {} distinct waits", seen.len());
        // And the same job waits in the same place twice running, or a job that
        // retried three times would wander.
        assert_eq!(backoff.delay_for(3, 7), backoff.delay_for(3, 7));
        assert_ne!(backoff.delay_for(3, 7), backoff.delay_for(4, 7));
    }

    #[test]
    fn the_retryable_line_is_the_status_code() {
        // A server saying the client is wrong.
        for status in [400, 401, 403, 404, 410, 418] {
            assert!(!is_retryable(&http(Some(status))), "{status} is not worth a retry");
        }
        // The three 4xx that mean "later", and the 5xx that mean "not now".
        for status in [408, 425, 429, 500, 502, 503, 504, 599] {
            assert!(is_retryable(&http(Some(status))), "{status} is worth a retry");
        }
        // No status at all is a connection that failed, not an answer.
        assert!(is_retryable(&http(None)));
        // Bytes that arrived wrong.
        assert!(is_retryable(&Error::hash_mismatch("/tmp/a.jar", "aa", "bb")));
        // A cancellation is the caller's own decision.
        assert!(!is_retryable(&Error::Cancelled));
        // A local file will still be a local file in a tenth of a second.
        assert!(!is_retryable(&Error::Io {
            path: "/tmp/a.jar".into(),
            source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
        }));
        assert!(!is_retryable(&Error::json("/tmp/a.json", "expected value")));
        assert!(!is_retryable(&Error::format("/tmp/a.jar", "not a zip")));
    }

    #[test]
    fn a_retried_call_reports_the_waits_it_made() {
        let mut attempts = 0;
        let waits: RefCell<Vec<Duration>> = RefCell::new(Vec::new());
        let result = retry(
            &Backoff::standard(),
            |try_number| {
                attempts += 1;
                assert_eq!(try_number, attempts);
                if attempts < 3 {
                    return Err(http(Some(503)));
                }
                Ok("the third attempt")
            },
            &mut |wait| waits.borrow_mut().push(wait),
        );
        assert_eq!(result.expect("the third attempt"), "the third attempt");
        assert_eq!(attempts, 3);
        // Two failures, two waits: before the second attempt and before the
        // third, and neither of them before the first.
        assert_eq!(
            *waits.borrow(),
            vec![Duration::from_millis(250), Duration::from_millis(500)]
        );
    }

    #[test]
    fn a_call_that_is_out_of_attempts_returns_the_last_real_error() {
        let mut attempts = 0;
        let error = retry::<()>(
            &Backoff::with_attempts(3),
            |_| {
                attempts += 1;
                Err(http(Some(500)))
            },
            &mut |_| {},
        )
        .expect_err("every attempt failed");
        assert_eq!(attempts, 3);
        // Not a summary: the caller sees the failure that actually happened.
        assert!(matches!(error, Error::Http { status: Some(500), .. }), "{error:?}");
    }

    #[test]
    fn a_failure_that_will_not_change_is_not_retried_and_neither_is_a_cancellation() {
        let mut attempts = 0;
        let error = retry::<()>(
            &Backoff::standard(),
            |_| {
                attempts += 1;
                Err(http(Some(404)))
            },
            &mut |_| panic!("a 404 must not wait"),
        )
        .expect_err("a 404");
        assert_eq!(attempts, 1);
        assert!(matches!(error, Error::Http { status: Some(404), .. }));

        let mut attempts = 0;
        let error = retry::<()>(
            &Backoff::standard(),
            |_| {
                attempts += 1;
                Err(Error::Cancelled)
            },
            &mut |_| panic!("a cancelled job must not wait"),
        )
        .expect_err("cancelled");
        assert_eq!(attempts, 1);
        assert!(matches!(error, Error::Cancelled));
    }

    #[test]
    fn one_attempt_policy_does_not_wait_between_nothing() {
        let mut waits = 0;
        let error = retry::<()>(
            &Backoff::with_attempts(1),
            |_| Err(http(Some(503))),
            &mut |_| waits += 1,
        )
        .expect_err("one attempt");
        assert_eq!(waits, 0);
        assert!(matches!(error, Error::Http { status: Some(503), .. }));
    }
}
