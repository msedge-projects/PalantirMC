//! # prism-net
//!
//! Phase 2 networking for the Prism Launcher rewrite in Rust.
//!
//! This crate plugs into the seams fixed by `prism-core`:
//!
//! * [`meta::OnlineMetaStore`] implements
//!   [`prism_core::resolve::MetaStore`] over `meta.prismlauncher.org` with a
//!   disk write-through cache in the [`prism_core::resolve::OfflineMetaStore`]
//!   layout.
//! * [`download`] provides the blocking library/asset download pipeline
//!   (atomic `.part` + rename) and `sha256` verification.
//! * [`auth`] carries the offline + Microsoft OAuth session types (no network
//!   here; the interactive flows land on top of these builders).
//! * [`modrinth`] provides the minimal Modrinth API v2 client types and URL
//!   builders.
//!
//! Design notes:
//!
//! * Blocking I/O only (`reqwest::blocking`); no async runtime is required.
//! * All fallible operations return the local [`Error`]; metadata lookups map
//!   it into [`prism_core::error::Error`] via [`Error::into_core`].
//! * No `.unwrap()`/`.expect()` outside tests.

pub mod auth;
pub mod download;
pub mod meta;
pub mod modrinth;

pub use auth::{MicrosoftOAuth, OfflineSession};
pub use download::{download_bytes, download_file, sha256_hex, verify_sha256};
pub use meta::{BlockingHttpFetcher, Fetcher, MapFetcher, OnlineMetaStore, DEFAULT_META_BASE_URL};
pub use modrinth::{
    ModrinthProjectVersion, ModrinthSearchHit, ModrinthSearchResponse, ModrinthVersionFile,
    search_url, version_url, MODRINTH_BASE_URL,
};

use std::path::PathBuf;

/// Local error type for `prism-net`.
///
/// Network failures stay here (instead of growing `prism-core`); use
/// [`Error::into_core`] to convert into [`prism_core::error::Error`] at the
/// `prism-core` seam (metadata resolution).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// HTTP transport failure (connection, timeout, non-2xx status, ...).
    #[error("http error for {url}: {detail}")]
    Http {
        /// The request URL that failed.
        url: String,
        /// Human-readable reason (status code, timeout, transport message).
        detail: String,
    },

    /// Filesystem failure, with the path that caused it.
    #[error("io error for {path}: {source}")]
    Io {
        /// The path involved in the failed operation.
        path: PathBuf,
        /// Underlying OS error.
        #[source]
        source: std::io::Error,
    },

    /// Malformed or unparsable JSON content.
    #[error("invalid json in {path}: {detail}")]
    Json {
        /// The file (cache path or URL-as-path) involved.
        path: PathBuf,
        /// Parser message.
        detail: String,
    },

    /// Parsed fine but the content is unsupported, or a request was invalid
    /// (bad hex digest, bad endpoint, unexpected payload, ...).
    #[error("unsupported format in {path}: {detail}")]
    Format {
        /// The file (cache path or URL-as-path) involved.
        path: PathBuf,
        /// What is unsupported.
        detail: String,
    },

    /// A downloaded file did not match its expected `sha256` hex digest.
    #[error("hash mismatch for {path}: expected {expected}, got {actual}")]
    HashMismatch {
        /// The file that failed verification.
        path: PathBuf,
        /// Expected lowercase hex digest.
        expected: String,
        /// Actual lowercase hex digest.
        actual: String,
    },
}

/// Crate result alias.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Build an HTTP error for `url` with a human-readable `detail`.
    pub fn http(url: impl Into<String>, detail: impl Into<String>) -> Self {
        Error::Http { url: url.into(), detail: detail.into() }
    }

    /// Wrap an [`std::io::Error`] with path context.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io { path: path.into(), source }
    }

    /// Build a JSON error for `path` with a parser `detail` message.
    pub fn json(path: impl Into<PathBuf>, detail: impl Into<String>) -> Self {
        Error::Json { path: path.into(), detail: detail.into() }
    }

    /// Build a format error for `path` with a human-readable `detail`.
    pub fn format(path: impl Into<PathBuf>, detail: impl Into<String>) -> Self {
        Error::Format { path: path.into(), detail: detail.into() }
    }

    /// Build a hash-mismatch error for `path`.
    pub fn hash_mismatch(
        path: impl Into<PathBuf>,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        Error::HashMismatch { path: path.into(), expected: expected.into(), actual: actual.into() }
    }

    /// Convert into the [`prism_core::error::Error`] seam.
    ///
    /// Mapping mirrors the `prism-core` constructors: IO stays IO, JSON stays
    /// JSON, and transport/hash problems become `Error::format` (with the URL
    /// used as the path for [`Error::Http`]) so resolution can surface them
    /// without new `prism-core` variants.
    pub fn into_core(self) -> prism_core::error::Error {
        match self {
            Error::Io { path, source } => prism_core::error::Error::io(path, source),
            Error::Json { path, detail } => prism_core::error::Error::json(path, detail),
            Error::Format { path, detail } => prism_core::error::Error::format(path, detail),
            Error::Http { url, detail } => {
                prism_core::error::Error::format(PathBuf::from(url), detail)
            }
            Error::HashMismatch { path, expected, actual } => {
                prism_core::error::Error::format(path, format!("sha256 mismatch: expected {expected}, got {actual}"))
            }
        }
    }
}

