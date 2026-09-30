//! Modpack detection and offline import.
//!
//! Supports Modrinth (`.mrpack`, `modrinth.index.json`) and CurseForge
//! (`manifest.json`) packs. Imports create a Prism instance, write the
//! `overrides` tree into the instance root and register the loader in
//! `mmc-pack.json`.
//!
//! No network access is performed anywhere in this module. [`plan_pack`] reads
//! a pack's index and returns the remote files it lists ([`PackFile`]) so the
//! caller — which owns the HTTP client and the progress bar — can fetch them;
//! the import functions write only what is inside the zip.

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
    let plan = plan_mrpack(&mut archive).map_err(|e| match e {
        Error::Zip(_) => Error::InvalidPack("missing modrinth.index.json".to_string()),
        other => other,
    })?;
    let mc_version = plan.minecraft.as_str();
    if mc_version.is_empty() {
        return Err(Error::InvalidPack("empty dependencies.minecraft".to_string()));
    }
    let instance = palantir_core::instance::Instance::create(instances_dir, name, mc_version)
        .map_err(core_err)?;
    extract_overrides(&mut archive, plan.overrides.as_str(), instance.root())?;
    register_loaders(&instance, &plan.loaders)?;
    Ok(instance.root().to_path_buf())
}

/// Apply a Modrinth (`.mrpack`) pack to an instance that already exists.
///
/// The half of [`import_mrpack`] that creates nothing: the pack's `overrides/`
/// tree is written over the instance root and its `fabric-loader`/
/// `quilt-loader`/`forge`/`neoforge` dependency is registered in the instance's
/// own `mmc-pack.json`. What the caller does with the answer is fetch the files
/// the index lists, because this crate performs no network access at all.
///
/// **Nothing here is deleted, and that is deliberate.** The reference's
/// *Re-install modpack* says it "resets the {type} content to its original state,
/// removing any mods or content you have added", and it does: its install path
/// clears the pack's own folders first. This launcher's does not, and the reason
/// is the same one *Repair instance* gives -- a reader's worlds, their configs and
/// the mods they added are theirs, and a button that can take them away is a
/// button that has to be sure. Re-applying the pack puts back every file the pack
/// names and leaves everything else where it is, which is what the card's own
/// sentence says (G133).
///
/// Only `.mrpack` is accepted: a CurseForge archive's `manifest.json` names its
/// files by CurseForge project and file id, which resolve only through an API
/// this launcher has no key for, so re-applying one would put back the overrides
/// and none of the mods.
pub fn apply_mrpack(
    zip_bytes: impl AsRef<[u8]>,
    instance: &palantir_core::instance::Instance,
) -> Result<PackPlan> {
    let data = zip_bytes.as_ref();
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(data)).map_err(|e| Error::Zip(e.to_string()))?;
    if archive.by_name("modrinth.index.json").is_err() {
        return Err(Error::InvalidPack(
            "missing modrinth.index.json".to_string(),
        ));
    }
    let plan = plan_mrpack(&mut archive)?;
    extract_overrides(&mut archive, plan.overrides.as_str(), instance.root())?;
    register_loaders(instance, &plan.loaders)?;
    Ok(plan)
}

/// One file a pack's index lists.
///
/// Nothing in this crate fetches these — the module performs no network access
/// at all — so the list is handed back to the caller, which owns the HTTP
/// client, the thread pool and the progress bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackFile {
    /// Instance-relative destination, as the pack writes it (`mods/sodium.jar`).
    pub path: String,
    /// Candidate download URLs, in the order the pack lists them. The first is
    /// the one a downloader should use; the rest are mirrors worth trying when
    /// it fails.
    pub downloads: Vec<String>,
    /// `sha1` hex digest when the pack publishes one.
    pub sha1: Option<String>,
    /// Declared size in bytes (`0` when the pack publishes none).
    pub size: u64,
}

