//! Data-root discovery and the launcher's folder layout.
//!
//! The layout is the one Prism's `Application.cpp` and `MinecraftInstance.cpp`
//! produce — `instances/`, `libraries/`, `versions/`, `assets/`, `icons/`,
//! `cache/` and the files `prismlauncher.cfg`, `accounts.json` — because being
//! layout-compatible is what lets an instance created in either launcher open in
//! the other. `instgroups.json` lives at the data root (Prism uses
//! `QDir::current().filePath("instgroups.json")`, whose CWD is the launcher
//! directory), with a legacy copy inside the instances dir handled by
//! [`crate::instance::Groups`].
//!
//! Two roots, and the difference matters on a machine that has both launchers:
//!
//! * [`PalantirPaths::home`] is *this product's* directory —
//!   `%APPDATA%\PalantirMC` and its macOS/Linux equivalents. This launcher's own
//!   preferences live there and nowhere else, so an answer about where the game
//!   data should go survives the choice being made.
//! * The data root is where the instances, assets and libraries are. It is the
//!   product directory unless a preference points it somewhere else — which is
//!   how somebody keeps using an install another launcher created, in place,
//!   instead of abandoning it.
//!
//! Portable mode is unchanged and wins over both: a `portable.dat` next to the
//! executable makes the executable's own directory the product directory *and*
//! the data root, and no migration is offered there because a portable tree is
//! already a complete install.
//!
//! The pointer itself is read here rather than in the shell, because every
//! binary in the workspace needs the same answer: a `palantir-cli verify` run
//! that resolved a different data root from the window would report on a
//! different install than the one the user is looking at.

use crate::error::Result;
use crate::util::ensure_dir;
use std::path::{Path, PathBuf};

/// Marker file that switches a launcher directory into portable mode
/// (`portable.dat` next to the executable).
pub const PORTABLE_MARKER: &str = "portable.dat";

/// This product's own directory name, under the platform's data location.
pub const DATA_DIR_NAME: &str = "PalantirMC";

/// The other launcher's directory name.
///
/// Read, never written: it names an install somebody already has, and it is the
/// one thing about the migration that is not ours to rename.
pub const LEGACY_DATA_DIR_NAME: &str = "PrismLauncher";

/// Environment override for this product's own directory.
///
/// The whole tree this launcher owns moves with it, which is what a test, a
/// second profile or a sandboxed run needs — and what the first-run question
/// would otherwise make impossible to exercise without touching `%APPDATA%`.
pub const HOME_ENV: &str = "PALANTIRMC_HOME";

/// The answer to the first-run question, as stored in the preferences.
pub const ROOT_CHOICE_KEEP: &str = "keep";
/// Use the product's own folder and start with nothing in it.
pub const ROOT_CHOICE_FRESH: &str = "fresh";

/// This launcher's own preferences file, at [`PalantirPaths::home`].
///
/// Named separately from the other launcher's `prismlauncher.cfg` on purpose:
/// the shared files are the game's, and this one is ours. It is declared here
/// so the shell that writes it and the code that reads one key out of it can
/// never disagree about its name.
pub const PREFS_FILE: &str = "palantirmc-desktop.json";

/// The key inside [`PREFS_FILE`] that points the data root elsewhere.
///
/// The pointer lives in the shell's own settings rather than in a file of its
/// own: a second file would be a second thing to keep in step, and the shell
/// already has to write the answer down somewhere.
pub const DATA_ROOT_KEY: &str = "app_directory";

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
pub struct PalantirPaths {
    /// The data root (portable dir or platform data directory).
    pub root: PathBuf,
}

impl PalantirPaths {
    /// Paths rooted at an explicit directory.
    pub fn at(root: impl Into<PathBuf>) -> PalantirPaths {
        PalantirPaths { root: root.into() }
    }

