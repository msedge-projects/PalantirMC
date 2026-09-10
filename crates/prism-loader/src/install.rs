//! Loader patch planning and installation.
//!
//! Builds Prism patch JSON (`patches/<uid>.json`) for Fabric, Forge,
//! NeoForge and Quilt and registers the version in `mmc-pack.json` via
//! [`prism_core::pack::PackProfile::set_version`]. Output round-trips
//! through [`prism_core::version::VersionFile::parse`].

use std::borrow::Borrow;
use std::path::PathBuf;

/// Errors from loader patch planning and installation.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Unknown loader uid.
    #[error("unknown loader uid: {0}")]
    UnknownLoader(String),
    /// JSON serialization failure.
    #[error("json error: {0}")]
    Json(String),
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

/// Convert a [`prism_core::error::Error`] into [`Error::Core`].
fn core_err(e: prism_core::error::Error) -> Error {
    Error::Core(e.to_string())
}

/// Plan a loader install, returning the patch JSON value.
///
/// `uid` must be one of `net.fabricmc.fabric-loader`, `net.minecraftforge`,
/// `net.neoforged` or `org.quiltmc.quilt-loader`. The value carries `uid`,
/// `version` (the loader version), `formatVersion`, a loader-specific
/// `mainClass`, `+traits`/`+tweakers` where applicable and a `requires`
/// pin on `net.minecraft` `equals` `game_version`. The result parses with
/// [`prism_core::version::VersionFile::parse`].
pub fn plan_loader_install(
    uid: impl AsRef<str>,
    game_version: impl AsRef<str>,
    loader_version: impl AsRef<str>,
) -> Result<serde_json::Value> {
    let uid = uid.as_ref();
    let game_version = game_version.as_ref();
    let loader_version = loader_version.as_ref();
    if uid.is_empty() {
        return Err(Error::UnknownLoader(uid.to_string()));
    }
    if loader_version.is_empty() {
        return Err(Error::Core("loader version must not be empty".to_string()));
    }
    if game_version.is_empty() {
        return Err(Error::Core("game version must not be empty".to_string()));
    }
    let requires = serde_json::json!([{"uid": "net.minecraft", "equals": game_version}]);
    let value = match uid {
        "net.fabricmc.fabric-loader" => serde_json::json!({
            "formatVersion": 1,
            "uid": uid,
            "name": "Fabric Loader",
            "version": loader_version,
            "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "+traits": ["fabric"],
            "requires": requires
        }),
        "net.minecraftforge" => serde_json::json!({
            "formatVersion": 1,
            "uid": uid,
            "name": "Forge",
            "version": loader_version,
            "mainClass": "net.minecraft.launchwrapper.Launch",
            "+tweakers": ["net.minecraftforge.fml.common.launcher.FMLTweaker"],
            "requires": requires
        }),
        "net.neoforged" => serde_json::json!({
            "formatVersion": 1,
            "uid": uid,
            "name": "NeoForge",
            "version": loader_version,
            "mainClass": "cpw.mods.bootstraplauncher.BootstrapLauncher",
            "requires": requires
        }),
        "org.quiltmc.quilt-loader" => serde_json::json!({
            "formatVersion": 1,
            "uid": uid,
            "name": "Quilt Loader",
            "version": loader_version,
            "mainClass": "org.quiltmc.loader.impl.launch.knot.KnotClient",
            "+traits": ["quilt"],
            "requires": requires
        }),
        _ => return Err(Error::UnknownLoader(uid.to_string())),
    };
    Ok(value)
}

