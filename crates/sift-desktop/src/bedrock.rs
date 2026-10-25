//! Bedrock: what this launcher may do with the edition it did not install.
//!
//! Bedrock on Windows is a Microsoft Store package. It is licensed per Microsoft
//! account, installed by the Store into `WindowsApps`, and there is no supported
//! way for another program to fetch or patch it -- Microsoft's Store terms and
//! the Minecraft EULA both put the game's own bytes out of reach, and a launcher
//! that downloaded them would be redistributing the game rather than launching
//! it.
//!
//! So this module is deliberately *not* a second install pipeline. It answers
//! three questions a Bedrock page legitimately has:
//!
//! * Is a copy installed? (`%LOCALAPPDATA%\Packages\<family>\LocalState` exists
//!   only once the Store has installed the game for this user.)
//! * What worlds does the player already have, and what packs are applied?
//! * What should the page say to someone who does not have it yet? A Store link,
//!   never a download.
//!
//! Everything here reads the player's own data folder. Nothing writes to the
//! package, nothing edits a world, and nothing is fetched from anywhere.
//!
//! ## Why the edition split is in the engine and not in the page
//!
//! The same reasoning `crates/sift-core` uses for its data root: the layout
//! is a fact about Windows, and a page that hard-coded it would be a page that
//! has to be found and corrected the day the folder moves. The tests below build
//! a data folder by hand, so the layout is asserted rather than assumed.

use std::path::{Path, PathBuf};

/// The Store package family name of Minecraft Bedrock for Windows.
///
/// The `LocalState` folder is the only part of the package a normal program may
/// read, and this is the name it hangs under.
pub const PACKAGE_FAMILY: &str = "Microsoft.MinecraftUWP_8wekyb3d8bbwe";

/// Where the Store page for Bedrock lives.
///
/// A deep link rather than a download URL: the Store does the licensing, and a
/// page that offers anything else is offering a redistributed copy.
pub const STORE_URL: &str = "ms-windows-store://pdp/?ProductId=9NBLGGH2JHXJ";

/// The relative path from `LocalState` to the folder the game calls `.minecraft`.
///
/// Bedrock keeps its worlds and packs under `games/com.mojang`, which is this
/// edition's answer to Java's `.minecraft`.
const GAME_DIR: &str = "games/com.mojang";

/// What this machine has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Presence {
    /// Installed for this user, with the game folder that was found.
    Installed {
        /// The `games/com.mojang` folder.
        root: PathBuf,
    },
    /// Windows, but no copy for this user.
    ///
    /// Distinct from [`Presence::Unsupported`] because the page's answer differs:
    /// "install it from the Store" versus "Bedrock is Windows-only".
    NotInstalled,
    /// Not Windows.
    Unsupported,
    /// Windows, but the launcher could not say where the user's data lives.
    Unknown {
        /// Why, in a sentence the page can show.
        reason: String,
    },
}

impl Presence {
    /// The game folder, when there is one.
    pub fn root(&self) -> Option<&Path> {
        match self {
            Presence::Installed { root } => Some(root),
            _ => None,
        }
    }

    /// Whether a copy the player already owns was found.
    pub fn is_installed(&self) -> bool {
        matches!(self, Presence::Installed { .. })
    }
}

/// Ask the machine where Bedrock's data folder is.
///
/// Only the *data* folder is looked for, and only for this user: a copy
/// installed for somebody else on the same machine is not this user's to launch,
/// which is why `%ProgramFiles%\WindowsApps` is never consulted.
pub fn presence() -> Presence {
    if !cfg!(windows) {
        return Presence::Unsupported;
    }
    match local_app_data() {
        Some(local) => presence_in(&local),
        None => Presence::Unknown {
            reason: String::from("LOCALAPPDATA is not set, so the game folder cannot be located"),
        },
    }
}

/// [`presence`] against a `%LOCALAPPDATA%` that the caller supplies.
///
/// The seam the tests use: the decision is "does this folder exist", and it is
/// answered without the process's own environment.
pub fn presence_in(local_app_data: &Path) -> Presence {
    let local_state = local_app_data
        .join("Packages")
        .join(PACKAGE_FAMILY)
        .join("LocalState");
    if !local_state.is_dir() {
        return Presence::NotInstalled;
    }
    // The package's folder exists, so the Store installed the game for this
    // user. The game folder inside it may not exist yet — the game creates it on
    // first launch — so its absence is "installed, nothing played yet" rather
    // than "not installed".
    Presence::Installed {
        root: local_state.join(GAME_DIR),
    }
}

