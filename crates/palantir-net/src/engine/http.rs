//! The one client every request goes through.
//!
//! Two things are being pooled here and they are not the same thing:
//!
//! * **One `reqwest::blocking::Client`.** A client owns the connection pool, the
//!   TLS session cache and the cookie jar; building one per request, which is
//!   what a `BlockingHttpFetcher::new` does, throws away the connection every
//!   time. Every request in this launcher now shares one handle.
//! * **One ceiling on how many are in flight.** [`Limit`] is the process-wide
//!   number: eight library jars and a metadata fetch at the same time is eleven
//!   connections, and the ceiling is what keeps it at the number the setting
//!   says.
//!
//! What this adds over the existing `BlockingHttpFetcher` is the part a bulk
//! download needs and a metadata read does not: a `Range` request that is
//! *honest about being ignored*, and a cancellation check between chunks rather
//! than only before the request. Nothing in this file is unit-tested against a
//! real socket -- `MapFetch` is where the offset contract is tested, because
//! that contract is the engine's and not `reqwest`'s -- and the live tests in
//! `tests/live.rs` are what say the two agree.

use std::io::{Read, Write};
use std::time::Duration;

use crate::engine::cancel::Cancel;
use crate::engine::limit::{Limit, Permit};
use crate::engine::request::{Fetch, Outcome, Request, CHUNK};
use crate::Error;

/// The name this launcher gives itself, as a `User-Agent`.
///
/// Modrinth's API guidelines ask clients to identify themselves, and a bare
/// `reqwest` default is what a service rate-limits first. `browse.rs` in the
/// desktop crate builds the same string; it cannot be imported from here --
/// the dependency runs the other way -- and the version is the workspace's, so
/// the two agree by construction rather than by a promise.
pub const USER_AGENT: &str = concat!("PalantirMC/", env!("CARGO_PKG_VERSION"));

/// Default per-request timeout.
///
/// The same 30s the metadata store uses. A launcher's requests are small files
/// and a search; 30s of silence is a broken connection rather than a slow line,
/// and the retry policy is what handles a timeout.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// How many requests may be in flight at once by default.
///
/// Eight is what the bulk downloader has used all along and what the asset CDN
/// is comfortable with; what changes is that the number now covers *every*
/// request rather than one phase's slice of them.
pub const DEFAULT_LIMIT: usize = 8;

/// One shared client and one shared ceiling.
#[derive(Debug, Clone)]
pub struct HttpPool {
    client: reqwest::blocking::Client,
    limit: Limit,
    timeout: Duration,
}

impl HttpPool {
    /// A pool with a ceiling of `limit` (at least 1) and a default timeout.
    ///
    /// The client is built once, here. `build()` fails only on a TLS backend
    /// this machine cannot initialise, which is a real possibility on a stripped
    /// install and not a reason for a launcher not to start: the fallback client
    /// is `reqwest`'s own default configuration, and a request made through it
    /// either works or reports a transport error like any other.
    pub fn new(limit: usize, timeout: Duration) -> HttpPool {
        let client = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        HttpPool { client, limit: Limit::new(limit), timeout }
    }

    /// The process-wide ceiling this pool draws from.
    pub fn limit(&self) -> &Limit {
        &self.limit
    }

    /// The per-request timeout.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Send `request`, having taken a slot in the ceiling.
    ///
    /// The permit lives as long as the response is being read, which is the
    /// point: the ceiling is about *connections open*, not about requests sent,
    /// and a slow body holds its connection for a long time.
    fn send(&self, request: &Request) -> Result<(reqwest::blocking::Response, Permit<'_>), Error> {
        let permit = self.limit.acquire();
        let mut builder = self.client.get(&request.url).timeout(self.timeout);
        if let Some(range) = request.range() {
            builder = builder.header("Range", range);
        }
        for (name, value) in &request.headers {
            builder = builder.header(name.as_str(), value.as_str());
        }
        let response = builder
            .send()
            .map_err(|error| Error::http(&request.url, error.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(Error::status(&request.url, status.as_u16()));
        }
        Ok((response, permit))
    }

    /// Read the response into `sink` chunk by chunk, stopping if `cancel` is set.
    fn drain(
        &self,
        request: &Request,
        mut response: reqwest::blocking::Response,
        sink: &mut dyn Write,
        cancel: &Cancel,
    ) -> Result<u64, Error> {
        let mut buffer = vec![0u8; CHUNK];
        let mut written = 0u64;
        loop {
            cancel.check()?;
            let read = response
                .read(&mut buffer)
                .map_err(|error| Error::http(&request.url, error.to_string()))?;
            if read == 0 {
                return Ok(written);
            }
            sink.write_all(&buffer[..read])
                .map_err(|error| Error::http(&request.url, error.to_string()))?;
            written += read as u64;
        }
    }
}

impl Default for HttpPool {
    fn default() -> HttpPool {
        HttpPool::new(DEFAULT_LIMIT, DEFAULT_TIMEOUT)
    }
}

impl Fetch for HttpPool {
    fn get(&self, request: &Request, cancel: &Cancel) -> Result<Vec<u8>, Error> {
        if request.offset.is_some() {
            // There is nothing to continue: the callers of this are parsing a
            // document, and a partial document is not a smaller answer, it is a
            // parse error. Refusing is better than sending a `Range` header for
            // a body that will be read whole.
            return Err(Error::format(&request.url, "a whole body cannot carry an offset"));
        }
        cancel.check()?;
        let (response, _permit) = self.send(request)?;
        let mut body = Vec::new();
        self.drain(request, response, &mut body, cancel)?;
        Ok(body)
    }

