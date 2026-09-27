//! The four mod loaders' own metadata: which builds exist for a game version.
//!
//! Why not Prism's repository, which this launcher reads today: it is the *other*
//! launcher's service, and it is a mirror of these four in the first place --
//! Prism fetches each loader's publication, rewrites it into its own shape and
//! serves that. `piston.rs` makes the same argument about Mojang's metadata, and
//! the answers are the same: a mirror is a day behind when a loader releases, and
//! every field it does not model is a field this launcher would have to guess.
//!
//! So four sources, and each is the one that loader publishes:
//!
//! | Loader | Where | What it is |
//! | --- | --- | --- |
//! | Fabric | `meta.fabricmc.net/v2/versions/loader/{game}` | every build usable on that game, newest first, each with its own `stable` flag |
//! | Quilt | `meta.quiltmc.org/v3/versions/loader/{game}` | the same shape, with no flag: a build is a release or it is a pre-release, and its version says which |
//! | NeoForge | `maven.neoforged.net/api/maven/versions/releases/...` | *every* NeoForge version ever published, so the game's own line has to be picked out of it |
//! | Forge | `files.minecraftforge.net/.../promotions_slim.json` | Forge's `latest` and `recommended` per game -- two builds, and for Forge those are the two that mean anything |
//!
//! ## What a caller gets
//!
//! [`Build`]s, newest first, each carrying whether its own source called it
//! stable, and -- for the two loaders that publish one -- the *launch profile* of
//! a build, which is the document a resolver merges: its libraries, its main
//! class and its mappings jar. Nothing here downloads or installs a loader, and
//! Forge and NeoForge have no profile to ask for: their launch profile is not a
//! document a service serves but the `version.json` inside an installer jar, and
//! a launcher is expected to *run* that installer's processors. Serving that is
//! install work, and it is named as such in `NEXT_STEPS.md` rather than faked
//! with a URL here.
//!
//! Believed for [`crate::engine::cache::DEFAULT_TTL`], like a version list: a
//! loader release is not a search result, and a list that is an hour old is still
//! a list of every build there is.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use palantir_core::version::VersionFile;

use crate::engine::cache::MetadataCache;
use crate::engine::cancel::Cancel;
use crate::engine::request::Fetch;
use crate::engine::retry::Backoff;
use crate::Error;

/// How many builds a caller is offered for one loader and game version.
///
/// Fabric has published over two hundred builds for a single game version, and a
/// picker that drew all of them would be a picker nobody scrolls. Sixty is the
/// same bound the shell's own loader dropdown already uses, and the newest builds
/// are the ones a user is looking for -- an old build is reachable by pinning the
/// version rather than by scrolling to it.
pub const MAX_BUILDS: usize = 60;

/// A mod loader this launcher can create an instance with.
///
/// The desktop's own `LoaderKind` is the interface's list -- it also carries
/// Vanilla, which has no metadata to ask for -- and turns into one of these when
/// there is a question for a service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Loader {
    /// Fabric, published at `meta.fabricmc.net`.
    Fabric,
    /// Quilt, published at `meta.quiltmc.org`.
    Quilt,
    /// NeoForge, published on its own maven.
    NeoForge,
    /// Minecraft Forge, published on its own maven and promoted by name.
    Forge,
}

impl Loader {
    /// Every loader with a build list to ask for.
    pub fn all() -> [Loader; 4] {
        [Loader::Fabric, Loader::NeoForge, Loader::Forge, Loader::Quilt]
    }

