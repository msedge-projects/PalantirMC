//! The Java runtime that Mojang ships for the game.
//!
//! Two documents, both from the metadata service:
//!
//! - the **product index** (`java-runtime/all`): platform -> runtime family
//!   -> a manifest pointer and availability. The platform names are
//!   Mojang's own (`windows-x64`, `mac-os-arm64`, `linux`, ...).
//! - a **runtime manifest**: one `files` map keyed by the runtime-relative
//!   path. Each entry is a `file` (with `raw` and `lzma` downloads and an
//!   `executable` flag), a `directory`, or a `link` (a symbolic link with
//!   a `target`).
//!
//! The index is not addressed by a stable URL: its path carries the hash
//! of its content (the launcher pins it). When Mojang re-issues the index
//! the old hash stops resolving, so the constant below is the pinned
//! pointer and a 404 for it is a loud error naming what to update, never a
//! silent empty result.
//!
//! What lands is a plain runtime directory: every `file` at its relative
//! path (verified by SHA-1), executables marked on platforms that have
//! the bit, `link`s made as symbolic links, `directory`s created.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use palantir_core::rules::Platform;
use palantir_net::client::Http;
use palantir_net::download::{self, DownloadOptions, Transfer};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// The product index's pinned location (its path carries its content hash).
pub const JAVA_RUNTIME_ALL_URL: &str = "https://piston-meta.mojang.com/v1/products/java-runtime/\
     2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

/// The product index: platform name -> runtime family -> entries.
pub type RuntimeIndex = BTreeMap<String, BTreeMap<String, Vec<RuntimeEntry>>>;

/// One published runtime: where its manifest is, and what version it says
/// it is. `availability` rides along unmodelled; it is the launcher's
/// signal for how finished a runtime is, not ours to interpret.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeEntry {
    pub manifest: ManifestRef,
    pub version: RuntimeVersion,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// A manifest's location and its promise.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestRef {
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

/// What the runtime calls itself (`1.8.0_202` and whether it is a JRE).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeVersion {
    pub name: String,
    #[serde(default)]
    pub released: Option<String>,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// One runtime's contents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeManifest {
    pub files: BTreeMap<String, RuntimeFile>,
}

/// One entry in a runtime: a file to fetch, a directory to make, or a
/// symbolic link to point at another entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RuntimeFile {
    File {
        downloads: Downloads,
        #[serde(default)]
        executable: bool,
        #[serde(flatten, default)]
        extra: BTreeMap<String, serde_json::Value>,
    },
    Directory {
        #[serde(flatten, default)]
        extra: BTreeMap<String, serde_json::Value>,
    },
    Link {
        target: String,
        #[serde(flatten, default)]
        extra: BTreeMap<String, serde_json::Value>,
    },
}

/// A file's two packagings: the bytes as they are, and the same bytes
/// LZMA-compressed (smaller to move; only some runtimes offer it).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Downloads {
    pub raw: FileDownload,
    #[serde(default)]
    pub lzma: Option<FileDownload>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileDownload {
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

/// The one file a platform gets: which Mojang platform name this is, and
/// the runtime family to pick from it.
pub struct RuntimeChoice<'a> {
    pub platform: &'a str,
    pub component: &'a str,
}
/// The index is published with a UTF-8 BOM; serde wants it gone.
pub fn parse_index(text: &str) -> Result<RuntimeIndex> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    serde_json::from_str(text).map_err(|source| Error::Invalid {
        what: "java runtime index",
        why: source.to_string(),
    })
}

/// The manifest pointer for one platform's runtime family.
pub fn select<'a>(index: &'a RuntimeIndex, choice: &RuntimeChoice<'_>) -> Result<&'a RuntimeEntry> {
    let families = index.get(choice.platform).ok_or_else(|| Error::Invalid {
        what: "java runtime index",
        why: format!("it names no platform {}", choice.platform),
    })?;
    let entries = families
        .get(choice.component)
        .ok_or_else(|| Error::Invalid {
            what: "java runtime index",
            why: format!(
                "it has no runtime {} for {}",
                choice.component, choice.platform
            ),
        })?;
    entries.first().ok_or_else(|| Error::Invalid {
        what: "java runtime index",
        why: format!(
            "runtime {} for {} is listed but empty",
            choice.component, choice.platform
        ),
    })
}

impl RuntimeManifest {
    /// Also BOM-tolerant: both documents come from the same service.
    pub fn parse(text: &str) -> Result<Self> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        serde_json::from_str(text).map_err(|source| Error::Invalid {
            what: "java runtime manifest",
            why: source.to_string(),
        })
    }
}

/// Where a runtime's platform-family directory lives under the java root.
pub fn runtime_dir(java_root: &Path, choice: &RuntimeChoice<'_>) -> PathBuf {
    java_root.join(choice.component).join(choice.platform)
}

