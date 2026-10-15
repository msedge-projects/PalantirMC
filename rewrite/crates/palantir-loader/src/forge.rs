//! Forge and NeoForge, installed from the vendors' own installer jars.
//!
//! The Forge family is not installed the way Fabric and Quilt are. Their meta
//! services publish a launcher profile directly; Forge and NeoForge publish a
//! *jar* that carries two documents and a pipeline:
//!
//! 1. the jar itself, from the vendor's Maven, named
//!    `<artifact>-<version>-installer.jar` under the vendor's own group path;
//! 2. `install_profile.json` (`spec: 1`) and the overlay document it names in
//!    `json` — an ordinary `inheritsFrom` document, which installs through
//!    `install_document` like every other loader;
//! 3. the profile's own `libraries`, which are the *toolchain the processors
//!    run from* and must be on disk before anything runs;
//! 4. the processor plan, run as headless Java by `launch::run_processors`.
//!
//! Why this order, and why the processors are planned here but not run here:
//! step 4 needs a Java runtime, and which runtime belongs to an install is a
//! fact about the *installed* document (`javaVersion.component`) — which only
//! exists once step 2 has landed. A module that fetched a runtime would be
//! guessing the component before it could read it, so the split is at the
//! boundary that makes the guess unnecessary: this module hands back the plan,
//! and the caller resolves the runtime from `report.version` and runs it.
//!
//! Two vendor facts the code depends on, both checked against their published
//! Mavens rather than assumed:
//!
//! - the versions are `1.20.1-47.4.26` for Forge (game, then loader) and
//!   `20.6.141` for NeoForge (the game's major/minor times ten, which is why
//!   the mapping back to `1.20.6` lives here and is tested);
//! - the installer jar has **no published hash** at either vendor, so the
//!   transfer is unverified by necessity — said out loud rather than hidden
//!   behind a hash that was never fetched.

use std::path::{Path, PathBuf};

use palantir_core::rules::Platform;
use palantir_net::cache::MetadataCache;
use palantir_net::download::{DownloadOptions, download};
use palantir_net::sync::Syncer;

use crate::error::{Error, Result};
use crate::install::{InstallReport, install_document};
use crate::installer::{InstallProfile, PlannedProcessor, ProcessorContext, Side, plan_processors};

/// Which vendor's installer is being installed.
///
/// The two are near-identical in shape and different in every string: base
/// URL, group path, artifact name, and what a version number means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vendor {
    Forge,
    NeoForge,
}

impl Vendor {
    pub const ALL: [Self; 2] = [Self::Forge, Self::NeoForge];

    pub fn name(self) -> &'static str {
        match self {
            Self::Forge => "Forge",
            Self::NeoForge => "NeoForge",
        }
    }

    /// The vendor's Maven root. NeoForge publishes to its own Maven rather
    /// than to Forge's, which is why a single "the Forge Maven" assumption
    /// would 404 halfway through an install.
    pub fn maven_base(self) -> &'static str {
        match self {
            Self::Forge => "https://maven.minecraftforge.net",
            Self::NeoForge => "https://maven.neoforged.net/releases",
        }
    }

    /// The group under which the installer artifacts live, in Maven's
    /// slash-separated layout.
    pub fn group_path(self) -> &'static str {
        match self {
            Self::Forge => "net/minecraftforge/forge",
            Self::NeoForge => "net/neoforged/neoforge",
        }
    }

    /// The artifact name, which is also the installer jar's prefix.
    pub fn artifact(self) -> &'static str {
        match self {
            Self::Forge => "forge",
            Self::NeoForge => "neoforge",
        }
    }

    /// This vendor's release listing: Maven's own `maven-metadata.xml`.
    ///
    /// Maven's metadata is used rather than either vendor's website: it is the
    /// document the installer artifacts themselves are published under, so a
    /// version it lists is a version whose jar can be fetched, and it needs no
    /// scraper to read.
    pub fn metadata_url(self) -> String {
        format!(
            "{}/{}/maven-metadata.xml",
            self.maven_base(),
            self.group_path()
        )
    }
}

/// The file name one release's installer jar is published under.
pub fn installer_file_name(vendor: Vendor, version: &str) -> String {
    format!("{}-{version}-installer.jar", vendor.artifact())
}