    /// The name the loader is published under, which is also the name Modrinth
    /// files tag themselves with: `fabric`, `neoforge`, `forge`, `quilt`.
    pub fn name(self) -> &'static str {
        match self {
            Loader::Fabric => "fabric",
            Loader::Quilt => "quilt",
            Loader::NeoForge => "neoforge",
            Loader::Forge => "forge",
        }
    }

    /// The loader [`Loader::name`] names, if it names one.
    ///
    /// A caller that has a loader's *name* -- the interface's own list is keyed by
    /// one, because it also carries Vanilla, which has no service to ask -- gets
    /// the answer `None` for anything that is not a loader, which is how vanilla
    /// falls out without a special case at the call site.
    pub fn from_name(name: &str) -> Option<Loader> {
        Loader::all().into_iter().find(|loader| loader.name().eq_ignore_ascii_case(name))
    }

    /// Where this loader's build list for `game` is.
    ///
    /// `game` is unused for Forge, and that is not a bug: Forge promotes its
    /// builds for every game version in one document, so the game is applied when
    /// the document is read rather than when it is asked for. It is still part of
    /// the signature, because a caller has no business knowing which loader
    /// happens to publish one URL for all games.
    pub fn list_url(self, game: &str) -> String {
        match self {
            Loader::Fabric => format!("https://meta.fabricmc.net/v2/versions/loader/{game}"),
            Loader::Quilt => format!("https://meta.quiltmc.org/v3/versions/loader/{game}"),
            Loader::NeoForge => NEOFORGE_VERSIONS_URL.to_string(),
            Loader::Forge => FORGE_PROMOTIONS_URL.to_string(),
        }
    }

    /// Where the launch profile of one build is, on the service that published
    /// the list it was picked from.
    ///
    /// `None` for Forge and NeoForge, and not because they publish nothing: a
    /// launch profile for those two is not a document a service serves. Their
    /// installer jar carries a `version.json`, and a launcher that reads it is
    /// expected to *run* the installer -- its processors patch the client jar and
    /// unzip the maven artifacts the profile names -- which is why this launcher
    /// resolves them through Prism's rewritten copy today. `None` is that fact
    /// stated once, rather than a URL that would answer 404.
    pub fn profile_url(self, game: &str, build: &str) -> Option<String> {
        match self {
            Loader::Fabric => Some(format!(
                "https://meta.fabricmc.net/v2/versions/loader/{game}/{build}/profile/json"
            )),
            Loader::Quilt => Some(format!(
                "https://meta.quiltmc.org/v3/versions/loader/{game}/{build}/profile/json"
            )),
            Loader::NeoForge | Loader::Forge => None,
        }
    }
}

/// NeoForge's whole publication, as its maven API describes it.
pub const NEOFORGE_VERSIONS_URL: &str =
    "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";

/// Forge's promoted builds, `latest` and `recommended` per game version.
pub const FORGE_PROMOTIONS_URL: &str =
    "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";

/// One build of one loader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Build {
    /// The build's own version string: `0.19.5`, `21.4.5`, `54.1.0`.
    pub version: String,
    /// Whether the loader's own source calls this build stable.
    ///
    /// Fabric says so outright; Quilt and NeoForge say it in the version string
    /// (a pre-release is one with a suffix); Forge's `recommended` is stable and
    /// its `latest` is merely newest. What a caller does with the difference is
    /// choose a default: [`default_build`] prefers this flag.
    pub stable: bool,
}

/// The build a create flow should open on: the newest stable one, or the newest
/// of all when the loader has published nothing stable for this game version.
pub fn default_build(builds: &[Build]) -> Option<&Build> {
    builds
        .iter()
        .find(|build| build.stable)
        .or_else(|| builds.first())
}

/// The loaders' own metadata, over the engine's cache.
///
/// One cache directory for all four and for both kinds of document -- the build
/// list and the launch profile -- because the entries are told apart by URL and a
/// loader's publication ages like any other version list.
pub struct LoaderMeta {
    /// Where a body lives once it has been fetched, and how long it is believed.
    documents: MetadataCache,
    /// The way bytes arrive. The engine's own pool in production, a scripted
    /// server in a test.
    fetch: Arc<dyn Fetch>,
}

/// The directory and nothing else: the fetch seam has no `Debug` of its own, and
/// a handle that printed it would be printing a connection pool.
impl std::fmt::Debug for LoaderMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoaderMeta").field("dir", &self.documents.dir()).finish_non_exhaustive()
    }
}

impl LoaderMeta {
    /// A handle over `cache`, fetching through `fetch`.
    pub fn new(cache: MetadataCache, fetch: Arc<dyn Fetch>) -> LoaderMeta {
        LoaderMeta { documents: cache, fetch }
    }

    /// The directory bodies are cached in.
    pub fn cache_dir(&self) -> &Path {
        self.documents.dir()
    }

