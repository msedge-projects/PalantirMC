//! Typed settings access over an INI file.
//!
//! Mirrors `launcher/settings/SettingsObject.cpp` semantics: values live in
//! the file as strings (QSettings/QVariant style) with typed conversion on
//! read. Defaults below follow the registration calls in
//! `BaseInstance.cpp` / `MinecraftInstance.cpp::loadSpecificSettings` and
//! Prism's global `Application.cpp` defaults.

use crate::error::Result;
use crate::ini::{self, IniMap};
use std::path::{Path, PathBuf};

/// Prism defaults for settings registered with explicit values.
pub mod defaults {
    /// Default minimum memory allocation in MiB (`MinMemAlloc`).
    pub const MIN_MEM_ALLOC: i64 = 128;
    /// Default maximum memory allocation in MiB (`MaxMemAlloc`).
    pub const MAX_MEM_ALLOC: i64 = 4096;
    /// Default permanent-generation size in MiB (`PermGen`); the launcher
    /// only emits `-XX:PermSize` when this differs from 64.
    pub const PERM_GEN: i64 = 64;
    /// Default Minecraft window width (`MinecraftWinWidth`).
    pub const MC_WIN_WIDTH: i64 = 854;
    /// Default Minecraft window height (`MinecraftWinHeight`).
    pub const MC_WIN_HEIGHT: i64 = 480;
    /// Default console log line cap (`ConsoleMaxLines`).
    pub const CONSOLE_MAX_LINES: i64 = 100_000;
    /// Default instance name (`name`).
    pub const INSTANCE_NAME: &str = "Unnamed Instance";
    /// Default icon key (`iconKey`).
    pub const ICON_KEY: &str = "default";
}

/// A settings file backed by an [`IniMap`].
#[derive(Debug, Clone)]
pub struct Settings {
    path: PathBuf,
    map: IniMap,
}

impl Settings {
    /// Load a settings file (`instance.cfg`, `prismlauncher.cfg`, ...).
    pub fn load(path: &Path) -> Result<Settings> {
        let map = ini::load_ini_file(path)?;
        Ok(Settings { path: path.to_path_buf(), map })
    }

    /// Create an empty settings object bound to a path.
    pub fn empty(path: impl Into<PathBuf>) -> Settings {
        Settings { path: path.into(), map: IniMap::new() }
    }

    /// Create settings from an already-loaded map.
    pub fn from_map(path: impl Into<PathBuf>, map: IniMap) -> Settings {
        Settings { path: path.into(), map }
    }

    /// The file this settings object is bound to.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Read-only access to the raw map.
    pub fn map(&self) -> &IniMap {
        &self.map
    }

    /// Mutable access to the raw map.
    pub fn map_mut(&mut self) -> &mut IniMap {
        &mut self.map
    }

    /// String value with default (QVariant `toString`).
    pub fn get_str(&self, key: &str, default: &str) -> String {
        self.map.get(key).unwrap_or(default).to_string()
    }

    /// Boolean value with default. Follows QVariant `toBool`: `true`/`false`
    /// literals, `1`/`0`, and non-zero numbers.
    pub fn get_bool(&self, key: &str, default: bool) -> bool {
        match self.map.get(key) {
            None => default,
            Some(v) => to_bool(v),
        }
    }

    /// Integer value with default. Non-numeric strings read as 0, matching
    /// QVariant `toLongLong`.
    pub fn get_i64(&self, key: &str, default: i64) -> i64 {
        match self.map.get(key) {
            None => default,
            Some(v) => v.trim().parse::<i64>().unwrap_or(0),
        }
    }

    /// Set a string value.
    pub fn set_str(&mut self, key: &str, value: impl Into<String>) {
        self.map.set(key, value);
    }

    /// Set a boolean value (`true`/`false` spelling, like QSettings).
    pub fn set_bool(&mut self, key: &str, value: bool) {
        self.map.set(key, if value { "true" } else { "false" });
    }

    /// Set an integer value (decimal, like QSettings).
    pub fn set_i64(&mut self, key: &str, value: i64) {
        self.map.set(key, value.to_string());
    }

    /// Remove a key. Returns true when it existed.
    pub fn remove(&mut self, key: &str) -> bool {
        self.map.remove(key)
    }

    /// Persist to the bound path atomically.
    pub fn save(&self) -> Result<()> {
        ini::save_ini_file(&self.path, &self.map)
    }
}

/// QVariant `toBool` semantics for a string value.
fn to_bool(v: &str) -> bool {
    let t = v.trim();
    if t.eq_ignore_ascii_case("true") {
        return true;
    }
    if t.eq_ignore_ascii_case("false") {
        return false;
    }
    match t.parse::<i64>() {
        Ok(n) => n != 0,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_save_round_trip_preserves_values() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("instance.cfg");
        let mut s = Settings::empty(&path);
        s.set_str("name", "Fabric 1.21.1");
        s.set_bool("OverrideMemory", true);
        s.set_i64("MaxMemAlloc", 8192);
        s.set_str("notes", "line1\nline2; ok");
        s.save().unwrap();

        let back = Settings::load(&path).unwrap();
        assert_eq!(back.get_str("name", ""), "Fabric 1.21.1");
        assert!(back.get_bool("OverrideMemory", false));
        assert_eq!(back.get_i64("MaxMemAlloc", 0), 8192);
        assert_eq!(back.get_str("notes", ""), "line1\nline2; ok");
        // ConfigVersion injected by the writer
        assert_eq!(back.map().get("ConfigVersion"), Some("1.3"));
    }

    #[test]
    fn defaults_are_used_for_missing_keys() {
        let s = Settings::empty("x.cfg");
        assert_eq!(s.get_str("name", defaults::INSTANCE_NAME), "Unnamed Instance");
        assert_eq!(s.get_str("iconKey", defaults::ICON_KEY), "default");
        assert!(!s.get_bool("JoinServerOnLaunch", false));
        assert_eq!(s.get_i64("MinMemAlloc", defaults::MIN_MEM_ALLOC), 128);
        assert_eq!(s.get_i64("MaxMemAlloc", defaults::MAX_MEM_ALLOC), 4096);
        assert_eq!(s.get_i64("PermGen", defaults::PERM_GEN), 64);
        assert_eq!(s.get_i64("MinecraftWinWidth", defaults::MC_WIN_WIDTH), 854);
    }

    #[test]
    fn qvariant_style_coercion_matches_qt() {
        let mut s = Settings::empty("x.cfg");
        s.set_str("a", "true");
        s.set_str("b", "TRUE");
        s.set_str("c", "2");
        s.set_str("d", "0");
        s.set_str("e", "junk");
        s.set_str("f", "  7  ");
        s.set_bool("g", false);
        s.set_i64("h", -12);
        assert!(s.get_bool("a", false));
        assert!(s.get_bool("b", false));
        assert!(s.get_bool("c", false)); // non-zero number
        assert!(!s.get_bool("d", false));
        assert!(!s.get_bool("e", false));
        assert!(s.get_bool("f", false));
        assert!(!s.get_bool("g", true));
        assert_eq!(s.get_i64("h", 0), -12);
        assert_eq!(s.get_i64("e", 0), 0); // junk -> 0
    }

    #[test]
    fn remove_reports_presence() {
        let mut s = Settings::empty("x.cfg");
        s.set_str("k", "v");
        assert!(s.remove("K")); // case-insensitive
        assert!(!s.remove("k"));
    }
}
