//! GUI view-model: the instance list and override-gated settings.
//!
//! No windowing code lives here, so the whole model stays testable offline;
//! `palantir-desktop`'s shell is the only thing that paints it.

use palantir_core::{
    instance::{groups::Groups, Instance},
    paths::PalantirPaths,
    settings::{defaults, Settings},
};

/// One row of the instance list view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceEntry {
    /// Folder id (`BaseInstance::id`).
    pub id: String,
    /// Display name (`name`).
    pub name: String,
    /// Icon key (`iconKey`).
    pub icon: String,
    /// Owning group from `instgroups.json`, if any.
    pub group: Option<String>,
    /// Total play time in seconds (`totalTimePlayed`).
    pub playtime_secs: i64,
}

/// Sorted, filterable view over the discovered instances.
#[derive(Debug, Default, Clone)]
pub struct InstanceListModel {
    entries: Vec<InstanceEntry>,
}

impl InstanceListModel {
    /// Load every instance under `paths.instances_dir()` via
    /// [`Instance::discover`], resolving group membership via
    /// [`Groups::load`]. Entries are sorted by name (see
    /// [`InstanceListModel::sort_by_name`]).
    ///
    /// Folders that fail to open (for example an unsupported
    /// `InstanceType`) are skipped, mirroring Prism's discovery which
    /// silently ignores such folders.
    pub fn load(paths: &PalantirPaths) -> Result<Self, crate::Error> {
        let groups = Groups::load(paths);
        let mut entries = Vec::new();
        for root in Instance::discover(&paths.instances_dir())? {
            let instance = match Instance::open(&root) {
                Ok(instance) => instance,
                Err(_) => continue,
            };
            let id = instance.id();
            entries.push(InstanceEntry {
                group: groups.group_of(&id).map(str::to_string),
                name: instance.name(),
                icon: instance.icon_key(),
                playtime_secs: instance.total_time_played_secs(),
                id,
            });
        }
        let mut model = InstanceListModel { entries };
        model.sort_by_name();
        Ok(model)
    }

    /// Sort entries by display name (case-insensitive), breaking ties by id
    /// so the order is deterministic.
    pub fn sort_by_name(&mut self) {
        self.entries.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.id.cmp(&b.id))
        });
    }

    /// All entries in display order.
    pub fn entries(&self) -> &[InstanceEntry] {
        &self.entries
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the list is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Entries whose name or id contains `query` (case-insensitive
    /// substring). An empty query matches everything.
    pub fn filter(&self, query: &str) -> Vec<&InstanceEntry> {
        let needle = query.to_lowercase();
        if needle.is_empty() {
            return self.entries.iter().collect();
        }
        self.entries
            .iter()
            .filter(|e| {
                e.name.to_lowercase().contains(&needle) || e.id.to_lowercase().contains(&needle)
            })
            .collect()
    }

    /// Group of an instance id, if it belongs to one.
    pub fn group_of(&self, id: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| e.group.as_deref())
    }
}

/// Settings view with Prism's override-gate semantics.
///
/// Prism keeps one global settings file (`prismlauncher.cfg`) plus optional
/// per-instance overrides. An instance value only takes effect while its
/// override flag (for example `OverrideMemory` for the memory settings) is
/// enabled on the instance; otherwise the global value wins. Keys without a
/// gate always read the global value.
#[derive(Debug, Clone)]
pub struct SettingsModel {
    global: Settings,
    instance: Option<Settings>,
}

impl SettingsModel {
    /// Model bound to the global settings only.
    pub fn new(global: Settings) -> Self {
        SettingsModel { global, instance: None }
    }

    /// Model with both global settings and per-instance overrides.
    pub fn with_instance(global: Settings, instance: Settings) -> Self {
        SettingsModel { global, instance: Some(instance) }
    }

    /// The global settings.
    pub fn global(&self) -> &Settings {
        &self.global
    }

    /// Mutable access to the global settings.
    pub fn global_mut(&mut self) -> &mut Settings {
        &mut self.global
    }

    /// The bound instance settings, if any.
    pub fn instance(&self) -> Option<&Settings> {
        self.instance.as_ref()
    }

    /// Whether the override gate `override_key` is enabled on the bound
    /// instance. Always `false` when no instance is bound.
    pub fn is_overridden(&self, override_key: &str) -> bool {
        self.instance
            .as_ref()
            .map(|s| s.get_bool(override_key, false))
            .unwrap_or(false)
    }

    /// Enable or disable the override gate `override_key` on the bound
    /// instance. Fails when no instance is bound.
    pub fn set_override(&mut self, override_key: &str, enabled: bool) -> Result<(), crate::Error> {
        match self.instance.as_mut() {
            Some(settings) => {
                settings.set_bool(override_key, enabled);
                Ok(())
            }
            None => Err(crate::Error::NoInstanceBound),
        }
    }

