//! Modpack detection and offline import.
//!
//! Supports Modrinth (`.mrpack`, `modrinth.index.json`) and CurseForge
//! (`manifest.json`) packs. Imports create a Prism instance, write the
//! `overrides` tree into the instance root and register the loader in
//! `mmc-pack.json`. No network access is performed: remote files listed in
//! the indexes are not downloaded.

use std::io::Read as _;
use std::path::{Component, Path, PathBuf};

/// Supported modpack container formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PackFormat {
    /// Modrinth pack (`modrinth.index.json` present).
    MrPack,
    /// CurseForge pack (`manifest.json` present).
    CurseForge,
    /// Neither marker found or the zip cannot be opened.
    #[default]
    Unknown,
}

/// Errors from modpack detection and import.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Filesystem failure with the path that caused it.
    #[error("io error for {path}: {source}")]
    Io {
        /// The path involved in the failed operation.
        path: PathBuf,
        /// Underlying OS error.
        #[source]
        source: std::io::Error,
    },
    /// Zip container failure.
    #[error("zip error: {0}")]
    Zip(String),
    /// JSON parsing failure.
    #[error("json error: {0}")]
    Json(String),
    /// Prism core failure (instance creation, profile save, ...).
    #[error("core error: {0}")]
    Core(String),
    /// Archive entry would escape the instance root.
    #[error("unsafe archive path: {0}")]
    UnsafePath(String),
    /// Pack index is missing or malformed.
    #[error("invalid pack: {0}")]
    InvalidPack(String),
}

/// Result alias for modpack operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Helper to build an [`Error::Io`].
fn io_err(path: &Path, source: std::io::Error) -> Error {
    Error::Io { path: path.to_path_buf(), source }
}

/// Convert a [`palantir_core::error::Error`] into a local [`Error::Core`].
fn core_err(e: palantir_core::error::Error) -> Error {
    Error::Core(e.to_string())
}

/// Detect the pack format from raw zip bytes.
///
/// Returns [`PackFormat::MrPack`] when `modrinth.index.json` is present,
/// [`PackFormat::CurseForge`] when `manifest.json` is present (and no
/// Modrinth index), and [`PackFormat::Unknown`] otherwise, including when
/// the bytes are not a zip archive at all.
pub fn detect_format(zip_bytes: impl AsRef<[u8]>) -> PackFormat {
    let data = zip_bytes.as_ref();
    let cursor = std::io::Cursor::new(data);
    let mut archive = match zip::ZipArchive::new(cursor) {
        Ok(a) => a,
        Err(_) => return PackFormat::Unknown,
    };
    let mut has_mrpack = false;
    let mut has_curse = false;
    let len = archive.len();
    let mut index: usize = 0;
    while index < len {
        let name = match archive.by_index(index) {
            Ok(f) => f.name().to_owned(),
            Err(_) => {
                index += 1;
                continue;
            }
        };
        if name == "modrinth.index.json" {
            has_mrpack = true;
        } else if name == "manifest.json" {
            has_curse = true;
        }
        index += 1;
    }
    if has_mrpack {
        PackFormat::MrPack
    } else if has_curse {
        PackFormat::CurseForge
    } else {
        PackFormat::Unknown
    }
}

/// Read a named file from a zip archive into a string.
fn read_zip_text(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    name: &str,
) -> Result<String> {
    let mut file = archive
        .by_name(name)
        .map_err(|e| Error::Zip(e.to_string()))?;
    let mut text = String::new();
    file.read_to_string(&mut text)
        .map_err(|e| Error::Io { path: PathBuf::from(name), source: e })?;
    Ok(text)
}