    fn get_to(
        &self,
        request: &Request,
        sink: &mut dyn Write,
        cancel: &Cancel,
    ) -> Result<Outcome, Error> {
        cancel.check()?;
        let (response, _permit) = self.send(request)?;
        // Decided before a byte is written, which is the whole contract: a
        // server that answered 200 to a `Range` request sent the body from
        // zero, and appending that to a half file is a corrupt file that only
        // its hash would catch.
        let resumed = match request.offset {
            Some(_) => response.status() == reqwest::StatusCode::PARTIAL_CONTENT,
            None => false,
        };
        if request.offset.is_some() && !resumed {
            return Ok(Outcome::Ignored);
        }
        let written = self.drain(request, response, sink, cancel)?;
        Ok(if resumed { Outcome::Resumed(written) } else { Outcome::Whole(written) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::request::MapFetch;

    #[test]
    fn the_user_agent_names_this_build() {
        assert!(USER_AGENT.starts_with("PalantirMC/"), "{USER_AGENT}");
        assert!(USER_AGENT.contains(env!("CARGO_PKG_VERSION")), "{USER_AGENT}");
        // Modrinth's guidelines: a client that names itself is answered.
        assert!(!USER_AGENT.contains(' '), "{USER_AGENT}");
    }

    #[test]
    fn the_pool_is_built_with_its_own_ceiling_and_cloned_as_the_same_one() {
        let pool = HttpPool::new(3, Duration::from_secs(5));
        assert_eq!(pool.limit().capacity(), 3);
        assert_eq!(pool.timeout(), Duration::from_secs(5));
        let clone = pool.clone();
        let held: Vec<_> = (0..3).map(|_| pool.limit().acquire()).collect();
        assert_eq!(clone.limit().available(), 0, "a clone shares the ceiling");
        drop(held);
        assert_eq!(clone.limit().available(), 3, "and gives the slots back");
        // And a zero setting is clamped rather than deadlocking a download.
        assert_eq!(HttpPool::new(0, Duration::from_secs(1)).limit().capacity(), 1);
        assert_eq!(HttpPool::default().limit().capacity(), DEFAULT_LIMIT);
    }

    #[test]
    fn a_whole_body_cannot_carry_an_offset() {
        // Checked before anything is sent, so the caller is told rather than
        // handed a body the server chose to send from somewhere else.
        let pool = HttpPool::default();
        let request = Request::from("http://192.0.2.1/x.json", 10);
        let error = pool
            .get(&request, &Cancel::new())
            .expect_err("an offset on a whole-body read");
        assert!(matches!(error, Error::Format { .. }), "{error:?}");
    }

    #[test]
    fn a_cancelled_token_never_reaches_the_network() {
        // 192.0.2.0/24 is TEST-NET-1: unroutable by design, so if this test
        // sent anything it would hang rather than pass.
        let pool = HttpPool::new(1, Duration::from_millis(50));
        let cancel = Cancel::new();
        cancel.cancel();
        let error = pool
            .get(&Request::get("http://192.0.2.1/x.json"), &cancel)
            .expect_err("cancelled before sending");
        assert!(matches!(error, Error::Cancelled));
        // The slot was not taken either: nothing was acquired to leak.
        assert_eq!(pool.limit().available(), 1);
    }

    #[test]
    fn the_offset_contract_is_held_by_the_double_the_pool_must_agree_with() {
        // Not a test of `HttpPool` -- it needs a socket -- but a statement of
        // what `HttpPool::get_to` must do, written against the double the rest
        // of the engine is tested with, so the two are read together.
        let fetch = MapFetch::new().with_route("u", crate::engine::request::Route::text("abc"));
        let mut whole = Vec::new();
        assert_eq!(
            fetch.get_to(&Request::get("u"), &mut whole, &Cancel::new()).expect("whole"),
            Outcome::Whole(3)
        );
        let mut resumed = Vec::new();
        assert_eq!(
            fetch
                .get_to(&Request::from("u", 1), &mut resumed, &Cancel::new())
                .expect("resumed"),
            Outcome::Resumed(2)
        );
    }
}
