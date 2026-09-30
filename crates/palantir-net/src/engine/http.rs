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
use crate::engine::request::{Fetch, Outcome, Request, Response, CHUNK};
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

    /// Send `request` and take a slot in the ceiling, without judging the
    /// status.
    ///
    /// The permit lives as long as the response is being read, which is the
    /// point: the ceiling is about *connections open*, not about requests sent,
    /// and a slow body holds its connection for a long time. The status check
    /// is not here because one answer is not a failure: a `304` on a
    /// conditional request is the service saying "your copy is current", and a
    /// caller that went through [`HttpPool::send`] would read it as an error and
    /// re-download the body it already has.
    fn send_any(
        &self,
        request: &Request,
    ) -> Result<(reqwest::blocking::Response, Permit<'_>), Error> {
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
        Ok((response, permit))
    }

    /// Send `request`, having taken a slot in the ceiling, and treat a non-2xx
    /// answer as the failure it is.
    fn send(&self, request: &Request) -> Result<(reqwest::blocking::Response, Permit<'_>), Error> {
        let (response, permit) = self.send_any(request)?;
        let status = response.status();
        if !status.is_success() {
            // The body is read here for one reason: it is the only place the
            // service's own sentence lives. A download's 404 and a document's
            // 401 are then told apart by what the service said rather than by
            // the code alone.
            let sentence = failure_sentence(&failure_body(response));
            return Err(Error::status_with(&request.url, status.as_u16(), sentence.as_deref()));
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

    /// The conditional `GET`: keep the validator, and accept `304` as an answer.
    ///
    /// This is the only place a non-2xx status is not an error, and it is
    /// deliberate: `304 Not Modified` means the caller's copy is still current,
    /// which is the whole point of a TTL that expires. The `ETag` is read from
    /// the response before the body is, because `reqwest`'s response owns its
    /// headers and consuming the body first loses them.
    fn get_with(&self, request: &Request, cancel: &Cancel) -> Result<Response, Error> {
        if request.offset.is_some() {
            // Same refusal as `get`: a conditional request is about a document
            // the caller holds whole.
            return Err(Error::format(&request.url, "a whole body cannot carry an offset"));
        }
        cancel.check()?;
        let (response, _permit) = self.send_any(request)?;
        let status = response.status();
        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        if status == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(Response { body: Vec::new(), etag, not_modified: true });
        }
        if !status.is_success() {
            let sentence = failure_sentence(&failure_body(response));
            return Err(Error::status_with(&request.url, status.as_u16(), sentence.as_deref()));
        }
        let mut body = Vec::new();
        self.drain(request, response, &mut body, cancel)?;
        Ok(Response { body, etag, not_modified: false })
    }
}

/// How much of a refusal's body is read to find its sentence.
///
/// A refusal is a small JSON document -- the largest measured is 150 bytes --
/// and a service that answers with an HTML error page must not have that page
/// read into memory, or into a notice, just to say what the status already says.
/// 4 KiB is well past every measured body and far short of a page.
const MAX_FAILURE_BODY: usize = 4 * 1024;

/// How long a kept sentence may be.
///
/// It is bound for a UI notice rather than for the body: a service that answers
/// with a paragraph is truncated here, and the status in front of it still says
/// what the code was.
const MAX_SENTENCE: usize = 200;

/// Read up to [`MAX_FAILURE_BODY`] bytes of a response that is already a failure.
///
/// A body that cannot be read is not a second failure: the caller is about to be
/// told about the status either way, and an `Error::io` here would replace the
/// server's answer with the read error's.
///
/// `pub(crate)` because the metadata store's own fetcher reads a refusal the same
/// way: it is the second place in this crate where a non-2xx becomes an
/// [`Error`], and two answers to "what did the service say" is one too many.
pub(crate) fn failure_body(response: reqwest::blocking::Response) -> Vec<u8> {
    let mut buffer = Vec::new();
    let mut reader = response.take(MAX_FAILURE_BODY as u64);
    if reader.read_to_end(&mut buffer).is_err() {
        return Vec::new();
    }
    buffer
}

/// The sentence a service put in its refusal's body, if it put one there.
///
/// Two shapes are measured (G110, G111): Modrinth's Labrinth and Archon answer
/// `{"error": …, "description": …}` -- "The provided client id was invalid",
/// "unsupported archon request version" -- and Minecraft answers
/// `{"errorMessage": …}`. `description` comes first because it is the sentence
/// written for a reader; `error` is a machine word (`auth_error`,
/// `invalid_client`) and is kept only as a last resort; `details` is not read at
/// all, because it is Labrinth's internal chain and on the one measured 401 that
/// carries it the chain says more than the summary above it.
///
/// A body that is not JSON is still a sentence when it is short and readable:
/// Archon's own 404 is the words `not found`. Markup, binary and anything longer
/// than [`MAX_SENTENCE`] are refused rather than pasted in.
pub(crate) fn failure_sentence(body: &[u8]) -> Option<String> {
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) {
        for key in ["description", "errorMessage", "error"] {
            if let Some(text) = value.get(key).and_then(|value| value.as_str()) {
                if let Some(sentence) = bounded(text) {
                    return Some(sentence);
                }
            }
        }
        return None;
    }
    let text = std::str::from_utf8(body).ok()?;
    bounded(text)
}

