//! Mod-folder helpers with Prism's `.disabled` suffix semantics.
//!
//! In Prism a disabled mod is the same file renamed from `name.jar` to
//! `name.jar.disabled`. This module owns the pure filename mapping plus the
//! small directory listing/rename helpers the Mods page paints.

use std::path::Path;

/// Suffix Prism appends to disable a mod file.
pub const DISABLED_SUFFIX: &str = ".disabled";

/// One row of the Mods page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModEntry {
    /// On-disk file name (may end in [`DISABLED_SUFFIX`]).
    pub file_name: String,
    /// Display name (suffix stripped).
    pub display_name: String,
    /// Whether the mod is currently enabled.
    pub enabled: bool,
}

/// Split a file name into `(display_name, enabled)`.
///
/// Returns `None` for names that are not mod files (`*.jar` enabled,
/// `*.jar.disabled` disabled).
pub fn split_mod_name(file_name: &str) -> Option<(String, bool)> {
    if file_name.is_empty() {
        return None;
    }
    if let Some(base) = file_name.strip_suffix(DISABLED_SUFFIX) {
        if !base.is_empty() && base.ends_with(".jar") {
            return Some((base.to_string(), false));
        }
        return None;
    }
    if file_name.ends_with(".jar") {
        return Some((file_name.to_string(), true));
    }
    None
}

/// Target on-disk name for `current` when it should be `enabled`.
///
/// Returns `None` when `current` is not a mod file. When the file is already
/// in the requested state the returned name equals `current` (the caller
/// skips the rename in that case).
pub fn toggled_file_name(current: &str, enabled: bool) -> Option<String> {
    match split_mod_name(current) {
        Some((base, _)) if enabled => Some(base),
        Some((base, _)) => {
            let mut out = base;
            out.push_str(DISABLED_SUFFIX);
            Some(out)
        }
        None => None,
    }
}

