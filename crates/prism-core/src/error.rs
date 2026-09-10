//! Error types. Mirrors Prism's error surfacing: every failure carries the
//! file it came from (see `INISettingsObject`, `Json`, `PackProfile` logging).

use std::path::PathBuf;

/// All fallible operations in this crate return `Result<T, Error>`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Filesystem failure, with the path that caused it.
    #[error("io error for {path}: {source}")]
    Io {
        /// The path involved in the failed operation.
        path: PathBuf,
        /// Underlying OS error.
        #[source]
        source: std::io::Error,
    },

    /// Malformed or unparsable INI content.
    #[error("invalid ini in {path}{at}: {detail}")]
    Ini {
        /// The INI file path.
        path: PathBuf,
        /// `line N` if known.
        at: String,
        /// What was wrong.
        detail: String,
    },

    /// Malformed or unparsable JSON content.
    #[error("invalid json in {path}: {detail}")]
    Json {
        /// The JSON file path.
        path: PathBuf,
        /// Parser message.
        detail: String,
    },

    /// Parsed fine but the format is unsupported (e.g. wrong formatVersion).
    #[error("unsupported format in {path}: {detail}")]
    Format {
        /// The file path.
        path: PathBuf,
        /// What is unsupported.
        detail: String,
    },

    /// A required JSON/INI field is missing.
    #[error("missing required field '{field}' in {path}")]
    MissingField {
        /// The file path.
        path: PathBuf,
        /// The missing field name.
        field: &'static str,
    },

    /// Instance folder name is illegal or already present.
    #[error("invalid instance folder name: {0}")]
    InvalidInstanceName(String),

    /// The instance does not exist at the given path.
    #[error("instance not found: {0}")]
    InstanceNotFound(PathBuf),

    /// Metadata (component) could not be resolved.
    #[error("cannot resolve component '{uid}' version '{version}': {detail}")]
    Resolve {
        /// Component uid.
        uid: String,
        /// Component version (may be empty).
        version: String,
        /// Reason.
        detail: String,
    },

    /// Name is empty after sanitization.
    #[error("name is empty after sanitization")]
    EmptyName,
}

impl Error {
    /// Wrap an [`std::io::Error`] with path context.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io { path: path.into(), source }
    }

    /// Build an INI error with an optional line number.
    pub fn ini(path: impl Into<PathBuf>, line: Option<usize>, detail: impl Into<String>) -> Self {
        Error::Ini {
            path: path.into(),
            at: line.map(|l| format!(" (line {l})")).unwrap_or_default(),
            detail: detail.into(),
        }
    }

    /// Build a JSON error from a serde message.
    pub fn json(path: impl Into<PathBuf>, detail: impl Into<String>) -> Self {
        Error::Json { path: path.into(), detail: detail.into() }
    }

    /// Build a format error.
    pub fn format(path: impl Into<PathBuf>, detail: impl Into<String>) -> Self {
        Error::Format { path: path.into(), detail: detail.into() }
    }
}

/// Crate result alias.
pub type Result<T> = std::result::Result<T, Error>;
