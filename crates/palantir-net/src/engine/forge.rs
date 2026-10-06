//! The two Forge-shaped loaders' own installers: Forge and NeoForge.
//!
//! Why an installer and not a document: neither loader serves a launch profile
//! the way Fabric and Quilt do ([`Loader::profile_url`] answers `None` for
//! them, and says why). What each publishes per build is an installer jar on
//! its own maven, and a launcher is expected to open that jar: `version.json`
//! inside it is the launch profile -- main class, libraries, arguments, in the
//! shape `palantir-core`'s `VersionFile` already models -- and
//! `install_profile.json` beside it is the install itself -- `data` (names for
//! files the processors read and write), `processors` (the tools that patch the
//! client jar and unpack the maven artifacts, which running is G100's work),
//! and the game version the build is for.
//!
//! Prism's copy of either profile is a rewrite of that file around ForgeWrapper,
//! a third-party project that runs those processors at launch instead: the
//! mirror's main class is ForgeWrapper's, its library list swaps the loader's
//! own artifact for the wrapper, and its `minecraftArguments` string is the
//! standard prefix plus the loader's extras. What this module reads is the
//! publisher's file, so those three differences are the measurement, not a
//! mismatch -- the live test asserts each of them by name.
//!
//! ## How far back the reading goes
//!
//! Installers carrying `version.json` are read directly. Older Forge builds
//! carry the profile as a `versionInfo` object inside `install_profile.json`
//! and no `version.json`; those are read from that key instead. Older still --
//! a profile that is only a `minecraftArguments` string -- is refused by name,
//! because a string from that era names tweaker classes this launcher does not
//! run and libraries on repositories that no longer answer.
//!
//! ## What the translation does
//!
//! `version.json` is Mojang's argument shape (`arguments.game`/`arguments.jvm`)
//! where this launcher's model reads the legacy string (`minecraftArguments`).
//! The translation joins the plain strings of `arguments.game` into that
//! string; conditional entries (objects with `rules`, which this era of Forge
//! does not publish but a future one might) are dropped rather than guessed
//! at. `arguments.jvm` is dropped for the reason the mirror drops it: its
//! tokens (`${classpath_separator}`, a module path) are ones no launch of this
//! launcher fills, and ForgeWrapper rebuilds that path at launch instead. A
//! `minecraftArguments` string already present (the `versionInfo` era) is kept
//! as is.
//!
//! One key is added rather than reshaped. Neither publisher's `version.json`
//! names a `mainJar`: `VersionFile::parse` builds a Mojang client coordinate out
//! of `id` when the key is absent -- and `1.21.1-forge-52.1.0` is not a version
//! Mojang serves a client for -- which lands as the error a launch refuses on.
//! The client the installer's own processors write is named in its `data` as
//! `PATCHED`, so the translation takes the main jar from there when the file
//! names none (G119), and leaves a file that names one alone.
//!
//! ## Where the bytes come from
//!
//! Every jar here travels through the content store, digest-checked against
//! the `.sha1` sidecar its maven publishes: the installer jar itself (whose
//! sidecar the loader's maven serves beside it) and every processor tool G100
//! resolves. An installer names no digest for any of them, and the sidecar is
//! the publisher's own word for what the bytes should be. The store is also
//! what makes a stalled host survivable: Forge's maven stalls a single slow
//! connection -- a 6MB installer outlasts one request timeout -- and the
//! store's staging file is where the next attempt continues rather than
//! restarts. The sidecars travel through the engine's metadata cache,
//! believed for [`crate::engine::cache::IMMUTABLE_TTL`] like a version file,
//! because a released build's bytes do not move; a jar whose maven states no
//! sidecar falls back to the cache's answer.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use palantir_core::version::{GradleSpecifier, VersionFile};

use crate::engine::cache::{MetadataCache, IMMUTABLE_TTL};
use crate::engine::cancel::Cancel;
use crate::engine::content::{ContentStore, Digest};
use crate::engine::download::{fetch_to_file, Download};
use crate::engine::loaders::Loader;
use crate::engine::piston::PistonMeta;
use crate::engine::request::Fetch;
use crate::engine::retry::Backoff;
use crate::Error;

/// Where Forge publishes its installer jars.
pub const FORGE_MAVEN: &str = "https://maven.minecraftforge.net/";
/// Where NeoForge publishes its installer jars.
pub const NEOFORGE_MAVEN: &str = "https://maven.neoforged.net/releases/";
/// The central repository, which hosts the tools either loader's processors
/// run on (jopt-simple, ASM, gson) when the loader's own maven does not.
pub const CENTRAL_MAVEN: &str = "https://repo1.maven.org/maven2/";
/// Mojang's libraries, for the mappings and deobfuscation artifacts the
/// processors name that neither loader mirrors.
pub const MOJANG_LIBRARIES: &str = "https://libraries.minecraft.net/";

/// The component uid `resolve` files the translated profile under.
///
/// The publisher's file names no component -- Mojang's files do not either, so
/// [`crate::engine::piston::PistonMeta::translated`] takes the uid the same
/// way. These are the uids the mirror serves the same builds under, which is
/// what makes the live test's comparison a comparison of the same component.
pub fn component_uid(loader: Loader) -> &'static str {
    match loader {
        Loader::Forge => "net.minecraftforge",
        Loader::NeoForge => "net.neoforged",
        Loader::Fabric | Loader::Quilt => "net.minecraft",
    }
}

/// Where one build's installer jar is.
///
/// `None` for Fabric and Quilt, for the same reason
/// [`Loader::profile_url`] is: their launch profile is a document their
/// service serves, so there is no installer to name.
pub fn installer_url(loader: Loader, game: &str, build: &str) -> Option<String> {
    match loader {
        Loader::Forge => {
            // A component version may carry the game in front of the build.
            // Prism writes Forge that way (`1.21.1-52.1.0`) and an instance
            // imported from it keeps the spelling, while this launcher's create
            // flow writes the promotions' `52.1.0`; the URL is
            // `forge-{game}-{build}-installer.jar` and must not repeat the game
            // either way. NeoForge spells the build alone in both.
            let prefix = format!("{game}-");
            let build = build.strip_prefix(prefix.as_str()).unwrap_or(build);
            Some(format!(
                "{FORGE_MAVEN}net/minecraftforge/forge/{game}-{build}/forge-{game}-{build}-installer.jar"
            ))
        }
        Loader::NeoForge => Some(format!(
            "{NEOFORGE_MAVEN}net/neoforged/neoforge/{build}/neoforge-{build}-installer.jar"
        )),
        Loader::Fabric | Loader::Quilt => None,
    }
}

/// One `data` entry of `install_profile.json`, client side.
///
/// A value is either a bracketed maven coordinate (`[net.minecraft:...]`),
/// an installer-relative path (`/data/client.lzma`), or a quoted literal
/// (`'2a8064cf...'`); anything else is passed through as text, because a name
/// the processors only ever compare is not worth refusing an install over. An
/// entry given per side (`{"client": ..., "server": ...}`) is read client
/// side: this launcher installs a client, and a server install is future work
/// the parser names rather than half-supports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataValue {
    /// A maven coordinate to resolve, without its brackets.
    Artifact(String),
    /// A file inside the installer jar.
    Path(String),
    /// Text, quotes stripped.
    Literal(String),
}

/// One `processors[]` entry: a tool to run, in order.
///
/// `sides` is empty for a processor that runs on both sides and names the one
/// side otherwise; a client install skips the server-only ones. `outputs` maps
/// an output path to the digest it must have -- an output already present and
/// matching is the resume case, and G100 skips the processor for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Processor {
    /// The tool's maven coordinate.
    pub jar: String,
    /// The tool's classpath, as maven coordinates.
    pub classpath: Vec<String>,
    /// The tool's arguments, naming `{data}` entries and `{TOKENS}`.
    pub args: Vec<String>,
    /// Output paths to digests; empty for a processor that only extracts.
    pub outputs: BTreeMap<String, String>,
    /// The sides this processor runs on; empty means both.
    pub sides: Vec<String>,
}

/// The install half of an installer jar: what G100 runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallSpec {
    /// The game version the build is for (`install_profile.json`'s
    /// `minecraft`), which is also `{MINECRAFT_VERSION}`.
    pub minecraft: String,
    /// The installer’s own version string.
    pub version: String,
    /// The install-time libraries: the tool dependencies the processors run
    /// on, resolved before any of them starts.
    pub libraries: Vec<String>,
    /// The `data` entries, client side.
    pub data: BTreeMap<String, DataValue>,
    /// The processors, in the order to run them.
    pub processors: Vec<Processor>,
}

/// An installer jar, opened: the profile for G99 and the install for G100.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedInstaller {
    /// `version.json`, or the `versionInfo` object of older builds, before
    /// translation (no `uid`, Mojang's argument shape).
    pub version_json: serde_json::Value,
    /// The install half.
    pub install: InstallSpec,
}

/// Read one entry of a zip by name.
///
/// A missing entry is `None` rather than an error, because `version.json` is
/// absent on exactly the older builds `versionInfo` covers -- the caller
/// decides what its absence means.
fn zip_entry(bytes: &[u8], origin: &str, name: &str) -> Result<Option<Vec<u8>>, Error> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|error| {
        Error::format(origin, format!("{origin} is not a jar: {error}"))
    })?;
    let found = archive.by_name(name).is_ok();
    if !found {
        return Ok(None);
    }
    let mut entry = archive.by_name(name).map_err(|error| {
        Error::format(origin, format!("{origin} has no readable {name}: {error}"))
    })?;
    let mut out = Vec::new();
    entry.read_to_end(&mut out).map_err(|error| {
        Error::format(origin, format!("{origin}'s {name} could not be read: {error}"))
    })?;
    Ok(Some(out))
}

/// Classify one `data` value, after the client side has been picked.
fn data_value(raw: &str) -> DataValue {
    let text = raw.trim();
    if text.len() >= 2 && text.starts_with('\'') && text.ends_with('\'') {
        DataValue::Literal(text[1..text.len() - 1].to_string())
    } else if text.starts_with('[') && text.ends_with(']') && text.len() >= 2 {
        DataValue::Artifact(text[1..text.len() - 1].to_string())
    } else if text.starts_with('/') {
        DataValue::Path(text.to_string())
    } else {
        DataValue::Literal(text.to_string())
    }
}

/// Pick the client side of a `data` value, which is either a string or a
/// `{"client": ..., "server": ...}` object.
fn client_side(value: &serde_json::Value, origin: &str, name: &str) -> Result<DataValue, Error> {
    if let Some(text) = value.as_str() {
        return Ok(data_value(text));
    }
    let picked = value
        .get("client")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            Error::format(origin, format!("{origin}'s data entry '{name}' has no client side"))
        })?;
    Ok(data_value(picked))
}

/// The string array at `key` of `object`, or empty when the key is absent.
///
/// A present-but-not-an-array key is an error naming it: a processor whose
/// classpath is an object is a file this launcher should not guess at.
fn string_array(
    object: &serde_json::Map<String, serde_json::Value>,
    origin: &str,
    what: &str,
) -> Result<Vec<String>, Error> {
    let Some(value) = object.get(what) else {
        return Ok(Vec::new());
    };
    let entries = value.as_array().ok_or_else(|| {
        Error::format(origin, format!("{origin}'s {what} is not an array"))
    })?;
    let mut out = Vec::with_capacity(entries.len());
    for entry in entries {
        let text = entry.as_str().ok_or_else(|| {
            Error::format(origin, format!("{origin}'s {what} holds something that is not a string"))
        })?;
        out.push(text.to_string());
    }
    Ok(out)
}