    /// This product's own directory.
    ///
    /// [`HOME_ENV`] wins, then portable mode, then the platform data location.
    /// This is where this launcher's preferences live, and it is a directory the
    /// product can call its own — the other launcher's folder is never used for
    /// our files.
    pub fn home() -> PalantirPaths {
        if let Some(dir) = std::env::var_os(HOME_ENV).map(PathBuf::from).filter(|p| p.is_absolute()) {
            return PalantirPaths { root: dir };
        }
        if let Some(dir) = portable_dir() {
            return PalantirPaths { root: dir };
        }
        PalantirPaths { root: platform_data_root(System::current()) }
    }

    /// The data root a real run should use.
    ///
    /// Portable mode wins outright — a `portable.dat` tree is a complete
    /// install, and resolving a pointer out of `%APPDATA%` for it would let one
    /// machine's preferences reach into another machine's stick. Otherwise this
    /// is this product's own directory, unless the settings point somewhere
    /// else, which is how an install another launcher created stays in use.
    pub fn detect() -> PalantirPaths {
        let home = PalantirPaths::home();
        if home.is_portable() {
            return home;
        }
        let configured = recorded_data_root(&home);
        PalantirPaths::at(resolve_data_root(&home, configured.as_deref()))
    }

    /// Whether this root is a portable tree, in which case it is also the
    /// product's own directory and nothing is offered to import.
    pub fn is_portable(&self) -> bool {
        self.root.join(PORTABLE_MARKER).is_file()
    }

    /// Instance folders root.
    ///
    /// Honors the `InstanceDir` override from `<root>/prismlauncher.cfg`
    /// (see [`PalantirPaths::configured_instances_dir`]): real Prism setups
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

/// Platform data location (`QStandardPaths::AppDataLocation` behavior) with
/// `name` appended:
/// Windows `%APPDATA%\<name>`, macOS `~/Library/Application Support/<name>`,
/// Linux `$XDG_DATA_HOME/<name>` (default `~/.local/share/<name>`).
pub fn platform_root_named(system: System, name: &str) -> PathBuf {
    let home = home_dir();
    match system {
        System::Windows => {
            let appdata = std::env::var_os("APPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.unwrap_or_default().join("AppData").join("Roaming"));
            appdata.join(name)
        }
        System::MacOS => {
            let base = home.unwrap_or_default();
            base.join("Library").join("Application Support").join(name)
        }
        System::Linux => {
            let xdg = std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute());
            let base = xdg.unwrap_or_else(|| home.unwrap_or_default().join(".local/share"));
            base.join(name)
        }
    }
}

/// This product's platform data directory (`%APPDATA%\PalantirMC`, ...).
pub fn platform_data_root(system: System) -> PathBuf {
    platform_root_named(system, DATA_DIR_NAME)
}

/// The other launcher's platform data directory on this machine.
pub fn platform_legacy_root(system: System) -> PathBuf {
    platform_root_named(system, LEGACY_DATA_DIR_NAME)
}

/// The executable's own directory, when it holds a [`PORTABLE_MARKER`].
fn portable_dir() -> Option<PathBuf> {
    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf))?;
    exe_dir.join(PORTABLE_MARKER).exists().then_some(exe_dir)
}

/// An install somebody already has, found on this machine.
///
/// Reported so the first run can offer to use it rather than start from
/// nothing. The path is *not* copied or modified: adopting it means pointing the
/// data root at it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacyInstall {
    /// The other launcher's data root.
    pub root: PathBuf,
    /// How many instance folders are directly inside its instances directory.
    pub instances: usize,
}

impl LegacyInstall {
    /// The count, worded, for a dialog button: "1 instance" / "3 instances".
    ///
    /// Here rather than at the call site because the two places that say it —
    /// the summary line and the button — have to agree, and a sentence built
    /// from the same number twice is the kind of thing that drifts apart.
    pub fn instances_label(&self) -> String {
        if self.instances == 1 {
            "1 instance".to_string()
        } else {
            format!("{} instances", self.instances)
        }
    }
}

/// The other launcher's install, if there is one.
pub fn legacy_install() -> Option<LegacyInstall> {
    legacy_install_at(&platform_legacy_root(System::current()))
}

