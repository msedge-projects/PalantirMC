//! # palantir-net
//!
//! Phase 2 networking for PalantirMC.
//!
//! This crate plugs into the seams fixed by `palantir-core`:
//!
//! * [`meta::OnlineMetaStore`] implements
//!   [`palantir_core::resolve::MetaStore`] over `meta.prismlauncher.org` with a
//!   disk write-through cache in the [`palantir_core::resolve::OfflineMetaStore`]
//!   layout.
//! * [`download`] provides the blocking library/asset download pipeline
//!   (atomic `.part` + rename) and `sha256` verification.
//! * [`auth`] carries the offline session type and drives the real Microsoft
//!   device-code login (device code → Xbox Live → XSTS → game token →
//!   entitlements → profile), with every HTTP call behind a trait so the chain
//!   is testable offline.
//! * [`engine`] is where every new request goes: one shared client, one
//!   process-wide concurrency ceiling, transfers that can be cancelled between
//!   chunks, downloads that resume from the part file they left behind, and one
//!   retry policy with a spread. Additive -- [`download`] and [`meta`] still
//!   serve the interface that exists, and the engine is what it moves onto.
//! * [`modrinth`] provides the minimal Modrinth API v2 client types and URL
//!   builders.
//! * [`java`] provides the Java runtime metadata the service publishes
//!   (`net.minecraft.java`): the platform tag a host needs, the runtime entries
//!   a major offers, and the per-file manifest Mojang's own JRE is described by.
//!   Parsing only — installing one is filesystem work for the caller.
//!
//! Design notes:
//!
//! * Blocking I/O only (`reqwest::blocking`); no async runtime is required.
//! * All fallible operations return the local [`Error`]; metadata lookups map
//!   it into [`palantir_core::error::Error`] via [`Error::into_core`].
//! * No `.unwrap()`/`.expect()` outside tests.

pub mod auth;
pub mod download;
/// The engine every request goes through: one client, one concurrency ceiling,
/// cancellable transfers, resumable downloads and one retry policy.
///
/// Stage 4 of the rewrite spec. Additive rather than a replacement: the older
/// `download` and `meta` modules still serve the interface that exists, and the
/// engine is what the pages and the install path move onto. See the module for
/// why the rules live there rather than at the call sites.
pub mod engine;
pub mod java;
pub mod meta;
pub mod modrinth;

pub use auth::{
    file_part_name, msa_auth_session, skin_upload_body, xsts_message, AuthError,
    BlockingHttpTransport, DeviceCodeResponse, HttpTransport, HttpResponse, MapTransport,
    MinecraftCape, MinecraftSession, MinecraftSkin, MinecraftSkins, MicrosoftAuth,
    MicrosoftOAuth, MsaToken, MultipartBody, OfflineSession, PollOutcome, SkinChange,
    DEFAULT_MICROSOFT_CLIENT_ID,
};
pub use download::{
    download_bytes, download_file, download_many, download_many_with_progress, sha256_hex,
    verify_sha256,
};
pub use engine::{
    artifact_path, component_uid, fetch_to_file, find_java, install, installer_url, is_retryable,
    maven_roots, maven_sha1, next_event, parse_installer, translate_profile, Backoff, Build,
    Cancel, Cached, ContentStore, DataValue, Digest, Download, Downloaded, Event, Fetch, HttpPool,
    InstallCtx, InstallSpec, InstalledProcessor, InstallerMeta, Job, JobId, Limit, Loader,
    LoaderMeta, Manifest, ManifestVersion, MetadataCache, ModrinthApi, Outcome, ParsedInstaller,
    PistonMeta, Processor, Request, Response, Scheduler, Search, Stored, CENTRAL_MAVEN,
    CLIENT_SIDE, DEFAULT_LIMIT, DEFAULT_TIMEOUT, DEFAULT_TTL, DEFAULT_WORKERS, FORGE_MAVEN,
    IMMUTABLE_TTL, MAX_BUILDS, MOJANG_LIBRARIES, NEOFORGE_MAVEN, PISTON_MANIFEST_URL, SEARCH_TTL,
    USER_AGENT,
};
pub use meta::{BlockingHttpFetcher, Fetcher, MapFetcher, OnlineMetaStore, DEFAULT_META_BASE_URL};
pub use modrinth::{
    date_label, search_url, user_projects_url, user_url, version_url, ModrinthDependency,
    ModrinthProjectVersion, ModrinthSearchHit, ModrinthSearchResponse, ModrinthUser,
    ModrinthUserProject, ModrinthVersionFile, NewsArticle, NewsFeed, MODRINTH_BASE_URL,
    NEWS_PAGE_URL, NEWS_URL,
};

use std::path::PathBuf;

