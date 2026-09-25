//! Where a page's data comes from.
//!
//! Stage 3 draws the pages; stage 4 builds the engine that answers them from the
//! network. In between there is a choice of two dishonesties -- pages that draw
//! invented data, or pages that spin forever -- and this module is the way out of
//! both: **the store answers what the launcher can already answer, and says out
//! loud what it cannot.**
//!
//! What it can answer is the launcher's own filesystem, which is ours rather than
//! the reference's:
//!
//! * the instance list and each instance's loader, game version, playtime and mod
//!   counts, from the same readers the old interface uses ([`crate::instances`]);
//! * an instance's own folders -- its mods, worlds, files and screenshots -- which
//!   are directory reads;
//! * the tail of an instance's newest log, which is a file read.
//!
//! What it cannot answer is everything that comes from a service: project pages,
//! Discover's search, Skins, Servers and the hosting half of an instance. Those
//! come back as [`page::Load::Failed`] carrying [`unavailable`]'s sentence, which
//! names the stage that brings them rather than pretending to be an empty list. A
//! page that shows "no results" when the request never happened is the failure mode
//! this module exists to prevent.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use palantir_core::paths::PalantirPaths;

use crate::instances::{self, InstanceCard};
use crate::mods::{self, ModEntry};
use crate::page::Load;

/// What the interface knows, and how it came to know it.
#[derive(Debug, Clone, Default)]
pub struct Store {
    /// The launcher's instances, read from disk.
    instances: Load<Vec<InstanceCard>>,
    /// Where they live, so an instance's own folders can be read.
    instances_dir: PathBuf,
}

impl Store {
    /// Read the launcher's instances.
    ///
    /// A synchronous scan at startup, which is what the old interface does too
    /// (`main.rs` says so): it is a directory walk and a handful of small files per
    /// instance, and making it asynchronous before the engine exists would be work
    /// that stage 4 removes.
    pub fn load(paths: &PalantirPaths) -> Store {
        let loaded = instances::load(paths);
        Store {
            instances: if loaded.cards.is_empty() {
                Load::Empty
            } else {
                Load::Ready(loaded.cards)
            },
            instances_dir: loaded.instances_dir,
        }
    }

    /// The instance list, in whatever state it is in.
    pub fn instances(&self) -> &Load<Vec<InstanceCard>> {
        &self.instances
    }

    /// One instance, by id.
    pub fn instance(&self, id: &str) -> Load<InstanceCard> {
        match &self.instances {
            Load::Ready(cards) => match cards.iter().find(|card| card.id == id) {
                Some(card) => Load::Ready(card.clone()),
                // Not an error: an address can name an instance that is gone, and
                // the reference draws "not found" rather than "something broke".
                None => Load::Empty,
            },
            Load::Empty => Load::Empty,
            Load::Failed(reason) => Load::Failed(reason.clone()),
            // An instance page can be reached before the scan finishes.
            Load::Idle | Load::Loading => Load::Loading,
        }
    }

    /// Where an instance's own files are.
    pub fn instance_dir(&self, id: &str) -> PathBuf {
        self.instances_dir.join(id)
    }

    /// The instances directory itself, for the reader that has no id yet.
    pub fn instances_dir(&self) -> &Path {
        &self.instances_dir
    }

    /// The reason a page's data is not here yet, naming the stage that brings it.
    pub fn unavailable(&self, what: &str) -> String {
        unavailable(what)
    }
}

/// The sentence a page shows when its data has to come from a service.
///
/// Deliberately plain about where the data is, and deliberately not shaped like an
/// error: it is the truth about a stage boundary, and the moment stage 4 lands it
/// disappears on its own.
pub fn unavailable(what: &str) -> String {
    format!("{what} arrives with the metadata engine (stage 4 of the rewrite).")
}

// ---- The instance's own folders -----------------------------------------

/// One world in `saves/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct World {
    /// Folder name, which is the world's name to Minecraft.
    pub name: String,
    /// Whether the world has been opened since it was created.
    pub played: bool,
    /// Seconds since the world folder was last written, or `None` if the clock
    /// could not be read.
    pub modified: Option<u64>,
}

