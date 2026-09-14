//! Version metadata model, library/rules handling and the merge engine.
//!
//! Mirrors `minecraft/OneSixVersionFormat.cpp`, `MojangVersionFormat.cpp`,
//! `VersionFile.cpp`, `LaunchProfile.cpp`, `Library.cpp`, `Rule.cpp`,
//! `GradleSpecifier.h` and `Version.cpp`.

pub mod compare;
pub mod gradle;
pub mod library;
pub mod profile;
pub mod rules;
pub mod version_file;

pub use compare::PalantirVersion;
pub use gradle::GradleSpecifier;
pub use library::{ApplicableFiles, DownloadInfo, Library, LibraryDownloads};
pub use profile::LaunchProfile;
pub use rules::{Action, Applied, OsSpec, Rule, RuntimeContext};
pub use version_file::VersionFile;

/// Severity of a problem, ordered `None < Warning < Error`
/// (`ProblemSeverity`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum ProblemSeverity {
    /// No problem.
    #[default]
    None,
    /// Warning — instance still launches.
    Warning,
    /// Error — launch should be refused.
    Error,
}

/// One reported problem on a component or profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// How severe the problem is.
    pub severity: ProblemSeverity,
    /// Human-readable description.
    pub message: String,
}

/// Asset index descriptor (`MojangAssetIndexInfo`): the `assetIndex` object
/// of a version file plus the `known` flag set when the object was actually
/// present (vs. reconstructed from a bare `assets` id).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssetIndexInfo {
    /// Optional storage path.
    pub path: Option<String>,
    /// SHA-1 of the index file.
    pub sha1: String,
    /// Compressed size in bytes.
    pub size: i64,
    /// Download URL.
    pub url: String,
    /// Total uncompressed size in bytes.
    pub total_size: i64,
    /// Asset index id (`assets` name), e.g. `17` or `legacy`.
    pub id: String,
    /// Whether the full descriptor was present in the JSON.
    pub known: bool,
}

impl AssetIndexInfo {
    /// A bare, not-downloaded index id (used for `assets: "x"` without an
    /// `assetIndex` object, and for the `legacy` fallback).
    pub fn bare(id: &str) -> AssetIndexInfo {
        AssetIndexInfo { id: id.to_string(), known: false, ..Default::default() }
    }
}

/// A `-javaagent:` entry (`Agent`): a library plus an optional agent
/// argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agent {
    /// The agent jar.
    pub library: Library,
    /// Optional `=argument` for `-javaagent:jar=arg`.
    pub argument: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn problem_severity_orders_none_warning_error() {
        assert!(ProblemSeverity::None < ProblemSeverity::Warning);
        assert!(ProblemSeverity::Warning < ProblemSeverity::Error);
        assert_eq!(ProblemSeverity::default(), ProblemSeverity::None);
    }

    #[test]
    fn bare_asset_index_is_not_known() {
        let a = AssetIndexInfo::bare("legacy");
        assert_eq!(a.id, "legacy");
        assert!(!a.known);
        assert_eq!(a.sha1, "");
    }
}
