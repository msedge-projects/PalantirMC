use crate::model::{self, Instance, Loader, Result};
use palantir_core::paths::DataRoot;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub folder: String,
    pub bytes: u64,
    pub directory: bool,
}
pub const FOLDERS: [&str; 5] = ["mods", "resourcepacks", "shaderpacks", "saves", "logs"];
// Check both directory boundaries: rejecting only an instance symlink still
// permits its game subdirectory to redirect file operations outside the root.
pub fn game_directory(root: &DataRoot, id: &str) -> Result<PathBuf> {
    model::instance_path(root, id)?;
    let path = root.game_dir(id);
    let metadata = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("Refusing a linked or non-directory game folder.".into());
    }
    Ok(path)
}
pub fn inventory(root: &DataRoot, id: &str) -> Result<Vec<Entry>> {
    game_directory(root, id)?;
    let mut entries = vec![];
    for folder in FOLDERS {
        let dir = root.game_dir(id).join(folder);
        if !dir.exists() {
            continue;
        }
        if std::fs::symlink_metadata(&dir)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("Refusing a linked content folder.".into());
        }
        for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            if kind.is_symlink() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            entries.push(Entry {
                name,
                folder: folder.into(),
                bytes: entry.metadata().map_err(|e| e.to_string())?.len(),
                directory: kind.is_dir(),
            });
        }
    }
    entries.sort_by_key(|e| (e.folder.clone(), e.name.to_lowercase()));
    Ok(entries)
}
pub fn local_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\\', ':'])
        || name.chars().any(char::is_control)
    {
        return Err("Invalid local filename.".into());
    }
    Ok(())
}
pub fn toggle_mod(root: &DataRoot, id: &str, name: &str) -> Result<()> {
    game_directory(root, id)?;
    local_name(name)?;
    let dir = root.game_dir(id).join("mods");
    if std::fs::symlink_metadata(&dir)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("Refusing a linked mods folder.".into());
    }
    let source = dir.join(name);
    if !std::fs::symlink_metadata(&source)
        .map_err(|e| e.to_string())?
        .is_file()
    {
        return Err("Only regular mod files can be changed.".into());
    }
    let target = if let Some(stem) = name.strip_suffix(".disabled") {
        dir.join(stem)
    } else {
        dir.join(format!("{name}.disabled"))
    };
    if target.exists() {
        return Err("The target filename already exists.".into());
    }
    std::fs::rename(source, target).map_err(|e| e.to_string())
}
pub fn remove_content(root: &DataRoot, id: &str, folder: &str, name: &str) -> Result<()> {
    let game = game_directory(root, id)?;
    local_name(name)?;
    if !["mods", "resourcepacks", "shaderpacks"].contains(&folder) {
        return Err(
            "Only additions can be removed here; worlds require their own backup workflow.".into(),
        );
    }
    let dir = game.join(folder);
    if std::fs::symlink_metadata(&dir)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("Refusing a linked content folder.".into());
    }
    let file = dir.join(name);
    if !std::fs::symlink_metadata(&file)
        .map_err(|e| e.to_string())?
        .is_file()
    {
        return Err("Only ordinary content files may be removed.".into());
    }
    std::fs::remove_file(file).map_err(|e| e.to_string())
}
pub fn open_folder(root: &DataRoot, id: Option<&str>, folder: &str) -> Result<()> {
    let path = match id {
        Some(id) => {
            game_directory(root, id)?;
            if !folder.is_empty() && !FOLDERS.contains(&folder) {
                return Err("Unknown content folder.".into());
            }
            root.game_dir(id).join(folder)
        }
        None => root.root.clone(),
    };
    if path.exists()
        && std::fs::symlink_metadata(&path)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
    {
        return Err("Refusing a linked content folder.".into());
    }
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    let mut command = if cfg!(windows) {
        std::process::Command::new("explorer.exe")
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else {
        std::process::Command::new("xdg-open")
    };
    command
        .arg(&path)
        .spawn()
        .map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
    Ok(())
}

pub fn import_directory(
    root: &DataRoot,
    source: &Path,
    name: &str,
    game: &str,
    loader: Loader,
) -> Result<Instance> {
    let source = source
        .canonicalize()
        .map_err(|e| format!("Import source: {e}"))?;
    if !source.is_dir() {
        return Err("Choose a game directory or a Prism instance directory.".into());
    }
    if source.starts_with(root.root.canonicalize().map_err(|e| e.to_string())?) {
        return Err("Choose a source outside this launcher's data directory.".into());
    }
    let (name, game, loader, pin, content) = if source.join("mmc-pack.json").exists() {
        let pack = palantir_loader::prism::MmcPack::parse(
            &std::fs::read_to_string(source.join("mmc-pack.json")).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let cfg = palantir_loader::prism::InstanceCfg::parse(
            &std::fs::read_to_string(source.join("instance.cfg")).map_err(|e| e.to_string())?,
        );
        let imported =
            palantir_loader::prism::import(&pack, &cfg, "minecraft").map_err(|e| e.to_string())?;
        let (loader, pin) =
            match imported.loader {
                palantir_loader::import::LoaderTarget::Vanilla => (Loader::Vanilla, None),
                palantir_loader::import::LoaderTarget::Fabric { loader } => {
                    (Loader::Fabric, Some(loader))
                }
                palantir_loader::import::LoaderTarget::Quilt { loader } => {
                    (Loader::Quilt, Some(loader))
                }
                _ => return Err(
                    "This loader is not supported by the desktop importer yet. Nothing was copied."
                        .into(),
                ),
            };
        let content = if source.join(".minecraft").is_dir() {
            source.join(".minecraft")
        } else {
            source.join("minecraft")
        };
        (
            if name.trim().is_empty() {
                imported.name
            } else {
                name.into()
            },
            imported.game,
            loader,
            pin,
            content,
        )
    } else {
        (
            name.to_string(),
            game.to_string(),
            loader,
            None,
            source.clone(),
        )
    };
    let content_metadata = std::fs::symlink_metadata(&content).map_err(|e| e.to_string())?;
    if content_metadata.file_type().is_symlink() || !content_metadata.is_dir() {
        return Err("Choose an ordinary game directory, not a linked content directory.".into());
    }
    let mut plan = vec![];
    let mut budget = 0u64;
    for folder in [
        "mods",
        "resourcepacks",
        "shaderpacks",
        "saves",
        "config",
        "defaultconfigs",
    ] {
        let path = content.join(folder);
        if path.exists() {
            collect(&path, Path::new(folder), &mut plan, &mut budget)?;
        }
    }
    for filename in ["options.txt", "servers.dat"] {
        let path = content.join(filename);
        if path.exists() {
            collect(&path, Path::new(filename), &mut plan, &mut budget)?;
        }
    }
    if plan.is_empty() {
        return Err("No worlds, configuration, or content files found in that directory.".into());
    }
    if !loader.desktop_supported() {
        return Err("This loader can be reviewed, but its desktop installer is not connected. Nothing was copied.".into());
    }
    let mut item = model::create(root, &name, &game, loader)?;
    item.loader_version = pin;
    // Persist the pinned loader before copying so a recoverable partial import
    // cannot silently launch using a different release after an interrupted copy.
    model::save_instance(root, &item)?;
    for (from, to) in plan {
        let dest = root.game_dir(&item.id).join(to);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::copy(from, dest).map_err(|e| {
            format!(
                "Copy interrupted; incomplete instance {} was kept for recovery: {e}",
                item.name
            )
        })?;
    }
    model::save_instance(root, &item)?;
    Ok(item)
}
fn collect(
    source: &Path,
    relative: &Path,
    plan: &mut Vec<(PathBuf, PathBuf)>,
    budget: &mut u64,
) -> Result<()> {
    let metadata = std::fs::symlink_metadata(source).map_err(|e| e.to_string())?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "Import contains a symbolic link: {}. Nothing was copied.",
            source.display()
        ));
    }
    if relative.components().count() > 40 || plan.len() >= 100_000 {
        return Err("Import exceeds the supported depth/file limit.".into());
    }
    if metadata.is_dir() {
        for entry in std::fs::read_dir(source).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            collect(
                &entry.path(),
                &relative.join(entry.file_name()),
                plan,
                budget,
            )?;
        }
    } else if metadata.is_file() {
        *budget = budget.saturating_add(metadata.len());
        if *budget > 32 * 1024 * 1024 * 1024 {
            return Err("Import exceeds the 32 GiB copy limit.".into());
        }
        plan.push((source.to_path_buf(), relative.to_path_buf()));
    }
    Ok(())
}
pub fn browse(path: &Path) -> Result<Vec<model::DirectoryEntry>> {
    let mut entries = Vec::new();
    for entry in
        std::fs::read_dir(path).map_err(|e| format!("Cannot browse {}: {e}", path.display()))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() || entry.path().extension().is_some_and(|e| e == "mrpack") {
            entries.push(model::DirectoryEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                path: entry.path().display().to_string(),
                directory: kind.is_dir(),
            });
        }
        if entries.len() > 10_000 {
            return Err("Folder has too many entries; enter a more specific path.".into());
        }
    }
    entries.sort_by_key(|e| (!e.directory, e.name.to_lowercase()));
    Ok(entries)
}
pub fn inspect_source(source: &Path) -> Result<model::ImportReview> {
    let source = source
        .canonicalize()
        .map_err(|e| format!("Import source: {e}"))?;
    if source.extension().is_some_and(|e| e == "mrpack") {
        let file = std::fs::File::open(&source).map_err(|e| e.to_string())?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
        let mut index = archive
            .by_name("modrinth.index.json")
            .map_err(|e| e.to_string())?;
        if index.size() > 8 * 1024 * 1024 {
            return Err("Pack index exceeds the 8 MiB inspection limit.".into());
        }
        let mut text = String::new();
        std::io::Read::read_to_string(&mut index, &mut text).map_err(|e| e.to_string())?;
        let index =
            palantir_loader::mrpack::MrpackIndex::parse(&text).map_err(|e| e.to_string())?;
        let pack = palantir_loader::mrpack::import(&index).map_err(|e| e.to_string())?;
        let (loader, _) = imported_loader(&pack.loader)?;
        let optional = index
            .files
            .iter()
            .filter(|f| f.env.as_ref().and_then(|e| e.client.as_deref()) == Some("optional"))
            .map(|f| f.path.clone())
            .collect();
        return Ok(model::ImportReview { source: source.display().to_string(), name: pack.name, game: pack.game, loader, files: index.files.len(), bytes: index.files.iter().map(|f| f.file_size.unwrap_or(0)).sum(), optional, notes: vec!["Pack metadata inspected without extracting or downloading files.".into(), "Archive installation and optional-file execution require backend pack coordination; no collection will be created by this review.".into()], copy_supported: false });
    }
    if !source.is_dir() {
        return Err("Choose a game directory, Prism directory, or .mrpack file.".into());
    }
    let mut name = source
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Imported collection".into());
    let mut game = String::new();
    let mut loader = Loader::Vanilla;
    let mut content = source.clone();
    if source.join("mmc-pack.json").is_file() {
        let pack = palantir_loader::prism::MmcPack::parse(
            &std::fs::read_to_string(source.join("mmc-pack.json")).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let cfg = palantir_loader::prism::InstanceCfg::parse(
            &std::fs::read_to_string(source.join("instance.cfg")).map_err(|e| e.to_string())?,
        );
        let imported =
            palantir_loader::prism::import(&pack, &cfg, "minecraft").map_err(|e| e.to_string())?;
        name = imported.name;
        game = imported.game;
        loader = imported_loader(&imported.loader)?.0;
        content = if source.join(".minecraft").is_dir() {
            source.join(".minecraft")
        } else {
            source.join("minecraft")
        };
    }
    let metadata = std::fs::symlink_metadata(&content).map_err(|e| e.to_string())?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(
            "The game content root must be an ordinary directory, not a symbolic link.".into(),
        );
    }
    let mut plan = Vec::new();
    let mut bytes = 0;
    for folder in [
        "mods",
        "resourcepacks",
        "shaderpacks",
        "saves",
        "config",
        "defaultconfigs",
        "options.txt",
        "servers.dat",
    ] {
        let path = content.join(folder);
        if path.exists() {
            collect(&path, Path::new(folder), &mut plan, &mut bytes)?;
        }
    }
    if plan.is_empty() {
        return Err(
            "No worlds, settings, or additions found. Choose the game's content folder.".into(),
        );
    }
    Ok(model::ImportReview {
        source: source.display().to_string(),
        name,
        game,
        loader,
        files: plan.len(),
        bytes,
        optional: Vec::new(),
        notes: vec![
            "The source stays unchanged. Accounts, tokens and launcher databases are excluded."
                .into(),
            "The copy is revalidated when submitted; symbolic links are refused.".into(),
        ],
        copy_supported: loader.desktop_supported(),
    })
}
fn imported_loader(
    target: &palantir_loader::import::LoaderTarget,
) -> Result<(Loader, Option<String>)> {
    use palantir_loader::import::LoaderTarget;
    match target {
        LoaderTarget::Vanilla => Ok((Loader::Vanilla, None)),
        LoaderTarget::Fabric { loader } => Ok((Loader::Fabric, Some(loader.clone()))),
        LoaderTarget::Quilt { loader } => Ok((Loader::Quilt, Some(loader.clone()))),
        LoaderTarget::Forge { loader } => Ok((Loader::Forge, Some(loader.clone()))),
        LoaderTarget::NeoForge { loader } => Ok((Loader::NeoForge, Some(loader.clone()))),
        LoaderTarget::Unknown { id, .. } => {
            Err(format!("Unknown loader {id}; no files were copied."))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_filenames_cannot_escape() {
        assert!(local_name("../x.jar").is_err());
        assert!(local_name("Some Mod.jar").is_ok());
    }
    fn test_root(label: &str) -> DataRoot {
        let root = DataRoot::new(std::env::temp_dir().join(format!(
                "palantir-{label}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )));
        std::fs::create_dir_all(&root.root).unwrap();
        root
    }
    #[test]
    fn mods_toggle_without_overwriting_an_existing_file() {
        let root = test_root("toggle");
        let item = model::create(&root, "Test", "1.20.1", Loader::Fabric).unwrap();
        let mods = root.game_dir(&item.id).join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("Some Mod.jar"), b"mod").unwrap();
        toggle_mod(&root, &item.id, "Some Mod.jar").unwrap();
        assert_eq!(
            inventory(&root, &item.id).unwrap()[0].name,
            "Some Mod.jar.disabled"
        );
        toggle_mod(&root, &item.id, "Some Mod.jar.disabled").unwrap();
        std::fs::write(mods.join("Some Mod.jar.disabled"), b"existing").unwrap();
        assert!(toggle_mod(&root, &item.id, "Some Mod.jar").is_err());
        assert_eq!(
            std::fs::read(mods.join("Some Mod.jar.disabled")).unwrap(),
            b"existing"
        );
        std::fs::remove_dir_all(root.root).unwrap();
    }
    #[test]
    fn directory_import_copies_worlds_and_excludes_accounts() {
        let root = test_root("import");
        let source = test_root("source");
        std::fs::create_dir_all(source.root.join("saves/Valley/region")).unwrap();
        std::fs::write(source.root.join("saves/Valley/region/a.mca"), b"region").unwrap();
        std::fs::write(source.root.join("launcher_accounts.json"), b"secret").unwrap();
        let item =
            import_directory(&root, &source.root, "Imported", "1.20.1", Loader::Vanilla).unwrap();
        assert_eq!(
            std::fs::read(root.game_dir(&item.id).join("saves/Valley/region/a.mca")).unwrap(),
            b"region"
        );
        assert!(
            !root
                .game_dir(&item.id)
                .join("launcher_accounts.json")
                .exists()
        );
        assert!(source.root.join("saves/Valley/region/a.mca").exists());
        assert!(
            import_directory(
                &root,
                &root.game_dir(&item.id),
                "Nested",
                "1.20.1",
                Loader::Vanilla
            )
            .is_err()
        );
        std::fs::remove_dir_all(root.root).unwrap();
        std::fs::remove_dir_all(source.root).unwrap();
    }
    #[test]
    fn prism_import_preserves_its_name_and_loader_pin() {
        let root = test_root("prism-import");
        let source = test_root("prism-source");
        std::fs::write(source.root.join("mmc-pack.json"), r#"{"formatVersion":1,"components":[{"uid":"net.minecraft","version":"1.20.1"},{"uid":"net.fabricmc.fabric-loader","version":"0.16.9"}]}"#).unwrap();
        std::fs::write(source.root.join("instance.cfg"), "name=Prism valley\n").unwrap();
        std::fs::create_dir_all(source.root.join(".minecraft/mods")).unwrap();
        std::fs::write(source.root.join(".minecraft/mods/test.jar"), b"mod").unwrap();
        let item = import_directory(&root, &source.root, "", "ignored", Loader::Vanilla).unwrap();
        assert_eq!(item.name, "Prism valley");
        assert_eq!(item.loader, Loader::Fabric);
        assert_eq!(
            model::load_instances(&root).unwrap()[0]
                .loader_version
                .as_deref(),
            Some("0.16.9")
        );
        std::fs::remove_dir_all(root.root).unwrap();
        std::fs::remove_dir_all(source.root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn linked_game_directories_and_import_files_are_refused() {
        let root = test_root("symlink");
        let source = test_root("symlink-source");
        let item = model::create(&root, "Test", "1.20.1", Loader::Vanilla).unwrap();
        std::fs::remove_dir(root.game_dir(&item.id)).unwrap();
        std::os::unix::fs::symlink(&source.root, root.game_dir(&item.id)).unwrap();
        assert!(inventory(&root, &item.id).is_err());
        std::fs::create_dir_all(source.root.join("mods")).unwrap();
        std::os::unix::fs::symlink("/dev/null", source.root.join("mods/linked.jar")).unwrap();
        assert!(
            import_directory(&root, &source.root, "Unsafe", "1.20.1", Loader::Vanilla).is_err()
        );
        assert_eq!(model::load_instances(&root).unwrap().len(), 1);
        std::fs::remove_dir_all(root.root).unwrap();
        std::fs::remove_dir_all(source.root).unwrap();
    }
    #[test]
    fn source_review_is_read_only_and_browsing_lists_import_candidates() {
        let source = test_root("review-source");
        std::fs::create_dir_all(source.root.join("mods")).unwrap();
        std::fs::write(source.root.join("mods/test.jar"), b"mod").unwrap();
        std::fs::write(source.root.join("launcher_accounts.json"), b"secret").unwrap();
        let review = inspect_source(&source.root).unwrap();
        assert_eq!(review.files, 1);
        assert_eq!(review.bytes, 3);
        assert!(review.copy_supported);
        assert!(source.root.join("launcher_accounts.json").exists());
        let entries = browse(&source.root).unwrap();
        assert!(entries.iter().any(|e| e.name == "mods" && e.directory));
        assert!(!entries.iter().any(|e| e.name == "launcher_accounts.json"));
        assert!(!source.root.join("instance.json").exists());
        std::fs::remove_dir_all(source.root).unwrap();
    }
    #[test]
    fn mrpack_review_reports_optional_files_without_installing() {
        let source = test_root("review-pack");
        let path = source.root.join("test.mrpack");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        zip.start_file(
            "modrinth.index.json",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        use std::io::Write;
        zip.write_all(br#"{"formatVersion":1,"game":"minecraft","versionId":"1","name":"Reviewed pack","files":[{"path":"mods/optional.jar","hashes":{"sha1":"abc","sha512":"def"},"env":{"client":"optional"},"downloads":["https://example.com/mod.jar"],"fileSize":123}],"dependencies":{"minecraft":"1.20.1","fabric-loader":"0.16.9"}}"#).unwrap();
        zip.finish().unwrap();
        let review = inspect_source(&path).unwrap();
        assert_eq!(review.name, "Reviewed pack");
        assert_eq!(review.game, "1.20.1");
        assert_eq!(review.optional, ["mods/optional.jar"]);
        assert!(!review.copy_supported);
        assert!(!source.root.join("mods").exists());
        std::fs::remove_dir_all(source.root).unwrap();
    }
    #[test]
    fn copy_plan_preserves_nested_worlds() {
        let dir = std::env::temp_dir().join(format!("palantir-copy-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("world/region")).unwrap();
        std::fs::write(dir.join("world/region/a.mca"), b"region").unwrap();
        let mut plan = vec![];
        collect(&dir, Path::new("saves"), &mut plan, &mut 0).unwrap();
        assert_eq!(plan[0].1, Path::new("saves/world/region/a.mca"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
