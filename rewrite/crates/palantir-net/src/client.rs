//! One pooled HTTP client for the whole launcher.
//!
//! One client, not one per download: connection setup is the expensive part
//! of fetching thousands of small files, and a pool reuses it across the
//! scheduler's workers. This wrapper is the only place that knows the HTTP
//! library exists, so the rest of the crate talks in bytes and statuses.

use std::io::{Read, Write};
use std::time::Duration;

use crate::error::{Error, Result};

/// Who we are when we ask. Some services answer differently to anonymous
/// clients, and a name here is how their operators reach us about traffic.
pub const USER_AGENT: &str = concat!("PalantirMC/", env!("CARGO_PKG_VERSION"));

/// How long to wait for the connection itself. A handshake that has not
/// completed in this long is a network problem worth retrying, not waiting on.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// The whole-request safety valve. Individual files run up to tens of
/// megabytes on slow links, so this is deliberately loose -- it exists to
/// bound a hung transfer, not to judge a slow one.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(900);

/// Keep a handful of idle connections warm per host: the scheduler's
/// concurrency limit is the real ceiling on parallelism.
const POOL_MAX_IDLE_PER_HOST: usize = 4;

/// The shared client.
pub struct Http {
    inner: reqwest::blocking::Client,
}

/// One answered request. Status and body handling live here.
pub struct Response {
    inner: reqwest::blocking::Response,
    url: String,
}

impl Http {
    /// Build the shared client. Called once; the scheduler and cache borrow it.
    pub fn new() -> Result<Self> {
        let inner = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .pool_max_idle_per_host(POOL_MAX_IDLE_PER_HOST)
            .build()
            .map_err(|source| Error::Http {
                url: "(client setup)".to_string(),
                status: None,
                why: source.to_string(),
            })?;
        Ok(Self { inner })
    }

    /// GET a URL.
    pub fn get(&self, url: &str) -> Result<Response> {
        self.send(url, None)
    }

    /// GET a URL from a byte offset on (`Range: bytes=<from>-`), which is
    /// what makes an interrupted transfer resumable. A server that does not
    /// resume answers 200 with the whole body; callers detect that and
    /// restart cleanly rather than appending a whole file to half of one.
    pub fn get_range(&self, url: &str, from: u64) -> Result<Response> {
        self.send(url, Some(from))
    }

    fn send(&self, url: &str, from: Option<u64>) -> Result<Response> {
        let mut request = self.inner.get(url);
        if let Some(from) = from {
            request = request.header("Range", format!("bytes={from}-"));
        }
        let inner = request.send().map_err(|source| Error::Http {
            url: url.to_string(),
            status: None,
            why: source.to_string(),
        })?;
        Ok(Response {
            inner,
            url: url.to_string(),
        })
    }
}

impl Response {
    /// The HTTP status.
    pub fn status(&self) -> u16 {
        self.inner.status().as_u16()
    }

    /// Whether the status is 2xx.
    pub fn is_success(&self) -> bool {
        self.inner.status().is_success()
    }

    /// The response body's length as the server declared it, when it did.
    pub fn content_length(&self) -> Option<u64> {
        self.inner.content_length()
    }

    /// Copy the body to `out`, at most `limit` bytes. Reading fewer bytes
    /// than the body holds is how a caller interrupts a transfer on purpose
    /// (and how a dropped connection looks: the error comes back here).
    pub fn copy_to(&mut self, out: &mut impl Write, limit: Option<u64>) -> Result<u64> {
        let copied = match limit {
            Some(limit) => std::io::copy(&mut self.by_ref().take(limit), out),
            None => std::io::copy(&mut self.inner, out),
        };
        copied.map_err(|source| Error::Io {
            path: std::path::PathBuf::from(self.url.clone()),
            source,
        })
    }

    /// Read the whole body as text (metadata documents).
    pub fn text(self) -> Result<String> {
        self.inner.text().map_err(|source| Error::Http {
            url: self.url,
            status: None,
            why: source.to_string(),
        })
    }

    /// Read the whole body as bytes (small files).
    pub fn bytes(self) -> Result<Vec<u8>> {
        self.inner
            .bytes()
            .map(|bytes| bytes.to_vec())
            .map_err(|source| Error::Http {
                url: self.url,
                status: None,
                why: source.to_string(),
            })
    }

    fn by_ref(&mut self) -> Ref<'_> {
        Ref {
            inner: &mut self.inner,
        }
    }
}

/// `Read` access to the body without giving `copy_to` ownership.
struct Ref<'a> {
    inner: &'a mut reqwest::blocking::Response,
}

impl std::io::Read for Ref<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        std::io::Read::read(self.inner, buf)
    }
}
