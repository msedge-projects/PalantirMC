//! Failures of an install, each naming what could not be installed and why.
//!
//! Same rule as the layers below: an error someone reads must say whether
//! the fault was the metadata, the network, the disk, or a request that
//! made no sense -- and point at the thing at fault.

use std::fmt;
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub enum Error {
    /// A metadata document could not be parsed or made coherent
    /// (an inheritance chain that does not resolve, a merge that does not
    /// apply).
    Core { source: palantir_core::Error },
    /// The network layer: transfers, caches, the store.
    Net { source: palantir_net::Error },
    /// A file could not be read or written.
    Io { path: PathBuf, source: io::Error },
    /// Structurally impossible input: a version the manifest does not
    /// list, a request with no meaning as stated.
    Invalid { what: &'static str, why: String },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Core { source } => write!(f, "{source}"),
            Error::Net { source } => write!(f, "{source}"),
            Error::Io { path, source } => {
                write!(f, "could not use {}: {source}", path.display())
            }
            Error::Invalid { what, why } => write!(f, "{what} is not usable: {why}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Core { source } => Some(source),
            Error::Net { source } => Some(source),
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

impl From<palantir_net::Error> for Error {
    fn from(source: palantir_net::Error) -> Self {
        Error::Net { source }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
