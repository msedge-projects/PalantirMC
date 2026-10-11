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
use serde_json::Value;

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

/// One file a plan will place, reduced to what the caller must act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedFile {
    /// Fetched and verified. `url` is the first of the record's downloads:
    /// the format lists them in the order they should be tried.
    Download {
        path: String,
        url: String,
        sha1: Option<String>,
        size: Option<u64>,
    },
    /// A record that names no download. The bytes are expected on this
    /// machine already (a migrated install, a hand-placed file), so the plan
    /// says so rather than fetching something blind.
    Local { path: String },
}

/// A directory in the archive whose contents overwrite the game directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Override {
    /// The archive directory, e.g. `overrides`.
    pub source: String,
    /// Where its contents land, relative to the game directory. Empty for a
    /// layer that keeps the pack's own relative paths.
    pub destination: String,
}

/// What a `.mrpack` will place, worked out before anything is fetched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackPlan {
    pub files: Vec<SelectedFile>,
    /// Overwrite layers in application order, base first.
    pub overrides: Vec<Override>,
}

/// Plan a client install of `index`.
///
/// This takes the raw `modrinth.index.json` value rather than a parsed
/// [`MrpackIndex`] because it runs at *review* time, before a pack has passed
/// the strict importer: a reviewer has to be able to describe a pack `parse`
/// would refuse, so the safety rules are re-applied here and a value that
/// never went through `parse` still cannot smuggle a path out of the game
/// directory.
///
/// `selected_optional` names the optional files the user accepted, by path --
/// the same identity the importer's selection uses, so one choice cannot mean
/// two things. Only the client's view is planned: a server install belongs to
/// a server flow this launcher does not have, so a file marked `unsupported`
/// for the client is absent from the plan rather than planned and refused
/// later.
///
/// What it deliberately does not do is fetch: the caller's syncer owns
/// transfers, resumption and hash verification, and a second owner here would
/// mean two writers for the same bytes.
pub fn plan_files(index: &Value, selected_optional: &[String]) -> Result<PackPlan> {
    let entries = index
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::Invalid {
            what: "modrinth.index.json",
            why: "files is missing or is not a list".to_string(),
        })?;
    let mut files = Vec::new();
    for entry in entries {
        let path = entry
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::Invalid {
                what: "modrinth.index.json",
                why: "a file entry names no path".to_string(),
            })?
            .to_string();
        if !is_safe_relative(&path) {
            return Err(Error::Invalid {
                what: "modrinth.index.json",
                why: format!("{path:?} would escape the game directory"),
            });
        }
        match client_support(entry) {
            Support::Unsupported => continue,
            Support::Optional if !selected_optional.iter().any(|chosen| chosen == &path) => {
                continue;
            }
            Support::Required | Support::Optional => {}
        }
        let url = entry
            .get("downloads")
            .and_then(Value::as_array)
            .and_then(|urls| urls.iter().find_map(Value::as_str))
            .map(str::to_string);
        files.push(match url {
            Some(url) => SelectedFile::Download {
                path,
                url,
                sha1: entry
                    .get("hashes")
                    .and_then(|hashes| hashes.get("sha1"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                size: entry.get("fileSize").and_then(Value::as_u64),
            },
            None => SelectedFile::Local { path },
        });
    }
    Ok(PackPlan {
        files,
        overrides: override_layers(),
    })
}

/// The overwrite layers, in application order.
///
/// Order is the whole content of this list: the format defines the client
/// layer as applied *over* the base one, so a file carried in both must end
/// as the client's. Only the archive knows which of these directories it
/// actually holds, so this records the layers and their precedence rather
/// than inventing a file list the index does not contain.
pub fn override_layers() -> Vec<Override> {
    ["overrides", "client-overrides"]
        .into_iter()
        .map(|source| Override {
            source: source.to_string(),
            destination: String::new(),
        })
        .collect()
}

/// One entry's support for the client side, defaulting the way the format
/// does: an absent `env`, or an absent side key inside it, means required.
fn client_support(entry: &Value) -> Support {
    entry
        .get("env")
        .and_then(|env| env.get("client"))
        .and_then(Value::as_str)
        .and_then(support_from)
        .unwrap_or(Support::Required)
}
