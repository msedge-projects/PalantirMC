//! The launcher's own preferences, as opposed to Prism's.
//!
//! Prism keeps *launcher* settings in `<root>/prismlauncher.cfg`, and that file
//! is not ours to rewrite: a Prism user's heap sizes, Java paths and instance
//! directory live in it, and a typo written by a second program would break
//! their launcher. So the settings that belong to *this* product go in their own
//! file, written atomically and left alone if it is unparseable.
//!
//! That file lives at [`PalantirPaths::home`] — this product's own directory —
//! and **not** at the data root, which is the whole reason the two are separate
//! paths. The data root can be an install another launcher created, and it can
//! change; where this launcher's own settings live cannot, or the answer about
//! where the game data should go would be stored inside the thing it answers.
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

use palantir_core::paths::PalantirPaths;
use serde::{Deserialize, Serialize};

use crate::theme::ColorTheme;

/// The preferences file, under this product's own directory.
///
/// The name is [`palantir_core::paths::PREFS_FILE`] rather than a second
/// literal: core reads one key out of this file to resolve the data root, and a
/// second spelling of the name here is the one way the two could stop meaning
/// the same file.
pub const PREFS_FILE: &str = palantir_core::paths::PREFS_FILE;

/// Concurrent downloads when nothing is configured.
pub const DEFAULT_CONCURRENT_DOWNLOADS: u32 = 6;
/// Concurrent disk writes when nothing is configured.
pub const DEFAULT_CONCURRENT_WRITES: u32 = 6;
/// Heap floor, in MiB, for an instance that does not override memory.
///
/// The same number the game's own settings default to
/// ([`palantir_core::settings::defaults::MIN_MEM_ALLOC`]) rather than a second
/// opinion about it. The settings pane shows this value and a launch obeys it,
/// and two constants that drifted apart would make the pane wrong about the
/// machine — which is the state this launcher shipped in until the fields were
/// wired up.
pub const DEFAULT_MIN_MEM_MIB: u32 = palantir_core::settings::defaults::MIN_MEM_ALLOC as u32;
/// Heap ceiling, in MiB, for an instance that does not override memory.
pub const DEFAULT_MAX_MEM_MIB: u32 = palantir_core::settings::defaults::MAX_MEM_ALLOC as u32;
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
    /// [`palantir_net::DEFAULT_MICROSOFT_CLIENT_ID`]), and
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
    /// Heap floor a launch uses, in MiB, for an instance that does not override
    /// memory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_min_mem_mib: Option<u32>,
    /// Heap ceiling a launch uses, in MiB, for an instance that does not
    /// override memory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_max_mem_mib: Option<u32>,
    /// Heap floor taken from the other launcher's `prismlauncher.cfg`, when this
    /// file has no number of its own.
    ///
    /// Deliberately *not* one of the fields above: reading Prism's numbers must
    /// never write them into this launcher's file, so a machine that has never
    /// touched the memory pane keeps a preferences file that says nothing about
    /// memory. It exists because a Prism install's heap is a real setting, and
    /// adopting an install should not quietly launch it with different numbers.
    #[serde(skip)]
    adopted_min_mem_mib: Option<u32>,
    /// Heap ceiling taken from `prismlauncher.cfg` (see
    /// [`Prefs::adopted_min_mem_mib`]).
    #[serde(skip)]
    adopted_max_mem_mib: Option<u32>,
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
    /// The data root, when it has been moved off this product's own directory.
    ///
    /// This is the pointer [`palantir_core::paths::resolve_data_root`] reads, and
    /// it is the *only* place the location is stored: an install adopted from
    /// another launcher is recorded here as its existing path rather than copied
    /// into a new one, which is why adopting gigabytes of libraries and assets
    /// costs one line of JSON.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_directory: Option<String>,
    /// Whether the first-run question — use the install that is already on this
    /// machine, or start in this product's own folder — has been put to the user.
    ///
    /// Without this the question would come back on every start, and a dialog
    /// that reappears until it is obeyed is not a question. It is written when
    /// the dialog is *answered*, so a window closed without an answer asks again
    /// rather than silently choosing.
    #[serde(skip_serializing_if = "is_false")]
    pub data_root_asked: bool,
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
            adopted_min_mem_mib: None,
            adopted_max_mem_mib: None,
            default_java_path: None,
            max_concurrent_downloads: None,
            max_concurrent_writes: None,
            always_show_copy_details: false,
            app_directory: None,
            data_root_asked: false,
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

    /// The heap floor a launch uses: what was set here, else what was adopted
    /// from the other launcher's config, else the shipped number.
    ///
    /// One answer rather than three, because the settings pane shows this value
    /// and a launch passes it to the JVM: a resolution the pane could not see
    /// would be a pane that is wrong about the machine.
    pub fn min_mem_mib(&self) -> u32 {
        self.default_min_mem_mib
            .or(self.adopted_min_mem_mib)
            .unwrap_or(DEFAULT_MIN_MEM_MIB)
    }

    /// The heap ceiling a launch uses (see [`Prefs::min_mem_mib`]).
    pub fn max_mem_mib(&self) -> u32 {
        self.default_max_mem_mib
            .or(self.adopted_max_mem_mib)
            .unwrap_or(DEFAULT_MAX_MEM_MIB)
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
            palantir_net::DEFAULT_MICROSOFT_CLIENT_ID.to_string()
        } else {
            configured.to_string()
        }
    }

    /// Take the heap numbers from the other launcher's config, when this file
    /// has none of its own.
    ///
    /// A machine that already has the other launcher installed has a heap it was
    /// configured with, and this launcher shows and uses that rather than a
    /// shipped default no launch would have used: adopting the install that is
    /// already there is about the data, and memory is part of how that install
    /// behaves. Prism's file is only ever read, here and everywhere else.
    ///
    /// The numbers land in [`Prefs::adopted_min_mem_mib`] and its ceiling rather
    /// than in the file's own fields, so nothing read here can reach
    /// `palantirmc-desktop.json` on the next save: a machine that has never
    /// touched the memory pane keeps a preferences file that says nothing about
    /// memory, and a number typed into the pane still wins over Prism's.
    fn adopt_prism_memory(&mut self, home: &PalantirPaths) {
        if self.default_min_mem_mib.is_some() && self.default_max_mem_mib.is_some() {
            return;
        }
        let Ok(settings) = palantir_core::settings::Settings::load(&home.global_config()) else {
            return;
        };
        // A number in the file that is not a number — or is zero, which is not a
        // heap — is not a heap to adopt. Prism itself reads nonsense as zero, and
        // a zero-megabyte heap is a launch that fails in the JVM rather than a
        // setting anybody chose.
        let heap = |key: &str| {
            settings
                .map()
                .get(key)
                .and_then(|raw| raw.trim().parse::<u32>().ok())
                .filter(|mib| *mib > 0)
        };
        if self.default_min_mem_mib.is_none() {
            self.adopted_min_mem_mib = heap(PRISM_MIN_MEM_KEY);
        }
        if self.default_max_mem_mib.is_none() {
            self.adopted_max_mem_mib = heap(PRISM_MAX_MEM_KEY);
        }
    }
}

