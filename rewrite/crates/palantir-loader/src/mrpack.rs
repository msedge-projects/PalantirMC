//! Modrinth's `.mrpack` modpack format, imported.
//!
//! A `.mrpack` is a zip (MIME `application/x-modrinth-modpack+zip`) whose
//! root carries `modrinth.index.json` -- the whole pack as a file list --
//! plus `overrides/`, `client-overrides/` and `server-overrides/`
//! directories that layer over the game directory. The format is
//! documented by Modrinth (support article 8802351); the fixtures are a
//! real pack's own index and zip (`THIRD_PARTY_NOTICES.md`).
//!
//! What the index says, and what it means here:
//!
//! - `files[].path` -- relative to the game directory. The specification
//!   warns importers outright: refuse anything that leaves it.
//! - `files[].hashes` -- MUST carry SHA-1 and SHA-512; anything less is
//!   not a usable record (verification is the whole point).
//! - `files[].env` -- per-side `required`/`optional`/`unsupported`;
//!   absent means required on both sides.
//! - `files[].downloads` -- HTTPS URLs, tried in order.
//! - `dependencies` -- `minecraft` plus one loader id. The specification
//!   warns that new ids may appear at any time, so an unrecognized id is
//!   reported, never silently dropped -- and when it is the *only*
//!   loader, it becomes the install's `Unknown`, which the caller must
//!   refuse rather than install as something else.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::import::{
    Env, FileSource, ImportedPack, LoaderTarget, OverrideLayer, PackFile, SideFilter, Support,
    is_safe_relative, support_from,
};

/// `modrinth.index.json`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MrpackIndex {
    #[serde(rename = "formatVersion")]
    pub format_version: u32,
    pub game: String,
    #[serde(rename = "versionId")]
    pub version_id: String,
    pub name: String,
    #[serde(default)]
    pub summary: Option<String>,
    pub files: Vec<MrpackFile>,
    pub dependencies: BTreeMap<String, String>,
}

/// One entry of the `files` array.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MrpackFile {
    pub path: String,
    pub hashes: BTreeMap<String, String>,
    #[serde(default)]
    pub env: Option<MrpackEnv>,
    pub downloads: Vec<String>,
    #[serde(rename = "fileSize", default)]
    pub file_size: Option<u64>,
}

/// The `env` object: absent side keys mean that side is required.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MrpackEnv {
    #[serde(default)]
    pub client: Option<String>,
    #[serde(default)]
    pub server: Option<String>,
}

impl MrpackIndex {
    /// Also BOM-tolerant: packs are produced by many tools.
    pub fn parse(text: &str) -> Result<Self> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let index: MrpackIndex = serde_json::from_str(text).map_err(|source| Error::Invalid {
            what: "modrinth.index.json",
            why: source.to_string(),
        })?;
        index.validate()?;
        Ok(index)
    }

    fn validate(&self) -> Result<()> {
        if self.format_version != 1 {
            return Err(Error::Invalid {
                what: "modrinth.index.json",
                why: format!("formatVersion {} is not 1", self.format_version),
            });
        }
        if self.game != "minecraft" {
            return Err(Error::Invalid {
                what: "modrinth.index.json",
                why: format!("game {:?} is not minecraft", self.game),
            });
        }
        let mut seen = std::collections::BTreeSet::new();
        for file in &self.files {
            if !is_safe_relative(&file.path) {
                return Err(Error::Invalid {
                    what: "modrinth.index.json",
                    why: format!("{:?} would escape the game directory", file.path),
                });
            }
            if !seen.insert(file.path.clone()) {
                return Err(Error::Invalid {
                    what: "modrinth.index.json",
                    why: format!("{:?} is named twice", file.path),
                });
            }
            // The specification says MUST: both hashes, or the record
            // cannot be verified and no launcher should fetch blind.
            for needed in ["sha1", "sha512"] {
                if !file.hashes.contains_key(needed) {
                    return Err(Error::Invalid {
                        what: "modrinth.index.json",
                        why: format!("{:?} has no {needed} hash", file.path),
                    });
                }
            }
            if file.downloads.is_empty() {
                return Err(Error::Invalid {
                    what: "modrinth.index.json",
                    why: format!("{:?} names no download", file.path),
                });
            }
        }
        Ok(())
    }
}

/// Translate the index into the install's order form.
pub fn import(index: &MrpackIndex) -> Result<ImportedPack> {
    let (loader, unknown) = loader_from(&index.dependencies);
    let game = index
        .dependencies
        .get("minecraft")
        .ok_or_else(|| Error::Invalid {
            what: "modrinth.index.json",
            why: "dependencies names no minecraft version".to_string(),
        })?
        .clone();
    let files = index
        .files
        .iter()
        .map(|file| PackFile {
            path: file.path.clone(),
            env: env_from(file),
            source: FileSource::Download {
                urls: file.downloads.clone(),
                hashes: file.hashes.clone(),
                size: file.file_size,
            },
        })
        .collect();
    Ok(ImportedPack {
        name: index.name.clone(),
        game,
        loader,
        files,
        // Side layers after the base layer: the specification defines
        // them as applied over `overrides`, overwriting its contents.
        overrides: vec![
            OverrideLayer {
                prefix: "overrides".to_string(),
                side: SideFilter::Both,
            },
            OverrideLayer {
                prefix: "client-overrides".to_string(),
                side: SideFilter::ClientOnly,
            },
            OverrideLayer {
                prefix: "server-overrides".to_string(),
                side: SideFilter::ServerOnly,
            },
        ],
        unknown,
    })
}

fn env_from(file: &MrpackFile) -> Env {
    let side = |value: &Option<String>| match value.as_deref() {
        None => Support::Required,
        Some(text) => support_from(text).unwrap_or(Support::Required),
    };
    match &file.env {
        None => Env::default(),
        Some(env) => Env {
            client: side(&env.client),
            server: side(&env.server),
        },
    }
}

/// The loader the dependency list names, and the ids this launcher does
/// not know. An unrecognized id alone is the loader -- installing as
/// vanilla would be a silent wrong install -- while alongside a known
/// loader it rides along in `unknown` for the caller to report.
fn loader_from(
    dependencies: &BTreeMap<String, String>,
) -> (LoaderTarget, BTreeMap<String, String>) {
    let mut unknown = BTreeMap::new();
    let mut loader = LoaderTarget::Vanilla;
    for (id, version) in dependencies {
        let target = match id.as_str() {
            "minecraft" => continue,
            "fabric-loader" => Some(LoaderTarget::Fabric {
                loader: version.clone(),
            }),
            "quilt-loader" => Some(LoaderTarget::Quilt {
                loader: version.clone(),
            }),
            "forge" => Some(LoaderTarget::Forge {
                loader: version.clone(),
            }),
            "neoforge" => Some(LoaderTarget::NeoForge {
                loader: version.clone(),
            }),
            _ => None,
        };
        match target {
            Some(target) => loader = target,
            None => {
                unknown.insert(id.clone(), version.clone());
            }
        }
    }
    if matches!(loader, LoaderTarget::Vanilla) {
        if let Some((id, version)) = unknown.iter().next() {
            loader = LoaderTarget::Unknown {
                id: id.clone(),
                version: version.clone(),
            };
        }
    }
    (loader, unknown)
}