impl PackFile {
    /// The destination under an instance root, or `None` when the pack names a
    /// path that must not be written.
    ///
    /// Refusing is deliberate rather than sanitising: a pack is untrusted input
    /// and a rewritten path would silently install a file somewhere the pack
    /// did not ask for. Interesting cases get dropped and reported instead.
    pub fn relative_path(&self) -> Option<PathBuf> {
        let path = Path::new(self.path.as_str());
        if path.as_os_str().is_empty() || path.is_absolute() {
            return None;
        }
        for component in path.components() {
            match component {
                Component::CurDir | Component::Normal(_) => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
            }
        }
        Some(path.to_path_buf())
    }

    /// Whether the file that is already at `dest` can be left alone.
    pub fn satisfied_at(&self, dest: &Path) -> bool {
        match std::fs::metadata(dest) {
            Ok(meta) if meta.is_file() => self.size == 0 || meta.len() >= self.size,
            _ => false,
        }
    }
}

/// What a pack asks for, read without writing anything.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackPlan {
    /// Minecraft version (`dependencies.minecraft` / `minecraft.version`).
    pub minecraft: String,
    /// Loader components, ready to be registered in `mmc-pack.json`.
    pub loaders: Vec<(String, String)>,
    /// Files with somewhere to fetch them from.
    pub files: Vec<PackFile>,
    /// Name of the overrides directory inside the zip (`overrides` by default).
    pub overrides: String,
    /// Entries that are listed but will not be installed, each with the reason.
    ///
    /// CurseForge lands here in full: its `files[]` is `projectID`/`fileID`
    /// pairs that only resolve through the CurseForge API, which needs a key
    /// this launcher does not have. Saying so is the whole point of the field —
    /// an import that quietly installed the overrides and none of the mods is
    /// what this used to do.
    pub skipped: Vec<String>,
}

impl PackPlan {
    /// How many of the pack's entries will not be fetched.
    pub fn skipped_count(&self) -> usize {
        self.skipped.len()
    }
}

/// Read a pack's index out of its zip container.
///
/// Both formats are read here so one call answers "what does this pack want?"
/// for either: the Minecraft version, the loader components, and the remote
/// files — the part an import used to drop on the floor.
pub fn plan_pack(zip_bytes: impl AsRef<[u8]>) -> Result<PackPlan> {
    let data = zip_bytes.as_ref();
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(data)).map_err(|e| Error::Zip(e.to_string()))?;
    if archive.by_name("modrinth.index.json").is_ok() {
        return plan_mrpack(&mut archive);
    }
    if archive.by_name("manifest.json").is_ok() {
        return plan_curseforge(&mut archive);
    }
    Err(Error::InvalidPack(
        "neither modrinth.index.json nor manifest.json is present".to_string(),
    ))
}

