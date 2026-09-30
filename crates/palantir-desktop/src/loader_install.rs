//! The Forge-shaped loaders' install step: what runs before a launch plans a
//! single file, because the files that plan would name do not exist until it has.
//!
//! Forge and NeoForge publish no launch profile document. Theirs is inside an
//! installer jar, and a launcher is expected to *run* that installer's
//! processors: they patch the client jar and unpack the maven artifacts the
//! profile names (G99, G100). Since G119 the resolve reads that profile out of
//! the publisher's own jar ([`crate::meta`]), so the plan a Forge instance
//! resolves to names products only this step writes. The order is therefore not
//! a preference: install first, plan second.
//!
//! What this module is, exactly, is the *instance* half of that: which loader an
//! instance's pack names, which game and build that is, where this launcher keeps
//! the content store and the shared libraries, and what reaches the log. The
//! installer itself -- fetching its jar against the digest its maven states,
//! fetching Mojang's own client jar, running the processors -- is
//! `palantir-net`'s `engine::forge`, so the live test and the launch run the same
//! sequence rather than two descriptions of one.

use std::path::Path;

use palantir_core::pack::PackProfile;
use palantir_core::paths::PalantirPaths;
use palantir_net::engine::{install_client, Backoff, Cancel, ClientInstall, ContentStore, Loader};

use crate::wire::Wire;

/// The loader an instance's pack names, and the game and build its installer is
/// asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoaderJob {
    /// Forge or NeoForge: the two whose install is an installer jar.
    pub loader: Loader,
    /// The game version, from the pack's `net.minecraft` component.
    pub game: String,
    /// The component version, as the pack spells it (`52.1.0`, or Prism's
    /// `1.21.1-52.1.0` on an imported instance): the installer's URL knows both
    /// spellings (`engine::forge::installer_url`), and this stays the string a
    /// reader sees on the instance's own card.
    pub build: String,
}

impl LoaderJob {
    /// How the loader is named in a log line.
    pub fn label(&self) -> &'static str {
        match self.loader {
            Loader::Forge => "Forge",
            Loader::NeoForge => "NeoForge",
            // The constructor only ever builds these two. The other arms are
            // named rather than folded into one: a loader this module was not
            // built for should read as itself in a log.
            Loader::Fabric => "Fabric",
            Loader::Quilt => "Quilt",
        }
    }
}

/// Which Forge-shaped loader `profile` names, if any.
///
/// `None` for vanilla, Fabric and Quilt -- their resolve needs no install step --
/// and for a loader component that is disabled, or one whose pack names no
/// Minecraft version for the installer to be matched against. A component with
/// an empty build is skipped for the same reason `instances::create` refuses
/// one: there is nothing to install and a URL built out of nothing is not a
/// question worth asking a service.
pub fn forge_shaped(profile: &PackProfile) -> Option<LoaderJob> {
    let game = profile
        .get("net.minecraft")
        .map(|component| component.version.trim().to_string())
        .filter(|version| !version.is_empty())?;
    for component in profile.components() {
        if !component.is_enabled() {
            continue;
        }
        let loader = match component.uid.as_str() {
            "net.minecraftforge" => Loader::Forge,
            "net.neoforged" => Loader::NeoForge,
            _ => continue,
        };
        let build = component.version.trim().to_string();
        if build.is_empty() {
            continue;
        }
        return Some(LoaderJob { loader, game, build });
    }
    None
}

