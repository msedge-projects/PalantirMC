//! What the engine asks for, and the seam every rule in it is tested through.
//!
//! [`Request`] is a URL plus the two things the engine needs to say about one: an
//! offset to continue a file from, and the headers a service asked for. It is
//! deliberately narrower than `reqwest`'s builder, and a value rather than a
//! closure for a reason -- a test can assert *what was asked* and not only what
//! came back, which is the difference between "the bytes are right" and "the
//! bytes were fetched again from zero".
//!
//! [`Fetch`] is the seam. `reqwest` cannot be reached from a unit test, and every
//! rule that matters here is a rule about *how the engine reacts*: a server that
//! honours an offset and one that ignores it, a 503 that clears, a cancellation
//! that lands half way through a body. [`MapFetch`] below is a server that can be
//! told to do all four, so those rules are tested rather than experienced.
//!
//! ## The offset contract
//!
//! A request carrying an offset says "continue from here", and the answer is one
//! of three:
//!
//! * [`Outcome::Resumed`] -- 206, and `n` bytes were written on from the offset;
//! * [`Outcome::Whole`] -- 200 on a request that asked for no offset, `n` bytes
//!   from the start;
//! * [`Outcome::Ignored`] -- the request carried an offset and the server sent
//!   the whole body anyway. **Nothing has been written**, and the caller starts
//!   again from zero. This is the case that decides whether a resumed download is
//!   safe: appending a full body to a half file is a corrupt jar that only fails
//!   its hash later, in front of a user.

use std::collections::HashMap;
use std::io::Write;
use std::sync::Mutex;

use crate::engine::cancel::Cancel;
use crate::Error;

/// How many bytes are read from a response at a time.
///
/// One buffer this size is the memory a transfer costs, whatever the file
/// weighs, and it is the interval at which cancellation is noticed: a 64 KB
/// chunk at a slow line speed is a few milliseconds. Smaller would make the
/// cancellation crisper and the transfer slower; this is the same 64 KB the
/// existing bulk downloader streams with, so the two paths cost the same.
pub const CHUNK: usize = 64 * 1024;

/// One HTTP request, as a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The URL to fetch.
    pub url: String,
    /// Where to continue from, if this request is finishing a partial file.
    ///
    /// `None` asks for the whole thing, which is both the first attempt and the
    /// restart after a server ignored an offset.
    pub offset: Option<u64>,
    /// Extra headers, in the order to send them.
    pub headers: Vec<(String, String)>,
}

impl Request {
    /// A request for the whole of `url`.
    pub fn get(url: impl Into<String>) -> Request {
        Request { url: url.into(), offset: None, headers: Vec::new() }
    }

    /// The same request, continuing from `offset`.
    pub fn from(url: impl Into<String>, offset: u64) -> Request {
        Request { url: url.into(), offset: Some(offset), headers: Vec::new() }
    }

