//! Loader patch installation.
//!
//! Writes a Prism patch file (`patches/<uid>.json`) and registers the
//! component version in `mmc-pack.json` through
//! [`palantir_core::pack::PackProfile::set_version`].
//!
//! A patch *replaces* the metadata's version file for that uid rather than
//! adding to it, which is what makes it useful for a user override and makes it
//! the wrong tool for installing a loader. A loader is installed by registering
//! its uid with a version (`PackProfile::set_version`); its libraries, entry
//! point and requirements then come from the metadata service for that
//! uid/version pair, exactly as Minecraft's do.
//!
//! This module used to synthesize a loader patch from a table of main classes.
//! That produced a file with no `libraries` at all, so the loader jar and
//! everything it needs never reached the classpath, and two of the four entry
//! points were invented rather than copied. Nothing calls it any more, and the
//! table is gone with it.
//!
//! [`write_patch`] stays: it is the override path, and it round-trips through
//! [`palantir_core::version::VersionFile::parse`].

use std::borrow::Borrow;
use std::path::PathBuf;

/// Errors from patch installation.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A uid was empty where one is required.
    #[error("component uid must not be empty")]
    EmptyUid,
    /// Prism core failure (profile load/save, patch write, ...).
    #[error("core error: {0}")]
    Core(String),
    /// Filesystem failure with the path that caused it.
    #[error("io error for {path}: {source}")]
    Io {
        /// The path involved in the failed operation.
        path: PathBuf,
        /// Underlying OS error.
        #[source]
        source: std::io::Error,
    },
}

/// Result alias for install operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Helper to build an [`Error::Io`].
fn io_err(path: &std::path::Path, source: std::io::Error) -> Error {
    Error::Io { path: path.to_path_buf(), source }
}

/// Convert a [`palantir_core::error::Error`] into [`Error::Core`].
fn core_err(e: palantir_core::error::Error) -> Error {
    Error::Core(e.to_string())
}

/// Write a patch for `instance`.
///
/// Serializes `patch` in Prism document format to `patches/<uid>.json`
/// (creating `patches/` as needed) and registers the patch version from
/// `patch["version"]` in `mmc-pack.json` via `PackProfile::set_version`.
/// Accepts both owned and borrowed instances, uids and patch values.
pub fn write_patch(
    instance: impl Borrow<palantir_core::instance::Instance>,
    uid: impl AsRef<str>,
    patch: impl Borrow<serde_json::Value>,
) -> Result<()> {
    let instance = instance.borrow();
    let uid = uid.as_ref();
    let patch = patch.borrow();
    if uid.is_empty() {
        return Err(Error::EmptyUid);
    }
    let version = patch
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    if version.is_empty() {
        return Err(Error::Core("patch is missing a non-empty \"version\"".to_string()));
    }
    let patches_dir = instance.patches_dir();
    std::fs::create_dir_all(&patches_dir).map_err(|e| io_err(&patches_dir, e))?;
    let patch_path = patches_dir.join(format!("{uid}.json"));
    let text = palantir_core::json::to_document_string(patch).map_err(core_err)?;
    palantir_core::util::atomic_write(&patch_path, text.as_bytes()).map_err(core_err)?;
    let pack_path = instance.mmc_pack_path();
    let pack_text = palantir_core::util::read_text(&pack_path).map_err(core_err)?;
    let mut profile =
        palantir_core::pack::PackProfile::from_text(&pack_text, &pack_path).map_err(core_err)?;
    profile.set_version(uid, version.as_str(), false);
    profile.save(&pack_path).map_err(core_err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn parse_patch(value: &serde_json::Value) -> palantir_core::version::VersionFile {
        palantir_core::version::VersionFile::parse(value, Path::new("patch.json"), false).unwrap()
    }

    /// An override written by hand: the patch text is the user's, so the only
    /// things under test are the file layout and the component registration.
    fn override_patch() -> serde_json::Value {
        serde_json::json!({
            "formatVersion": 1,
            "uid": "net.fabricmc.fabric-loader",
            "name": "Fabric Loader (overridden)",
            "version": "0.16.9",
            "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "+traits": ["fabric"]
        })
    }

    #[test]
    fn write_patch_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let instance =
            palantir_core::instance::Instance::create(dir.path(), "Patch Test", "1.20.1").unwrap();
        let patch = override_patch();
        write_patch(&instance, "net.fabricmc.fabric-loader", &patch).unwrap();
        let patch_path = instance.patches_dir().join("net.fabricmc.fabric-loader.json");
        assert!(patch_path.is_file());
        let text = std::fs::read_to_string(&patch_path).unwrap();
        let back: serde_json::Value = serde_json::from_str(&text).unwrap();
        let f = parse_patch(&back);
        assert_eq!(f.uid, "net.fabricmc.fabric-loader");
        assert_eq!(f.version, "0.16.9");
        assert!(f.traits.contains("fabric"));
        let profile = palantir_core::pack::PackProfile::load(&instance.mmc_pack_path()).unwrap();
        assert_eq!(
            profile.get("net.fabricmc.fabric-loader").unwrap().version,
            "0.16.9"
        );
    }

    #[test]
    fn write_patch_accepts_owned_values() {
        let dir = tempfile::tempdir().unwrap();
        let instance =
            palantir_core::instance::Instance::create(dir.path(), "Owned", "1.20.1").unwrap();
        write_patch(instance, String::from("net.fabricmc.fabric-loader"), override_patch())
            .unwrap();
    }

    #[test]
    fn an_empty_uid_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let instance =
            palantir_core::instance::Instance::create(dir.path(), "Empty", "1.20.1").unwrap();
        assert!(matches!(
            write_patch(&instance, "", override_patch()),
            Err(Error::EmptyUid)
        ));
    }

    #[test]
    fn a_patch_without_a_version_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let instance =
            palantir_core::instance::Instance::create(dir.path(), "NoVer", "1.20.1").unwrap();
        let patch = serde_json::json!({ "uid": "net.fabricmc.fabric-loader" });
        assert!(matches!(
            write_patch(&instance, "net.fabricmc.fabric-loader", patch),
            Err(Error::Core(_))
        ));
        // Nothing half-written: a refused patch leaves no file behind.
        assert!(!instance
            .patches_dir()
            .join("net.fabricmc.fabric-loader.json")
            .exists());
    }
}