/// Where one release's installer jar is published.
pub fn installer_url(vendor: Vendor, version: &str) -> String {
    installer_url_at(vendor.maven_base(), vendor, version)
}

/// The same vendor path under another base: a mirror, or the mock server in
/// tests.
///
/// This exists for the same reason `profiles::LoaderSources::at` does. The
/// vendor's Maven is a host this launcher cannot redirect, so without a base a
/// caller could only ever reach the real service -- which would leave the
/// installer fetch, the toolchain placement and the processor plan with no
/// offline coverage at all.
pub fn installer_url_at(base: &str, vendor: Vendor, version: &str) -> String {
    format!(
        "{}/{}/{version}/{}",
        base.trim_end_matches('/'),
        vendor.group_path(),
        installer_file_name(vendor, version)
    )
}

/// The game version a vendor's version number installs over.
///
/// Forge names both parts in one string (`1.20.1-47.4.26`), and the game is
/// whatever precedes the first dash — with `None` for the pre-1.7 generation,
/// which named only the loader (`9.11.1.1345`) and left the game to the
/// profile. NeoForge names only the loader, in the scheme its own
/// documentation defines: the release's major and minor *are* the game's, with
/// the leading `1.` back on (`20.6.141` -> `1.20.6`), and a minor of `0`
/// belonging to the `.0` game release (`21.0.167` -> `1.21`).
///
/// A number that does not follow the vendor's scheme maps to nothing rather
/// than to a guess: installing a pack's `1.20.1` collection over whatever a
/// misread version number suggested is a wrong install whose failure would
/// surface somewhere else entirely.
pub fn game_version(vendor: Vendor, version: &str) -> Option<String> {
    match vendor {
        Vendor::Forge => version
            .split_once('-')
            .map(|(game, _)| game)
            .filter(|game| !game.is_empty())
            .map(str::to_string),
        Vendor::NeoForge => {
            let mut parts = version.split('.');
            let major: u32 = parts.next()?.parse().ok()?;
            let minor: u32 = parts.next()?.parse().ok()?;
            // The game's own ids moved to a year-and-release scheme, and
            // NeoForge's leading two numbers follow them: `26.1`, `26.2` and
            // `26.3` are real ids in the version manifest (checked
            // 2026-10-12) while `1.26` is not one at all, so the pair *is*
            // the game rather than a number to put `1.` in front of. Nothing
            // exists between the two schemes to be ambiguous about: this
            // vendor ships 20.x and 21.x, then 26.x.
            if major >= 26 {
                return Some(format!("{major}.{minor}"));
            }
            // Below that, NeoForge's numbering starts at 20.2 (game 1.20.2);
            // anything under it is not a release of theirs.
            if major < 20 {
                return None;
            }
            Some(if minor == 0 {
                format!("1.{major}")
            } else {
                format!("1.{major}.{minor}")
            })
        }
    }
}

/// The releases a vendor's listing publishes for one game version, newest
/// first.
///
/// The order is the releases' own numbers, not the document's. Maven's
/// metadata is written in deploy order, and neither vendor's is a release
/// order: Forge's own file opens its 1.20.1 run *descending* (`47.4.5`,
/// `47.4.4`, `47.4.3`, ...) and thousands of lines later carries `47.4.0`
/// through `47.4.26` ascending, while its 1.21 run starts at `51.0.33` and
/// descends (both checked against the live file on 2026-10-12). Reading the
/// document as "last is newest" therefore picks an old release for 1.21 and
/// happens to pick the right one for 1.20.1 — this list feeds the desktop's
/// automatic choice, so it is compared numerically instead.
///
/// `pick_loader` takes Fabric's own ordering as the strongest signal because
/// that listing really is ordered; this one provably is not, which is the
/// difference worth writing down.
pub fn versions_for_game(listing_xml: &str, vendor: Vendor, game: &str) -> Vec<String> {
    let mut versions = version_elements(listing_xml)
        .into_iter()
        .filter(|version| game_version(vendor, version).as_deref() == Some(game))
        .collect::<Vec<_>>();
    // A stable sort: two versions that compare equal keep the document's own
    // order, so the result is deterministic whatever the file looks like.
    versions.sort_by_key(|version| std::cmp::Reverse(release_order(vendor, version)));
    versions
}