/// One file in an instance's directory, for the Files tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Name as it is on disk.
    pub name: String,
    /// A directory rather than a file.
    pub directory: bool,
    /// Size in bytes, zero for a directory.
    pub bytes: u64,
}

/// The instances' mods, from the same reader the old interface uses.
pub fn content(instance_dir: &Path) -> Vec<ModEntry> {
    mods::list_mods(&instance_dir.join("mods"))
}

/// The worlds in `saves/`, name-sorted.
///
/// A world that has a `level.dat` has been opened at least once: that file is
/// written when the world is saved, and a folder with only `region/` in it is one
/// Minecraft created and never saved -- which is why `played` exists rather than
/// being guessed from the folder's contents.
pub fn worlds(instance_dir: &Path) -> Vec<World> {
    let mut worlds = Vec::new();
    let Ok(entries) = std::fs::read_dir(instance_dir.join("saves")) else {
        return worlds;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let modified = std::fs::metadata(path.join("level.dat"))
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|since| since.as_secs());
        worlds.push(World { name, played: modified.is_some(), modified });
    }
    worlds.sort_by(|left, right| left.name.cmp(&right.name));
    worlds
}

/// One level of an instance's directory, directories first then files, each
/// name-sorted.
pub fn files(directory: &Path) -> Vec<Entry> {
    let mut entries = Vec::new();
    let Ok(read) = std::fs::read_dir(directory) else {
        return entries;
    };
    for entry in read.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let directory = path.is_dir();
        let bytes = if directory {
            0
        } else {
            std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0)
        };
        entries.push(Entry { name, directory, bytes });
    }
    entries.sort_by(|left, right| {
        right.directory.cmp(&left.directory).then_with(|| left.name.cmp(&right.name))
    });
    entries
}

/// The screenshots in `screenshots/`, newest name first.
///
/// Names rather than pixels: decoding is the engine's image cache (stage 4), and a
/// page that decoded a directory of 1080p PNGs on every frame would be worse than
/// one that lists them.
pub fn screenshots(instance_dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = files(&instance_dir.join("screenshots"))
        .into_iter()
        .filter(|entry| !entry.directory)
        .filter(|entry| {
            let extension = entry.name.rsplit('.').next().unwrap_or_default().to_ascii_lowercase();
            matches!(extension.as_str(), "png" | "jpg" | "jpeg")
        })
        .map(|entry| entry.name)
        .collect();
    names.sort();
    names.reverse();
    names
}

/// The tail of an instance's newest log, newest line last.
///
/// `logs/latest.log` is what both Mojang's launcher and Prism write, and it is the
/// file the reference's Logs tab reads. The tail rather than the whole file: a log
/// that has been appended to for a year is megabytes, and a console shows the end
/// of it.
pub fn log_tail(instance_dir: &Path, lines: usize) -> Option<String> {
    let text = std::fs::read_to_string(instance_dir.join("logs").join("latest.log")).ok()?;
    let all: Vec<&str> = text.lines().collect();
    let start = all.len().saturating_sub(lines);
    Some(all[start..].join("\n"))
}