/// Join an overrides entry to the instance root with traversal protection.
fn join_safe(root: &Path, rel: &Path) -> Result<PathBuf> {
    if rel.is_absolute() {
        return Err(Error::UnsafePath(rel.to_string_lossy().into_owned()));
    }
    for comp in rel.components() {
        match comp {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(Error::UnsafePath(rel.to_string_lossy().into_owned()));
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }
    Ok(root.join(rel))
}

/// Extract entries under `overrides_dir/` from `archive` into `instance_root`.
#[allow(clippy::too_many_lines)]
fn extract_overrides(
    archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>,
    overrides_dir: &str,
    instance_root: &Path,
) -> Result<()> {
    let prefix = if overrides_dir.is_empty() {
        String::new()
    } else {
        let trimmed = overrides_dir.trim_matches('/');
        if trimmed.is_empty() {
            String::new()
        } else {
            let mut p = trimmed.to_string();
            p.push('/');
            p
        }
    };
    if prefix.is_empty() {
        return Ok(());
    }
    let len = archive.len();
    let mut index: usize = 0;
    while index < len {
        let (raw_name, is_dir, unix_mode) = {
            let file = archive.by_index(index).map_err(|e| Error::Zip(e.to_string()))?;
            (file.name().to_owned(), file.is_dir(), file.unix_mode())
        };
        // Silence unused-variable on non-unix targets where permissions are a no-op.
        #[cfg(not(unix))]
        let _ = unix_mode;
        let rel_str = match raw_name.strip_prefix(prefix.as_str()) {
            Some(rest) => rest,
            None => {
                index += 1;
                continue;
            }
        };
        if rel_str.is_empty() {
            index += 1;
            continue;
        }
        // Zip entry names always use `/`; reject backslash tricks on Windows
        // by treating them as ordinary characters (Path will handle them).
        let rel_path = Path::new(rel_str);
        let out_path = join_safe(instance_root, rel_path)?;
        if is_dir || raw_name.ends_with('/') {
            std::fs::create_dir_all(&out_path).map_err(|e| io_err(&out_path, e))?;
        } else {
            if let Some(parent) = out_path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent).map_err(|e| io_err(parent, e))?;
                }
            }
            let mut file = archive.by_index(index).map_err(|e| Error::Zip(e.to_string()))?;
            let mut out =
                std::fs::File::create(&out_path).map_err(|e| io_err(&out_path, e))?;
            std::io::copy(&mut file, &mut out).map_err(|e| io_err(&out_path, e))?;
            #[cfg(unix)]
            {
                if let Some(mode) = unix_mode {
                    use std::os::unix::fs::PermissionsExt as _;
                    let perms = std::fs::Permissions::from_mode(mode);
                    std::fs::set_permissions(&out_path, perms)
                        .map_err(|e| io_err(&out_path, e))?;
                }
            }
        }
        index += 1;
    }
    Ok(())
}

/// Map a Modrinth dependency key to a Prism component uid.
fn mrpack_loader_uid(key: &str) -> Option<&'static str> {
    let lower = key.to_lowercase();
    match lower.as_str() {
        "fabric-loader" => Some("net.fabricmc.fabric-loader"),
        "quilt-loader" => Some("org.quiltmc.quilt-loader"),
        "forge" => Some("net.minecraftforge"),
        "neoforge" | "neoforged" => Some("net.neoforged"),
        _ => None,
    }
}

/// Parse a CurseForge `modLoaders` id (`forge-47.2.0`, `fabric-0.16.9`, ...).
///
/// Returns `(component_uid, loader_version)`. A trailing `-<mc_version>`
/// suffix (e.g. `fabric-0.16.9-1.20.1`) is stripped to the loader version.
fn parse_curse_loader_id(id: &str, mc_version: &str) -> Option<(String, String)> {
    let (prefix, rest) = id.split_once('-')?;
    if rest.is_empty() {
        return None;
    }
    let lower = prefix.to_lowercase();
    let uid = match lower.as_str() {
        "forge" => "net.minecraftforge",
        "neoforge" | "neoforged" => "net.neoforged",
        "fabric" => "net.fabricmc.fabric-loader",
        "quilt" => "org.quiltmc.quilt-loader",
        _ => return None,
    };
    let mut version = rest.to_string();
    if !mc_version.is_empty() {
        let suffix = format!("-{mc_version}");
        if let Some(stripped) = version.strip_suffix(suffix.as_str()) {
            version = stripped.to_string();
        }
    }
    if version.is_empty() {
        return None;
    }
    Some((uid.to_string(), version))
}

/// Register loader components in the instance's `mmc-pack.json`.
fn register_loaders(instance: &palantir_core::instance::Instance, loaders: &[(String, String)]) -> Result<()> {
    if loaders.is_empty() {
        return Ok(());
    }
    let path = instance.mmc_pack_path();
    let text = palantir_core::util::read_text(&path).map_err(core_err)?;
    let mut profile =
        palantir_core::pack::PackProfile::from_text(&text, &path).map_err(core_err)?;
    for (uid, version) in loaders {
        profile.set_version(uid.as_str(), version.as_str(), false);
    }
    profile.save(&path).map_err(core_err)?;
    Ok(())
}