/// A release's own numbers, as the key two of them are compared by.
///
/// Forge keeps the game in front of the loader (`1.20.1-47.4.26`), so the
/// loader's numbers are what follows the first dash; NeoForge's whole version
/// is its own numbering. Anything from a `-` or a `+` on is a label
/// (`-beta`, `+snapshot-6`) rather than a number, and a component that will
/// not parse reads as zero so that a comparison always has an answer — the
/// alternative, refusing to order the list, would leave a player with a
/// picker that cannot say which release is newer.
fn release_order(vendor: Vendor, version: &str) -> Vec<u64> {
    let numbering = match vendor {
        Vendor::Forge => version
            .split_once('-')
            .map(|(_, loader)| loader)
            .unwrap_or(version),
        Vendor::NeoForge => version,
    };
    let head = numbering.split(['-', '+']).next().unwrap_or("");
    head.split('.')
        .map(|part| part.parse().unwrap_or(0))
        .collect()
}

/// The text of every `<version>` element, in document order.
///
/// A scan rather than an XML dependency: the document is Maven's own
/// generated metadata, whose shape is fixed and whose only element this
/// launcher reads is the version list. Matching the element with its closing
/// bracket matters — `<versions>` opens the list that contains them.
fn version_elements(listing_xml: &str) -> Vec<String> {
    const OPEN: &str = "<version>";
    const CLOSE: &str = "</version>";
    let mut out = Vec::new();
    let mut rest = listing_xml;
    while let Some(start) = rest.find(OPEN) {
        let after = &rest[start + OPEN.len()..];
        let Some(end) = after.find(CLOSE) else {
            rest = after;
            continue;
        };
        let version = after[..end].trim();
        if !version.is_empty() {
            out.push(version.to_string());
        }
        rest = &after[end + CLOSE.len()..];
    }
    out
}

/// What to install: which vendor's release, over which game directory.
#[derive(Debug, Clone, Copy)]
pub struct VendorInstall<'a> {
    pub vendor: Vendor,
    pub version: &'a str,
    /// Only for asset indexes that write into a game directory.
    pub game_dir: Option<&'a Path>,
    /// The Maven base the installer jar is fetched from. `None` is the
    /// vendor's own published service, which is what a launcher normally wants;
    /// a test points it at the mock server that stands in for that service.
    pub base: Option<&'a str>,
}

/// An installed vendor release whose processor pipeline has not run yet.
///
/// `processors` is the plan, not a report: nothing has been executed when this
/// is returned. The caller resolves a Java runtime from `report.version` and
/// hands both to `launch::run_processors`.
#[derive(Debug)]
pub struct StagedInstall {
    pub report: InstallReport,
    /// The installer jar, kept because two of the plan's arguments name it and
    /// because the profile's packaged `data` files are staged beside it.
    pub installer: PathBuf,
    pub processors: Vec<PlannedProcessor>,
}

impl StagedInstall {
    /// Does this install need headless Java to finish?
    ///
    /// Every generation this launcher installs does; a profile with no
    /// processors is a shape that would silently skip the patched artifacts
    /// the overlay's libraries name, so the caller says so rather than
    /// treating an empty plan as success.
    pub fn needs_processors(&self) -> bool {
        !self.processors.is_empty()
    }
}

/// Install one vendor release into the data root the syncer owns.
///
/// `into` is where the installer jar is staged (the caller's version
/// directory is the natural choice); the profile's packaged `data` files are
/// staged beside it by the planner, the way the vendor's own installer does
/// into a temporary directory that dies with it.
///
/// The processors are planned with `{MINECRAFT_JAR}` pointing at the jar the
/// *installed* document names: the resolved document keeps its own copy of the
/// game's client bytes at its own version id, byte for byte what the vendor's
/// processor arguments expect to patch.
pub fn install(
    syncer: &Syncer<'_>,
    cache: &MetadataCache,
    manifest_url: &str,
    request: VendorInstall<'_>,
    platform: &Platform,
    into: &Path,
) -> Result<StagedInstall> {
    let base = request.base.unwrap_or_else(|| request.vendor.maven_base());
    let installer = fetch_installer(syncer, base, request.vendor, request.version, into)?;
    let (profile, overlay) = installer_documents(&installer)?;
    let report = install_document(
        syncer,
        cache,
        manifest_url,
        platform,
        &overlay,
        request.game_dir,
    )?;
    place_toolchain(syncer, &profile, platform)?;
    let processors = plan_processors(
        &profile,
        &ProcessorContext {
            root: &syncer.root.root,
            library_dir: &syncer.root.libraries_dir(),
            minecraft_jar: &syncer.root.version_jar(&report.version.id),
            installer: &installer,
            side: Side::Client,
        },
    )?;
    Ok(StagedInstall {
        report,
        installer,
        processors,
    })
}

