//! Data-root discovery and the standard Prism folder layout.
//!
//! Mirrors the layout produced by Prism's `Application.cpp` and
//! `MinecraftInstance.cpp`: `instances/`, `libraries/`, `versions/`,
//! `assets/`, `icons/`, `cache/` and the files `prismlauncher.cfg`,
//! `accounts.json`. `instgroups.json` lives at the data root (Prism uses
//! `QDir::current().filePath("instgroups.json")`, whose CWD is the launcher
//! directory), with a legacy copy inside the instances dir handled by
//! [`crate::instance::Groups`].

use crate::error::Result;
use crate::util::ensure_dir;
use std::path::{Path, PathBuf};

/// Marker file that switches a launcher directory into portable mode
/// (`portable.dat` next to the executable).
pub const PORTABLE_MARKER: &str = "portable.dat";

/// Global settings file name.
pub const GLOBAL_CONFIG_FILE: &str = "prismlauncher.cfg";

/// Accounts file name.
pub const ACCOUNTS_FILE: &str = "accounts.json";

/// Instance group file name (at the data root).
pub const GROUPS_FILE: &str = "instgroups.json";

/// Target operating system, abstracted for testability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum System {
    /// Windows.
    Windows,
    /// Linux and other Unix-like systems.
    Linux,
    /// macOS.
    MacOS,
}

impl System {
    /// The host system.
    pub fn current() -> System {
        #[cfg(target_os = "windows")]
        {
            System::Windows
        }
        #[cfg(target_os = "macos")]
        {
            System::MacOS
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            System::Linux
        }
    }
}

/// Standard directory/file locations under a data root.
#[derive(Clone, Debug)]
pub struct PrismPaths {
    /// The data root (portable dir or platform data directory).
    pub root: PathBuf,
}

impl PrismPaths {
    /// Paths rooted at an explicit directory.
    pub fn at(root: impl Into<PathBuf>) -> PrismPaths {
        PrismPaths { root: root.into() }
    }

