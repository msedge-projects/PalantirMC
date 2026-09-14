//! Live tests: the real services, not fixtures.
//!
//! Everything here is `#[ignore]`d, so `cargo test` stays offline and
//! deterministic and a laptop without a network never sees a red suite. CI runs
//! them deliberately:
//!
//! ```text
//! cargo test -p palantir-net --test live --locked -- --ignored --test-threads=1
//! ```
//!
//! They exist because the rules they cover were wrong in ways fixtures could
//! not show. A versionless component was looked up as `<uid>/.json`, which is
//! not a URL the service has, so a freshly created instance could not resolve;
//! and the version-*list* URL was the flat `<uid>.json`, which answers 404 for
//! every uid, so no loader build could be listed. Both passed their unit tests
//! the whole time, because both fixtures supplied the bytes the code expected.
//!
//! Keep these assertions about *shape and identity* — a main class, a published
//! digest, a component that has to appear — not about a version number that
//! moves. A live test that needs an edit every release teaches people to edit
//! it instead of reading it.

use palantir_core::instance::Instance;
use palantir_core::pack::PackProfile;
use palantir_core::resolve::{resolve, MetaStore};
use palantir_core::version::{ProblemSeverity, RuntimeContext};
use palantir_net::{verify_sha256, MicrosoftAuth, OnlineMetaStore, DEFAULT_META_BASE_URL};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// A modern release, present on the service and needing Java 21.
const GAME: &str = "1.21.1";
/// The loader uid the create dialog registers for Fabric.
const FABRIC_UID: &str = "net.fabricmc.fabric-loader";

/// A store pointed at the live service, caching under `dir`.
fn live_store(dir: &Path) -> OnlineMetaStore {
    OnlineMetaStore::new(DEFAULT_META_BASE_URL, dir).with_timeout(Duration::from_secs(60))
}

/// A throwaway data root with an instance created exactly as the launcher
/// creates one: `instance.cfg` plus `mmc-pack.json`, no patches.
fn fresh_instance(dir: &Path, name: &str) -> Instance {
    let instances = dir.join("instances");
    std::fs::create_dir_all(&instances).expect("creating the instances dir");
    Instance::create(&instances, name, GAME).expect("creating the instance")
}

/// Resolve `profile` against the live service.
fn resolve_against_the_live_service(
    dir: &Path,
    instance: &Instance,
) -> palantir_core::resolve::Resolution {
    let mut store = live_store(&dir.join("meta"));
    let profile = PackProfile::load(&instance.mmc_pack_path()).expect("reading mmc-pack.json");
    resolve(
        &profile,
        &instance.patches_dir(),
        &mut store,
        &RuntimeContext::current_host(),
    )
    .expect("resolving the profile")
}

/// The Fabric build the service says supports `game`, newest first.
///
/// Fabric's version list is game-agnostic — no entry pins Minecraft — so the
/// game a build supports is only in the version *file*, which is the same thing
/// the launcher has to read before it can offer a build with confidence.
fn fabric_build_for(store: &mut OnlineMetaStore, game: &str) -> String {
    let list = store
        .version_list(FABRIC_UID)
        .expect("fabric version list is required for this test");
    assert!(!list.is_empty(), "the service listed no Fabric builds at all");
    for entry in list.iter().rev().take(40).filter(|e| !e.version.is_empty()) {
        let file = match store.version_file(FABRIC_UID, &entry.version) {
            Ok(file) => file,
            Err(_) => continue,
        };
        if file
            .requires
            .iter()
            .any(|r| r.uid == "net.minecraft" && r.equals_version == game)
        {
            return entry.version.clone();
        }
    }
    panic!("no Fabric build on the service claims to support {game}");
}

