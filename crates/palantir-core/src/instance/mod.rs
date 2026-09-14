//! Instance management: discovery, metadata and folder layout.
//!
//! Mirrors `InstanceList.cpp` (discovery rules), `BaseInstance.cpp`
//! (metadata keys/defaults), `MinecraftInstance.cpp` (game-root rule and
//! folder paths) and `InstanceCreationTask` staging (create/rename).

pub mod groups;

use crate::error::{Error, Result};
use crate::settings::{defaults, Settings};
use crate::util::{ensure_dir, now_millis, unique_dir_name};
use std::path::{Path, PathBuf};

/// A discovered Prism instance bound to `instance.cfg`.
#[derive(Debug, Clone)]
pub struct Instance {
    root: PathBuf,
    settings: Settings,
}

impl Instance {
    /// Open the instance rooted at `root` (the folder that contains
    /// `instance.cfg`). Fails when the file is missing, unreadable or when
    /// `InstanceType` is present and not `OneSix` (Prism skips such folders
    /// during discovery).
    pub fn open(root: &Path) -> Result<Instance> {
        let cfg = root.join("instance.cfg");
        if !cfg.exists() {
            return Err(Error::InstanceNotFound(root.to_path_buf()));
        }
        let settings = Settings::load(&cfg)?;
        let inst_type = settings.get_str("InstanceType", "");
        if !inst_type.is_empty() && inst_type != "OneSix" {
            return Err(Error::format(cfg, format!("unsupported InstanceType '{inst_type}'")));
        }
        Ok(Instance { root: root.to_path_buf(), settings })
    }

    /// Like [`Instance::open`] but returns `None` for folders without
    /// `instance.cfg` (the discovery skip case).
    pub fn try_open(root: &Path) -> Result<Option<Instance>> {
        if !root.join("instance.cfg").exists() {
            return Ok(None);
        }
        Instance::open(root).map(Some)
    }

