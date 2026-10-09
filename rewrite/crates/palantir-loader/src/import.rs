//! The shape every importer translates to.
//!
//! A foreign packaging -- a Modrinth `.mrpack`, a Prism/MultiMC export --
//! arrives as its own format and must leave as "a game version, a loader,
//! a set of files": exactly what `install_loader` and the syncer consume.
//! The importers (`mrpack`, `prism`) differ in how they read a pack; this
//! module is what they agree on, so a pack from anywhere installs through
//! one door and the caller never learns a second install path.
//!
//! Anything an importer cannot translate arrives in `unknown` rather than
//! being dropped: a modpack that pins a loader this launcher does not know
//! would otherwise install "successfully" as something else, and the
//! failure would surface as a broken game much later.

use std::collections::BTreeMap;

/// Which loader a pack pins. `Unknown` keeps the growth the formats
/// promise -- both Modrinth and MultiMC add ids over time -- and names the
/// id so the caller can refuse it loudly instead of installing a
/// different game than the pack describes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoaderTarget {
    Vanilla,
    Fabric { loader: String },
    Quilt { loader: String },
    Forge { loader: String },
    NeoForge { loader: String },
    Unknown { id: String, version: String },
}

/// How much one file wants each side of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    Required,
    Optional,
    Unsupported,
}

impl Support {
    fn parse(text: &str) -> Option<Self> {
        match text {
            "required" => Some(Support::Required),
            "optional" => Some(Support::Optional),
            "unsupported" => Some(Support::Unsupported),
            _ => None,
        }
    }
}

/// Per-side support for one file. A file that says nothing about sides
/// exists on both: that is what the pack formats mean by an absent `env`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Env {
    pub client: Support,
    pub server: Support,
}

impl Default for Env {
    fn default() -> Self {
        Self {
            client: Support::Required,
            server: Support::Required,
        }
    }
}

/// Which side of an archive override layer applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SideFilter {
    Both,
    ClientOnly,
    ServerOnly,
}

/// A directory inside the import archive whose contents copy over the
/// game directory. Layers apply in order: a side layer after the base
/// layer, because the formats define the side layer as overwriting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverrideLayer {
    /// The archive directory prefix, e.g. `overrides`.
    pub prefix: String,
    pub side: SideFilter,
}

/// Where one pack file's bytes come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileSource {
    /// Downloaded from one of these URLs and verified against the hashes.
    Download {
        urls: Vec<String>,
        hashes: BTreeMap<String, String>,
        size: Option<u64>,
    },
    /// Carried inside the import archive itself (an overrides entry).
    Embedded { archive_path: String },
    /// Carried on this machine already: the game content of an install
    /// being migrated. Copied, never fetched.
    Local { absolute: String },
    /// Named by catalogue id (CurseForge's projectID/fileID) rather than
    /// by URL or name: the catalogue's API resolves it to a download URL,
    /// a file name and hashes at install time, so `PackFile::path` is
    /// empty until then.
    Catalogue { project_id: u64, file_id: u64 },
}

/// One file the install must place in the game directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackFile {
    /// Relative to the game directory. Joined, never trusted. Empty for
    /// `Catalogue` sources, which learn their name at resolution.
    pub path: String,
    pub env: Env,
    pub source: FileSource,
}

/// A pack imported from a foreign packaging: the install's order form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedPack {
    /// The instance name the pack gives itself.
    pub name: String,
    /// The game version to install.
    pub game: String,
    pub loader: LoaderTarget,
    pub files: Vec<PackFile>,
    /// Archive directories that layer over the game directory.
    pub overrides: Vec<OverrideLayer>,
    /// Dependencies this importer could not translate, id -> version.
    pub unknown: BTreeMap<String, String>,
}

/// One side's view of a pack: what must be placed, and what the user may
/// still decline. Optional files are offered, never silently installed --
/// the formats say plainly that the choice belongs to the user.
#[derive(Debug, Clone)]
pub struct Selection<'a> {
    pub required: Vec<&'a PackFile>,
    pub optional: Vec<&'a PackFile>,
}

impl ImportedPack {
    /// The files one side of the game wants.
    pub fn select(&self, side: crate::installer::Side) -> Selection<'_> {
        let mut selection = Selection {
            required: Vec::new(),
            optional: Vec::new(),
        };
        for file in &self.files {
            let support = match side {
                crate::installer::Side::Client => file.env.client,
                crate::installer::Side::Server => file.env.server,
            };
            match support {
                Support::Required => selection.required.push(file),
                Support::Optional => selection.optional.push(file),
                Support::Unsupported => {}
            }
        }
        selection
    }

    /// The override layers one side applies, base layer first.
    pub fn override_layers(&self, side: crate::installer::Side) -> Vec<&OverrideLayer> {
        self.overrides
            .iter()
            .filter(|layer| {
                matches!(
                    (layer.side, side),
                    (SideFilter::Both, _)
                        | (SideFilter::ClientOnly, crate::installer::Side::Client)
                        | (SideFilter::ServerOnly, crate::installer::Side::Server)
                )
            })
            .collect()
    }
}

/// Parse one `env` value string as the formats spell it.
pub(crate) fn support_from(text: &str) -> Option<Support> {
    Support::parse(text)
}

/// Is `rel` a path that stays inside the game directory however the
/// platform reads it?
///
/// The string rules are the data root's, deliberately: path semantics
/// differ per platform (`/etc/passwd` is not absolute on Windows yet
/// still leaves the root), so the check must mean the same everywhere.
/// The `.mrpack` specification warns about exactly this -- an import zip
/// is attacker-supplied if the user downloaded one.
pub(crate) fn is_safe_relative(rel: &str) -> bool {
    !rel.is_empty()
        && !rel.starts_with(['/', '\\'])
        && !rel.contains('\\')
        && !rel.contains(':')
        && !rel
            .split('/')
            .any(|component| component == ".." || component.is_empty())
}
