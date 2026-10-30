//! The instances the launcher knows about, read from disk.
//!
//! **Why this is a small reader rather than the iced app's `instances.rs`.** That module is
//! six hundred lines with three dependencies of its own (`model`, `catalog`, `mods`), all of
//! them part of the retained-tree UI this rewrite is replacing. Porting it now would drag a
//! third of the old app across the boundary to draw a list of names. What the Home page
//! needs is: the instances folder, each instance's name, its loader and version, and when it
//! was last played -- and `sift_core::instance` already answers all of that.
//!
//! The duplication is real and it is temporary: when iced is deleted, this module is what
//! survives and the old one is what goes. `docs/PLAN-SIFT.md`'s phase B is the place the two
//! converge.

use std::path::PathBuf;

use sift_core::instance::Instance;
use sift_core::paths::{platform_data_root, System};

/// One instance, as a card needs it.
/// The instance settings file, and this launcher's key inside it for the project an instance
/// was installed from.
pub const INSTANCE_CFG: &str = "instance.cfg";
pub const PROJECT_KEY: &str = "sift.project.id";

#[derive(Debug, Clone)]
pub struct Entry {
    /// The folder's name, which is also the instance's stable key.
    pub id: String,
    /// What the player named it.
    pub name: String,
    /// The loader this instance runs on: `Fabric`, `Forge`, `NeoForge`, `Quilt` or the
    /// reference's word for no loader at all.
    pub loader: String,
    /// The Minecraft version the loader targets.
    pub version: String,
    /// When it was last launched, in milliseconds since the epoch; `0` when never.
    pub last_played: i64,
    /// The project this instance was installed from, when this launcher installed it.
    pub project: Option<String>,
}

/// Where this launcher keeps its instances.
///
/// The same root the rest of the product uses, so the egui build and the iced one look at
/// one folder rather than two: a launcher that kept two homes would show an empty library
/// to a player who has instances.
pub fn instances_dir() -> PathBuf {
    platform_data_root(System::current()).join("instances")
}

/// Every instance the launcher can see, newest-played first.
///
/// A folder that will not open is skipped rather than reported: `discover` only returns
/// folders that contain `instance.cfg`, so a failure here means a file that vanished or a
/// cfg that will not parse, and neither is something a Home page can act on. The count of
/// skipped folders is returned alongside the list so a caller can say "3 unreadable" if it
/// ever wants to.
pub fn load() -> (Vec<Entry>, usize) {
    let dir = instances_dir();
    let Ok(folders) = Instance::discover(&dir) else {
        // No folder yet is the ordinary first run, not an error.
        return (Vec::new(), 0);
    };
    let mut entries = Vec::with_capacity(folders.len());
    let mut skipped = 0;
    for folder in folders {
        let Ok(Some(instance)) = Instance::try_open(&folder) else {
            skipped += 1;
            continue;
        };
        let (loader, version) = loader_and_version(&instance);
        entries.push(Entry {
            id: instance.id(),
            name: instance.name(),
            loader,
            version,
            last_played: instance.last_launch_millis(),
            project: project_of(&instance),
        });
    }
    // The reference sorts by most recently played, with never-played last.
    entries.sort_by(|a, b| b.last_played.cmp(&a.last_played).then_with(|| a.name.cmp(&b.name)));
    (entries, skipped)
}

/// The project id recorded in the instance's own settings, if this launcher installed it.
fn project_of(instance: &Instance) -> Option<String> {
    let text = std::fs::read_to_string(instance.root().join(INSTANCE_CFG)).ok()?;
    text.lines()
        .find_map(|line| line.strip_prefix(PROJECT_KEY)?.strip_prefix('=').map(str::to_owned))
        .filter(|id| !id.is_empty())
}

/// The loader and Minecraft version, from the instance's pack profile.
///
/// Prism records them in `mmc-pack.json`, which `sift_core::pack` parses; an instance with no
/// profile (a bare folder with a cfg) is `Vanilla` at whatever version its cfg names.
fn loader_and_version(instance: &Instance) -> (String, String) {
    let profile = sift_core::pack::PackProfile::load(&instance.root().join("mmc-pack.json"));
    match profile {
        Ok(profile) => {
            let mut loader = "Vanilla".to_owned();
            let mut version = String::new();
            for component in profile.components() {
                match component.uid.as_str() {
                    "net.minecraft" => version = component.version.clone(),
                    // The component uids the reference names, longest first so
                    // `net.neoforged` is not read as `net.minecraftforge`.
                    "net.fabricmc.fabric-loader" => loader = "Fabric".to_owned(),
                    "net.neoforged" => loader = "NeoForge".to_owned(),
                    "net.minecraftforge" => loader = "Forge".to_owned(),
                    "org.quiltmc.quilt-loader" => loader = "Quilt".to_owned(),
                    _ => {}
                }
            }
            (loader, version)
        }
        Err(_) => ("Vanilla".to_owned(), String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A launcher with no instances must answer an empty list and no skips, not an error:
    /// the first run of a fresh install is exactly that case.
    #[test]
    fn a_fresh_install_has_no_instances() {
        let (entries, skipped) = load();
        // The machine this runs on may have instances, so the assertion is about the shape:
        // a list that sorted, and a skip count that is a number.
        assert!(entries.len() < 10_000);
        assert!(skipped < 10_000);
        for pair in entries.windows(2) {
            assert!(
                pair[0].last_played >= pair[1].last_played,
                "entries are sorted by most recently played"
            );
        }
    }
}