    /// The same request with a header added.
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Request {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// The `Range` header's value, when this request carries an offset.
    pub fn range(&self) -> Option<String> {
        self.offset.map(|offset| format!("bytes={offset}-"))
    }

    /// The value of a header this request carries, matched case-insensitively.
    ///
    /// HTTP header names are case-insensitive, and a `Fetch` that compared them
    /// as strings would fail to recognise a conditional request whenever a
    /// caller spelled `if-none-match` the way the service does.
    pub fn header_value(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(have, _)| have.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// What a fetch did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// No offset was asked for, and `0` bytes were written from the start.
    Whole(u64),
    /// An offset was asked for and honoured: `1` bytes were written on from it.
    Resumed(u64),
    /// An offset was asked for and ignored. Nothing was written.
    Ignored,
}

impl Outcome {
    /// How many bytes this outcome carried.
    pub fn bytes(self) -> u64 {
        match self {
            Outcome::Whole(bytes) | Outcome::Resumed(bytes) => bytes,
            Outcome::Ignored => 0,
        }
    }

    /// Whether the body continued a partial file rather than starting one.
    pub fn resumed(self) -> bool {
        matches!(self, Outcome::Resumed(_))
    }
}

/// A whole body, and what the service said about it.
///
/// [`Outcome`] is about a *transfer*: how it continued, or that it could not.
/// This is about a `GET` whose answer a caller is going to keep -- a metadata
/// file, a version list -- where the two facts that decide whether it has to be
/// asked for again are the validator the service gave ("is this still
/// current?") and the answer to that question when it is asked. Neither is on
/// the `get` path, because `bytes()` consumes the response and its headers with
/// it; carrying them up here is what lets a cache revalidate rather than
/// re-download.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Response {
    /// The body. Empty when the service withheld one because the copy the
    /// caller holds is still current.
    pub body: Vec<u8>,
    /// The validator to send back as `If-None-Match`, when the service gave one.
    ///
    /// `None` means this response cannot be revalidated and has to be fetched
    /// again in full once it goes stale, which is the honest reading of a
    /// service that sends no `ETag`.
    pub etag: Option<String>,
    /// Whether the body was withheld because the caller's copy is current.
    pub not_modified: bool,
}

impl Response {
    /// A body with no validator and nothing withheld.
    pub fn whole(body: impl Into<Vec<u8>>) -> Response {
        Response { body: body.into(), etag: None, not_modified: false }
    }
}

/// Somewhere bytes come from.
///
/// `Send + Sync` because a pool of workers shares one *across threads*: the
/// engine's whole point is that several transfers run at once, and the thing
/// they share has to be usable from all of them -- which is a stronger claim than
/// `Sync` alone, and one the scheduler needs in order to move its `Arc` onto a
/// worker.
pub trait Fetch: Send + Sync {
    /// Fetch the whole body.
    ///
    /// Used for what the engine parses rather than stores: a metadata file, a
    /// version list, a search response. An offset on the request is an error
    /// here -- there is nothing to continue -- so implementations may treat the
    /// body as complete.
    fn get(&self, request: &Request, cancel: &Cancel) -> Result<Vec<u8>, Error>;

    /// Fetch the body into `sink`, following the offset contract above.
    ///
    /// `cancel` is checked between chunks, which is what makes cancelling a
    /// transfer take milliseconds rather than the length of the file.
    fn get_to(
        &self,
        request: &Request,
        sink: &mut dyn Write,
        cancel: &Cancel,
    ) -> Result<Outcome, Error>;

    /// Fetch a whole body and keep what the service said about it.
    ///
    /// The default is [`Fetch::get`] with no validator, which is what a fetch
    /// that cannot see headers can honestly offer: a cache over it re-fetches
    /// after its TTL instead of asking whether it has to. [`crate::engine::http::HttpPool`]
    /// overrides this with the real thing, and that override is what makes a
    /// stale metadata file cost a 304 rather than a download.
    fn get_with(&self, request: &Request, cancel: &Cancel) -> Result<Response, Error> {
        Ok(Response::whole(self.get(request, cancel)?))
    }
}

// ---- A server that can be told what to do ---------------------------------

/// One scripted response.
///
/// Everything here models something a real server does that changes how the
/// engine must behave, and nothing else: the point is not to simulate HTTP but
/// to make the four interesting reactions reproducible.
#[derive(Debug, Clone)]
pub struct Route {
    /// The body the server answers with.
    pub body: Vec<u8>,
    /// Whether `Range` is honoured (206) or ignored (200).
    pub honours_range: bool,
    /// How many further attempts fail before the body is served.
    pub failures: usize,
    /// The status those failures carry.
    pub status: u16,
    /// How many bytes are written per chunk.
    ///
    /// The real fetch uses [`CHUNK`]; a test that wants the cancellation to land
    /// part way through a body sets this to something small and the check
    /// between chunks gets its chance.
    pub chunk: usize,
    /// A token to cancel, and after how many chunks to do it.
    pub stop_after: Option<(usize, Cancel)>,
    /// How long the server takes before it starts answering.
    ///
    /// The one knob that makes a scheduler test deterministic rather than
    /// lucky: with one worker held by a slow job, a second job is *provably*
    /// still in the queue when the test cancels it.
    pub delay: Option<std::time::Duration>,
    /// The validator this route announces as `ETag`, when it has one.
    ///
    /// A route with a validator answers `304` to a request carrying a matching
    /// `If-None-Match`, which is the whole behaviour a TTL'd cache is built on.
    /// A route without one never does, which is what a service that sends no
    /// `ETag` looks like from here.
    pub etag: Option<String>,
}

impl Default for Route {
    fn default() -> Route {
        Route {
            body: Vec::new(),
            honours_range: true,
            failures: 0,
            status: 503,
            chunk: CHUNK,
            stop_after: None,
            delay: None,
            etag: None,
        }
    }
}

impl Route {
    /// A route that always serves `body` and honours an offset.
    pub fn body(body: impl Into<Vec<u8>>) -> Route {
        Route { body: body.into(), ..Route::default() }
    }

