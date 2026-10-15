//! Bedrock content: what a pack archive says it is, and the per-world file
//! that switches a pack on for one world.
//!
//! Two facts drive everything here, and both come from the formats rather than
//! from taste:
//!
//! - **The container extension names a container, not a destination.** A
//!   `.mcpack` is a zip holding either a resource pack or a behaviour pack, and
//!   only the `manifest.json` inside says which; a `.mcaddon` is the same zip
//!   shape holding several packs that need not share one destination. Filing a
//!   pack by its file name therefore puts half of them in the wrong folder, in
//!   a state the player cannot see, so a destination is read out of the
//!   manifest or the archive is refused ([`addons`] already refuses the same
//!   split at the name).
//! - **Activation is per world.** A pack copied into `resource_packs` is
//!   installed but not in effect: a world's `world_resource_packs.json` /
//!   `world_behavior_packs.json` lists the pack ids that world turned on. This
//!   module reads and writes exactly those lists, and writes them atomically,
//!   because a half-written list is a world whose packs stop loading.
//!
//! What this module does not do is install: it plans. Copying the archive's
//! bytes into `resource_packs`/`behavior_packs`/`skin_packs` belongs to the
//! caller that knows the machine's `com.mojang` root, and nothing here opens a
//! world's database.

use std::io::{Read, Write};
use std::path::Path;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::addons::{self, AddonKind};

/// A pack's own `manifest.json`.
///
/// Only the fields a launcher must act on are modelled; `min_engine_version`
/// and the rest ride along unread, because a pack this launcher does not
/// understand is still a pack the player can see in the game.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PackManifest {
    #[serde(default)]
    pub format_version: Value,
    pub header: PackHeader,
    #[serde(default)]
    pub modules: Vec<PackModule>,
}

/// The identity half of a manifest: the uuid the game keys a pack by, the
/// version it matches against, and the name a player sees.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PackHeader {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub version: Value,
    #[serde(default)]
    pub description: Option<String>,
}

/// One module. Its `type` is the only thing that says which folder the pack
/// belongs in.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PackModule {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub version: Value,
}

/// What a pack is, as far as a destination is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackKind {
    /// A resource pack: `resource_packs`.
    Resource,
    /// A behaviour pack: `behavior_packs`.
    Behaviour,
    /// A skin pack: `skin_packs`.
    Skin,
    /// A `client_data` module, which has no folder this launcher may write to.
    Data,
    /// Nothing recognisable, or two destinations at once: refused rather than
    /// guessed.
    Unknown,
}

/// The kind one module type names.
///
/// `data` is Bedrock's own word for a behaviour pack (`client_data` is the
/// client-only sibling), and a type this launcher has never seen is `Unknown`
/// rather than the nearest guess.
fn module_kind(module_type: &str) -> PackKind {
    match module_type {
        "resources" => PackKind::Resource,
        "data" => PackKind::Behaviour,
        "skin_packs" => PackKind::Skin,
        "client_data" => PackKind::Data,
        _ => PackKind::Unknown,
    }
}

/// The folder under `com.mojang` a kind belongs in, and `None` where the kind
/// does not decide one.
pub fn destination_folder(kind: PackKind) -> Option<&'static str> {
    match kind {
        PackKind::Resource => Some("resource_packs"),
        PackKind::Behaviour => Some("behavior_packs"),
        PackKind::Skin => Some("skin_packs"),
        PackKind::Data | PackKind::Unknown => None,
    }
}

/// What a manifest's modules say the pack is.
///
/// One destination or none: a manifest naming both a resource module and a
/// behaviour module is describing two packs in one archive, and copying it into
/// either folder would leave the other half unavailable while looking
/// installed. That is `Unknown`, which the caller refuses.
pub fn kind_of(manifest: &PackManifest) -> PackKind {
    let mut decided: Option<PackKind> = None;
    let mut data_only = false;
    for module in &manifest.modules {
        let kind = module_kind(&module.kind);
        if kind == PackKind::Data {
            data_only = true;
            continue;
        }
        if kind == PackKind::Unknown {
            continue;
        }
        match decided {
            None => decided = Some(kind),
            Some(existing) if existing == kind => {}
            Some(_) => return PackKind::Unknown,
        }
    }
    match decided {
        Some(kind) => kind,
        None if data_only => PackKind::Data,
        None => PackKind::Unknown,
    }
}

