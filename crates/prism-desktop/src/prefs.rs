//! The launcher's own preferences, as opposed to Prism's.
//!
//! Prism keeps *launcher* settings in `<root>/prismlauncher.cfg`, and that file
//! is not ours to rewrite: a Prism user's heap sizes, Java paths and instance
//! directory live in it, and a typo written by a second program would break
//! their launcher. So the settings that belong to *this* product go in their own
//! file next to it, written atomically and left alone if it is unparseable.
//!
//! Only what the shell actually offers is stored, and every field is skipped
//! when it still holds its default — so a user who has changed nothing gets a
//! one-line file, and a preference nobody can set is a schema waiting to be
//! wrong.
//!
//! Fields that have a *meaningful* default which is not the zero value are
//! `Option`s where `None` reads as that default ("six concurrent downloads" is
//! `None`, not `Some(6)`), because a skip predicate for "equals six" would be a
//! constant in the schema rather than a statement about the file.

use std::collections::BTreeMap;
use std::path::PathBuf;

use prism_core::paths::PrismPaths;
use serde::{Deserialize, Serialize};

use crate::theme::ColorTheme;

/// The preferences file, under the data root.
pub const PREFS_FILE: &str = "palantirmc-desktop.json";

/// Concurrent downloads when nothing is configured.
pub const DEFAULT_CONCURRENT_DOWNLOADS: u32 = 6;
/// Concurrent disk writes when nothing is configured.
pub const DEFAULT_CONCURRENT_WRITES: u32 = 6;
/// Heap floor, in MiB, for an instance that does not override it.
pub const DEFAULT_MIN_MEM_MIB: u32 = 512;
/// Heap ceiling, in MiB, for an instance that does not override it.
pub const DEFAULT_MAX_MEM_MIB: u32 = 4096;
/// The only interface language this build ships.
pub const DEFAULT_LOCALE: &str = "en-US";

/// "Not changed from the default", for a bool whose default is false.
fn is_false(value: &bool) -> bool {
    !*value
}

/// "Not changed from the default", for a bool whose default is true.
fn is_true(value: &bool) -> bool {
    *value
}