/// Where the preferences live: this product's own directory, not the data root.
pub fn path(home: &PalantirPaths) -> PathBuf {
    home.root.join(PREFS_FILE)
}

/// Read the preferences, falling back to the defaults.
///
/// A missing file is the normal first run; an unreadable or unparseable one is
/// treated the same way rather than being an error the user has to deal with.
/// The corrupt file is left in place — overwriting it would destroy whatever
/// the user was editing when it broke.
pub fn load(home: &PalantirPaths) -> Prefs {
    let mut prefs: Prefs = std::fs::read_to_string(path(home))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    prefs.adopt_prism_memory(home);
    prefs
}

/// Prism's key for the heap floor in `prismlauncher.cfg`.
pub const PRISM_MIN_MEM_KEY: &str = "MinMemAlloc";
/// Prism's key for the heap ceiling in `prismlauncher.cfg`.
pub const PRISM_MAX_MEM_KEY: &str = "MaxMemAlloc";

/// Write the preferences, atomically.
///
/// Atomic so a crash mid-write cannot leave a half-written file that the next
/// run reads as the defaults: the theme would silently reset.
pub fn save(home: &PalantirPaths, prefs: &Prefs) -> Result<(), String> {
    let text = serde_json::to_string_pretty(prefs).map_err(|error| error.to_string())?;
    let mut bytes = text.into_bytes();
    bytes.push(b'\n');
    palantir_core::util::atomic_write(&path(home), &bytes).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> (tempfile::TempDir, PalantirPaths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = PalantirPaths::at(dir.path());
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
        assert_eq!(prefs.microsoft_client_id(), palantir_net::DEFAULT_MICROSOFT_CLIENT_ID);
        let configured = Prefs { microsoft_client_id: "my-app-id".into(), ..Prefs::default() };
        assert_eq!(configured.microsoft_client_id(), "my-app-id");
        // Whitespace is not a client id.
        let blank = Prefs { microsoft_client_id: "   ".into(), ..Prefs::default() };
        assert_eq!(blank.microsoft_client_id(), palantir_net::DEFAULT_MICROSOFT_CLIENT_ID);
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

    /// A machine that already runs the other launcher has a heap it was set up
    /// with, and this one shows and uses those numbers rather than a shipped
    /// default no launch would have used.
    #[test]
    fn the_heap_the_other_launcher_has_is_adopted_rather_than_copied() {
        let (_dir, paths) = root();
        let prism = "[General]\nMinMemAlloc=1024\nMaxMemAlloc=8192\n";
        std::fs::write(paths.global_config(), prism).unwrap();
        let prefs = load(&paths);
        assert_eq!(prefs.min_mem_mib(), 1024);
        assert_eq!(prefs.max_mem_mib(), 8192);

        // Reading them must not turn them into this launcher's own settings: a
        // machine that has never touched the memory pane keeps a preferences
        // file that says nothing about memory. And Prism's file is written by
        // nobody, here as everywhere else.
        save(&paths, &prefs).unwrap();
        let written = std::fs::read_to_string(path(&paths)).unwrap();
        assert!(!written.contains("mem_mib"), "the adopted numbers were copied: {written}");
        assert_eq!(std::fs::read_to_string(paths.global_config()).unwrap(), prism);
    }

    #[test]
    fn a_number_set_here_beats_the_one_the_other_launcher_has() {
        let (_dir, paths) = root();
        std::fs::write(
            paths.global_config(),
            "[General]\nMinMemAlloc=1024\nMaxMemAlloc=8192\n",
        )
        .unwrap();
        save(
            &paths,
            &Prefs { default_max_mem_mib: Some(2048), ..Prefs::default() },
        )
        .unwrap();
        let prefs = load(&paths);
        assert_eq!(prefs.max_mem_mib(), 2048, "the number typed here is the one a launch uses");
        assert_eq!(prefs.min_mem_mib(), 1024, "the one not set here still comes from Prism");
    }

    #[test]
    fn a_heap_that_is_not_a_heap_is_not_adopted() {
        // Zero megabytes is not a heap, and a typo is not a number: adopting
        // either would turn a readable global config into a launch that dies in
        // the JVM instead of one that uses the shipped numbers.
        let (_dir, paths) = root();
        std::fs::write(
            paths.global_config(),
            "[General]\nMinMemAlloc=0\nMaxMemAlloc=  \n",
        )
        .unwrap();
        let prefs = load(&paths);
        assert_eq!(prefs.min_mem_mib(), DEFAULT_MIN_MEM_MIB);
        assert_eq!(prefs.max_mem_mib(), DEFAULT_MAX_MEM_MIB);
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
            // Nothing was adopted from anywhere: this file is the whole story.
            adopted_min_mem_mib: None,
            adopted_max_mem_mib: None,
            default_java_path: Some("C:/jdk21/bin/javaw.exe".into()),
            max_concurrent_downloads: Some(3),
            max_concurrent_writes: Some(2),
            always_show_copy_details: true,
            app_directory: Some("D:/minecraft".into()),
            data_root_asked: true,
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
    fn the_pointer_the_shell_writes_is_the_one_core_reads() {
        // Two crates, one key. The shell writes `app_directory` through serde and
        // core reads it by hand, so nothing but this test connects the two — and
        // a rename on either side would otherwise leave every start resolving
        // this product's folder while the settings pane showed something else.
        let (_dir, paths) = root();
        assert_eq!(palantir_core::paths::recorded_data_root(&paths), None);
        let prefs = Prefs { app_directory: Some("D:/minecraft".into()), ..Prefs::default() };
        save(&paths, &prefs).unwrap();
        assert_eq!(
            palantir_core::paths::recorded_data_root(&paths).as_deref(),
            Some("D:/minecraft")
        );
        assert_eq!(
            palantir_core::paths::resolve_data_root(&paths, Some("D:/minecraft")),
            std::path::PathBuf::from("D:/minecraft")
        );
    }

    #[test]
    fn answering_the_first_run_question_survives_a_restart() {
        // A question that comes back on every start is not a question. A window
        // closed *without* answering leaves it unasked, so the next start asks
        // again rather than silently deciding.
        let (_dir, paths) = root();
        assert!(!load(&paths).data_root_asked, "a fresh install has not been asked");
        let answered = Prefs { data_root_asked: true, ..Prefs::default() };
        save(&paths, &answered).unwrap();
        assert!(load(&paths).data_root_asked);
        let text = std::fs::read_to_string(path(&paths)).unwrap();
        assert!(text.contains("data_root_asked"), "got: {text}");
    }

    #[test]
    fn declining_the_question_is_not_recorded_as_a_pointer_to_somewhere() {
        // "Start in this product's own folder" is the *absence* of a pointer,
        // and an absent pointer has to mean the default rather than an empty
        // path that `resolve_data_root` would then have to guess about.
        let (_dir, paths) = root();
        let declined = Prefs { data_root_asked: true, ..Prefs::default() };
        save(&paths, &declined).unwrap();
        let back = load(&paths);
        assert_eq!(back.app_directory, None);
        assert_eq!(palantir_core::paths::recorded_data_root(&paths), None);
        assert_eq!(palantir_core::paths::resolve_data_root(&paths, None), paths.root);
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
