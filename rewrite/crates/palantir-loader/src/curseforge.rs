//! CurseForge modpack exports, imported.
//!
//! CurseForge's export format (`manifest.json` + `overrides/`) is the
//! lingua franca of modpack distribution: the CurseForge app produces
//! it, and GDLauncher and ATLauncher both consume and re-produce it. The
//! manifest names the game, the mod loader (`modLoaders[].id` like
//! `fabric-0.19.5`), and the files as *catalogue ids* (`projectID`/
//! `fileID`) -- no URLs, no hashes, no names. The catalogue's API
//! resolves those at install time; this translation carries the ids
//! through as `FileSource::Catalogue` so nothing is guessed early.
//!
//! The fixture is a real published manifest (`THIRD_PARTY_NOTICES.md`).
//!
//! Loader ids are `<kind>-<version>`; the kind is one of the four this
//! launcher installs, and anything else -- OptiFine, LiteLoader, a
//! future one -- is reported as unknown rather than dropped.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::import::{
    Env, FileSource, ImportedPack, LoaderTarget, OverrideLayer, PackFile, SideFilter, Support,
};

/// `manifest.json`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Manifest {
    pub minecraft: Minecraft,
    #[serde(rename = "manifestType")]
    pub manifest_type: String,
    #[serde(rename = "manifestVersion")]
    pub manifest_version: u32,
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub files: Vec<ManifestFile>,
    #[serde(default)]
    pub overrides: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Minecraft {
    pub version: String,
    #[serde(rename = "modLoaders", default)]
    pub mod_loaders: Vec<ModLoader>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ModLoader {
    /// `<kind>-<version>`, e.g. `forge-47.4.26`.
    pub id: String,
    #[serde(default)]
    pub primary: bool,
}

/// One file, by catalogue id.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ManifestFile {
    #[serde(rename = "projectID")]
    pub project_id: u64,
    #[serde(rename = "fileID")]
    pub file_id: u64,
    #[serde(default)]
    pub required: bool,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl Manifest {
    /// Also BOM-tolerant: the app writes these files constantly.
    pub fn parse(text: &str) -> Result<Self> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let manifest: Manifest = serde_json::from_str(text).map_err(|source| Error::Invalid {
            what: "manifest.json",
            why: source.to_string(),
        })?;
        if manifest.manifest_type != "minecraftModpack" {
            return Err(Error::Invalid {
                what: "manifest.json",
                why: format!(
                    "manifestType {:?} is not minecraftModpack",
                    manifest.manifest_type
                ),
            });
        }
        if manifest.manifest_version != 1 {
            return Err(Error::Invalid {
                what: "manifest.json",
                why: format!("manifestVersion {} is not 1", manifest.manifest_version),
            });
        }
        Ok(manifest)
    }
}

/// Translate the manifest into the install's order form.
pub fn import(manifest: &Manifest) -> Result<ImportedPack> {
    let (loader, unknown) = loader_from(&manifest.minecraft.mod_loaders);
    let files = manifest
        .files
        .iter()
        .map(|file| PackFile {
            // A catalogue file has no name until the API answers; the
            // installer places it under mods/ when it resolves.
            path: String::new(),
            // The `required` flag is the format's own optional marker:
            // a pack author may publish a file the player can decline.
            env: Env {
                client: if file.required {
                    Support::Required
                } else {
                    Support::Optional
                },
                server: if file.required {
                    Support::Required
                } else {
                    Support::Optional
                },
            },
            source: FileSource::Catalogue {
                project_id: file.project_id,
                file_id: file.file_id,
            },
        })
        .collect();
    let mut overrides = Vec::new();
    if let Some(prefix) = &manifest.overrides {
        // The overrides directory is optional and named by the manifest;
        // its name is pack-supplied, so it gets the same safety rule as
        // any other pack-supplied path.
        if !prefix.is_empty() {
            overrides.push(OverrideLayer {
                prefix: prefix.clone(),
                side: SideFilter::Both,
            });
        }
    }
    Ok(ImportedPack {
        name: manifest.name.clone(),
        game: manifest.minecraft.version.clone(),
        loader,
        files,
        overrides,
        unknown,
    })
}

/// The primary loader, or the pack's own pick when there is no primary;
/// ids this launcher does not install are reported, never dropped.
fn loader_from(mod_loaders: &[ModLoader]) -> (LoaderTarget, BTreeMap<String, String>) {
    let mut unknown = BTreeMap::new();
    let mut loader = LoaderTarget::Vanilla;
    let chosen = mod_loaders
        .iter()
        .find(|loader| loader.primary)
        .or_else(|| mod_loaders.first());
    for entry in mod_loaders {
        if Some(entry) != chosen {
            // A second loader is a pack bug or an extra like OptiFine;
            // keep it visible rather than silently losing it.
            unknown.insert(entry.id.clone(), String::new());
            continue;
        }
        let Some((kind, version)) = entry.id.split_once('-') else {
            unknown.insert(entry.id.clone(), String::new());
            continue;
        };
        let version = version.to_string();
        loader = match kind {
            "forge" => LoaderTarget::Forge { loader: version },
            "neoforge" => LoaderTarget::NeoForge { loader: version },
            "fabric" => LoaderTarget::Fabric { loader: version },
            "quilt" => LoaderTarget::Quilt { loader: version },
            _ => {
                unknown.insert(entry.id.clone(), String::new());
                LoaderTarget::Unknown {
                    id: kind.to_string(),
                    version,
                }
            }
        };
    }
    (loader, unknown)
}