/// Instance creation and instance loading, end to end, against the service the
/// launcher actually uses.
///
/// This is the test the version-fill rule exists for: `Instance::create` writes
/// an `org.lwjgl3` slot with no version, and resolution has to work out that it
/// means the version `net.minecraft` requires. Before that rule, this instance
/// could not resolve and so could not launch.
#[test]
#[ignore = "live: reaches the metadata service"]
fn a_created_instance_resolves_against_the_live_service() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let instance = fresh_instance(tmp.path(), "Live Vanilla");

    let profile = PackProfile::load(&instance.mmc_pack_path()).expect("reading mmc-pack.json");
    assert!(
        profile
            .components()
            .iter()
            .any(|c| c.uid == "org.lwjgl3" && c.version.is_empty()),
        "the instance no longer carries the versionless LWJGL slot this test is about"
    );

    let resolution = resolve_against_the_live_service(tmp.path(), &instance);
    assert_eq!(
        resolution.severity(),
        ProblemSeverity::None,
        "resolution reported: {:?}",
        resolution.problems
    );
    assert_eq!(resolution.profile.minecraft_version, GAME);
    assert_eq!(resolution.profile.main_class, "net.minecraft.client.main.Main");

    let assets = resolution
        .profile
        .minecraft_assets
        .as_ref()
        .expect("no asset index resolved");
    assert!(!assets.id.is_empty(), "the asset index has no id");
    assert_eq!(assets.sha1.len(), 40, "asset index sha1: {:?}", assets.sha1);
    assert!(
        assets.url.ends_with(&format!("{}.json", assets.id)),
        "asset index url does not point at the index: {}",
        assets.url
    );

    // The slot nobody versioned was resolved from the requirement naming it.
    let lwjgl = resolution
        .components
        .iter()
        .find(|c| c.uid == "org.lwjgl3")
        .expect("org.lwjgl3 vanished from the resolution");
    assert!(
        lwjgl.version.starts_with("3."),
        "LWJGL3 resolved to {:?}",
        lwjgl.version
    );
    assert!(
        resolution
            .profile
            .libraries
            .iter()
            .any(|lib| lib.name.group().contains("lwjgl")),
        "no LWJGL library reached the classpath"
    );
    // LWJGL 3 ships its natives as separate rule-gated artifacts rather than as
    // `natives` classifiers, so they are ordinary libraries here -- and the ones
    // that survived `is_active` are the ones this host can run.
    assert!(
        resolution
            .profile
            .libraries
            .iter()
            .any(|lib| lib.name.artifact().contains("-natives-")),
        "no LWJGL natives for this host reached the classpath"
    );
    assert!(
        resolution.profile.compatible_java_majors.contains(&21),
        "Java majors resolved: {:?}",
        resolution.profile.compatible_java_majors
    );
}

/// A Fabric instance: the loader is a component, and everything it needs has to
/// arrive with it.
///
/// The loader jar and its ASM stack live in the metadata's version file for the
/// loader, and the mappings live in a component the loader merely *requires*.
/// Both were missing while the create dialog wrote a synthesized patch instead:
/// the patch replaced the metadata's file, so the libraries never existed, and
/// nothing added the component that was only required.
#[test]
#[ignore = "live: reaches the metadata service"]
fn a_fabric_instance_gets_its_loader_and_mappings_from_the_live_service() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let instance = fresh_instance(tmp.path(), "Live Fabric");
    let mut store = live_store(&tmp.path().join("meta"));
    let build = fabric_build_for(&mut store, GAME);

    // What the create dialog does for a chosen loader.
    let path = instance.mmc_pack_path();
    let mut profile = PackProfile::load(&path).expect("reading mmc-pack.json");
    profile.set_version(FABRIC_UID, &build, true);
    profile.save(&path).expect("writing mmc-pack.json");

    let resolution = resolve_against_the_live_service(tmp.path(), &instance);
    assert_eq!(
        resolution.severity(),
        ProblemSeverity::None,
        "resolution reported: {:?}",
        resolution.problems
    );
    assert_eq!(
        resolution.profile.main_class,
        "net.fabricmc.loader.impl.launch.knot.KnotClient",
        "Fabric {build} did not supply the loader's entry point"
    );

    // The loader and the library stack it loads with.
    let artifacts: Vec<String> = resolution
        .profile
        .libraries
        .iter()
        .map(|lib| lib.name.artifact().to_string())
        .collect();
    for needed in ["fabric-loader", "sponge-mixin"] {
        assert!(
            artifacts.iter().any(|a| a == needed),
            "Fabric {build} is installed without {needed}; libraries: {artifacts:?}"
        );
    }

    // The mappings: required by the loader, listed by nobody, at the game
    // version.
    let mappings = resolution
        .components
        .iter()
        .find(|c| c.uid == "net.fabricmc.intermediary")
        .expect("the mappings the loader requires were never resolved");
    assert_eq!(mappings.version, GAME);
}