/// List the mods in `mods_dir`, sorted case-insensitively by display name.
/// A missing/unreadable directory yields an empty list.
pub fn list_mods(mods_dir: &Path) -> Vec<ModEntry> {
    let mut out: Vec<ModEntry> = Vec::new();
    let entries = match std::fs::read_dir(mods_dir) {
        Ok(entries) => entries,
        Err(_) => return out,
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let is_file = match entry.file_type() {
            Ok(kind) => kind.is_file(),
            Err(_) => continue,
        };
        if !is_file {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some((display_name, enabled)) = split_mod_name(&name) {
            out.push(ModEntry { file_name: name, display_name, enabled });
        }
    }
    // Fold each name once, not twice per comparison in an O(n log n) sort.
    out.sort_by_cached_key(|entry| (entry.display_name.to_lowercase(), entry.file_name.clone()));
    out
}

/// Rename a mod file to enable/disable it. A no-op when the file is already
/// in the requested state.
pub fn set_mod_enabled(mods_dir: &Path, file_name: &str, enabled: bool) -> Result<(), String> {
    if file_name.contains('/') || file_name.contains('\\') {
        return Err(format!("refusing to touch path-like mod name '{file_name}'"));
    }
    let target = match toggled_file_name(file_name, enabled) {
        Some(target) => target,
        None => return Err(format!("not a mod file: '{file_name}'")),
    };
    if target.contains('/') || target.contains('\\') {
        return Err(format!("refusing to touch path-like mod name '{target}'"));
    }
    if target == file_name {
        return Ok(());
    }
    let source = mods_dir.join(file_name);
    let destination = mods_dir.join(&target);
    // A rename may replace another jar. A hard link reserves the new name
    // atomically without copying jar bytes or overwriting an existing entry.
    if std::fs::hard_link(&source, &destination).is_err() {
        // Portable installs may live on FAT/exFAT, which cannot hard-link.
        // Reserve the name before a bounded-memory copy on those filesystems.
        let mut input = std::fs::File::open(&source)
            .map_err(|error| format!("reading mod '{file_name}': {error}"))?;
        let mut output = std::fs::OpenOptions::new().write(true).create_new(true)
            .open(&destination)
            .map_err(|error| format!("renaming mod '{file_name}' to '{target}': {error}"))?;
        let copied = std::io::copy(&mut input, &mut output)
            .and_then(|_| output.sync_all());
        drop(output);
        if let Err(error) = copied {
            let _ = std::fs::remove_file(&destination);
            return Err(format!("copying mod '{file_name}': {error}"));
        }
    }
    if let Err(error) = std::fs::remove_file(&source) {
        let _ = std::fs::remove_file(&destination);
        return Err(format!("renaming mod '{file_name}': {error}"));
    }
    Ok(())
}

/// Plain file-name listing for simple content folders (`resourcepacks/`,
/// `shaderpacks/`, `saves/`): every entry (files and directories),
/// sorted case-insensitively. Missing/unreadable directories yield `[]`.
#[cfg(test)]
pub fn list_content_names(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return out,
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        out.push(entry.file_name().to_string_lossy().into_owned());
    }
    out.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_names_recognizes_jar_and_disabled() {
        assert_eq!(split_mod_name("sodium.jar"), Some(("sodium.jar".to_string(), true)));
        assert_eq!(
            split_mod_name("sodium.jar.disabled"),
            Some(("sodium.jar".to_string(), false))
        );
        assert_eq!(split_mod_name("readme.txt"), None);
        assert_eq!(split_mod_name("weird.disabled"), None);
        assert_eq!(split_mod_name(""), None);
        // Degenerate but well-formed: the base is still a `.jar` name.
        assert_eq!(split_mod_name(".jar.disabled"), Some((".jar".to_string(), false)));
        assert_eq!(split_mod_name("nested.jar.zip"), None);
    }

    #[test]
    fn toggle_mapping_round_trips() {
        assert_eq!(toggled_file_name("a.jar", false).as_deref(), Some("a.jar.disabled"));
        assert_eq!(toggled_file_name("a.jar.disabled", true).as_deref(), Some("a.jar"));
        // Already in the requested state: identity (caller skips rename).
        assert_eq!(toggled_file_name("a.jar", true).as_deref(), Some("a.jar"));
        assert_eq!(toggled_file_name("a.jar.disabled", false).as_deref(), Some("a.jar.disabled"));
        assert_eq!(toggled_file_name("notes.txt", true), None);
        // Double-disable does not stack the suffix.
        let once = toggled_file_name("a.jar", false).unwrap_or_default();
        assert_eq!(toggled_file_name(&once, false).as_deref(), Some("a.jar.disabled"));
    }

    #[test]
    fn list_and_toggle_round_trip_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        assert!(std::fs::create_dir_all(&mods).is_ok());
        assert!(std::fs::write(mods.join("b.jar"), b"fake").is_ok());
        assert!(std::fs::write(mods.join("a.jar.disabled"), b"fake").is_ok());
        assert!(std::fs::write(mods.join("notes.txt"), b"nope").is_ok());
        assert!(std::fs::create_dir_all(mods.join("subdir.jar")).is_ok());

        let listed = list_mods(&mods);
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].display_name, "a.jar");
        assert!(!listed[0].enabled);
        assert_eq!(listed[1].display_name, "b.jar");
        assert!(listed[1].enabled);

        assert!(set_mod_enabled(&mods, "b.jar", false).is_ok());
        assert!(!mods.join("b.jar").exists());
        assert!(mods.join("b.jar.disabled").is_file());
        assert!(set_mod_enabled(&mods, "b.jar.disabled", true).is_ok());
        assert!(mods.join("b.jar").is_file());

        // No-op rename and error cases.
        assert!(set_mod_enabled(&mods, "b.jar", true).is_ok());
        assert!(set_mod_enabled(&mods, "notes.txt", true).is_err());
        assert!(set_mod_enabled(&mods, "../evil.jar", true).is_err());

        // Missing dir lists as empty.
        assert!(list_mods(&dir.path().join("nope")).is_empty());
        assert!(list_content_names(&dir.path().join("nope")).is_empty());
    }

    #[test]
    fn toggling_never_overwrites_an_existing_mod() {
        let dir = tempfile::tempdir().unwrap();
        for enabled in [false, true] {
            std::fs::write(dir.path().join("a.jar"), b"enabled version").unwrap();
            std::fs::write(dir.path().join("a.jar.disabled"), b"disabled version").unwrap();
            let source = if enabled { "a.jar.disabled" } else { "a.jar" };
            assert!(set_mod_enabled(dir.path(), source, enabled).is_err());
            assert_eq!(std::fs::read(dir.path().join("a.jar")).unwrap(), b"enabled version");
            assert_eq!(std::fs::read(dir.path().join("a.jar.disabled")).unwrap(), b"disabled version");
        }
    }

    #[test]
    fn content_names_sorted_case_insensitively() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["zeta.zip", "Alpha", "beta.zip"] {
            assert!(std::fs::write(dir.path().join(name), b"x").is_ok());
        }
        assert_eq!(list_content_names(dir.path()), vec!["Alpha", "beta.zip", "zeta.zip"]);
    }
}
