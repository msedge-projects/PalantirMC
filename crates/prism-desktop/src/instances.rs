//! Instance discovery, summaries and creation.
//!
//! The GUI needs more than a folder name: every card shows the game version,
//! the mod loader and its build, playtime, mod counts and the memory ceiling,
//! and the Create Instance dialog must produce an instance that really has the
//! chosen loader installed (a `patches/<uid>.json` plus the matching
//! `mmc-pack.json` component), not just a renamed folder.
//!
//! All of it is plain filesystem work on `prism-core`, so the whole module is
//! unit-tested against temp dirs — no window, no network.

use std::path::{Path, PathBuf};

use prism_core::instance::{groups::Groups, Instance};
use prism_core::pack::PackProfile;
use prism_core::paths::PrismPaths;
use prism_core::settings::defaults;
use prism_gui::InstanceEntry;

use crate::catalog::LoaderKind;
use crate::mods::list_mods;

/// Label used for instances that belong to no group.
pub const UNGROUPED_LABEL: &str = "Ungrouped";

/// Everything a card and the detail sidebar need about one instance.
#[derive(Debug, Clone, PartialEq)]
pub struct InstanceCard {
    /// Folder id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// `iconKey`.
    pub icon: String,
    /// Owning group, if any.
    pub group: Option<String>,
    /// `net.minecraft` component version (`""` when the pack file is unreadable).
    pub mc_version: String,
    /// Mod loader, `Vanilla` when there is none.
    pub loader: LoaderKind,
    /// Loader build for the loader component (`""` for vanilla).
    pub loader_version: String,
    /// Seconds played, from `totalTimePlayed`.
    pub playtime_secs: i64,
    /// Mod files present in `mods/`.
    pub mods_total: usize,
    /// Of those, the enabled ones.
    pub mods_enabled: usize,
    /// `MaxMemAlloc` from `instance.cfg`.
    pub max_mem_mb: i64,
    /// Why the instance could not be fully summarized, if anything went wrong.
    pub problem: Option<String>,
}

impl InstanceCard {
    /// `Fabric 0.19.5 · 26.2`, or just the game version for vanilla.
    pub fn subtitle(&self) -> String {
        if self.loader == LoaderKind::Vanilla || self.loader_version.is_empty() {
            self.mc_version.clone()
        } else {
            format!("{} {} · {}", self.loader.label(), self.loader_version, self.mc_version)
        }
    }

    /// Human playtime ("3h 12m", "45m", "—" when never played).
    pub fn playtime_label(&self) -> String {
        let secs = self.playtime_secs.max(0);
        if secs == 0 {
            return "Never played".to_string();
        }
        let hours = secs / 3600;
        let minutes = (secs % 3600) / 60;
        if hours >= 48 {
            format!("{}d {}h", hours / 24, hours % 24)
        } else if hours > 0 {
            format!("{hours}h {minutes}m")
        } else {
            format!("{minutes}m")
        }
    }

    /// Whether a mod loader is installed.
    pub fn has_loader(&self) -> bool {
        self.loader.loads_mods() && !self.loader_version.is_empty()
    }
}

/// Result of a full background scan: cards plus the group index and whatever
/// was pre-selected.
#[derive(Debug, Clone, Default)]
pub struct LoadedInstances {
    /// One summary per discovered instance, name-sorted.
    pub cards: Vec<InstanceCard>,
    /// Group membership + collapsed flags.
    pub groups: Groups,
    /// Pre-selected instance id (`InstanceDir` override applied).
    pub selected: Option<String>,
    /// Resolved instances directory.
    pub instances_dir: PathBuf,
    /// One-line outcome for the status strip.
    pub status: String,
}