/// The same discovery against an explicit root, so it can be tested without
/// touching the machine's real `%APPDATA%`.
pub fn legacy_install_at(root: &Path) -> Option<LegacyInstall> {
    if !root.is_dir() {
        return None;
    }
    let instances = count_instances(root);
    (instances > 0).then(|| LegacyInstall { root: root.to_path_buf(), instances })
}

/// How many instance folders a root holds, by that root's own configuration.
///
/// The instance directory is whatever the root's `prismlauncher.cfg` says it
/// is: a custom `InstanceDir` is common, and reading the file rather than
/// assuming `instances/` is the difference between finding an install and
/// deciding there is nothing there.
///
/// Only directories count. An `instances/` folder holding a stray README is not
/// an install, and a dialog that offered to bring across a text file would be
/// the kind of "help" a first run remembers for the wrong reason.
pub fn count_instances(root: &Path) -> usize {
    let instances_dir = PalantirPaths::at(root).configured_instances_dir();
    std::fs::read_dir(&instances_dir)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .count()
}

/// The data root a stored pointer names.
///
/// Pure, so the rule is testable without a machine that happens to have both
/// launchers on it:
///
/// * nothing configured, or nothing but whitespace, means this product's own
///   directory;
/// * a *relative* path is refused and treated as nothing configured. It would
///   otherwise mean a different install depending on the working directory the
///   window was started from, which is not a thing a path in a settings file may
///   mean;
/// * an absolute path is taken as written, and deliberately *not* required to
///   exist: a root on a drive that is not mounted yet is still the root, and
///   answering "yours, then" on a transient failure would silently point the
///   launcher at an empty folder.
///
/// The path is returned as given rather than normalized — `%APPDATA%` is the
/// user's to see, and it should look the same in the settings pane, in the log
/// and in this answer.
pub fn resolve_data_root(home: &PalantirPaths, configured: Option<&str>) -> PathBuf {
    let Some(raw) = configured else {
        return home.root.clone();
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return home.root.clone();
    }
    let candidate = PathBuf::from(trimmed);
    if candidate.is_absolute() {
        candidate
    } else {
        home.root.clone()
    }
}

/// One key out of the shell's settings file.
///
/// A deliberately partial view of a schema the shell owns: the CLI and the pack
/// tools need the *location*, not the twenty switches beside it, and redeclaring
/// the whole file here to find one key would be a second copy of a format that
/// would then drift from the first.
#[derive(serde::Deserialize)]
struct DataRootPointer {
    #[serde(default)]
    app_directory: Option<String>,
}