    /// Discover instances under `instances_dir`. Mirrors
    /// `InstanceList::discoverInstances`: only folders containing
    /// `instance.cfg` count, symlinks pointing back into the instances root
    /// are ignored, and results are sorted by folder name for determinism
    /// (Prism relies on directory order, which is alphabetical in practice).
    pub fn discover(instances_dir: &Path) -> Result<Vec<PathBuf>> {
        let mut out: Vec<PathBuf> = Vec::new();
        let root_canonical = std::fs::canonicalize(instances_dir).unwrap_or_else(|_| instances_dir.to_path_buf());
        let entries = match std::fs::read_dir(instances_dir) {
            Ok(e) => e,
            Err(e) => return Err(Error::io(instances_dir, e)),
        };
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => return Err(Error::io(instances_dir, e)),
            };
            let path = entry.path();
            let is_symlink = std::fs::symlink_metadata(&path)
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false);
            if is_symlink {
                // Ignore symlinks that lead into the configured instance root.
                let target = std::fs::read_link(&path).unwrap_or_else(|_| path.clone());
                let resolved = if target.is_absolute() {
                    target.clone()
                } else {
                    instances_dir.join(target)
                };
                if let Ok(canon) = std::fs::canonicalize(&resolved) {
                    if canon.starts_with(&root_canonical) {
                        continue;
                    }
                }
            }
            if !path.join("instance.cfg").exists() {
                continue;
            }
            out.push(path);
        }
        out.sort();
        Ok(out)
    }

    /// Create a new instance folder in `instances_dir` with the given
    /// display name. The folder name is a sanitized unique form of the name
    /// (`FS::DirNameFromString`). Writes `instance.cfg` (ConfigVersion,
    /// InstanceType=OneSix, name, iconKey, uuid) and an `mmc-pack.json`
    /// carrying the requested Minecraft version. Offline-safe.
    pub fn create(instances_dir: &Path, name: &str, mc_version: &str) -> Result<Instance> {
        ensure_dir(instances_dir)?;
        let id = unique_dir_name(instances_dir, name)?;
        let root = instances_dir.join(&id);
        ensure_dir(&root)?;
        ensure_dir(&root.join("minecraft"))?;
        ensure_dir(&root.join("jarmods"))?;
        ensure_dir(&root.join("libraries"))?;

        let mut settings = Settings::empty(root.join("instance.cfg"));
        settings.set_str("name", name);
        settings.set_str("iconKey", defaults::ICON_KEY);
        settings.set_str("InstanceType", "OneSix");
        settings.set_str("uuid", uuid::Uuid::new_v4().simple().to_string());
        settings.save()?;

        let profile = crate::pack::PackProfile::vanilla(mc_version);
        profile.save(&root.join("mmc-pack.json"))?;

        Ok(Instance { root, settings })
    }

    /// The instance root folder (contains `instance.cfg`).
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The instance id — the instance folder's file name (`BaseInstance::id`).
    pub fn id(&self) -> String {
        self.root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// Typed settings backed by `instance.cfg`.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Mutable settings; call [`Instance::save`] to persist.
    pub fn settings_mut(&mut self) -> &mut Settings {
        &mut self.settings
    }

    /// Persist `instance.cfg`.
    pub fn save(&self) -> Result<()> {
        self.settings.save()
    }

    // ---- BaseInstance metadata -------------------------------------------

    /// Display name (`name`, default `Unnamed Instance`).
    pub fn name(&self) -> String {
        self.settings.get_str("name", defaults::INSTANCE_NAME)
    }

    /// Set the display name.
    pub fn set_name(&mut self, name: &str) {
        self.settings.set_str("name", name);
    }

    /// Icon key (`iconKey`, default `default`).
    pub fn icon_key(&self) -> String {
        self.settings.get_str("iconKey", defaults::ICON_KEY)
    }

    /// Set the icon key.
    pub fn set_icon_key(&mut self, key: &str) {
        self.settings.set_str("iconKey", key);
    }

    /// Notes text (`notes`).
    pub fn notes(&self) -> String {
        self.settings.get_str("notes", "")
    }

    /// Set the notes text.
    pub fn set_notes(&mut self, notes: &str) {
        self.settings.set_str("notes", notes);
    }

    /// Last launch time in milliseconds since the epoch (`lastLaunchTime`,
    /// 0 when never launched — Prism stores msecs, not ISO strings).
    pub fn last_launch_millis(&self) -> i64 {
        self.settings.get_i64("lastLaunchTime", 0)
    }

    /// Set the last launch time (Prism stamps this when the game starts).
    pub fn set_last_launch_millis(&mut self, millis: i64) {
        self.settings.set_i64("lastLaunchTime", millis);
    }

    /// Stamp the last-launch time to now.
    pub fn mark_launched(&mut self) {
        self.set_last_launch_millis(now_millis());
    }

    /// Total play time in seconds (`totalTimePlayed`). Negative stored
    /// values read as 0 (Prism resets them on load).
    pub fn total_time_played_secs(&self) -> i64 {
        self.settings.get_i64("totalTimePlayed", 0).max(0)
    }

    /// Add play time in seconds; also updates `lastTimePlayed`.
    pub fn add_play_time_secs(&mut self, secs: i64) {
        let total = self.total_time_played_secs() + secs.max(0);
        self.settings.set_i64("totalTimePlayed", total);
        self.settings.set_i64("lastTimePlayed", secs.max(0));
    }

    /// Play time of the most recent session in seconds (`lastTimePlayed`).
    pub fn last_time_played_secs(&self) -> i64 {
        self.settings.get_i64("lastTimePlayed", 0).max(0)
    }

    /// Instance UUID in Id128 form (32 hex digits, no dashes), matching
    /// `QUuid::toString(QUuid::Id128)`.
    pub fn uuid(&self) -> String {
        self.settings.get_str("uuid", "")
    }

    /// Instance type (`InstanceType`); empty for brand-new entries, `OneSix`
    /// for every Minecraft instance Prism writes.
    pub fn instance_type(&self) -> String {
        self.settings.get_str("InstanceType", "")
    }

    /// Force the instance type (normalizes to `OneSix` like
    /// `MinecraftInstance::loadSpecificSettings` does).
    pub fn set_instance_type(&mut self, t: &str) {
        self.settings.set_str("InstanceType", if t == "OneSix" { "OneSix" } else { t });
    }

    /// Linked instance ids (`linkedInstances`, stored as a compact JSON
    /// string array, mirroring `Json::fromStringList`).
    pub fn linked_instances(&self) -> Vec<String> {
        let raw = self.settings.get_str("linkedInstances", "[]");
        match serde_json::from_str::<serde_json::Value>(&raw) {
            Ok(serde_json::Value::Array(items)) => items
                .into_iter()
                .filter_map(|v| match v {
                    serde_json::Value::String(s) => Some(s),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Set linked instance ids.
    pub fn set_linked_instances(&mut self, ids: &[String]) {
        let json = serde_json::Value::Array(ids.iter().map(|s| serde_json::Value::String(s.clone())).collect());
        self.settings.set_str("linkedInstances", json.to_string());
    }

    // ---- MinecraftInstance folder layout ---------------------------------

    /// Game directory. Prism prefers `minecraft/` and only uses
    /// `.minecraft/` when it exists and `minecraft/` does not
    /// (`MinecraftInstance::gameRoot`).
    pub fn game_root(&self) -> PathBuf {
        let mc = self.root.join("minecraft");
        let dot = self.root.join(".minecraft");
        // Prism `MinecraftInstance::gameRoot`: prefer `minecraft/`, fall back
        // to `.minecraft/` only when `minecraft/` is missing.
        if !mc.exists() && dot.exists() {
            dot
        } else {
            mc
        }
    }

    /// `game_root()/bin` (native library extraction for legacy setups).
    pub fn bin_root(&self) -> PathBuf {
        self.game_root().join("bin")
    }

    /// `game_root()/resources` — where a *legacy* asset index's files are
    /// reconstructed by logical name (`MinecraftInstance::resourcesDir`).
    ///
    /// The one place `${game_assets}` points for a `legacy`/`pre-1.6` index, so
    /// the install step and the launch line must both ask it rather than each
    /// guessing `minecraft/resources`.
    pub fn resources_dir(&self) -> PathBuf {
        self.game_root().join("resources")
    }

    /// `natives/` inside the instance root.
    pub fn natives_dir(&self) -> PathBuf {
        self.root.join("natives")
    }

    /// Instance-local `libraries/` (jar mods' custom jars, agents).
    pub fn local_libraries_dir(&self) -> PathBuf {
        self.root.join("libraries")
    }

    /// `jarmods/` inside the instance root.
    pub fn jar_mods_dir(&self) -> PathBuf {
        self.root.join("jarmods")
    }

    /// `patches/` — custom component JSON files.
    pub fn patches_dir(&self) -> PathBuf {
        self.root.join("patches")
    }

    /// `mmc-pack.json` path.
    pub fn mmc_pack_path(&self) -> PathBuf {
        self.root.join("mmc-pack.json")
    }

    /// Mods folder (`game_root()/mods`).
    pub fn mods_dir(&self) -> PathBuf {
        self.game_root().join("mods")
    }

    /// Rename the instance folder (changes the id, not the display name).
    pub fn rename(&mut self, new_name: &str) -> Result<()> {
        let parent = self
            .root
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| Error::InvalidInstanceName(self.id().to_string()))?;
        let new_id = unique_dir_name(&parent, new_name)?;
        let new_root = parent.join(&new_id);
        std::fs::rename(&self.root, &new_root).map_err(|e| Error::io(&self.root, e))?;
        self.root = new_root;
        self.settings = Settings::load(&self.root.join("instance.cfg"))?;
        Ok(())
    }

    /// Delete an instance folder by id (hard delete, like
    /// `InstanceList::deleteInstance`; trash support arrives with the GUI).
    pub fn delete(instances_dir: &Path, id: &str) -> Result<()> {
        let root = instances_dir.join(id);
        if !root.join("instance.cfg").exists() {
            return Err(Error::InstanceNotFound(root));
        }
        std::fs::remove_dir_all(&root).map_err(|e| Error::io(&root, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn create_then_open_round_trips_metadata() {
        let dir = tmp();
        let mut inst = Instance::create(dir.path(), "My Test", "1.21.1").unwrap();
        assert_eq!(inst.id(), "My Test");
        assert_eq!(inst.name(), "My Test");
        assert_eq!(inst.instance_type(), "OneSix");
        assert_eq!(inst.icon_key(), "default");
        assert_eq!(inst.uuid().len(), 32);
        assert_eq!(inst.last_launch_millis(), 0);
        assert_eq!(inst.total_time_played_secs(), 0);

        let reopened = Instance::open(&dir.path().join("My Test")).unwrap();
        assert_eq!(reopened.name(), "My Test");
        assert_eq!(reopened.uuid(), inst.uuid());

        inst.mark_launched();
        inst.add_play_time_secs(90);
        inst.save().unwrap();
        let again = Instance::open(&dir.path().join("My Test")).unwrap();
        assert!(again.last_launch_millis() > 0);
        assert_eq!(again.total_time_played_secs(), 90);
        assert_eq!(again.last_time_played_secs(), 90);
    }

    #[test]
    fn create_scaffolds_files_and_negative_playtime_reads_as_zero() {
        let dir = tmp();
        let inst = Instance::create(dir.path(), "Scaffold", "1.20.4").unwrap();
        assert!(inst.mmc_pack_path().is_file());
        assert!(inst.jar_mods_dir().is_dir());
        assert!(inst.local_libraries_dir().is_dir());
        let mut s = Settings::empty("x");
        s.set_i64("totalTimePlayed", -5);
        let mut inst2 = Instance { root: inst.root.clone(), settings: s };
        assert_eq!(inst2.total_time_played_secs(), 0);
        inst2.add_play_time_secs(-100); // clamped, no change
        assert_eq!(inst2.total_time_played_secs(), 0);
    }

    #[test]
    fn discovery_requires_instance_cfg_and_sorts() {
        let dir = tmp();
        Instance::create(dir.path(), "b", "1.20.4").unwrap();
        Instance::create(dir.path(), "a", "1.20.4").unwrap();
        std::fs::create_dir_all(dir.path().join("not_an_instance")).unwrap();
        std::fs::write(dir.path().join("file.txt"), "x").unwrap();
        let found = Instance::discover(dir.path()).unwrap();
        let ids: Vec<String> = found.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn discovery_skips_symlinks_into_root() {
        let dir = tmp();
        Instance::create(dir.path(), "real", "1.20.4").unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(dir.path().join("real"), dir.path().join("link")).unwrap();
            let found = Instance::discover(dir.path()).unwrap();
            let ids: Vec<String> =
                found.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
            assert_eq!(ids, vec!["real"]);
        }
    }

    #[test]
    fn open_rejects_missing_cfg_and_unknown_instance_type() {
        let dir = tmp();
        assert!(matches!(Instance::open(dir.path()), Err(Error::InstanceNotFound(_))));
        let bad = dir.path().join("bad");
        std::fs::create_dir_all(&bad).unwrap();
        std::fs::write(bad.join("instance.cfg"), "InstanceType=Legacy\nConfigVersion=1.3\n").unwrap();
        assert!(Instance::open(&bad).is_err());
        assert!(Instance::try_open(&dir.path().join("empty")).unwrap().is_none());
    }

    #[test]
    fn game_root_prefers_minecraft_and_falls_back_to_dot_minecraft() {
        let dir = tmp();
        let inst = Instance::create(dir.path(), "gr", "1.20.4").unwrap();
        let root = inst.root.clone();
        let probe = Instance::open(&root).unwrap();
        assert_eq!(probe.game_root(), root.join("minecraft"));
        std::fs::create_dir_all(root.join(".minecraft")).unwrap();
        let probe = Instance::open(&root).unwrap();
        // both exist -> minecraft wins
        assert_eq!(probe.game_root(), root.join("minecraft"));
        std::fs::remove_dir_all(root.join("minecraft")).unwrap();
        let probe = Instance::open(&root).unwrap();
        // only .minecraft exists -> .minecraft wins
        assert_eq!(probe.game_root(), root.join(".minecraft"));
    }

    #[test]
    fn linked_instances_json_round_trip() {
        let dir = tmp();
        let mut inst = Instance::create(dir.path(), "li", "1.20.4").unwrap();
        assert!(inst.linked_instances().is_empty());
        inst.set_linked_instances(&["other".to_string(), "second".to_string()]);
        assert_eq!(inst.linked_instances(), vec!["other", "second"]);
        assert_eq!(inst.settings.map().get("linkedInstances"), Some("[\"other\",\"second\"]"));
    }

    #[test]
    fn rename_moves_folder_keeps_display_name() {
        let dir = tmp();
        let mut inst = Instance::create(dir.path(), "Old Id", "1.20.4").unwrap();
        inst.rename("New Id").unwrap();
        assert_eq!(inst.id(), "New Id");
        assert_eq!(inst.name(), "Old Id");
        assert!(Instance::open(&dir.path().join("New Id")).is_ok());
        assert!(!dir.path().join("Old Id").exists());
    }

    #[test]
    fn delete_requires_existing_instance() {
        let dir = tmp();
        Instance::create(dir.path(), "gone", "1.20.4").unwrap();
        Instance::delete(dir.path(), "gone").unwrap();
        assert!(Instance::delete(dir.path(), "gone").is_err());
    }
}