/// Open an installer jar: the profile and the install.
///
/// `origin` is the URL the bytes came from, and names every failure: a jar
/// that cannot be opened is only debuggable if the error says which build it
/// was supposed to be.
pub fn parse_installer(bytes: &[u8], origin: &str) -> Result<ParsedInstaller, Error> {
    let profile_bytes = zip_entry(bytes, origin, "version.json")?;
    let install_bytes = zip_entry(bytes, origin, "install_profile.json")?.ok_or_else(|| {
        Error::format(origin, format!("{origin} has no install_profile.json"))
    })?;
    let install_text = String::from_utf8_lossy(&install_bytes).into_owned();
    let install: serde_json::Value = serde_json::from_str(&install_text)
        .map_err(|error| Error::json(origin, error.to_string()))?;
    let install_object = install.as_object().ok_or_else(|| {
        Error::format(origin, format!("{origin}'s install_profile.json is not an object"))
    })?;

    let version_json = match profile_bytes {
        Some(body) => {
            let text = String::from_utf8_lossy(&body).into_owned();
            serde_json::from_str(&text).map_err(|error| Error::json(origin, error.to_string()))?
        }
        None => install_object
            .get("versionInfo")
            .cloned()
            .filter(|info| info.as_object().is_some_and(|obj| obj.contains_key("mainClass")))
            .ok_or_else(|| {
                Error::format(
                    origin,
                    format!(
                        "{origin} carries neither version.json nor a versionInfo profile: \
                         installers of that age are refused by name"
                    ),
                )
            })?,
    };

    let mut data = BTreeMap::new();
    if let Some(entries) = install_object.get("data").and_then(serde_json::Value::as_object) {
        for (name, value) in entries {
            data.insert(name.clone(), client_side(value, origin, name)?);
        }
    }
    let mut processors = Vec::new();
    if let Some(entries) = install_object.get("processors").and_then(serde_json::Value::as_array) {
        for entry in entries {
            let object = entry.as_object().ok_or_else(|| {
                Error::format(origin, format!("{origin} has a processor that is not an object"))
            })?;
            let jar = object
                .get("jar")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    Error::format(origin, format!("{origin} has a processor with no jar"))
                })?
                .to_string();
            let mut outputs = BTreeMap::new();
            if let Some(declared) = object.get("outputs").and_then(serde_json::Value::as_object) {
                for (path, digest) in declared {
                    let hex = digest.as_str().ok_or_else(|| {
                        Error::format(
                            origin,
                            format!("{origin}'s processor '{jar}' declares an output that is not a string"),
                        )
                    })?;
                    outputs.insert(path.clone(), hex.to_string());
                }
            }
            processors.push(Processor {
                jar,
                classpath: string_array(object, origin, "classpath")?,
                args: string_array(object, origin, "args")?,
                outputs,
                sides: string_array(object, origin, "sides")?,
            });
        }
    }

    let mut libraries = Vec::new();
    if let Some(entries) = install_object.get("libraries").and_then(serde_json::Value::as_array) {
        for entry in entries {
            let coord = entry
                .get("name")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    Error::format(origin, format!("{origin} has an install library with no name"))
                })?;
            libraries.push(coord.to_string());
        }
    }

    let text_of = |key: &str| {
        install_object.get(key).and_then(serde_json::Value::as_str).unwrap_or_default().to_string()
    };
    Ok(ParsedInstaller {
        version_json,
        install: InstallSpec {
            minecraft: text_of("minecraft"),
            version: text_of("version"),
            libraries,
            data,
            processors,
        },
    })
}

/// Translate one installer's `version.json` into the shape this launcher's
/// model reads, ready for [`VersionFile::parse`].
///
/// The component id and version are the caller's: the publisher's file names
/// the game build in `id` and carries no component id, and the caller is the
/// one that knows which uid it asked for -- the same split
/// [`crate::engine::piston::PistonMeta::translated`] makes.
pub fn translate_profile(
    version_json: &serde_json::Value,
    uid: &str,
    version: &str,
) -> Result<serde_json::Value, String> {
    let obj = version_json
        .as_object()
        .ok_or_else(|| "a launch profile is an object".to_string())?;
    let mut out = obj.clone();
    // The game's arguments in whichever shape this build published them: the
    // legacy string is kept, and the plain strings of `arguments.game` become
    // it. Conditional entries are dropped -- see the module doc -- and so is
    // `arguments.jvm`, whose tokens no launch fills.
    let has_legacy = out
        .get("minecraftArguments")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|text| !text.is_empty());
    if !has_legacy {
        let game: Vec<String> = obj
            .get("arguments")
            .and_then(|arguments| arguments.get("game"))
            .and_then(serde_json::Value::as_array)
            .map(|entries| {
                entries.iter().filter_map(serde_json::Value::as_str).map(str::to_string).collect()
            })
            .unwrap_or_default();
        if !game.is_empty() {
            out.insert(
                "minecraftArguments".to_string(),
                serde_json::Value::String(game.join(" ")),
            );
        }
    }
    out.remove("arguments");
    if let Some(id) = obj.get("id").and_then(serde_json::Value::as_str) {
        if !out.contains_key("name") {
            out.insert("name".to_string(), serde_json::Value::String(id.to_string()));
        }
    }
    out.insert("uid".to_string(), serde_json::Value::String(uid.to_string()));
    out.insert("version".to_string(), serde_json::Value::String(version.to_string()));
    Ok(serde_json::Value::Object(out))
}

/// The two Forge-shaped loaders' own installers, over the engine's cache.
///
/// One cache directory for every installer jar and maven sidecar, because the
/// entries are told apart by URL and a released build's bytes do not move.
/// The fetch seam is the engine's own pool in production and a scripted server
/// in a test.
pub struct InstallerMeta {
    /// Where an installer jar lives once it has been fetched, believed for a
    /// year: it describes something already released, like a version file.
    documents: MetadataCache,
    /// The way bytes arrive.
    fetch: Arc<dyn Fetch>,
}

/// The directory and nothing else: the fetch seam has no `Debug` of its own.
impl std::fmt::Debug for InstallerMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstallerMeta").field("dir", &self.documents.dir()).finish_non_exhaustive()
    }
}

impl InstallerMeta {
    /// A handle over `cache`, fetching through `fetch`.
    pub fn new(cache: MetadataCache, fetch: Arc<dyn Fetch>) -> InstallerMeta {
        let documents = cache.with_ttl(IMMUTABLE_TTL);
        InstallerMeta { documents, fetch }
    }

    /// The directory bodies are cached in.
    pub fn cache_dir(&self) -> &std::path::Path {
        self.documents.dir()
    }

    /// The installer jar's bytes for one build.
    ///
    /// Through the content store under the digest the installer jar's own
    /// `.sha1` sidecar states -- resumed across attempts, verified on the
    /// way in -- or through the metadata cache when the maven states none.
    ///
    /// Blocking, like every engine call.
    pub fn installer_bytes(
        &self,
        loader: Loader,
        game: &str,
        build: &str,
        store: &ContentStore,
        cancel: &Cancel,
        backoff: &Backoff,
    ) -> Result<Vec<u8>, Error> {
        let Some(url) = installer_url(loader, game, build) else {
            return Err(Error::format(
                PathBuf::from(loader.name()),
                format!(
                    "{} publishes no installer to read: its launch profile is a document",
                    loader.name()
                ),
            ));
        };
        let sidecar = format!("{url}.sha1");
        if let Some(digest) = maven_sha1(self, &sidecar, cancel, backoff)? {
            store.fetch_blocking(self.fetch.as_ref(), &url, &digest, cancel, backoff)?;
            return store.read(&digest);
        }
        let cached = self.documents.get(&url, self.fetch.as_ref(), cancel, backoff)?;
        Ok(cached.body)
    }

    /// An installer jar, opened: its bytes and its parsed profile and install.
    ///
    /// The bytes come back because G100 needs them again -- the `/data/...`
    /// files the processors read live in the same jar -- and fetching the
    /// jar twice for one install would be the waste the cache exists to stop.
    ///
    /// Blocking, like every engine call.
    pub fn parsed(
        &self,
        loader: Loader,
        game: &str,
        build: &str,
        store: &ContentStore,
        cancel: &Cancel,
        backoff: &Backoff,
    ) -> Result<(Vec<u8>, ParsedInstaller), Error> {
        let Some(url) = installer_url(loader, game, build) else {
            return Err(Error::format(
                PathBuf::from(loader.name()),
                format!(
                    "{} publishes no installer to read: its launch profile is a document",
                    loader.name()
                ),
            ));
        };
        let bytes = self.installer_bytes(loader, game, build, store, cancel, backoff)?;
        let parsed = parse_installer(&bytes, &url)?;
        Ok((bytes, parsed))
    }

    /// The launch profile of one build, read out of its installer jar and
    /// translated into the shape `resolve` merges.
    ///
    /// A loader with no installer to name is an error naming that, rather
    /// than an empty file the resolver would merge and then blame on the
    /// instance -- the same function [`Loader::profile_url`]'s `None` serves
    /// in [`crate::engine::loaders::LoaderMeta::profile`].
    ///
    /// Blocking, like every engine call.
    pub fn profile(
        &self,
        loader: Loader,
        game: &str,
        build: &str,
        store: &ContentStore,
        cancel: &Cancel,
        backoff: &Backoff,
    ) -> Result<VersionFile, palantir_core::error::Error> {
        let Some(url) = installer_url(loader, game, build) else {
            return Err(palantir_core::error::Error::format(
                PathBuf::from(loader.name()),
                format!(
                    "{} publishes no installer to read: its launch profile is a document",
                    loader.name()
                ),
            ));
        };
        let (_, parsed) =
            self.parsed(loader, game, build, store, cancel, backoff).map_err(Error::into_core)?;
        let uid = component_uid(loader);
        let mut translated = translate_profile(&parsed.version_json, uid, build)
            .map_err(|detail| palantir_core::error::Error::json(&url, detail))?;
        // Neither publisher's `version.json` names a main jar: both leave the
        // client to the launch wrapper this launcher does not run (G107), so a
        // parse of the file alone falls back to a Mojang coordinate built out
        // of `id` -- and `1.21.1-forge-52.1.0` is not a version Mojang serves a
        // client for -- which lands as the error a launch refuses on. The
        // installer's own `PATCHED` entry is the client its processors write
        // (G100), so naming it as the main jar is what makes the translated
        // profile launchable here; a file that does name one is left alone.
        if let Some(object) = translated.as_object_mut() {
            if !object.contains_key("mainJar") {
                if let Some(DataValue::Artifact(coord)) = parsed.install.data.get("PATCHED") {
                    object.insert("mainJar".to_string(), serde_json::json!({ "name": coord }));
                }
            }
        }
        VersionFile::parse(&translated, &PathBuf::from(&url), false)
    }
}

/// The maven roots tried for one loader's artifacts, in order.
///
/// The loader's own maven first: it hosts the installer tools and usually
/// mirrors the rest. Central second, for the third-party tools (jopt-simple,
/// ASM, gson) either loader runs on. Mojang's libraries last, for the
/// mappings and deobfuscation artifacts neither loader mirrors.
pub fn maven_roots(loader: Loader) -> [&'static str; 3] {
    match loader {
        Loader::Forge => [FORGE_MAVEN, CENTRAL_MAVEN, MOJANG_LIBRARIES],
        Loader::NeoForge => [NEOFORGE_MAVEN, CENTRAL_MAVEN, MOJANG_LIBRARIES],
        Loader::Fabric | Loader::Quilt => [CENTRAL_MAVEN, MOJANG_LIBRARIES, FORGE_MAVEN],
    }
}

/// The side a client install runs.
///
/// Processors with empty `sides` run on both; the rest name theirs. A server
/// install is the same walk with `"server"`, which is future work the parser
/// already reads.
pub const CLIENT_SIDE: &str = "client";

/// Where one processor ran, and whether it had to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledProcessor {
    /// The processor's index in `processors[]`.
    pub index: usize,
    /// The processor's maven coordinate.
    pub jar: String,
    /// True when the server-only processor was skipped, or every output was
    /// already present and matching.
    pub skipped: bool,
}