    /// The same route from a `&str`, for JSON fixtures.
    pub fn text(body: &str) -> Route {
        Route::body(body.as_bytes().to_vec())
    }

    /// A server that ignores `Range`, as a static file host may.
    pub fn ignoring_range(mut self) -> Route {
        self.honours_range = false;
        self
    }

    /// A server that fails `times` attempts with `status` before answering.
    pub fn failing(mut self, times: usize, status: u16) -> Route {
        self.failures = times;
        self.status = status;
        self
    }

    /// Write the body in `chunk`-sized pieces.
    pub fn chunked(mut self, chunk: usize) -> Route {
        self.chunk = chunk.max(1);
        self
    }

    /// Take `delay` before answering, as a slow host does.
    pub fn pausing(mut self, delay: std::time::Duration) -> Route {
        self.delay = Some(delay);
        self
    }

    /// Announce a validator, so a matching `If-None-Match` is answered `304`.
    pub fn tagged(mut self, etag: &str) -> Route {
        self.etag = Some(etag.to_string());
        self
    }

    /// Cancel `cancel` once `chunks` pieces have been written.
    ///
    /// What a test uses to make a cancellation land *inside* a body, which is
    /// the case the loop's check exists for and the case a pre-cancelled token
    /// cannot reach.
    pub fn stopping_after(mut self, chunks: usize, cancel: Cancel) -> Route {
        self.stop_after = Some((chunks.max(1), cancel));
        self
    }
}

/// A `Fetch` backed by a map of URL to [`Route`], for tests.
#[derive(Debug, Default)]
pub struct MapFetch {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    routes: HashMap<String, Route>,
    calls: Vec<Request>,
}

impl MapFetch {
    /// A server with no routes: everything is a 404.
    pub fn new() -> MapFetch {
        MapFetch::default()
    }

    /// Script `url`.
    pub fn with_route(self, url: impl Into<String>, route: Route) -> MapFetch {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .routes
            .insert(url.into(), route);
        self
    }

    /// Replace a route's body, which is how a test models a server that answers
    /// differently on a later attempt.
    pub fn set_body(&self, url: &str, body: impl Into<Vec<u8>>) {
        let mut inner = self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(route) = inner.routes.get_mut(url) {
            route.body = body.into();
        }
    }

    /// Replace a whole route, which is how a test models a service that
    /// *republishes* something: different bytes **and** a different validator,
    /// where `set_body` alone would leave a validator that says the old bytes
    /// are still current.
    pub fn set_route(&self, url: &str, route: Route) {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .routes
            .insert(url.to_string(), route);
    }

    /// Every request made, in order, failures included.
    ///
    /// The failures matter: "three attempts" is a claim about this list, and a
    /// test that only counted successes could not tell a retry from a single
    /// successful request.
    pub fn requests(&self) -> Vec<Request> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).calls.clone()
    }

    /// How many requests were made.
    pub fn count(&self) -> usize {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).calls.len()
    }

    /// Serve one request: record it, fail it if it is still scripted to fail,
    /// then write what the route says.
    fn serve(
        &self,
        request: &Request,
        sink: &mut dyn Write,
        cancel: &Cancel,
    ) -> Result<Outcome, Error> {
        let route = self.scripted(request)?;
        write_route(request, &route, sink, cancel)
    }

    /// Record that a request arrived, apply the failure script to it, and
    /// return what is left to serve.
    ///
    /// Split out of `serve` so that the conditional path applies the *same*
    /// script: "two 503s and then the body" is a claim about every request to
    /// that URL, and it has to hold whichever method made it. The lock is
    /// released before the caller answers, because the caller is a server and a
    /// server that holds a map's mutex while it writes a body is a server that
    /// cannot serve two things at once.
    fn scripted(&self, request: &Request) -> Result<Route, Error> {
        let mut inner = self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        inner.calls.push(request.clone());
        let Some(route) = inner.routes.get_mut(&request.url) else {
            return Err(Error::status(&request.url, 404));
        };
        if route.failures > 0 {
            route.failures -= 1;
            return Err(Error::status(&request.url, route.status));
        }
        Ok(route.clone())
    }
}

