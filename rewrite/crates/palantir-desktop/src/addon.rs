//! A .mcaddon is a set of packs, not one guessed destination. Every member
//! is reviewed before any destination is created; existing packs are refused.
#[cfg(test)]
#[path = "addon_tests.rs"]
mod tests;
use crate::{local, model::Result};
use palantir_bedrock::content;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

const MAX_NESTED: u64 = 128 * 1024 * 1024;
const MAX_BYTES: u64 = 32 * 1024 * 1024 * 1024;
#[derive(Debug, Clone, PartialEq)]
pub struct Pack {
    pub member: Option<String>,
    pub prefix: String,
    pub name: String,
    pub uuid: String,
    pub version: serde_json::Value,
    pub destination: PathBuf,
    pub files: Vec<(usize, String, u64)>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Bundle {
    pub packs: Vec<Pack>,
    pub activations: Vec<local::Activation>,
}
fn zip<T: Read + std::io::Seek>(reader: T) -> Result<zip::ZipArchive<T>> {
    zip::ZipArchive::new(reader).map_err(|e| e.to_string())
}
fn checked_entries<T: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<T>,
) -> Result<Vec<(usize, String, u64)>> {
    if archive.len() > 100_000 {
        return Err("Add-on exceeds 100,000 entries.".into());
    }
    let mut entries: Vec<(usize, String, u64)> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for index in 0..archive.len() {
        let file = archive.by_index(index).map_err(|e| e.to_string())?;
        let name = file.name().trim_end_matches('/');
        if name.contains('\\')
            || name.starts_with('/')
            || name.split('/').count() > 40
            || file.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
        {
            return Err("Unsafe or linked add-on entry.".into());
        }
        for part in name.split('/') {
            local::safe_name(part)?;
        }
        if !seen.insert(name.to_lowercase()) {
            return Err("Duplicate add-on entry, including case variants.".into());
        }
        if !file.is_dir() {
            entries.push((index, name.into(), file.size()));
        }
    }
    for (_, name, _) in &entries {
        let mut parent = Path::new(name).parent();
        while let Some(path) = parent {
            let parent_name = path.to_string_lossy().replace('\\', "/").to_lowercase();
            if entries
                .iter()
                .any(|(_, name, _)| name.to_lowercase() == parent_name)
            {
                return Err("Add-on file conflicts with a directory.".into());
            }
            parent = path.parent();
        }
    }
    Ok(entries)
}
fn nested(archive: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Result<Vec<u8>> {
    let file = archive.by_name(name).map_err(|e| e.to_string())?;
    if file.size() > MAX_NESTED {
        return Err("Nested .mcpack exceeds 128 MiB.".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_NESTED + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_NESTED {
        return Err("Nested .mcpack exceeds 128 MiB.".into());
    }
    Ok(bytes)
}
fn pack<T: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<T>,
    entries: &[(usize, String, u64)],
    prefix: &str,
    member: Option<String>,
    root: &Path,
) -> Result<Pack> {
    let manifest_path = format!("{prefix}manifest.json");
    let file = archive.by_name(&manifest_path).map_err(|e| e.to_string())?;
    if file.size() > 1024 * 1024 {
        return Err("Pack manifest exceeds 1 MiB.".into());
    }
    let mut text = String::new();
    file.take(1024 * 1024 + 1)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    if text.len() > 1024 * 1024 {
        return Err("Pack manifest exceeds 1 MiB.".into());
    }
    let manifest: content::PackManifest = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if !content::is_uuid(&manifest.header.uuid) || !content::is_version(&manifest.header.version) {
        return Err("Pack manifest has an invalid UUID or version.".into());
    }
    let kind = content::kind_of(&manifest);
    let folder = content::destination_folder(kind)
        .ok_or("Pack modules do not define one supported destination.")?;
    let parent = root.join(folder);
    match std::fs::symlink_metadata(&parent) {
        Ok(_) => local::directory(&parent)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.to_string()),
    }
    let destination = parent.join(manifest.header.uuid.to_lowercase());
    local::absent(&destination)?;
    let files = entries
        .iter()
        .filter_map(|(index, name, size)| {
            name.strip_prefix(prefix)
                .map(|relative| (*index, relative.to_string(), *size))
        })
        .collect();
    Ok(Pack {
        member,
        prefix: prefix.into(),
        name: manifest.header.name,
        uuid: manifest.header.uuid.to_lowercase(),
        version: manifest.header.version,
        destination,
        files,
    })
}
/// None means a single manifest archive, handled by the existing single-pack path.
pub fn review(source: &Path, root: &Path, world: Option<&str>) -> Result<Option<Bundle>> {
    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Choose a named Bedrock pack container.")?;
    if !matches!(
        palantir_bedrock::addons::classify(name),
        palantir_bedrock::addons::AddonKind::Pack
            | palantir_bedrock::addons::AddonKind::Addon
            | palantir_bedrock::addons::AddonKind::SkinPack
    ) {
        return Err(
            "Choose a .mcpack, .mcaddon or .mcskin container; worlds belong to the world flow."
                .into(),
        );
    }
    let mut archive = zip(std::fs::File::open(source).map_err(|e| e.to_string())?)?;
    let entries = checked_entries(&mut archive)?;
    let manifests: Vec<_> = entries
        .iter()
        .filter(|(_, n, _)| n == "manifest.json" || n.ends_with("/manifest.json"))
        .map(|(_, n, _)| n.strip_suffix("manifest.json").unwrap_or("").to_string())
        .collect();
    let members: Vec<_> = entries
        .iter()
        .filter(|(_, n, _)| n.to_ascii_lowercase().ends_with(".mcpack"))
        .map(|(_, n, _)| n.clone())
        .collect();
    if manifests.len() == 1 && members.is_empty() {
        return Ok(None);
    }
    if manifests.is_empty() && members.is_empty() {
        return Err("Add-on contains no pack manifests or .mcpack members.".into());
    }
    if manifests.len() + members.len() > 64 {
        return Err("Add-on exceeds 64 packs.".into());
    }
    let mut packs = Vec::new();
    for prefix in &manifests {
        packs.push(pack(&mut archive, &entries, prefix, None, root)?);
    }
    let nested_total = entries
        .iter()
        .filter(|(_, n, _)| members.contains(n))
        .try_fold(0u64, |sum, (_, _, size)| sum.checked_add(*size))
        .ok_or("Nested pack size overflow.")?;
    if nested_total > 512 * 1024 * 1024 {
        return Err("Nested packs exceed 512 MiB combined.".into());
    }
    for member in &members {
        let mut inner = zip(Cursor::new(nested(&mut archive, member)?))?;
        let files = checked_entries(&mut inner)?;
        let prefixes: Vec<_> = files
            .iter()
            .filter(|(_, n, _)| n == "manifest.json" || n.ends_with("/manifest.json"))
            .map(|(_, n, _)| n.strip_suffix("manifest.json").unwrap_or("").to_string())
            .collect();
        if prefixes.len() != 1 {
            return Err("Each nested .mcpack must contain exactly one manifest.".into());
        }
        if files
            .iter()
            .any(|(_, name, _)| !name.starts_with(&prefixes[0]))
        {
            return Err("Nested pack contains files outside its manifest folder.".into());
        }
        packs.push(pack(
            &mut inner,
            &files,
            &prefixes[0],
            Some(member.clone()),
            root,
        )?);
    }
    // Refuse overlapping roots and stray files rather than silently dropping data.
    for (index, prefix) in manifests.iter().enumerate() {
        if manifests
            .iter()
            .enumerate()
            .any(|(other, p)| other != index && p.starts_with(prefix))
        {
            return Err("Overlapping pack manifest folders.".into());
        }
    }
    for (_, name, _) in &entries {
        if !members.contains(name) && !manifests.iter().any(|p| name.starts_with(p)) {
            return Err("Add-on contains files outside its pack folders.".into());
        }
    }
    let mut ids = std::collections::BTreeSet::new();
    let mut bytes = 0u64;
    let mut count = 0;
    for pack in &packs {
        if !ids.insert(pack.uuid.clone()) {
            return Err("Add-on repeats a pack UUID; no destinations were created.".into());
        }
        for (_, _, size) in &pack.files {
            bytes = bytes.checked_add(*size).ok_or("Add-on size overflow.")?;
            count += 1;
        }
    }
    if bytes > MAX_BYTES || count > 100_000 {
        return Err("Add-on exceeds 32 GiB or 100,000 extracted files.".into());
    }
    let mut activations = Vec::new();
    if let Some(world) = world {
        local::safe_name(world)?;
        let directory = root.join("minecraftWorlds").join(world);
        local::directory(&directory)?;
        if !local::checked(&directory.join("level.dat"))?.is_file() {
            return Err("Selected world has no level.dat.".into());
        }
        for (folder, filename) in [
            ("resource_packs", "world_resource_packs.json"),
            ("behavior_packs", "world_behavior_packs.json"),
        ] {
            let selected: Vec<_> = packs
                .iter()
                .filter(|p| p.destination.parent() == Some(root.join(folder).as_path()))
                .collect();
            if selected.is_empty() {
                continue;
            }
            let path = directory.join(filename);
            local::absent(&path.with_extension("tmp"))?;
            let mut receipts = Vec::new();
            for file in [&path, &path.with_extension("bak")] {
                receipts.push(match std::fs::symlink_metadata(file) {
                    Ok(_) => Some(local::stamp(
                        file,
                        file.file_name()
                            .ok_or("Invalid pack list.")?
                            .to_string_lossy()
                            .into(),
                    )?),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                    Err(e) => return Err(e.to_string()),
                });
            }
            let before = content::read_activation(&path)?;
            let mut after = before.clone();
            for pack in selected {
                after = content::activate(&after, &pack.uuid, &pack.version)?;
            }
            activations.push(local::Activation {
                path,
                before,
                after,
                receipts,
            });
        }
    }
    Ok(Some(Bundle { packs, activations }))
}
fn extract<T: Read + std::io::Seek>(archive: &mut zip::ZipArchive<T>, pack: &Pack) -> Result<()> {
    for (index, relative, size) in &pack.files {
        let path = pack.destination.join(relative);
        let parent = path.parent().ok_or("Missing entry parent.")?;
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        local::directory(parent)?;
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        let mut file = archive
            .by_index(*index)
            .map_err(|e| e.to_string())?
            .take(size + 1);
        if std::io::copy(&mut file, &mut output).map_err(|e| e.to_string())? != *size {
            return Err("Add-on entry size mismatch.".into());
        }
    }
    Ok(())
}
pub fn apply(plan: &local::Plan, bundle: &Bundle) -> Result<String> {
    let mut archive = zip(std::fs::File::open(&plan.source).map_err(|e| e.to_string())?)?;
    let mut created = Vec::new();
    let result = (|| {
        for pack in &bundle.packs {
            let parent = pack.destination.parent().ok_or("Missing pack parent.")?;
            if !parent.exists() {
                std::fs::create_dir(parent).map_err(|e| e.to_string())?;
            }
            local::directory(parent)?;
            std::fs::create_dir(&pack.destination).map_err(|e| e.to_string())?;
            created.push(pack.destination.clone());
            if let Some(member) = &pack.member {
                let mut inner = zip(Cursor::new(nested(&mut archive, member)?))?;
                extract(&mut inner, pack)?;
            } else {
                extract(&mut archive, pack)?;
            }
        }
        if local::stamp(&plan.source, plan.files[0].relative.clone())? != plan.files[0] {
            return Err("Add-on changed during extraction.".into());
        }
        Ok(())
    })();
    if let Err(error) = result {
        for path in created {
            std::fs::remove_dir_all(path)
                .map_err(|cleanup| format!("{error}; partial add-on cleanup failed: {cleanup}"))?;
        }
        return Err(error);
    }
    for activation in &bundle.activations {
        if content::read_activation(&activation.path)? != activation.before {
            return Err("Add-on installed, but a world pack list changed. Enable the packs in-game; activation was not applied.".into());
        }
    }
    for activation in &bundle.activations {
        content::write_activation(&activation.path, &activation.after).map_err(|e| format!("Add-on installed; activation could not finish: {e}. Check both world pack lists or enable the packs in-game."))?;
    }
    Ok(format!(
        "Imported {} packs. {}",
        bundle.packs.len(),
        if bundle.activations.is_empty() {
            "No world pack lists were changed."
        } else {
            "Reviewed resource and behaviour packs enabled for the selected world."
        }
    ))
}