/// A service's own words, trimmed and bounded, or nothing if they are not words.
fn bounded(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() || text.contains('<') {
        // Markup is a page rather than a sentence, and the status beside it is a
        // better thing to show a reader than the first line of an error page.
        return None;
    }
    if text.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
        return None;
    }
    if text.chars().count() <= MAX_SENTENCE {
        return Some(text.to_string());
    }
    let mut sentence: String = text.chars().take(MAX_SENTENCE).collect();
    sentence.push('…');
    Some(sentence)
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
    fn a_refusal_keeps_the_service_s_own_sentence() {
        // The four shapes measured against the live services (G110, G111), which
        // is the whole reason this exists: the status is the same 401 for each.
        let labrinth = br#"{"error":"invalid_client","description":"The provided client id was invalid"}"#;
        assert_eq!(
            failure_sentence(labrinth).as_deref(),
            Some("The provided client id was invalid")
        );

        let minecraft = br#"{"errorMessage":"The access token is invalid"}"#;
        assert_eq!(failure_sentence(minecraft).as_deref(), Some("The access token is invalid"));

        // Archon refuses a request version by name and sends no description.
        let archon = br#"{"error":"unsupported archon request version"}"#;
        assert_eq!(
            failure_sentence(archon).as_deref(),
            Some("unsupported archon request version")
        );

        // And its 404 is plain words rather than a document.
        assert_eq!(failure_sentence(b"not found").as_deref(), Some("not found"));
    }

    #[test]
    fn details_is_not_the_sentence_and_a_kept_one_is_the_first_thing_written_for_a_reader() {
        // Labrinth's `description` here is its internal summary; the array beside
        // it is the chain. The summary is what a notice shows, short of reading
        // Labrinth's own code to rank them, and this test is the record of that
        // choice rather than a claim that the summary is a good sentence.
        let body = br#"{"error":"auth_error","description":"flattening v2 not-found response","details":["authenticating API request","Authentication method was not valid"]}"#;
        assert_eq!(failure_sentence(body).as_deref(), Some("flattening v2 not-found response"));
        // A description that is present but empty falls through to the next key.
        let empty = br#"{"error":"invalid_client","description":"   ","errorMessage":"say this"}"#;
        assert_eq!(failure_sentence(empty).as_deref(), Some("say this"));
    }

    #[test]
    fn a_body_that_is_not_a_sentence_is_left_out_rather_than_pasted_in() {
        assert_eq!(failure_sentence(b""), None);
        assert_eq!(failure_sentence(b"   \n"), None);
        assert_eq!(failure_sentence(b"<html><body>502 Bad Gateway</body></html>"), None);
        // Bytes that are not text at all, which a proxy can answer with.
        assert_eq!(failure_sentence(&[0x00, 0xff, 0x10]), None);
        // JSON with no string a reader can use.
        assert_eq!(failure_sentence(br#"{"error":{"code":7}}"#), None);
        assert_eq!(failure_sentence(b"[]"), None);

        // Two hundred characters are kept; the next one is truncated, so a
        // paragraph-long internal description cannot fill a notice.
        let long = "a".repeat(MAX_SENTENCE);
        assert_eq!(failure_sentence(long.as_bytes()).map(|s| s.chars().count()), Some(MAX_SENTENCE));
        let longer = "a".repeat(MAX_SENTENCE + 1);
        let got = failure_sentence(longer.as_bytes()).expect("still a sentence");
        assert_eq!(got.chars().count(), MAX_SENTENCE + 1, "the ellipsis is the extra one");
        assert!(got.ends_with('…'), "{got}");
    }

    #[test]
    fn the_status_and_the_sentence_are_one_line_and_the_bare_string_is_unchanged() {
        // Nothing else in the tree moves: without a sentence this is the exact
        // string every assertion above and every log line already holds.
        let bare = Error::status("https://example.invalid/x", 404).to_string();
        assert!(bare.ends_with("http status 404"), "{bare}");

        let said = Error::status_with(
            "https://example.invalid/x",
            401,
            Some("you are not authorized to view this resource"),
        )
        .to_string();
        assert!(said.contains("http status 401: you are not authorized to view this resource"), "{said}");

        // And the field a retry policy reads is untouched by the sentence.
        match Error::status_with("https://example.invalid/x", 503, Some("down for maintenance")) {
            Error::Http { status, .. } => assert_eq!(status, Some(503)),
            other => panic!("a 503 is an HTTP failure: {other:?}"),
        }
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
    fn a_conditional_read_of_a_partial_file_is_refused_before_it_is_sent() {
        // A conditional request is about a document the caller holds whole, so
        // the offset is refused the same way `get` refuses one. 192.0.2.0/24 is
        // unroutable, so a request that was actually sent would hang.
        let pool = HttpPool::new(1, Duration::from_millis(50));
        let error = pool
            .get_with(&Request::from("http://192.0.2.1/x.json", 10), &Cancel::new())
            .expect_err("an offset on a conditional read");
        assert!(matches!(error, Error::Format { .. }), "{error:?}");
        assert_eq!(pool.limit().available(), 1, "no slot was taken");
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