/// The Modrinth half of [`plan_pack`].
fn plan_mrpack(archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>) -> Result<PackPlan> {
    let text = read_zip_text(archive, "modrinth.index.json")?;
    let index: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| Error::Json(e.to_string()))?;
    let mut plan = PackPlan {
        minecraft: index
            .get("dependencies")
            .and_then(|d| d.get("minecraft"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        overrides: index
            .get("overrides")
            .and_then(|v| v.as_str())
            .unwrap_or("overrides")
            .to_string(),
        ..PackPlan::default()
    };
    if let Some(deps) = index.get("dependencies").and_then(|v| v.as_object()) {
        for (key, value) in deps {
            if let Some(uid) = mrpack_loader_uid(key.as_str()) {
                if let Some(version) = value.as_str().filter(|v| !v.is_empty()) {
                    plan.loaders.push((uid.to_string(), version.to_string()));
                }
            }
        }
    }
    let entries = match index.get("files").and_then(|v| v.as_array()) {
        Some(entries) => entries,
        None => return Ok(plan),
    };
    for entry in entries {
        let path = entry.get("path").and_then(|v| v.as_str()).unwrap_or_default();
        // `env.client == "unsupported"` is the pack saying the file is for the
        // server. Installing it anyway is a client that crashes on startup, so
        // the pack's own answer is honoured rather than second-guessed.
        let client_env = entry
            .get("env")
            .and_then(|env| env.get("client"))
            .and_then(|v| v.as_str());
        if client_env == Some("unsupported") {
            continue;
        }
        let downloads: Vec<String> = entry
            .get("downloads")
            .and_then(|v| v.as_array())
            .map(|urls| {
                urls.iter()
                    .filter_map(|u| u.as_str())
                    .filter(|u| !u.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let file = PackFile {
            path: path.to_string(),
            downloads,
            sha1: entry
                .get("hashes")
                .and_then(|h| h.get("sha1"))
                .and_then(|v| v.as_str())
                .map(str::to_string),
            size: entry.get("fileSize").and_then(|v| v.as_u64()).unwrap_or(0),
        };
        if file.downloads.is_empty() {
            plan.skipped.push(format!("{path} (the index lists no download URL)"));
        } else if file.relative_path().is_none() {
            plan.skipped
                .push(format!("{path} (unsafe path — refusing to write it)"));
        } else {
            plan.files.push(file);
        }
    }
    Ok(plan)
}

/// The CurseForge half of [`plan_pack`].
///
/// Its `files[]` names a project and a file id, never a URL: resolving those is
/// the CurseForge API, and every request there needs a key. The entries are
/// therefore reported as skipped with the reason, which turns "the import
/// silently fetched nothing" into a line the user can read.
fn plan_curseforge(archive: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>) -> Result<PackPlan> {
    let text = read_zip_text(archive, "manifest.json")?;
    let manifest: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| Error::Json(e.to_string()))?;
    let minecraft = manifest.get("minecraft");
    let mut plan = PackPlan {
        minecraft: minecraft
            .and_then(|m| m.get("version"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        overrides: manifest
            .get("overrides")
            .and_then(|v| v.as_str())
            .unwrap_or("overrides")
            .to_string(),
        ..PackPlan::default()
    };
    if let Some(arr) = minecraft.and_then(|m| m.get("modLoaders")).and_then(|v| v.as_array()) {
        let mut primary: Option<(String, String)> = None;
        let mut first: Option<(String, String)> = None;
        for entry in arr {
            let Some(id) = entry.get("id").and_then(|v| v.as_str()) else {
                continue;
            };
            let Some(parsed) = parse_curse_loader_id(id, plan.minecraft.as_str()) else {
                continue;
            };
            if first.is_none() {
                first = Some(parsed.clone());
            }
            if entry.get("primary").and_then(|v| v.as_bool()).unwrap_or(false) {
                primary = Some(parsed);
                break;
            }
        }
        if let Some(chosen) = primary.or(first) {
            plan.loaders.push(chosen);
        }
    }
    if let Some(files) = manifest.get("files").and_then(|v| v.as_array()) {
        for entry in files {
            let project = entry
                .get("projectID")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);
            let file = entry
                .get("fileID")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(0);
            plan.skipped.push(format!(
                "CurseForge project {project} file {file} (needs the CurseForge API)"
            ));
        }
    }
    Ok(plan)
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
    let plan = plan_curseforge(&mut archive).map_err(|e| match e {
        Error::Zip(_) => Error::InvalidPack("missing manifest.json".to_string()),
        other => other,
    })?;
    let mc_version = plan.minecraft.as_str();
    if mc_version.is_empty() {
        return Err(Error::InvalidPack("empty minecraft.version".to_string()));
    }
    let instance = palantir_core::instance::Instance::create(instances_dir, name, mc_version)
        .map_err(core_err)?;
    extract_overrides(&mut archive, plan.overrides.as_str(), instance.root())?;
    register_loaders(&instance, &plan.loaders)?;
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

    #[test]
    fn plans_the_files_an_mrpack_lists() {
        let index = serde_json::json!({
            "formatVersion": 1,
            "dependencies": {"minecraft": "1.20.1", "fabric-loader": "0.16.9"},
            "files": [
                {
                    "path": "mods/sodium.jar",
                    "hashes": {"sha1": "aa11"},
                    "fileSize": 400,
                    "downloads": ["https://cdn.example.invalid/sodium.jar", "https://mirror.invalid/sodium.jar"]
                },
                {
                    "path": "mods/server-only.jar",
                    "hashes": {"sha1": "bb22"},
                    "fileSize": 10,
                    "env": {"client": "unsupported", "server": "required"},
                    "downloads": ["https://cdn.example.invalid/server.jar"]
                },
                {
                    "path": "../../escape.jar",
                    "hashes": {"sha1": "cc33"},
                    "downloads": ["https://cdn.example.invalid/escape.jar"]
                },
                {
                    "path": "mods/no-url.jar",
                    "hashes": {"sha1": "dd44"}
                }
            ]
        });
        let bytes = build_zip(&[(
            "modrinth.index.json",
            serde_json::to_vec(&index).unwrap().as_slice(),
        )]);
        let plan = plan_pack(bytes.as_slice()).unwrap();
        assert_eq!(plan.minecraft, "1.20.1");
        assert_eq!(
            plan.loaders,
            vec![("net.fabricmc.fabric-loader".to_string(), "0.16.9".to_string())]
        );
        // The server-only file is the pack's own decision and is not installed;
        // the unsafe and URL-less entries are reported, not written.
        assert_eq!(plan.files.len(), 1);
        assert_eq!(plan.files[0].path, "mods/sodium.jar");
        assert_eq!(plan.files[0].downloads.len(), 2, "mirrors are kept in order");
        assert_eq!(plan.files[0].sha1.as_deref(), Some("aa11"));
        assert_eq!(plan.files[0].size, 400);
        assert_eq!(plan.skipped_count(), 2);
        assert!(plan.skipped.iter().any(|line| line.contains("escape.jar")));
        assert!(plan.skipped.iter().any(|line| line.contains("no-url.jar")));
        assert_eq!(
            plan.files[0].relative_path(),
            Some(PathBuf::from("mods").join("sodium.jar"))
        );
        assert!(PackFile {
            path: "C:\\windows\\x.jar".to_string(),
            downloads: vec!["u".to_string()],
            sha1: None,
            size: 0,
        }
        .relative_path()
        .is_none());
    }

    #[test]
    fn a_curseforge_plan_reports_what_it_cannot_fetch() {
        let manifest = serde_json::json!({
            "manifestType": "minecraftModpack",
            "manifestVersion": 1,
            "minecraft": {"version": "1.20.1", "modLoaders": [{"id": "forge-47.2.0", "primary": true}]},
            "overrides": "overrides",
            "files": [{"projectID": 306612, "fileID": 1234, "required": true}]
        });
        let bytes = build_zip(&[(
            "manifest.json",
            serde_json::to_vec(&manifest).unwrap().as_slice(),
        )]);
        let plan = plan_pack(bytes.as_slice()).unwrap();
        assert_eq!(plan.minecraft, "1.20.1");
        assert_eq!(
            plan.loaders,
            vec![("net.minecraftforge".to_string(), "47.2.0".to_string())]
        );
        assert!(plan.files.is_empty());
        assert_eq!(plan.skipped_count(), 1);
        assert!(plan.skipped[0].contains("306612"));
        assert!(plan.skipped[0].contains("CurseForge API"));
    }

    #[test]
    fn a_pack_file_is_satisfied_by_a_big_enough_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = PackFile {
            path: "mods/a.jar".to_string(),
            downloads: vec!["u".to_string()],
            sha1: None,
            size: 4,
        };
        let dest = dir.path().join("a.jar");
        assert!(!file.satisfied_at(&dest));
        std::fs::write(&dest, b"abc").unwrap();
        assert!(!file.satisfied_at(&dest), "a short file is fetched again");
        std::fs::write(&dest, b"abcd").unwrap();
        assert!(file.satisfied_at(&dest));
    }

    #[test]
    fn applying_a_pack_over_an_instance_keeps_what_the_reader_added() {
        // The G133 half of the pack path: an instance that already exists gets
        // the pack's overrides written over its root and its loader registered,
        // and *nothing is deleted* -- not the worlds and mods the reader added,
        // and not a file the pack's previous version wrote that this one does not
        // name. It is the property the installation tab's own sentence promises.
        let index = serde_json::json!({
            "formatVersion": 1,
            "dependencies": {"minecraft": "1.21.4", "fabric-loader": "0.16.9"},
            "overrides": "overrides"
        });
        let bytes = build_zip(&[
            (
                "modrinth.index.json",
                serde_json::to_vec(&index).unwrap().as_slice(),
            ),
            ("overrides/config/pack.cfg", b"new"),
        ]);
        let dir = tempfile::tempdir().unwrap();
        let instances = dir.path().join("instances");
        let instance =
            palantir_core::instance::Instance::create(&instances, "Cobblemon", "1.21.4").unwrap();
        let root = instance.root().to_path_buf();
        std::fs::create_dir_all(root.join("config")).unwrap();
        std::fs::create_dir_all(root.join("saves").join("world")).unwrap();
        std::fs::create_dir_all(root.join("mods")).unwrap();
        // What the pack's previous version wrote, and the three things a
        // re-install must not touch.
        std::fs::write(root.join("config").join("pack.cfg"), b"old").unwrap();
        std::fs::write(root.join("config").join("left-behind.cfg"), b"mine").unwrap();
        std::fs::write(root.join("saves").join("world").join("level.dat"), b"my world")
            .unwrap();
        std::fs::write(root.join("mods").join("manual.jar"), b"my mod").unwrap();

        let plan = apply_mrpack(bytes.as_slice(), &instance).unwrap();
        assert_eq!(plan.minecraft, "1.21.4");
        // The override is written over the root with `overrides/` stripped, and a
        // file of the same name is replaced rather than merged.
        assert_eq!(
            std::fs::read(root.join("config").join("pack.cfg")).unwrap(),
            b"new"
        );
        // The loader is registered, so the instance the pack was laid over
        // resolves the pack's own profile at its next launch.
        let pack = palantir_core::util::read_text(&root.join("mmc-pack.json")).unwrap();
        assert!(pack.contains("net.fabricmc.fabric-loader"), "pack: {pack}");
        assert!(pack.contains("0.16.9"), "pack: {pack}");
        // And nothing is gone.
        assert!(
            root.join("config").join("left-behind.cfg").is_file(),
            "a config the new pack does not name stays where it is"
        );
        assert!(
            root.join("saves").join("world").join("level.dat").is_file(),
            "the reader's world stays"
        );
        assert!(
            root.join("mods").join("manual.jar").is_file(),
            "the mod the reader added stays"
        );
    }

    #[test]
    fn applying_a_curseforge_archive_is_refused_because_its_files_cannot_be_fetched() {
        // `apply_mrpack` reads the Modrinth index or says why it cannot: a
        // CurseForge archive's files are project/file id pairs that resolve only
        // through an API this launcher has no key for, so re-applying one would
        // put back the overrides and none of the mods.
        let manifest = serde_json::json!({
            "manifestType": "minecraftModpack",
            "manifestVersion": 1,
            "minecraft": {"version": "1.20.1", "modLoaders": []},
            "overrides": "overrides"
        });
        let bytes = build_zip(&[(
            "manifest.json",
            serde_json::to_vec(&manifest).unwrap().as_slice(),
        )]);
        let dir = tempfile::tempdir().unwrap();
        let instance =
            palantir_core::instance::Instance::create(dir.path(), "Curse", "1.20.1").unwrap();
        let error = apply_mrpack(bytes.as_slice(), &instance).unwrap_err();
        assert!(
            matches!(&error, Error::InvalidPack(why) if why.contains("modrinth.index.json")),
            "got: {error:?}"
        );
    }
}