/// Where one release's installer jar is staged under `into`.
pub fn installer_path(into: &Path, vendor: Vendor, version: &str) -> PathBuf {
    into.join(installer_file_name(vendor, version))
}

/// Fetch the installer jar into `into`.
///
/// Unverified by necessity: neither vendor publishes a hash for the installer
/// jar itself (the classes inside are hashed, the archive is not), so passing
/// `None` here is the honest state of the metadata rather than a check that
/// was skipped.
fn fetch_installer(
    syncer: &Syncer<'_>,
    base: &str,
    vendor: Vendor,
    version: &str,
    into: &Path,
) -> Result<PathBuf> {
    std::fs::create_dir_all(into).map_err(|source| Error::Io {
        path: into.to_path_buf(),
        source,
    })?;
    let path = installer_path(into, vendor, version);
    download(
        syncer.http,
        &installer_url_at(base, vendor, version),
        &path,
        None,
        None,
        &DownloadOptions::default(),
    )?;
    Ok(path)
}

/// Both documents the installer jar carries: the profile, and the overlay the
/// profile names in `json` (written as a jar path with a leading slash, which
/// is the vendor's build tree showing through).
fn installer_documents(installer: &Path) -> Result<(InstallProfile, String)> {
    let file = std::fs::File::open(installer).map_err(|source| Error::Io {
        path: installer.to_path_buf(),
        source,
    })?;
    let mut archive = zip::ZipArchive::new(file).map_err(|source| Error::Io {
        path: installer.to_path_buf(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, source),
    })?;

    let found = read_entry(&mut archive, "install_profile.json");
    let profile_text = found.ok_or_else(|| Error::Invalid {
        what: "Forge-family installer",
        why: format!("{} carries no install_profile.json", installer.display()),
    })?;
    let profile = InstallProfile::parse(&profile_text)?;
    if profile.spec != 1 {
        return Err(Error::Invalid {
            what: "install profile",
            why: format!("spec {} is not the 1 this launcher installs", profile.spec),
        });
    }

    let named = profile
        .json
        .as_deref()
        .unwrap_or("version.json")
        .trim_start_matches('/');
    let overlay = read_entry(&mut archive, named)
        .or_else(|| read_entry(&mut archive, "version.json"))
        .ok_or_else(|| Error::Invalid {
            what: "Forge-family installer",
            why: format!(
                "{} carries neither {named} nor version.json",
                installer.display()
            ),
        })?;
    Ok((profile, overlay))
}

/// One entry of an open archive as text, or `None` when it is not there.
fn read_entry(archive: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Option<String> {
    let mut entry = archive.by_name(name).ok()?;
    let mut text = String::new();
    std::io::Read::read_to_string(&mut entry, &mut text).ok()?;
    Some(text)
}

/// Place the profile's own libraries: the toolchain the processors run from.
///
/// This is not the version's classpath — that lands with the overlay's own
/// sync — and it has to be here before the first processor starts, or
/// binpatching fails with a missing class rather than with anything that names
/// the install.
fn place_toolchain(
    syncer: &Syncer<'_>,
    profile: &InstallProfile,
    platform: &Platform,
) -> Result<()> {
    for library in &profile.libraries {
        let Some(resolved) = library.resolve(platform)? else {
            continue;
        };
        let Some(file) = resolved.artifact else {
            continue;
        };
        let path = syncer.root.library_file(&file.rel_path)?;
        // No promised hash means nothing to re-verify, so a file already there
        // is the one this launcher placed.
        if file.sha1.is_none() && path.is_file() {
            continue;
        }
        let url = file.url.clone().ok_or_else(|| Error::Invalid {
            what: "install profile",
            why: format!("toolchain library {} names no download URL", file.rel_path),
        })?;
        download(
            syncer.http,
            &url,
            &path,
            file.sha1.as_deref(),
            file.size,
            &DownloadOptions::default(),
        )?;
    }
    Ok(())
}
