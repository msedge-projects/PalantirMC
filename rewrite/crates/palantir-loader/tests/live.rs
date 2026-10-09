//! Phase 3's done-when, live: every loader installs into a temp data root
//! and the result launches headless Java.
//!
//! These tests are `#[ignore]`d like every network test in this project:
//! the offline suite is the deterministic gate, and this one asks the real
//! services for a game version each, a Java runtime each, and -- for the
//! Forge family -- the vendors' own installer jars. Modern game versions
//! are mostly their asset sets, so the five together move on the order of
//! 2 GB; 1.5.2 (vanilla) is the small one at ~100 MB. A failure here is a
//! failure: a service moved, a document changed, or an assumption broke.
//!
//! The launch half *watches* the spawned game process rather than waiting
//! for it to finish: a game with nowhere to draw either keeps running (the
//! test kills it after a grace period) or dies into its output, and both
//! prove that the fetched runtime executed the plan's main class with its
//! classpath intact. What is never acceptable -- and fails the test -- is
//! the JVM refusing the command: no main class, a missing class, a flag the
//! runtime does not know, or a natives tree the process was never pointed
//! at.
//!
//! Run with:
//! `cargo test -p palantir-loader --test live --locked -- --ignored --test-threads=1`

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use palantir_core::assets::{AssetDestination, AssetIndex};
use palantir_core::paths::DataRoot;
use palantir_core::rules::{Arch, Os, Platform};
use palantir_core::version::{Version, VersionManifest};
use palantir_loader::install::{VERSION_MANIFEST_URL, install_document};
use palantir_loader::installer::{InstallProfile, ProcessorContext, Side, plan_processors};
use palantir_loader::java::{
    JAVA_RUNTIME_ALL_URL, RuntimeChoice, RuntimeManifest, fetch_runtime, mojang_platform,
    parse_index, runtime_dir, select,
};
use palantir_loader::launch::{
    LaunchContext, LaunchPlan, build_launch_plan, extract_natives, run_processors,
};
use palantir_loader::profiles::{LoaderInstall, LoaderKind, LoaderSources, install_loader};
use palantir_net::cache::{MANIFEST_TTL, MetadataCache, VERSION_TTL, cache_dir_for};
use palantir_net::client::Http;
use palantir_net::download::{DownloadOptions, download};
use palantir_net::scheduler::Scheduler;
use palantir_net::store::ContentStore;
use palantir_net::sync::Syncer;

fn platform() -> Platform {
    Platform::new(Os::host(), "1.0", Arch::host())
}

/// One test's world: a temp directory holding a data root, a game
/// directory, and everything the install and launch put there.
struct Sandbox {
    dir: PathBuf,
    /// The data root's path, kept beside it: `ProcessorContext` wants a
    /// `&Path` and `DataRoot` hands out no accessor for its own.
    root_path: PathBuf,
    root: DataRoot,
    http: Http,
    cache: MetadataCache,
    scheduler: Scheduler,
    store: ContentStore,
}

impl Sandbox {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("palantirmc-live-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("game")).unwrap();
        let root_path = dir.join("root");
        let root = DataRoot::new(root_path.clone());
        let http = Http::new().unwrap();
        let cache = MetadataCache::new(cache_dir_for(&root));
        let store = ContentStore::new(root.content_dir());
        let scheduler = Scheduler::with_default_limit();
        Self {
            dir,
            root_path,
            root,
            http,
            cache,
            scheduler,
            store,
        }
    }

    fn game_dir(&self) -> PathBuf {
        self.dir.join("game")
    }