    /// Detect the data root like Prism does: portable mode when
    /// `portable.dat` exists next to the executable, otherwise the platform
    /// data directory.
    pub fn detect() -> PrismPaths {
        let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf));
        if let Some(dir) = exe_dir {
            if dir.join(PORTABLE_MARKER).exists() {
                return PrismPaths { root: dir };
            }
        }
        PrismPaths { root: platform_data_root(System::current()) }
    }

    /// Instance folders root.
    ///
    /// Honors the `InstanceDir` override from `<root>/prismlauncher.cfg`
    /// (see [`PrismPaths::configured_instances_dir`]): real Prism setups
    /// often point this at a custom location outside the data root, so every
    /// consumer (discovery, groups, launch, GUI) must go through this method
    /// rather than assuming `<root>/instances`.
    pub fn instances_dir(&self) -> PathBuf {
        self.configured_instances_dir()
    }

    /// Instance folders root with the `InstanceDir` override from
    /// `<root>/prismlauncher.cfg` applied.
    ///
    /// The value is read with [`crate::ini::load_ini_file`] (which handles
    /// both the verbatim `Key=Value` form and keys under `[General]`). A
    /// missing/unparseable file or an empty value falls back to
    /// `<root>/instances`. Relative values are returned as-is (Prism only
    /// ever writes absolute paths here).
    pub fn configured_instances_dir(&self) -> PathBuf {
        let fallback = self.root.join("instances");
        let map = match crate::ini::load_ini_file(&self.global_config()) {
            Ok(map) => map,
            Err(_) => return fallback,
        };
        match map.get("InstanceDir") {
            Some(raw) => {
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    fallback
                } else {
                    PathBuf::from(trimmed)
                }
            }
            None => fallback,
        }
    }

    /// Currently-selected instance id (`SelectedInstance` from
    /// `<root>/prismlauncher.cfg`), if present and non-blank.
    ///
    /// Callers must still check the id against the discovered instances (a
    /// stale value is ignored). A missing/unparseable file yields `None`.
    pub fn selected_instance_id(&self) -> Option<String> {
        let map = match crate::ini::load_ini_file(&self.global_config()) {
            Ok(map) => map,
            Err(_) => return None,
        };
        match map.get("SelectedInstance") {
            Some(raw) => {
                let id = raw.trim();
                if id.is_empty() {
                    None
                } else {
                    Some(id.to_string())
                }
            }
            None => None,
        }
    }

    /// Global settings file.
    pub fn global_config(&self) -> PathBuf {
        self.root.join(GLOBAL_CONFIG_FILE)
    }

    /// Accounts file.
    pub fn accounts_file(&self) -> PathBuf {
        self.root.join(ACCOUNTS_FILE)
    }

    /// Instance group file (data root, matching Prism's CWD-relative write).
    pub fn groups_file(&self) -> PathBuf {
        self.root.join(GROUPS_FILE)
    }

    /// Legacy group file location inside the instances dir.
    pub fn legacy_groups_file(&self) -> PathBuf {
        self.instances_dir().join(GROUPS_FILE)
    }

    /// Shared assets root (`assets/`).
    pub fn assets_dir(&self) -> PathBuf {
        self.root.join("assets")
    }

    /// Icon storage (`icons/`).
    pub fn icons_dir(&self) -> PathBuf {
        self.root.join("icons")
    }

    /// Shared library cache (`libraries/`).
    pub fn libraries_dir(&self) -> PathBuf {
        self.root.join("libraries")
    }

    /// Shared version jars (`versions/`).
    pub fn versions_dir(&self) -> PathBuf {
        self.root.join("versions")
    }

    /// HTTP cache root (`cache/`).
    pub fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }

    /// Cached component metadata (`cache/meta/`).
    pub fn meta_dir(&self) -> PathBuf {
        self.cache_dir().join("meta")
    }

    /// Launcher logs (`logs/`).
    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    /// Create the standard directory skeleton (like Prism's startup
    /// `mkpath` calls). Missing parents are created too.
    pub fn ensure_layout(&self) -> Result<()> {
        for dir in [
            self.instances_dir(),
            self.assets_dir(),
            self.icons_dir(),
            self.libraries_dir(),
            self.versions_dir(),
            self.cache_dir(),
            self.meta_dir(),
            self.logs_dir(),
        ] {
            ensure_dir(&dir)?;
        }
        Ok(())
    }
}