/// Import a Modrinth (`.mrpack`) pack from memory.
///
/// Creates an instance named `name` in `instances_dir` with the Minecraft
/// version from `modrinth.index.json` (`dependencies.minecraft`), writes the
/// `overrides` tree into the instance root and registers any
/// `fabric-loader`/`quilt-loader`/`forge`/`neoforge` dependency in
/// `mmc-pack.json`. Returns the new instance root. Fully offline: remote
/// files are not downloaded.
pub fn import_mrpack(
    zip_bytes: impl AsRef<[u8]>,
    instances_dir: impl AsRef<Path>,
    name: impl AsRef<str>,
) -> Result<PathBuf> {
    let data = zip_bytes.as_ref();
    let instances_dir = instances_dir.as_ref();
    let name = name.as_ref();
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(data)).map_err(|e| Error::Zip(e.to_string()))?;
    let index_text = read_zip_text(&mut archive, "modrinth.index.json").map_err(|e| match e {
        Error::Zip(_) => Error::InvalidPack("missing modrinth.index.json".to_string()),
        other => other,
    })?;
    let index: serde_json::Value =
        serde_json::from_str(&index_text).map_err(|e| Error::Json(e.to_string()))?;
    let mc_version = index
        .get("dependencies")
        .and_then(|d| d.get("minecraft"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::InvalidPack("modrinth.index.json missing dependencies.minecraft".to_string()))?
        .to_string();
    if mc_version.is_empty() {
        return Err(Error::InvalidPack("empty dependencies.minecraft".to_string()));
    }
    let overrides_dir = index
        .get("overrides")
        .and_then(|v| v.as_str())
        .unwrap_or("overrides")
        .to_string();
    let instance = palantir_core::instance::Instance::create(instances_dir, name, mc_version.as_str())
        .map_err(core_err)?;
    extract_overrides(&mut archive, overrides_dir.as_str(), instance.root())?;
    let mut loaders: Vec<(String, String)> = Vec::new();
    if let Some(deps) = index.get("dependencies").and_then(|v| v.as_object()) {
        for (key, value) in deps {
            if let Some(uid) = mrpack_loader_uid(key.as_str()) {
                if let Some(version) = value.as_str() {
                    if !version.is_empty() {
                        loaders.push((uid.to_string(), version.to_string()));
                    }
                }
            }
        }
    }
    register_loaders(&instance, &loaders)?;
    Ok(instance.root().to_path_buf())
}