/// A byte count as the reference's `formatBytes` writes it.
///
/// Binary units with the `KiB`/`MiB`/`GiB` labels the reference's own strings use
/// (`format.bytes.0`..`format.bytes.4`), one decimal place past the first unit.
pub fn bytes_label(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join("palantirmc-store-tests").join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a scratch directory");
        root
    }

    #[test]
    fn an_unanswered_page_says_which_stage_answers_it() {
        // The sentence a reader sees has to be true: it names stage 4, and it does
        // not read like a failure of their machine.
        let reason = unavailable("Discover's search");
        assert!(reason.contains("stage 4"), "{reason}");
        assert!(reason.starts_with("Discover's search"), "{reason}");
        assert!(!reason.to_lowercase().contains("error"), "{reason}");
    }

    #[test]
    fn a_store_with_no_instances_says_empty_rather_than_nothing_happened() {
        // `Empty` and `Ready(vec![])` are different answers, and the pages draw
        // them differently: one is "you have none yet", the other is a list.
        let paths = PalantirPaths::at(scratch("empty-store"));
        let store = Store::load(&paths);
        assert_eq!(store.instances(), &Load::Empty);
        // And an id that is not there is empty rather than a failure.
        assert_eq!(store.instance("nothing"), Load::Empty);
        assert_eq!(store.instance_dir("nothing"), store.instances_dir().join("nothing"));
    }

    #[test]
    fn the_worlds_are_read_from_saves_and_know_whether_they_were_played() {
        let dir = scratch("worlds");
        std::fs::create_dir_all(dir.join("saves").join("Played")).expect("a world");
        std::fs::write(dir.join("saves").join("Played").join("level.dat"), b"x").expect("level.dat");
        std::fs::create_dir_all(dir.join("saves").join("Never saved")).expect("a folder");
        std::fs::write(dir.join("saves").join("loose.txt"), b"x").expect("a loose file");
        let found = worlds(&dir);
        assert_eq!(found.len(), 2, "a loose file is not a world");
        assert_eq!(found[0].name, "Never saved");
        assert!(!found[0].played, "a folder with no level.dat was never saved");
        assert!(found[1].played);
        assert!(found[1].modified.is_some());
        // A directory that does not exist is no worlds, not a panic.
        assert!(worlds(&dir.join("nowhere")).is_empty());
    }

    #[test]
    fn the_file_list_puts_directories_first_then_names() {
        let dir = scratch("files");
        std::fs::create_dir_all(dir.join("mods")).expect("a directory");
        std::fs::write(dir.join("options.txt"), b"hello").expect("a file");
        std::fs::write(dir.join("instance.cfg"), b"x").expect("a file");
        let entries = files(&dir);
        assert_eq!(entries[0].name, "mods");
        assert!(entries[0].directory);
        assert_eq!(entries[0].bytes, 0);
        assert_eq!(
            entries.iter().map(|entry| entry.name.as_str()).collect::<Vec<_>>(),
            vec!["mods", "instance.cfg", "options.txt"]
        );
        assert_eq!(entries[2].bytes, 5);
        assert!(files(&dir.join("nowhere")).is_empty());
    }

    #[test]
    fn only_images_are_screenshots_and_the_newest_come_first() {
        let dir = scratch("shots");
        let shots = dir.join("screenshots");
        std::fs::create_dir_all(&shots).expect("the folder");
        for name in ["2026-01-01_12.00.00.png", "2026-02-02_12.00.00.png", "notes.txt"] {
            std::fs::write(shots.join(name), b"x").expect("a file");
        }
        let names = screenshots(&dir);
        assert_eq!(names, vec!["2026-02-02_12.00.00.png", "2026-01-01_12.00.00.png"]);
        assert!(screenshots(&dir.join("nowhere")).is_empty());
    }

    #[test]
    fn a_log_is_read_from_its_end() {
        let dir = scratch("logs");
        std::fs::create_dir_all(dir.join("logs")).expect("the folder");
        let lines: Vec<String> = (0..100).map(|index| format!("line {index}")).collect();
        std::fs::write(dir.join("logs").join("latest.log"), lines.join("\n")).expect("a log");
        let tail = log_tail(&dir, 3).expect("a tail");
        assert_eq!(tail, "line 97\nline 98\nline 99");
        // A short log is the whole log, and a missing one is nothing rather than a
        // failure.
        let short = log_tail(&dir, 500).expect("a tail");
        assert!(short.starts_with("line 0"));
        assert_eq!(log_tail(&dir.join("nowhere"), 10), None);
    }

    #[test]
    fn a_byte_count_uses_the_reference_s_own_units() {
        assert_eq!(bytes_label(0), "0 B");
        assert_eq!(bytes_label(1023), "1023 B");
        assert_eq!(bytes_label(1024), "1.0 KiB");
        assert_eq!(bytes_label(1536), "1.5 KiB");
        assert_eq!(bytes_label(1024 * 1024), "1.0 MiB");
        assert_eq!(bytes_label(3 * 1024 * 1024 * 1024), "3.0 GiB");
        assert_eq!(bytes_label(5 * 1024_u64.pow(4)), "5.0 TiB");
        // Past the last unit it stays in it rather than inventing one.
        assert!(bytes_label(u64::MAX).ends_with("TiB"));
    }
}
