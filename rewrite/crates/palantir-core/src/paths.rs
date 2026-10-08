//! Where everything lives on disk: one root, one layout, written down.
//!
//! The layout is ours (the game's own file formats are specified; where a
//! launcher keeps them is not). One root holds everything the launcher owns,
//! shared between versions wherever the game itself shares it:
//!
//! ```text
//! <root>/
//!   instances/<id>/instance.json   our own instance descriptor
//!   instances/<id>/game/           that instance's game directory
//!   versions/<id>/<id>.json        version metadata, kept for re-launch
//!   versions/<id>/<id>.jar         the client jar
//!   libraries/<maven layout>       shared across versions (they are)
//!   assets/indexes/<id>.json       asset index documents
//!   assets/objects/<hh>/<hash>     the shared object store
//!   assets/virtual/<id>/           per-index trees for virtual indexes
//!   content/<hh>/<hash>            the hash-keyed content store
//!   java/<component>/              Java runtimes, by Mojang component name
//!   cache/                         partial downloads and metadata cache
//! ```
//!
//! Two reasons for the split: libraries and asset objects are byte-identical
//! across versions (N versions must not mean N copies of lwjgl), and
//! everything resumable or regenerable lives under `cache/` where losing it
//! costs nothing.
//!
//! Metadata names files *relative* to these roots (Maven paths, object
//! hashes). Those names arrive from the network, so every join here refuses
//! absolute paths and `..` rather than trusting them.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// The application directory name under the platform's data directory.
pub const APP_DIR_WINDOWS: &str = "PalantirMC";
pub const APP_DIR_MACOS: &str = "PalantirMC";
pub const APP_DIR_LINUX: &str = "palantirmc";

/// The launcher's data root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataRoot {
    pub root: PathBuf,
}

impl DataRoot {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Where the launcher's data lives on the running system:
    ///
    /// - Windows: `%LOCALAPPDATA%\PalantirMC`. Local, not roaming: a game
    ///   tree is gigabytes, and a roaming profile would carry it across the
    ///   network on every sign-in.
    /// - macOS: `~/Library/Application Support/PalantirMC`.
    /// - Linux: `$XDG_DATA_HOME/palantirmc`, else `~/.local/share/palantirmc`.
    pub fn platform_default() -> Result<Self> {
        let root = if cfg!(target_os = "windows") {
            let base = env_dir("LOCALAPPDATA")
                .or_else(|| env_dir("APPDATA"))
                .ok_or(Error::Invalid {
                    what: "data root",
                    why: "neither %LOCALAPPDATA% nor %APPDATA% is set".to_string(),
                })?;
            base.join(APP_DIR_WINDOWS)
        } else if cfg!(target_os = "macos") {
            home_dir()?
                .join("Library")
                .join("Application Support")
                .join(APP_DIR_MACOS)
        } else {
            match env_dir("XDG_DATA_HOME") {
                Some(base) => base.join(APP_DIR_LINUX),
                None => home_dir()?.join(".local").join("share").join(APP_DIR_LINUX),
            }
        };
        Ok(Self::new(root))
    }

    pub fn instances_dir(&self) -> PathBuf {
        self.root.join("instances")
    }

    pub fn instance_dir(&self, id: &str) -> PathBuf {
        self.instances_dir().join(id)
    }

    /// Our own instance descriptor (phase 5 fills it in).
    pub fn instance_descriptor(&self, id: &str) -> PathBuf {
        self.instance_dir(id).join("instance.json")
    }

    /// The instance's game directory: where the game reads and writes.
    pub fn game_dir(&self, id: &str) -> PathBuf {
        self.instance_dir(id).join("game")
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.root.join("versions")
    }

    pub fn version_json(&self, id: &str) -> PathBuf {
        self.versions_dir().join(id).join(format!("{id}.json"))
    }

    pub fn version_jar(&self, id: &str) -> PathBuf {
        self.versions_dir().join(id).join(format!("{id}.jar"))
    }

    pub fn libraries_dir(&self) -> PathBuf {
        self.root.join("libraries")
    }

    /// One library file, by its Maven-layout path from the metadata.
    pub fn library_file(&self, rel_path: &str) -> Result<PathBuf> {
        join_checked(&self.libraries_dir(), rel_path, "library path")
    }

    pub fn assets_dir(&self) -> PathBuf {
        self.root.join("assets")
    }

    pub fn asset_indexes_dir(&self) -> PathBuf {
        self.assets_dir().join("indexes")
    }