/// `%LOCALAPPDATA%`, when the platform has one.
fn local_app_data() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
}

/// One world in the player's folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct World {
    /// The folder itself, which is the world's id.
    pub dir: PathBuf,
    /// What the game calls it, read from `levelname.txt`.
    pub name: String,
    /// Whether a `world_icon.jpeg` sits beside it.
    pub has_icon: bool,
}

impl World {
    /// The name to draw: the world's own, or its folder name when the file is
    /// missing. A world with no `levelname.txt` is still a world.
    pub fn label(&self) -> &str {
        if self.name.trim().is_empty() {
            self.dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
        } else {
            &self.name
        }
    }
}

/// Every world under `root`, named and sorted the way a list wants them.
///
/// A missing folder is an empty list rather than an error: "Bedrock has no worlds
/// yet" is the normal state of a fresh install, and a page that showed an error
/// for it would be telling a new player something is broken.
pub fn worlds(root: &Path) -> Vec<World> {
    let dir = root.join("minecraftWorlds");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<World> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| {
            let dir = entry.path();
            World {
                name: std::fs::read_to_string(dir.join("levelname.txt"))
                    .map(|text| text.trim().to_string())
                    .unwrap_or_default(),
                has_icon: dir.join("world_icon.jpeg").is_file(),
                dir,
            }
        })
        .collect();
    out.sort_by(|a, b| a.label().to_lowercase().cmp(&b.label().to_lowercase()));
    out
}

/// Which kind of pack a folder holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackKind {
    /// Blocks, items, textures: what a player sees.
    Resource,
    /// Behaviour: what a world does.
    Behavior,
    /// Both, which Bedrock allows in one folder.
    Both,
}

impl PackKind {
    /// The label a section header draws.
    pub fn label(self) -> &'static str {
        match self {
            PackKind::Resource => "Resource packs",
            PackKind::Behavior => "Behaviour packs",
            PackKind::Both => "Resource and behaviour packs",
        }
    }

    /// The folder under `root` this kind is listed from.
    fn folder(self) -> &'static str {
        match self {
            PackKind::Resource | PackKind::Both => "resource_packs",
            PackKind::Behavior => "behavior_packs",
        }
    }
}

/// One pack folder in the player's data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pack {
    /// The folder itself.
    pub dir: PathBuf,
    /// The name from the pack's own `manifest.json`, when it parses.
    pub name: String,
    /// The `uuid` field of the header, when it parses.
    pub uuid: String,
    /// Whether this folder is a resource pack, a behaviour pack or both.
    pub kind: PackKind,
}

impl Pack {
    /// The name to draw, falling back to the folder name.
    pub fn label(&self) -> &str {
        if self.name.trim().is_empty() {
            self.dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
        } else {
            &self.name
        }
    }
}

/// Every pack of `kind` under `root`, sorted by name.
pub fn packs(root: &Path, kind: PackKind) -> Vec<Pack> {
    let dir = root.join(kind.folder());
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<Pack> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| {
            let dir = entry.path();
            let (name, uuid) = pack_header(&dir.join("manifest.json"));
            Pack {
                dir,
                name,
                uuid,
                kind,
            }
        })
        .collect();
    out.sort_by(|a, b| a.label().to_lowercase().cmp(&b.label().to_lowercase()));
    out
}

