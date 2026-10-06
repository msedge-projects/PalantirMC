//! The metadata a launch resolves through: where one version file comes from.
//!
//! `resolve` asks one question per component -- the version file for a uid and a
//! version -- and this module is the answer. Two of the questions are answered by
//! the service that published the thing, through [`crate::wire::Wire`]; the rest
//! are still answered by Prism's mirror, and what is left there is measured
//! rather than assumed:
//!
//! * **Minecraft's own version file**, which is piston's -- read through
//!   `engine::piston` and translated into the shape this launcher's model reads by
//!   `palantir_core::version::mojang`, whose documentation carries the tables. The
//!   mirror's copy of the same file is that translation done elsewhere, which is
//!   why this is a slice rather than a changed URL.
//! * **Forge and NeoForge.** Their launch profile is inside an installer jar, and
//!   a launcher is expected to *run* that installer's processors -- they patch the
//!   client jar and unpack maven artifacts -- so there is no document a service
//!   serves. [`Loader::profile_url`] answers `None` for them and carries the
//!   measurement. This store answers them from the installer's own `version.json`
//!   (`engine::forge::InstallerMeta`) rather than from Prism's rewrite of that file
//!   around ForgeWrapper (G119): the wrapper's job is to run those processors at
//!   *launch*, and this launcher has already run them at install (G100).
//! * **Everything else**, including the mappings components
//!   (`net.fabricmc.intermediary`, `org.quiltmc.hashed`) an *imported* instance may
//!   list. A Fabric instance this launcher creates lists two components, and the
//!   loader's own profile already carries the mappings jar for its game version
//!   among its libraries -- which is why the mirror's `requires` for a second
//!   component is not needed here.
//!
//! ## What the loader's own file does not carry
//!
//! Two things this launcher would read and cannot: a digest per library (Fabric
//! and Quilt put `sha1` at the top level of a library entry, where this launcher's
//! model reads it under `downloads.artifact`, and Prism's copy drops it too), and
//! the loader's own JVM arguments (Fabric's profile has
//! `-DFabricMcEmu= net.minecraft.client.main.Main`, which the mirror's copy also
//! does not carry). Neither is a regression against the mirror; both are named so
//! that the next reader does not have to measure them again.

use palantir_core::error::Error as CoreError;
use palantir_core::instance::Instance;
use palantir_core::pack::PackProfile;
use palantir_core::paths::PalantirPaths;
use palantir_core::resolve::{MetaStore, VersionEntry};
use palantir_core::version::VersionFile;
use palantir_net::engine::{
    Backoff, Cancel, ContentStore, InstallerMeta, Loader, LoaderMeta, PistonMeta,
};
use palantir_net::{OnlineMetaStore, DEFAULT_META_BASE_URL};

use crate::wire::Wire;

/// The uid of the Minecraft component, which is also the version a loader's own
/// profile URL is built for.
const MINECRAFT_UID: &str = "net.minecraft";

/// A launcher's metadata for one instance's launch.
pub struct PublisherMeta {
    /// The loaders' own profiles, over the wire's cache and client.
    loaders: LoaderMeta,
    /// Forge's and NeoForge's installer jars, over the same cache and client:
    /// their launch profile is read out of the jar itself (G99), and the install
    /// that patches the client reads the same bytes.
    installers: InstallerMeta,
    /// Where the installer jar and the files its processors touch are filed by
    /// digest: the same store the launch's own install reads, so a launch after
    /// an install finds its client jar rather than fetching it again.
    content: ContentStore,
    /// Mojang's own version files, over the same cache and client.
    piston: PistonMeta,
    /// Prism's mirror, for every question the publishers above do not answer in
    /// the shape this launcher reads.
    mirror: OnlineMetaStore,
    /// The game version this instance runs, which is *part* of a loader profile's
    /// URL rather than a detail of it.
    ///
    /// `None` when the instance's pack profile names no Minecraft version, and
    /// then the mirror answers for the loader too: a URL built out of a version
    /// nobody could read would be a launch failing on a question the user never
    /// asked.
    game: Option<String>,
}