/// Read one instance's loader, version, mods and memory ceiling.
pub fn summarize(instances_dir: &Path, entry: &InstanceEntry) -> InstanceCard {
    let mut card = InstanceCard {
        id: entry.id.clone(),
        name: entry.name.clone(),
        icon: entry.icon.clone(),
        group: entry.group.clone(),
        mc_version: String::new(),
        loader: LoaderKind::Vanilla,
        loader_version: String::new(),
        playtime_secs: entry.playtime_secs,
        mods_total: 0,
        mods_enabled: 0,
        max_mem_mb: defaults::MAX_MEM_ALLOC,
        problem: None,
    };
    let instance = match Instance::open(&instances_dir.join(&entry.id)) {
        Ok(instance) => instance,
        Err(error) => {
            card.problem = Some(format!("cannot open instance: {error}"));
            return card;
        }
    };
    card.max_mem_mb = instance.settings().get_i64("MaxMemAlloc", defaults::MAX_MEM_ALLOC);
    match PackProfile::load(&instance.mmc_pack_path()) {
        Ok(profile) => {
            for component in profile.components() {
                if component.uid == "net.minecraft" {
                    card.mc_version = component.version.clone();
                    continue;
                }
                if let Some(kind) = LoaderKind::from_uid(&component.uid) {
                    if component.is_enabled() {
                        card.loader = kind;
                        card.loader_version = component.version.clone();
                    }
                }
            }
        }
        Err(error) => card.problem = Some(format!("cannot read mmc-pack.json: {error}")),
    }
    let mods = list_mods(&instance.mods_dir());
    card.mods_total = mods.len();
    card.mods_enabled = mods.iter().filter(|entry| entry.enabled).count();
    card
}

/// Scan every instance under `paths` and summarize it. Failures degrade to an
/// empty list plus a status line, never a crash.
pub fn load(paths: &PrismPaths) -> LoadedInstances {
    let instances_dir = paths.configured_instances_dir();
    let groups = Groups::load(paths);
    let model = match prism_gui::InstanceListModel::load(paths) {
        Ok(model) => model,
        Err(error) => {
            return LoadedInstances {
                groups,
                instances_dir: instances_dir.clone(),
                status: format!("listing instances failed: {error}"),
                ..Default::default()
            };
        }
    };
    let cards: Vec<InstanceCard> =
        model.entries().iter().map(|entry| summarize(&instances_dir, entry)).collect();
    let selected = resolve_selected_id(paths, &cards);
    let status = if cards.is_empty() {
        format!("No instances yet — press N to create one (looked in {}).", instances_dir.display())
    } else {
        format!("{} instance(s) ready", cards.len())
    };
    LoadedInstances { cards, groups, selected, instances_dir, status }
}

/// Pre-selection for startup: `SelectedInstance` when it still exists.
pub fn resolve_selected_id(paths: &PrismPaths, cards: &[InstanceCard]) -> Option<String> {
    let want = paths.selected_instance_id()?;
    if want.trim().is_empty() {
        return None;
    }
    cards.iter().any(|card| card.id == want).then_some(want)
}

// ---- Creating ----------------------------------------------------------

/// What the Create Instance dialog collected.
#[derive(Debug, Clone, PartialEq)]
pub struct NewInstance {
    /// Display name (also the folder id, uniquified on collision).
    pub name: String,
    /// Chosen loader.
    pub loader: LoaderKind,
    /// Chosen game version.
    pub game: String,
    /// Chosen loader build (required unless `loader` is vanilla).
    pub loader_build: Option<String>,
    /// `iconKey` to store (`icon_source` wins when both are set).
    pub icon_key: Option<String>,
    /// A PNG on disk to install as a custom icon (drag & drop / upload).
    pub icon_source: Option<PathBuf>,
    /// `MaxMemAlloc` to set (enables the `OverrideMemory` gate when `Some`).
    pub max_mem_mb: Option<i64>,
}

impl NewInstance {
    /// A vanilla instance for `game`.
    pub fn vanilla(name: &str, game: &str) -> NewInstance {
        NewInstance {
            name: name.to_string(),
            loader: LoaderKind::Vanilla,
            game: game.to_string(),
            loader_build: None,
            icon_key: None,
            icon_source: None,
            max_mem_mb: None,
        }
    }
}

/// A created instance plus anything that did not go perfectly.
#[derive(Debug, Clone, PartialEq)]
pub struct CreatedInstance {
    /// New folder id.
    pub id: String,
    /// Non-fatal problems (the instance exists either way).
    pub warnings: Vec<String>,
}