    fn syncer(&self) -> Syncer<'_> {
        Syncer::new(
            &self.http,
            &self.scheduler,
            &self.store,
            &self.root,
            DownloadOptions::default(),
        )
    }

    /// One version's document as the manifest serves it.
    fn version_document(&self, id: &str) -> String {
        let manifest_text = self
            .cache
            .fetch_text(
                &self.http,
                "version-manifest",
                VERSION_MANIFEST_URL,
                MANIFEST_TTL,
            )
            .unwrap();
        let manifest = VersionManifest::parse(&manifest_text).unwrap();
        let entry = manifest
            .find(id)
            .unwrap_or_else(|| panic!("the version manifest does not name {id}"));
        self.cache
            .fetch_text(
                &self.http,
                &format!("version-{id}"),
                &entry.url,
                VERSION_TTL,
            )
            .unwrap()
    }

    /// Install a document -- vanilla or an overlay -- the way an install
    /// does: resolved over its parents, written, needs landed.
    fn install(&self, document: &str) -> Version {
        let syncer = self.syncer();
        let report = install_document(
            &syncer,
            &self.cache,
            VERSION_MANIFEST_URL,
            &platform(),
            document,
            Some(&self.game_dir()),
        )
        .unwrap();
        assert!(
            report.version.inherits_from.is_none(),
            "the written document still names a parent"
        );
        println!("{}: installed ({:?})", report.version.id, report.sync);
        report.version
    }

    /// The runtime the version's own document asks for, landed and
    /// verified: the product index picks the manifest, the manifest
    /// places the files, `bin/java` comes back.
    fn java_for(&self, version: &Version) -> PathBuf {
        let component = version
            .java_version
            .as_ref()
            .map(|java| java.component.clone())
            // Documents from before the runtime index existed still get the
            // Java they were built for: jre-legacy is that one.
            .unwrap_or_else(|| "jre-legacy".to_string());
        let index_text = self
            .cache
            .fetch_text(
                &self.http,
                "java-runtime-all",
                JAVA_RUNTIME_ALL_URL,
                MANIFEST_TTL,
            )
            .unwrap();
        let index = parse_index(&index_text).unwrap();
        let choice = RuntimeChoice {
            platform: mojang_platform(&platform()).unwrap(),
            component: &component,
        };
        let entry = select(&index, &choice).unwrap();
        let manifest_text = self
            .cache
            .fetch_text(
                &self.http,
                &format!("java-manifest-{}-{}", choice.platform, choice.component),
                &entry.manifest.url,
                MANIFEST_TTL,
            )
            .unwrap();
        let manifest = RuntimeManifest::parse(&manifest_text).unwrap();
        let into = runtime_dir(&self.root.java_dir(), &choice);
        let report =
            fetch_runtime(&self.http, &manifest, &into, DownloadOptions::default()).unwrap();
        println!(
            "runtime {component} for {}: {} fetched, {} reused",
            choice.platform, report.fetched, report.reused
        );
        into.join("bin")
            .join(if cfg!(windows) { "java.exe" } else { "java" })
    }

    /// Where the installed version's assets are read from -- the layout
    /// the index asked for decides: the shared store, the game
    /// directory's `resources/`, or a virtual tree.
    fn asset_root_for(&self, version: &Version) -> PathBuf {
        let Some(index_ref) = &version.asset_index else {
            return self.root.assets_dir();
        };
        let text = std::fs::read_to_string(self.root.asset_index_file(&index_ref.id))
            .expect("the install wrote no asset index");
        let index = AssetIndex::parse(&text).unwrap();
        match index.destination() {
            AssetDestination::ObjectStore => self.root.assets_dir(),
            AssetDestination::Resources => self.game_dir().join("resources"),
            AssetDestination::Virtual => self.root.virtual_assets_dir(&index_ref.id),
        }
    }

    /// The Forge family's install, from the vendor's installer jar: the
    /// jar carries both documents (the profile names the overlay under
    /// `json`), the overlay installs like any other version, the
    /// profile's toolchain lands under the library root, and the planned
    /// processors run as headless Java with their receipts deciding who
    /// runs at all. Returns what landed and the runtime that ran it.
    fn install_forge_family(
        &self,
        maven: &str,
        group_path: &str,
        artifact: &str,
        version: &str,
    ) -> (Version, PathBuf) {
        let url = format!("{maven}/{group_path}/{version}/{artifact}-{version}-installer.jar");
        let jar = self.dir.join(format!("{artifact}-{version}-installer.jar"));
        download(
            &self.http,
            &url,
            &jar,
            None,
            None,
            &DownloadOptions::default(),
        )
        .unwrap_or_else(|e| panic!("fetching the installer jar from {url}: {e}"));

        let (profile_text, overlay_text) = installer_documents(&jar);
        let profile = InstallProfile::parse(&profile_text).unwrap();
        let installed = self.install(&overlay_text);

        // The toolchain the processors run *from*: the profile's own
        // libraries, placed before anything asks for them.
        for library in &profile.libraries {
            let Some(resolved) = library.resolve(&platform()).unwrap() else {
                continue;
            };
            let Some(file) = resolved.artifact else {
                continue;
            };
            let path = self.root.library_file(&file.rel_path).unwrap();
            if file.sha1.is_none() && path.is_file() {
                continue; // no promise to re-verify, and it is already there
            }
            let file_url = file
                .url
                .unwrap_or_else(|| panic!("toolchain library at {} names no URL", file.rel_path));
            download(
                &self.http,
                &file_url,
                &path,
                file.sha1.as_deref(),
                file.size,
                &DownloadOptions::default(),
            )
            .unwrap_or_else(|e| panic!("fetching {file_url}: {e}"));
        }

        let java = self.java_for(&installed);
        let plan = plan_processors(
            &profile,
            &ProcessorContext {
                root: &self.root_path,
                library_dir: &self.root.libraries_dir(),
                // The game jar the pipeline patches: in this layout the
                // install keeps the vanilla client bytes at its own
                // version id, byte for byte what the vendor's paths name.
                minecraft_jar: &self.root.version_jar(&installed.id),
                installer: &jar,
                side: Side::Client,
            },
        )
        .unwrap();
        assert!(!plan.is_empty(), "the install profile plans no processors");
        let report = run_processors(&java, &self.root.libraries_dir(), &plan).unwrap();
        println!(
            "processors: {} ran, {} skipped on their receipts",
            report.ran.len(),
            report.skipped.len()
        );
        (installed, java)
    }

    /// The done-when's second half: extract this platform's natives, plan
    /// the launch *from the written document* (the install contract is
    /// that a launch needs nothing else), and watch the process run.
    fn launch(&self, version: &Version, java: &Path) {
        assert!(
            java.is_file(),
            "the fetched runtime has no java at {}",
            java.display()
        );

        let written_text = std::fs::read_to_string(self.root.version_json(&version.id)).unwrap();
        let written = Version::parse(&written_text).unwrap();
        assert_eq!(written.id, version.id);

        let natives = self.dir.join("natives");
        for library in &written.libraries {
            let Some(resolved) = library.resolve(&platform()).unwrap() else {
                continue;
            };
            let Some(natives_ref) = resolved.natives else {
                continue;
            };
            let jar = self.root.library_file(&natives_ref.rel_path).unwrap();
            extract_natives(&jar, &natives, &resolved.extract_exclude).unwrap();
        }

        let context = LaunchContext {
            // A launcher signs the player in; a live test has no account,
            // but modern authlib parses the uuid before it draws anything
            // -- empty crashes the game at boot.
            player_name: "Player".to_string(),
            player_uuid: "00000000-0000-0000-0000-000000000000".to_string(),
            java: java.to_path_buf(),
            game_dir: self.game_dir(),
            assets_root: self.root.assets_dir(),
            natives_dir: natives,
            library_root: self.root.libraries_dir(),
            client_jar: self.root.version_jar(&written.id),
            game_assets: self.asset_root_for(&written),
            ..LaunchContext::default()
        };
        let plan = build_launch_plan(&written, &platform(), &context).unwrap();
        // The plan-shape invariant, checked live: this document's whole
        // argument vocabulary is the expansion table's, and no token the
        // format defines may reach the process unresolved.
        for arg in plan.jvm_args.iter().chain(&plan.game_args) {
            assert!(!arg.contains("${"), "a placeholder survived into {arg:?}");
        }
        watch(&plan);
    }
}