    /// Effective string value: the instance value while `override_key`
    /// gates it on, otherwise the global value. A missing instance key
    /// inherits the global value. `None` as gate always reads global.
    pub fn get_str(&self, key: &str, override_key: Option<&str>, default: &str) -> String {
        let global = self.global.get_str(key, default);
        match (override_key, self.instance.as_ref()) {
            (Some(gate), Some(instance)) if instance.get_bool(gate, false) => {
                instance.get_str(key, &global)
            }
            _ => global,
        }
    }

    /// Effective boolean value (see [`SettingsModel::get_str`]).
    pub fn get_bool(&self, key: &str, override_key: Option<&str>, default: bool) -> bool {
        let global = self.global.get_bool(key, default);
        match (override_key, self.instance.as_ref()) {
            (Some(gate), Some(instance)) if instance.get_bool(gate, false) => {
                instance.get_bool(key, global)
            }
            _ => global,
        }
    }

    /// Effective integer value (see [`SettingsModel::get_str`]).
    pub fn get_i64(&self, key: &str, override_key: Option<&str>, default: i64) -> i64 {
        let global = self.global.get_i64(key, default);
        match (override_key, self.instance.as_ref()) {
            (Some(gate), Some(instance)) if instance.get_bool(gate, false) => {
                instance.get_i64(key, global)
            }
            _ => global,
        }
    }

    /// Set a global value.
    pub fn set_global_str(&mut self, key: &str, value: impl Into<String>) {
        self.global.set_str(key, value);
    }

    /// Set a global boolean value.
    pub fn set_global_bool(&mut self, key: &str, value: bool) {
        self.global.set_bool(key, value);
    }

    /// Set a global integer value.
    pub fn set_global_i64(&mut self, key: &str, value: i64) {
        self.global.set_i64(key, value);
    }

    /// Set an instance value. Fails when no instance is bound. Note this
    /// only takes effect while the corresponding override gate is enabled
    /// (see [`SettingsModel::set_override`]).
    pub fn set_instance_str(&mut self, key: &str, value: impl Into<String>) -> Result<(), crate::Error> {
        match self.instance.as_mut() {
            Some(settings) => {
                settings.set_str(key, value);
                Ok(())
            }
            None => Err(crate::Error::NoInstanceBound),
        }
    }

    /// Set an instance boolean value (see [`SettingsModel::set_instance_str`]).
    pub fn set_instance_bool(&mut self, key: &str, value: bool) -> Result<(), crate::Error> {
        match self.instance.as_mut() {
            Some(settings) => {
                settings.set_bool(key, value);
                Ok(())
            }
            None => Err(crate::Error::NoInstanceBound),
        }
    }

    /// Set an instance integer value (see [`SettingsModel::set_instance_str`]).
    pub fn set_instance_i64(&mut self, key: &str, value: i64) -> Result<(), crate::Error> {
        match self.instance.as_mut() {
            Some(settings) => {
                settings.set_i64(key, value);
                Ok(())
            }
            None => Err(crate::Error::NoInstanceBound),
        }
    }

    /// Effective `(MinMemAlloc, MaxMemAlloc)` pair behind the
    /// `OverrideMemory` gate.
    pub fn effective_memory(&self) -> (i64, i64) {
        const GATE: Option<&str> = Some("OverrideMemory");
        (
            self.get_i64("MinMemAlloc", GATE, defaults::MIN_MEM_ALLOC),
            self.get_i64("MaxMemAlloc", GATE, defaults::MAX_MEM_ALLOC),
        )
    }

    /// Persist the global settings and, when bound, the instance settings.
    pub fn save(&self) -> Result<(), crate::Error> {
        self.global.save()?;
        if let Some(instance) = self.instance.as_ref() {
            instance.save()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use palantir_core::instance::groups::Groups;

    fn test_paths() -> (tempfile::TempDir, PalantirPaths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = PalantirPaths::at(dir.path());
        std::fs::create_dir_all(paths.instances_dir()).unwrap();
        (dir, paths)
    }

    fn create_named(instances: &std::path::Path, name: &str, icon: &str, playtime: i64) {
        let mut inst = Instance::create(instances, name, "1.21.1").unwrap();
        inst.set_icon_key(icon);
        inst.add_play_time_secs(playtime);
        inst.save().unwrap();
    }

    #[test]
    fn load_sorts_by_name_and_maps_fields() {
        let (_dir, paths) = test_paths();
        create_named(&paths.instances_dir(), "Zulu", "grass", 60);
        create_named(&paths.instances_dir(), "alpha", "dirt", 0);
        create_named(&paths.instances_dir(), "Mike", "stone", 3600);

        let model = InstanceListModel::load(&paths).unwrap();
        let names: Vec<&str> = model.entries().iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "Mike", "Zulu"]);

        let zulu = model.entries().iter().find(|e| e.id == "Zulu").unwrap();
        assert_eq!(zulu.icon, "grass");
        assert_eq!(zulu.playtime_secs, 60);
        assert_eq!(zulu.group, None);
    }