/// Local error type for `palantir-net`.
///
/// Network failures stay here (instead of growing `palantir-core`); use
/// [`Error::into_core`] to convert into [`palantir_core::error::Error`] at the
/// `palantir-core` seam (metadata resolution).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// HTTP transport failure (connection, timeout, non-2xx status, ...).
    ///
    /// `status` is the HTTP status when the server gave one, and `None` for a
    /// transport problem that never got an answer. The retry policy needs to
    /// tell those apart -- a 503 is the server asking for a second attempt and
    /// a 404 is the server meaning it -- and a string like "http status 503" is
    /// not something a decision should be parsed back out of.
    #[error("http error for {url}: {detail}")]
    Http {
        /// The request URL that failed.
        url: String,
        /// Human-readable reason (status code, timeout, transport message).
        detail: String,
        /// The HTTP status, when the server answered at all.
        status: Option<u16>,
    },

    /// Work stopped because the caller asked it to.
    ///
    /// Not a failure of anything: it is the answer to "please stop", and it is
    /// a variant of its own rather than a flavour of HTTP error so that a retry
    /// policy can refuse to retry it and a progress line can say "cancelled"
    /// rather than "failed".
    #[error("cancelled")]
    Cancelled,

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
    ///
    /// For a transport problem, where no status arrived. Use [`Error::status`]
    /// when the server answered.
    pub fn http(url: impl Into<String>, detail: impl Into<String>) -> Self {
        Error::Http { url: url.into(), detail: detail.into(), status: None }
    }

    /// Build an HTTP error from a status code the server sent.
    ///
    /// The detail is derived from the code so that the message and the field
    /// cannot disagree, and so that every non-2xx in the engine reads the same
    /// way in a log.
    pub fn status(url: impl Into<String>, status: u16) -> Self {
        Error::status_with(url, status, None)
    }

    /// Build an HTTP error from a status code *and* whatever sentence the
    /// service put in the body.
    ///
    /// The services in this launcher answer a refusal two ways: Minecraft sends
    /// `{"errorMessage": …}`, and Modrinth's Labrinth and Archon send
    /// `{"error": …, "description": …}` (measured, G110 and G111). The status
    /// alone is the same `401` for every one of those, so a page that shows it
    /// tells a reader less than the service was willing to say -- which is why
    /// the sentence is kept when it exists and the old string is unchanged when
    /// it does not.
    pub fn status_with(url: impl Into<String>, status: u16, sentence: Option<&str>) -> Self {
        let detail = match sentence {
            Some(sentence) => format!("http status {status}: {sentence}"),
            None => format!("http status {status}"),
        };
        Error::Http { url: url.into(), detail, status: Some(status) }
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

    /// Convert into the [`palantir_core::error::Error`] seam.
    ///
    /// Mapping mirrors the `palantir-core` constructors: IO stays IO, JSON stays
    /// JSON, and transport/hash problems become `Error::format` (with the URL
    /// used as the path for [`Error::Http`]) so resolution can surface them
    /// without new `palantir-core` variants.
    pub fn into_core(self) -> palantir_core::error::Error {
        match self {
            Error::Io { path, source } => palantir_core::error::Error::io(path, source),
            Error::Json { path, detail } => palantir_core::error::Error::json(path, detail),
            Error::Format { path, detail } => palantir_core::error::Error::format(path, detail),
            Error::Http { url, detail, .. } => {
                palantir_core::error::Error::format(PathBuf::from(url), detail)
            }
            Error::Cancelled => palantir_core::error::Error::format(
                PathBuf::from("<cancelled>"),
                "the transfer was cancelled",
            ),
            Error::HashMismatch { path, expected, actual } => {
                palantir_core::error::Error::format(path, format!("sha256 mismatch: expected {expected}, got {actual}"))
            }
        }
    }
}

impl From<Error> for palantir_core::error::Error {
    /// Convert a local [`Error`] into the `palantir-core` seam (see
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
            Error::Http { url, detail, status } => {
                assert_eq!(url, "https://example.invalid/x.json");
                assert_eq!(detail, "timeout");
                // A transport problem never got an answer, so it has no status.
                // That distinction is what the retry policy reads.
                assert_eq!(status, None);
            }
            _ => panic!("expected Http"),
        }
        // And the other constructor keeps the code it was given.
        match Error::status("https://example.invalid/x.json", 503) {
            Error::Http { status, detail, .. } => {
                assert_eq!(status, Some(503));
                assert_eq!(detail, "http status 503");
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
        let core: palantir_core::error::Error = Error::io("/tmp/x", io_err).into_core();
        match core {
            palantir_core::error::Error::Io { path, .. } => {
                assert_eq!(path, PathBuf::from("/tmp/x"));
            }
            _ => panic!("expected core Io"),
        }
    }

    #[test]
    fn into_core_maps_json_to_core_json() {
        let core: palantir_core::error::Error = Error::json("/tmp/x.json", "boom").into_core();
        match core {
            palantir_core::error::Error::Json { detail, .. } => assert_eq!(detail, "boom"),
            _ => panic!("expected core Json"),
        }
    }

    #[test]
    fn into_core_maps_format_to_core_format() {
        let core: palantir_core::error::Error = Error::format("/tmp/x", "nope").into_core();
        match core {
            palantir_core::error::Error::Format { detail, .. } => assert_eq!(detail, "nope"),
            _ => panic!("expected core Format"),
        }
    }

    #[test]
    fn into_core_maps_http_to_core_format_with_url_as_path() {
        let core: palantir_core::error::Error =
            Error::http("https://example.invalid/a", "denied").into_core();
        match core {
            palantir_core::error::Error::Format { path, detail } => {
                assert_eq!(path, PathBuf::from("https://example.invalid/a"));
                assert_eq!(detail, "denied");
            }
            _ => panic!("expected core Format"),
        }
    }

    #[test]
    fn into_core_maps_hash_mismatch_to_core_format() {
        let core: palantir_core::error::Error =
            Error::hash_mismatch("/tmp/a.jar", "aa", "bb").into_core();
        match core {
            palantir_core::error::Error::Format { path, detail } => {
                assert_eq!(path, PathBuf::from("/tmp/a.jar"));
                assert!(detail.contains("aa") && detail.contains("bb"));
            }
            _ => panic!("expected core Format"),
        }
    }

    #[test]
    fn from_impl_delegates_to_into_core() {
        let core: palantir_core::error::Error = palantir_core::error::Error::from(
            Error::format("/tmp/x", "via-from"),
        );
        match core {
            palantir_core::error::Error::Format { detail, .. } => assert_eq!(detail, "via-from"),
            _ => panic!("expected core Format"),
        }
    }
}