/// The data root this product's own settings point at, if it points anywhere.
///
/// A missing, unreadable or unparseable file answers `None` — the same
/// degradation the shell's own loader makes, and for the same reason: a
/// hand-edited settings file must not stop the launcher from starting.
pub fn recorded_data_root(home: &PalantirPaths) -> Option<String> {
    let text = std::fs::read_to_string(home.root.join(PREFS_FILE)).ok()?;
    let parsed: DataRootPointer = serde_json::from_str(&text).ok()?;
    parsed.app_directory.filter(|value| !value.trim().is_empty())
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
        let p = PalantirPaths::at("/data");
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
        let p = PalantirPaths::at(tmp.path());
        p.ensure_layout().unwrap();
        for dir in [p.instances_dir(), p.assets_dir(), p.icons_dir(), p.libraries_dir(), p.versions_dir(), p.cache_dir(), p.meta_dir(), p.logs_dir()] {
            assert!(dir.is_dir(), "missing {:?}", dir);
        }
    }

    #[test]
    fn platform_roots_match_qstandardpaths() {
        assert_eq!(
            platform_data_root(System::Windows),
            std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_default().join("PalantirMC")
        );
        assert_eq!(
            platform_data_root(System::MacOS),
            home_dir().unwrap_or_default().join("Library/Application Support/PalantirMC")
        );
        let expected = match std::env::var_os("XDG_DATA_HOME") {
            Some(v) => PathBuf::from(v).join("PalantirMC"),
            None => home_dir().unwrap_or_default().join(".local/share/PalantirMC"),
        };
        assert_eq!(platform_data_root(System::Linux), expected);
    }

    #[test]
    fn the_legacy_root_is_the_other_launcher_beside_ours() {
        // Same location, different name: the migration reads a directory the
        // product does not own, and the two must not be the same path.
        for system in [System::Windows, System::MacOS, System::Linux] {
            let ours = platform_data_root(system);
            let theirs = platform_legacy_root(system);
            assert_ne!(ours, theirs);
            assert_eq!(ours.parent(), theirs.parent());
            assert_eq!(ours.file_name().unwrap(), DATA_DIR_NAME);
            assert_eq!(theirs.file_name().unwrap(), LEGACY_DATA_DIR_NAME);
        }
    }

    #[test]
    fn a_legacy_install_is_found_when_it_holds_instances() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        // No directory at all is not an install...
        assert_eq!(legacy_install_at(&root.join("missing")), None);
        // ...a directory with an empty instances/ is not one either: offering
        // to bring across nothing is a dialog that wastes a first run.
        std::fs::create_dir_all(root.join("instances")).unwrap();
        assert_eq!(legacy_install_at(root), None);
        std::fs::create_dir_all(root.join("instances/One")).unwrap();
        std::fs::create_dir_all(root.join("instances/Two")).unwrap();
        std::fs::write(root.join("instances/README.txt"), "not an instance").unwrap();
        assert_eq!(
            legacy_install_at(root),
            Some(LegacyInstall { root: root.to_path_buf(), instances: 2 }),
            "only directories count as instances"
        );
    }

    #[test]
    fn counting_instances_ignores_files_and_missing_directories() {
        let tmp = tempfile::tempdir().unwrap();
        // Nothing at all is zero rather than an error: this is asked on every
        // first run, including on machines that have never held a launcher.
        assert_eq!(count_instances(tmp.path()), 0);
        std::fs::create_dir_all(tmp.path().join("instances/Keep")).unwrap();
        std::fs::write(tmp.path().join("instances/loose-file"), "").unwrap();
        assert_eq!(count_instances(tmp.path()), 1, "a file is not an instance");
    }

    #[test]
    fn the_install_label_is_singular_only_for_one() {
        let root = PathBuf::from("/p");
        let label = |instances| LegacyInstall { root: root.clone(), instances }.instances_label();
        assert_eq!(label(1), "1 instance");
        assert_eq!(label(2), "2 instances");
        assert_eq!(label(0), "0 instances");
    }

    #[test]
    fn a_legacy_install_with_a_moved_instance_dir_is_found_where_it_says() {
        // The other launcher lets the instance directory be relocated, and a
        // real setup often does. Counting `<root>/instances` in that case would
        // find nothing and refuse to offer an install that is right there.
        let tmp = tempfile::tempdir().unwrap();
        let elsewhere = tmp.path().join("elsewhere").join("instances");
        std::fs::create_dir_all(elsewhere.join("Pack")).unwrap();
        let forward = elsewhere.display().to_string().replace('\\', "/");
        std::fs::write(
            tmp.path().join(GLOBAL_CONFIG_FILE),
            format!("InstanceDir={forward}\nConfigVersion=1.3\n"),
        )
        .unwrap();
        let found = legacy_install_at(tmp.path()).expect("the moved directory holds an instance");
        assert_eq!(found.instances, 1);
    }

    #[test]
    fn an_unconfigured_data_root_is_this_products_own_directory() {
        let home = PalantirPaths::at("/home/u/.local/share/PalantirMC");
        assert_eq!(resolve_data_root(&home, None), home.root);
        assert_eq!(resolve_data_root(&home, Some("")), home.root);
        assert_eq!(resolve_data_root(&home, Some("   ")), home.root);
    }

    #[test]
    fn a_relative_data_root_is_refused_rather_than_obeyed() {
        // A relative path would mean a different install depending on the
        // working directory the window was started from. `C:minecraft` is
        // relative on Windows too — it means "the current directory on C:" —
        // which is exactly the shape that must not be obeyed.
        let home = PalantirPaths::at("/home/u/.local/share/PalantirMC");
        assert_eq!(resolve_data_root(&home, Some("instances/../..")), home.root);
        assert_eq!(resolve_data_root(&home, Some("C:minecraft")), home.root);
    }

    #[test]
    fn an_absolute_data_root_is_taken_even_before_it_exists() {
        // A root on an unmounted drive is still the root. Answering "yours,
        // then" on a transient failure would point the launcher at an empty
        // folder and look, from the outside, like the instances were deleted.
        let home = PalantirPaths::at("/home/u/.local/share/PalantirMC");
        let absolute = if cfg!(windows) { "D:\\games\\mc" } else { "/mnt/games/mc" };
        assert_eq!(resolve_data_root(&home, Some(absolute)), PathBuf::from(absolute));
        // Surrounding whitespace is not part of a path.
        let padded = format!("  {absolute}  ");
        assert_eq!(resolve_data_root(&home, Some(&padded)), PathBuf::from(absolute));
        assert!(!PathBuf::from(absolute).exists(), "the point is that it need not exist");
    }

    #[test]
    fn the_recorded_pointer_is_read_from_the_settings_file() {
        let tmp = tempfile::tempdir().unwrap();
        let home = PalantirPaths::at(tmp.path());
        // Nothing written: nothing pointed at.
        assert_eq!(recorded_data_root(&home), None);
        std::fs::write(
            tmp.path().join(PREFS_FILE),
            b"{\"color_theme\":\"dark\",\"app_directory\":\"D:/mc\"}",
        )
        .unwrap();
        assert_eq!(recorded_data_root(&home).as_deref(), Some("D:/mc"));
        // A blank value is not a pointer, and a damaged file is not an error.
        std::fs::write(tmp.path().join(PREFS_FILE), b"{\"app_directory\":\"   \"}").unwrap();
        assert_eq!(recorded_data_root(&home), None);
        std::fs::write(tmp.path().join(PREFS_FILE), b"{ not json").unwrap();
        assert_eq!(recorded_data_root(&home), None);
    }

    #[test]
    fn detect_answers_the_pointer_for_both_binaries() {
        // The CLI and the window must not resolve different roots: the pointer
        // is read here, once, so that they cannot.
        let tmp = tempfile::tempdir().unwrap();
        let home = PalantirPaths::at(tmp.path());
        let elsewhere = tmp.path().join("legacy");
        std::fs::write(
            tmp.path().join(PREFS_FILE),
            format!("{{\"app_directory\":{:?}}}", elsewhere.display().to_string()),
        )
        .unwrap();
        let recorded = recorded_data_root(&home).expect("the file names a root");
        assert_eq!(resolve_data_root(&home, Some(&recorded)), elsewhere);
    }

    #[test]
    fn configured_instances_dir_defaults_without_config() {
        let tmp = tempfile::tempdir().unwrap();
        let p = PalantirPaths::at(tmp.path());
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
        let p = PalantirPaths::at(tmp.path());
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
        let p = PalantirPaths::at(tmp.path());
        assert_eq!(p.configured_instances_dir(), PathBuf::from(&forward));
    }

    #[test]
    fn configured_instances_dir_ignores_empty_and_garbage() {
        let tmp = tempfile::tempdir().unwrap();
        let fallback = tmp.path().join("instances");
        std::fs::write(tmp.path().join(GLOBAL_CONFIG_FILE), "InstanceDir=\nConfigVersion=1.3\n").unwrap();
        let p = PalantirPaths::at(tmp.path());
        assert_eq!(p.configured_instances_dir(), fallback);

        std::fs::write(tmp.path().join(GLOBAL_CONFIG_FILE), "garbage without equals\n").unwrap();
        let p = PalantirPaths::at(tmp.path());
        assert_eq!(p.configured_instances_dir(), fallback);
        assert!(p.selected_instance_id().is_none());
    }

    #[test]
    fn selected_instance_id_round_trips_and_trims() {
        let tmp = tempfile::tempdir().unwrap();
        let p = PalantirPaths::at(tmp.path());
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