    pub fn asset_index_file(&self, id: &str) -> PathBuf {
        self.asset_indexes_dir().join(format!("{id}.json"))
    }

    pub fn asset_objects_dir(&self) -> PathBuf {
        self.assets_dir().join("objects")
    }

    /// One object in the shared store, by its store path (`hh/hash`).
    pub fn asset_object_file(&self, store_rel_path: &str) -> Result<PathBuf> {
        join_checked(&self.asset_objects_dir(), store_rel_path, "asset object")
    }

    /// A virtual index's tree, which the game reads as its asset root.
    pub fn virtual_assets_dir(&self, index_id: &str) -> PathBuf {
        self.assets_dir().join("virtual").join(index_id)
    }

    /// The hash-keyed content store (phase 2's resumable downloads).
    pub fn content_dir(&self) -> PathBuf {
        self.root.join("content")
    }

    /// Java runtimes, one directory per Mojang component name.
    pub fn java_dir(&self) -> PathBuf {
        self.root.join("java")
    }

    /// Partial downloads and the metadata cache: disposable by design.
    pub fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }
}

/// A name from metadata joined under a root -- unless it tries to leave it.
///
/// The rules are string rules on purpose, so they mean the same on every
/// platform -- path semantics do not (`/etc/passwd` is not absolute on
/// Windows yet still leaves the root, and `C:\evil` is one harmless name on
/// Unix). The Maven layout needs only forward slashes and never a colon,
/// and a colon or backslash cannot appear in a Windows filename at all, so
/// refusing them outright loses nothing and closes the drive/UNC/ADS
/// shapes. `..` climbs everywhere, so it is refused everywhere.
fn join_checked(root: &Path, rel: &str, what: &'static str) -> Result<PathBuf> {
    let escapes = rel.is_empty()
        || rel.starts_with(['/', '\\'])
        || rel.contains('\\')
        || rel.contains(':')
        || rel
            .split(['/', '\\'])
            .any(|component| component == ".." || component.is_empty());
    if escapes {
        return Err(Error::Invalid {
            what,
            why: format!("{rel:?} would escape its root"),
        });
    }
    Ok(root.join(rel))
}

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn home_dir() -> Result<PathBuf> {
    env_dir("HOME")
        .or_else(|| env_dir("USERPROFILE"))
        .ok_or_else(|| Error::Invalid {
            what: "data root",
            why: "no home directory is set".to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_layout_is_the_written_one() {
        let root = DataRoot::new("/data");
        assert_eq!(
            root.instance_descriptor("a"),
            PathBuf::from("/data/instances/a/instance.json")
        );
        assert_eq!(root.game_dir("a"), PathBuf::from("/data/instances/a/game"));
        assert_eq!(
            root.version_json("1.20.1"),
            PathBuf::from("/data/versions/1.20.1/1.20.1.json")
        );
        assert_eq!(
            root.version_jar("1.20.1"),
            PathBuf::from("/data/versions/1.20.1/1.20.1.jar")
        );
        assert_eq!(
            root.asset_index_file("1.12"),
            PathBuf::from("/data/assets/indexes/1.12.json")
        );
        assert_eq!(
            root.virtual_assets_dir("pre-1.6"),
            PathBuf::from("/data/assets/virtual/pre-1.6")
        );
    }

    #[test]
    fn metadata_paths_are_joined_not_trusted() {
        let root = DataRoot::new("/data");
        assert_eq!(
            root.library_file("com/example/x/1/x-1.jar").unwrap(),
            PathBuf::from("/data/libraries/com/example/x/1/x-1.jar")
        );
        assert!(root.library_file("../../etc/passwd").is_err());
        assert!(root.library_file("/etc/passwd").is_err());
        assert!(root.library_file("..\\..\\evil.dll").is_err());
        assert!(root.library_file("C:\\evil.dll").is_err());
        assert!(root.library_file("file:stream").is_err());
        assert!(root.library_file("").is_err());
        assert!(root.asset_object_file("ab/hash").is_ok());
        assert!(root.asset_object_file("ab/../hash").is_err());
    }

    #[test]
    fn the_platform_default_names_our_directory() {
        // Which directory it picks depends on the machine; that it lands in
        // the application's own folder does not.
        let root = DataRoot::platform_default().unwrap();
        let leaf = root
            .root
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let expected = if cfg!(target_os = "windows") {
            APP_DIR_WINDOWS
        } else if cfg!(target_os = "macos") {
            APP_DIR_MACOS
        } else {
            APP_DIR_LINUX
        };
        assert_eq!(leaf, expected, "unexpected data root {:?}", root.root);
    }
}
