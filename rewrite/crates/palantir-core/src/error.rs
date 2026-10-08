//! Failures, each naming what was being read and what was wrong with it.
//!
//! A launcher's error messages are user-visible: when metadata cannot be
//! read, the person waiting to play needs to know which document failed and
//! why, not that something went wrong somewhere.

use std::fmt;
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub enum Error {
    /// A file could not be read or written.
    Io { path: PathBuf, source: io::Error },
    /// A metadata document is not the JSON it claims to be.
    Parse {
        what: &'static str,
        source: serde_json::Error,
    },
    /// Structurally valid metadata that says something impossible -- an
    /// unknown placeholder in a launch argument, a hash too short to split.
    Invalid { what: &'static str, why: String },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io { path, source } => {
                write!(f, "could not read {}: {source}", path.display())
            }
            Error::Parse { what, source } => write!(f, "{what} is not valid: {source}"),
            Error::Invalid { what, why } => write!(f, "{what} is not usable: {why}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
            Error::Parse { source, .. } => Some(source),
            Error::Invalid { .. } => None,
        }
    }
}

impl Error {
    /// Wrap a serde failure with the name of the document it belongs to.
    pub(crate) fn parse(what: &'static str) -> impl Fn(serde_json::Error) -> Error {
        move |source| Error::Parse { what, source }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