/// One manifest entry placed at its path, for the caller to fetch or make.
#[derive(Debug)]
pub enum Placement {
    /// Fetch `download` to this path; `executable` marks it runnable.
    File {
        path: PathBuf,
        download: FileDownload,
        executable: bool,
    },
    Directory {
        path: PathBuf,
    },
    Link {
        path: PathBuf,
        target: String,
    },
}

/// Every entry of a manifest, resolved against the runtime's directory.
///
/// A relative path that would escape the runtime directory is refused the
/// same way the data root refuses one: a manifest is fetched content like
/// any other and its names are joined, never trusted.
pub fn placements(manifest: &RuntimeManifest, into: &Path) -> Result<Vec<Placement>> {
    let mut out = Vec::new();
    for (name, entry) in &manifest.files {
        if name.contains("..") || name.starts_with('/') || name.contains('\\') {
            return Err(Error::Invalid {
                what: "java runtime manifest",
                why: format!("{name:?} is not a relative path inside the runtime"),
            });
        }
        let path = into.join(name);
        out.push(match entry {
            RuntimeFile::File {
                downloads,
                executable,
                ..
            } => Placement::File {
                path,
                download: downloads.raw.clone(),
                executable: *executable,
            },
            RuntimeFile::Directory { .. } => Placement::Directory { path },
            RuntimeFile::Link { target, .. } => Placement::Link {
                path,
                target: target.clone(),
            },
        });
    }
    Ok(out)
}

/// What landing a runtime did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeReport {
    /// Files fetched from the network.
    pub fetched: usize,
    /// Files already on disk and verified.
    pub reused: usize,
    /// Bytes fetched.
    pub bytes: u64,
}

/// Land a runtime at `into`: directories made, every file fetched and
/// verified by its promised hash, executables marked, links pointed.
///
/// A file already there and correct is left alone -- the transfer layer
/// verifies before it keeps -- so a second run over an existing runtime
/// asks the network nothing. Links are made last and only on platforms
/// that have them: no runtime Mojang ships for Windows contains one.
pub fn fetch_runtime(
    http: &Http,
    manifest: &RuntimeManifest,
    into: &Path,
    options: DownloadOptions,
) -> Result<RuntimeReport> {
    let mut report = RuntimeReport::default();
    let placed = placements(manifest, into)?;

    for placement in &placed {
        if let Placement::Directory { path } = placement {
            std::fs::create_dir_all(path).map_err(|source| Error::Io {
                path: path.clone(),
                source,
            })?;
        }
    }
    for placement in &placed {
        let Placement::File {
            path,
            download: file,
            executable,
        } = placement
        else {
            continue;
        };
        let transfer: Transfer = download::download(
            http,
            &file.url,
            path,
            Some(&file.sha1),
            Some(file.size),
            &options,
        )?;
        if transfer.already_present {
            report.reused += 1;
        } else {
            report.fetched += 1;
            report.bytes += transfer.bytes;
        }
        if *executable {
            mark_executable(path)?;
        }
    }
    for placement in &placed {
        let Placement::Link { path, target } = placement else {
            continue;
        };
        make_link(path, target)?;
    }
    Ok(report)
}

#[cfg(unix)]
fn mark_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path)
        .map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?
        .permissions();
    permissions.set_mode(permissions.mode() | 0o111);
    std::fs::set_permissions(path, permissions).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(not(unix))]
fn mark_executable(_path: &Path) -> Result<()> {
    // Windows has no executable bit; the file is runnable by extension.
    Ok(())
}

#[cfg(unix)]
fn make_link(path: &Path, target: &str) -> Result<()> {
    if path.exists() {
        return Ok(()); // already pointed
    }
    std::os::unix::fs::symlink(target, path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(not(unix))]
fn make_link(path: &Path, _target: &str) -> Result<()> {
    Err(Error::Invalid {
        what: "java runtime manifest",
        why: format!(
            "{} is a symbolic link and this platform does not take them; \
             no runtime Mojang ships for it contains one",
            path.display()
        ),
    })
}

/// The Mojang platform name for a running system. The names are Mojang's
/// own; the mapping is ours, from what each OS/arch pair is called there.
pub fn mojang_platform(platform: &Platform) -> Result<&'static str> {
    use palantir_core::rules::{Arch, Os};
    match (platform.os, platform.arch) {
        (Os::Windows, Arch::X86_64) => Ok("windows-x64"),
        (Os::Windows, Arch::X86) => Ok("windows-x86"),
        (Os::Linux, Arch::X86_64) => Ok("linux"),
        (Os::Linux, Arch::X86) => Ok("linux-i386"),
        (Os::MacOs, Arch::X86_64) => Ok("mac-os"),
        (Os::MacOs, Arch::Aarch64) => Ok("mac-os-arm64"),
        (os, arch) => Err(Error::Invalid {
            what: "java runtime platform",
            why: format!("{os:?}/{arch:?} is not a platform Mojang ships a runtime for"),
        }),
    }
}
