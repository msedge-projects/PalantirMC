//! Prism/MultiMC instance exports, imported.
//!
//! A Prism or MultiMC export is a zip of the instance directory:
//! `instance.cfg` (the name and settings) and `mmc-pack.json` (the
//! components: the game, the loader, the libraries they carry). The
//! component format is Prism's own `PackProfile` serialization
//! (`formatVersion: 1`, `components: [{uid, version, ...}]`); the game
//! directory rides along beside them.
//!
//! Component uids are Java-package-style, and the mapping was read off
//! Prism's own source (their `PackProfile.cpp` serialization) plus two
//! real instances (`THIRD_PARTY_NOTICES.md`): `net.minecraft` is the
//! game, `net.minecraftforge`/`net.neoforged`/`net.fabricmc.fabric-loader`/
//! `org.quiltmc.quilt-loader` are the loaders, `org.lwjgl*` and
//! `net.fabricmc.intermediary` are dependency-only components the loader
//! or game already covers. Anything else is a component this launcher
//! does not implement -- reported, never dropped, and never mistaken for
//! vanilla.
//!
//! `instance.cfg` is INI-ish (`key=value`, sections): its `name` is the
//! instance's name. MultiMC exports game content under `minecraft/`, so
//! that directory is the override layer.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::import::{ImportedPack, LoaderTarget, OverrideLayer, SideFilter, is_safe_relative};

/// `mmc-pack.json`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MmcPack {
    #[serde(rename = "formatVersion")]
    pub format_version: u32,
    #[serde(default)]
    pub components: Vec<Component>,
}

/// One component of the pack.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Component {
    pub uid: String,
    #[serde(default)]
    pub version: Option<String>,
    /// Prism's "the game cannot run without this" flag; the game
    /// component sets it.
    #[serde(default)]
    pub important: bool,
    /// A dependency of something else, not a thing of its own: the
    /// format marks lwjgl and intermediary this way.
    #[serde(rename = "dependencyOnly", default)]
    pub dependency_only: bool,
    #[serde(rename = "cachedName", default)]
    pub cached_name: Option<String>,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl MmcPack {
    /// Also BOM-tolerant: these files round-trip through many editors.
    pub fn parse(text: &str) -> Result<Self> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let pack: MmcPack = serde_json::from_str(text).map_err(|source| Error::Invalid {
            what: "mmc-pack.json",
            why: source.to_string(),
        })?;
        if pack.format_version != 1 {
            return Err(Error::Invalid {
                what: "mmc-pack.json",
                why: format!("formatVersion {} is not 1", pack.format_version),
            });
        }
        Ok(pack)
    }
}

/// `instance.cfg`: the INI-shaped side of an instance. Only the keys an
/// import needs are modelled; the rest ride along untouched.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InstanceCfg {
    pub name: Option<String>,
    pub icon_key: Option<String>,
    /// Every `key=value` seen, so a caller can read the rest.
    pub entries: BTreeMap<String, String>,
}

impl InstanceCfg {
    pub fn parse(text: &str) -> Self {
        let mut cfg = InstanceCfg::default();
        for line in text.lines() {
            let line = line.trim();
            // INI sections ([General]) and blank lines carry no pairs.
            if line.is_empty() || line.starts_with(['[', '#', ';']) {
                continue;
            }
            // Split on the first '=' only: values (javaArgs, notes) may
            // contain '=' themselves.
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim().to_string();
                match key.as_str() {
                    "name" => cfg.name = Some(value.to_string()),
                    "iconKey" => cfg.icon_key = Some(value.to_string()),
                    _ => {}
                }
                cfg.entries.insert(key, value.to_string());
            }
        }
        cfg
    }
}

/// Translate a Prism/MultiMC instance into the install's order form.
/// `game_dir_prefix` is the export's game-content directory (`minecraft`
/// in a MultiMC export).
pub fn import(pack: &MmcPack, cfg: &InstanceCfg, game_dir_prefix: &str) -> Result<ImportedPack> {
    if !is_safe_relative(game_dir_prefix) {
        return Err(Error::Invalid {
            what: "mmc-pack.json",
            why: format!("{game_dir_prefix:?} is not a safe directory name"),
        });
    }
    let mut game: Option<String> = None;
    let mut loader = LoaderTarget::Vanilla;
    let mut unknown = BTreeMap::new();
    for component in &pack.components {
        let version = component.version.clone().unwrap_or_default();
        match component.uid.as_str() {
            "net.minecraft" => game = Some(version),
            "net.minecraftforge" => loader = LoaderTarget::Forge { loader: version },
            "net.neoforged" => loader = LoaderTarget::NeoForge { loader: version },
            "net.fabricmc.fabric-loader" => loader = LoaderTarget::Fabric { loader: version },
            "org.quiltmc.quilt-loader" => loader = LoaderTarget::Quilt { loader: version },
            // Dependency-only components name things the game or loader
            // document already carries: lwjgl comes with the game's
            // libraries, intermediary with the Fabric profile.
            uid if component.dependency_only || uid.starts_with("org.lwjgl") => {}
            uid => {
                unknown.insert(uid.to_string(), version);
            }
        }
    }
    let game = game.ok_or_else(|| Error::Invalid {
        what: "mmc-pack.json",
        why: "no net.minecraft component names the game version".to_string(),
    })?;
    if matches!(loader, LoaderTarget::Vanilla) {
        // A custom component alone is a loader this launcher does not
        // implement; installing it as vanilla would be a silent wrong
        // install.
        if let Some((id, version)) = unknown.iter().next() {
            loader = LoaderTarget::Unknown {
                id: id.clone(),
                version: version.clone(),
            };
        }
    }
    Ok(ImportedPack {
        name: cfg.name.clone().unwrap_or_else(|| game.clone()),
        game,
        loader,
        files: Vec::new(),
        overrides: vec![OverrideLayer {
            prefix: game_dir_prefix.to_string(),
            side: SideFilter::Both,
        }],
        unknown,
    })
}