/// Write a loader patch for `instance`.
///
/// Serializes `patch` in Prism document format to `patches/<uid>.json`
/// (creating `patches/` as needed) and registers the patch version from
/// `patch["version"]` in `mmc-pack.json` via `PackProfile::set_version`.
/// Accepts both owned and borrowed instances, uids and patch values.
pub fn write_patch(
    instance: impl Borrow<prism_core::instance::Instance>,
    uid: impl AsRef<str>,
    patch: impl Borrow<serde_json::Value>,
) -> Result<()> {
    let instance = instance.borrow();
    let uid = uid.as_ref();
    let patch = patch.borrow();
    if uid.is_empty() {
        return Err(Error::UnknownLoader(uid.to_string()));
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
    let text = prism_core::json::to_document_string(patch).map_err(core_err)?;
    prism_core::util::atomic_write(&patch_path, text.as_bytes()).map_err(core_err)?;
    let pack_path = instance.mmc_pack_path();
    let pack_text = prism_core::util::read_text(&pack_path).map_err(core_err)?;
    let mut profile =
        prism_core::pack::PackProfile::from_text(&pack_text, &pack_path).map_err(core_err)?;
    profile.set_version(uid, version.as_str(), false);
    profile.save(&pack_path).map_err(core_err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn parse_patch(value: &serde_json::Value) -> prism_core::version::VersionFile {
        prism_core::version::VersionFile::parse(value, Path::new("patch.json"), false).unwrap()
    }

    #[test]
    fn fabric_shape_round_trips() {
        let v = plan_loader_install("net.fabricmc.fabric-loader", "1.20.1", "0.16.9").unwrap();
        assert_eq!(v["uid"], "net.fabricmc.fabric-loader");
        assert_eq!(v["version"], "0.16.9");
        assert_eq!(v["mainClass"], "net.fabricmc.loader.impl.launch.knot.KnotClient");
        assert!(v["+traits"].as_array().unwrap().iter().any(|t| t == "fabric"));
        let f = parse_patch(&v);
        assert_eq!(f.uid, "net.fabricmc.fabric-loader");
        assert_eq!(f.version, "0.16.9");
        assert_eq!(f.main_class, "net.fabricmc.loader.impl.launch.knot.KnotClient");
        assert!(f.traits.contains("fabric"));
    }

    #[test]
    fn forge_shape_round_trips() {
        let v = plan_loader_install("net.minecraftforge", "1.20.1", "47.2.0").unwrap();
        assert_eq!(v["uid"], "net.minecraftforge");
        assert_eq!(v["version"], "47.2.0");
        assert!(v["+tweakers"].as_array().unwrap().len() > 0);
        let f = parse_patch(&v);
        assert_eq!(f.uid, "net.minecraftforge");
        assert!(!f.add_tweakers.is_empty());
    }

    #[test]
    fn neoforge_and_quilt_shapes_round_trip() {
        let neo = plan_loader_install("net.neoforged", "1.20.1", "21.1.0").unwrap();
        assert_eq!(neo["uid"], "net.neoforged");
        let f = parse_patch(&neo);
        assert_eq!(f.uid, "net.neoforged");
        assert!(!f.main_class.is_empty());

        let quilt = plan_loader_install("org.quiltmc.quilt-loader", "1.20.1", "0.25.0").unwrap();
        assert_eq!(quilt["uid"], "org.quiltmc.quilt-loader");
        assert_eq!(
            quilt["mainClass"],
            "org.quiltmc.loader.impl.launch.knot.KnotClient"
        );
        let q = parse_patch(&quilt);
        assert!(q.traits.contains("quilt"));
    }

    #[test]
    fn unknown_loader_is_an_error() {
        assert!(matches!(
            plan_loader_install("unknown.loader", "1.20.1", "1.0"),
            Err(Error::UnknownLoader(_))
        ));
    }

    #[test]
    fn write_patch_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let instance =
            prism_core::instance::Instance::create(dir.path(), "Patch Test", "1.20.1").unwrap();
        let patch =
            plan_loader_install("net.fabricmc.fabric-loader", "1.20.1", "0.16.9").unwrap();
        write_patch(&instance, "net.fabricmc.fabric-loader", &patch).unwrap();
        let patch_path = instance.patches_dir().join("net.fabricmc.fabric-loader.json");
        assert!(patch_path.is_file());
        let text = std::fs::read_to_string(&patch_path).unwrap();
        let back: serde_json::Value = serde_json::from_str(&text).unwrap();
        let f = parse_patch(&back);
        assert_eq!(f.version, "0.16.9");
        let profile =
            prism_core::pack::PackProfile::load(&instance.mmc_pack_path()).unwrap();
        assert_eq!(
            profile.get("net.fabricmc.fabric-loader").unwrap().version,
            "0.16.9"
        );
    }

    #[test]
    fn write_patch_accepts_owned_values() {
        let dir = tempfile::tempdir().unwrap();
        let instance =
            prism_core::instance::Instance::create(dir.path(), "Owned", "1.20.1").unwrap();
        let patch = plan_loader_install(
            String::from("net.neoforged"),
            String::from("1.20.1"),
            String::from("21.1.0"),
        )
        .unwrap();
        write_patch(instance, String::from("net.neoforged"), patch).unwrap();
    }
}