/// Everything a Forge-shaped install writes, and runs with.
///
/// `minecraft_jar` is the caller's contract to keep: Mojang's client jar,
/// fetched and digest-checked through `piston.rs` and the content store --
/// this module never re-downloads it under another name, and a processor
/// reading any other jar would patch the wrong game. `java` is a runtime the
/// caller located (see [`find_java`]); the install fails before any processor
/// runs when it is missing rather than halfway through one.
pub struct InstallCtx<'a> {
    /// The installer metadata, over the engine's cache and client.
    pub meta: &'a InstallerMeta,
    /// The content store processor jars are digest-checked into.
    pub store: &'a ContentStore,
    /// Where artifacts land in maven layout; `{LIBRARY_DIR}`.
    pub library_dir: &'a Path,
    /// The instance's game directory; `{ROOT}` and the processors' cwd.
    pub root: &'a Path,
    /// Where the installer jar was written; `{INSTALLER}`.
    pub installer_path: &'a Path,
    /// The installer jar's bytes, for the `/data/...` files processors read.
    pub installer_bytes: &'a [u8],
    /// Mojang's client jar; `{MINECRAFT_JAR}`.
    pub minecraft_jar: &'a Path,
    /// Scratch directory the `/data/...` files are extracted into.
    pub extract_dir: &'a Path,
    /// The `java` binary processors run on.
    pub java: &'a Path,
    /// Which side is being installed (`client` today).
    pub side: &'a str,
    /// The game version (`install_profile.json`'s `minecraft`);
    /// `{MINECRAFT_VERSION}`.
    pub game: &'a str,
}

/// Read the `.sha1` sidecar maven publishes beside an artifact.
///
/// `None` is a 404 -- a root that does not host this file -- so the caller
/// tries the next root. Any other failure is the failure, because a host that
/// answers 500 for a sidecar will answer worse for the jar. The sidecar body
/// is read to the first whitespace-separated token: some hosts append the
/// file name after the digest.
pub fn maven_sha1(
    meta: &InstallerMeta,
    sidecar_url: &str,
    cancel: &Cancel,
    backoff: &Backoff,
) -> Result<Option<Digest>, Error> {
    match meta.documents.get(sidecar_url, meta.fetch.as_ref(), cancel, backoff) {
        Ok(held) => {
            let text = String::from_utf8_lossy(&held.body).into_owned();
            let hex = text.split_whitespace().next().unwrap_or_default();
            match Digest::parse(hex) {
                Ok(digest) if digest.kind() == "sha1" => Ok(Some(digest)),
                _ => Err(Error::format(
                    PathBuf::from(sidecar_url),
                    format!("{sidecar_url} does not name a sha1: {hex:?}"),
                )),
            }
        }
        Err(Error::Http { status: Some(404), .. }) => Ok(None),
        Err(other) => Err(other),
    }
}

/// Resolve a maven coordinate to its file under `library_dir`.
///
/// The coordinate may arrive bracketed (`[group:artifact:version]`); the
/// brackets are the install profile's, not maven's. The digest comes from the
/// first root whose `.sha1` sidecar answers, and the jar travels through the
/// content store under it -- verified on the way in, deduplicated across
/// installs. A file already at the destination with the same digest is the
/// resume case and costs no request. A coordinate no root states a digest for
/// is fetched on trust into place: refusing it would refuse installs over
/// missing metadata, and the processor outputs verified afterwards are the
/// backstop -- a tampered tool produces the wrong products and the install
/// fails there instead of launching them.
pub fn artifact_path(
    meta: &InstallerMeta,
    store: &ContentStore,
    library_dir: &Path,
    coord: &str,
    roots: &[&str],
    cancel: &Cancel,
    backoff: &Backoff,
) -> Result<PathBuf, Error> {
    let bare = coord.trim().trim_start_matches('[').trim_end_matches(']').trim();
    let spec = GradleSpecifier::parse(bare);
    if !spec.valid() {
        return Err(Error::format(
            PathBuf::from(coord),
            format!("'{coord}' is not a maven coordinate a processor can name"),
        ));
    }
    let relative = spec.to_path("");
    if relative.is_empty() {
        return Err(Error::format(
            PathBuf::from(coord),
            format!("'{coord}' has no maven layout to resolve to"),
        ));
    }
    let dest = library_dir.join(&relative);
    for root in roots {
        let url = format!("{root}{relative}");
        let sidecar = format!("{url}.sha1");
        let digest = match maven_sha1(meta, &sidecar, cancel, backoff)? {
            Some(digest) => digest,
            None => continue,
        };
        if dest.is_file() && digest.verify_file(&dest).is_ok() {
            return Ok(dest);
        }
        store.fetch_blocking(meta.fetch.as_ref(), &url, &digest, cancel, backoff)?;
        if let Some(parent) = dest.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|error| Error::io(parent, error))?;
            }
        }
        let stored = store.path(&digest);
        std::fs::copy(&stored, &dest).map_err(|error| Error::io(&dest, error))?;
        return Ok(dest);
    }
    // No root stated a digest. Trust, but say so in the only place the file
    // can be told apart from a checked one: it lives outside the store.
    let first = roots.first().copied().unwrap_or(CENTRAL_MAVEN);
    let url = format!("{first}{relative}");
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|error| Error::io(parent, error))?;
        }
    }
    let download = Download::new(&url, &dest);
    let mut sleep = |wait: std::time::Duration| std::thread::sleep(wait);
    fetch_to_file(meta.fetch.as_ref(), &download, cancel, backoff, &mut sleep)?;
    Ok(dest)
}

/// One argument with its `{TOKENS}` and `[coordinates]` expanded.
///
/// Expansion is path math only and never downloads: the official installer
/// downloads the whole library set before any processor runs, so every
/// coordinate already has a file by the time a tool names it, and a product
/// of one processor (which exists on no maven) is just the path another one
/// writes. [`install`] resolves that set up front; expanding here only says
/// where each name points.
struct Expander<'a> {
    /// `"processor {i} ({jar})"`, carried into every failure.
    label: String,
    data: &'a BTreeMap<String, DataValue>,
    side: &'a str,
    game: &'a str,
    root: &'a Path,
    installer: &'a Path,
    installer_bytes: &'a [u8],
    extract_dir: &'a Path,
    library_dir: &'a Path,
    minecraft_jar: &'a Path,
}

impl Expander<'_> {
    /// The file a coordinate names under the library dir.
    fn coord_path(&self, coord: &str, arg: &str) -> Result<PathBuf, Error> {
        let spec = GradleSpecifier::parse(coord.trim());
        if !spec.valid() || spec.to_path("").is_empty() {
            return Err(self.wrap(arg, format!("'{coord}' is not a maven coordinate")));
        }
        Ok(self.library_dir.join(spec.to_path("")))
    }

    /// A failure with the processor and the argument attached, in a sentence
    /// a reader can act on: which tool, which argument, and what was wrong.
    fn wrap(&self, arg: &str, detail: String) -> Error {
        Error::format(
            PathBuf::from(&self.label),
            format!("{} argument '{arg}': {detail}", self.label),
        )
    }

    /// Expand one `{NAME}` token.
    fn token(&self, name: &str, arg: &str) -> Result<String, Error> {
        match name {
            "MINECRAFT_JAR" => Ok(self.minecraft_jar.to_string_lossy().into_owned()),
            "ROOT" => Ok(self.root.to_string_lossy().into_owned()),
            "INSTALLER" => Ok(self.installer.to_string_lossy().into_owned()),
            "LIBRARY_DIR" => Ok(self.library_dir.to_string_lossy().into_owned()),
            "SIDE" => Ok(self.side.to_string()),
            "MINECRAFT_VERSION" => Ok(self.game.to_string()),
            _ => {
                let value = self.data.get(name).ok_or_else(|| {
                    self.wrap(arg, format!("'{{{name}}}' names no data entry and no known token"))
                })?;
                match value {
                    DataValue::Literal(text) => Ok(text.clone()),
                    DataValue::Artifact(coord) => {
                        Ok(self.coord_path(coord, arg)?.to_string_lossy().into_owned())
                    }
                    DataValue::Path(entry) => Ok(self.installer_file(entry, arg)?.to_string_lossy().into_owned()),
                }
            }
        }
    }

    /// Extract an installer-relative file (`/data/...`) into the scratch
    /// directory, or reuse the extraction: the same entry is named by several
    /// processors and extracting it twice is pure waste.
    fn installer_file(&self, entry: &str, arg: &str) -> Result<PathBuf, Error> {
        let name = entry.trim_start_matches('/');
        let dest = self.extract_dir.join(name);
        if dest.is_file() {
            return Ok(dest);
        }
        let body = zip_entry(self.installer_bytes, "<installer>", name)
            .map_err(|error| self.wrap(arg, error.to_string()))?
            .ok_or_else(|| {
                self.wrap(arg, format!("the installer holds no '{entry}'"))
            })?;
        if let Some(parent) = dest.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|error| Error::io(parent, error))?;
            }
        }
        std::fs::write(&dest, &body).map_err(|error| Error::io(&dest, error))?;
        Ok(dest)
    }

    /// Expand one raw argument.
    fn expand(&self, arg: &str) -> Result<String, Error> {
        let bytes = arg.as_bytes();
        let mut out = String::with_capacity(arg.len());
        let mut i = 0;
        while i < bytes.len() {
            let rest = &arg[i..];
            if rest.starts_with('{') {
                let Some(end) = rest.find('}') else {
                    return Err(self.wrap(arg, "an opening '{' with no closing '}'".to_string()));
                };
                out.push_str(&self.token(&rest[1..end], arg)?);
                i += end + 1;
            } else if rest.starts_with('[') {
                let Some(end) = rest.find(']') else {
                    return Err(self.wrap(arg, "an opening '[' with no closing ']'".to_string()));
                };
                out.push_str(&self.coord_path(&rest[1..end], arg)?.to_string_lossy());
                i += end + 1;
            } else {
                let next = rest.find(['{', '[']).unwrap_or(rest.len());
                out.push_str(&rest[..next]);
                i += next;
            }
        }
        Ok(out)
    }
}

/// Read a jar's `Main-Class`: what `java -cp <jars>` runs.
///
/// Manifest lines wrap at 72 bytes with a leading space on continuations, so
/// the value is unfolded before it is read; a jar with no manifest or no
/// `Main-Class` names itself in the error, because the fix is to look at the
/// artifact rather than at the install.
fn main_class_of_jar(path: &Path) -> Result<String, Error> {
    let bytes = std::fs::read(path).map_err(|error| Error::io(path, error))?;
    let text = zip_entry(&bytes, &path.to_string_lossy(), "META-INF/MANIFEST.MF")?
        .ok_or_else(|| {
            Error::format(path, format!("{} has no manifest, so there is no Main-Class to run", path.display()))
        })?;
    let text = String::from_utf8_lossy(&text).into_owned();
    let mut main: Option<String> = None;
    for line in text.lines() {
        if let Some(continued) = line.strip_prefix(' ') {
            if let Some(current) = main.as_mut() {
                current.push_str(continued);
            }
            continue;
        }
        if main.is_some() {
            break;
        }
        if let Some(value) = line.strip_prefix("Main-Class:") {
            main = Some(value.trim().to_string());
        }
    }
    main.filter(|class| !class.is_empty()).ok_or_else(|| {
        Error::format(path, format!("{} declares no Main-Class, so there is nothing to run", path.display()))
    })
}

/// The `java` binary name on this platform.
fn java_exe_name() -> &'static str {
    if cfg!(windows) {
        "java.exe"
    } else {
        "java"
    }
}

/// Find a `java` binary in `dirs`, which is what makes the search testable:
/// the directories are the process's `PATH` in production and a scratch
/// directory in a test.
fn find_java_in(dirs: &[PathBuf]) -> Option<PathBuf> {
    dirs.iter()
        .map(|dir| dir.join(java_exe_name()))
        .find(|candidate| candidate.is_file())
}