/// Both documents a vendor installer carries: the install profile and the
/// overlay it points at (`json` names it, written as a jar path with a
/// leading slash).
fn installer_documents(jar: &Path) -> (String, String) {
    let file = std::fs::File::open(jar).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let profile_text = zip_text(&mut archive, "install_profile.json")
        .unwrap_or_else(|| panic!("{} carries no install_profile.json", jar.display()));
    let profile = InstallProfile::parse(&profile_text).unwrap();
    let named = profile
        .json
        .as_deref()
        .unwrap_or("version.json")
        .trim_start_matches('/');
    let overlay_text = zip_text(&mut archive, named)
        .or_else(|| zip_text(&mut archive, "version.json"))
        .unwrap_or_else(|| panic!("{} carries no {named}", jar.display()));
    (profile_text, overlay_text)
}

fn zip_text(archive: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Option<String> {
    let mut entry = archive.by_name(name).ok()?;
    let mut text = String::new();
    entry.read_to_string(&mut text).ok()?;
    Some(text)
}

/// Output that means the JVM never reached the game: an argument the
/// runtime rejects, a class the classpath does not carry, or a natives
/// tree the process was never pointed at.
const JVM_REFUSES: &[&str] = &[
    "Could not find or load main class",
    "ClassNotFoundException",
    "NoClassDefFoundError",
    "UnsupportedClassVersionError",
    "Unrecognized option",
    "Could not create the Java Virtual Machine",
    "no lwjgl in java.library.path",
];

/// Spawn the plan and watch the process for up to twenty seconds. Still
/// running at the grace -- kill it, that is a game. Exited already -- its
/// output has to show *something*: a process that leaves no trace and
/// fails never ran this plan. Environmental deaths (nowhere to draw) are
/// output too, and are allowed; the JVM refusing the command is not.
fn watch(plan: &LaunchPlan) {
    let mut command = plan.command();
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .unwrap_or_else(|e| panic!("spawning the game process failed: {e}"));
    let mut stdout = child.stdout.take().expect("stdout was piped");
    let mut stderr = child.stderr.take().expect("stderr was piped");
    let out = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        bytes
    });
    let err = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes);
        bytes
    });

    let deadline = Instant::now() + Duration::from_secs(20);
    let mut killed = false;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            killed = true;
            let _ = child.kill();
            break child.wait().unwrap();
        }
        std::thread::sleep(Duration::from_millis(200));
    };

    let mut bytes = out.join().unwrap();
    bytes.extend(err.join().unwrap());
    let text = String::from_utf8_lossy(&bytes);
    println!(
        "--- {} exited with {status}, killed at grace: {killed} ---\n{text}",
        plan.main_class
    );

    for marker in JVM_REFUSES {
        assert!(
            !text.contains(marker),
            "the runtime refused the command ({marker}):\n{text}"
        );
    }
    // A main thread that *died* fails the test -- unless it died the way a
    // machine with nowhere to draw kills its games, which is the room the
    // process runs in, not the plan that started it. The markers are the
    // display layers' own words: bootstrap logging prints class *names*
    // with slashes (`org/lwjgl/glfw/GLFW`), which must not pass for a
    // windowing failure.
    if text.contains("Exception in thread \"main\"") {
        let the_room = [
            "Failed to open display",
            "org.lwjgl.LWJGLException",
            "java.awt.HeadlessException",
            "GLFW error",
            "the display",
        ];
        assert!(
            the_room.iter().any(|marker| text.contains(marker)),
            "the game's main thread died of something other than the room it runs in:\n{text}"
        );
    }
    assert!(
        killed || !text.trim().is_empty(),
        "the process exited without a trace of having run"
    );
}