/// Everything the shell remembers between runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// The chosen color theme, by [`ColorTheme::id`].
    ///
    /// Stored as the id string rather than the enum so a file written by a
    /// newer build — or hand-edited — degrades to the default theme instead of
    /// failing the whole parse and losing the rest of the file.
    pub color_theme: String,
    /// Azure application id to sign in with, when the user has their own.
    ///
    /// Empty — and therefore not written — by default, which is what keeps the
    /// file down to the settings the shell can actually change. The shipped
    /// default is Prism Launcher's public client id (see
    /// [`prism_net::DEFAULT_MICROSOFT_CLIENT_ID`]), and
    /// `PALANTIRMC_MSA_CLIENT_ID` overrides it for a one-off run without editing
    /// any file.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub microsoft_client_id: String,

    // ---- Display > Appearance -----------------------------------------
    /// Keep one theme on this device rather than following an account.
    ///
    /// Shipped false, and the pane draws it disabled, because sync needs a
    /// Modrinth account and this launcher does not have one. When it is false
    /// the switch is meaningless — so `true` here is either a hand-edit or a
    /// file from a future build, and it is honoured as written.
    #[serde(skip_serializing_if = "is_false")]
    pub sync_theme_across_devices: bool,
    /// Draw with the effects that need a working GPU.
    #[serde(skip_serializing_if = "is_true")]
    pub advanced_rendering: bool,
    /// Let the OS draw the title bar instead of the shell's own.
    #[serde(skip_serializing_if = "is_false")]
    pub native_decorations: bool,
    /// Open links that leave the launcher in the default browser.
    #[serde(skip_serializing_if = "is_false")]
    pub external_links_new_tab: bool,

    // ---- Display > Features -------------------------------------------
    /// Show the Worlds tab on an instance.
    #[serde(skip_serializing_if = "is_true")]
    pub show_worlds_tab: bool,
    /// Show the Files tab on an instance.
    #[serde(skip_serializing_if = "is_true")]
    pub show_files_tab: bool,
    /// Show the Screenshots tab on an instance.
    #[serde(skip_serializing_if = "is_true")]
    pub show_screenshots_tab: bool,
    /// Let the Screenshots page show every instance at once.
    #[serde(skip_serializing_if = "is_true")]
    pub show_all_screenshots_in_sidebar: bool,
    /// Put a skin selector in the left rail.
    #[serde(skip_serializing_if = "is_false")]
    pub show_skin_selector_in_sidebar: bool,
    /// Put the recently-played instances at the foot of the left rail.
    #[serde(skip_serializing_if = "is_true")]
    pub quick_instances_in_sidebar: bool,
    /// Show the Jump back in section on the Play page.
    #[serde(skip_serializing_if = "is_true")]
    pub show_jump_in_section: bool,

    // ---- Display > Behavior -------------------------------------------
    /// Get out of the way when the game starts.
    #[serde(skip_serializing_if = "is_true")]
    pub minimize_on_launch: bool,
    /// Hide the right-hand panel unless it is asked for.
    #[serde(skip_serializing_if = "is_false")]
    pub hide_right_sidebar: bool,
    /// Draw library entries as compact rows instead of full cards.
    #[serde(skip_serializing_if = "is_false")]
    pub compact_instance_cards: bool,
    /// Show how long each instance has been played.
    #[serde(skip_serializing_if = "is_false")]
    pub show_play_time: bool,
    /// Ask before installing a modpack from outside Modrinth.
    #[serde(skip_serializing_if = "is_true")]
    pub warn_unknown_modpacks: bool,
    /// Skip the warnings that do not change the outcome.
    #[serde(skip_serializing_if = "is_false")]
    pub skip_non_essential_warnings: bool,

    // ---- Display > Language -------------------------------------------
    /// The interface language, by BCP-47 tag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    /// Whether the hidden developer tab is showing.
    ///
    /// Turned on by clicking the version in the Settings footer six times, as
    /// the reference does. Survives a restart because the reference stores it
    /// too, and a hidden tab that reappears on every launch is a puzzle rather
    /// than a setting.
    #[serde(skip_serializing_if = "is_false")]
    pub developer_mode: bool,

    // ---- Account > Privacy --------------------------------------------
    /// Send anonymous usage counts. Off, and not offered, because this
    /// launcher has no analytics endpoint to send them to.
    #[serde(skip_serializing_if = "is_false")]
    pub telemetry: bool,
    /// Announce the running game over Discord's RPC socket.
    #[serde(skip_serializing_if = "is_false")]
    pub discord_rpc: bool,

    // ---- Instances ---------------------------------------------------
    /// Heap floor a new instance starts with, in MiB.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_min_mem_mib: Option<u32>,
    /// Heap ceiling a new instance starts with, in MiB.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_max_mem_mib: Option<u32>,
    /// Java binary a new instance starts with.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_java_path: Option<String>,
    /// How many downloads may be in flight at once.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_concurrent_downloads: Option<u32>,
    /// How many files may be written at once.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_concurrent_writes: Option<u32>,
    /// Keep the full copy details on screen rather than behind a hover.
    #[serde(skip_serializing_if = "is_false")]
    pub always_show_copy_details: bool,
    /// The data root, when it has been moved off Prism's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_directory: Option<String>,
    /// A Java binary per major version, keyed by the major ("25", "21", …).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub java_paths: BTreeMap<String, String>,
}

impl Default for Prefs {
    /// The shipped defaults, which are **not** the zero values.
    ///
    /// Derived `Default` would answer `false` to every switch, and "advanced
    /// rendering: off" is not a neutral choice — it is a different, worse
    /// launcher. Hand-written so each default is a decision.
    fn default() -> Self {
        Prefs {
            color_theme: String::new(),
            microsoft_client_id: String::new(),
            sync_theme_across_devices: false,
            advanced_rendering: true,
            native_decorations: false,
            external_links_new_tab: false,
            show_worlds_tab: true,
            show_files_tab: true,
            show_screenshots_tab: true,
            show_all_screenshots_in_sidebar: true,
            show_skin_selector_in_sidebar: false,
            quick_instances_in_sidebar: true,
            show_jump_in_section: true,
            minimize_on_launch: true,
            hide_right_sidebar: false,
            compact_instance_cards: false,
            show_play_time: false,
            warn_unknown_modpacks: true,
            skip_non_essential_warnings: false,
            locale: None,
            developer_mode: false,
            telemetry: false,
            discord_rpc: false,
            default_min_mem_mib: None,
            default_max_mem_mib: None,
            default_java_path: None,
            max_concurrent_downloads: None,
            max_concurrent_writes: None,
            always_show_copy_details: false,
            app_directory: None,
            java_paths: BTreeMap::new(),
        }
    }
}

impl Prefs {
    /// The theme this file selects, defaulting when it names nothing known.
    pub fn theme(&self) -> ColorTheme {
        ColorTheme::from_id(&self.color_theme)
    }