    /// Every build of `loader` usable on `game`, newest first.
    ///
    /// Blocking, like every engine call. An empty list is a real answer -- a game
    /// version this loader never published for -- and is not an error; a failure
    /// here means the service or the body was wrong, and the two are told apart by
    /// the caller because an empty list and a failed request draw differently.
    pub fn builds(
        &self,
        loader: Loader,
        game: &str,
        cancel: &Cancel,
        backoff: &Backoff,
    ) -> Result<Vec<Build>, Error> {
        let url = loader.list_url(game);
        let cached = self.documents.get(&url, self.fetch.as_ref(), cancel, backoff)?;
        let text = String::from_utf8_lossy(&cached.body).into_owned();
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|error| Error::json(url.clone(), error.to_string()))?;
        let mut builds =
            parse(loader, game, &value).map_err(|detail| Error::json(url.clone(), detail))?;
        sort_newest_first(&mut builds);
        builds.truncate(MAX_BUILDS);
        Ok(builds)
    }

    /// The launch profile of one build of `loader`, as the loader's own service
    /// serves it.
    ///
    /// This is the document a resolver merges: the loader's libraries -- the ASM
    /// stack it loads, its jars, and the mappings jar *for this game version*,
    /// which Fabric and Quilt both list among them -- its main class, and the
    /// loader's own JVM arguments. It is Mojang's shape (`id`, `inheritsFrom`,
    /// `libraries`, `mainClass`, `arguments`) with no `order`, which is Prism's
    /// addition to the same file, so it parses the way piston's version files do:
    /// a missing `order` is the shape of the source rather than a warning.
    ///
    /// `game` is part of the question rather than a detail of the URL: both
    /// services publish one profile per game version, and the mappings inside
    /// the one that comes back are built for that game.
    ///
    /// Blocking, like every engine call. A loader with no profile to ask for
    /// ([`Loader::profile_url`]) is an error naming that, rather than an empty
    /// file the resolver would merge and then blame on the instance.
    pub fn profile(
        &self,
        loader: Loader,
        game: &str,
        build: &str,
        cancel: &Cancel,
        backoff: &Backoff,
    ) -> Result<VersionFile, palantir_core::error::Error> {
        let Some(url) = loader.profile_url(game, build) else {
            return Err(palantir_core::error::Error::format(
                PathBuf::from(loader.name()),
                format!(
                    "{} publishes no launch profile to read: its own is inside an \
                     installer this launcher does not run yet",
                    loader.name()
                ),
            ));
        };
        let held = self
            .documents
            .get(&url, self.fetch.as_ref(), cancel, backoff)
            .map_err(Error::into_core)?;
        let text = String::from_utf8_lossy(&held.body).into_owned();
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|error| palantir_core::error::Error::json(&url, error.to_string()))?;
        VersionFile::parse(&value, &PathBuf::from(&url), false)
    }
}

/// One loader's list, read out of the document its own service published.
fn parse(loader: Loader, game: &str, value: &serde_json::Value) -> Result<Vec<Build>, String> {
    match loader {
        Loader::Fabric => parse_with_flags(value),
        Loader::Quilt => parse_with_flags(value),
        Loader::NeoForge => parse_neoforge(value, game),
        Loader::Forge => parse_forge(value, game),
    }
}

