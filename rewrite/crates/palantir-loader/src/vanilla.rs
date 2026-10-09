//! The standard `.minecraft` install, imported.
//!
//! The official Minecraft Launcher, TLauncher, SKLauncher, Badlion,
//! Legacy Launcher and most others keep the same layout Mojang's
//! launcher documented: `launcher_profiles.json` naming *profiles* (a
//! display name, a game directory, a version id) over `versions/`, where
//! each version is `<id>/<id>.json` -- an ordinary version document, the
//! format `palantir-core` already speaks. Lunar and Feather wrap this
//! layout in their own roots but keep the same shapes inside.
//!
//! So an import is: read the profiles, and for each one that has a
//! version document on hand, take the document as the pack's own
//! metadata (`install_document` installs it verbatim) and the profile's
//! game directory as the content to migrate. The fixture is a real
//! `launcher_profiles.json` (`THIRD_PARTY_NOTICES.md`), with its
//! authentication database removed -- the file carries an access token
//! and an importer never reads it.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::import::{Env, FileSource, ImportedPack, LoaderTarget, PackFile};

/// `launcher_profiles.json`, the parts an import reads.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LauncherProfiles {
    pub profiles: BTreeMap<String, Profile>,
    #[serde(rename = "selectedProfile", default)]
    pub selected_profile: Option<String>,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// One installation profile.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Profile {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(rename = "gameDir", default)]
    pub game_dir: Option<String>,
    #[serde(rename = "lastVersionId", default)]
    pub last_version_id: Option<String>,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl LauncherProfiles {
    /// Also BOM-tolerant; the file is rewritten by every launcher fork.
    pub fn parse(text: &str) -> Result<Self> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        serde_json::from_str(text).map_err(|source| Error::Invalid {
            what: "launcher_profiles.json",
            why: source.to_string(),
        })
    }

    /// The profile the launcher last used, if the file still names it.
    pub fn selected(&self) -> Option<(&str, &Profile)> {
        let id = self.selected_profile.as_deref()?;
        self.profiles
            .get_key_value(id)
            .map(|(k, v)| (k.as_str(), v))
    }
}

/// One profile's content, as the files a migration copies.
///
/// A profile with a `gameDir` keeps its worlds, options and mods there;
/// one without shares the `.minecraft` root. The files are the directory
/// itself -- the installer walks it -- because the set of what a game
/// directory contains is the game's business, not the metadata's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migration {
    pub instance_name: String,
    pub game: String,
    pub game_dir: String,
}

/// Translate one profile into a migration. `root` is the `.minecraft`
/// directory the profiles file lives in; a profile's `gameDir` is taken
/// as an absolute path or relative to it.
pub fn migration(profiles: &LauncherProfiles, id: &str) -> Result<Migration> {
    let (key, profile) = profiles
        .profiles
        .get_key_value(id)
        .map(|(k, v)| (k.as_str(), v))
        .ok_or_else(|| Error::Invalid {
            what: "launcher_profiles.json",
            why: format!("no profile named {id:?}"),
        })?;
    let game = profile
        .last_version_id
        .clone()
        .ok_or_else(|| Error::Invalid {
            what: "launcher_profiles.json",
            why: format!("profile {key:?} names no lastVersionId"),
        })?;
    Ok(Migration {
        instance_name: profile.name.clone().unwrap_or_else(|| key.to_string()),
        game,
        game_dir: profile.game_dir.clone().unwrap_or_default(),
    })
}

/// Fold one migration into a pack: the profile's game version and its
/// game directory's content as local files.
///
/// The version document is read by the caller (it lives at
/// `versions/<id>/<id>.json`) and installed through `install_document`;
/// this translation covers the content side of the migration.
pub fn import(migration: &Migration) -> ImportedPack {
    let mut files = Vec::new();
    if !migration.game_dir.is_empty() {
        // The directory itself is the content: one Local file carrying
        // the tree, copied wholesale by the installer.
        files.push(PackFile {
            path: String::new(),
            env: Env::default(),
            source: FileSource::Local {
                absolute: migration.game_dir.clone(),
            },
        });
    }
    ImportedPack {
        name: migration.instance_name.clone(),
        game: migration.game.clone(),
        loader: LoaderTarget::Vanilla,
        files,
        overrides: Vec::new(),
        unknown: BTreeMap::new(),
    }
}