/// The module types a manifest names, so an error can say what was found rather
/// than only what was wanted.
fn module_types(manifest: &PackManifest) -> String {
    if manifest.modules.is_empty() {
        return "no modules".to_string();
    }
    manifest
        .modules
        .iter()
        .map(|module| module.kind.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// One file's I/O failure, named the way every other error in this module names
/// its path.
fn io_error(path: &Path, error: std::io::Error) -> String {
    format!("{}: {error}", path.display())
}

/// A pack read out of an archive: its manifest, what it is, and how many zip
/// entries it holds.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedPack {
    pub manifest: PackManifest,
    pub kind: PackKind,
    pub entries: usize,
}

/// Where a pack's `manifest.json` sits inside the archive.
///
/// Root first (a `.mcpack`), then exactly one entry one level down (the
/// `.mcaddon` layout). Two nested candidates are reported as ambiguous: the
/// archive holds several packs and nothing in the file says which one the
/// player meant.
fn manifest_entry(names: &[String]) -> Result<String, String> {
    if names.iter().any(|name| name == "manifest.json") {
        return Ok("manifest.json".to_string());
    }
    let nested = names
        .iter()
        .filter(|name| {
            let Some(prefix) = name.strip_suffix("/manifest.json") else {
                return false;
            };
            !prefix.is_empty() && !prefix.contains('/')
        })
        .collect::<Vec<_>>();
    match nested.len() {
        1 => Ok(nested[0].to_string()),
        0 => Err("it carries no manifest.json, so nothing says which pack it is".to_string()),
        _ => Err(format!(
            "it carries {} manifests ({}), so which pack to install cannot be decided",
            nested.len(),
            nested
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// Read one pack archive: open the zip, find its manifest, and classify it.
pub fn parse_pack_archive(archive: &Path) -> Result<ParsedPack, String> {
    let file = std::fs::File::open(archive).map_err(|error| io_error(archive, error))?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|error| format!("{} is not a readable zip: {error}", archive.display()))?;
    let names = zip
        .file_names()
        .map(str::to_string)
        .collect::<Vec<String>>();
    let entry = manifest_entry(&names).map_err(|why| format!("{}: {why}", archive.display()))?;
    let mut manifest_text = String::new();
    let manifest_file = zip
        .by_name(&entry)
        .map_err(|error| format!("{}: {entry}: {error}", archive.display()))?;
    if manifest_file.size() > 1024 * 1024 {
        return Err("Pack manifest exceeds 1 MiB.".into());
    }
    manifest_file
        .take(1024 * 1024 + 1)
        .read_to_string(&mut manifest_text)
        .map_err(|error| format!("{}: {entry}: {error}", archive.display()))?;
    if manifest_text.len() > 1024 * 1024 {
        return Err("Pack manifest exceeds 1 MiB.".into());
    }
    let manifest: PackManifest = serde_json::from_str(&manifest_text)
        .map_err(|error| format!("{}: {entry}: {error}", archive.display()))?;
    Ok(ParsedPack {
        kind: kind_of(&manifest),
        manifest,
        entries: names.len(),
    })
}

/// A pack worked out before anything is copied: where it goes, and the identity
/// its world activation files will have to name.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedPack {
    pub kind: PackKind,
    pub uuid: String,
    pub version: Value,
    /// `None` only when a caller planned without deciding a destination;
    /// [`plan_pack`] refuses that case instead of returning it.
    pub destination: Option<&'static str>,
    pub name: String,
}

/// A pack uuid: 8-4-4-4-12 hexadecimal with hyphens.
///
/// The game keys packs by uuid, so an identity that is not one is a pack that
/// installs and then never loads. Refusing it here keeps a fabricated id out of
/// an activation file, where the failure would surface as a pack silently doing
/// nothing.
pub fn is_uuid(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (index, byte) in bytes.iter().copied().enumerate() {
        match index {
            8 | 13 | 18 | 23 => {
                if byte != b'-' {
                    return false;
                }
            }
            _ => {
                if !(byte as char).is_ascii_hexdigit() {
                    return false;
                }
            }
        }
    }
    true
}

/// A pack version: the format's own `[major, minor, patch]` list. A version
/// that is not a list is one the game cannot match an activation against.
pub fn is_version(value: &Value) -> bool {
    value.as_array().is_some_and(|parts| {
        !parts.is_empty() && parts.len() <= 4 && parts.iter().all(Value::is_u64)
    })
}

/// Plan the install of one downloaded archive under the name it arrived as.
///
/// The name is used only to refuse: a world archive belongs to the world flow,
/// and a name that is not a Bedrock container at all is not something to guess
/// at. Everything else comes out of the manifest inside.
pub fn plan_pack(archive: &Path, file_name: &str) -> Result<PlannedPack, String> {
    match addons::classify(file_name) {
        AddonKind::Pack | AddonKind::Addon | AddonKind::SkinPack => {}
        AddonKind::World => {
            return Err(format!(
                "{file_name} is a world, which is imported as a world."
            ));
        }
        AddonKind::Unknown => {
            return Err(format!(
                "{file_name} is not a Bedrock container this launcher knows."
            ));
        }
    }
    let parsed = parse_pack_archive(archive)?;
    if !is_uuid(&parsed.manifest.header.uuid) {
        return Err(format!(
            "{file_name} names the pack id {:?}, which is not a uuid; the game could never load it.",
            parsed.manifest.header.uuid
        ));
    }
    if !is_version(&parsed.manifest.header.version) {
        return Err(format!(
            "{file_name} names the pack version {:?}, which is not a version list; the game could not match it.",
            parsed.manifest.header.version
        ));
    }
    let destination = match destination_folder(parsed.kind) {
        Some(folder) => folder,
        None => {
            return Err(format!(
                "{file_name} declares {} and no destination this launcher may write; nothing was copied.",
                module_types(&parsed.manifest)
            ));
        }
    };
    Ok(PlannedPack {
        kind: parsed.kind,
        uuid: parsed.manifest.header.uuid.clone(),
        version: parsed.manifest.header.version.clone(),
        destination: Some(destination),
        name: parsed.manifest.header.name.clone(),
    })
}

/// Is this file name a pack container (as opposed to a world, which the world
/// flow owns)?
pub fn is_pack_archive(file_name: &str) -> bool {
    matches!(
        addons::classify(file_name),
        AddonKind::Pack | AddonKind::Addon
    )
}

/// The packs one world has switched on, as the world's own file lists them.
///
/// A missing list is an empty list -- a world that has never had a pack exists
/// and is not an error -- and the write path's backup is consulted when the
/// list itself is gone, so a launcher killed mid-write still reads the last
/// good activation instead of silently switching a player's packs off.
pub fn read_activation(path: &Path) -> Result<Vec<Value>, String> {
    let backup = path.with_extension("bak");
    let source = if path.exists() {
        path
    } else if backup.exists() {
        &backup
    } else {
        return Ok(Vec::new());
    };
    match std::fs::symlink_metadata(source) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(format!(
                "{} is a symbolic link; refusing to read a world's pack list through it.",
                source.display()
            ));
        }
        Ok(metadata) if !metadata.is_file() => {
            return Err(format!("{} is not a file.", source.display()));
        }
        Ok(_) => {}
        Err(error) => return Err(format!("{}: {error}", source.display())),
    }
    let text = std::fs::read_to_string(source).map_err(|error| io_error(source, error))?;
    let value: Value =
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", source.display()))?;
    let entries = value
        .as_array()
        .ok_or_else(|| format!("{} is not a list of active packs.", source.display()))?;
    for entry in entries {
        if !entry.is_object() {
            return Err(format!(
                "{} holds an entry, not a pack identity.",
                source.display()
            ));
        }
    }
    Ok(entries.clone())
}

/// The list with `pack_id` activated at `version`.
///
/// One pack appears once: re-activating the same id is an update in place, not
/// a second row, because a duplicate would leave the game matching whichever
/// entry it read first. Entries naming other packs -- including ids this
/// launcher never installed -- are kept exactly as they were.
pub fn activate(entries: &[Value], pack_id: &str, version: &Value) -> Result<Vec<Value>, String> {
    if !is_uuid(pack_id) {
        return Err(format!("{pack_id:?} is not a pack uuid; refusing it."));
    }
    if !is_version(version) {
        return Err(format!(
            "{version:?} is not a pack version list; refusing it."
        ));
    }
    let existing = entries
        .iter()
        .position(|entry| entry.get("pack_id").and_then(Value::as_str) == Some(pack_id));
    let mut updated = entries.to_vec();
    match existing {
        Some(index) => {
            if let Some(object) = updated[index].as_object_mut() {
                object.insert("version".to_string(), version.clone());
            }
        }
        None => updated.push(json!({ "pack_id": pack_id, "version": version })),
    }
    Ok(updated)
}

/// Write a world's pack list back, atomically.
///
/// The list is replaced through a staging file and a backup, the same way the
/// launcher's own instance and preference files are: the game reads this file
/// while the launcher is not running, so a partially written list would be a
/// world whose packs stop loading with nothing to explain it. The world folder
/// itself is never created -- a launcher that invents a world folder is
/// inventing a world.
pub fn write_activation(path: &Path, entries: &[Value]) -> Result<(), String> {
    for entry in entries {
        if !entry.is_object() {
            return Err("A pack list entry must be a pack identity.".to_string());
        }
    }
    let Some(directory) = path.parent() else {
        return Err(format!(
            "{} has no directory to write into.",
            path.display()
        ));
    };
    if !directory.is_dir() {
        return Err(format!(
            "{} is not a directory; refusing to create a world folder that is not there.",
            directory.display()
        ));
    }
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(format!(
                "{} is a symbolic link; refusing to write a world's pack list through it.",
                path.display()
            ));
        }
        Ok(metadata) if !metadata.is_file() => {
            return Err(format!("{} is not a file.", path.display()));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("{}: {error}", path.display())),
    }
    let text = serde_json::to_string_pretty(&entries)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let staging = path.with_extension("tmp");
    let backup = path.with_extension("bak");
    // Exclusive creation refuses linked/stale staging files instead of writing
    // through them. A backup may only be an ordinary file owned by this flow.
    if let Ok(metadata) = std::fs::symlink_metadata(&backup) {
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(format!(
                "{} is not an ordinary backup file.",
                backup.display()
            ));
        }
    }
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staging)
        .map_err(|error| io_error(&staging, error))?;
    let staged = output
        .write_all(text.as_bytes())
        .and_then(|()| output.sync_all());
    drop(output);
    if let Err(error) = staged {
        std::fs::remove_file(&staging)
            .map_err(|cleanup| format!("{error}; staging cleanup failed: {cleanup}"))?;
        return Err(io_error(&staging, error));
    }
    // Windows cannot rename over an existing file, so the previous list is
    // moved aside first; it is the file `read_activation` falls back to.
    if path.exists() {
        if backup.exists() {
            std::fs::remove_file(&backup).map_err(|error| io_error(&backup, error))?;
        }
        std::fs::rename(path, &backup).map_err(|error| io_error(path, error))?;
    }
    std::fs::rename(&staging, path).map_err(|error| io_error(path, error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;

    const PACK_ID: &str = "11111111-2222-3333-4444-555555555555";
    const OTHER_ID: &str = "22222222-3333-4444-5555-666666666666";

    fn temp_dir(tag: &str) -> PathBuf {
        let name = format!("palantir-bedrock-content-{tag}-{}", std::process::id());
        let path = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_zip(path: &Path, entries: &[(&str, &str)]) {
        let file = std::fs::File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        for (name, body) in entries {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(body.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
    }

    fn manifest(module: &str) -> String {
        format!(
            r#"{{"format_version":2,"header":{{"name":"Clouds","description":"Sky","uuid":"{PACK_ID}","version":[1,0,0]}},"modules":[{{"type":"{module}"}}]}}"#
        )
    }

    fn pack(zip: &Path, body: &str) {
        write_zip(zip, &[("manifest.json", body)]);
    }

    #[test]
    fn a_resource_and_a_behaviour_module_decide_their_own_folders() {
        let dir = temp_dir("kinds");
        let resource = dir.join("clouds.mcpack");
        pack(&resource, &manifest("resources"));
        let planned = plan_pack(&resource, "clouds.mcpack").unwrap();
        assert_eq!(planned.kind, PackKind::Resource);
        assert_eq!(planned.destination, Some("resource_packs"));
        assert_eq!(planned.name, "Clouds");
        assert_eq!(planned.version, json!([1, 0, 0]));

        let behaviour = dir.join("gravity.mcpack");
        pack(&behaviour, &manifest("data"));
        let planned = plan_pack(&behaviour, "gravity.mcpack").unwrap();
        assert_eq!(planned.kind, PackKind::Behaviour);
        assert_eq!(planned.destination, Some("behavior_packs"));

        let skin = dir.join("heroes.mcskin");
        pack(&skin, &manifest("skin_packs"));
        let planned = plan_pack(&skin, "heroes.mcskin").unwrap();
        assert_eq!(planned.kind, PackKind::Skin);
        assert_eq!(planned.destination, Some("skin_packs"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_addon_carrying_its_manifest_one_level_down_is_read() {
        let dir = temp_dir("nested");
        let addon = dir.join("island.mcaddon");
        let nested = manifest("resources");
        write_zip(&addon, &[("island/manifest.json", nested.as_str())]);
        let parsed = parse_pack_archive(&addon).unwrap();
        assert_eq!(parsed.kind, PackKind::Resource);
        assert_eq!(parsed.entries, 1);
        let planned = plan_pack(&addon, "island.mcaddon").unwrap();
        assert_eq!(planned.destination, Some("resource_packs"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_archive_without_a_manifest_says_so_instead_of_guessing() {
        let dir = temp_dir("no-manifest");
        let empty = dir.join("empty.mcpack");
        write_zip(&empty, &[("readme.txt", "nothing here")]);
        let error = parse_pack_archive(&empty).unwrap_err();
        assert!(error.contains("no manifest.json"), "{error}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn an_addon_with_two_manifests_is_refused_rather_than_guessed() {
        let dir = temp_dir("ambiguous");
        let addon = dir.join("two.mcaddon");
        let first = manifest("resources");
        let second = manifest("data");
        write_zip(
            &addon,
            &[
                ("one/manifest.json", first.as_str()),
                ("two/manifest.json", second.as_str()),
            ],
        );
        let error = parse_pack_archive(&addon).unwrap_err();
        assert!(error.contains("cannot be decided"), "{error}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_manifest_naming_two_destinations_is_refused() {
        let dir = temp_dir("two-kinds");
        let both = dir.join("both.mcpack");
        let body = concat!(
            r#"{"format_version":2,"header":{"name":"Both","uuid":"#,
            r#""11111111-2222-3333-4444-555555555555","version":[1,0,0]},"#,
            r#""modules":[{"type":"resources"},{"type":"data"}]}"#
        );
        pack(&both, body);
        let error = plan_pack(&both, "both.mcpack").unwrap_err();
        assert!(error.contains("resources, data"), "{error}");
        assert!(error.contains("no destination"), "{error}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_module_type_this_launcher_does_not_know_is_refused() {
        let dir = temp_dir("unknown-module");
        let scripted = dir.join("scripted.mcpack");
        pack(&scripted, &manifest("script"));
        let error = plan_pack(&scripted, "scripted.mcpack").unwrap_err();
        assert!(error.contains("script"), "{error}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_pack_id_that_is_not_a_uuid_is_refused() {
        assert!(is_uuid(PACK_ID));
        assert!(is_uuid("ABCDEF01-2345-6789-ABCD-EF0123456789"));
        for broken in [
            "",
            "11111111-2222-3333-4444-55555555555",
            "11111111-2222-3333-4444-5555555555555",
            "1111111g-2222-3333-4444-555555555555",
            "11111111_2222_3333_4444_555555555555",
            "clouds",
        ] {
            assert!(!is_uuid(broken), "{broken} was accepted");
        }

        let dir = temp_dir("bogus-uuid");
        let bogus = dir.join("bogus.mcpack");
        let body = concat!(
            r#"{"format_version":2,"header":{"name":"Bogus","uuid":"not-a-uuid","#,
            r#""version":[1,0,0]},"modules":[{"type":"resources"}]}"#
        );
        pack(&bogus, body);
        let error = plan_pack(&bogus, "bogus.mcpack").unwrap_err();
        assert!(error.contains("not a uuid"), "{error}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_pack_version_that_is_not_a_version_list_is_refused() {
        let dir = temp_dir("bogus-version");
        let bogus = dir.join("bogus.mcpack");
        let body = concat!(
            r#"{"format_version":2,"header":{"name":"Bogus","uuid":"#,
            r#""11111111-2222-3333-4444-555555555555","version":"1.0"},"#,
            r#""modules":[{"type":"resources"}]}"#
        );
        pack(&bogus, body);
        let error = plan_pack(&bogus, "bogus.mcpack").unwrap_err();
        assert!(error.contains("not a version list"), "{error}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_world_or_an_unrelated_file_name_is_not_a_pack_container() {
        let dir = temp_dir("names");
        let archive = dir.join("payload.bin");
        pack(&archive, &manifest("resources"));
        let world = plan_pack(&archive, "valley.mcworld").unwrap_err();
        assert!(world.contains("imported as a world"), "{world}");
        let unrelated = plan_pack(&archive, "pack.zip").unwrap_err();
        assert!(unrelated.contains("not a Bedrock container"), "{unrelated}");
        assert!(is_pack_archive("clouds.mcpack"));
        assert!(is_pack_archive("island.mcaddon"));
        assert!(!is_pack_archive("valley.mcworld"));
        assert!(!is_pack_archive("clouds.mcpack.zip"));
        assert!(!is_pack_archive(""));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_world_that_never_had_a_pack_reads_as_an_empty_list() {
        let dir = temp_dir("missing-list");
        let world = dir.join("minecraftWorlds").join("abc");
        std::fs::create_dir_all(&world).unwrap();
        let list = world.join("world_resource_packs.json");
        assert!(read_activation(&list).unwrap().is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_pack_list_that_is_not_a_list_is_refused() {
        let dir = temp_dir("bad-list");
        let path = dir.join("world_resource_packs.json");
        std::fs::write(&path, r#"{"pack_id":"x"}"#).unwrap();
        let not_a_list = read_activation(&path).unwrap_err();
        assert!(not_a_list.contains("not a list"), "{not_a_list}");

        std::fs::write(&path, r#"["not an identity"]"#).unwrap();
        let not_an_entry = read_activation(&path).unwrap_err();
        assert!(
            not_an_entry.contains("not a pack identity"),
            "{not_an_entry}"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn activating_appends_once_and_updates_a_changed_version_in_place() {
        let first = activate(&[], PACK_ID, &json!([1, 0, 0])).unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0]["pack_id"], json!(PACK_ID));
        assert_eq!(first[0]["version"], json!([1, 0, 0]));

        let again = activate(&first, PACK_ID, &json!([1, 0, 0])).unwrap();
        assert_eq!(again.len(), 1, "the same pack must not be listed twice");

        let updated = activate(&first, PACK_ID, &json!([2, 1, 0])).unwrap();
        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0]["version"], json!([2, 1, 0]));

        // A pack this launcher never installed stays exactly as it was.
        let foreign_id = "99999999-8888-7777-6666-555555555555";
        let foreign = json!({ "pack_id": foreign_id, "version": [3, 0, 0] });
        let mut entries = first.clone();
        entries.push(foreign.clone());
        let kept = activate(&entries, OTHER_ID, &json!([1, 2, 3])).unwrap();
        assert_eq!(kept.len(), 3);
        assert_eq!(kept[1], foreign);
    }

    #[test]
    fn a_fabricated_identity_is_refused_before_it_reaches_a_world() {
        let bogus_id = activate(&[], "not-a-uuid", &json!([1, 0, 0]));
        assert!(bogus_id.is_err());
        let bogus_version = activate(&[], PACK_ID, &json!("1.0"));
        assert!(bogus_version.is_err());
    }

    #[test]
    fn a_pack_list_round_trips_and_never_invents_a_world_folder() {
        let dir = temp_dir("round-trip");
        let world = dir.join("minecraftWorlds").join("abc");
        std::fs::create_dir_all(&world).unwrap();
        let path = world.join("world_behavior_packs.json");
        let entries = activate(&[], PACK_ID, &json!([1, 0, 0])).unwrap();
        write_activation(&path, &entries).unwrap();
        assert_eq!(read_activation(&path).unwrap(), entries);

        // Replacing an existing list keeps exactly the new contents.
        let second = activate(&entries, OTHER_ID, &json!([1, 1, 0])).unwrap();
        write_activation(&path, &second).unwrap();
        assert_eq!(read_activation(&path).unwrap(), second);
        assert_eq!(read_activation(&path).unwrap().len(), 2);

        let missing = dir.join("minecraftWorlds").join("gone");
        let list = missing.join("world_resource_packs.json");
        let refused = write_activation(&list, &entries).unwrap_err();
        assert!(refused.contains("refusing to create"), "{refused}");
        assert!(!missing.exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn module_kinds_and_destinations_are_the_documented_ones() {
        assert_eq!(module_kind("resources"), PackKind::Resource);
        assert_eq!(module_kind("data"), PackKind::Behaviour);
        assert_eq!(module_kind("skin_packs"), PackKind::Skin);
        assert_eq!(module_kind("client_data"), PackKind::Data);
        assert_eq!(module_kind("script"), PackKind::Unknown);
        assert_eq!(
            destination_folder(PackKind::Resource),
            Some("resource_packs")
        );
        assert_eq!(
            destination_folder(PackKind::Behaviour),
            Some("behavior_packs")
        );
        assert_eq!(destination_folder(PackKind::Skin), Some("skin_packs"));
        assert_eq!(destination_folder(PackKind::Data), None);
        assert_eq!(destination_folder(PackKind::Unknown), None);

        let only_data = r#"{"header":{"name":"Client","uuid":"11111111-2222-3333-4444-555555555555","version":[1,0,0]},"modules":[{"type":"client_data"}]}"#;
        let manifest: PackManifest = serde_json::from_str(only_data).unwrap();
        assert_eq!(kind_of(&manifest), PackKind::Data);

        let no_modules = r#"{"header":{"name":"Empty","uuid":"11111111-2222-3333-4444-555555555555","version":[1,0,0]}}"#;
        let manifest: PackManifest = serde_json::from_str(no_modules).unwrap();
        assert_eq!(kind_of(&manifest), PackKind::Unknown);

        assert!(is_version(&json!([1, 0, 0])));
        assert!(!is_version(&json!([])));
        assert!(!is_version(&json!(2)));
    }
}