#[test]
#[ignore = "downloads 1.5.2 and its Java runtime from live services (~100 MB)"]
fn live_vanilla_installs_into_a_temp_root_and_launches() {
    let sandbox = Sandbox::new("vanilla");
    let document = sandbox.version_document("1.5.2");
    let version = sandbox.install(&document);
    assert_eq!(version.id, "1.5.2");
    let java = sandbox.java_for(&version);
    sandbox.launch(&version, &java);
}

#[test]
#[ignore = "downloads 1.14.4, Fabric's profile, and a Java runtime from live services (~300 MB)"]
fn live_fabric_installs_into_a_temp_root_and_launches() {
    let sandbox = Sandbox::new("fabric");
    let sources = LoaderSources::vendor(LoaderKind::Fabric);
    let request = LoaderInstall {
        sources: &sources,
        game: "1.14.4",
        loader: None,
        game_dir: Some(&sandbox.game_dir()),
    };
    let syncer = sandbox.syncer();
    let report = install_loader(
        &syncer,
        &sandbox.cache,
        VERSION_MANIFEST_URL,
        request,
        &platform(),
    )
    .unwrap();
    let version = report.version;
    assert!(
        version.id.starts_with("fabric-loader"),
        "unexpected installed id {}",
        version.id
    );
    let java = sandbox.java_for(&version);
    sandbox.launch(&version, &java);
}

#[test]
#[ignore = "downloads 1.14.4, Quilt's profile, and a Java runtime from live services (~300 MB)"]
fn live_quilt_installs_into_a_temp_root_and_launches() {
    let sandbox = Sandbox::new("quilt");
    let sources = LoaderSources::vendor(LoaderKind::Quilt);
    let request = LoaderInstall {
        sources: &sources,
        game: "1.14.4",
        loader: None,
        game_dir: Some(&sandbox.game_dir()),
    };
    let syncer = sandbox.syncer();
    let report = install_loader(
        &syncer,
        &sandbox.cache,
        VERSION_MANIFEST_URL,
        request,
        &platform(),
    )
    .unwrap();
    let version = report.version;
    assert!(
        version.id.starts_with("quilt-loader"),
        "unexpected installed id {}",
        version.id
    );
    let java = sandbox.java_for(&version);
    sandbox.launch(&version, &java);
}

#[test]
#[ignore = "downloads 1.20.1, Forge's installer jar, and a Java runtime from live services (~650 MB)"]
fn live_forge_installs_into_a_temp_root_and_launches() {
    let sandbox = Sandbox::new("forge");
    let (version, java) = sandbox.install_forge_family(
        "https://maven.minecraftforge.net",
        "net/minecraftforge/forge",
        "forge",
        "1.20.1-47.4.26",
    );
    assert!(
        version.id.contains("-forge-"),
        "unexpected installed id {}",
        version.id
    );
    sandbox.launch(&version, &java);
}

#[test]
#[ignore = "downloads 1.20.6, NeoForge's installer jar, and a Java runtime from live services (~650 MB)"]
fn live_neoforge_installs_into_a_temp_root_and_launches() {
    let sandbox = Sandbox::new("neoforge");
    let (version, java) = sandbox.install_forge_family(
        "https://maven.neoforged.net/releases",
        "net/neoforged/neoforge",
        "neoforge",
        "20.6.141",
    );
    assert!(
        version.id.contains("neoforge"),
        "unexpected installed id {}",
        version.id
    );
    sandbox.launch(&version, &java);
}