/// Locate a `java` to run the processors with, through `palantir-core`'s own
/// locator.
///
/// The `PATH` comes first: the runtime the user runs is the one the tools
/// should get, and a machine that runs this launcher usually runs it from a
/// shell that already found Java. Then the well-known roots
/// (`palantir_core::java::scan_installs`), highest major first -- the tools
/// run on anything modern, so newest wins. `None` is a machine with no Java,
/// which the install reports rather than discovers halfway through a chain.
pub fn find_java() -> Option<PathBuf> {
    let path_dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).collect())
        .unwrap_or_default();
    if let Some(found) = find_java_in(&path_dirs) {
        return Some(found);
    }
    let mut installs =
        palantir_core::java::scan_installs(palantir_core::paths::System::current());
    installs.sort_by_key(|install| install.version.as_ref().map(|v| v.major()).unwrap_or(0));
    installs.into_iter().rev().find_map(|install| {
        let candidate = install.home.join("bin").join(java_exe_name());
        candidate.is_file().then_some(candidate)
    })
}

/// Every `{TOKEN}` an argument list names, in order, duplicates kept.
///
/// Malformed braces are left for [`Expander::expand`] to refuse with the
/// processor and the argument attached; this is only the collection pass.
fn brace_tokens(texts: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for text in texts {
        let mut rest = *text;
        while let Some(start) = rest.find('{') {
            rest = &rest[start + 1..];
            if let Some(end) = rest.find('}') {
                out.push(rest[..end].to_string());
                rest = &rest[end + 1..];
            } else {
                break;
            }
        }
    }
    out
}

/// Every `[coordinate]` an argument list names, in order, duplicates kept.
///
/// Malformed brackets are likewise the expander's to refuse.
fn inline_coords(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for arg in args {
        let mut rest = arg.as_str();
        while let Some(start) = rest.find('[') {
            rest = &rest[start + 1..];
            if let Some(end) = rest.find(']') {
                out.push(rest[..end].to_string());
                rest = &rest[end + 1..];
            } else {
                break;
            }
        }
    }
    out
}

/// Whether a `{TOKEN}` is one of the install's own rather than a `data` name.
fn is_well_known(token: &str) -> bool {
    matches!(
        token,
        "MINECRAFT_JAR" | "ROOT" | "INSTALLER" | "LIBRARY_DIR" | "SIDE" | "MINECRAFT_VERSION"
    )
}

/// Run an installer's processors, in order, the way the official installer does.
///
/// Two phases, like the official installer: first every library, tool and
/// input the running processors name is resolved through the maven roots
/// (digest-checked by the content store), then each processor expands its
/// arguments from `data` and runs under `ctx.java` with the instance root as
/// its working directory. What is *not* resolved is a product of the chain
/// itself -- an output one processor writes for the next, which exists on no
/// maven and is found by asking one for it. Server-only processors are
/// skipped on a client install, and a processor whose every output is already
/// present and matching is the resume case. A processor that fails takes the
/// install with it: the error names the processor, and the tool's own output
/// says which argument it choked on.
///
/// Blocking, like every engine call. The client jar the processors read is
/// the caller's -- Mojang's, fetched and digest-checked through `piston.rs`,
/// never re-downloaded under another name.
pub fn install(
    loader: Loader,
    spec: &InstallSpec,
    ctx: &InstallCtx<'_>,
    cancel: &Cancel,
    backoff: &Backoff,
) -> Result<Vec<InstalledProcessor>, Error> {
    for dir in [ctx.library_dir, ctx.root, ctx.extract_dir] {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir).map_err(|error| Error::io(dir, error))?;
        }
    }
    if !ctx.java.is_file() {
        return Err(Error::format(
            PathBuf::from("<java>"),
            "no Java runtime was located to run the installer processors with".to_string(),
        ));
    }
    let roots = maven_roots(loader);
    let label_of = |index: usize, jar: &str| format!("processor {index} ({jar})");
    let expanding = |index: usize, jar: &str| Expander {
        label: label_of(index, jar),
        data: &spec.data,
        side: ctx.side,
        game: ctx.game,
        root: ctx.root,
        installer: ctx.installer_path,
        installer_bytes: ctx.installer_bytes,
        extract_dir: ctx.extract_dir,
        library_dir: ctx.library_dir,
        minecraft_jar: ctx.minecraft_jar,
    };
    let running = |processor: &Processor| {
        processor.sides.is_empty() || processor.sides.iter().any(|side| side == ctx.side)
    };
    // The products first, so resolving the inputs can tell them apart: a
    // product exists on no maven, and asking one for it is a 404 that would
    // fail an install whose files are all exactly where they should be. The
    // resume check rides along: expanding outputs is path math, so an
    // install that is already done is known before it costs a request.
    let mut products = BTreeSet::new();
    let mut resumed = vec![false; spec.processors.len()];
    let mut expanded: Vec<Vec<(PathBuf, String)>> = Vec::with_capacity(spec.processors.len());
    for (index, processor) in spec.processors.iter().enumerate() {
        let expanding = expanding(index, &processor.jar);
        let mut outputs = Vec::with_capacity(processor.outputs.len());
        for (path, digest) in &processor.outputs {
            let expanded_path = PathBuf::from(expanding.expand(path)?);
            // Only a processor that will run contributes products: a
            // skipped one's outputs are never written on this side, so an
            // input sharing their path still needs resolving.
            if running(processor) {
                products.insert(expanded_path.clone());
            }
            outputs.push((expanded_path, expanding.expand(digest)?));
        }
        resumed[index] = running(processor)
            && !outputs.is_empty()
            && outputs.iter().all(|(path, sha)| {
                path.is_file()
                    && Digest::parse(sha).map(|digest| digest.verify_file(path).is_ok()).unwrap_or(true)
            });
        expanded.push(outputs);
    }
    let resolve = |coord: &str| {
        artifact_path(ctx.meta, ctx.store, ctx.library_dir, coord, &roots, cancel, backoff)
    };
    for coord in &spec.libraries {
        resolve(coord)?;
    }
    for (index, processor) in spec.processors.iter().enumerate() {
        if !running(processor) || resumed[index] {
            continue;
        }
        let label = label_of(index, &processor.jar);
        let wrap = |error: Error| {
            Error::format(PathBuf::from(&label), format!("{label}: {error}"))
        };
        resolve(&processor.jar).map_err(wrap)?;
        for coord in &processor.classpath {
            resolve(coord).map_err(wrap)?;
        }
    }
    // The inputs: data artifacts and inline coordinates the processors that
    // will actually run name, except the products of the chain itself.
    let mut inputs = Vec::new();
    for (index, processor) in spec.processors.iter().enumerate() {
        if !running(processor) || resumed[index] {
            continue;
        }
        let mut texts: Vec<&str> =
            processor.args.iter().map(String::as_str).collect();
        texts.extend(processor.outputs.keys().map(String::as_str));
        texts.extend(processor.outputs.values().map(String::as_str));
        for token in brace_tokens(&texts) {
            if is_well_known(&token) {
                continue;
            }
            if let Some(DataValue::Artifact(coord)) = spec.data.get(&token) {
                inputs.push(coord.clone());
            }
        }
        inputs.extend(inline_coords(&processor.args));
    }
    for coord in &inputs {
        let parsed = GradleSpecifier::parse(coord.trim());
        if parsed.valid() && products.contains(&ctx.library_dir.join(parsed.to_path(""))) {
            continue;
        }
        match resolve(coord) {
            Ok(_) => {}
            // A data entry no maven hosts is a product of the chain or a
            // file a tool fetches itself (MCP_DATA extracts its mappings out
            // of the neoform zip; DOWNLOAD_MOJMAPS fetches Mojang's own):
            // the run decides, with the processor named, rather than phase
            // zero refusing an install over a file that was never meant to
            // be resolved. Any other failure is the failure.
            Err(Error::Http { status: Some(404), .. }) => {}
            Err(error) => return Err(error),
        }
    }
    let mut report = Vec::with_capacity(spec.processors.len());
    for (index, processor) in spec.processors.iter().enumerate() {
        let label = label_of(index, &processor.jar);
        if !running(processor) || resumed[index] {
            report.push(InstalledProcessor { index, jar: processor.jar.clone(), skipped: true });
            continue;
        }
        let expanding = expanding(index, &processor.jar);
        let outputs = std::mem::take(&mut expanded[index]);
        let mut args = Vec::with_capacity(processor.args.len());
        for arg in &processor.args {
            args.push(expanding.expand(arg)?);
        }
        // The official installer downloads the whole library set before any
        // processor runs, which creates these directories as a side effect.
        // Here the set arrives in the phase above, so the outputs' parents
        // are made before the tool runs rather than discovered missing by it.
        for (path, _) in &outputs {
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent).map_err(|error| Error::io(parent, error))?;
                }
            }
        }
        // Resolved in the phase above; path math locates them now.
        let tool = expanding.coord_path(&processor.jar, &processor.jar).map_err(|error| {
            Error::format(PathBuf::from(&label), format!("{label}: {error}"))
        })?;
        let mut classpath = vec![tool.clone()];
        for coord in &processor.classpath {
            classpath.push(
                expanding.coord_path(coord, coord).map_err(|error| {
                    Error::format(PathBuf::from(&label), format!("{label}: {error}"))
                })?,
            );
        }
        let main = main_class_of_jar(&tool).map_err(|error| {
            Error::format(PathBuf::from(&label), format!("{label}: {error}"))
        })?;
        let separator = if cfg!(windows) { ";" } else { ":" };
        let classpath = classpath
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(separator);
        cancel.check()?;
        let output = std::process::Command::new(ctx.java)
            .arg("-cp")
            .arg(&classpath)
            .arg(&main)
            .args(&args)
            .current_dir(ctx.root)
            .output()
            .map_err(|error| {
                Error::format(
                    PathBuf::from(&label),
                    format!("{label} could not start {}: {error}", ctx.java.display()),
                )
            })?;
        if !output.status.success() {
            let mut stderr = String::from_utf8_lossy(&output.stdout).into_owned();
            stderr.push_str(&String::from_utf8_lossy(&output.stderr));
            let mut tail: String = stderr.chars().rev().take(2000).collect();
            tail = tail.chars().rev().collect();
            return Err(Error::format(
                PathBuf::from(&label),
                format!("{label} failed with {}: {tail}", output.status),
            ));
        }
        for (path, sha) in &outputs {
            if !path.is_file() {
                return Err(Error::format(
                    PathBuf::from(&label),
                    format!("{label} finished without writing {}", path.display()),
                ));
            }
            if let Ok(digest) = Digest::parse(sha) {
                digest.verify_file(path).map_err(|_| {
                    Error::format(
                        PathBuf::from(&label),
                        format!("{label} wrote {} with the wrong bytes: not the digest it declared", path.display()),
                    )
                })?;
            }
        }
        report.push(InstalledProcessor { index, jar: processor.jar.clone(), skipped: false });
    }
    Ok(report)
}

/// Everything one Forge-shaped install needs: the two jars it works with,
/// where it writes, and how it runs.
///
/// `root` and `library_dir` are the directories the official installer's
/// `{ROOT}` and `{LIBRARY_DIR}` name, and they have to be the ones a launch
/// reads: the patched client is a classpath entry under the maven coordinate
/// the installer's own `version.json` carries, so a library directory of this
/// function's own would install the loader somewhere no launch looks.
pub struct ClientInstall<'a> {
    /// The installer metadata, over the engine's cache and client.
    pub meta: &'a InstallerMeta,
    /// Mojang's own metadata: the client jar the processors patch.
    pub piston: &'a PistonMeta,
    /// The way bytes arrive, for the client jar's content-store fetch.
    pub fetch: &'a dyn Fetch,
    /// Where every fetched file is filed by its digest before it is used.
    pub store: &'a ContentStore,
    /// The instance's game directory; `{ROOT}` and the processors' cwd.
    pub root: &'a Path,
    /// Where artifacts land in maven layout; `{LIBRARY_DIR}`.
    pub library_dir: &'a Path,
    /// Scratch directory for the installer jar and its `/data/...` files.
    pub scratch: &'a Path,
    /// The `java` binary the processors run on.
    pub java: &'a Path,
    /// How a running install is stopped.
    pub cancel: &'a Cancel,
    /// How a failed request is retried.
    pub backoff: &'a Backoff,
}