/// A version list is what a build list is built from, and the layout it is read
/// from is the layout the service serves.
#[test]
#[ignore = "live: reaches the metadata service"]
fn version_lists_are_read_from_the_layout_the_service_serves() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let mut store = live_store(&tmp.path().join("meta"));

    for uid in ["net.minecraft", FABRIC_UID, "org.lwjgl3"] {
        let list = store.version_list(uid).unwrap_or_else(|e| panic!("{uid} list: {e}"));
        assert!(!list.is_empty(), "{uid} listed no versions");
        assert!(
            list.iter().any(|e| !e.version.is_empty()),
            "{uid} returned entries without versions"
        );
    }

    // Gaming versions carry the game the user picks from, and every Minecraft
    // release publishes the digest of its own version file.
    let minecraft = store.version_list("net.minecraft").expect("minecraft list");
    assert!(
        minecraft.iter().any(|e| e.version == GAME),
        "the release list no longer contains {GAME}"
    );
    assert!(
        minecraft.iter().any(|e| e.sha256.len() == 64),
        "the release list stopped publishing sha256, which the cache check below relies on"
    );
}

/// The download path, end to end: fetch a version file through the store,
/// confirm it landed in the cache at the path the offline store reads, and
/// confirm the bytes are the bytes the service published.
#[test]
#[ignore = "live: reaches the metadata service"]
fn a_fetched_version_file_is_cached_and_matches_its_published_digest() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let meta: PathBuf = tmp.path().join("meta");
    let mut store = live_store(&meta);

    let list = store.version_list("net.minecraft").expect("minecraft list");
    let entry = list
        .iter()
        .find(|e| e.version == GAME)
        .expect("the release list no longer contains the test's game version");
    assert_eq!(entry.sha256.len(), 64, "no sha256 published for {GAME}");

    let file = store
        .version_file("net.minecraft", GAME)
        .expect("fetching the version file");
    assert_eq!(file.version, GAME);
    assert_eq!(file.main_class, "net.minecraft.client.main.Main");

    // Written in the layout the offline store (and Prism) reads.
    let cached = meta.join("net.minecraft").join(format!("{GAME}.json"));
    assert!(cached.is_file(), "{} was never written", cached.display());
    verify_sha256(&cached, &entry.sha256).expect("the cached bytes are not the published file");

    // And the second read is a cache hit, not a fetch: an offline store sees
    // the same file through the same layout.
    let mut offline =
        palantir_core::resolve::OfflineMetaStore::new(meta.clone());
    let again = offline
        .version_file("net.minecraft", GAME)
        .expect("the offline store could not read what the online store wrote");
    assert_eq!(again.version, GAME);
}

/// Microsoft's front door, with the client id the launcher ships.
///
/// This cannot finish a login — that needs a human at a browser — but it is the
/// step that fails when a client id, a scope or an endpoint is wrong, and those
/// are the three ways sign-in breaks silently. A code in hand proves
/// Microsoft accepted all three.
#[test]
#[ignore = "live: reaches Microsoft's device-code endpoint"]
fn microsoft_issues_a_device_code_for_the_shipped_client_id() {
    let auth = MicrosoftAuth::with_public_client_id();
    let code = auth
        .request_device_code()
        .expect("Microsoft refused the device-code request");

    assert!(!code.device_code.is_empty(), "no device code was issued");
    assert!(
        code.user_code.len() >= 6 && code.user_code.len() <= 12,
        "user code looks wrong: {:?}",
        code.user_code
    );
    assert!(
        code.user_code
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-'),
        "user code has unexpected characters: {:?}",
        code.user_code
    );
    assert!(
        code.verification_uri.starts_with("https://"),
        "verification uri: {:?}",
        code.verification_uri
    );
    assert!(
        code.expires_in > 0,
        "the code expires immediately: {}",
        code.expires_in
    );
    assert!(
        code.interval >= 5,
        "Microsoft asked for {}s between polls; the launcher polls as often as it is told",
        code.interval
    );
}