/// Read `header.name` and `header.uuid` out of a pack's `manifest.json`.
///
/// A pack whose manifest is missing or malformed is still listed, with an empty
/// name and uuid: the folder exists, so the player has it, and hiding it would
/// be a worse answer than showing its directory name.
fn pack_header(manifest: &Path) -> (String, String) {
    let Ok(text) = std::fs::read_to_string(manifest) else {
        return (String::new(), String::new());
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return (String::new(), String::new());
    };
    let header = value.get("header");
    let field = |name: &str| {
        header
            .and_then(|header| header.get(name))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    (field("name"), field("uuid"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `%LOCALAPPDATA%` with no Store package in it.
    #[test]
    fn a_machine_without_the_package_is_not_installed() {
        let dir = tempfile::tempdir().expect("a temp folder");
        assert_eq!(
            presence_in(dir.path()),
            Presence::NotInstalled,
            "no Packages folder means the Store never installed it"
        );
        // Nothing under it is a world or a pack, and asking is not an error.
        assert!(worlds(dir.path()).is_empty());
        assert!(packs(dir.path(), PackKind::Resource).is_empty());
    }

    /// The Store's own layout, built by hand.
    #[test]
    fn an_installed_copy_is_found_by_the_store_layout() {
        let local = tempfile::tempdir().expect("a temp folder");
        let root = local
            .path()
            .join("Packages")
            .join(PACKAGE_FAMILY)
            .join("LocalState")
            .join(GAME_DIR);
        std::fs::create_dir_all(root.join("minecraftWorlds")).expect("a game folder");
        match presence_in(local.path()) {
            Presence::Installed { root: found } => assert_eq!(found, root),
            other => panic!("{other:?} is not an installed copy"),
        }
    }

    /// Worlds sort by their own names, and one without a name falls back to its
    /// folder rather than disappearing.
    #[test]
    fn worlds_are_named_from_their_own_file() {
        let dir = tempfile::tempdir().expect("a temp folder");
        let worlds_dir = dir.path().join("minecraftWorlds");
        let beta = worlds_dir.join("abc123");
        let alpha = worlds_dir.join("def456");
        let nameless = worlds_dir.join("ghi789");
        for path in [&beta, &alpha, &nameless] {
            std::fs::create_dir_all(path).expect("a world");
        }
        std::fs::write(beta.join("levelname.txt"), "Beta base\n").expect("a name");
        std::fs::write(alpha.join("levelname.txt"), "alpha base").expect("a name");
        std::fs::write(alpha.join("world_icon.jpeg"), b"\xff\xd8\xff").expect("an icon");

        let listed = worlds(dir.path());
        assert_eq!(listed.len(), 3);
        assert_eq!(listed[0].label(), "alpha base");
        assert_eq!(listed[1].label(), "Beta base", "sorted without case");
        assert!(listed[0].has_icon);
        assert!(!listed[1].has_icon);
        // The folder name is the fallback, so the world is still on the list.
        assert_eq!(listed[2].label(), "ghi789");
    }

    /// A pack's name comes from its manifest, and a broken manifest does not
    /// remove the pack from the list.
    #[test]
    fn packs_are_named_from_their_manifest_and_survive_a_broken_one() {
        let dir = tempfile::tempdir().expect("a temp folder");
        let good = dir.path().join("resource_packs").join("aaa");
        let broken = dir.path().join("resource_packs").join("bbb");
        std::fs::create_dir_all(&good).expect("a pack");
        std::fs::create_dir_all(&broken).expect("a pack");
        std::fs::write(
            good.join("manifest.json"),
            br#"{"header":{"name":"Faithful 32x","uuid":"1111","version":[1,0,0]},"modules":[]}"#,
        )
        .expect("a manifest");
        std::fs::write(broken.join("manifest.json"), b"{oops").expect("a manifest");

        let listed = packs(dir.path(), PackKind::Resource);
        assert_eq!(listed.len(), 2);
        let faithful = listed
            .iter()
            .find(|pack| !pack.uuid.is_empty())
            .expect("the pack with a manifest");
        assert_eq!(faithful.label(), "Faithful 32x");
        assert_eq!(faithful.uuid, "1111");
        // The unreadable one keeps its folder as its name and an empty uuid.
        let broken = listed
            .iter()
            .find(|pack| pack.uuid.is_empty())
            .expect("the pack with a broken manifest");
        assert_eq!(broken.label(), "bbb");
        // Behaviour packs are their own folder, and this one has none.
        assert!(packs(dir.path(), PackKind::Behavior).is_empty());
    }

    #[test]
    fn pack_kinds_name_their_own_folders() {
        assert_eq!(PackKind::Resource.folder(), "resource_packs");
        assert_eq!(PackKind::Behavior.folder(), "behavior_packs");
        // "Both" lists from the resource folder, which is where Bedrock puts a
        // combined pack's folder.
        assert_eq!(PackKind::Both.folder(), "resource_packs");
        for kind in [PackKind::Resource, PackKind::Behavior, PackKind::Both] {
            assert!(!kind.label().is_empty());
        }
    }

    /// The one thing the page must never do: offer a download.
    #[test]
    fn the_store_link_is_a_store_link() {
        assert!(STORE_URL.starts_with("ms-windows-store://"));
        assert!(PACKAGE_FAMILY.starts_with("Microsoft.MinecraftUWP_"));
    }
}