/// Write a scripted route's body into `sink`, following the offset contract.
///
/// A free function rather than a method because it needs nothing from the map:
/// what it writes is decided entirely by the request and the route, which is
/// also why a test can call it directly to state the contract.
fn write_route(
    request: &Request,
    route: &Route,
    sink: &mut dyn Write,
    cancel: &Cancel,
) -> Result<Outcome, Error> {
    if let Some(delay) = route.delay {
        std::thread::sleep(delay);
    }
    let (start, outcome) = match request.offset {
        // A server that will not continue, and one that cannot because the
        // part file is longer than the body: both mean "start again from
        // zero", and neither may write a byte first.
        Some(_) if !route.honours_range => return Ok(Outcome::Ignored),
        Some(offset) if offset > route.body.len() as u64 => return Ok(Outcome::Ignored),
        Some(offset) => (offset as usize, Outcome::Resumed(0)),
        None => (0, Outcome::Whole(0)),
    };
    let mut written = 0u64;
    let mut chunks = 0usize;
    // `chunk` is a public field, and `chunks(0)` panics: a route built by hand
    // with a zero there is a test being sloppy, not a reason to abort a run.
    for piece in route.body[start..].chunks(route.chunk.max(1)) {
        cancel.check()?;
        sink.write_all(piece)
            .map_err(|error| Error::http(&request.url, error.to_string()))?;
        written += piece.len() as u64;
        chunks += 1;
        if let Some((after, token)) = &route.stop_after {
            if chunks >= *after {
                token.cancel();
            }
        }
    }
    Ok(match outcome {
        Outcome::Resumed(_) => Outcome::Resumed(written),
        _ => Outcome::Whole(written),
    })
}

impl Fetch for MapFetch {
    fn get(&self, request: &Request, cancel: &Cancel) -> Result<Vec<u8>, Error> {
        let mut body = Vec::new();
        match self.serve(request, &mut body, cancel)? {
            Outcome::Ignored => Err(Error::format(&request.url, "no body for an offset")),
            _ => Ok(body),
        }
    }

    fn get_to(
        &self,
        request: &Request,
        sink: &mut dyn Write,
        cancel: &Cancel,
    ) -> Result<Outcome, Error> {
        self.serve(request, sink, cancel)
    }

