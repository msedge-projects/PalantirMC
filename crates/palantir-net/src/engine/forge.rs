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
//! ## Where the bytes come from
//!
//! The installer jar travels through the engine's metadata cache, believed for
//! [`crate::engine::cache::IMMUTABLE_TTL`] like a version file: it describes a
//! released build, so it is not rewritten. The processor jars G100 resolves
//! travel through the content store instead, digest-checked against the `.sha1`
//! sidecar their maven publishes -- an installer names no digest for them, and
//! the sidecar is the publisher's own word for what the bytes should be.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;

use palantir_core::version::VersionFile;

use crate::engine::cache::{MetadataCache, IMMUTABLE_TTL};
use crate::engine::cancel::Cancel;
use crate::engine::loaders::Loader;
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
        Loader::Forge => Some(format!(
            "{FORGE_MAVEN}net/minecraftforge/forge/{game}-{build}/forge-{game}-{build}-installer.jar"
        )),
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

    let text_of = |key: &str| {
        install_object.get(key).and_then(serde_json::Value::as_str).unwrap_or_default().to_string()
    };
    Ok(ParsedInstaller {
        version_json,
        install: InstallSpec {
            minecraft: text_of("minecraft"),
            version: text_of("version"),
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
    /// Blocking, like every engine call.
    pub fn installer_bytes(
        &self,
        loader: Loader,
        game: &str,
        build: &str,
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
        let cached = self.documents.get(&url, self.fetch.as_ref(), cancel, backoff)?;
        Ok(cached.body)
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
        let bytes = self.installer_bytes(loader, game, build, cancel, backoff).map_err(Error::into_core)?;
        let parsed = parse_installer(&bytes, &url).map_err(Error::into_core)?;
        let uid = component_uid(loader);
        let translated = translate_profile(&parsed.version_json, uid, build)
            .map_err(|detail| palantir_core::error::Error::json(&url, detail))?;
        VersionFile::parse(&translated, &PathBuf::from(&url), false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::cache::DEFAULT_TTL;
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

    /// A handle over a scratch directory whose installer URL serves `jar`, so
    /// a test that asks for anything else fails instead of dialling out.
    fn meta(name: &str, url: &str, jar: Vec<u8>) -> (InstallerMeta, Arc<MapFetch>) {
        let root = std::env::temp_dir().join("palantirmc-engine-forge").join(name);
        let _ = std::fs::remove_dir_all(&root);
        let fetch = Arc::new(MapFetch::new());
        fetch.set_route(url, Route::body(jar));
        let meta = InstallerMeta::new(MetadataCache::new(root, Duration::ZERO), fetch.clone());
        (meta, fetch)
    }

    #[test]
    fn a_modern_profile_comes_out_of_the_installer_with_the_game_args_as_its_string() {
        let url = installer_url(Loader::Forge, "1.21.1", "52.1.0").expect("Forge publishes one");
        let (meta, fetch) = meta("modern", &url, modern_jar());
        let cancel = Cancel::new();
        let backoff = Backoff::with_attempts(1);

        let file = meta
            .profile(Loader::Forge, "1.21.1", "52.1.0", &cancel, &backoff)
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
        // Asked again, the jar is already held: one installer, one request.
        meta.profile(Loader::Forge, "1.21.1", "52.1.0", &cancel, &backoff).expect("again");
        assert_eq!(fetch.count(), 1, "the second ask was answered from disk");
    }

    #[test]
    fn an_old_build_is_read_from_version_info_with_its_string_kept() {
        let url = installer_url(Loader::Forge, "1.12.2", "14.23.5.2860").expect("one");
        let (meta, _) = meta("version-info", &url, version_info_jar());
        let file = meta
            .profile(Loader::Forge, "1.12.2", "14.23.5.2860", &Cancel::new(), &Backoff::with_attempts(1))
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
        let (meta, _) = meta("ancient", &url, ancient_jar());
        let error = meta
            .profile(Loader::Forge, "1.6.4", "9.11.1.1345", &Cancel::new(), &Backoff::with_attempts(1))
            .expect_err("nothing to read");
        let message = error.to_string();
        assert!(message.contains(&url), "{message}");
        assert!(message.contains("refused"), "{message}");
    }

    #[test]
    fn data_entries_are_classified_and_read_client_side() {
        let parsed = parse_installer(&modern_jar(), "<test>").expect("the modern jar");
        assert_eq!(parsed.install.minecraft, "1.21.1");
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
        let meta = InstallerMeta::new(MetadataCache::new(root, DEFAULT_TTL), fetch.clone());
        assert!(installer_url(Loader::Fabric, "1.21.4", "0.19.5").is_none());
        let error = meta
            .profile(Loader::Fabric, "1.21.4", "0.19.5", &Cancel::new(), &Backoff::with_attempts(1))
            .expect_err("no installer to read");
        assert!(error.to_string().contains("no installer"), "{error}");
        assert_eq!(fetch.count(), 0, "and nothing was asked for");
    }

    #[test]
    fn a_body_that_is_not_a_jar_names_the_url_it_came_from() {
        let url = installer_url(Loader::NeoForge, "1.21.1", "21.1.172").expect("one");
        let (meta, _) = meta("broken", &url, b"not a jar".to_vec());
        let failure = meta
            .profile(Loader::NeoForge, "1.21.1", "21.1.172", &Cancel::new(), &Backoff::with_attempts(1))
            .expect_err("a body that is not a jar");
        assert!(failure.to_string().contains(&url), "{failure}");
    }
}