/// Run `job`'s installer for the instance at `instance_root`.
///
/// `java` is the runtime the processors run on. The caller locates it, because
/// a launch has already chosen one and a machine with none should hear so before
/// this starts rather than halfway through a processor chain. `wire` is the
/// launcher's one client and one cache; the content store and the scratch
/// directory sit under the launcher's own `cache/`, so a second launch of the
/// same build resumes rather than downloads again.
///
/// **Blocking**, like every engine call: the shell runs a launch on its worker,
/// never on the frame thread.
pub fn install(
    paths: &PalantirPaths,
    instance_root: &Path,
    job: &LoaderJob,
    wire: &Wire,
    java: &Path,
    log: &mut dyn FnMut(String),
) -> Result<(), String> {
    let installers = wire.installers();
    let piston = wire.piston();
    let store = ContentStore::new(paths.cache_dir().join("content"));
    let scratch = paths
        .cache_dir()
        .join("loader")
        .join(format!("{}-{}-{}", job.loader.name(), job.game, job.build));
    let cancel = Cancel::new();
    let backoff = Backoff::default();
    let report = install_client(
        job.loader,
        &job.game,
        &job.build,
        &ClientInstall {
            meta: &installers,
            piston: &piston,
            fetch: wire.fetch().as_ref(),
            store: &store,
            root: instance_root,
            library_dir: &paths.libraries_dir(),
            scratch: &scratch,
            java,
            cancel: &cancel,
            backoff: &backoff,
        },
    )
    .map_err(|error| {
        format!(
            "{} {} for Minecraft {} could not be installed: {error}",
            job.label(),
            job.build,
            job.game
        )
    })?;
    log(format!(
        "{} {}: {} processor(s) ran, {} skipped",
        job.label(),
        job.build,
        report.ran(),
        report.skipped()
    ));
    if let Some(patched) = &report.patched_client {
        let bytes = std::fs::metadata(patched).map(|meta| meta.len()).unwrap_or_default();
        log(format!("patched client: {} ({bytes} bytes)", patched.display()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use palantir_net::engine::request::{MapFetch, Route};
    use palantir_net::engine::Digest;
    use palantir_net::PISTON_MANIFEST_URL;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::Arc;

    /// A pack profile from components, in the shape `mmc-pack.json` writes.
    fn pack(components: &[(&str, &str)]) -> PackProfile {
        let entries: Vec<String> = components
            .iter()
            .map(|(uid, version)| format!(r#"{{"uid":"{uid}","version":"{version}"}}"#))
            .collect();
        let text = format!(r#"{{"formatVersion":1,"components":[{}]}}"#, entries.join(","));
        PackProfile::from_text(&text, &PathBuf::from("mmc-pack.json")).expect("a pack profile")
    }

    /// A jar from entry names and bodies, stored rather than compressed.
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

    /// An installer jar for a build with no client-side processors, so an
    /// install of it runs nothing and the test needs no Java tool to exist.
    fn idle_installer(minecraft: &str) -> Vec<u8> {
        let version = serde_json::json!({
            "id": format!("{minecraft}-forge-52.1.0"),
            "mainClass": "net.minecraftforge.bootstrap.ForgeBootstrap",
            "arguments": { "game": [] },
            "libraries": []
        });
        let profile = serde_json::json!({
            "spec": 1,
            "profile": "forge",
            "version": format!("{minecraft}-forge-52.1.0"),
            "minecraft": minecraft,
            "libraries": [],
            "data": {},
            "processors": []
        });
        write_jar(&[
            ("version.json", &version.to_string()),
            ("install_profile.json", &profile.to_string()),
        ])
    }

    #[test]
    fn a_pack_names_its_forge_shaped_loader() {
        // The create flow's spelling: the promotions' build alone.
        let job = forge_shaped(&pack(&[("net.minecraft", "1.21.1"), ("net.minecraftforge", "52.1.0")]))
            .expect("a Forge job");
        assert_eq!(job.loader, Loader::Forge);
        assert_eq!(job.game, "1.21.1");
        assert_eq!(job.build, "52.1.0");

        // Prism's spelling, which an imported instance carries: the component
        // version is passed through as the pack spells it, and the installer URL
        // is what knows the game in front of the build is a prefix
        // (`engine::forge::installer_url`, asserted there for both spellings).
        let job = forge_shaped(&pack(&[
            ("net.minecraft", "1.21.1"),
            ("net.minecraftforge", "1.21.1-52.1.0"),
        ]))
        .expect("a Forge job");
        assert_eq!(job.build, "1.21.1-52.1.0");

        let job = forge_shaped(&pack(&[("net.minecraft", "1.21.1"), ("net.neoforged", "21.1.172")]))
            .expect("a NeoForge job");
        assert_eq!(job.loader, Loader::NeoForge);
        assert_eq!(job.build, "21.1.172");

        // Vanilla, and the two loaders whose profile is a document, need no
        // install step at all.
        assert_eq!(
            forge_shaped(&pack(&[("net.minecraft", "1.21.1")])),
            None
        );
        assert_eq!(
            forge_shaped(&pack(&[
                ("net.minecraft", "1.21.1"),
                ("net.fabricmc.fabric-loader", "0.19.5"),
            ])),
            None
        );
        // A pack with no game version cannot ask an installer anything.
        assert_eq!(
            forge_shaped(&pack(&[("net.minecraftforge", "52.1.0")])),
            None
        );
    }

    #[test]
    fn a_disabled_loader_component_is_not_installed() {
        let text = r#"{"formatVersion":1,"components":[
            {"uid":"net.minecraft","version":"1.21.1"},
            {"uid":"net.minecraftforge","version":"52.1.0","disabled":true}
        ]}"#;
        let profile = PackProfile::from_text(text, &PathBuf::from("mmc-pack.json"))
            .expect("a pack profile");
        assert_eq!(forge_shaped(&profile), None);
    }

    /// The plumbing the launch depends on, without a network: the install reads
    /// its documents off the wire, files the client jar in the launcher's own
    /// content store, and reports the processors in the log the launch draws.
    #[test]
    fn an_install_files_the_client_in_the_launcher_s_own_cache() {
        let dir = tempfile::tempdir().expect("a scratch directory");
        let paths = PalantirPaths::at(dir.path());
        let instance_root = dir.path().join("instances").join("Forged");

        // Mojang's half: a manifest, a version file, and the client jar itself.
        let client = b"the client jar".to_vec();
        let client_sha1 = Digest::sha1(&client).hex().to_string();
        let version_url = "https://piston.invalid/1.21.1.json";
        let version = format!(
            r#"{{
              "id": "1.21.1", "type": "release", "releaseTime": "2024-08-08T12:00:00+00:00",
              "mainClass": "net.minecraft.client.main.Main", "assets": "17",
              "assetIndex": {{"id": "17", "sha1": "aa", "size": 1, "totalSize": 1,
                             "url": "https://piston.invalid/17.json"}},
              "javaVersion": {{"component": "java-runtime-delta", "majorVersion": 21}},
              "downloads": {{"client": {{"sha1": "{client_sha1}", "size": {},
                  "url": "https://piston.invalid/client.jar"}}}},
              "arguments": {{"jvm": [], "game": []}},
              "libraries": []
            }}"#,
            client.len()
        );
        let version_digest = Digest::sha1(version.as_bytes()).hex().to_string();
        let jar = idle_installer("1.21.1");
        let installer_url = palantir_net::engine::installer_url(Loader::Forge, "1.21.1", "52.1.0")
            .expect("Forge publishes one");

        let fetch = MapFetch::new();
        fetch.set_route(
            PISTON_MANIFEST_URL,
            Route::body(
                format!(
                    r#"{{"latest": {{"release": "1.21.1", "snapshot": "25w02a"}},
                        "versions": [{{"id": "1.21.1", "type": "release",
                                       "url": "{version_url}",
                                       "releaseTime": "2024-08-08T12:00:00+00:00",
                                       "sha1": "{version_digest}"}}]}}"#
                )
                .into_bytes(),
            ),
        );
        fetch.set_route(version_url, Route::body(version.into_bytes()));
        fetch.set_route("https://piston.invalid/client.jar", Route::body(client.clone()));
        fetch.set_route(&installer_url, Route::body(jar.clone()));
        fetch.set_route(&format!("{installer_url}.sha1"), Route::text(Digest::sha1(&jar).hex()));
        let fetch = Arc::new(fetch);
        let wire = Wire::over(paths.meta_dir(), fetch.clone());

        // Nothing in this fixture runs, so the "java" only has to be a file the
        // engine's own check accepts; nothing executes it.
        let java = dir.path().join("java.exe");
        std::fs::write(&java, b"").expect("a stand-in for a java binary");

        let job = LoaderJob { loader: Loader::Forge, game: "1.21.1".to_string(), build: "52.1.0".to_string() };
        let mut lines = Vec::new();
        install(&paths, &instance_root, &job, &wire, &java, &mut |line| lines.push(line))
            .expect("the install");

        assert!(
            lines.iter().any(|line| line.contains("Forge 52.1.0: 0 processor(s) ran, 0 skipped")),
            "{lines:?}"
        );
        // The client jar is filed under the launcher's cache, by digest (two
        // characters of fan-out, then the digest): that is what makes the launch
        // after this one cost no request for it.
        let client_path = paths
            .cache_dir()
            .join("content")
            .join(&client_sha1[..2])
            .join(&client_sha1);
        assert!(client_path.is_file(), "{}", client_path.display());
        // Five documents: piston's manifest and version file, the installer's
        // sidecar, the installer jar, and Mojang's client jar -- and no sixth.
        assert_eq!(fetch.count(), 5, "requests: {}", fetch.count());
    }

    /// A build the installer cannot produce is a launch that must not start: the
    /// sentence names the loader, the build and the game, because the failure is
    /// read in a console a reader is looking at while the instance does nothing.
    #[test]
    fn a_failed_install_says_which_build_failed() {
        let dir = tempfile::tempdir().expect("a scratch directory");
        let paths = PalantirPaths::at(dir.path());
        let fetch = Arc::new(MapFetch::new());
        let wire = Wire::over(paths.meta_dir(), fetch);
        let java = dir.path().join("java.exe");
        std::fs::write(&java, b"").expect("a stand-in for a java binary");

        let job = LoaderJob { loader: Loader::Forge, game: "1.21.1".to_string(), build: "52.1.0".to_string() };
        let error = install(
            &paths,
            &dir.path().join("instances").join("Forged"),
            &job,
            &wire,
            &java,
            &mut |_| {},
        )
        .expect_err("nothing is served");
        assert!(error.contains("Forge 52.1.0 for Minecraft 1.21.1"), "{error}");
    }
}