    #[test]
    fn load_resolves_groups_and_skips_junk() {
        let (_dir, paths) = test_paths();
        create_named(&paths.instances_dir(), "grouped", "default", 0);
        create_named(&paths.instances_dir(), "solo", "default", 0);
        std::fs::create_dir_all(paths.instances_dir().join("not-an-instance")).unwrap();

        let mut groups = Groups::default();
        groups.set_group("grouped", Some("Packs"));
        groups.save(&paths).unwrap();

        let model = InstanceListModel::load(&paths).unwrap();
        assert_eq!(model.len(), 2);
        assert_eq!(model.group_of("grouped"), Some("Packs"));
        assert_eq!(model.group_of("solo"), None);
        assert_eq!(model.group_of("missing"), None);
    }

    #[test]
    fn load_missing_dir_errors() {
        let dir = tempfile::tempdir().unwrap();
        let paths = PalantirPaths::at(dir.path().join("no-such-root"));
        assert!(InstanceListModel::load(&paths).is_err());
    }

    #[test]
    fn filter_matches_name_or_id_case_insensitively() {
        let (_dir, paths) = test_paths();
        create_named(&paths.instances_dir(), "Fabric Dreams", "default", 0);
        create_named(&paths.instances_dir(), "Vanilla", "default", 0);

        let model = InstanceListModel::load(&paths).unwrap();
        assert_eq!(model.filter("").len(), 2);
        assert_eq!(model.filter("fabric").len(), 1);
        assert_eq!(model.filter("DREAMS").len(), 1);
        assert_eq!(model.filter("vanilla").len(), 1);
        assert_eq!(model.filter("zzz").len(), 0);
        // id match (folder name == display name here)
        assert_eq!(model.filter("Fabric Dreams").len(), 1);
    }

    #[test]
    fn settings_override_gate_selects_global_vs_instance() {
        let dir = tempfile::tempdir().unwrap();
        let mut global = Settings::empty(dir.path().join("prismlauncher.cfg"));
        global.set_i64("MaxMemAlloc", 4096);
        global.set_str("JavaPath", "/usr/bin/java");
        let mut instance = Settings::empty(dir.path().join("instance.cfg"));
        instance.set_i64("MaxMemAlloc", 8192);
        instance.set_str("JavaPath", "/opt/java/bin/java");

        let mut model = SettingsModel::with_instance(global, instance);
        // Gate off: global wins even though instance values exist.
        assert_eq!(model.get_i64("MaxMemAlloc", Some("OverrideMemory"), 0), 4096);
        assert!(!model.is_overridden("OverrideMemory"));

        model.set_override("OverrideMemory", true).unwrap();
        assert!(model.is_overridden("OverrideMemory"));
        assert_eq!(model.get_i64("MaxMemAlloc", Some("OverrideMemory"), 0), 8192);
        assert_eq!(model.effective_memory(), (defaults::MIN_MEM_ALLOC, 8192));

        // Ungated keys always read global.
        assert_eq!(model.get_str("JavaPath", None, ""), "/usr/bin/java");
    }

    #[test]
    fn settings_without_instance_reads_global_and_rejects_overrides() {
        let dir = tempfile::tempdir().unwrap();
        let mut global = Settings::empty(dir.path().join("prismlauncher.cfg"));
        global.set_i64("MaxMemAlloc", 2048);
        let mut model = SettingsModel::new(global);

        assert_eq!(model.get_i64("MaxMemAlloc", Some("OverrideMemory"), 0), 2048);
        assert!(!model.is_overridden("OverrideMemory"));
        assert!(model.set_override("OverrideMemory", true).is_err());
        assert!(model.set_instance_i64("MaxMemAlloc", 1).is_err());
        assert!(model.set_instance_str("k", "v").is_err());
        assert!(model.set_instance_bool("k", true).is_err());

        model.set_global_i64("MaxMemAlloc", 3072);
        model.set_global_str("Name", "n");
        model.set_global_bool("B", true);
        assert_eq!(model.get_i64("MaxMemAlloc", Some("OverrideMemory"), 0), 3072);
        model.save().unwrap();
        let back = Settings::load(model.global().path()).unwrap();
        assert_eq!(back.get_i64("MaxMemAlloc", 0), 3072);
    }
}