/// Create an instance and install the requested loader for real.
///
/// Order matters: `Instance::create` writes `instance.cfg` + `mmc-pack.json`,
/// then the loader patch is written through `prism-loader` (which also
/// registers the component), then settings/icon, then a single save.
pub fn create(paths: &PrismPaths, spec: &NewInstance) -> Result<CreatedInstance, String> {
    let name = spec.name.trim();
    if name.is_empty() {
        return Err("give the instance a name".to_string());
    }
    if spec.game.trim().is_empty() {
        return Err("pick a game version".to_string());
    }
    let instances_dir = paths.configured_instances_dir();
    let mut warnings = Vec::new();
    let mut instance = Instance::create(&instances_dir, name, spec.game.trim())
        .map_err(|error| format!("creating the instance folder failed: {error}"))?;
    let id = instance.id();

    if spec.loader.loads_mods() {
        match spec.loader_build.as_deref().map(str::trim) {
            Some(build) if !build.is_empty() => {
                let uid = spec.loader.uid().unwrap_or_default();
                match prism_loader::plan_loader_install(uid, spec.game.trim(), build) {
                    Ok(patch) => {
                        if let Err(error) = prism_loader::write_patch(&instance, uid, &patch) {
                            warnings.push(format!(
                                "{} {} was not installed: {error}",
                                spec.loader.label(),
                                build
                            ));
                        }
                    }
                    Err(error) => warnings
                        .push(format!("{} {} was not installed: {error}", spec.loader.label(), build)),
                }
            }
            _ => warnings.push(format!(
                "no {} build was selected, so the instance is vanilla for now",
                spec.loader.label()
            )),
        }
    }

    if let Some(mem) = spec.max_mem_mb {
        instance.settings_mut().set_bool("OverrideMemory", true);
        instance.settings_mut().set_i64("MaxMemAlloc", mem);
    }

    if let Some(source) = spec.icon_source.as_deref() {
        match import_icon_file(paths, source, &id) {
            Ok(key) => instance.set_icon_key(&key),
            Err(error) => warnings.push(error),
        }
    } else if let Some(key) = spec.icon_key.as_deref().filter(|key| !key.is_empty()) {
        instance.set_icon_key(key);
    }

    if let Err(error) = instance.save() {
        return Err(format!("saving '{id}' failed: {error}"));
    }
    // Prism creates these on launch; making them now gives the pages
    // something to open straight away.
    for dir in [instance.mods_dir(), instance.game_root().join("saves")] {
        let _ = std::fs::create_dir_all(dir);
    }
    Ok(CreatedInstance { id, warnings })
}

// ---- Icons -------------------------------------------------------------

/// PNG magic: custom icons must really be PNGs (Prism only reads PNG icons).
const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Install a PNG as `id`'s custom icon, returning the `iconKey` to store.
///
/// Prism resolves non-builtin keys to `<icons_dir>/<key>.png`, so the file is
/// copied there under a sanitized name.
pub fn import_icon_file(paths: &PrismPaths, source: &Path, id: &str) -> Result<String, String> {
    let bytes = std::fs::read(source)
        .map_err(|error| format!("reading icon '{}' failed: {error}", source.display()))?;
    if bytes.len() < PNG_MAGIC.len() || bytes[..PNG_MAGIC.len()] != PNG_MAGIC {
        return Err(format!("'{}' is not a PNG image", source.display()));
    }
    let stem = match source.file_stem().and_then(|stem| stem.to_str()) {
        Some(stem) if !stem.is_empty() => stem,
        _ => id,
    };
    let key = prism_core::util::sanitize_dir_name(stem);
    let key = if key.is_empty() { id.to_string() } else { key };
    let icons_dir = paths.icons_dir();
    prism_core::util::ensure_dir(&icons_dir)
        .map_err(|error| format!("creating {} failed: {error}", icons_dir.display()))?;
    let dest = icons_dir.join(format!("{key}.png"));
    prism_core::util::atomic_write(&dest, &bytes)
        .map_err(|error| format!("writing {} failed: {error}", dest.display()))?;
    Ok(key)
}