impl From<Error> for prism_core::error::Error {
    /// Convert a local [`Error`] into the `prism-core` seam (see
    /// [`Error::into_core`]).
    fn from(err: Error) -> Self {
        err.into_core()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_constructor_roundtrips_fields() {
        let e = Error::http("https://example.invalid/x.json", "timeout");
        match e {
            Error::Http { url, detail } => {
                assert_eq!(url, "https://example.invalid/x.json");
                assert_eq!(detail, "timeout");
            }
            _ => panic!("expected Http"),
        }
    }

    #[test]
    fn io_constructor_keeps_path() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let e = Error::io("/tmp/cache/x.json", io_err);
        match e {
            Error::Io { path, .. } => assert_eq!(path, PathBuf::from("/tmp/cache/x.json")),
            _ => panic!("expected Io"),
        }
    }

    #[test]
    fn json_constructor_keeps_detail() {
        let e = Error::json("/tmp/x.json", "expected value");
        assert!(e.to_string().contains("expected value"));
    }

    #[test]
    fn format_constructor_keeps_detail() {
        let e = Error::format("/tmp/x.json", "bad thing");
        assert!(e.to_string().contains("bad thing"));
    }

    #[test]
    fn hash_mismatch_constructor_keeps_digests() {
        let e = Error::hash_mismatch("/tmp/a.jar", "aa", "bb");
        let s = e.to_string();
        assert!(s.contains("aa") && s.contains("bb"));
    }

    #[test]
    fn into_core_maps_io_to_core_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "nf");
        let core: prism_core::error::Error = Error::io("/tmp/x", io_err).into_core();
        match core {
            prism_core::error::Error::Io { path, .. } => {
                assert_eq!(path, PathBuf::from("/tmp/x"));
            }
            _ => panic!("expected core Io"),
        }
    }

    #[test]
    fn into_core_maps_json_to_core_json() {
        let core: prism_core::error::Error = Error::json("/tmp/x.json", "boom").into_core();
        match core {
            prism_core::error::Error::Json { detail, .. } => assert_eq!(detail, "boom"),
            _ => panic!("expected core Json"),
        }
    }

    #[test]
    fn into_core_maps_format_to_core_format() {
        let core: prism_core::error::Error = Error::format("/tmp/x", "nope").into_core();
        match core {
            prism_core::error::Error::Format { detail, .. } => assert_eq!(detail, "nope"),
            _ => panic!("expected core Format"),
        }
    }

    #[test]
    fn into_core_maps_http_to_core_format_with_url_as_path() {
        let core: prism_core::error::Error =
            Error::http("https://example.invalid/a", "denied").into_core();
        match core {
            prism_core::error::Error::Format { path, detail } => {
                assert_eq!(path, PathBuf::from("https://example.invalid/a"));
                assert_eq!(detail, "denied");
            }
            _ => panic!("expected core Format"),
        }
    }

    #[test]
    fn into_core_maps_hash_mismatch_to_core_format() {
        let core: prism_core::error::Error =
            Error::hash_mismatch("/tmp/a.jar", "aa", "bb").into_core();
        match core {
            prism_core::error::Error::Format { path, detail } => {
                assert_eq!(path, PathBuf::from("/tmp/a.jar"));
                assert!(detail.contains("aa") && detail.contains("bb"));
            }
            _ => panic!("expected core Format"),
        }
    }

    #[test]
    fn from_impl_delegates_to_into_core() {
        let core: prism_core::error::Error = prism_core::error::Error::from(
            Error::format("/tmp/x", "via-from"),
        );
        match core {
            prism_core::error::Error::Format { detail, .. } => assert_eq!(detail, "via-from"),
            _ => panic!("expected core Format"),
        }
    }
}
