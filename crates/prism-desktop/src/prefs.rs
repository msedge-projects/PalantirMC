//! The launcher's own preferences, as opposed to Prism's.
//!
//! Prism keeps *launcher* settings in `<root>/prismlauncher.cfg`, and that file
//! is not ours to rewrite: a Prism user's heap sizes, Java paths and instance
//! directory live in it, and a typo written by a second program would break
//! their launcher. So the settings that belong to *this* product go in their own
//! file next to it, written atomically and left alone if it is unparseable.
//!
//! Only what the shell actually offers is stored. Today that is the color theme;
//! a preference nobody can set is a schema waiting to be wrong.

use std::path::PathBuf;

use prism_core::paths::PrismPaths;
use serde::{Deserialize, Serialize};

use crate::theme::ColorTheme;

/// The preferences file, under the data root.
pub const PREFS_FILE: &str = "palantirmc-desktop.json";

/// Everything the shell remembers between runs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// The chosen color theme, by [`ColorTheme::id`].
    ///
    /// Stored as the id string rather than the enum so a file written by a
    /// newer build — or hand-edited — degrades to the default theme instead of
    /// failing the whole parse and losing the rest of the file.
    pub color_theme: String,
}

impl Prefs {
    /// The theme this file selects, defaulting when it names nothing known.
    pub fn theme(&self) -> ColorTheme {
        ColorTheme::from_id(&self.color_theme)
    }

    /// The file for a given choice.
    pub fn with_theme(theme: ColorTheme) -> Prefs {
        Prefs { color_theme: theme.id().to_string() }
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

/// Remember one theme choice, reporting whether it reached the disk.
pub fn save_theme(paths: &PrismPaths, theme: ColorTheme) -> Result<(), String> {
    save(paths, &Prefs::with_theme(theme))
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
            save_theme(&paths, theme).unwrap();
            assert_eq!(load(&paths).theme(), theme, "{} did not survive", theme.id());
        }
    }

    #[test]
    fn the_file_is_json_with_only_what_the_shell_offers() {
        let (_dir, paths) = root();
        save_theme(&paths, ColorTheme::Oled).unwrap();
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
    fn the_file_is_not_prisms_config() {
        // Writing this launcher's settings must never touch the file Prism owns.
        let (_dir, paths) = root();
        let prism = paths.global_config();
        std::fs::write(&prism, b"[General]\nMaxMemAlloc=4096\n").unwrap();
        save_theme(&paths, ColorTheme::Light).unwrap();
        assert_eq!(
            std::fs::read_to_string(&prism).unwrap(),
            "[General]\nMaxMemAlloc=4096\n",
            "prismlauncher.cfg must be left exactly as it was"
        );
        assert_ne!(path(&paths), prism);
    }
}