/// Platform data root (`QStandardPaths::AppDataLocation` behavior for
/// Prism's organization name):
/// Windows `%APPDATA%\PrismLauncher`, macOS
/// `~/Library/Application Support/PrismLauncher`, Linux
/// `$XDG_DATA_HOME/PrismLauncher` (default `~/.local/share/PrismLauncher`).
pub fn platform_data_root(system: System) -> PathBuf {
    let home = home_dir();
    match system {
        System::Windows => {
            let appdata = std::env::var_os("APPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.unwrap_or_default().join("AppData").join("Roaming"));
            appdata.join("PrismLauncher")
        }
        System::MacOS => {
            let base = home.unwrap_or_default();
            base.join("Library").join("Application Support").join("PrismLauncher")
        }
        System::Linux => {
            let xdg = std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute());
            let base = xdg.unwrap_or_else(|| home.unwrap_or_default().join(".local/share"));
            base.join("PrismLauncher")
        }
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_paths_are_stable() {
        let p = PrismPaths::at("/data");
        assert_eq!(p.instances_dir(), PathBuf::from("/data/instances"));
        assert_eq!(p.global_config(), PathBuf::from("/data/prismlauncher.cfg"));
        assert_eq!(p.accounts_file(), PathBuf::from("/data/accounts.json"));
        assert_eq!(p.groups_file(), PathBuf::from("/data/instgroups.json"));
        assert_eq!(p.legacy_groups_file(), PathBuf::from("/data/instances/instgroups.json"));
        assert_eq!(p.meta_dir(), PathBuf::from("/data/cache/meta"));
    }

    #[test]
    fn ensure_layout_creates_everything() {
        let tmp = tempfile::tempdir().unwrap();
        let p = PrismPaths::at(tmp.path());
        p.ensure_layout().unwrap();
        for dir in [p.instances_dir(), p.assets_dir(), p.icons_dir(), p.libraries_dir(), p.versions_dir(), p.cache_dir(), p.meta_dir(), p.logs_dir()] {
            assert!(dir.is_dir(), "missing {:?}", dir);
        }
    }

    #[test]
    fn platform_roots_match_qstandardpaths() {
        assert_eq!(
            platform_data_root(System::Windows),
            std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_default().join("PrismLauncher")
        );
        assert_eq!(
            platform_data_root(System::MacOS),
            home_dir().unwrap_or_default().join("Library/Application Support/PrismLauncher")
        );
        let expected = match std::env::var_os("XDG_DATA_HOME") {
            Some(v) => PathBuf::from(v).join("PrismLauncher"),
            None => home_dir().unwrap_or_default().join(".local/share/PrismLauncher"),
        };
        assert_eq!(platform_data_root(System::Linux), expected);
    }

    #[test]
    fn configured_instances_dir_defaults_without_config() {
        let tmp = tempfile::tempdir().unwrap();
        let p = PrismPaths::at(tmp.path());
        assert_eq!(p.configured_instances_dir(), tmp.path().join("instances"));
        assert_eq!(p.instances_dir(), tmp.path().join("instances"));
        assert!(p.selected_instance_id().is_none());
    }

    #[test]
    fn configured_instances_dir_honors_general_section() {
        let tmp = tempfile::tempdir().unwrap();
        let custom = tmp.path().join("custom-instances");
        // Real Prism files use forward slashes (`E:/Games/...`); backslashes
        // would be mangled by the Qt escape decoding, in Prism too.
        let forward = custom.display().to_string().replace('\\', "/");
        let cfg = format!("[General]\nConfigVersion=1.3\nInstanceDir={forward}\n");
        std::fs::write(tmp.path().join(GLOBAL_CONFIG_FILE), cfg).unwrap();
        let p = PrismPaths::at(tmp.path());
        assert_eq!(p.configured_instances_dir(), PathBuf::from(&forward));
        assert_eq!(p.instances_dir(), PathBuf::from(&forward));
    }

    #[test]
    fn configured_instances_dir_honors_verbatim_top_level_key() {
        // Legacy/top-level form with no section header and no ConfigVersion.
        let tmp = tempfile::tempdir().unwrap();
        let custom = tmp.path().join("alt-instances");
        let forward = custom.display().to_string().replace('\\', "/");
        let cfg = format!("InstanceDir={forward}\n");
        std::fs::write(tmp.path().join(GLOBAL_CONFIG_FILE), cfg).unwrap();
        let p = PrismPaths::at(tmp.path());
        assert_eq!(p.configured_instances_dir(), PathBuf::from(&forward));
    }

    #[test]
    fn configured_instances_dir_ignores_empty_and_garbage() {
        let tmp = tempfile::tempdir().unwrap();
        let fallback = tmp.path().join("instances");
        std::fs::write(tmp.path().join(GLOBAL_CONFIG_FILE), "InstanceDir=\nConfigVersion=1.3\n").unwrap();
        let p = PrismPaths::at(tmp.path());
        assert_eq!(p.configured_instances_dir(), fallback);

        std::fs::write(tmp.path().join(GLOBAL_CONFIG_FILE), "garbage without equals\n").unwrap();
        let p = PrismPaths::at(tmp.path());
        assert_eq!(p.configured_instances_dir(), fallback);
        assert!(p.selected_instance_id().is_none());
    }

    #[test]
    fn selected_instance_id_round_trips_and_trims() {
        let tmp = tempfile::tempdir().unwrap();
        let p = PrismPaths::at(tmp.path());
        assert!(p.selected_instance_id().is_none());

        std::fs::write(
            tmp.path().join(GLOBAL_CONFIG_FILE),
            "[General]\nConfigVersion=1.3\nSelectedInstance=  1.21.1  \n",
        )
        .unwrap();
        assert_eq!(p.selected_instance_id().as_deref(), Some("1.21.1"));

        std::fs::write(tmp.path().join(GLOBAL_CONFIG_FILE), "SelectedInstance=\n").unwrap();
        assert!(p.selected_instance_id().is_none());
    }
}