/// Fabric's and Quilt's shared shape: an array of `{ "loader": { ... } }`.
///
/// Fabric states stability (`"stable": true`); Quilt does not, and its entries
/// carry only a version. Rather than two parsers the flag is read when it is
/// there and derived from the version when it is not, which is exactly what the
/// two services mean by it: `0.20.0-beta.9` is a beta and `0.19.0` is not.
fn parse_with_flags(value: &serde_json::Value) -> Result<Vec<Build>, String> {
    let entries = value
        .as_array()
        .ok_or_else(|| "a build list is an array".to_string())?;
    let mut builds = Vec::with_capacity(entries.len());
    for entry in entries {
        let Some(version) = entry
            .get("loader")
            .and_then(|loader| loader.get("version"))
            .and_then(serde_json::Value::as_str)
        else {
            // One malformed entry is not a reason to lose the list: a service
            // that grows a second shape for one build should not take the picker
            // down with it.
            continue;
        };
        let stable = entry
            .get("loader")
            .and_then(|loader| loader.get("stable"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or_else(|| !is_prerelease(version));
        builds.push(Build { version: version.to_string(), stable });
    }
    Ok(builds)
}

/// NeoForge's maven API: every version it ever published, in one array.
///
/// NeoForge numbers its builds after the game version they are for -- `21.4.5`
/// is for `1.21.4`, `21.1.10` for `1.21.1`, `21.0.167` for `1.21.0` (which is
/// what people call `1.21`) -- so the game's line is a prefix, built here rather
/// than guessed: `1.21.4` is `21.4.`, `1.21` is `21.0.`. A game version with no
/// line of its own -- a snapshot, or a release before NeoForge existed -- gets the
/// empty list it deserves.
fn parse_neoforge(value: &serde_json::Value, game: &str) -> Result<Vec<Build>, String> {
    let Some(versions) = value.get("versions").and_then(serde_json::Value::as_array) else {
        return Err("NeoForge's publication is an object with a `versions` array".to_string());
    };
    let Some(prefix) = neoforge_line(game) else {
        return Ok(Vec::new());
    };
    let mut builds = Vec::new();
    for version in versions.iter().filter_map(serde_json::Value::as_str) {
        if !version.starts_with(&prefix) {
            continue;
        }
        builds.push(Build { version: version.to_string(), stable: !is_prerelease(version) });
    }
    Ok(builds)
}

/// The prefix NeoForge's builds for `game` carry, or `None` for a game version
/// its scheme cannot describe.
///
/// Only releases can be described: NeoForge's snapshot builds are numbered after
/// the snapshot (`0.25w14craftmine.3-beta`), and a rule invented for those here
/// would be a rule that is wrong the next time Mojang names a snapshot.
fn neoforge_line(game: &str) -> Option<String> {
    let rest = game.strip_prefix("1.")?;
    let mut parts = rest.split('.');
    let minor: u32 = parts.next()?.parse().ok()?;
    let patch: u32 = match parts.next() {
        Some(patch) => patch.parse().ok()?,
        None => 0,
    };
    if parts.next().is_some() {
        return None;
    }
    Some(format!("{minor}.{patch}."))
}

/// Forge's promotions: `{ "<game>-recommended": "<build>", "<game>-latest": "<build>" }`.
///
/// Two builds and no list, because that is what Forge publishes: its maven holds
/// every build ever made, but the two it *stands behind* for a game version are
/// these. `recommended` is the build Forge suggests, which is why it is the stable
/// one; `latest` is the newest, which is not the same claim.
fn parse_forge(value: &serde_json::Value, game: &str) -> Result<Vec<Build>, String> {
    let Some(promos) = value.get("promos").and_then(serde_json::Value::as_object) else {
        return Err("Forge's promotions are an object with a `promos` map".to_string());
    };
    let mut builds: Vec<Build> = Vec::new();
    for (suffix, stable) in [("recommended", true), ("latest", false)] {
        let key = format!("{game}-{suffix}");
        let Some(version) = promos.get(&key).and_then(serde_json::Value::as_str) else {
            continue;
        };
        // A game version whose recommended and latest builds are the same build
        // -- the common case for an older release -- is one build, and it is the
        // one Forge recommends.
        match builds.iter_mut().find(|build| build.version == version) {
            Some(existing) => existing.stable |= stable,
            None => builds.push(Build { version: version.to_string(), stable }),
        }
    }
    Ok(builds)
}

/// Whether a version string names something a loader would not call a release.
///
/// Quilt and NeoForge both say it this way (`0.20.0-beta.9`, `20.2.3-beta`), and
/// the rule is deliberately narrow: a suffix means a pre-release, and nothing else
/// does, so `0.19.5` and `21.4.5` are releases.
fn is_prerelease(version: &str) -> bool {
    version.contains('-')
}

/// Newest first, by the version's own numbers.
///
/// The services do not agree on an order -- Fabric and Quilt publish newest
/// first, NeoForge's maven API publishes oldest first -- so the order is decided
/// here rather than trusted, and a build list is the same list whichever service
/// it came from. A pre-release sorts before the release it precedes.
pub fn sort_newest_first(builds: &mut [Build]) {
    builds.sort_by(|left, right| compare(&right.version, &left.version));
}

/// Compare two loader version strings: numbers first, then a pre-release below
/// the release it belongs to.
fn compare(left: &str, right: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    let (left_release, left_suffix) = split_prerelease(left);
    let (right_release, right_suffix) = split_prerelease(right);
    let mut left_parts = left_release.split('.');
    let mut right_parts = right_release.split('.');
    loop {
        match (left_parts.next(), right_parts.next()) {
            (None, None) => break,
            (left_part, right_part) => {
                let left_number = left_part.and_then(|part| part.parse::<u64>().ok()).unwrap_or(0);
                let right_number =
                    right_part.and_then(|part| part.parse::<u64>().ok()).unwrap_or(0);
                match left_number.cmp(&right_number) {
                    Ordering::Equal => {}
                    other => return other,
                }
            }
        }
    }
    // Same numbers: the one without a pre-release suffix is the newer, and two
    // pre-releases are compared as strings -- they are dates and words, not a
    // scheme this launcher should have an opinion about.
    match (left_suffix, right_suffix) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(left), Some(right)) => left.cmp(right),
    }
}