    /// The conditional path: answer `304` when the caller sent back the
    /// validator this route announces.
    ///
    /// A whole-body read, like `get`, so an offset is refused rather than
    /// half-served. A route with no validator can never answer `304`, which is
    /// the behaviour of a service that sends no `ETag` -- and the reason a
    /// cache in front of one has to re-download.
    fn get_with(&self, request: &Request, cancel: &Cancel) -> Result<Response, Error> {
        if request.offset.is_some() {
            return Err(Error::format(&request.url, "a whole body cannot carry an offset"));
        }
        let route = self.scripted(request)?;
        if let Some(etag) = &route.etag {
            if request.header_value("If-None-Match") == Some(etag.as_str()) {
                return Ok(Response {
                    body: Vec::new(),
                    etag: Some(etag.clone()),
                    not_modified: true,
                });
            }
        }
        let mut body = Vec::new();
        write_route(request, &route, &mut body, cancel)?;
        Ok(Response { body, etag: route.etag.clone(), not_modified: false })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const URL: &str = "https://example.invalid/a.jar";

    #[test]
    fn a_request_says_what_it_asks_for() {
        let whole = Request::get(URL);
        assert_eq!(whole.offset, None);
        assert_eq!(whole.range(), None);
        let resumed = Request::from(URL, 1024).header("Accept", "application/octet-stream");
        assert_eq!(resumed.range().as_deref(), Some("bytes=1024-"));
        assert_eq!(resumed.headers, vec![("Accept".to_string(), "application/octet-stream".to_string())]);
        // The offset is part of what was asked, so two requests that differ only
        // in it are not the same request.
        assert_ne!(Request::get(URL), Request::from(URL, 0));
    }

    #[test]
    fn an_outcome_says_how_much_it_carried_and_how() {
        assert_eq!(Outcome::Whole(10).bytes(), 10);
        assert_eq!(Outcome::Resumed(4).bytes(), 4);
        assert_eq!(Outcome::Ignored.bytes(), 0);
        assert!(Outcome::Resumed(4).resumed());
        assert!(!Outcome::Whole(4).resumed());
    }

    #[test]
    fn a_whole_body_is_served_and_the_request_is_recorded() {
        let fetch = MapFetch::new().with_route(URL, Route::text("hello"));
        let mut body = Vec::new();
        let outcome = fetch
            .get_to(&Request::get(URL), &mut body, &Cancel::new())
            .expect("a body");
        assert_eq!(outcome, Outcome::Whole(5));
        assert_eq!(body, b"hello");
        assert_eq!(fetch.count(), 1);
        assert_eq!(fetch.requests()[0].url, URL);
        assert_eq!(fetch.get(&Request::get(URL), &Cancel::new()).expect("bytes"), b"hello");
        assert_eq!(fetch.count(), 2);
    }

    #[test]
    fn an_honoured_offset_serves_only_what_is_left() {
        let fetch = MapFetch::new().with_route(URL, Route::text("hello"));
        let mut body = Vec::new();
        let outcome = fetch
            .get_to(&Request::from(URL, 3), &mut body, &Cancel::new())
            .expect("a body");
        assert_eq!(outcome, Outcome::Resumed(2));
        assert_eq!(body, b"lo", "the first three bytes were not sent again");
    }

    #[test]
    fn an_ignored_offset_writes_nothing_at_all() {
        // The rule that makes a resumed download safe: if the server will not
        // continue, the caller must start over, and the only way it can know is
        // that the sink is untouched.
        for route in [Route::text("hello").ignoring_range(), Route::text("hi")] {
            let fetch = MapFetch::new().with_route(URL, route);
            let mut body = b"keep me".to_vec();
            let outcome = fetch
                .get_to(&Request::from(URL, 4), &mut body, &Cancel::new())
                .expect("an answer");
            assert_eq!(outcome, Outcome::Ignored);
            assert_eq!(body, b"keep me", "nothing was written");
        }
    }

    #[test]
    fn the_scripted_failures_run_out_and_the_body_arrives_once() {
        let fetch = MapFetch::new().with_route(URL, Route::text("body").failing(2, 503));
        let cancel = Cancel::new();
        assert!(matches!(
            fetch.get(&Request::get(URL), &cancel),
            Err(Error::Http { status: Some(503), .. })
        ));
        assert!(matches!(
            fetch.get(&Request::get(URL), &cancel),
            Err(Error::Http { status: Some(503), .. })
        ));
        assert_eq!(fetch.get(&Request::get(URL), &cancel).expect("the third"), b"body");
        // A missing route is a 404, not a panic.
        assert!(matches!(
            fetch.get(&Request::get("https://example.invalid/nothing"), &cancel),
            Err(Error::Http { status: Some(404), .. })
        ));
    }

    #[test]
    fn a_body_written_piece_by_piece_stops_where_it_is_told_to() {
        let stop = Cancel::new();
        let fetch = MapFetch::new().with_route(
            URL,
            Route::body(vec![b'x'; 100]).chunked(10).stopping_after(3, stop.clone()),
        );
        let mut body = Vec::new();
        let error = fetch
            .get_to(&Request::get(URL), &mut body, &stop)
            .expect_err("cancelled part way");
        assert!(matches!(error, Error::Cancelled));
        // Three chunks of ten, and the fourth chunk's check is what stopped it.
        assert_eq!(body.len(), 30);
    }

    #[test]
    fn a_pre_cancelled_token_stops_before_a_byte_is_written() {
        let cancel = Cancel::new();
        cancel.cancel();
        let fetch = MapFetch::new().with_route(URL, Route::text("hello"));
        let mut body = Vec::new();
        let error = fetch
            .get_to(&Request::get(URL), &mut body, &cancel)
            .expect_err("cancelled before the first chunk");
        assert!(matches!(error, Error::Cancelled));
        assert!(body.is_empty());
        // The request was still made: the cancellation is noticed between
        // chunks, and there is no chunk before the response arrives.
        assert_eq!(fetch.count(), 1);
    }

    #[test]
    fn a_header_is_found_whatever_case_the_caller_spelled_it_in() {
        let request = Request::get(URL).header("if-none-match", "\"v1\"");
        assert_eq!(request.header_value("If-None-Match"), Some("\"v1\""));
        assert_eq!(request.header_value("IF-NONE-MATCH"), Some("\"v1\""));
        assert_eq!(request.header_value("Accept"), None);
    }

    #[test]
    fn a_validator_the_route_announced_comes_back_as_not_modified() {
        let fetch = MapFetch::new().with_route(URL, Route::text("the body").tagged("\"v1\""));
        let cancel = Cancel::new();
        let first = fetch.get_with(&Request::get(URL), &cancel).expect("an answer");
        assert_eq!(first.body, b"the body");
        assert_eq!(first.etag.as_deref(), Some("\"v1\""));
        assert!(!first.not_modified);

        // The same route, asked with the validator it gave: no body at all, and
        // the request still counted, because "still current" is an answer and
        // not an absence of one.
        let again = fetch
            .get_with(&Request::get(URL).header("If-None-Match", "\"v1\""), &cancel)
            .expect("an answer");
        assert!(again.not_modified);
        assert!(again.body.is_empty());
        assert_eq!(again.etag.as_deref(), Some("\"v1\""));
        assert_eq!(fetch.count(), 2);
    }

    #[test]
    fn a_validator_that_does_not_match_is_answered_with_the_body() {
        let fetch = MapFetch::new().with_route(URL, Route::text("changed").tagged("\"v2\""));
        let answer = fetch
            .get_with(&Request::get(URL).header("If-None-Match", "\"v1\""), &Cancel::new())
            .expect("an answer");
        assert!(!answer.not_modified);
        assert_eq!(answer.body, b"changed");
        assert_eq!(answer.etag.as_deref(), Some("\"v2\""), "the new validator is kept");
    }

    #[test]
    fn a_route_without_a_validator_always_sends_the_body() {
        // What a service with no `ETag` looks like from here, and the reason a
        // cache in front of one has to re-download rather than ask.
        let fetch = MapFetch::new().with_route(URL, Route::text("body"));
        let answer = fetch
            .get_with(&Request::get(URL).header("If-None-Match", "\"v1\""), &Cancel::new())
            .expect("an answer");
        assert!(!answer.not_modified);
        assert_eq!(answer.body, b"body");
        assert_eq!(answer.etag, None);
    }

    #[test]
    fn the_scripted_failures_apply_to_a_conditional_read_too() {
        let fetch = MapFetch::new().with_route(URL, Route::text("late").tagged("\"v1\"").failing(2, 503));
        let cancel = Cancel::new();
        for _ in 0..2 {
            assert!(matches!(
                fetch.get_with(&Request::get(URL), &cancel),
                Err(Error::Http { status: Some(503), .. })
            ));
        }
        let answer = fetch.get_with(&Request::get(URL), &cancel).expect("the third");
        assert_eq!(answer.body, b"late");
    }

    /// A fetch that only offers `get`/`get_to`, as an implementation that
    /// cannot see headers would.
    struct Plain(MapFetch);

    impl Fetch for Plain {
        fn get(&self, request: &Request, cancel: &Cancel) -> Result<Vec<u8>, Error> {
            self.0.get(request, cancel)
        }

        fn get_to(
            &self,
            request: &Request,
            sink: &mut dyn Write,
            cancel: &Cancel,
        ) -> Result<Outcome, Error> {
            self.0.get_to(request, sink, cancel)
        }
    }

    #[test]
    fn the_default_conditional_read_is_a_plain_get_with_nothing_kept() {
        // The trait's default, and the honest one: a fetch that cannot see
        // headers has no validator to offer, so the caller is told it has to
        // re-fetch rather than being promised a `304` that can never come.
        let plain = Plain(MapFetch::new().with_route(URL, Route::text("body").tagged("\"v1\"")));
        let answer = plain
            .get_with(&Request::get(URL).header("If-None-Match", "\"v1\""), &Cancel::new())
            .expect("an answer");
        assert_eq!(answer.body, b"body");
        assert_eq!(answer.etag, None);
        assert!(!answer.not_modified);
        assert_eq!(Response::whole("x"), Response { body: b"x".to_vec(), etag: None, not_modified: false });
    }
}