impl PublisherMeta {
    /// The store a launch uses: one instance's Minecraft version, the publishers'
    /// own services over `wire`, and the mirror beside them.
    pub fn for_instance(wire: &Wire, paths: &PalantirPaths, instance_id: &str) -> PublisherMeta {
        PublisherMeta::over(
            wire,
            DEFAULT_META_BASE_URL,
            paths.meta_dir(),
            paths.cache_dir().join("content"),
            instance_game(paths, instance_id),
        )
    }


    /// The same store with the mirror's base URL and directory, and the game
    /// version, named rather than read off an instance.
    fn over(
        wire: &Wire,
        mirror_base: impl Into<String>,
        mirror_dir: impl Into<std::path::PathBuf>,
        content_dir: impl Into<std::path::PathBuf>,
        game: Option<String>,
    ) -> PublisherMeta {
        PublisherMeta {
            loaders: wire.loaders(),
            installers: wire.installers(),
            content: ContentStore::new(content_dir),
            piston: wire.piston(),
            mirror: OnlineMetaStore::new(mirror_base, mirror_dir),
            game,
        }
    }

    /// The route one question takes, given what this store knows.
    ///
    /// Its own function because it is the whole of the decision, and because it
    /// can be read and tested without a service: a uid whose publisher serves a
    /// profile, and an instance that names the game version to ask it about, and
    /// the publisher answers; anything else is the mirror's.
    fn source(&self, uid: &str) -> Source {
        if uid == MINECRAFT_UID {
            return Source::Piston;
        }
        match (published_loader(uid), self.game.as_deref()) {
            (Some(loader), Some(_)) => Source::Publisher(loader),
            _ => Source::Mirror,
        }
    }
}

impl MetaStore for PublisherMeta {
    fn version_file(&mut self, uid: &str, version: &str) -> Result<VersionFile, CoreError> {
        match self.source(uid) {
            Source::Piston => self.piston.translated(
                version,
                MINECRAFT_UID,
                &Cancel::new(),
                &Backoff::default(),
            ),
            Source::Publisher(loader) => {
                // `source` only answers `Publisher` when the game version is
                // known, which is the middle of the URL this asks for.
                let game = self.game.clone().unwrap_or_default();
                match loader {
                    // The two Forge-shaped loaders publish no profile document:
                    // theirs is read out of their own installer jar and
                    // translated here (G99), and the install the profile belongs
                    // to runs before any launch plans a file (G119).
                    Loader::Forge | Loader::NeoForge => self.installers.profile(
                        loader,
                        &game,
                        version,
                        &self.content,
                        &Cancel::new(),
                        &Backoff::default(),
                    ),
                    Loader::Fabric | Loader::Quilt => self.loaders.profile(
                        loader,
                        &game,
                        version,
                        &Cancel::new(),
                        &Backoff::default(),
                    ),
                }
            }
            Source::Mirror => self.mirror.version_file(uid, version),
        }
    }

    fn version_list(&mut self, uid: &str) -> Result<Vec<VersionEntry>, CoreError> {
        // A launch never asks this: the versions are the ones the instance's own
        // pack profile pins, and the *pickers* read `engine::loaders` directly.
        // Delegated rather than emptied, because "the mirror's list" is still the
        // honest answer for the callers that do ask.
        self.mirror.version_list(uid)
    }
}

/// Where one version file comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    /// Mojang's own service, whose file this launcher translates.
    Piston,
    /// The loader's own service, which publishes a profile per game version.
    Publisher(Loader),
    /// Prism's mirror, for the questions the publishers do not answer in the
    /// shape this launcher reads.
    Mirror,
}