/// What one [`install_client`] wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientInstallReport {
    /// Every processor, in the order the installer lists them.
    pub processors: Vec<InstalledProcessor>,
    /// Mojang's client jar as the content store holds it -- the file the
    /// processors read, not a second copy under this module's own name.
    pub minecraft_jar: PathBuf,
    /// The patched client, when the installer's `data` names one (`PATCHED`).
    pub patched_client: Option<PathBuf>,
}

impl ClientInstallReport {
    /// Processors that ran.
    pub fn ran(&self) -> usize {
        self.processors.iter().filter(|processor| !processor.skipped).count()
    }

    /// Processors that were skipped: server-only, or already done.
    pub fn skipped(&self) -> usize {
        self.processors.iter().filter(|processor| processor.skipped).count()
    }
}

/// Install one Forge-shaped loader build into an instance, the way the
/// loader's own installer does.
///
/// The two files the installer takes for granted are fetched here rather than
/// assumed: the installer jar, through [`InstallerMeta`] and against the digest
/// its maven's `.sha1` sidecar states, and Mojang's client jar, through
/// [`PistonMeta`] and against the digest the manifest published. Both travel
/// through the content store, so the install that follows them (and the next
/// launch of the same instance) finds what is already here rather than
/// fetching it a second time.
///
/// What comes back is the processors' own report -- which ran, which were
/// skipped as server-only or as already done -- beside the two files that
/// matter afterwards: Mojang's client jar, and the patched client when the
/// installer names one. The launch profile that goes with this install is
/// [`InstallerMeta::profile`]'s; this function is the other half, and the two
/// read the same jar, so a caller that runs this and then resolves through
/// `meta` is resolving the same build it installed.
///
/// Blocking, like every engine call.
pub fn install_client(
    loader: Loader,
    game: &str,
    build: &str,
    setup: &ClientInstall<'_>,
) -> Result<ClientInstallReport, Error> {
    if installer_url(loader, game, build).is_none() {
        return Err(Error::format(
            PathBuf::from(loader.name()),
            format!(
                "{} publishes no installer to run: its launch profile is a document",
                loader.name()
            ),
        ));
    }
    let (bytes, parsed) =
        setup.meta.parsed(loader, game, build, setup.store, setup.cancel, setup.backoff)?;
    // The client jar below is fetched for `game`, and the processors are told
    // `{MINECRAFT_VERSION}` from the installer's own file: an installer whose
    // two disagree would patch a client for another version, which is the one
    // mismatch worth refusing by name rather than discovering at launch.
    if parsed.install.minecraft.trim().is_empty() {
        return Err(Error::format(
            PathBuf::from(loader.name()),
            format!("{} {build}'s install profile names no Minecraft version", loader.name()),
        ));
    }
    if parsed.install.minecraft != game {
        return Err(Error::format(
            PathBuf::from(loader.name()),
            format!(
                "{} {build} installs Minecraft {}, but the instance names {game}",
                loader.name(),
                parsed.install.minecraft
            ),
        ));
    }
    // Mojang's client jar: the manifest's own digest, checked into the store
    // before a processor sees the file.
    let translated = setup
        .piston
        .translated(game, "net.minecraft", setup.cancel, setup.backoff)
        .map_err(|error| {
            Error::format(
                PathBuf::from(game),
                format!("Minecraft {game}'s version file could not be read: {error}"),
            )
        })?;
    let artifact = translated
        .main_jar
        .as_ref()
        .and_then(|jar| jar.mojang_downloads.as_ref())
        .and_then(|downloads| downloads.artifact.as_ref())
        .ok_or_else(|| {
            Error::format(
                PathBuf::from(game),
                format!("Minecraft {game} publishes no client jar for the processors to patch"),
            )
        })?;
    let digest = Digest::parse(&artifact.sha1).map_err(|error| {
        Error::format(
            PathBuf::from(&artifact.url),
            format!("{}: {error}", artifact.url),
        )
    })?;
    setup
        .store
        .fetch_blocking(setup.fetch, &artifact.url, &digest, setup.cancel, setup.backoff)?;
    let minecraft_jar = setup.store.path(&digest);

    // The client a launch's classpath will name. Reading the coordinate out of
    // `PATCHED` is how the official installer knows it too; a build whose
    // processors ran but which did not leave that file has not installed, and
    // the profile must not be resolved as if it had.
    let patched_client = match parsed.install.data.get("PATCHED") {
        Some(DataValue::Artifact(coord)) => {
            let spec = GradleSpecifier::parse(coord);
            if spec.valid() && !spec.to_path("").is_empty() {
                Some(setup.library_dir.join(spec.to_path("")))
            } else {
                return Err(Error::format(
                    PathBuf::from(loader.name()),
                    format!("{} {build} names '{coord}' as its patched client", loader.name()),
                ));
            }
        }
        _ => None,
    };

    // A client chain that declares no outputs leaves the per-digest resume
    // below nothing to compare, and the last step of that chain is what writes
    // `PATCHED`. NeoForge's `install_profile.json` is that shape -- measured on
    // 21.1.172: ten processors and not one `outputs` between them -- so without
    // this a second launch runs its six client-side processors again, minutes
    // of Java to write bytes that are already on disk. Forge declares digests,
    // so its own resume path still answers and this is not consulted; a chain
    // that ran out before its last step has no `PATCHED` and falls through to
    // it.
    let no_outputs = parsed
        .install
        .processors
        .iter()
        .filter(|processor| {
            processor.sides.is_empty()
                || processor.sides.iter().any(|side| side == CLIENT_SIDE)
        })
        .all(|processor| processor.outputs.is_empty());
    if no_outputs {
        if let Some(patched) = &patched_client {
            if patched.is_file() {
                let processors = parsed
                    .install
                    .processors
                    .iter()
                    .enumerate()
                    .map(|(index, processor)| InstalledProcessor {
                        index,
                        jar: processor.jar.clone(),
                        skipped: true,
                    })
                    .collect();
                return Ok(ClientInstallReport {
                    processors,
                    minecraft_jar,
                    patched_client: patched_client.clone(),
                });
            }
        }
    }

    std::fs::create_dir_all(setup.scratch).map_err(|error| Error::io(setup.scratch, error))?;
    let installer_path = setup.scratch.join("installer.jar");
    std::fs::write(&installer_path, &bytes).map_err(|error| Error::io(&installer_path, error))?;
    let extract_dir = setup.scratch.join("data");
    let ctx = InstallCtx {
        meta: setup.meta,
        store: setup.store,
        library_dir: setup.library_dir,
        root: setup.root,
        installer_path: &installer_path,
        installer_bytes: &bytes,
        minecraft_jar: &minecraft_jar,
        extract_dir: &extract_dir,
        java: setup.java,
        side: CLIENT_SIDE,
        game: &parsed.install.minecraft,
    };
    let processors = install(loader, &parsed.install, &ctx, setup.cancel, setup.backoff)?;

    if let Some(patched) = &patched_client {
        if !patched.is_file() {
            return Err(Error::format(
                PathBuf::from(loader.name()),
                format!(
                    "{} {build} ran its processors without writing {}",
                    loader.name(),
                    patched.display()
                ),
            ));
        }
    }
    Ok(ClientInstallReport { processors, minecraft_jar, patched_client })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::cache::DEFAULT_TTL;
    use crate::engine::piston::PISTON_MANIFEST_URL;
    use crate::engine::request::{MapFetch, Route};
    use std::io::Write;
    use std::time::Duration;

    /// The installer URL shapes, and the loaders that have none.
    #[test]
    fn an_installer_url_names_the_build_its_jar_holds() {
        assert_eq!(
            installer_url(Loader::Forge, "1.21.1", "52.1.0").as_deref(),
            Some("https://maven.minecraftforge.net/net/minecraftforge/forge/1.21.1-52.1.0/forge-1.21.1-52.1.0-installer.jar")
        );
        // The spelling Prism writes an imported instance's component with is
        // the same jar: the game in front of the build is the prefix, not part
        // of the build, and a URL that repeats it answers 404.
        assert_eq!(
            installer_url(Loader::Forge, "1.21.1", "1.21.1-52.1.0"),
            installer_url(Loader::Forge, "1.21.1", "52.1.0")
        );
        assert_eq!(
            installer_url(Loader::NeoForge, "1.21.1", "21.1.172").as_deref(),
            Some("https://maven.neoforged.net/releases/net/neoforged/neoforge/21.1.172/neoforge-21.1.172-installer.jar")
        );
        assert!(installer_url(Loader::Fabric, "1.21.4", "0.19.5").is_none());
        assert!(installer_url(Loader::Quilt, "1.21.4", "0.19.1").is_none());
        assert_eq!(component_uid(Loader::Forge), "net.minecraftforge");
        assert_eq!(component_uid(Loader::NeoForge), "net.neoforged");
    }

    /// Build a minimal installer jar in memory: `version.json` plus an
    /// `install_profile.json` with one processor, the modern shape.
    fn modern_jar() -> Vec<u8> {
        let version = serde_json::json!({
            "id": "1.21.1-forge-52.1.0",
            "time": "2024-12-03T10:12:57+00:00",
            "releaseTime": "2024-12-03T10:12:57+00:00",
            "type": "release",
            "mainClass": "net.minecraftforge.bootstrap.ForgeBootstrap",
            "arguments": {
                "game": ["--launchTarget", "forge_client"],
                "jvm": ["-Djava.net.preferIPv6Addresses=system"]
            },
            "libraries": [
                { "name": "net.minecraftforge:forge:1.21.1-52.1.0:universal" },
                { "name": "net.minecraftforge:forge:1.21.1-52.1.0:client" }
            ]
        });
        let profile = serde_json::json!({
            "spec": 1,
            "profile": "forge",
            "version": "1.21.1-forge-52.1.0",
            "minecraft": "1.21.1",
            "libraries": [{ "name": "test:extra:3" }],
            "data": {
                "BINPATCH": "/data/client.lzma",
                "PATCHED": "[net.minecraftforge:forge:1.21.1-52.1.0:client]",
                "PATCHED_SHA": "'abc123'"
            },
            "processors": [
                {
                    "jar": "net.minecraftforge:binarypatcher:1.2.0",
                    "classpath": ["net.sf.jopt-simple:jopt-simple:6.0-alpha-3"],
                    "args": ["--clean", "{MC_OFF}", "--output", "{PATCHED}", "--apply", "{BINPATCH}"],
                    "outputs": { "{PATCHED}": "{PATCHED_SHA}" }
                }
            ]
        });
        write_jar(&[("version.json", &version.to_string()), ("install_profile.json", &profile.to_string())])
    }

    /// An older shape: no `version.json`, the profile under `versionInfo`.
    fn version_info_jar() -> Vec<u8> {
        let profile = serde_json::json!({
            "spec": 0,
            "profile": "forge",
            "version": "1.12.2-forge-14.23.5.2860",
            "minecraft": "1.12.2",
            "data": {},
            "processors": [],
            "versionInfo": {
                "id": "1.12.2-forge-14.23.5.2860",
                "mainClass": "net.minecraft.launchwrapper.Launch",
                "minecraftArguments": "--username ${auth_player_name} --tweakClass net.minecraftforge.fml.common.launcher.FMLTweaker",
                "libraries": [{ "name": "net.minecraftforge:forge:1.12.2-14.23.5.2860:universal" }]
            }
        });
        write_jar(&[("install_profile.json", &profile.to_string())])
    }

    /// Older still: neither `version.json` nor a `versionInfo` profile.
    fn ancient_jar() -> Vec<u8> {
        let profile = serde_json::json!({
            "profile": "forge",
            "version": "1.6.4-forge-9.11.1.1345",
            "minecraft": "1.6.4",
            "minecraftArguments": "--username ${auth_player_name}",
            "data": {},
            "processors": []
        });
        write_jar(&[("install_profile.json", &profile.to_string())])
    }

    /// A jar from entry names and bodies, stored rather than compressed: the
    /// test cares about the JSON, not the codec.
    fn write_jar(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut out);
            let options = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            for (name, body) in entries {
                writer.start_file(*name, options).expect("starting a zip entry");
                writer.write_all(body.as_bytes()).expect("writing a zip entry");
            }
            writer.finish().expect("finishing the jar");
        }
        out.into_inner()
    }

    /// A handle over a scratch directory whose installer URL serves `jar` and
    /// whose sidecar states the jar's own digest, so a test that asks for
    /// anything else fails instead of dialling out.
    fn meta(name: &str, url: &str, jar: Vec<u8>) -> (InstallerMeta, Arc<MapFetch>, ContentStore) {
        use crate::engine::content::Digest;

        let root = std::env::temp_dir().join("palantirmc-engine-forge").join(name);
        let _ = std::fs::remove_dir_all(&root);
        let fetch = Arc::new(MapFetch::new());
        fetch.set_route(url, Route::body(jar.clone()));
        fetch.set_route(&format!("{url}.sha1"), Route::text(Digest::sha1(&jar).hex()));
        let meta =
            InstallerMeta::new(MetadataCache::new(root.join("meta"), DEFAULT_TTL), fetch.clone());
        let store = ContentStore::new(root.join("store"));
        (meta, fetch, store)
    }

    #[test]
    fn a_modern_profile_comes_out_of_the_installer_with_the_game_args_as_its_string() {
        let url = installer_url(Loader::Forge, "1.21.1", "52.1.0").expect("Forge publishes one");
        let (meta, fetch, store) = meta("modern", &url, modern_jar());
        let cancel = Cancel::new();
        let backoff = Backoff::with_attempts(1);

        let file = meta
            .profile(Loader::Forge, "1.21.1", "52.1.0", &store, &cancel, &backoff)
            .expect("the profile");
        assert_eq!(file.uid, "net.minecraftforge");
        assert_eq!(file.version, "52.1.0");
        assert_eq!(file.main_class, "net.minecraftforge.bootstrap.ForgeBootstrap");
        assert_eq!(file.minecraft_arguments, "--launchTarget forge_client");
        assert!(
            !file.has_order,
            "the publisher's file carries no `order`: that key is the mirror's addition"
        );
        assert!(file.requires.is_empty(), "and no requirement the publisher did not state");
        assert_eq!(file.libraries.len(), 2);
        // The profile names the patched client as its main jar even though the
        // publisher's own file does not: without it a parse falls back to a
        // Mojang coordinate built out of `id`, which is an error a launch
        // refuses on rather than a jar anything could start.
        assert_eq!(
            file.main_jar.as_ref().map(|jar| jar.name.serialize()),
            Some("net.minecraftforge:forge:1.21.1-52.1.0:client".to_string()),
            "the main jar is the client the install writes, named from `PATCHED`"
        );
        assert!(
            !file
                .problems
                .iter()
                .any(|problem| problem.severity == palantir_core::version::ProblemSeverity::Error),
            "problems: {:?}",
            file.problems
        );
        // Asked again, everything is already held: one sidecar and one
        // installer fetch between the two asks.
        meta.profile(Loader::Forge, "1.21.1", "52.1.0", &store, &cancel, &backoff).expect("again");
        assert_eq!(fetch.count(), 2, "the second ask was answered from disk and store");
    }

    #[test]
    fn an_old_build_is_read_from_version_info_with_its_string_kept() {
        let url = installer_url(Loader::Forge, "1.12.2", "14.23.5.2860").expect("one");
        let (meta, _, store) = meta("version-info", &url, version_info_jar());
        let file = meta
            .profile(Loader::Forge, "1.12.2", "14.23.5.2860", &store, &Cancel::new(), &Backoff::with_attempts(1))
            .expect("the old profile");
        assert_eq!(file.main_class, "net.minecraft.launchwrapper.Launch");
        assert!(
            file.minecraft_arguments.contains("--tweakClass"),
            "the era's own argument string is kept as published"
        );
    }

    #[test]
    fn an_installer_with_no_profile_in_it_is_refused_by_name() {
        let url = installer_url(Loader::Forge, "1.6.4", "9.11.1.1345").expect("one");
        let (meta, _, store) = meta("ancient", &url, ancient_jar());
        let error = meta
            .profile(Loader::Forge, "1.6.4", "9.11.1.1345", &store, &Cancel::new(), &Backoff::with_attempts(1))
            .expect_err("nothing to read");
        let message = error.to_string();
        assert!(message.contains(&url), "{message}");
        assert!(message.contains("refused"), "{message}");
    }

    /// An installer jar whose install half is one server-only processor, so a
    /// client install runs nothing and the test needs no Java tool to exist.
    fn idle_installer(minecraft: &str) -> Vec<u8> {
        let version = serde_json::json!({
            "id": "1.21.1-forge-52.1.0",
            "mainClass": "net.minecraftforge.bootstrap.ForgeBootstrap",
            "arguments": { "game": [] },
            "libraries": []
        });
        let profile = serde_json::json!({
            "spec": 1,
            "profile": "forge",
            "version": "1.21.1-forge-52.1.0",
            "minecraft": minecraft,
            "libraries": [],
            "data": {},
            "processors": [{
                "jar": "net.minecraftforge:binarypatcher:1.2.0",
                "classpath": [],
                "args": [],
                "outputs": {},
                "sides": ["server"]
            }]
        });
        write_jar(&[
            ("version.json", &version.to_string()),
            ("install_profile.json", &profile.to_string()),
        ])
    }

    /// An installer whose one client-side processor declares no outputs, which
    /// is the shape NeoForge's real `install_profile.json` has: the per-digest
    /// resume has nothing to compare, and the client the chain names is the only
    /// evidence that it ran. Its processor's jar is a coordinate no route in
    /// these tests serves, so an install that resolves it fails rather than
    /// quietly skipping this path.
    fn unchecked_installer(minecraft: &str) -> Vec<u8> {
        let version = serde_json::json!({
            "id": format!("{minecraft}-neoforge-21.1.172"),
            "mainClass": "cpw.mods.bootstraplauncher.BootstrapLauncher",
            "arguments": { "game": [] },
            "libraries": []
        });
        let profile = serde_json::json!({
            "spec": 1,
            "profile": "neoforge",
            "version": format!("{minecraft}-neoforge-21.1.172"),
            "minecraft": minecraft,
            "libraries": [],
            "data": { "PATCHED": "[net.neoforged:neoforge:21.1.172:client]" },
            "processors": [{
                "jar": "net.neoforged.installertools:binarypatcher:2.1.2:fatjar",
                "classpath": [],
                "args": [],
                "outputs": {},
                "sides": []
            }]
        });
        write_jar(&[
            ("version.json", &version.to_string()),
            ("install_profile.json", &profile.to_string()),
        ])
    }

    /// Publish a piston manifest, a version file and a client jar for `game`,
    /// returning the client's bytes. The version file is the shape Mojang
    /// serves, trimmed; the digests are the ones the caller can check.
    fn piston_routes(fetch: &MapFetch, game: &str) -> Vec<u8> {
        let client = b"the client jar".to_vec();
        let client_sha1 = Digest::sha1(&client).hex().to_string();
        let version_url = "https://piston.invalid/1.21.1.json";
        let version = format!(
            r#"{{"id":"{game}","type":"release","releaseTime":"2024-12-03T10:12:57+00:00",
                "mainClass":"net.minecraft.client.main.Main","assets":"19",
                "assetIndex":{{"id":"19","sha1":"aa","size":1,"totalSize":1,
                    "url":"https://piston.invalid/19.json"}},
                "javaVersion":{{"component":"java-runtime-delta","majorVersion":21}},
                "downloads":{{"client":{{"sha1":"{client_sha1}","size":{},
                    "url":"https://piston.invalid/client.jar"}}}},
                "arguments":{{"jvm":[],"game":[]}},"libraries":[]}}"#,
            client.len()
        );
        let digest = Digest::sha1(version.as_bytes()).hex().to_string();
        let manifest = format!(
            r#"{{"latest":{{"release":"{game}","snapshot":"25w02a"}},
                "versions":[{{"id":"{game}","type":"release","url":"{version_url}",
                    "releaseTime":"2024-12-03T10:12:57+00:00","sha1":"{digest}"}}]}}"#
        );
        fetch.set_route(PISTON_MANIFEST_URL, Route::body(manifest.into_bytes()));
        fetch.set_route(version_url, Route::body(version.into_bytes()));
        fetch.set_route("https://piston.invalid/client.jar", Route::body(client.clone()));
        client
    }

    /// The composition `install_client` exists for, without a network: the
    /// installer jar comes off its maven against its own sidecar, Mojang's
    /// client jar comes through piston against the manifest's digest, and the
    /// processors report in the installer's own terms.
    #[test]
    fn an_install_fetches_the_client_it_patches_and_reports_its_processors() {
        let root = std::env::temp_dir().join("palantirmc-engine-forge").join("client-install");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a scratch directory");
        let fetch = MapFetch::new();
        let client = piston_routes(&fetch, "1.21.1");
        let jar = idle_installer("1.21.1");
        let url = installer_url(Loader::Forge, "1.21.1", "52.1.0").expect("Forge publishes one");
        fetch.set_route(&url, Route::body(jar.clone()));
        fetch.set_route(&format!("{url}.sha1"), Route::text(Digest::sha1(&jar).hex()));
        let fetch = Arc::new(fetch);

        let meta = InstallerMeta::new(
            MetadataCache::new(root.join("meta"), DEFAULT_TTL),
            fetch.clone(),
        );
        let piston = PistonMeta::new(
            MetadataCache::new(root.join("piston"), DEFAULT_TTL),
            fetch.clone(),
        );
        let store = ContentStore::new(root.join("store"));
        let instance = root.join("instance");
        let library_dir = instance.join("libraries");
        let java = root.join("java.exe");
        std::fs::write(&java, b"").expect("a stand-in for a java binary");
        let cancel = Cancel::new();
        let backoff = Backoff::with_attempts(1);

        let report = install_client(
            Loader::Forge,
            "1.21.1",
            "52.1.0",
            &ClientInstall {
                meta: &meta,
                piston: &piston,
                fetch: fetch.as_ref(),
                store: &store,
                root: &instance,
                library_dir: &library_dir,
                scratch: &root.join("scratch"),
                java: &java,
                cancel: &cancel,
                backoff: &backoff,
            },
        )
        .expect("the install");

        assert_eq!(report.processors.len(), 1, "{report:?}");
        assert_eq!(
            (report.ran(), report.skipped()),
            (0, 1),
            "the only processor is server-only: {report:?}"
        );
        assert_eq!(
            std::fs::read(&report.minecraft_jar).expect("the client jar"),
            client,
            "the jar the processors read is the one Mojang published"
        );
        assert!(
            report.patched_client.is_none(),
            "this installer names no patched client: {report:?}"
        );
    }

    /// A chain that declares no outputs is read from the client it wrote: the
    /// file is there, so nothing runs -- and nothing is even resolved, which is
    /// what this test can tell from a service that has no route for the
    /// processor's jar.
    #[test]
    fn an_install_that_declares_no_outputs_is_read_from_the_client_it_wrote() {
        let root = std::env::temp_dir().join("palantirmc-engine-forge").join("unchecked-install");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a scratch directory");
        let fetch = MapFetch::new();
        piston_routes(&fetch, "1.21.1");
        let jar = unchecked_installer("1.21.1");
        let url = installer_url(Loader::NeoForge, "1.21.1", "21.1.172")
            .expect("NeoForge publishes one");
        fetch.set_route(&url, Route::body(jar.clone()));
        fetch.set_route(&format!("{url}.sha1"), Route::text(Digest::sha1(&jar).hex()));
        let fetch = Arc::new(fetch);

        let meta = InstallerMeta::new(
            MetadataCache::new(root.join("meta"), DEFAULT_TTL),
            fetch.clone(),
        );
        let piston = PistonMeta::new(
            MetadataCache::new(root.join("piston"), DEFAULT_TTL),
            fetch.clone(),
        );
        let store = ContentStore::new(root.join("store"));
        let instance = root.join("instance");
        let library_dir = instance.join("libraries");
        let patched = library_dir
            .join("net")
            .join("neoforged")
            .join("neoforge")
            .join("21.1.172")
            .join("neoforge-21.1.172-client.jar");
        std::fs::create_dir_all(patched.parent().expect("a parent directory"))
            .expect("the maven layout");
        std::fs::write(&patched, b"the client a first install wrote").expect("the patched client");
        // Deliberately not a program: a fast path that shelled out to it would
        // fail this test rather than pass it quietly.
        let java = root.join("java.exe");
        std::fs::write(&java, b"").expect("a stand-in for a java binary");

        let report = install_client(
            Loader::NeoForge,
            "1.21.1",
            "21.1.172",
            &ClientInstall {
                meta: &meta,
                piston: &piston,
                fetch: fetch.as_ref(),
                store: &store,
                root: &instance,
                library_dir: &library_dir,
                scratch: &root.join("scratch"),
                java: &java,
                cancel: &Cancel::new(),
                backoff: &Backoff::with_attempts(1),
            },
        )
        .expect("the install");

        assert_eq!(
            report.patched_client.as_deref(),
            Some(patched.as_path()),
            "the report names the client that was found"
        );
        assert_eq!((report.ran(), report.skipped()), (0, 1), "{report:?}");
        assert!(
            !fetch
                .requests()
                .iter()
                .any(|request| request.url.contains("binarypatcher")),
            "the processor's jar was resolved even though nothing had to run: {:?}",
            fetch.requests().iter().map(|request| &request.url).collect::<Vec<_>>()
        );
    }

    /// An installer that patches another Minecraft than the instance runs is
    /// refused before anything is fetched: the client jar in hand is not the
    /// one its processors would patch.
    #[test]
    fn an_installer_for_another_minecraft_is_refused_rather_than_patched() {
        let root = std::env::temp_dir().join("palantirmc-engine-forge").join("client-mismatch");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a scratch directory");
        let fetch = MapFetch::new();
        // No piston routes at all: a service that answers nothing is what makes
        // "the mismatch was refused before the client was fetched" an
        // assertion rather than a reading of the source order.
        let jar = idle_installer("1.20.1");
        let url = installer_url(Loader::Forge, "1.21.1", "52.1.0").expect("Forge publishes one");
        fetch.set_route(&url, Route::body(jar.clone()));
        fetch.set_route(&format!("{url}.sha1"), Route::text(Digest::sha1(&jar).hex()));
        let fetch = Arc::new(fetch);

        let meta = InstallerMeta::new(
            MetadataCache::new(root.join("meta"), DEFAULT_TTL),
            fetch.clone(),
        );
        let piston = PistonMeta::new(
            MetadataCache::new(root.join("piston"), DEFAULT_TTL),
            fetch.clone(),
        );
        let store = ContentStore::new(root.join("store"));
        let java = root.join("java.exe");
        std::fs::write(&java, b"").expect("a stand-in for a java binary");

        let error = install_client(
            Loader::Forge,
            "1.21.1",
            "52.1.0",
            &ClientInstall {
                meta: &meta,
                piston: &piston,
                fetch: fetch.as_ref(),
                store: &store,
                root: &root.join("instance"),
                library_dir: &root.join("instance").join("libraries"),
                scratch: &root.join("scratch"),
                java: &java,
                cancel: &Cancel::new(),
                backoff: &Backoff::with_attempts(1),
            },
        )
        .expect_err("a mismatch");
        let message = error.to_string();
        assert!(message.contains("1.20.1"), "{message}");
        assert!(message.contains("1.21.1"), "{message}");
    }

    #[test]
    fn data_entries_are_classified_and_read_client_side() {
        let parsed = parse_installer(&modern_jar(), "<test>").expect("the modern jar");
        assert_eq!(parsed.install.minecraft, "1.21.1");
        assert_eq!(parsed.install.libraries, vec!["test:extra:3".to_string()]);
        assert_eq!(
            parsed.install.data.get("BINPATCH"),
            Some(&DataValue::Path("/data/client.lzma".to_string()))
        );
        assert_eq!(
            parsed.install.data.get("PATCHED"),
            Some(&DataValue::Artifact("net.minecraftforge:forge:1.21.1-52.1.0:client".to_string()))
        );
        assert_eq!(
            parsed.install.data.get("PATCHED_SHA"),
            Some(&DataValue::Literal("abc123".to_string()))
        );
        assert_eq!(parsed.install.processors.len(), 1);
        let processor = &parsed.install.processors[0];
        assert_eq!(processor.jar, "net.minecraftforge:binarypatcher:1.2.0");
        assert!(processor.sides.is_empty(), "no sides means both");
        assert_eq!(processor.outputs.len(), 1);

        // A per-side entry is read client side.
        let sided = serde_json::json!({"client": "[a:b:1]", "server": "[a:b:2]"});
        assert_eq!(
            client_side(&sided, "<test>", "X").expect("client side"),
            DataValue::Artifact("a:b:1".to_string())
        );
        // And one with no client side names itself.
        let server_only = serde_json::json!({"server": "[a:b:2]"});
        assert!(client_side(&server_only, "<test>", "X").is_err());
    }

    #[test]
    fn a_loader_that_publishes_no_installer_says_so_instead_of_asking() {
        let root = std::env::temp_dir().join("palantirmc-engine-forge").join("no-installer");
        let _ = std::fs::remove_dir_all(&root);
        let fetch = Arc::new(MapFetch::new());
        let meta = InstallerMeta::new(MetadataCache::new(root.join("meta"), DEFAULT_TTL), fetch.clone());
        let store = ContentStore::new(root.join("store"));
        assert!(installer_url(Loader::Fabric, "1.21.4", "0.19.5").is_none());
        let error = meta
            .profile(Loader::Fabric, "1.21.4", "0.19.5", &store, &Cancel::new(), &Backoff::with_attempts(1))
            .expect_err("no installer to read");
        assert!(error.to_string().contains("no installer"), "{error}");
        assert_eq!(fetch.count(), 0, "and nothing was asked for");
    }

    #[test]
    fn a_body_that_is_not_a_jar_names_the_url_it_came_from() {
        let url = installer_url(Loader::NeoForge, "1.21.1", "21.1.172").expect("one");
        let (meta, _, store) = meta("broken", &url, b"not a jar".to_vec());
        let failure = meta
            .profile(Loader::NeoForge, "1.21.1", "21.1.172", &store, &Cancel::new(), &Backoff::with_attempts(1))
            .expect_err("a body that is not a jar");
        assert!(failure.to_string().contains(&url), "{failure}");
    }

    /// Each loader's artifacts are tried on its own maven first, because that
    /// is the host that has the installer tools; central and Mojang's
    /// libraries are the fallbacks for everything else the processors name.
    #[test]
    fn artifacts_are_tried_on_the_loaders_own_maven_first() {
        assert_eq!(
            maven_roots(Loader::Forge),
            [FORGE_MAVEN, CENTRAL_MAVEN, MOJANG_LIBRARIES]
        );
        assert_eq!(
            maven_roots(Loader::NeoForge),
            [NEOFORGE_MAVEN, CENTRAL_MAVEN, MOJANG_LIBRARIES]
        );
    }

    /// A maven layout and its sidecar: the digest the sidecar states is the
    /// digest the jar is checked against, and a root without the file is the
    /// next root's question rather than a failure.
    #[test]
    fn a_sidecar_states_the_digest_and_a_missing_one_moves_on() {
        use crate::engine::content::Digest;

        let root = std::env::temp_dir().join("palantirmc-engine-forge").join("sidecar");
        let _ = std::fs::remove_dir_all(&root);
        let fetch = Arc::new(MapFetch::new());
        let meta = InstallerMeta::new(MetadataCache::new(root, Duration::ZERO), fetch.clone());
        let cancel = Cancel::new();
        let backoff = Backoff::with_attempts(1);

        let body = b"tool bytes";
        let hex = Digest::sha1(body).hex().to_string();
        fetch.set_route("https://maven.invalid/a/b/1/b-1.jar.sha1", Route::text(&hex));
        let digest = maven_sha1(&meta, "https://maven.invalid/a/b/1/b-1.jar.sha1", &cancel, &backoff)
            .expect("the sidecar")
            .expect("a digest");
        assert_eq!(digest.hex(), hex);

        // A body that is not a digest is a broken mirror, not a missing file.
        fetch.set_route("https://maven.invalid/c/d/2/d-2.jar.sha1", Route::text("nope"));
        assert!(maven_sha1(&meta, "https://maven.invalid/c/d/2/d-2.jar.sha1", &cancel, &backoff).is_err());
        // And a 404 is `None`: the next root's question.
        assert!(
            maven_sha1(&meta, "https://maven.invalid/nothing.jar.sha1", &cancel, &backoff)
                .expect("a 404")
                .is_none()
        );
    }

    /// An expander over scripted maven: tokens, data entries, inline
    /// coordinates, and the two failures that name the processor and the
    /// argument.
    fn expander<'a>(
        data: &'a BTreeMap<String, DataValue>,
        dirs: &'a TestDirs,
        installer_bytes: &'a [u8],
    ) -> Expander<'a> {
        Expander {
            label: "processor 0 (test:tool:1)".to_string(),
            data,
            side: CLIENT_SIDE,
            game: "1.21.1",
            root: &dirs.root,
            installer: &dirs.installer,
            installer_bytes,
            extract_dir: &dirs.extract,
            library_dir: &dirs.libraries,
            minecraft_jar: &dirs.client,
        }
    }

    struct TestDirs {
        base: PathBuf,
        root: PathBuf,
        libraries: PathBuf,
        extract: PathBuf,
        installer: PathBuf,
        client: PathBuf,
        java: PathBuf,
        cancel: Cancel,
        backoff: Backoff,
    }

    impl TestDirs {
        fn new(name: &str) -> TestDirs {
            let base = std::env::temp_dir().join("palantirmc-engine-forge").join(name);
            let _ = std::fs::remove_dir_all(&base);
            let dirs = TestDirs {
                root: base.join("root"),
                libraries: base.join("libraries"),
                extract: base.join("extract"),
                installer: base.join("installer.jar"),
                client: base.join("client.jar"),
                java: base.join("java"),
                base,
                cancel: Cancel::new(),
                backoff: Backoff::with_attempts(1),
            };
            std::fs::create_dir_all(&dirs.root).expect("root");
            std::fs::write(&dirs.java, b"").expect("a java placeholder");
            std::fs::write(&dirs.client, b"client").expect("a client placeholder");
            dirs
        }

        fn ctx<'a>(
            &'a self,
            meta: &'a InstallerMeta,
            store: &'a ContentStore,
            installer_bytes: &'a [u8],
        ) -> InstallCtx<'a> {
            InstallCtx {
                meta,
                store,
                library_dir: &self.libraries,
                root: &self.root,
                installer_path: &self.installer,
                installer_bytes,
                minecraft_jar: &self.client,
                extract_dir: &self.extract,
                java: &self.java,
                side: CLIENT_SIDE,
                game: "1.21.1",
            }
        }
    }

    /// Script one artifact: its sidecar states the body's digest and the jar
    /// route serves the body, the way a maven does.
    fn script_artifact(fetch: &MapFetch, root: &str, coord: &str, body: &[u8]) {
        use crate::engine::content::Digest;

        let spec = GradleSpecifier::parse(coord);
        assert!(spec.valid(), "{coord} is not a coordinate");
        let relative = spec.to_path("");
        fetch.set_route(
            &format!("{root}{relative}.sha1"),
            Route::text(Digest::sha1(body).hex()),
        );
        fetch.set_route(&format!("{root}{relative}"), Route::body(body.to_vec()));
    }

    #[test]
    fn arguments_expand_tokens_data_and_inline_coordinates() {
        let dirs = TestDirs::new("expand");

        let installer = write_jar(&[("data/blob.bin", "blob")]);
        let mut data = BTreeMap::new();
        data.insert("TOOL".to_string(), DataValue::Artifact("test:tool:1".to_string()));
        data.insert("WORD".to_string(), DataValue::Literal("hello".to_string()));
        data.insert("BLOB".to_string(), DataValue::Path("/data/blob.bin".to_string()));
        let expander = expander(&data, &dirs, &installer);

        // Expansion is path math: resolving is the install's phase zero, so
        // naming a coordinate costs no request here.
        let tool = expander.expand("{TOOL}").expect("a data artifact");
        assert!(tool.ends_with("test/tool/1/tool-1.jar"), "{tool}");
        assert!(!PathBuf::from(&tool).exists(), "expansion fetches nothing");
        assert_eq!(expander.expand("{WORD}").expect("a literal"), "hello");
        let blob = expander.expand("{BLOB}").expect("an installer file");
        assert_eq!(std::fs::read(&blob).expect("the extraction"), b"blob");
        assert_eq!(
            expander.expand("--in {MINECRAFT_JAR} --root {ROOT} --side {SIDE} --game {MINECRAFT_VERSION}").expect("tokens"),
            format!(
                "--in {} --root {} --side client --game 1.21.1",
                dirs.client.display(),
                dirs.root.display()
            )
        );
        let inline = expander.expand("--cp [test:lib:2]").expect("an inline coordinate");
        assert!(inline.ends_with("test/lib/2/lib-2.jar"), "{inline}");

        // The two failures a user can fix: an unknown name, and an
        // unterminated one. Both name the processor and the argument.
        for bad in ["{NOPE}", "{UNCLOSED", "[test:lib:2"] {
            let error = expander.expand(bad).expect_err(bad);
            let message = error.to_string();
            assert!(message.contains("processor 0 (test:tool:1)"), "{message}");
            assert!(message.contains(bad.trim_end_matches("UNCLOSED")), "{message}");
        }
    }

    /// A jar with no manifest has nothing to run: the error names the
    /// processor and the jar, before any process is spawned.
    fn manifestless_jar() -> Vec<u8> {
        write_jar(&[("data/x.txt", "x")])
    }

    #[test]
    fn install_runs_sides_skips_and_resume_without_a_process() {
        use crate::engine::content::Digest;

        let dirs = TestDirs::new("install-flow");
        let root = std::env::temp_dir().join("palantirmc-engine-forge").join("install-flow-cache");
        let _ = std::fs::remove_dir_all(&root);
        let fetch = Arc::new(MapFetch::new());
        let meta = InstallerMeta::new(MetadataCache::new(root, Duration::ZERO), fetch.clone());
        let store = ContentStore::new(dirs.base.join("store"));
        let installer = write_jar(&[("version.json", "{}"), ("install_profile.json", "{}")]);

        // An output already present and matching: the processor is the
        // resume case and its tool is never even resolved.
        let done_path = dirs.root.join("done.bin");
        std::fs::write(&done_path, b"done").expect("the output");
        let done_sha = Digest::sha1(b"done").hex().to_string();
        let spec = InstallSpec {
            minecraft: "1.21.1".to_string(),
            version: "1.21.1-forge-1".to_string(),
            libraries: Vec::new(),
            data: BTreeMap::new(),
            processors: vec![
                Processor {
                    jar: "test:server-only:1".to_string(),
                    classpath: Vec::new(),
                    args: Vec::new(),
                    outputs: BTreeMap::new(),
                    sides: vec!["server".to_string()],
                },
                Processor {
                    jar: "test:already-done:1".to_string(),
                    classpath: Vec::new(),
                    args: Vec::new(),
                    outputs: BTreeMap::from([(
                        done_path.to_string_lossy().into_owned(),
                        done_sha,
                    )]),
                    sides: Vec::new(),
                },
            ],
        };
        let report = install(
            Loader::Forge,
            &spec,
            &dirs.ctx(&meta, &store, &installer),
            &dirs.cancel,
            &dirs.backoff,
        )
        .expect("the install");
        assert_eq!(
            report,
            vec![
                InstalledProcessor { index: 0, jar: "test:server-only:1".to_string(), skipped: true },
                InstalledProcessor { index: 1, jar: "test:already-done:1".to_string(), skipped: true },
            ]
        );
        assert_eq!(fetch.count(), 0, "a skipped processor costs no request");
    }

    /// Phase zero resolves what a running processor names -- libraries, tools
    /// and inputs -- but never a product of the chain: a product exists on no
    /// maven, and asking one for it would fail an install over a file the
    /// next processor was going to write.
    #[test]
    fn install_resolves_inputs_but_not_products_before_running() {
        let dirs = TestDirs::new("install-inputs");
        let root = std::env::temp_dir().join("palantirmc-engine-forge").join("install-inputs-cache");
        let _ = std::fs::remove_dir_all(&root);
        let fetch = Arc::new(MapFetch::new());
        let meta = InstallerMeta::new(MetadataCache::new(root, Duration::ZERO), fetch.clone());
        let store = ContentStore::new(dirs.base.join("store"));
        let installer = write_jar(&[("version.json", "{}"), ("install_profile.json", "{}")]);
        // The tool resolves but has nothing to run: the failure this test
        // drives at proves the inputs arrived first.
        script_artifact(&fetch, FORGE_MAVEN, "test:headless:1", &manifestless_jar());
        script_artifact(&fetch, FORGE_MAVEN, "test:need:1", b"need!");
        script_artifact(&fetch, FORGE_MAVEN, "test:inline:1", b"inline!");

        let product_path = dirs.libraries.join("test/made/1/made-1.jar");
        let spec = InstallSpec {
            minecraft: "1.21.1".to_string(),
            version: "1.21.1-forge-1".to_string(),
            libraries: Vec::new(),
            data: BTreeMap::from([
                ("NEED".to_string(), DataValue::Artifact("test:need:1".to_string())),
                ("MADE".to_string(), DataValue::Artifact("test:made:1".to_string())),
                ("TOOLMADE".to_string(), DataValue::Artifact("test:toolmade:1".to_string())),
            ]),
            processors: vec![Processor {
                jar: "test:headless:1".to_string(),
                classpath: Vec::new(),
                args: vec![
                    "--need".to_string(),
                    "{NEED}".to_string(),
                    "--extra".to_string(),
                    "[test:inline:1]".to_string(),
                    "--out".to_string(),
                    "{MADE}".to_string(),
                    "--toolmade".to_string(),
                    "{TOOLMADE}".to_string(),
                ],
                outputs: BTreeMap::from([(
                    "{MADE}".to_string(),
                    "'00'".to_string(),
                )]),
                sides: Vec::new(),
            }],
        };
        let error = install(
            Loader::Forge,
            &spec,
            &dirs.ctx(&meta, &store, &installer),
            &dirs.cancel,
            &dirs.backoff,
        )
        .expect_err("no Main-Class to run");
        // Both inputs arrived; the declared product cost no request at all,
        // and the undeclared one survived every maven answering 404 because
        // the run, not phase zero, is what judges it.
        assert!(dirs.libraries.join("test/need/1/need-1.jar").is_file());
        assert!(dirs.libraries.join("test/inline/1/inline-1.jar").is_file());
        assert!(!product_path.exists(), "a product is written, never fetched");
        assert_eq!(fetch.count(), 10, "tool, inputs and inline resolved; leftovers tolerated");
        assert!(error.to_string().contains("Main-Class"), "{}", error);
    }

    #[test]
    fn a_processor_without_a_main_class_is_refused_before_it_runs() {
        let dirs = TestDirs::new("no-main");
        let root = std::env::temp_dir().join("palantirmc-engine-forge").join("no-main-cache");
        let _ = std::fs::remove_dir_all(&root);
        let fetch = Arc::new(MapFetch::new());
        let meta = InstallerMeta::new(MetadataCache::new(root, Duration::ZERO), fetch.clone());
        let store = ContentStore::new(dirs.base.join("store"));
        let installer = write_jar(&[("version.json", "{}"), ("install_profile.json", "{}")]);
        script_artifact(&fetch, FORGE_MAVEN, "test:headless:1", &manifestless_jar());

        let spec = InstallSpec {
            minecraft: "1.21.1".to_string(),
            version: "1.21.1-forge-1".to_string(),
            libraries: Vec::new(),
            data: BTreeMap::new(),
            processors: vec![Processor {
                jar: "test:headless:1".to_string(),
                classpath: Vec::new(),
                args: vec!["--task".to_string(), "NOTHING".to_string()],
                outputs: BTreeMap::new(),
                sides: Vec::new(),
            }],
        };
        let error = install(
            Loader::Forge,
            &spec,
            &dirs.ctx(&meta, &store, &installer),
            &dirs.cancel,
            &dirs.backoff,
        )
        .expect_err("no Main-Class to run");
        let message = error.to_string();
        assert!(message.contains("processor 0 (test:headless:1)"), "{message}");
        assert!(message.contains("Main-Class"), "{message}");
    }

    #[test]
    fn a_coordinate_no_root_states_a_digest_for_is_taken_on_trust() {
        use crate::engine::content::Digest;

        let dirs = TestDirs::new("trust");
        let root = std::env::temp_dir().join("palantirmc-engine-forge").join("trust-cache");
        let _ = std::fs::remove_dir_all(&root);
        let fetch = Arc::new(MapFetch::new());
        let meta = InstallerMeta::new(MetadataCache::new(root, Duration::ZERO), fetch.clone());
        let store = ContentStore::new(dirs.base.join("store"));

        // No sidecar anywhere: both roots 404 it. The jar itself is served.
        let body = b"trusted bytes";
        fetch.set_route("https://a.invalid/t/tru/3/tru-3.jar", Route::body(body.to_vec()));
        let path = artifact_path(
            &meta,
            &store,
            &dirs.libraries,
            "t:tru:3",
            &["https://a.invalid/", "https://b.invalid/"],
            &dirs.cancel,
            &dirs.backoff,
        )
        .expect("the trust fallback");
        assert_eq!(std::fs::read(&path).expect("the file"), body);
        // ...but outside the store, which is what tells the two apart.
        assert!(
            !store.verified(&Digest::sha1(body)),
            "an unchecked file must not be mistaken for a checked one"
        );
    }

    #[test]
    fn java_is_found_on_a_path_before_it_is_scanned_for() {
        let base = std::env::temp_dir().join("palantirmc-engine-forge").join("find-java");
        let _ = std::fs::remove_dir_all(&base);
        let first = base.join("first");
        let second = base.join("second");
        std::fs::create_dir_all(&first).expect("first");
        std::fs::create_dir_all(&second).expect("second");
        assert!(find_java_in(&[first.clone(), second.clone()]).is_none());
        std::fs::write(second.join(java_exe_name()), b"").expect("a java");
        assert_eq!(find_java_in(&[first, second.clone()]), Some(second.join(java_exe_name())));
    }
}