    /// The file for a given choice.
    pub fn with_theme(theme: ColorTheme) -> Prefs {
        Prefs { color_theme: theme.id().to_string(), ..Prefs::default() }
    }

    /// Downloads allowed in flight, with the shipped default filled in.
    pub fn concurrent_downloads(&self) -> u32 {
        self.max_concurrent_downloads.unwrap_or(DEFAULT_CONCURRENT_DOWNLOADS)
    }

    /// Writes allowed in flight, with the shipped default filled in.
    pub fn concurrent_writes(&self) -> u32 {
        self.max_concurrent_writes.unwrap_or(DEFAULT_CONCURRENT_WRITES)
    }

    /// The heap floor a new instance starts with.
    pub fn min_mem_mib(&self) -> u32 {
        self.default_min_mem_mib.unwrap_or(DEFAULT_MIN_MEM_MIB)
    }

    /// The heap ceiling a new instance starts with.
    pub fn max_mem_mib(&self) -> u32 {
        self.default_max_mem_mib.unwrap_or(DEFAULT_MAX_MEM_MIB)
    }

    /// The interface language, defaulting to the one this build ships.
    pub fn locale(&self) -> &str {
        self.locale.as_deref().unwrap_or(DEFAULT_LOCALE)
    }

    /// The Java binary to run `major` with, if one has been chosen.
    pub fn java_path(&self, major: &str) -> Option<&str> {
        self.java_paths.get(major).map(String::as_str).filter(|path| !path.is_empty())
    }

    /// The OAuth client id the sign-in flow should use.
    ///
    /// Order: the environment (`PALANTIRMC_MSA_CLIENT_ID`), then the file, then
    /// Prism's public client id. The environment variable exists so a user can
    /// point their own Azure application at one run without editing a file, and
    /// it wins so that "run it once with this id" always means what it says.
    pub fn microsoft_client_id(&self) -> String {
        let from_env = std::env::var("PALANTIRMC_MSA_CLIENT_ID").unwrap_or_default();
        let from_env = from_env.trim();
        if !from_env.is_empty() {
            return from_env.to_string();
        }
        let configured = self.microsoft_client_id.trim();
        if configured.is_empty() {
            prism_net::DEFAULT_MICROSOFT_CLIENT_ID.to_string()
        } else {
            configured.to_string()
        }
    }
}

/// Where the preferences live for a data root.
pub fn path(paths: &PrismPaths) -> PathBuf {
    paths.root.join(PREFS_FILE)
}