/// Install an icon file onto an existing instance and persist the key.
pub fn set_instance_icon(paths: &PrismPaths, id: &str, source: &Path) -> Result<String, String> {
    let key = import_icon_file(paths, source, id)?;
    let mut instance = Instance::open(&paths.configured_instances_dir().join(id))
        .map_err(|error| format!("cannot open '{id}': {error}"))?;
    instance.set_icon_key(&key);
    instance.save().map_err(|error| format!("saving '{id}' failed: {error}"))?;
    Ok(key)
}

// ---- Importing from other launchers ------------------------------------

/// One instance found in another launcher's data directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportCandidate {
    /// Instance name (folder name).
    pub name: String,
    /// Source folder to copy.
    pub source: PathBuf,
    /// Which launcher it came from.
    pub origin: &'static str,
}

/// Instance directories of other launchers, as `(instances dir, origin)`.
///
/// Prism Launcher's on-disk format is exactly what this workspace reads, so
/// importing is a copy. (The Modrinth App keeps its profiles in its own
/// database format, which would need a converter — out of scope, and the
/// dialog does not pretend otherwise.)
pub fn candidate_roots() -> Vec<(PathBuf, &'static str)> {
    let mut roots = Vec::new();
    let mut push = |dir: PathBuf, origin: &'static str| {
        if dir.is_dir() {
            roots.push((dir, origin));
        }
    };
    if let Some(appdata) = std::env::var_os("APPDATA").map(PathBuf::from) {
        push(appdata.join("PrismLauncher").join("instances"), "Prism Launcher");
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        push(home.join(".local/share/PrismLauncher/instances"), "Prism Launcher");
        push(
            home.join("Library/Application Support/PrismLauncher/instances"),
            "Prism Launcher",
        );
    }
    roots
}

/// Scan one instances directory for importable instances.
pub fn scan_root(instances_dir: &Path, origin: &'static str) -> Vec<ImportCandidate> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(instances_dir) else { return out };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        // A real instance has an instance.cfg; skip stray folders.
        if !path.join("instance.cfg").is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        out.push(ImportCandidate { name, source: path, origin });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.source.cmp(&b.source)));
    out
}

/// Drop candidates whose id would collide with an instance we already have.
pub fn skip_existing(
    candidates: Vec<ImportCandidate>,
    existing: &[String],
) -> Vec<ImportCandidate> {
    candidates
        .into_iter()
        .filter(|candidate| !existing.iter().any(|id| id == &candidate.name))
        .collect()
}

/// Ids of the instances already in `instances_dir`.
pub fn existing_ids(instances_dir: &Path) -> Vec<String> {
    Instance::discover(instances_dir)
        .unwrap_or_default()
        .iter()
        .filter_map(|dir| dir.file_name().map(|name| name.to_string_lossy().into_owned()))
        .collect()
}

/// Every importable instance across the known launchers, excluding ids that
/// already exist in `paths`.
pub fn find_importable(paths: &PrismPaths) -> Vec<ImportCandidate> {
    let instances_dir = paths.configured_instances_dir();
    let mut found = Vec::new();
    for (root, origin) in candidate_roots() {
        // Our own folder is what `load` scans; offering it back as an import
        // would only ever collide with itself.
        if root == instances_dir {
            continue;
        }
        found.extend(scan_root(&root, origin));
    }
    skip_existing(found, &existing_ids(&instances_dir))
}

/// Copy an instance folder into `paths`, uniquifying the id on collision.
pub fn import_instance(paths: &PrismPaths, source: &Path) -> Result<String, String> {
    let instances_dir = paths.configured_instances_dir();
    let name = source
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| format!("'{}' has no folder name", source.display()))?;
    let id = prism_core::util::unique_dir_name(&instances_dir, &name)
        .map_err(|error| format!("picking a folder name failed: {error}"))?;
    let dest = instances_dir.join(&id);
    copy_dir_recursive(source, &dest)?;
    Ok(id)
}

