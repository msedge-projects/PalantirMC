//! The view-model the shell reads: the instance list, and override-gated settings.
//!
//! No windowing code lives here, so the whole model stays testable offline; the
//! shell is the only thing that paints it. It was `palantir-gui` -- a crate whose
//! only dependant was this one and whose reason to exist was a CLI that is gone --
//! and it moved in whole rather than being reshaped on the way: what it models is
//! Prism-shaped because `palantir_core::settings` is, and both go together when
//! the flattening importer replaces the last of them.
//!
//! Two of its readers are not the old shell, and they are why it could not simply
//! be deleted with it: `instances.rs` loads [`InstanceListModel`] to build the
//! cards the library draws, and `launch.rs` resolves a run through
//! [`SettingsModel`]'s override gates.

use palantir_core::{
    instance::{groups::Groups, Instance},
    paths::PalantirPaths,
    settings::{defaults, Settings},
};

/// What a model operation could not do.
///
/// Hand-written rather than derived: this module is the only place in the crate
/// that raises one, the trait is three methods, and a `thiserror` dependency
/// added for one `Display` arm is a dependency the whole crate carries from then
/// on.
///
/// It was an enum of two, and the second arm -- *an instance-bound operation was
/// requested without a bound instance* -- went with the write side above: nothing
/// that runs could raise it any more, and an error variant nothing constructs is
/// a sentence kept for a reader who will never see it.
#[derive(Debug)]
pub struct Error(pub palantir_core::Error);

impl From<palantir_core::Error> for Error {
    fn from(error: palantir_core::Error) -> Error {
        Error(error)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The old crate's own words, kept: they are what the reader above shows a
        // user, and a message changed in a move is a message nobody chose.
        write!(formatter, "core error: {}", self.0)
    }
}

impl std::error::Error for Error {}


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
    pub fn load(paths: &PalantirPaths) -> Result<Self, Error> {
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

}

// Four methods went with the move -- `len`, `is_empty`, `filter` and `group_of`
// -- and they went for a reason that is not "nothing calls them yet": the
// library's own page supersedes them. `pages/home.rs` holds one list, filters it
// in place from its search field and sorts it by its own control, which is what
// the reference's `Library.vue` does; a second filter in a model the page never
// reads would be a second answer to the same question, and the answer the user
// sees would be the page's. What is left is what the two readers need: the list,
// in order, for `instances.rs` to summarize.

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
    /// Model with both global settings and per-instance overrides.
    pub fn with_instance(global: Settings, instance: Settings) -> Self {
        SettingsModel { global, instance: Some(instance) }
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

    /// Effective `(MinMemAlloc, MaxMemAlloc)` pair behind the
    /// `OverrideMemory` gate.
    pub fn effective_memory(&self) -> (i64, i64) {
        const GATE: Option<&str> = Some("OverrideMemory");
        (
            self.get_i64("MinMemAlloc", GATE, defaults::MIN_MEM_ALLOC),
            self.get_i64("MaxMemAlloc", GATE, defaults::MAX_MEM_ALLOC),
        )
    }

}

// The write side went with the move: `new`, `global`, `global_mut`, `instance`,
// `is_overridden`, `set_override`, the three `set_global_*`, the three
// `set_instance_*` and `save`. What that leaves is a *reader* -- read the value
// in force, with the instance's own override gate deciding whether the instance
// or the launcher's file wins -- which is exactly what `launch.rs` does with it.
// The write side belongs to the instance-settings page stage 3 still owes, and it
// is four lines through `palantir_core::settings::Settings` when that page
// arrives; what it is not is a hundred and twenty lines of API kept here in the
// meantime for a page that has not been written. `preserve the old shell's
// model` was the move's whole job, and the compiler's dead-code pass is what says
// where that job ends.

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
        let entries = model.entries();
        assert_eq!(entries.len(), 2, "the folder that is not an instance is not an entry");
        assert_eq!(entries[0].name, "grouped");
        assert_eq!(entries[0].group.as_deref(), Some("Packs"));
        assert_eq!(entries[1].group, None);
    }

    #[test]
    fn load_missing_dir_errors() {
        let dir = tempfile::tempdir().unwrap();
        let paths = PalantirPaths::at(dir.path().join("no-such-root"));
        assert!(InstanceListModel::load(&paths).is_err());
    }

    #[test]
    fn the_override_gate_selects_the_global_value_or_the_instance_s_own() {
        // The gate is the whole of this type's behaviour, and it is what
        // `launch.rs` resolves a run through: the instance's value counts only
        // while the instance's own flag is set, and a key without a gate always
        // reads the launcher's file. Both directions are asserted, because a
        // model that ignored the flag and always read the instance would pass the
        // first half of this test on its own.
        let dir = tempfile::tempdir().unwrap();
        let mut global = Settings::empty(dir.path().join("prismlauncher.cfg"));
        global.set_i64("MaxMemAlloc", 4096);
        global.set_str("JavaPath", "/usr/bin/java");
        let mut instance = Settings::empty(dir.path().join("instance.cfg"));
        instance.set_i64("MaxMemAlloc", 8192);
        instance.set_str("JavaPath", "/opt/java/bin/java");

        let model = SettingsModel::with_instance(global, instance);
        // Gate off: the launcher's own value wins even though the instance has
        // one of its own.
        assert_eq!(model.get_i64("MaxMemAlloc", Some("OverrideMemory"), 0), 4096);
        assert_eq!(model.effective_memory(), (defaults::MIN_MEM_ALLOC, 4096));
        // Gate on: the instance's value, which is what the instance settings page
        // writes when it turns *Override memory* on.
        let mut instance = Settings::empty(dir.path().join("instance.cfg"));
        instance.set_bool("OverrideMemory", true);
        instance.set_i64("MaxMemAlloc", 8192);
        let gated = SettingsModel::with_instance(
            Settings::empty(dir.path().join("prismlauncher.cfg")),
            instance,
        );
        assert_eq!(gated.get_i64("MaxMemAlloc", Some("OverrideMemory"), 0), 8192);
        assert_eq!(gated.effective_memory(), (defaults::MIN_MEM_ALLOC, 8192));
        // An ungated key reads the launcher's file whatever the instance holds.
        assert_eq!(model.get_str("JavaPath", None, ""), "/usr/bin/java");
    }
}