/// Read the preferences, falling back to the defaults.
///
/// A missing file is the normal first run; an unreadable or unparseable one is
/// treated the same way rather than being an error the user has to deal with.
/// The corrupt file is left in place — overwriting it would destroy whatever
/// the user was editing when it broke.
pub fn load(paths: &PrismPaths) -> Prefs {
    std::fs::read_to_string(path(paths))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Write the preferences, atomically.
///
/// Atomic so a crash mid-write cannot leave a half-written file that the next
/// run reads as the defaults: the theme would silently reset.
pub fn save(paths: &PrismPaths, prefs: &Prefs) -> Result<(), String> {
    let text = serde_json::to_string_pretty(prefs).map_err(|error| error.to_string())?;
    let mut bytes = text.into_bytes();
    bytes.push(b'\n');
    prism_core::util::atomic_write(&path(paths), &bytes).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> (tempfile::TempDir, PrismPaths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = PrismPaths::at(dir.path());
        (dir, paths)
    }

    #[test]
    fn a_missing_file_is_the_default_theme() {
        let (_dir, paths) = root();
        assert!(!path(&paths).exists());
        assert_eq!(load(&paths).theme(), ColorTheme::Dark);
    }

    #[test]
    fn a_theme_choice_survives_a_round_trip() {
        let (_dir, paths) = root();
        for theme in ColorTheme::ALL {
            save(&paths, &Prefs::with_theme(theme)).unwrap();
            assert_eq!(load(&paths).theme(), theme, "{} did not survive", theme.id());
        }
    }

    #[test]
    fn the_file_is_json_with_only_what_the_shell_offers() {
        let (_dir, paths) = root();
        save(&paths, &Prefs::with_theme(ColorTheme::Oled)).unwrap();
        let text = std::fs::read_to_string(path(&paths)).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(parsed["color_theme"], "oled");
        assert_eq!(
            parsed.as_object().unwrap().len(),
            1,
            "only the settings that exist should be written: {text}"
        );
    }

    #[test]
    fn a_damaged_or_strange_file_never_breaks_startup() {
        let (_dir, paths) = root();
        for text in [&b"{ not json"[..], b"", b"[]", b"null", b"{\"color_theme\": 7}"] {
            std::fs::write(path(&paths), text).unwrap();
            assert_eq!(load(&paths).theme(), ColorTheme::Dark, "text: {text:?}");
        }
        // A valid file naming a theme this build does not have still loads, and
        // keeps the choice it can honour.
        std::fs::write(path(&paths), b"{\"color_theme\":\"neon\"}").unwrap();
        assert_eq!(load(&paths).theme(), ColorTheme::Dark);
    }

    #[test]
    fn the_client_id_falls_back_to_prisms_public_app() {
        let prefs = Prefs::default();
        assert_eq!(prefs.microsoft_client_id(), prism_net::DEFAULT_MICROSOFT_CLIENT_ID);
        let configured = Prefs { microsoft_client_id: "my-app-id".into(), ..Prefs::default() };
        assert_eq!(configured.microsoft_client_id(), "my-app-id");
        // Whitespace is not a client id.
        let blank = Prefs { microsoft_client_id: "   ".into(), ..Prefs::default() };
        assert_eq!(blank.microsoft_client_id(), prism_net::DEFAULT_MICROSOFT_CLIENT_ID);
    }

    #[test]
    fn an_empty_client_id_is_not_written_to_the_file() {
        let (_dir, paths) = root();
        save(&paths, &Prefs::with_theme(ColorTheme::Dark)).unwrap();
        let text = std::fs::read_to_string(path(&paths)).unwrap();
        assert!(!text.contains("microsoft_client_id"), "got: {text}");
    }

    #[test]
    fn changing_the_theme_keeps_a_configured_client_id() {
        let (_dir, paths) = root();
        save(
            &paths,
            &Prefs {
                color_theme: "dark".into(),
                microsoft_client_id: "mine".into(),
                ..Prefs::default()
            },
        )
        .unwrap();
        // Changing the theme is not a reason to forget a configured client id,
        // which is why the whole struct is written rather than one key.
        let mut prefs = load(&paths);
        prefs.color_theme = ColorTheme::Oled.id().to_string();
        save(&paths, &prefs).unwrap();
        let back = load(&paths);
        assert_eq!(back.theme(), ColorTheme::Oled);
        assert_eq!(back.microsoft_client_id, "mine");
    }

    #[test]
    fn the_file_is_not_prisms_config() {
        // Writing this launcher's settings must never touch the file Prism owns.
        let (_dir, paths) = root();
        let prism = paths.global_config();
        std::fs::write(&prism, b"[General]\nMaxMemAlloc=4096\n").unwrap();
        save(&paths, &Prefs::with_theme(ColorTheme::Light)).unwrap();
        assert_eq!(
            std::fs::read_to_string(&prism).unwrap(),
            "[General]\nMaxMemAlloc=4096\n",
            "prismlauncher.cfg must be left exactly as it was"
        );
        assert_ne!(path(&paths), prism);
    }

    #[test]
    fn the_defaults_are_decisions_rather_than_zeroes() {
        // A derived `Default` would answer `false` to all of these, and three of
        // them would then describe a different launcher.
        let prefs = Prefs::default();
        assert!(prefs.advanced_rendering, "a GPU is there until proven otherwise");
        assert!(prefs.show_worlds_tab && prefs.show_files_tab && prefs.show_screenshots_tab);
        assert!(prefs.minimize_on_launch);
        assert!(prefs.warn_unknown_modpacks);
        assert!(!prefs.native_decorations, "the shell draws its own frame");
        assert!(!prefs.hide_right_sidebar);
        assert!(!prefs.telemetry, "nothing is collected, so nothing is on");
    }

    #[test]
    fn the_filled_in_defaults_are_the_documented_numbers() {
        let prefs = Prefs::default();
        assert_eq!(prefs.concurrent_downloads(), DEFAULT_CONCURRENT_DOWNLOADS);
        assert_eq!(prefs.concurrent_writes(), DEFAULT_CONCURRENT_WRITES);
        assert_eq!(prefs.min_mem_mib(), DEFAULT_MIN_MEM_MIB);
        assert_eq!(prefs.max_mem_mib(), DEFAULT_MAX_MEM_MIB);
        assert_eq!(prefs.locale(), DEFAULT_LOCALE);
        assert_eq!(prefs.java_path("21"), None);
    }

    #[test]
    fn a_setting_nobody_touched_is_not_written_to_the_file() {
        // The point of skipping defaults: the file is the diff from the
        // defaults, so it stays readable and a new field does not appear in it
        // just because this build added it.
        let (_dir, paths) = root();
        let untouched = Prefs { color_theme: "dark".into(), ..Prefs::default() };
        save(&paths, &untouched).unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path(&paths)).unwrap()).unwrap();
        assert_eq!(parsed.as_object().unwrap().len(), 1, "got: {parsed}");

        // …and a changed one is written, including a return *to* the default,
        // which is why every non-zero default needs its own predicate.
        let changed = Prefs {
            color_theme: "dark".into(),
            advanced_rendering: false,
            show_worlds_tab: false,
            ..Prefs::default()
        };
        save(&paths, &changed).unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path(&paths)).unwrap()).unwrap();
        assert_eq!(parsed["advanced_rendering"], serde_json::json!(false));
        assert_eq!(parsed["show_worlds_tab"], serde_json::json!(false));
        assert_eq!(load(&paths), changed, "a written file must read back identical");
    }

    #[test]
    fn a_file_from_before_these_settings_existed_still_loads() {
        // The compatibility promise: the old file is one key, and every new
        // setting comes from the defaults rather than from `false`.
        let (_dir, paths) = root();
        std::fs::write(path(&paths), b"{\"color_theme\":\"oled\"}").unwrap();
        let back = load(&paths);
        assert_eq!(back.theme(), ColorTheme::Oled);
        assert!(back.advanced_rendering, "an absent field is the default, not false");
        assert!(back.minimize_on_launch);
        assert_eq!(back.concurrent_downloads(), DEFAULT_CONCURRENT_DOWNLOADS);
    }

    #[test]
    fn every_setting_survives_a_round_trip() {
        // Catches a `skip_serializing_if` that skips a value it should keep,
        // which is the one way this schema can silently lose a choice.
        let (_dir, paths) = root();
        let mut java_paths = BTreeMap::new();
        java_paths.insert("21".to_string(), "C:/jdk21/bin/javaw.exe".to_string());
        let every = Prefs {
            color_theme: "light".into(),
            microsoft_client_id: "abc".into(),
            sync_theme_across_devices: true,
            advanced_rendering: false,
            native_decorations: true,
            external_links_new_tab: true,
            show_worlds_tab: false,
            show_files_tab: false,
            show_screenshots_tab: false,
            show_all_screenshots_in_sidebar: false,
            show_skin_selector_in_sidebar: true,
            quick_instances_in_sidebar: false,
            show_jump_in_section: false,
            minimize_on_launch: false,
            hide_right_sidebar: true,
            compact_instance_cards: true,
            show_play_time: true,
            warn_unknown_modpacks: false,
            skip_non_essential_warnings: true,
            locale: Some("de-DE".into()),
            developer_mode: true,
            telemetry: true,
            discord_rpc: true,
            default_min_mem_mib: Some(1024),
            default_max_mem_mib: Some(8192),
            default_java_path: Some("C:/jdk21/bin/javaw.exe".into()),
            max_concurrent_downloads: Some(3),
            max_concurrent_writes: Some(2),
            always_show_copy_details: true,
            app_directory: Some("D:/minecraft".into()),
            java_paths,
        };
        save(&paths, &every).unwrap();
        assert_eq!(load(&paths), every);
    }

    #[test]
    fn a_setting_of_the_wrong_type_never_breaks_startup() {
        // Same promise the theme already made, extended to the numbers: a
        // hand-edited file must not take the window down.
        let (_dir, paths) = root();
        for text in [
            &b"{\"advanced_rendering\": \"yes\"}"[..],
            b"{\"max_concurrent_downloads\": \"six\"}",
            b"{\"java_paths\": []}",
            b"{\"locale\": 7}",
        ] {
            std::fs::write(path(&paths), text).unwrap();
            let back = load(&paths);
            assert!(back.advanced_rendering, "text: {text:?}");
            assert_eq!(back.concurrent_downloads(), DEFAULT_CONCURRENT_DOWNLOADS);
        }
    }

    #[test]
    fn a_blank_java_path_reads_as_no_choice() {
        // The Java pane clears a path by storing an empty string, and an empty
        // string is not a binary.
        let mut prefs = Prefs::default();
        prefs.java_paths.insert("21".into(), String::new());
        assert_eq!(prefs.java_path("21"), None);
        prefs.java_paths.insert("21".into(), "javaw".into());
        assert_eq!(prefs.java_path("21"), Some("javaw"));
    }
}