/// Split `0.19.5` into its numbers and `0.20.0-beta.9` into numbers plus suffix.
fn split_prerelease(version: &str) -> (&str, Option<&str>) {
    match version.split_once('-') {
        Some((release, suffix)) => (release, Some(suffix)),
        None => (version, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::request::{MapFetch, Route};
    use std::time::Duration;

    /// A trimmed Fabric body: the real shape, three builds, newest first.
    const FABRIC_BODY: &str = r#"[
      { "loader": { "separator": ".", "build": 5, "maven": "net.fabricmc:fabric-loader:0.19.5",
                    "version": "0.19.5", "stable": true },
        "intermediary": { "maven": "net.fabricmc:intermediary:1.21.4", "version": "1.21.4",
                          "stable": true },
        "launcherMeta": { "version": 2, "min_java_version": 8 } },
      { "loader": { "separator": ".", "build": 4, "maven": "net.fabricmc:fabric-loader:0.19.4",
                    "version": "0.19.4", "stable": false },
        "intermediary": { "maven": "net.fabricmc:intermediary:1.21.4", "version": "1.21.4",
                          "stable": true } },
      { "loader": { "separator": ".", "build": 2, "maven": "net.fabricmc:fabric-loader:0.19.2",
                    "version": "0.19.2", "stable": true },
        "intermediary": { "maven": "net.fabricmc:intermediary:1.21.4", "version": "1.21.4",
                          "stable": true } }
    ]"#;

    /// A trimmed Quilt body: the same shape, and no `stable` flag anywhere, which
    /// is why the version string has to carry that meaning.
    const QUILT_BODY: &str = r#"[
      { "loader": { "maven": "org.quiltmc:quilt-loader:0.20.0-beta.9",
                    "version": "0.20.0-beta.9", "build": 9, "separator": "." },
        "intermediary": { "maven": "org.quiltmc:hashed:1.21.4", "version": "1.21.4" } },
      { "loader": { "maven": "org.quiltmc:quilt-loader:0.19.1",
                    "version": "0.19.1", "build": 1, "separator": "." },
        "intermediary": { "maven": "org.quiltmc:hashed:1.21.4", "version": "1.21.4" } }
    ]"#;

    /// A trimmed NeoForge publication: three game lines and the oldest-first order
    /// the maven API really uses.
    const NEOFORGE_BODY: &str = r#"{"isSnapshot": false, "versions": [
      "20.2.3-beta", "20.2.88", "21.0.167", "21.1.10", "21.1.72", "21.4.4", "21.4.5",
      "21.4.100-beta"
    ]}"#;

    /// A trimmed Forge promotions document: both keys for one game, only `latest`
    /// for another, and the same build under both keys for a third.
    const FORGE_BODY: &str = r#"{
      "homepage": "https://files.minecraftforge.net/net/minecraftforge/forge/",
      "promos": {
        "1.21.4-latest": "54.1.0",
        "1.21.4-recommended": "54.0.0",
        "1.20.1-latest": "47.4.0",
        "1.16.5-latest": "36.2.42",
        "1.16.5-recommended": "36.2.42"
      }
    }"#;

    /// A trimmed Fabric launch profile: the real shape, with the mappings jar
    /// among its libraries and no `order` anywhere.
    const FABRIC_PROFILE: &str = r#"{
      "id": "fabric-loader-0.19.5-1.21.4",
      "inheritsFrom": "1.21.4",
      "releaseTime": "2026-09-01T00:00:00+00:00",
      "time": "2026-09-01T00:00:00+00:00",
      "type": "release",
      "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
      "arguments": { "game": [], "jvm": ["-DFabricMcEmu= net.minecraft.client.main.Main "] },
      "libraries": [
        { "name": "org.ow2.asm:asm:9.10.1", "url": "https://maven.fabricmc.net/" },
        { "name": "net.fabricmc:intermediary:1.21.4", "url": "https://maven.fabricmc.net/" },
        { "name": "net.fabricmc:fabric-loader:0.19.5", "url": "https://maven.fabricmc.net/" }
      ]
    }"#;

    /// A handle over a scratch directory whose only routes are the ones a test
    /// scripts, so a test that forgets one fails instead of dialling out.
    fn meta(name: &str, ttl: Duration) -> (LoaderMeta, Arc<MapFetch>) {
        let root = std::env::temp_dir().join("palantirmc-engine-loaders").join(name);
        let _ = std::fs::remove_dir_all(&root);
        let fetch = Arc::new(MapFetch::new());
        let meta = LoaderMeta::new(MetadataCache::new(root, ttl), fetch.clone());
        (meta, fetch)
    }

    fn run(meta: &LoaderMeta, loader: Loader, game: &str) -> Result<Vec<Build>, Error> {
        meta.builds(loader, game, &Cancel::new(), &Backoff::with_attempts(1))
    }

    fn versions(builds: &[Build]) -> Vec<&str> {
        builds.iter().map(|build| build.version.as_str()).collect()
    }

    #[test]
    fn a_fabric_profile_comes_from_fabric_with_the_mappings_inside_it() {
        let (meta, fetch) = meta("fabric-profile", crate::engine::cache::DEFAULT_TTL);
        let url = Loader::Fabric
            .profile_url("1.21.4", "0.19.5")
            .expect("Fabric publishes one");
        fetch.set_route(&url, Route::text(FABRIC_PROFILE));

        let profile = meta
            .profile(Loader::Fabric, "1.21.4", "0.19.5", &Cancel::new(), &Backoff::with_attempts(1))
            .expect("the profile");
        assert_eq!(profile.main_class, "net.fabricmc.loader.impl.launch.knot.KnotClient");
        assert!(
            !profile.has_order,
            "the loader's own file carries no `order`: that key is Prism's addition to the same document"
        );
        assert!(profile.requires.is_empty(), "and no requirement to reach the mappings by");
        let libraries: Vec<String> = profile
            .libraries
            .iter()
            .map(|library| format!("{}:{}", library.name.artifact(), library.name.version()))
            .collect();
        assert_eq!(
            libraries,
            vec!["asm:9.10.1", "intermediary:1.21.4", "fabric-loader:0.19.5"],
            "the mappings jar is the loader's own library, not a second component to resolve"
        );

        // Asked again, the document is already held: the cache is what makes a
        // second lookup free rather than a second request.
        meta.profile(Loader::Fabric, "1.21.4", "0.19.5", &Cancel::new(), &Backoff::with_attempts(1))
            .expect("the profile again");
        assert_eq!(fetch.count(), 1, "one document, one request");
    }

    #[test]
    fn a_loader_that_publishes_no_profile_says_so_instead_of_asking() {
        // Forge's own launch profile is inside an installer a launcher is meant
        // to run, so there is no URL to be wrong about: the answer names that.
        let (meta, fetch) = meta("forge-profile", crate::engine::cache::DEFAULT_TTL);
        assert!(Loader::Forge.profile_url("1.21.4", "54.1.0").is_none());
        let error = meta
            .profile(Loader::Forge, "1.21.4", "54.1.0", &Cancel::new(), &Backoff::with_attempts(1))
            .expect_err("no profile to read");
        assert!(error.to_string().contains("installer"), "{error}");
        assert_eq!(fetch.count(), 0, "and nothing was asked for");
    }

    #[test]
    fn fabric_lists_its_builds_newest_first_with_the_flag_its_service_sends() {
        let (meta, fetch) = meta("fabric", crate::engine::cache::DEFAULT_TTL);
        let url = Loader::Fabric.list_url("1.21.4");
        fetch.set_route(&url, Route::text(FABRIC_BODY));

        let builds = run(&meta, Loader::Fabric, "1.21.4").expect("the list");
        assert_eq!(versions(&builds), vec!["0.19.5", "0.19.4", "0.19.2"]);
        assert_eq!(
            builds.iter().map(|build| build.stable).collect::<Vec<_>>(),
            vec![true, false, true],
            "0.19.4 is Fabric's own \"not stable\", flag rather than precedence"
        );
        // A build list is cached under its own URL, so the picker asking twice
        // costs one request.
        assert!(meta.documents.cached(&url).is_some(), "the body was kept, with an age");
        let _ = run(&meta, Loader::Fabric, "1.21.4").expect("the list again");
        assert_eq!(fetch.count(), 1, "the second ask was answered from disk");
    }

    #[test]
    fn quilt_reads_the_same_shape_and_calls_a_suffix_a_pre_release() {
        let (meta, fetch) = meta("quilt", crate::engine::cache::DEFAULT_TTL);
        fetch.set_route(&Loader::Quilt.list_url("1.21.4"), Route::text(QUILT_BODY));

        let builds = run(&meta, Loader::Quilt, "1.21.4").expect("the list");
        // 0.20.0-beta.9 is newer than 0.19.1 and is not what a create flow should
        // open on, which is the whole reason the two fields are separate.
        assert_eq!(versions(&builds), vec!["0.20.0-beta.9", "0.19.1"]);
        assert_eq!(
            builds.iter().map(|build| build.stable).collect::<Vec<_>>(),
            vec![false, true]
        );
        assert_eq!(
            default_build(&builds).map(|build| build.version.as_str()),
            Some("0.19.1")
        );
    }

    #[test]
    fn neoforge_picks_the_game_s_own_line_out_of_every_build_it_ever_published() {
        let (meta, fetch) = meta("neoforge", crate::engine::cache::DEFAULT_TTL);
        let url = Loader::NeoForge.list_url("1.21.4");
        assert_eq!(url, NEOFORGE_VERSIONS_URL, "one publication covers every game");
        fetch.set_route(&url, Route::text(NEOFORGE_BODY));

        let builds = run(&meta, Loader::NeoForge, "1.21.4").expect("the list");
        // Only 21.4's, newest first -- NeoForge's own array is oldest first and
        // says nothing about stability. `21.4.100-beta` leads because its build
        // number is newer than 21.4.5's: a pre-release sorts below the release of
        // its *own* number, not below every release.
        assert_eq!(versions(&builds), vec!["21.4.100-beta", "21.4.5", "21.4.4"]);
        assert_eq!(
            builds.iter().map(|build| build.stable).collect::<Vec<_>>(),
            vec![false, true, true]
        );

        // The game's line for a release with no patch component is its `0` line,
        // and a release NeoForge never published for is an empty list rather than
        // an error.
        assert_eq!(versions(&run(&meta, Loader::NeoForge, "1.21").expect("1.21")), vec!["21.0.167"]);
        let empty = run(&meta, Loader::NeoForge, "1.20.1").expect("1.20.1");
        assert!(empty.is_empty(), "NeoForge's first releases were for 1.20.2");
        let snapshot = run(&meta, Loader::NeoForge, "25w02a").expect("a snapshot");
        assert!(snapshot.is_empty());
    }

    #[test]
    fn forge_offers_the_two_builds_it_promotes_and_calls_the_recommended_one_stable() {
        let (meta, fetch) = meta("forge", crate::engine::cache::DEFAULT_TTL);
        let url = Loader::Forge.list_url("1.21.4");
        assert_eq!(url, FORGE_PROMOTIONS_URL, "Forge promotes every game in one document");
        fetch.set_route(&url, Route::text(FORGE_BODY));

        let builds = run(&meta, Loader::Forge, "1.21.4").expect("the list");
        assert_eq!(versions(&builds), vec!["54.1.0", "54.0.0"]);
        assert_eq!(
            builds.iter().map(|build| build.stable).collect::<Vec<_>>(),
            vec![false, true],
            "Forge's `recommended` is the stable one, not its newest"
        );
        assert_eq!(default_build(&builds).map(|build| build.version.as_str()), Some("54.0.0"));

        // A game version with only a `latest` build gets one build, and one whose
        // two keys name the same build gets one build rather than two rows.
        assert_eq!(versions(&run(&meta, Loader::Forge, "1.20.1").expect("1.20.1")), vec!["47.4.0"]);
        let same = run(&meta, Loader::Forge, "1.16.5").expect("1.16.5");
        assert_eq!(versions(&same), vec!["36.2.42"]);
        assert!(same[0].stable, "the one row carries both keys' meaning");
    }

    #[test]
    fn a_build_list_is_at_most_the_picker_s_bound_long() {
        // Fabric really does publish over two hundred builds for a game version,
        // and a dropdown that drew them all is a dropdown nobody scrolls.
        let many: Vec<String> = (0..(MAX_BUILDS + 40))
            .map(|build| format!("{{\"loader\": {{\"version\": \"0.1.{build}\", \"stable\": true}}}}"))
            .collect();
        let body = format!("[{}]", many.join(","));
        let (meta, fetch) = meta("bound", crate::engine::cache::DEFAULT_TTL);
        fetch.set_route(&Loader::Fabric.list_url("1.21.4"), Route::text(&body));

        let builds = run(&meta, Loader::Fabric, "1.21.4").expect("the list");
        assert_eq!(builds.len(), MAX_BUILDS);
        assert_eq!(builds[0].version, "0.1.99", "the newest builds are the ones kept");
    }

    #[test]
    fn a_body_that_is_not_a_build_list_names_the_url_it_came_from() {
        let (meta, fetch) = meta("broken", crate::engine::cache::DEFAULT_TTL);
        let url = Loader::Quilt.list_url("1.21.4");
        fetch.set_route(&url, Route::text("{\"not\": \"a list\"}"));

        let failure = run(&meta, Loader::Quilt, "1.21.4").expect_err("a shape that is not a list");
        let message = failure.to_string();
        assert!(message.contains(&url), "{message}");
        assert!(message.contains("a build list is an array"), "{message}");
    }

    #[test]
    fn a_loader_is_found_by_the_name_it_is_published_under_and_vanilla_is_not_one() {
        // What the interface's own list needs: it carries Vanilla, which has no
        // service and no build list, so "not a loader" has to be an answer rather
        // than a panic or a special case at every call site.
        assert_eq!(Loader::from_name("fabric"), Some(Loader::Fabric));
        assert_eq!(Loader::from_name("NeoForge"), Some(Loader::NeoForge));
        assert_eq!(Loader::from_name("vanilla"), None);
        assert_eq!(Loader::from_name(""), None);
        assert_eq!(Loader::from_name("prism"), None);
    }

    #[test]
    fn versions_compare_by_their_numbers_before_their_suffixes() {
        // The rule the sort rests on, stated on its own: a longer version is not
        // automatically newer, and a pre-release belongs below its release.
        assert_eq!(compare("0.19.5", "0.19.4"), std::cmp::Ordering::Greater);
        assert_eq!(compare("21.4.5", "21.4.100"), std::cmp::Ordering::Less);
        assert_eq!(compare("0.20.0", "0.20.0-beta.9"), std::cmp::Ordering::Greater);
        assert_eq!(compare("1.0", "1.0.0"), std::cmp::Ordering::Equal);

        let mut builds = vec![
            Build { version: "0.19.4".to_string(), stable: true },
            Build { version: "0.20.0-beta.1".to_string(), stable: false },
            Build { version: "0.19.10".to_string(), stable: true },
        ];
        sort_newest_first(&mut builds);
        assert_eq!(versions(&builds), vec!["0.20.0-beta.1", "0.19.10", "0.19.4"]);
    }
}
