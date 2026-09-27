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
use palantir_net::{
    verify_sha256, Fetcher, MicrosoftAuth, OnlineMetaStore, DEFAULT_META_BASE_URL,
};
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

/// The newest Fabric build, and the newest Minecraft release the service has
/// Fabric mappings for.
///
/// Fabric's metadata never names a Minecraft version — the loader requires the
/// mappings by uid alone, and *which* game it belongs to is whichever
/// `net.fabricmc.intermediary` exists, because the resolver locks that uid to the
/// game version. So the coherent pair to test with is the newest build plus a
/// release the service has mappings for, and asking for it here keeps the test
/// off the Minecraft release calendar.
///
/// Both lists arrive **newest first**; `catalog.rs` sorts explicitly rather than
/// trusting that, which is why a wrong assumption here showed up as a claim that
/// no Fabric build supports anything.
fn newest_fabric_pair(store: &mut OnlineMetaStore) -> (String, String) {
    let builds = store
        .version_list(FABRIC_UID)
        .expect("fabric version list is required for this test");
    let build = builds
        .iter()
        .find(|e| !e.version.is_empty())
        .expect("the service listed no Fabric builds at all")
        .version
        .clone();

    let releases = store
        .version_list("net.minecraft")
        .expect("the release list is required for this test");
    for entry in releases.iter().filter(|e| e.type_ == "release").take(12) {
        if store
            .version_file("net.fabricmc.intermediary", &entry.version)
            .is_ok()
        {
            return (build, entry.version.clone());
        }
    }
    panic!("none of the twelve newest Minecraft releases has Fabric mappings on the service");
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
    let mut store = live_store(&tmp.path().join("meta"));
    let (build, game) = newest_fabric_pair(&mut store);

    let instances = tmp.path().join("instances");
    std::fs::create_dir_all(&instances).expect("creating the instances dir");
    let instance =
        Instance::create(&instances, "Live Fabric", &game).expect("creating the instance");

    // What the create dialog does for a chosen loader.
    let path = instance.mmc_pack_path();
    let mut profile = PackProfile::load(&path).expect("reading mmc-pack.json");
    profile.set_version(FABRIC_UID, &build, true);
    profile.save(&path).expect("writing mmc-pack.json");

    let resolution = resolve_against_the_live_service(tmp.path(), &instance);
    assert_eq!(
        resolution.severity(),
        ProblemSeverity::None,
        "Fabric {build} on Minecraft {game} reported: {:?}",
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
    assert_eq!(mappings.version, game);
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

/// The Java the service publishes, walked through the same parser the launcher
/// runs when a machine has no Java at an accepted major.
///
/// A version file says which Java it needs, in two fields of its own
/// (`compatibleJavaMajors`, `compatibleJavaName`); the launcher then fetches the
/// runtime the service names, which is three metadata files deep: the
/// `net.minecraft.java` list, the major's version file, and Mojang's per-file
/// manifest. Every one of those is a shape that a fixture would happily get
/// wrong in the same way the code did — the whole point of asking the service.
///
/// The last assertion is the one that matters most. Each runtime entry publishes
/// a digest, and the launcher refuses to install a manifest whose bytes do not
/// match it. If that digest described something *other* than the manifest — a
/// tarball, an inner file, the version file — then every install would be
/// refused as tampered with, and no offline test could tell, because it would
/// have been written with the same belief.
#[test]
#[ignore = "live: reaches the metadata service"]
fn the_java_runtime_the_service_publishes_can_be_walked_to_a_java_binary() {
    use palantir_net::java::{
        host_runtime_os, parse_manifest, parse_runtimes, pick_runtime, runtime_file_url,
        runtime_list_url, runtime_version, JAVA_RUNTIMES_UID,
    };
    use sha1::{Digest, Sha1};

    let tmp = tempfile::tempdir().expect("temp dir");
    let mut store = live_store(&tmp.path().join("meta"));

    // The list the major is looked up in, and the one URL builder that has to
    // agree with the store's own layout.
    let majors = store
        .version_list(JAVA_RUNTIMES_UID)
        .unwrap_or_else(|e| panic!("{JAVA_RUNTIMES_UID} list: {e}"));
    assert_eq!(
        runtime_list_url(DEFAULT_META_BASE_URL),
        store.version_list_url(JAVA_RUNTIMES_UID),
        "the runtime list is built at a different URL than the rest of the metadata"
    );
    // 1.12.2 wants Java 8 and 1.21.1 wants Java 21; a service that lists neither
    // could not answer a single instance this launcher creates.
    for wanted in ["java8", "java21"] {
        assert!(
            majors.iter().any(|entry| entry.version == wanted),
            "{JAVA_RUNTIMES_UID} no longer lists {wanted}: {:?}",
            majors.iter().map(|e| e.version.as_str()).collect::<Vec<_>>()
        );
    }

    // What the game itself asks for.
    let game = store
        .version_file("net.minecraft", GAME)
        .expect("fetching the game version file");
    assert!(
        !game.compatible_java_majors.is_empty(),
        "{GAME} stopped declaring a compatible Java major, which is what a \
         launcher needs to pick one"
    );
    assert!(
        !game.compatible_java_name.is_empty(),
        "{GAME} stopped naming a Java runtime, so nothing can be fetched when \
         the machine has no matching Java"
    );
    for major in &game.compatible_java_majors {
        assert!(
            majors.iter().any(|entry| entry.version == runtime_version(*major)),
            "{GAME} wants Java {major}, which {JAVA_RUNTIMES_UID} does not publish"
        );
    }

    // The major's version file, for the newest major the game accepts.
    let major = *game.compatible_java_majors.iter().max().expect("a compatible major");
    let url = runtime_file_url(DEFAULT_META_BASE_URL, &runtime_version(major));
    let fetcher = palantir_net::BlockingHttpFetcher::new(Duration::from_secs(60));
    let bytes = fetcher.fetch(&url).unwrap_or_else(|e| panic!("{url}: {e}"));
    let entries = parse_runtimes(&bytes, Path::new(&url)).expect("parsing the runtime file");
    assert!(!entries.is_empty(), "{url} listed no runtimes at all");
    assert!(
        entries
            .iter()
            .all(|entry| !entry.os.is_empty() && !entry.name.is_empty()),
        "a runtime entry came back without a platform or a name"
    );
    assert!(
        entries.iter().any(|entry| entry.major == major),
        "{url} names {major} but carries no entry for it"
    );

    // The entry this host would install, named by the game itself.
    let host = host_runtime_os(&RuntimeContext::current_host());
    let entry = pick_runtime(&entries, &host, &game.compatible_java_name).unwrap_or_else(|| {
        panic!(
            "{GAME} names '{}' and wants Java {major}, but {} has no such runtime",
            game.compatible_java_name,
            url
        )
    });
    assert!(
        entry.is_manifest(),
        "the {host} runtime the game names is a '{}' download now, which the \
         launcher does not unpack",
        entry.kind
    );
    assert_eq!(entry.sha1.len(), 40, "the runtime entry stopped publishing a sha1");
    assert!(entry.url.starts_with("https://"), "runtime url: {}", entry.url);

    // And the manifest behind it, which is what the installer reads.
    let manifest_bytes = fetcher
        .fetch(&entry.url)
        .unwrap_or_else(|e| panic!("{}: {e}", entry.url));
    let digest = Sha1::digest(&manifest_bytes);
    let digest = digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    assert_eq!(
        digest,
        entry.sha1.to_ascii_lowercase(),
        "the runtime entry's published digest is not the digest of the manifest it \
         points at, so no runtime would ever be installed"
    );

    let files = parse_manifest(&manifest_bytes, Path::new(&entry.url))
        .expect("parsing the JRE manifest");
    assert!(
        files.len() > 50,
        "a JRE manifest with {} file(s) is not a runtime",
        files.len()
    );
    let exe = if cfg!(windows) { "bin/java.exe" } else { "bin/java" };
    let binary = files
        .iter()
        .find(|file| file.path == exe)
        .unwrap_or_else(|| panic!("the manifest has no {exe}"));
    assert!(binary.executable, "{exe} is not marked executable");
    assert_eq!(binary.sha1.len(), 40, "{exe} has no published digest");
    assert!(binary.size > 0, "{exe} has no published size");
    assert!(
        binary.url.starts_with("https://"),
        "{exe} is served from {}",
        binary.url
    );
}

/// Where Mojang's resource CDN serves asset objects. The same string the
/// desktop crate calls `ASSET_OBJECT_BASE_URL`, spelled out here because the
/// point of this test is to not share an assumption with the code it checks.
const ASSET_OBJECT_BASE_URL: &str = "https://resources.download.minecraft.net";

/// One real asset object, fetched from the URL the launcher builds for it.
///
/// The bug this exists for: the URL was derived from the *storage* path
/// (`objects/<xx>/<hash>`, the layout under the data root's `assets/` folder,
/// which is also what Prism writes), and the CDN has no such path — it serves
/// `/<xx>/<hash>`. So a first install made one request per object, got 5057 of
/// 5057 back as 404, moved zero bytes and refused to launch. Every unit fixture
/// agreed with the code, because every fixture built its expected URL through
/// the same helper the code built the real one.
///
/// What is asserted is identity as well as status: the bytes served at the
/// object's URL must hash to the object's own name. A 200 from an error page, a
/// redirect to a bucket listing, or an object served under the wrong path would
/// all pass a length check and fail this one.
#[test]
#[ignore = "live: reaches Mojang's asset CDN"]
fn an_asset_object_is_served_at_the_cdn_layout_the_launcher_builds() {
    use palantir_core::assets::{object_cdn_path, object_relative_path, AssetIndex};
    use sha1::{Digest, Sha1};

    let tmp = tempfile::tempdir().expect("temp dir");
    let mut store = live_store(&tmp.path().join("meta"));
    let game = store
        .version_file("net.minecraft", GAME)
        .expect("fetching the game version file");
    let index = game
        .asset_index
        .filter(|index| index.known)
        .unwrap_or_else(|| panic!("{GAME} no longer publishes a downloadable asset index"));
    assert!(
        index.url.starts_with("https://"),
        "the asset index is served from {}",
        index.url
    );

    let fetcher = palantir_net::BlockingHttpFetcher::new(Duration::from_secs(120));
    let index_bytes = fetcher
        .fetch(&index.url)
        .unwrap_or_else(|e| panic!("{}: {e}", index.url));
    let parsed = AssetIndex::parse(&String::from_utf8_lossy(&index_bytes))
        .expect("parsing the live asset index");
    assert!(
        parsed.objects.len() > 1000,
        "an index of {} object(s) is not the game's assets",
        parsed.objects.len()
    );

    // Any object will do, but a real one: the index is what a first install
    // downloads, so the first entry it names is what a first install fetches.
    let (name, object) = parsed.objects.iter().next().expect("the index is empty");
    let url = format!("{ASSET_OBJECT_BASE_URL}/{}", object_cdn_path(&object.hash));
    let bytes = fetcher.fetch(&url).unwrap_or_else(|e| {
        panic!(
            "'{name}' is not served at {url}: {e} — this is the URL the launcher \
             builds for every asset object"
        )
    });
    let digest = Sha1::digest(&bytes);
    let digest = digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    assert_eq!(
        digest, object.hash,
        "{url} served bytes that are not the object the index names"
    );
    if object.size > 0 {
        assert_eq!(
            bytes.len() as i64,
            object.size,
            "{url} served the right digest at the wrong length"
        );
    }

    // And the layout this used to build: the storage path is not a URL the CDN
    // serves, which is the whole reason the two helpers have different names.
    let storage = format!("{ASSET_OBJECT_BASE_URL}/{}", object_relative_path(&object.hash));
    assert_ne!(storage, url);
    assert!(
        fetcher.fetch(&storage).is_err(),
        "{storage} is served by the CDN after all, so the storage layout is a \
         working URL and this assertion — the record of the 404 that mattered — \
         can go"
    );
}

/// Modrinth's API through the engine, which is the call the Discover page will
/// make.
///
/// Two things are measured, and neither is available any other way.
///
/// The first is the `facets` parameter. Modrinth takes it as *JSON* in a query
/// string -- `[["project_type:mod"]]`, percent-encoded -- and a client that
/// encodes it wrong gets either a 400 or a 200 full of the wrong kind of project.
/// The unit tests hold the encoder against an expected URL, which is exactly the
/// kind of agreement a fixture and its code are capable of getting wrong
/// together, so the live run asks the service: a typed search has to come back
/// with hits, and the project named by the first one has to be a *page* -- its
/// own document, the team that owns it, and versions a launcher could install.
///
/// The second is the cache. A second identical search must cost *no* request, and
/// that is asserted by counting what the pool actually sent -- a search that was
/// cached and one that was asked for again look the same from the far side of the
/// `Arc`.
#[test]
#[ignore = "live: reaches api.modrinth.com"]
fn the_live_modrinth_api_answers_a_typed_search_and_a_version_list() {
    use palantir_net::engine::{Backoff, Cancel, HttpPool, MetadataCache, ModrinthApi, Search};

    let tmp = tempfile::tempdir().expect("temp dir");
    // A counting wrapper would be nicer than counting through the pool, but the
    // pool's own `requests` are not visible from here, so the assertion the
    // second time is the *bytes*: the same response object, from the cache.
    let pool = std::sync::Arc::new(HttpPool::default());
    let api = ModrinthApi::new(MetadataCache::new(tmp.path().join("modrinth"), palantir_net::DEFAULT_TTL), pool);
    let cancel = Cancel::new();
    let backoff = Backoff::with_attempts(2);

    // A typed search, sorted the way the reference's Discover page sorts by
    // default. A wrong `facets` encoding is a 400 here.
    let search = Search::new("sodium").of_type("mod").sorted_by("downloads");
    let response = api
        .search(&search, &cancel, &backoff)
        .unwrap_or_else(|e| panic!("{}: {e}", search.url()));
    assert!(
        !response.hits.is_empty(),
        "a search for 'sodium' among mods returned nothing"
    );
    assert!(response.total_hits > 0, "total_hits is {}", response.total_hits);
    let first = &response.hits[0];
    assert!(!first.title.is_empty() && !first.project_ref().is_empty(), "{first:?}");

    // The cache: the same question, answered without a request. (The answer is
    // compared field for field, so a response that came back different would be
    // a failure here rather than a quietly different page.)
    let again = api
        .search(&search, &cancel, &backoff)
        .expect("the same search, from the cache");
    assert_eq!(again, response, "a cached search is the same answer");

    // And the project the search named is a document of its own: the title, the
    // body a page draws, and the counts. A card's fields come off the search, but
    // opening it is this request, and a page cannot be built from a search hit.
    let project = api
        .project(first.project_ref(), &cancel, &backoff)
        .unwrap_or_else(|e| panic!("{}: {e}", first.project_ref()));
    assert!(!project.id.is_empty() && !project.title.is_empty(), "{project:?}");
    assert!(
        !project.body.trim().is_empty(),
        "{} opens with no description at all",
        project.title
    );
    assert_eq!(project.title, first.title, "the search and the project disagree about the title");

    // The team is a second request, and it is where a page's byline comes from:
    // Modrinth's project document names a team id and no person. What the team
    // list *is* was measured here rather than assumed -- Sodium's three members
    // come back with the roles `Maintainer`, `Project Lead`, `Maintainer`, and
    // every one of them carries `ordering: 0`, so neither the order nor an
    // `Owner` role names the owner and the store credits the `Project Lead`
    // instead (`store::author_of`). What this asserts is the part a service
    // change would break: members come back, with names and with roles.
    let members = api
        .members(first.project_ref(), &cancel, &backoff)
        .unwrap_or_else(|e| panic!("{}'s team: {e}", first.project_ref()));
    assert!(!members.is_empty(), "{} has no team at all", project.title);
    assert!(
        members.iter().all(|member| !member.user.username.is_empty()),
        "a member with no name on {}: {members:?}",
        project.title
    );
    assert!(
        members.iter().any(|member| !member.role.is_empty()),
        "nobody on {}'s team has a role: {members:?}",
        project.title
    );

    // And it has versions, with files and digests.
    let versions = api
        .versions(first.project_ref(), &cancel, &backoff)
        .unwrap_or_else(|e| panic!("{}: {e}", first.project_ref()));
    assert!(!versions.is_empty(), "{} has no versions", first.project_ref());
    let version = &versions[0];
    assert!(!version.version_number.is_empty(), "{version:?}");
    let file = version
        .primary_file()
        .unwrap_or_else(|| panic!("{} has a version with no file", first.title));
    assert!(file.url.starts_with("https://"), "{}", file.url);
    assert!(
        file.sha1().is_some() || file.sha512().is_some(),
        "{} publishes no digest to verify a download with",
        file.filename
    );
}

/// Modrinth's news feed: the one document the panel draws that is not the API.
///
/// It is asserted rather than assumed for the reason every live test in this file
/// exists: the shape was read off the live service (`/news/feed/articles.json`,
/// 45 articles, each with `title`, `summary`, `thumbnail`, `date` and `link`), and
/// a fixture written from a guess would have agreed with a parser that read the
/// wrong keys. What this catches is the service changing the envelope under the
/// panel -- `articles` renamed, a date that is no longer ISO-8601, a link that is
/// no longer absolute.
#[test]
#[ignore = "live: reaches modrinth.com"]
fn the_live_news_feed_parses_into_articles_the_panel_can_draw() {
    use palantir_net::engine::{Backoff, Cancel, HttpPool, MetadataCache, ModrinthApi};
    use palantir_net::modrinth::NEWS_URL;

    let tmp = tempfile::tempdir().expect("temp dir");
    let pool = std::sync::Arc::new(HttpPool::default());
    let api = ModrinthApi::new(
        MetadataCache::new(tmp.path().join("modrinth"), palantir_net::DEFAULT_TTL),
        pool,
    );
    let cancel = Cancel::new();
    let backoff = Backoff::with_attempts(2);

    let news = api
        .news(&cancel, &backoff)
        .unwrap_or_else(|e| panic!("{NEWS_URL}: {e}"));
    assert!(!news.is_empty(), "the feed has no articles at all");
    // The first four are what the panel draws, so those are the four this is
    // about: every field the card uses has to be there for the live feed.
    for article in news.iter().take(4) {
        assert!(!article.title.is_empty(), "an article with no title: {article:?}");
        assert!(
            article.link.starts_with("https://"),
            "an article whose link is not absolute: {}",
            article.link
        );
        assert!(
            article.date.len() >= 10 && article.date.as_bytes()[4] == b'-',
            "a date that is not ISO-8601: {}",
            article.date
        );
        // And the format the card draws it in is a month name and a year, which
        // is the one thing the parser does to the date.
        let label = article.date_label();
        assert!(
            label.contains(", 20"),
            "{}'s date drew as '{label}'",
            article.title
        );
    }

    // Newest first is the order the panel's four come out in, and it is the
    // service's order rather than this launcher's: a feed that reversed would put
    // the oldest announcement at the top of the sidebar.
    let dates: Vec<&str> = news.iter().take(4).map(|article| article.date.as_str()).collect();
    let mut sorted = dates.clone();
    sorted.sort_unstable_by(|a, b| b.cmp(a));
    assert_eq!(dates, sorted, "the feed is not newest first");
}

/// Mojang's own metadata, which is the source this launcher has never used.
///
/// The shell this rewrite replaces reads `meta.prismlauncher.org`, a mirror:
/// Prism fetches piston, rewrites it into its own shape and serves that. A
/// mirror is the wrong answer here for a reason a fixture cannot show -- every
/// field it drops is a field the launcher has to guess at -- and for a reason
/// this test is written to catch: the shape this launcher parses has to be the
/// shape Mojang actually publishes, not the shape a fixture and the code agreed
/// on between themselves.
///
/// What is asserted is the chain a launch depends on, end to end: the manifest
/// names a latest release, the release's `sha1` in that manifest is the digest
/// of the version file that arrives, and that file parses into the main class,
/// the library list and the asset index a launch is built from. Three real
/// requests, and the third one is the document that decides what a classpath is.
#[test]
#[ignore = "live: reaches piston-meta.mojang.com"]
fn the_live_piston_manifest_names_a_release_whose_version_file_parses() {
    use palantir_net::engine::{Backoff, Cancel, HttpPool, MetadataCache, PistonMeta, DEFAULT_TTL};

    let tmp = tempfile::tempdir().expect("temp dir");
    let pool = std::sync::Arc::new(HttpPool::default());
    let meta = PistonMeta::new(MetadataCache::new(tmp.path().join("piston"), DEFAULT_TTL), pool);
    let cancel = Cancel::new();
    let backoff = Backoff::with_attempts(2);

    let manifest = meta.manifest(&cancel, &backoff).expect("Mojang's version manifest");
    assert!(
        manifest.versions.len() > 500,
        "a manifest of {} version(s) is not Minecraft's",
        manifest.versions.len()
    );
    assert!(
        manifest.releases().count() > 100,
        "only {} release(s) in the list",
        manifest.releases().count()
    );
    let release = manifest
        .newest_release()
        .expect("the manifest names a latest release that is not in its own list");
    assert_eq!(release.id, manifest.latest_release);
    assert!(
        release.sha1.is_some(),
        "the manifest publishes no digest for {}, so the version file cannot be checked",
        release.id
    );

    assert!(
        manifest.find(&manifest.latest_release).is_some(),
        "the manifest names '{}' as latest and does not list it",
        manifest.latest_release
    );
    let id = manifest.latest_release.clone();
    let (id, file) = meta
        .latest_release(&cancel, &backoff)
        .unwrap_or_else(|e| panic!("{id}: {e}"));
    assert_eq!(id, manifest.latest_release);
    assert_eq!(
        file.main_class, "net.minecraft.client.main.Main",
        "{id} launches through {}",
        file.main_class
    );
    assert!(
        file.libraries.len() > 20,
        "{id} lists {} librar(ies), which is not a Minecraft version",
        file.libraries.len()
    );
    assert!(
        !file.has_order,
        "{id} carries an 'order' key, which is Prism's addition rather than Mojang's"
    );
    assert_eq!(file.type_, "release");
    let index = file
        .asset_index
        .as_ref()
        .filter(|index| index.known)
        .unwrap_or_else(|| panic!("{id} publishes no downloadable asset index"));
    assert!(index.url.starts_with("https://"), "the asset index is at {}", index.url);
    assert!(
        !index.sha1.is_empty(),
        "{id}'s asset index comes with no digest to check it against"
    );
}

/// The translation, against the file it replaces.
///
/// `from_mojang` exists because this launcher's model reads Prism's *rewrite* of
/// Mojang's version file rather than Mojang's own shape, so a unit test can only
/// say that the translation agrees with the fixture its author wrote -- and that
/// fixture was written from the same reading of the mirror as the code. This asks
/// both services for the same version and compares their answers field by field,
/// which is what turns "the translation does what the mirror did" into a
/// measurement, and which is the only way to catch a field the mirror moves and
/// the code does not.
///
/// What cannot agree is named rather than papered over: the mirror drops the
/// `assets` key (its `assetIndex` carries the id), adds an `XR:Initial` trait for
/// a feature this launcher does not offer, and serves LWJGL as a component of its
/// own rather than among the game's libraries. The argument strings are compared
/// for equality, `--clientId ${clientid} --xuid ${auth_xuid}` included, because
/// the translation drops that pair for exactly the reason the mirror does: no
/// launch of this launcher fills those two tokens.
#[test]
#[ignore = "live: reaches piston-meta.mojang.com and meta.prismlauncher.org"]
fn the_translation_agrees_with_the_mirror_the_shell_read() {
    use palantir_net::engine::{Backoff, Cancel, HttpPool, MetadataCache, PistonMeta, DEFAULT_TTL};

    let tmp = tempfile::tempdir().expect("temp dir");
    let pool = std::sync::Arc::new(HttpPool::default());
    let piston = PistonMeta::new(
        MetadataCache::new(tmp.path().join("piston"), DEFAULT_TTL),
        pool,
    );
    let cancel = Cancel::new();
    let backoff = Backoff::with_attempts(2);

    let ours = piston
        .translated(GAME, "net.minecraft", &cancel, &backoff)
        .unwrap_or_else(|e| panic!("{GAME} from piston: {e}"));
    let theirs = live_store(&tmp.path().join("mirror"))
        .version_file("net.minecraft", GAME)
        .unwrap_or_else(|e| panic!("{GAME} from the mirror: {e}"));

    // The version file's own identity: piston names no component, the caller's
    // uid is what a resolver files it under.
    assert_eq!(ours.uid, "net.minecraft");
    assert_eq!(ours.version, GAME);
    assert_eq!(ours.main_class, theirs.main_class);
    assert!(ours.minecraft_arguments.starts_with("--username"));
    assert_eq!(
        ours.minecraft_arguments, theirs.minecraft_arguments,
        "the argument strings differ, so one of the two is not passing the \
         arguments this launcher fills and no others"
    );
    assert_eq!(ours.compatible_java_majors, theirs.compatible_java_majors);
    assert_eq!(ours.compatible_java_name, theirs.compatible_java_name);

    // The asset index, field for field -- the one document a launch cannot get
    // wrong without the game refusing to start.
    let ours_index = ours.asset_index.as_ref().expect("an asset index");
    let theirs_index = theirs.asset_index.as_ref().expect("an asset index");
    assert_eq!(ours_index.id, theirs_index.id);
    assert_eq!(ours_index.url, theirs_index.url);
    assert_eq!(ours_index.sha1, theirs_index.sha1);
    assert_eq!(ours_index.size, theirs_index.size);

    // The client jar: the mirror writes the same Maven coordinate for it that the
    // translation does, and the same digest the manifest publishes.
    let ours_jar = ours.main_jar.as_ref().expect("the client jar");
    let theirs_jar = theirs.main_jar.as_ref().expect("the client jar");
    assert_eq!(ours_jar.name.serialize(), theirs_jar.name.serialize());
    let ours_client = ours_jar
        .mojang_downloads
        .as_ref()
        .and_then(|downloads| downloads.artifact.as_ref())
        .expect("its artifact");
    let theirs_client = theirs_jar
        .mojang_downloads
        .as_ref()
        .and_then(|downloads| downloads.artifact.as_ref())
        .expect("its artifact");
    assert_eq!(ours_client.url, theirs_client.url);
    assert_eq!(ours_client.sha1, theirs_client.sha1);
    assert_eq!(ours_client.size, theirs_client.size);

    // The behaviours: the same set, minus the one this launcher has no use for.
    let expected: std::collections::BTreeSet<String> = theirs
        .traits
        .iter()
        .filter(|trait_| trait_.as_str() != "XR:Initial")
        .cloned()
        .collect();
    assert_eq!(ours.traits, expected, "the derived traits are not the mirror's");

    // The libraries. The mirror's list is a subset of Mojang's -- every name it
    // serves is in the file -- and the difference is LWJGL, which the mirror keeps
    // in a component of its own (`org.lwjgl3`, named in the `requires` this
    // launcher no longer needs) and which Mojang names among the game's own
    // libraries, natives included.
    for library in &theirs.libraries {
        let name = library.name.serialize();
        assert!(
            ours.libraries.iter().any(|l| l.name.serialize() == name),
            "the mirror serves {name}, which the translation lost"
        );
    }
    assert!(
        ours.libraries.iter().any(|l| l.name.group() == "org.lwjgl"),
        "{GAME} resolved with no LWJGL libraries at all"
    );
    assert!(
        !theirs.libraries.iter().any(|l| l.name.group() == "org.lwjgl"),
        "the mirror now serves LWJGL among the game's libraries, so the second \
         component this launcher used to resolve is a duplicate"
    );
}

/// The content store against the CDN, which is where "never download the same
/// jar twice" has to be true to be worth anything.
///
/// The unit tests prove the store's rules with a double. What they cannot say is
/// that the digest a service publishes is the digest this store computes: Mojang
/// names each asset object with a 40-character `sha1` and nothing else, so
/// `Digest::parse` has to read it as a sha1 and `verify_file` has to agree with
/// it. If they disagreed, every asset would be refused as tampered with -- and a
/// fixture would have agreed with the code, because the fixture would have been
/// written from the same assumption.
///
/// The second half is the claim that makes the store a feature rather than a
/// detail: the same object asked for twice costs one request. The real CDN is
/// what makes that worth measuring -- a double answers the same way twice no
/// matter what the store does with the file.
#[test]
#[ignore = "live: reaches Mojang's asset CDN"]
fn an_asset_object_is_fetched_once_and_then_answered_from_the_store() {
    use palantir_core::assets::{object_cdn_path, AssetIndex};
    use palantir_net::engine::{Backoff, Cancel, ContentStore, Digest, HttpPool, Stored};

    let tmp = tempfile::tempdir().expect("temp dir");
    let mut meta = live_store(&tmp.path().join("meta"));
    let game = meta
        .version_file("net.minecraft", GAME)
        .expect("fetching the game version file");
    let index_ref = game
        .asset_index
        .filter(|index| index.known)
        .unwrap_or_else(|| panic!("{GAME} no longer publishes a downloadable asset index"));
    let fetcher = palantir_net::BlockingHttpFetcher::new(Duration::from_secs(120));
    let index_bytes = fetcher
        .fetch(&index_ref.url)
        .unwrap_or_else(|e| panic!("{}: {e}", index_ref.url));
    let parsed = AssetIndex::parse(&String::from_utf8_lossy(&index_bytes))
        .expect("parsing the live asset index");
    let (name, object) = parsed.objects.iter().next().expect("the index is empty");

    // The published name, read the way the launcher reads it: by its shape.
    let digest = Digest::parse(&object.hash).unwrap_or_else(|e| {
        panic!("the asset index names '{name}' as '{}', which is not a digest: {e}", object.hash)
    });
    assert_eq!(digest.kind(), "sha1", "Mojang names asset objects with a sha1");
    let url = format!("{ASSET_OBJECT_BASE_URL}/{}", object_cdn_path(&object.hash));

    let pool = HttpPool::default();
    let cancel = Cancel::new();
    let store = ContentStore::new(tmp.path().join("content"));
    let first = store
        .fetch_blocking(&pool, &url, &digest, &cancel, &Backoff::with_attempts(2))
        .unwrap_or_else(|e| panic!("{url}: {e}"));
    match first {
        Stored::Fetched(bytes) => assert!(bytes > 0, "{url} served nothing"),
        Stored::AlreadyThere => panic!("nothing was stored yet"),
    }
    assert!(store.verified(&digest), "the bytes in the store are the object the index names");

    let second = store
        .fetch_blocking(&pool, &url, &digest, &cancel, &Backoff::with_attempts(2))
        .expect("the second look");
    assert_eq!(second, Stored::AlreadyThere, "{url} was fetched twice");
    assert!(!store.staging_path(&digest).exists(), "a staging file was left behind");
    let (files, bytes) = store.stats();
    assert_eq!(files, 1);
    assert_eq!(bytes, std::fs::metadata(store.path(&digest)).map(|m| m.len()).unwrap_or(0));
}

/// The one assumption in the engine that a double cannot hold.
///
/// `MapFetch` proves the *engine's* rules: that an offset is only continued when
/// the server says it continued, that an ignored offset writes nothing, that a
/// cancellation lands between chunks. What it cannot say is that `reqwest` and
/// the service agree with it -- that a `Range: bytes=N-` request really comes
/// back `206 Partial Content`, that `read` really ends at zero, and that the
/// bytes around the seam are the bytes of the file rather than of two files.
///
/// That last one is the reason this test exists rather than a shape check: a
/// resume that is off by a byte produces a jar that is the right length and the
/// wrong file, and only comparing the stitched result with the whole download
/// notices. The body is a metadata version list, so nothing here depends on a
/// version that moves.
#[test]
#[ignore = "live: needs meta.prismlauncher.org"]
fn a_ranged_request_really_continues_from_the_offset_it_asked_for() {
    use palantir_net::engine::{Cancel, Fetch, HttpPool, Outcome, Request};

    let url = format!("{DEFAULT_META_BASE_URL}/net.minecraft/index.json");
    let pool = HttpPool::default();
    let cancel = Cancel::new();

    let whole = pool
        .get(&Request::get(&url), &cancel)
        .unwrap_or_else(|e| panic!("{url}: {e}"));
    assert!(
        whole.len() > 1000,
        "a version list of {} bytes is not one",
        whole.len()
    );

    // Ask to continue from 100 bytes in, with those bytes already in hand.
    const SEAM: u64 = 100;
    let mut stitched = whole[..SEAM as usize].to_vec();
    let outcome = pool
        .get_to(&Request::from(&url, SEAM), &mut stitched, &cancel)
        .unwrap_or_else(|e| panic!("{url} at {SEAM}: {e}"));
    match outcome {
        Outcome::Resumed(bytes) => assert_eq!(
            bytes,
            whole.len() as u64 - SEAM,
            "the continuation was not the rest of the body"
        ),
        Outcome::Ignored => panic!(
            "{url} answered a Range request with the whole body. Resume is a \
             no-op here, and `Outcome::Ignored` is the engine correctly \
             restarting -- worth knowing, because every interrupted download \
             against this host starts again from zero"
        ),
        Outcome::Whole(bytes) => panic!(
            "{url} sent {bytes} bytes from zero for a ranged request, which the \
             offset contract does not allow"
        ),
    }
    assert_eq!(
        stitched, whole,
        "the bytes around the seam are not the bytes of the body"
    );
}

/// The metadata cache against the service it will actually be pointed at.
///
/// The unit tests hold the cache's rules against a scripted server; what they
/// cannot say is whether this host speaks the same protocol. Three things are
/// checked here that only a real answer can settle:
///
/// * a first lookup fetches, stores both halves and reports that it was not
///   free, and the entry it wrote is readable by a second cache over the same
///   directory -- which is what a launcher restart does;
/// * a lookup inside the TTL is answerable from the disk alone, which is
///   structural: `fresh` takes no `Fetch` at all;
/// * a lookup past the TTL either revalidates or re-downloads, and the answer is
///   the same document either way.
///
/// The TTL is zero -- the one value that means "always ask" -- so the second
/// request exercises the expired path every run instead of on a lucky clock.
/// Whether this host sends an `ETag` is not something the engine may assume, so
/// the test says which of the two happened rather than failing over the one this
/// service chose; what it does insist on is that a conditional request is never
/// an error and never a different document.
#[test]
#[ignore = "live: needs meta.prismlauncher.org"]
fn the_metadata_cache_revalidates_but_answers_the_same_document() {
    use palantir_net::engine::{Backoff, Cancel, HttpPool, MetadataCache, IMMUTABLE_TTL};

    let dir = std::env::temp_dir().join("palantirmc-live-metadata-cache");
    let _ = std::fs::remove_dir_all(&dir);
    let url = format!("{DEFAULT_META_BASE_URL}/net.minecraft/index.json");
    let pool = HttpPool::default();
    let cancel = Cancel::new();
    let cache = MetadataCache::new(&dir, Duration::ZERO);

    let first = cache
        .get(&url, &pool, &cancel, &Backoff::with_attempts(2))
        .unwrap_or_else(|e| panic!("{url}: {e}"));
    assert!(
        first.body.len() > 1000,
        "a version list of {} bytes is not one",
        first.body.len()
    );
    assert!(!first.from_disk, "the first lookup had nowhere to read from");
    assert_eq!(cache.len(), 1, "the entry was written to disk");
    assert!(cache.cached(&url).is_some(), "and it has an age on it");

    // A second cache over the same directory is a second launcher run: the
    // entry has to be readable by the layout alone.
    let restart = MetadataCache::new(&dir, IMMUTABLE_TTL);
    let held = restart
        .fresh(&url)
        .unwrap_or_else(|| panic!("a restart cannot read the entry it just wrote"));
    assert_eq!(held.body, first.body, "the bytes survived the round trip");
    assert!(held.from_disk);

    let second = cache
        .get(&url, &pool, &cancel, &Backoff::with_attempts(2))
        .unwrap_or_else(|e| panic!("{url} again: {e}"));
    assert_eq!(second.body, first.body, "the same document either way");
    match (&first.etag, second.from_disk) {
        (Some(etag), true) => println!("revalidated with {etag}, no body sent"),
        (Some(etag), false) => {
            println!("sent {etag} back and the host answered with the body anyway")
        }
        (None, false) => println!("this host sends no ETag, so the body came down again"),
        (None, true) => panic!("a 304 without a validator is not a protocol this host speaks"),
    }
}

#[test]
#[ignore]
fn every_loader_publishes_its_builds_where_this_code_says_it_does() {
    use palantir_net::engine::{
        default_build, Backoff, Cancel, HttpPool, Loader, LoaderMeta, MetadataCache, DEFAULT_TTL,
    };

    let dir = std::env::temp_dir().join("palantirmc-live-loaders");
    let _ = std::fs::remove_dir_all(&dir);
    let meta = LoaderMeta::new(
        MetadataCache::new(&dir, DEFAULT_TTL),
        std::sync::Arc::new(HttpPool::default()),
    );
    let cancel = Cancel::new();

    // All four, because the four are four different services and the whole point
    // of reading them directly is that each one's shape is its own: a URL that
    // moved, or a field that was renamed, is the failure fixtures cannot show.
    for loader in Loader::all() {
        let builds = meta
            .builds(loader, GAME, &cancel, &Backoff::with_attempts(2))
            .unwrap_or_else(|error| panic!("{}: {error}", loader.name()));
        assert!(
            !builds.is_empty(),
            "{} published no build for {GAME} -- the URL or the filter is wrong",
            loader.name()
        );
        assert!(
            builds.iter().all(|build| !build.version.trim().is_empty()),
            "{}: a build with no version in it",
            loader.name()
        );
        let chosen = default_build(&builds)
            .unwrap_or_else(|| panic!("{}: no build to open on", loader.name()));
        println!(
            "{:<9} {} builds for {GAME}, newest {}, a create flow would open on {}",
            loader.name(),
            builds.len(),
            builds[0].version,
            chosen.version
        );
    }
    assert!(meta.cache_dir().exists(), "the bodies were written where a second run reads them");
}

/// The two Forge-shaped loaders' own installers, against the files they replace.
///
/// `InstallerMeta::profile` exists because this launcher's model reads Prism's
/// *rewrite* of the installer's `version.json` rather than the publisher's own
/// file, so a unit test can only say that the translation agrees with the
/// fixture its author wrote -- and that fixture was written from the same
/// reading of the installer as the code. This asks the loader's maven and the
/// mirror for the same build and compares their answers, which is what turns
/// "the profile comes from the installer" into a measurement.
///
/// What cannot agree is named rather than papered over: the mirror rewrites
/// the file around ForgeWrapper (its main class, and its own artifact in place
/// of the loader's), drops `order`-less purity for an `order` and a
/// `requires` naming the game, and serves the standard argument prefix the
/// `net.minecraft` component contributes at merge time. The loader's own game
/// arguments are the tail of the mirror's string in both cases.
#[test]
#[ignore = "live: reaches maven.minecraftforge.net, maven.neoforged.net and meta.prismlauncher.org"]
fn the_installers_profile_agrees_with_the_mirror_except_for_the_wrapper() {
    use palantir_net::engine::{Backoff, Cancel, HttpPool, InstallerMeta, MetadataCache, DEFAULT_TTL};

    /// One pinned build per loader: the build the mirror also serves, so the
    /// comparison is of the same component and version on both sides.
    const BUILDS: &[(&str, &str, &str, &str, &str)] = &[
        // loader name, game, build, mirror uid, the loader's own main class.
        ("forge", "1.21.1", "52.1.0", "net.minecraftforge", "net.minecraftforge.bootstrap.ForgeBootstrap"),
        ("neoforge", "1.21.1", "21.1.172", "net.neoforged", "cpw.mods.bootstraplauncher.BootstrapLauncher"),
    ];

    let tmp = tempfile::tempdir().expect("temp dir");
    let pool = std::sync::Arc::new(HttpPool::default());
    let installers = InstallerMeta::new(
        MetadataCache::new(tmp.path().join("installers"), DEFAULT_TTL),
        pool,
    );
    let cancel = Cancel::new();
    let backoff = Backoff::with_attempts(2);

    for (name, game, build, uid, main) in BUILDS {
        let loader = palantir_net::engine::Loader::from_name(name)
            .unwrap_or_else(|| panic!("{name} is not a loader"));
        let ours = installers
            .profile(loader, game, build, &cancel, &backoff)
            .unwrap_or_else(|e| panic!("{name} {build} from its installer: {e}"));
        let theirs = live_store(&tmp.path().join("mirror"))
            .version_file(uid, build)
            .unwrap_or_else(|e| panic!("{uid} {build} from the mirror: {e}"));

        // The version file's own identity: the installer names the game build
        // in `id` and no component, so the uid and version are the caller's.
        assert_eq!(ours.uid, *uid);
        assert_eq!(ours.version, *build, "{name}: the version is the build that was asked for");
        assert_eq!(ours.main_class, *main, "{name} {build} launches through {}", ours.main_class);
        assert_eq!(
            theirs.main_class, "io.github.zekerzhayard.forgewrapper.installer.Main",
            "{uid} {build} is no longer rewritten around ForgeWrapper: {main}",
            main = theirs.main_class
        );

        // The libraries: every library the mirror serves but the wrapper is
        // in the installer's file, and everything the installer names but the
        // mirror drops is the loader's own artifact (Forge) or the logging
        // stack the wrapper replaces (NeoForge).
        for library in &theirs.libraries {
            if library.name.artifact() == "ForgeWrapper" {
                continue;
            }
            let name = library.name.serialize();
            assert!(
                ours.libraries.iter().any(|l| l.name.serialize() == name),
                "{uid} {build}: the mirror serves {name}, which the installer lost"
            );
        }
        let mut only_ours: Vec<String> =
            ours.libraries.iter().map(|l| l.name.serialize()).filter(|name| {
                !theirs.libraries.iter().any(|l| l.name.serialize() == *name)
            }).collect();
        only_ours.sort();
        let groups: Vec<String> = only_ours
            .iter()
            .map(|name| {
                palantir_core::version::GradleSpecifier::parse(name).group().to_string()
            })
            .collect();
        if *name == "forge" {
            assert_eq!(
                only_ours,
                vec![format!("net.minecraftforge:forge:{game}-{build}:client")],
                "{uid} {build}: the difference is not just the loader's own artifact: {only_ours:?}"
            );
        } else {
            assert_eq!(
                only_ours.len(),
                3,
                "{uid} {build}: the difference is not just the logging stack: {only_ours:?}"
            );
            assert!(
                groups.iter().all(|group| group == "org.apache.logging.log4j"),
                "{uid} {build}: unexpected libraries only the installer names: {only_ours:?}"
            );
        }

        // The arguments: the mirror serves the standard prefix (the game
        // component's half of the merge) with the loader's own game arguments
        // after it, so the translation is the tail of the mirror's string.
        assert!(
            !ours.minecraft_arguments.is_empty(),
            "{name} {build} translated to no game arguments at all"
        );
        let tail = if *name == "forge" {
            format!(
                "{} --fml.forgeGroup net.minecraftforge --fml.forgeVersion {build} --fml.mcVersion {game}",
                ours.minecraft_arguments
            )
        } else {
            ours.minecraft_arguments.clone()
        };
        assert!(
            theirs.minecraft_arguments.ends_with(&tail),
            "{uid} {build}:\n  mirror: {}\n  tail:   {tail}",
            theirs.minecraft_arguments
        );

        // The rewrite's own additions: an `order` and a requirement naming
        // the game, neither of which the publisher states.
        assert!(theirs.has_order, "{uid} {build} carries no order now");
        assert!(
            !ours.has_order,
            "{uid} {build} carries an 'order' key, which is the mirror's addition"
        );
        assert!(
            theirs.requires.iter().any(|r| r.uid == "net.minecraft"),
            "{uid} {build} no longer requires the game it patches"
        );
        assert!(
            ours.requires.is_empty(),
            "{uid} {build} states a requirement the publisher did not"
        );
    }
}
