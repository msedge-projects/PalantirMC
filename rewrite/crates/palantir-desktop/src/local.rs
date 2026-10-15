//! Filesystem plans cross the worker boundary, never arbitrary shell commands.
//! Confirmation re-reads the plan so a stale review cannot authorize new bytes.
#[cfg(test)]
#[path = "local_tests.rs"]
mod tests;
use crate::model::Result;
use crate::surfaces::AdditionKind;
use palantir_bedrock::{Channel, content};
use palantir_net::download::sha1_file;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

const MAX_FILES: usize = 100_000;
const MAX_BYTES: u64 = 32 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Java(String),
    Bedrock(Channel),
}
impl Target {
    pub fn key(&self) -> String {
        match self {
            Self::Java(id) => id.clone(),
            Self::Bedrock(_) => "bedrock-local".into(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    World {
        target: Target,
        folder: String,
        output: String,
    },
    Addition {
        target: Target,
        kind: AdditionKind,
        source: String,
        world: Option<String>,
    },
}
impl Input {
    pub fn target(&self) -> &Target {
        match self {
            Self::World { target, .. } | Self::Addition { target, .. } => target,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStamp {
    pub relative: String,
    pub bytes: u64,
    pub sha1: String,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Activation {
    pub path: PathBuf,
    pub before: Vec<serde_json::Value>,
    pub after: Vec<serde_json::Value>,
    pub receipts: Vec<Option<FileStamp>>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub input: Input,
    pub root: PathBuf,
    pub source: PathBuf,
    pub destination: PathBuf,
    pub label: String,
    pub files: Vec<FileStamp>,
    pub bytes: u64,
    pub expanded_files: Vec<(String, u64)>,
    pub pack: Option<content::PlannedPack>,
    pub bundle: Option<Box<crate::addon::Bundle>>,
    pub activation: Option<Activation>,
}
#[derive(Default)]
pub struct State {
    pub revision: u64,
    pub loading: bool,
    pub plan: Option<Plan>,
    pub error: Option<String>,
    pub closed_game: bool,
    pub activation_world: Option<String>,
    /// The last confirmed operation's receipt or failure. It lives in the
    /// review dialog because the modal scrim hides the banner behind it.
    pub outcome: Option<std::result::Result<String, String>>,
}
impl State {
    pub fn invalidate(&mut self) {
        self.revision += 1;
        self.loading = false;
        self.plan = None;
        self.error = None;
        self.closed_game = false;
        self.outcome = None;
    }
}

pub fn checked(path: &Path) -> Result<std::fs::Metadata> {
    if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err("Use an absolute path without parent-directory segments.".into());
    }
    let mut current = PathBuf::new();
    for part in path.components() {
        current.push(part);
        let meta = std::fs::symlink_metadata(&current)
            .map_err(|e| format!("{}: {e}", current.display()))?;
        let linked = meta.file_type().is_symlink();
        #[cfg(windows)]
        let linked = {
            use std::os::windows::fs::MetadataExt;
            linked || meta.file_attributes() & 0x400 != 0
        };
        if linked {
            return Err(format!("Refusing linked path {}.", current.display()));
        }
    }
    std::fs::symlink_metadata(path).map_err(|e| e.to_string())
}
pub(crate) fn directory(path: &Path) -> Result<()> {
    if !checked(path)?.is_dir() {
        return Err(format!("{} is not a directory.", path.display()));
    }
    Ok(())
}
pub(crate) fn absent(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
        Ok(_) => Err(format!(
            "{} already exists. Choose a new destination; nothing is overwritten.",
            path.display()
        )),
    }
}
pub(crate) fn stamp(path: &Path, relative: String) -> Result<FileStamp> {
    let meta = checked(path)?;
    if !meta.is_file() || meta.len() > MAX_BYTES {
        return Err("Only bounded regular files are supported.".into());
    }
    Ok(FileStamp {
        relative,
        bytes: meta.len(),
        sha1: sha1_file(path).map_err(|e| e.to_string())?,
    })
}
fn collect(
    root: &Path,
    path: &Path,
    out: &mut Vec<FileStamp>,
    total: &mut u64,
    depth: usize,
) -> Result<()> {
    directory(path)?;
    if depth > 40 {
        return Err("World exceeds the 40-directory depth limit.".into());
    }
    for entry in std::fs::read_dir(path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = checked(&entry.path())?;
        crate::files::local_name(&entry.file_name().to_string_lossy())?;
        if kind.is_dir() {
            collect(root, &entry.path(), out, total, depth + 1)?;
        } else if kind.is_file() {
            let rel = entry
                .path()
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            let file = stamp(&entry.path(), rel)?;
            *total = total
                .checked_add(file.bytes)
                .ok_or("World size overflow.")?;
            out.push(file);
            if *total > MAX_BYTES || out.len() > MAX_FILES {
                return Err("World exceeds 32 GiB or 100,000 files.".into());
            }
        } else {
            return Err("World contains a non-regular file.".into());
        }
    }
    Ok(())
}

pub fn review(root: &Path, input: &Input) -> Result<Plan> {
    directory(root)?;
    match input {
        Input::World {
            target,
            folder,
            output,
        } => {
            crate::files::local_name(folder)?;
            let source = root
                .join(if matches!(target, Target::Java(_)) {
                    "saves"
                } else {
                    "minecraftWorlds"
                })
                .join(folder);
            directory(&source)?;
            if !checked(&source.join("level.dat"))?.is_file() {
                return Err("This folder has no regular level.dat world file.".into());
            }
            let destination = PathBuf::from(output.trim());
            let extension = if matches!(target, Target::Java(_)) {
                "zip"
            } else {
                "mcworld"
            };
            if !destination.is_absolute()
                || destination
                    .extension()
                    .and_then(|v| v.to_str())
                    .is_none_or(|v| !v.eq_ignore_ascii_case(extension))
            {
                return Err(format!(
                    "Choose an absolute output path ending in .{extension}."
                ));
            }
            directory(
                destination
                    .parent()
                    .ok_or("Choose an existing output directory.")?,
            )?;
            absent(&destination)?;
            if destination
                .parent()
                .ok_or("Missing output directory.")?
                .canonicalize()
                .map_err(|e| e.to_string())?
                .starts_with(source.canonicalize().map_err(|e| e.to_string())?)
            {
                return Err("The output cannot be inside the world it copies.".into());
            }
            let mut files = Vec::new();
            let mut bytes = 0;
            collect(&source, &source, &mut files, &mut bytes, 0)?;
            files.sort_by(|a, b| a.relative.cmp(&b.relative));
            Ok(Plan {
                input: input.clone(),
                root: root.into(),
                source,
                destination,
                label: folder.clone(),
                files,
                bytes,
                expanded_files: Vec::new(),
                pack: None,
                bundle: None,
                activation: None,
            })
        }
        Input::Addition {
            target,
            kind,
            source,
            world,
        } => {
            let source = PathBuf::from(source.trim());
            let name = source
                .file_name()
                .and_then(|v| v.to_str())
                .ok_or("Choose a named local file.")?;
            crate::files::local_name(name)?;
            let label = name.to_string();
            let file = stamp(&source, name.into())?;
            let mut pack = None;
            let mut activation = None;
            let mut expanded_files = Vec::new();
            let destination = match target {
                Target::Java(_) => {
                    let (folder, ext) = match kind {
                        AdditionKind::Mod => ("mods", "jar"),
                        AdditionKind::Resource => ("resourcepacks", "zip"),
                        AdditionKind::Shader => ("shaderpacks", "zip"),
                        _ => return Err("Bedrock content cannot be added to Java.".into()),
                    };
                    if source
                        .extension()
                        .and_then(|v| v.to_str())
                        .is_none_or(|v| !v.eq_ignore_ascii_case(ext))
                    {
                        return Err(format!("This content type requires a .{ext} file."));
                    }
                    if *kind == AdditionKind::Mod && file.bytes == 0 {
                        return Err("An empty file is not a mod.".into());
                    }
                    let dir = root.join(folder);
                    if dir.try_exists().map_err(|e| e.to_string())? {
                        directory(&dir)?;
                    } else {
                        absent(&dir)?;
                    }
                    dir.join(name)
                }
                Target::Bedrock(_) => {
                    if *kind != AdditionKind::BedrockPack {
                        return Err("Java content cannot be added to Bedrock.".into());
                    }
                    if let Some(bundle) = crate::addon::review(&source, root, world.as_deref())? {
                        let expanded_files = bundle
                            .packs
                            .iter()
                            .flat_map(|pack| {
                                pack.files.iter().map(move |(_, relative, bytes)| {
                                    (
                                        format!("{}/{}", pack.destination.display(), relative),
                                        *bytes,
                                    )
                                })
                            })
                            .collect();
                        return Ok(Plan {
                            input: input.clone(),
                            root: root.into(),
                            source,
                            destination: root.into(),
                            label,
                            bytes: file.bytes,
                            files: vec![file],
                            expanded_files,
                            pack: None,
                            bundle: Some(Box::new(bundle)),
                            activation: None,
                        });
                    }
                    let planned = content::plan_pack(&source, name)?;
                    expanded_files = archive_entries(&source)?
                        .into_iter()
                        .map(|(_, name, bytes)| (name, bytes))
                        .collect();
                    let folder = planned.destination.ok_or("Pack has no destination.")?;
                    let dir = root.join(folder);
                    if dir.try_exists().map_err(|e| e.to_string())? {
                        directory(&dir)?;
                    } else {
                        absent(&dir)?;
                    }
                    if let Some(world) = world {
                        if !matches!(
                            planned.kind,
                            content::PackKind::Resource | content::PackKind::Behaviour
                        ) {
                            return Err(
                                "Only resource and behaviour packs activate per world.".into()
                            );
                        }
                        crate::files::local_name(world)?;
                        let world_dir = root.join("minecraftWorlds").join(world);
                        directory(&world_dir)?;
                        if !checked(&world_dir.join("level.dat"))?.is_file() {
                            return Err("Selected world is no longer present.".into());
                        }
                        let path = world_dir.join(if planned.kind == content::PackKind::Resource {
                            "world_resource_packs.json"
                        } else {
                            "world_behavior_packs.json"
                        });
                        let mut receipts = Vec::new();
                        for file in [&path, &path.with_extension("bak")] {
                            receipts.push(match std::fs::symlink_metadata(file) {
                                Ok(_) => Some(stamp(
                                    file,
                                    file.file_name()
                                        .ok_or("Invalid pack-list name.")?
                                        .to_string_lossy()
                                        .into(),
                                )?),
                                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                                Err(e) => return Err(e.to_string()),
                            });
                        }
                        absent(&path.with_extension("tmp"))?;
                        let before = content::read_activation(&path)?;
                        let after = content::activate(&before, &planned.uuid, &planned.version)?;
                        activation = Some(Activation {
                            path,
                            before,
                            after,
                            receipts,
                        });
                    }
                    let dest = dir.join(planned.uuid.to_lowercase());
                    pack = Some(planned);
                    dest
                }
            };
            absent(&destination)?;
            Ok(Plan {
                input: input.clone(),
                root: root.into(),
                source,
                destination,
                label,
                bytes: file.bytes,
                files: vec![file],
                expanded_files,
                pack,
                bundle: None,
                activation,
            })
        }
    }
}

/// Every archive path is checked even when it lies outside the chosen manifest's
/// prefix, so a disguised multi-pack or zip-slip input is refused, not guessed.
pub(crate) fn safe_name(part: &str) -> Result<()> {
    crate::files::local_name(part)?;
    let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
    if part.contains(['<', '>', '"', '|', '?', '*'])
        || part.ends_with(['.', ' '])
        || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
    {
        return Err("Archive contains a filename reserved by Windows.".into());
    }
    Ok(())
}
type ArchiveEntry = (usize, String, u64);
fn archive_entries(source: &Path) -> Result<Vec<ArchiveEntry>> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(source).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if zip.len() > MAX_FILES {
        return Err("Pack exceeds 100,000 entries.".into());
    }
    let manifests: Vec<_> = zip
        .file_names()
        .filter(|n| *n == "manifest.json" || n.ends_with("/manifest.json"))
        .map(str::to_string)
        .collect();
    if manifests.len() != 1 {
        return Err(
            "Import one pack with one manifest at a time; split multi-pack add-ons first.".into(),
        );
    }
    let prefix = manifests[0]
        .strip_suffix("manifest.json")
        .ok_or("Missing manifest.")?
        .to_string();
    let mut names = std::collections::BTreeSet::new();
    let mut files = Vec::new();
    let mut total = 0u64;
    for index in 0..zip.len() {
        let entry = zip.by_index(index).map_err(|e| e.to_string())?;
        let name = entry.name();
        if name.contains('\\')
            || name.starts_with('/')
            || entry.unix_mode().is_some_and(|v| v & 0o170000 == 0o120000)
        {
            return Err("Linked or unsafe archive entry.".into());
        }
        for part in name.trim_end_matches('/').split('/') {
            safe_name(part)?;
        }
        if name.split('/').count() > 40 {
            return Err("Pack path is too deep.".into());
        }
        let relative = name
            .strip_prefix(&prefix)
            .ok_or("Archive contains files outside its pack folder.")?;
        if relative.is_empty() || entry.is_dir() {
            continue;
        }
        if !names.insert(relative.to_lowercase()) {
            return Err("Duplicate archive destination (including case variants).".into());
        }
        total = total
            .checked_add(entry.size())
            .ok_or("Pack size overflow.")?;
        if total > MAX_BYTES {
            return Err("Pack expands past 32 GiB.".into());
        }
        files.push((index, relative.into(), entry.size()));
    }
    // A file cannot also be the parent of another file, on either platform.
    for (_, name, _) in &files {
        let mut parent = Path::new(name).parent();
        while let Some(path) = parent {
            if names.contains(&path.to_string_lossy().replace('\\', "/").to_lowercase()) {
                return Err("Conflicting archive paths.".into());
            }
            parent = path.parent();
        }
    }
    Ok(files)
}

fn verify_world_copy(plan: &Plan) -> Result<()> {
    let mut now = Vec::new();
    let mut bytes = 0;
    collect(&plan.source, &plan.source, &mut now, &mut bytes, 0)?;
    now.sort_by(|a, b| a.relative.cmp(&b.relative));
    if now != plan.files {
        return Err("World changed while copying. Close the game and review again.".into());
    }
    let mut zip =
        zip::ZipArchive::new(std::fs::File::open(&plan.destination).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if zip.len() != plan.files.len() {
        return Err("Archive file count does not match review.".into());
    }
    for file in &plan.files {
        let mut entry = zip.by_name(&file.relative).map_err(|e| e.to_string())?;
        if entry.size() != file.bytes {
            return Err("Archive file size does not match review.".into());
        }
        // Verify through a bounded streaming digest, not a whole-world allocation.
        let hash = palantir_net::download::sha1_reader(&mut entry).map_err(|e| e.to_string())?;
        if hash != file.sha1 {
            return Err("Archive bytes do not match review.".into());
        }
    }
    Ok(())
}
pub fn apply(root: &Path, plan: &Plan) -> Result<String> {
    if review(root, &plan.input)? != *plan {
        return Err("Files, destination, or pack lists changed since review. Review again; nothing was written.".into());
    }
    if let Some(bundle) = &plan.bundle {
        return crate::addon::apply(plan, bundle);
    }
    match &plan.input {
        Input::World {
            target: Target::Bedrock(_),
            ..
        } => {
            let report = palantir_bedrock::worlds::export_world(
                &plan.source,
                &plan.destination,
                &plan.label,
            )?;
            if let Err(error) = verify_world_copy(plan) {
                std::fs::remove_file(&plan.destination)
                    .map_err(|cleanup| format!("{error}; cleanup failed: {cleanup}"))?;
                return Err(error);
            }
            Ok(format!(
                "Exported {} files ({} bytes) to {}. The original world was left untouched.",
                report.entries,
                report.bytes,
                plan.destination.display()
            ))
        }
        Input::World { .. } => {
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&plan.destination)
                .map_err(|e| e.to_string())?;
            let result = (|| {
                let mut zip = zip::ZipWriter::new(file);
                for entry in &plan.files {
                    zip.start_file(&entry.relative, zip::write::SimpleFileOptions::default())
                        .map_err(|e| e.to_string())?;
                    let path = plan.source.join(&entry.relative);
                    checked(&path)?;
                    std::io::copy(
                        &mut std::fs::File::open(&path).map_err(|e| e.to_string())?,
                        &mut zip,
                    )
                    .map_err(|e| e.to_string())?;
                }
                zip.finish()
                    .map_err(|e| e.to_string())?
                    .sync_all()
                    .map_err(|e| e.to_string())?;
                verify_world_copy(plan)
            })();
            if let Err(error) = result {
                std::fs::remove_file(&plan.destination)
                    .map_err(|e| format!("{error}; partial archive cleanup failed: {e}"))?;
                return Err(error);
            }
            Ok(format!(
                "Saved {} files to {}. The original world was left untouched.",
                plan.files.len(),
                plan.destination.display()
            ))
        }
        Input::Addition { target, .. } => {
            let parent = plan
                .destination
                .parent()
                .ok_or("Missing destination directory.")?;
            if !parent.exists() {
                std::fs::create_dir(parent).map_err(|e| e.to_string())?;
            }
            directory(parent)?;
            if matches!(target, Target::Java(_)) {
                let mut output = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&plan.destination)
                    .map_err(|e| e.to_string())?;
                let result = (|| {
                    std::io::copy(
                        &mut std::fs::File::open(&plan.source).map_err(|e| e.to_string())?,
                        &mut output,
                    )
                    .map_err(|e| e.to_string())?;
                    output.flush().map_err(|e| e.to_string())?;
                    if stamp(&plan.destination, plan.files[0].relative.clone())? != plan.files[0] {
                        return Err("Source changed during copying.".into());
                    }
                    output.sync_all().map_err(|e| e.to_string())
                })();
                drop(output);
                if let Err(error) = result {
                    std::fs::remove_file(&plan.destination)
                        .map_err(|e| format!("{error}; cleanup failed: {e}"))?;
                    return Err(error);
                }
            } else {
                let entries = archive_entries(&plan.source)?;
                let mut zip = zip::ZipArchive::new(
                    std::fs::File::open(&plan.source).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                std::fs::create_dir(&plan.destination).map_err(|e| e.to_string())?;
                let result = (|| {
                    for (index, relative, size) in entries {
                        let path = plan.destination.join(relative);
                        std::fs::create_dir_all(path.parent().ok_or("Missing entry parent.")?)
                            .map_err(|e| e.to_string())?;
                        directory(path.parent().ok_or("Missing entry parent.")?)?;
                        let mut output = std::fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(&path)
                            .map_err(|e| e.to_string())?;
                        let mut entry = zip
                            .by_index(index)
                            .map_err(|e| e.to_string())?
                            .take(size + 1);
                        if std::io::copy(&mut entry, &mut output).map_err(|e| e.to_string())?
                            != size
                        {
                            return Err("Archive entry size changed.".into());
                        }
                    }
                    if stamp(&plan.source, plan.files[0].relative.clone())? != plan.files[0] {
                        return Err("Pack changed during extraction.".into());
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    std::fs::remove_dir_all(&plan.destination)
                        .map_err(|e| format!("{error}; partial pack cleanup failed: {e}"))?;
                    return Err(error);
                }
                if let Some(activation) = &plan.activation {
                    // Recheck after extraction as well; an external writer may
                    // have changed the list while the pack was being unpacked.
                    if content::read_activation(&activation.path)? != activation.before {
                        return Err("Pack installed, but world pack list changed. Activation was not applied; enable the pack in-game.".into());
                    }
                    content::write_activation(&activation.path, &activation.after).map_err(
                        |e| {
                            format!(
                                "Pack installed, but activation failed: {e}. Enable it in-game."
                            )
                        },
                    )?;
                }
            }
            Ok(format!(
                "Imported {} to {}. {}",
                plan.label,
                plan.destination.display(),
                if plan.activation.is_some() {
                    "Enabled for the selected world; other pack entries were preserved."
                } else {
                    "No world pack lists were changed."
                }
            ))
        }
    }
}
