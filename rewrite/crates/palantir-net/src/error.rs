//! Failures, each naming the URL or file and what went wrong with it.
//!
//! Network errors are user-visible: "connection reset" without a URL leaves
//! someone guessing whether their network, the service, or the launcher is
//! at fault. Every variant here carries enough to say which.

use std::fmt;
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub enum Error {
    /// A metadata document could not be parsed or used.
    Core { source: palantir_core::Error },
    /// A file could not be read or written.
    Io { path: PathBuf, source: io::Error },
    /// A request failed. `status` is the HTTP status when the server
    /// answered at all; `why` says the rest (connection refused, timeout).
    Http {
        url: String,
        status: Option<u16>,
        why: String,
    },
    /// A completed transfer did not match the hash the metadata promised.
    /// The bytes are deleted: wrong bytes are worse than no bytes.
    Hash {
        path: PathBuf,
        expected: String,
        actual: String,
    },
    /// Structurally impossible input: a hash too short to bucket, a cache
    /// key that is not a safe file name.
    Invalid { what: &'static str, why: String },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Core { source } => write!(f, "{source}"),
            Error::Io { path, source } => {
                write!(f, "could not use {}: {source}", path.display())
            }
            Error::Http { url, status, why } => match status {
                Some(status) => write!(f, "{url} answered {status}: {why}"),
                None => write!(f, "{url} failed: {why}"),
            },
            Error::Hash {
                path,
                expected,
                actual,
            } => write!(
                f,
                "{} has hash {actual}, but the metadata promised {expected}",
                path.display()
            ),
            Error::Invalid { what, why } => write!(f, "{what} is not usable: {why}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Core { source } => Some(source),
            Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<palantir_core::Error> for Error {
    fn from(source: palantir_core::Error) -> Self {
        Error::Core { source }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