/// The loader a publisher serves a launch profile for, by component uid.
///
/// All four are here. Forge and NeoForge answer through their own installer jar
/// rather than a URL ([`Loader::profile_url`] is still `None` for them); the flip
/// from Prism's ForgeWrapper rewrite to the publisher's own translated profile is
/// G119, and G107 measured what it changes: the wrapper runs the installer's
/// processors at launch, which is a job the install has already done.
fn published_loader(uid: &str) -> Option<Loader> {
    match uid {
        "net.fabricmc.fabric-loader" => Some(Loader::Fabric),
        "org.quiltmc.quilt-loader" => Some(Loader::Quilt),
        "net.minecraftforge" => Some(Loader::Forge),
        "net.neoforged" => Some(Loader::NeoForge),
        _ => None,
    }
}

/// The Minecraft version an instance runs, read from its pack profile.
///
/// The same component the resolver pins first, read here because a loader's own
/// profile URL needs the game version before `resolve` has decided anything about
/// it. A profile that cannot be read, or one with no Minecraft component, has no
/// game version to offer -- which is a state the store can be in, not an error
/// worth failing a launch over: the mirror answers instead.
fn instance_game(paths: &PalantirPaths, instance_id: &str) -> Option<String> {
    let instance = Instance::open(&paths.configured_instances_dir().join(instance_id)).ok()?;
    let profile = PackProfile::load(&instance.mmc_pack_path()).ok()?;
    let version = profile.get(MINECRAFT_UID)?.version.trim().to_string();
    (!version.is_empty()).then_some(version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Arc;

    use crate::wire::Script;
    use palantir_core::instance::Instance;
    use palantir_core::resolve::resolve;
    use palantir_core::version::{ProblemSeverity, RuntimeContext};
    use palantir_net::engine::Digest;
    use palantir_net::PISTON_MANIFEST_URL;

    /// A jar from entry names and bodies, stored rather than compressed: the
    /// fixtures care about the JSON inside, not the codec.
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

    /// A store over a service that answers nothing, for the questions that are
    /// about routing: what is asked of whom, without a document anywhere.
    fn routing(game: Option<&str>) -> PublisherMeta {
        let dir = std::env::temp_dir().join("palantirmc-meta-routing");
        PublisherMeta::over(
            &Script::new().wire(),
            "https://mirror.invalid/v1",
            &dir,
            dir.join("content"),
            game.map(str::to_string),
        )
    }

    #[test]
    fn the_publishers_answer_for_their_own_loaders_and_the_mirror_for_the_rest() {
        let store = routing(Some("1.21.4"));
        assert_eq!(store.source("net.minecraft"), Source::Piston);
        assert_eq!(store.source("net.fabricmc.fabric-loader"), Source::Publisher(Loader::Fabric));
        assert_eq!(store.source("org.quiltmc.quilt-loader"), Source::Publisher(Loader::Quilt));
        // The two Forge-shaped uids are the publishers' now, through their
        // installer jars; the mappings components an imported instance may list
        // are still the mirror's.
        assert_eq!(store.source("net.minecraftforge"), Source::Publisher(Loader::Forge));
        assert_eq!(store.source("net.neoforged"), Source::Publisher(Loader::NeoForge));
        assert_eq!(store.source("net.fabricmc.intermediary"), Source::Mirror);
    }

    #[test]
    fn an_instance_that_names_no_game_version_leaves_the_loader_to_the_mirror() {
        // A loader profile is published per game version, so the question cannot
        // be asked without one. Answering from the mirror is the choice; failing
        // a launch over a URL nobody could build is the other.
        assert_eq!(routing(None).source("net.fabricmc.fabric-loader"), Source::Mirror);
    }

    #[test]
    fn a_fabric_instance_resolves_its_loader_from_fabric() {
        // The whole chain, minus the network: the game version comes off the pack
        // profile the create flow wrote, the URL comes out of it, and the document
        // that arrives is the loader's own launch profile.
        let dir = tempfile::tempdir().expect("a scratch directory");
        let paths = PalantirPaths::at(dir.path());
        let created = crate::instances::create(
            &paths,
            &crate::instances::NewInstance {
                loader: crate::catalog::LoaderKind::Fabric,
                loader_build: Some("0.19.5".to_string()),
                ..crate::instances::NewInstance::vanilla("Fabricly", "1.21.4")
            },
        )
        .expect("an instance");

        let url = Loader::Fabric
            .profile_url("1.21.4", "0.19.5")
            .expect("Fabric publishes one");
        let mut script = Script::new();
        script.insert_str(
            &url,
            r#"{
              "id": "fabric-loader-0.19.5-1.21.4",
              "inheritsFrom": "1.21.4",
              "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
              "arguments": { "game": [] },
              "libraries": [
                { "name": "net.fabricmc:intermediary:1.21.4", "url": "https://maven.fabricmc.net/" },
                { "name": "net.fabricmc:fabric-loader:0.19.5", "url": "https://maven.fabricmc.net/" }
              ]
            }"#,
        );
        let wire = script.wire();
        let mut store = PublisherMeta::for_instance(&wire, &paths, &created.id);

        let file = store
            .version_file("net.fabricmc.fabric-loader", "0.19.5")
            .expect("the loader's own profile");
        assert_eq!(file.main_class, "net.fabricmc.loader.impl.launch.knot.KnotClient");
        assert_eq!(file.libraries.len(), 2, "the mappings jar is one of its libraries");
        assert!(file.requires.is_empty(), "nothing to reach the mappings by");
    }

    /// The flip, end to end and without a network: an instance whose pack names
    /// Forge resolves through the installer jar on Forge's own maven, and the
    /// profile that comes back is the loader's own launch -- ForgeBootstrap --
    /// rather than the ForgeWrapper file the mirror serves. The mirror's base URL
    /// is one the scripted service has nothing at, so a question asked of it
    /// would fail the resolution rather than pass unnoticed, and the request
    /// count is asserted for the same reason.
    #[test]
    fn a_forge_instance_resolves_its_loader_from_its_own_installer() {
        let dir = tempfile::tempdir().expect("a scratch directory");
        let paths = PalantirPaths::at(dir.path());
        let created = crate::instances::create(
            &paths,
            &crate::instances::NewInstance {
                loader: crate::catalog::LoaderKind::Forge,
                loader_build: Some("52.1.0".to_string()),
                ..crate::instances::NewInstance::vanilla("Forged", "1.21.1")
            },
        )
        .expect("an instance");

        // The loader's own file, in the shape the publisher's really has: its
        // main class and a library naming the client, and no `mainJar` at all
        // -- that is what the install's `PATCHED` entry is for, because a
        // profile with no `mainJar` falls back to a Mojang client jar built
        // out of `id` ("1.21.1-forge-52.1.0"), which is not a version and
        // which the parser records as an error the resolver refuses to launch
        // on. No wrapper anywhere -- if this test's profile came from the
        // mirror, the main class would be ForgeWrapper's rather than the
        // loader's.
        let version = serde_json::json!({
            "id": "1.21.1-forge-52.1.0",
            "mainClass": "net.minecraftforge.bootstrap.ForgeBootstrap",
            "arguments": { "game": ["--launchTarget", "forge_client"] },
            "libraries": [{
                "name": "net.minecraftforge:forge:1.21.1-52.1.0:client",
                "url": "https://maven.minecraftforge.invalid/"
            }]
        });
        let install = serde_json::json!({
            "spec": 1, "profile": "forge", "version": "1.21.1-forge-52.1.0",
            "minecraft": "1.21.1", "libraries": [],
            "data": { "PATCHED": "[net.minecraftforge:forge:1.21.1-52.1.0:client]" },
            "processors": []
        });
        let jar = write_jar(&[
            ("version.json", &version.to_string()),
            ("install_profile.json", &install.to_string()),
        ]);

        // Mojang's half of the same resolution, in the shape piston serves.
        const VANILLA_URL: &str = "https://piston.invalid/1.21.1.json";
        const VANILLA: &str = r#"{
          "id": "1.21.1", "type": "release", "releaseTime": "2024-08-08T12:00:00+00:00",
          "mainClass": "net.minecraft.client.main.Main", "assets": "17",
          "assetIndex": {"id": "17", "sha1": "aa", "size": 1, "totalSize": 1,
                         "url": "https://piston.invalid/17.json"},
          "javaVersion": {"component": "java-runtime-delta", "majorVersion": 21},
          "downloads": {"client": {"sha1": "bb", "size": 2, "url": "https://piston.invalid/client.jar"}},
          "arguments": {"jvm": [], "game": []},
          "libraries": []
        }"#;
        let digest = Digest::sha1(VANILLA.as_bytes()).hex().to_string();
        let installer_url = palantir_net::engine::installer_url(Loader::Forge, "1.21.1", "52.1.0")
            .expect("Forge publishes one");
        let mut script = Script::new();
        script.insert_str(
            PISTON_MANIFEST_URL,
            &format!(
                r#"{{"latest": {{"release": "1.21.1", "snapshot": "25w02a"}},
                    "versions": [{{"id": "1.21.1", "type": "release",
                                   "url": "{VANILLA_URL}",
                                   "releaseTime": "2024-08-08T12:00:00+00:00",
                                   "sha1": "{digest}"}}]}}"#
            ),
        );
        script.insert_str(VANILLA_URL, VANILLA);
        script.insert(installer_url.clone(), jar.clone());
        script.insert(
            format!("{installer_url}.sha1"),
            Digest::sha1(&jar).hex().to_string(),
        );

        let fetch = Arc::new(script.fetch());
        let wire = Wire::over(dir.path().join("cache"), fetch.clone());
        let mut store = PublisherMeta::for_instance(&wire, &paths, &created.id);

        let instance = Instance::open(&paths.instances_dir().join(&created.id))
            .expect("the instance on disk");
        let profile = PackProfile::load(&instance.mmc_pack_path()).expect("its pack profile");
        let resolution = resolve(
            &profile,
            &instance.patches_dir(),
            &mut store,
            &RuntimeContext::current_host(),
        )
        .expect("a resolution");

        assert_eq!(
            resolution.severity(),
            ProblemSeverity::None,
            "problems: {:?}, components: {:?}",
            resolution.problems,
            resolution
                .components
                .iter()
                .map(|component| (&component.uid, &component.problems))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            resolution.profile.main_class, "net.minecraftforge.bootstrap.ForgeBootstrap",
            "the loader's own launch, not ForgeWrapper's"
        );
        assert!(
            resolution
                .profile
                .libraries
                .iter()
                .any(|library| library.name.serialize()
                    == "net.minecraftforge:forge:1.21.1-52.1.0:client"),
            "the profile names the client the install patches: {:?}",
            resolution
                .profile
                .libraries
                .iter()
                .map(|library| library.name.serialize())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            resolution
                .profile
                .main_jar
                .as_ref()
                .map(|jar| jar.name.serialize()),
            Some("net.minecraftforge:forge:1.21.1-52.1.0:client".to_string()),
            "the launch's main jar is the patched client, not Mojang's"
        );
        // Four documents: piston's manifest and version file, and the
        // installer jar with the sidecar that states its digest. A fifth would
        // be the mirror being consulted, which is the flip this slice undoes.
        assert_eq!(
            fetch.count(),
            4,
            "requests: {:?}",
            fetch.requests().iter().map(|request| &request.url).collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_vanilla_instance_resolves_its_game_file_from_piston_alone() {
        // The slice's claim, end to end and without a network: the profile names
        // `net.minecraft`, the document arrives from Mojang's own service, and the
        // classpath the resolution builds carries the LWJGL libraries the game's
        // file names -- which is what Prism's copy of the same version does not
        // name, because it serves those as a component of its own. The mirror's
        // base URL is one the scripted service has nothing at, so a single
        // question asked of it would come back as a problem: the resolution being
        // clean is what makes "from piston alone" a measurement rather than a
        // reading of `source`.
        let dir = tempfile::tempdir().expect("a scratch directory");
        let paths = PalantirPaths::at(dir.path());
        let created = crate::instances::create(
            &paths,
            &crate::instances::NewInstance::vanilla("Plainly", "1.21.4"),
        )
        .expect("an instance");

        // Mojang's shape, trimmed: the game's arguments with the pair this
        // launcher cannot fill, the Java the version needs, the client jar, and
        // an LWJGL entry of the kind 1.19 and later publish as its own entry.
        const BODY: &str = r#"{
          "id": "1.21.4", "type": "release", "releaseTime": "2024-12-03T10:12:57+00:00",
          "mainClass": "net.minecraft.client.main.Main", "assets": "19",
          "assetIndex": {"id": "19", "sha1": "aa", "size": 10, "totalSize": 20,
                         "url": "https://piston.invalid/19.json"},
          "javaVersion": {"component": "java-runtime-delta", "majorVersion": 21},
          "downloads": {"client": {"sha1": "bb", "size": 28,
                                   "url": "https://piston.invalid/client.jar"}},
          "arguments": {"jvm": ["-Djava.library.path=${natives_directory}"],
                        "game": ["--username", "${auth_player_name}",
                                 "--clientId", "${clientid}", "--xuid", "${auth_xuid}"]},
          "libraries": [
            {"name": "com.mojang:brigadier:1.3.10",
             "downloads": {"artifact": {"path": "com/mojang/brigadier/1.3.10/brigadier-1.3.10.jar",
                                        "sha1": "cc", "size": 5,
                                        "url": "https://libraries.invalid/brigadier.jar"}}},
            {"name": "org.lwjgl:lwjgl:3.3.3",
             "downloads": {"artifact": {"path": "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3.jar",
                                        "sha1": "dd", "size": 6,
                                        "url": "https://libraries.invalid/lwjgl.jar"}}}
          ]
        }"#;
        let digest = Digest::sha1(BODY.as_bytes()).hex().to_string();
        let mut script = Script::new();
        script.insert_str(
            PISTON_MANIFEST_URL,
            &format!(
                r#"{{"latest": {{"release": "1.21.4", "snapshot": "25w02a"}},
                    "versions": [{{"id": "1.21.4", "type": "release",
                                   "url": "https://piston.invalid/1.21.4.json",
                                   "releaseTime": "2024-12-03T10:12:57+00:00",
                                   "sha1": "{digest}"}}]}}"#
            ),
        );
        script.insert_str("https://piston.invalid/1.21.4.json", BODY);

        let wire = script.wire();
        let mut store = PublisherMeta::over(
            &wire,
            "https://mirror.invalid/v1",
            dir.path().join("meta"),
            dir.path().join("content"),
            Some("1.21.4".to_string()),
        );
        let instance = Instance::open(&paths.instances_dir().join(&created.id))
            .expect("the instance on disk");
        let profile = PackProfile::load(&instance.mmc_pack_path()).expect("its pack profile");
        let resolution = resolve(
            &profile,
            &instance.patches_dir(),
            &mut store,
            &RuntimeContext::current_host(),
        )
        .expect("a resolution");

        assert_eq!(
            resolution.severity(),
            ProblemSeverity::None,
            "problems: {:?}",
            resolution.problems
        );
        assert_eq!(resolution.profile.main_class, "net.minecraft.client.main.Main");
        assert_eq!(resolution.profile.compatible_java_majors, vec![21]);
        assert_eq!(
            resolution.profile.minecraft_arguments,
            "--username ${auth_player_name}",
            "the pair the launcher cannot fill is not on the command line"
        );
        // LWJGL is on the classpath because the game's file names it, and the one
        // component the profile names is what put it there.
        assert!(
            resolution
                .profile
                .libraries
                .iter()
                .any(|library| library.name.group() == "org.lwjgl"),
            "{:?}",
            resolution
                .profile
                .libraries
                .iter()
                .map(|library| library.name.serialize())
                .collect::<Vec<_>>()
        );
    }

}