/// Import a CurseForge pack from memory.
///
/// Creates an instance named `name` in `instances_dir` with the Minecraft
/// version from `manifest.json` (`minecraft.version`), writes the
/// `overrides` tree into the instance root and registers the primary (or
/// first) `minecraft.modLoaders` entry (`forge-`, `fabric-`, `quilt-`,
/// `neoforge-`) in `mmc-pack.json`. Returns the new instance root. Fully
/// offline: remote files are not downloaded.
pub fn import_curseforge(
    zip_bytes: impl AsRef<[u8]>,
    instances_dir: impl AsRef<Path>,
    name: impl AsRef<str>,
) -> Result<PathBuf> {
    let data = zip_bytes.as_ref();
    let instances_dir = instances_dir.as_ref();
    let name = name.as_ref();
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(data)).map_err(|e| Error::Zip(e.to_string()))?;
    let manifest_text = read_zip_text(&mut archive, "manifest.json").map_err(|e| match e {
        Error::Zip(_) => Error::InvalidPack("missing manifest.json".to_string()),
        other => other,
    })?;
    let manifest: serde_json::Value =
        serde_json::from_str(&manifest_text).map_err(|e| Error::Json(e.to_string()))?;
    let mc_version = manifest
        .get("minecraft")
        .and_then(|m| m.get("version"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::InvalidPack("manifest.json missing minecraft.version".to_string()))?
        .to_string();
    if mc_version.is_empty() {
        return Err(Error::InvalidPack("empty minecraft.version".to_string()));
    }
    let overrides_dir = manifest
        .get("overrides")
        .and_then(|v| v.as_str())
        .unwrap_or("overrides")
        .to_string();
    let instance = palantir_core::instance::Instance::create(instances_dir, name, mc_version.as_str())
        .map_err(core_err)?;
    extract_overrides(&mut archive, overrides_dir.as_str(), instance.root())?;
    let mut loaders: Vec<(String, String)> = Vec::new();
    if let Some(arr) = manifest
        .get("minecraft")
        .and_then(|m| m.get("modLoaders"))
        .and_then(|v| v.as_array())
    {
        let mut primary: Option<(String, String)> = None;
        let mut first: Option<(String, String)> = None;
        for entry in arr {
            let id = match entry.get("id").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => continue,
            };
            let parsed = match parse_curse_loader_id(id, mc_version.as_str()) {
                Some(p) => p,
                None => continue,
            };
            if first.is_none() {
                first = Some(parsed.clone());
            }
            let is_primary = entry.get("primary").and_then(|v| v.as_bool()).unwrap_or(false);
            if is_primary {
                primary = Some(parsed);
                break;
            }
        }
        if let Some(p) = primary.or(first) {
            loaders.push(p);
        }
    }
    register_loaders(&instance, &loaders)?;
    Ok(instance.root().to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write as _};

    fn build_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let buf = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(buf);
        let options =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (name, data) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn detects_formats() {
        let mr = build_zip(&[("modrinth.index.json", b"{}")]);
        assert_eq!(detect_format(mr.as_slice()), PackFormat::MrPack);
        let cf = build_zip(&[("manifest.json", b"{}")]);
        assert_eq!(detect_format(cf.as_slice()), PackFormat::CurseForge);
        let neither = build_zip(&[("other.txt", b"x")]);
        assert_eq!(detect_format(neither.as_slice()), PackFormat::Unknown);
        assert_eq!(detect_format(b"not a zip".as_slice()), PackFormat::Unknown);
        // Modrinth wins when both markers are present.
        let both = build_zip(&[("modrinth.index.json", b"{}"), ("manifest.json", b"{}")]);
        assert_eq!(detect_format(both.as_slice()), PackFormat::MrPack);
    }

    #[test]
    fn imports_mrpack_offline() {
        let index = serde_json::json!({
            "formatVersion": 1,
            "game": "minecraft",
            "versionId": "1.0",
            "name": "Test",
            "dependencies": {"minecraft": "1.20.1", "fabric-loader": "0.16.9"},
            "overrides": "overrides"
        });
        let bytes = build_zip(&[
            ("modrinth.index.json", serde_json::to_vec(&index).unwrap().as_slice()),
            ("overrides/config/test.cfg", b"hello"),
        ]);
        let dir = tempfile::tempdir().unwrap();
        let root =
            import_mrpack(bytes.as_slice(), dir.path(), "Mr Test").unwrap();
        assert!(root.join("config").join("test.cfg").is_file());
        assert_eq!(
            std::fs::read(root.join("config").join("test.cfg")).unwrap(),
            b"hello"
        );
        let profile = palantir_core::pack::PackProfile::load(&root.join("mmc-pack.json")).unwrap();
        let comp = profile.get("net.fabricmc.fabric-loader").unwrap();
        assert_eq!(comp.version, "0.16.9");
        assert_eq!(profile.get("net.minecraft").unwrap().version, "1.20.1");
    }

    #[test]
    fn imports_curseforge_offline() {
        let manifest = serde_json::json!({
            "manifestType": "minecraftModpack",
            "manifestVersion": 1,
            "name": "Test",
            "version": "1.0",
            "author": "t",
            "minecraft": {"version": "1.20.1", "modLoaders": [{"id": "forge-47.2.0", "primary": true}]},
            "overrides": "overrides",
            "files": []
        });
        let bytes = build_zip(&[
            ("manifest.json", serde_json::to_vec(&manifest).unwrap().as_slice()),
            ("overrides/config/c.cfg", b"curse"),
        ]);
        let dir = tempfile::tempdir().unwrap();
        let root =
            import_curseforge(bytes.as_slice(), dir.path(), "Curse Test").unwrap();
        assert_eq!(
            std::fs::read(root.join("config").join("c.cfg")).unwrap(),
            b"curse"
        );
        let profile = palantir_core::pack::PackProfile::load(&root.join("mmc-pack.json")).unwrap();
        let comp = profile.get("net.minecraftforge").unwrap();
        assert_eq!(comp.version, "47.2.0");
    }

    #[test]
    fn rejects_unsafe_overrides() {
        let index = serde_json::json!({
            "formatVersion": 1,
            "dependencies": {"minecraft": "1.20.1"},
            "overrides": "overrides"
        });
        let bytes = build_zip(&[
            ("modrinth.index.json", serde_json::to_vec(&index).unwrap().as_slice()),
            ("overrides/../../evil.txt", b"bad"),
        ]);
        let dir = tempfile::tempdir().unwrap();
        let err = import_mrpack(bytes.as_slice(), dir.path(), "Evil").unwrap_err();
        assert!(matches!(err, Error::UnsafePath(_)));
    }

    #[test]
    fn parses_curse_loader_ids() {
        assert_eq!(
            parse_curse_loader_id("forge-47.2.0", "1.20.1"),
            Some(("net.minecraftforge".to_string(), "47.2.0".to_string()))
        );
        assert_eq!(
            parse_curse_loader_id("fabric-0.16.9-1.20.1", "1.20.1"),
            Some(("net.fabricmc.fabric-loader".to_string(), "0.16.9".to_string()))
        );
        assert_eq!(parse_curse_loader_id("unknown-1.0", "1.20.1"), None);
    }
}