/// Recursively copy a directory tree (files and directories; symlinks and
/// special files are skipped, which is all Prism ever writes in an instance).
pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    let entries = std::fs::read_dir(src).map_err(|e| format!("reading '{}': {e}", src.display()))?;
    std::fs::create_dir_all(dst).map_err(|e| format!("creating '{}': {e}", dst.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("listing '{}': {e}", src.display()))?;
        let kind = entry
            .file_type()
            .map_err(|e| format!("stating '{}': {e}", entry.path().display()))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if kind.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else if kind.is_file() {
            std::fs::copy(&from, &to).map_err(|e| format!("copying '{}': {e}", from.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_paths() -> (tempfile::TempDir, PrismPaths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = PrismPaths::at(dir.path());
        std::fs::create_dir_all(paths.instances_dir()).unwrap();
        (dir, paths)
    }

    fn fabric_spec(name: &str, game: &str, build: &str) -> NewInstance {
        NewInstance {
            name: name.to_string(),
            loader: LoaderKind::Fabric,
            game: game.to_string(),
            loader_build: Some(build.to_string()),
            ..NewInstance::vanilla(name, game)
        }
    }

    #[test]
    fn creating_a_fabric_instance_installs_the_loader_for_real() {
        let (_dir, paths) = test_paths();
        let created = create(&paths, &fabric_spec("Fabric Dreams", "1.21.1", "0.19.5")).unwrap();
        assert_eq!(created.id, "Fabric Dreams");
        assert!(created.warnings.is_empty(), "warnings: {:?}", created.warnings);

        let instance = Instance::open(&paths.instances_dir().join(&created.id)).unwrap();
        let patch = instance.patches_dir().join("net.fabricmc.fabric-loader.json");
        assert!(patch.is_file(), "loader patch must be written");
        let profile = PackProfile::load(&instance.mmc_pack_path()).unwrap();
        assert_eq!(profile.get("net.fabricmc.fabric-loader").unwrap().version, "0.19.5");
        assert_eq!(profile.get("net.minecraft").unwrap().version, "1.21.1");
        assert!(instance.mods_dir().is_dir(), "mods folder is created up front");

        // …and the card agrees with the disk.
        let model = prism_gui::InstanceListModel::load(&paths).unwrap();
        let card = summarize(&paths.instances_dir(), &model.entries()[0]);
        assert_eq!(card.loader, LoaderKind::Fabric);
        assert_eq!(card.loader_version, "0.19.5");
        assert_eq!(card.mc_version, "1.21.1");
        assert_eq!(card.subtitle(), "Fabric 0.19.5 · 1.21.1");
        assert!(card.has_loader());
    }

    #[test]
    fn creating_a_vanilla_instance_adds_no_loader() {
        let (_dir, paths) = test_paths();
        let created = create(&paths, &NewInstance::vanilla("Plain", "26.2")).unwrap();
        let instance = Instance::open(&paths.instances_dir().join(&created.id)).unwrap();
        let profile = PackProfile::load(&instance.mmc_pack_path()).unwrap();
        // `net.minecraft` plus the lwjgl3 component every Prism profile carries.
        assert_eq!(profile.components().len(), 2);
        assert_eq!(profile.get("net.minecraft").unwrap().version, "26.2");
        assert!(
            !profile
                .components()
                .iter()
                .any(|component| LoaderKind::from_uid(&component.uid).is_some()),
            "vanilla must not carry a loader component"
        );
        assert!(!instance.patches_dir().join("net.fabricmc.fabric-loader.json").exists());

        // …and the card agrees with the disk.
        let model = prism_gui::InstanceListModel::load(&paths).unwrap();
        let card = summarize(&paths.instances_dir(), &model.entries()[0]);
        assert_eq!(card.loader, LoaderKind::Vanilla);
        assert_eq!(card.subtitle(), "26.2");
        assert!(!card.has_loader());
    }

    #[test]
    fn loader_without_a_build_warns_but_still_creates() {
        let (_dir, paths) = test_paths();
        let spec = NewInstance {
            loader_build: None,
            ..fabric_spec("Half Loaded", "1.21.1", "0.19.5")
        };
        let created = create(&paths, &spec).unwrap();
        assert_eq!(created.warnings.len(), 1);
        assert!(created.warnings[0].contains("no Fabric build"));
        let model = prism_gui::InstanceListModel::load(&paths).unwrap();
        let card = summarize(&paths.instances_dir(), &model.entries()[0]);
        assert_eq!(card.loader, LoaderKind::Vanilla);
        assert_eq!(card.subtitle(), "1.21.1");
    }

    #[test]
    fn creation_validates_name_and_game_version() {
        let (_dir, paths) = test_paths();
        let blank = NewInstance::vanilla("   ", "26.2");
        assert!(create(&paths, &blank).unwrap_err().contains("name"));
        let no_game = NewInstance::vanilla("Fine", " ");
        assert!(create(&paths, &no_game).unwrap_err().contains("game version"));
    }

    #[test]
    fn duplicate_names_get_unique_folders() {
        let (_dir, paths) = test_paths();
        let first = create(&paths, &NewInstance::vanilla("Same", "26.2")).unwrap();
        let second = create(&paths, &NewInstance::vanilla("Same", "26.2")).unwrap();
        assert_eq!(first.id, "Same");
        assert_ne!(second.id, first.id);
        assert!(paths.instances_dir().join(&second.id).is_dir());
    }

    #[test]
    fn custom_icons_are_copied_into_the_icons_dir() {
        let (dir, paths) = test_paths();
        let png = dir.path().join("my pack.png");
        std::fs::write(&png, [&PNG_MAGIC[..], b"pretend pixels"].concat()).unwrap();
        let created = create(
            &paths,
            &NewInstance { icon_source: Some(png.clone()), ..NewInstance::vanilla("Iconed", "26.2") },
        )
        .unwrap();
        assert!(created.warnings.is_empty(), "warnings: {:?}", created.warnings);
        let icon = paths.icons_dir().join("my pack.png");
        assert!(icon.is_file());
        let instance = Instance::open(&paths.instances_dir().join(&created.id)).unwrap();
        assert_eq!(instance.icon_key(), "my pack");

        // Non-PNG uploads are refused with a reason.
        let bogus = dir.path().join("bogus.png");
        std::fs::write(&bogus, b"not an image").unwrap();
        let err = set_instance_icon(&paths, &created.id, &bogus).unwrap_err();
        assert!(err.contains("not a PNG"), "error: {err}");
        assert!(import_icon_file(&paths, &dir.path().join("missing.png"), "x").is_err());
    }

    #[test]
    fn memory_override_is_written_when_requested() {
        let (_dir, paths) = test_paths();
        let spec = NewInstance { max_mem_mb: Some(6144), ..NewInstance::vanilla("Memory", "26.2") };
        let created = create(&paths, &spec).unwrap();
        let instance = Instance::open(&paths.instances_dir().join(&created.id)).unwrap();
        assert_eq!(instance.settings().get_i64("MaxMemAlloc", 0), 6144);
        assert!(instance.settings().get_bool("OverrideMemory", false));
        let model = prism_gui::InstanceListModel::load(&paths).unwrap();
        assert_eq!(summarize(&paths.instances_dir(), &model.entries()[0]).max_mem_mb, 6144);
    }

    #[test]
    fn load_reports_cards_and_preselection() {
        let (_dir, paths) = test_paths();
        create(&paths, &fabric_spec("Beta", "1.21.1", "0.19.5")).unwrap();
        create(&paths, &NewInstance::vanilla("alpha", "26.2")).unwrap();
        std::fs::write(paths.global_config(), "[General]\nConfigVersion=1.3\nSelectedInstance=alpha\n")
            .unwrap();

        let loaded = load(&paths);
        let names: Vec<&str> = loaded.cards.iter().map(|card| card.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "Beta"]);
        assert_eq!(loaded.selected.as_deref(), Some("alpha"));
        assert!(loaded.status.contains("2 instance"));
        assert!(loaded.cards[0].problem.is_none());

        // A stale pre-selection is ignored.
        std::fs::write(paths.global_config(), "[General]\nConfigVersion=1.3\nSelectedInstance=Gone\n")
            .unwrap();
        assert!(load(&paths).selected.is_none());
    }

    #[test]
    fn playtime_labels_are_human() {
        let mut card = InstanceCard {
            id: "x".into(),
            name: "x".into(),
            icon: "default".into(),
            group: None,
            mc_version: "26.2".into(),
            loader: LoaderKind::Vanilla,
            loader_version: String::new(),
            playtime_secs: 0,
            mods_total: 0,
            mods_enabled: 0,
            max_mem_mb: 4096,
            problem: None,
        };
        assert_eq!(card.playtime_label(), "Never played");
        card.playtime_secs = 90;
        assert_eq!(card.playtime_label(), "1m");
        card.playtime_secs = 3 * 3600 + 12 * 60;
        assert_eq!(card.playtime_label(), "3h 12m");
        card.playtime_secs = 50 * 3600;
        assert_eq!(card.playtime_label(), "2d 2h");
    }

    #[test]
    fn scan_root_only_accepts_real_instances() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("Real")).unwrap();
        std::fs::write(dir.path().join("Real").join("instance.cfg"), b"[General]\n").unwrap();
        std::fs::create_dir_all(dir.path().join("NotAnInstance")).unwrap();
        std::fs::write(dir.path().join("loose.txt"), b"x").unwrap();

        let found = scan_root(dir.path(), "Prism Launcher");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Real");
        assert_eq!(found[0].origin, "Prism Launcher");
        assert!(scan_root(&dir.path().join("missing"), "Prism Launcher").is_empty());
    }

    #[test]
    fn importing_copies_the_tree_and_uniquifies_the_id() {
        let (dir, paths) = test_paths();
        let source = dir.path().join("Borrowed");
        std::fs::create_dir_all(source.join("mods")).unwrap();
        std::fs::write(source.join("instance.cfg"), b"[General]\nname=Borrowed\n").unwrap();
        std::fs::write(source.join("mods").join("sodium.jar"), b"jar").unwrap();

        let id = import_instance(&paths, &source).unwrap();
        assert_eq!(id, "Borrowed");
        assert!(paths.instances_dir().join("Borrowed").join("mods").join("sodium.jar").is_file());
        let again = import_instance(&paths, &source).unwrap();
        assert_ne!(again, id);
        assert!(import_instance(&paths, &dir.path().join("nope")).is_err());
    }

    #[test]
    fn copy_dir_recursive_copies_trees() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("a.txt"), b"a").unwrap();
        std::fs::write(src.join("sub").join("b.txt"), b"b").unwrap();
        let dst = dir.path().join("dst");
        copy_dir_recursive(&src, &dst).unwrap();
        assert_eq!(std::fs::read(dst.join("a.txt")).unwrap(), b"a");
        assert_eq!(std::fs::read(dst.join("sub").join("b.txt")).unwrap(), b"b");
        assert!(copy_dir_recursive(&dir.path().join("missing"), &dst).is_err());
    }

    #[test]
    fn find_importable_skips_ids_that_already_exist() {
        let (dir, paths) = test_paths();
        // A foreign launcher root holding one real instance and one stray
        // folder (no `instance.cfg`).
        let root = dir.path().join("BorrowedLauncher");
        let borrowed = root.join("Borrowed");
        std::fs::create_dir_all(&borrowed).unwrap();
        std::fs::write(borrowed.join("instance.cfg"), b"[General]\n").unwrap();
        std::fs::create_dir_all(root.join("stray-folder")).unwrap();

        let found = scan_root(&root, "Prism Launcher");
        assert_eq!(found.len(), 1, "a real instance is a folder with instance.cfg");
        assert_eq!(found[0].name, "Borrowed");
        assert_eq!(found[0].origin, "Prism Launcher");
        assert_eq!(scan_root(&root.join("missing"), "Prism Launcher"), Vec::new());

        let kept = skip_existing(found.clone(), &["SomethingElse".to_string()]);
        assert_eq!(kept.len(), 1);
        let skipped = skip_existing(found, &["Borrowed".to_string()]);
        assert!(skipped.is_empty(), "an id we already have is not offered again");

        // The machine-wide scan is a thin wrapper over the two above.
        let _ = find_importable(&paths);
    }
}
