//! Where a page's data comes from.
//!
//! The pages want data; the engine in `palantir-net` can fetch it; this module is
//! where the two meet. Until a page has a request behind it there is a choice of
//! two dishonesties -- pages that draw invented data, or pages that spin forever --
//! and this module is the way out of both: **the store answers what the launcher
//! can already answer, and says out loud what it cannot.**
//!
//! What it can answer is the launcher's own filesystem, which is ours rather than
//! the reference's:
//!
//! * the instance list and each instance's loader, game version, playtime and mod
//!   counts, from the same readers the old interface uses ([`crate::instances`]);
//! * an instance's own folders -- its mods, worlds, files and screenshots -- which
//!   are directory reads;
//! * the tail of an instance's newest log, which is a file read.
//!
//! What it cannot answer from disk is everything that comes from a service, and
//! that is what the [`Engine`] below is for: Discover's search goes out through
//! the engine in `palantir-net` and comes back as hits. A request the store still
//! cannot make answers with [`not_implemented`]'s sentence, which says what is
//! missing rather than pretending to be an empty list. A page that shows "no
//! results" when the request never happened is the failure mode this module exists
//! to prevent.
//!
//! ## The one thread boundary
//!
//! A page cannot make a request. The engine is *blocking* on purpose
//! (`engine::http` is the blocking `reqwest`: one pool, one ceiling, one retry
//! policy, none of which need a runtime), and a page is drawn on the frame thread,
//! so a call from a page would drop a frame for every millisecond the service
//! took. [`Store::search`] is therefore synchronous -- it returns when the answer
//! is in -- and the shell is the one caller that runs it somewhere else. Above
//! that line everything is ordinary code: a request is a value, an answer is a
//! [`Load`], and no page contains an `async`.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use palantir_core::paths::PalantirPaths;
use palantir_net::engine::{
    Backoff, Cancel, Fetch, HttpPool, MetadataCache, ModrinthApi, PistonMeta,
};
use palantir_net::engine::Search as ApiSearch;
use palantir_net::{DEFAULT_LIMIT, DEFAULT_TIMEOUT, DEFAULT_TTL};

use crate::instances::{self, ImportCandidate, InstanceCard, NewInstance};
use crate::mods::{self, ModEntry};
use crate::page::Load;
use crate::pages::discover::Hit;

/// What the interface knows, and how it came to know it.
#[derive(Debug, Clone, Default)]
pub struct Store {
    /// The launcher's instances, read from disk.
    instances: Load<Vec<InstanceCard>>,
    /// Where they live, so an instance's own folders can be read.
    instances_dir: PathBuf,
    /// The way out to a service, when this store has one.
    engine: Option<Engine>,
    /// Where the launcher's own files are, for the operations that write rather
    /// than read. `None` in a store with no launcher behind it, which is what a
    /// test over a scratch cache is -- and then creating an instance answers with
    /// the not-implemented sentence rather than writing somewhere invented.
    paths: Option<PalantirPaths>,
}

/// The network behind a store: one pool, one cache, one policy.
///
/// A store has an engine or it does not, and the two are different in a way the
/// reader can see: without one, a request answers with `not_implemented`. That is
/// why this is an `Option` rather than a store that quietly has no engine -- a
/// test, or a launcher run with no cache directory it can write, gets the honest
/// sentence instead of a request that goes nowhere.
///
/// Cheap to clone, and it has to be: the shell clones it into the thread a search
/// runs on, which is the whole of what crosses the boundary.
#[derive(Debug, Clone)]
pub struct Engine {
    /// Modrinth over the engine's cache. An `Arc` because it is handed to a
    /// worker thread per request rather than borrowed across one.
    api: Arc<ModrinthApi>,
    /// Mojang's own metadata over the *same* cache and pool as Modrinth's.
    ///
    /// One directory rather than two, because the launcher asks both services the
    /// same kind of question -- "what does this document say right now" -- and a
    /// second cache is a second set of stale answers nobody knows about. An `Arc`
    /// for the same reason the API's is: the engine is cloned into a worker.
    piston: Arc<PistonMeta>,
}

impl Engine {
    /// The engine for a launcher's own directory: one HTTP pool under the
    /// process-wide ceiling, and metadata cached in `cache/meta/`.
    ///
    /// The directory is [`PalantirPaths::meta_dir`] rather than one of this
    /// module's choosing: it is the launcher's existing cache root, it is
    /// already created by `ensure_layout`, and a second cache directory would be
    /// a second set of stale answers nobody knows about.
    pub fn new(paths: &PalantirPaths) -> Engine {
        Engine::over(paths.meta_dir(), Arc::new(HttpPool::new(DEFAULT_LIMIT, DEFAULT_TIMEOUT)))
    }

    /// The same engine over an explicit cache directory and fetch seam.
    ///
    /// The seam is `palantir_net::engine::request::Fetch`, which is what makes a
    /// search testable without a network: a test passes a `MapFetch` that answers
    /// what it scripted, and the code under test cannot tell the difference.
    pub fn over(cache_dir: impl Into<PathBuf>, fetch: Arc<dyn Fetch>) -> Engine {
        let dir = cache_dir.into();
        let cache = MetadataCache::new(dir.clone(), DEFAULT_TTL);
        Engine {
            api: Arc::new(ModrinthApi::new(cache, fetch.clone())),
            piston: Arc::new(PistonMeta::new(MetadataCache::new(dir, DEFAULT_TTL), fetch)),
        }
    }

    /// Modrinth over this engine's cache.
    pub fn api(&self) -> &ModrinthApi {
        &self.api
    }

    /// Mojang's metadata over the same cache and pool.
    pub fn piston(&self) -> &PistonMeta {
        &self.piston
    }
}

// ---- The launcher's own files, and the way out to a service ----------------

impl Store {
    /// Read the launcher's instances.
    ///
    /// A synchronous scan at startup, which is what the old interface does too
    /// (`main.rs` says so): it is a directory walk and a handful of small files per
    /// instance, and a scan that small is not worth moving off the UI thread.
    pub fn load(paths: &PalantirPaths) -> Store {
        let loaded = instances::load(paths);
        Store {
            instances: if loaded.cards.is_empty() {
                Load::Empty
            } else {
                Load::Ready(loaded.cards)
            },
            instances_dir: loaded.instances_dir,
            // A store read from disk has no way out to a service until it is
            // given one: see [`Store::with_engine`], and the module documentation
            // for why the absence is a state a page can be told about.
            engine: None,
            paths: Some(paths.clone()),
        }
    }

    /// Read the instance list again, after something changed it.
    ///
    /// A create or an import writes a folder, and the list the pages are drawn
    /// from was read at startup: without this, an instance that was just made is
    /// on disk and not on the page that made it.
    pub fn reload(&mut self) {
        let Some(paths) = &self.paths else {
            return;
        };
        let loaded = instances::load(paths);
        self.instances = if loaded.cards.is_empty() {
            Load::Empty
        } else {
            Load::Ready(loaded.cards)
        };
        self.instances_dir = loaded.instances_dir;
    }

    /// The newest release Mojang publishes, as the launcher's own metadata
    /// reader sees it.
    ///
    /// This is what a create flow opens on when it has no version chosen: the
    /// reference asks for a version, and a launcher that has to be told which
    /// Minecraft exists is a launcher that will be wrong the week a release lands.
    pub fn current_release(&self) -> Result<String, String> {
        let Some(engine) = &self.engine else {
            return Err(not_implemented("Minecraft's version list"));
        };
        let manifest = engine
            .piston()
            .manifest(&Cancel::new(), &Backoff::default())
            .map_err(|error| format!("Mojang's version list could not be read: {error}"))?;
        manifest
            .newest_release()
            .map(|version| version.id.clone())
            .ok_or_else(|| "Mojang's version list names no release".to_string())
    }

    /// Create an instance in this launcher's own format.
    ///
    /// **Blocking**, for [`Store::search`]'s reason: it writes a folder, a config
    /// and a version profile, and the shell runs it on the thread it runs requests
    /// on. `game` is the version the instance is for; `None` asks Mojang which one
    /// is current, which is what the create flow does when nothing was chosen.
    ///
    /// The answer is the new instance's id -- what the address to open it with is
    /// made of -- and the shell reloads the list it draws from.
    pub fn create_instance(&self, name: &str, game: Option<&str>) -> Result<String, String> {
        let Some(paths) = &self.paths else {
            return Err(not_implemented("Creating an instance"));
        };
        let game = match game.map(str::trim).filter(|game| !game.is_empty()) {
            Some(game) => game.to_string(),
            None => self.current_release()?,
        };
        instances::create(paths, &NewInstance::vanilla(name, &game)).map(|created| created.id)
    }

    /// What is already on this machine and could be imported.
    ///
    /// Every launcher this one knows how to read (`instances::candidate_roots`),
    /// with the ones whose id is already taken skipped.
    pub fn importable(&self) -> Vec<ImportCandidate> {
        match &self.paths {
            Some(paths) => instances::find_importable(paths),
            None => Vec::new(),
        }
    }

    /// Import one instance, by the folder it was found in.
    ///
    /// **Blocking** as well, and for the same reason: an import is a copy of a
    /// whole instance tree.
    pub fn import_instance(&self, source: &Path) -> Result<String, String> {
        let Some(paths) = &self.paths else {
            return Err(not_implemented("Importing from another launcher"));
        };
        instances::import_instance(paths, source)
    }

    /// The instance list, in whatever state it is in.
    pub fn instances(&self) -> &Load<Vec<InstanceCard>> {
        &self.instances
    }

    /// One instance, by id.
    pub fn instance(&self, id: &str) -> Load<InstanceCard> {
        match &self.instances {
            Load::Ready(cards) => match cards.iter().find(|card| card.id == id) {
                Some(card) => Load::Ready(card.clone()),
                // Not an error: an address can name an instance that is gone, and
                // the reference draws "not found" rather than "something broke".
                None => Load::Empty,
            },
            Load::Empty => Load::Empty,
            Load::Failed(reason) => Load::Failed(reason.clone()),
            // An instance page can be reached before the scan finishes.
            Load::Idle | Load::Loading => Load::Loading,
        }
    }

    /// Where an instance's own files are.
    pub fn instance_dir(&self, id: &str) -> PathBuf {
        self.instances_dir.join(id)
    }

    /// The instances directory itself, for the reader that has no id yet.
    pub fn instances_dir(&self) -> &Path {
        &self.instances_dir
    }

    /// The reason a page's data is not here yet.
    pub fn not_implemented(&self, what: &str) -> String {
        not_implemented(what)
    }

    /// The engine behind this store, if it has one.
    pub fn engine(&self) -> Option<&Engine> {
        self.engine.as_ref()
    }

    /// The same store with an engine behind it.
    pub fn with_engine(mut self, engine: Engine) -> Store {
        self.engine = Some(engine);
        self
    }

    /// Run one search, and answer when the answer is in.
    ///
    /// **Blocking.** The shell hands this to a thread of its own; see the module
    /// documentation for why that boundary is here rather than inside a page.
    ///
    /// A store with no engine answers with the same sentence a page opens with,
    /// which is the point: a reader cannot tell which of the two they are looking
    /// at, and neither of them is an empty list of results.
    pub fn search(&self, query: &ApiSearch) -> Result<Vec<Hit>, String> {
        let Some(engine) = &self.engine else {
            return Err(not_implemented("Discover's search"));
        };
        // A token per request, because the engine takes one: a search that is
        // superseded can be stopped mid-body once the shell keeps this token,
        // which is the next thing this seam wants. The retry policy is the
        // engine's own default rather than a number chosen here -- a call site
        // that decided how many attempts to make is the failure mode the engine
        // exists to prevent.
        let cancel = Cancel::new();
        let answer = engine
            .api()
            .search(query, &cancel, &Backoff::default())
            .map_err(|error| error.to_string())?;
        Ok(answer.hits.iter().map(Hit::from_api).collect())
    }
}

/// The sentence a page shows when it cannot answer from disk yet.
///
/// Deliberately plain about what is missing, and deliberately not shaped like an
/// error: a feature this launcher has not built is not a failure of anything.
///
/// It used to name a stage -- *"arrives with the metadata engine (stage 4 of the
/// rewrite)"* -- and that sentence was retired on purpose once the engine
/// actually existed. A stage number is a developer's word; it goes stale the
/// moment the stage lands, which is exactly what happened to this one; and a
/// reader told *when* something is coming instead of *what* is missing has been
/// told nothing they can act on. What is left is the shape that stays true
/// whatever lands next.
pub fn not_implemented(what: &str) -> String {
    format!("{what} is not implemented yet.")
}

/// Run a blocking request on a thread of its own, and await the answer.
///
/// This is the whole crossing between the engine, which is blocking, and iced,
/// which is not: the work happens on a thread that owns it, and what comes back is
/// a value through a channel. It is a function here rather than four lines in the
/// shell for one reason -- a test can await it without a window, so the crossing
/// itself is gated rather than assumed.
///
/// The work answers with a `Result`, and so does this, one level rather than two:
/// a *failure* is the store's own reason (a 404, a digest that did not match), and
/// a worker that dies before answering at all -- a panic in a `Drop`, a thread the
/// OS refuses -- is the same shape with a sentence of its own, because a receiver
/// whose sender went with the thread resolves rather than waiting forever.
pub async fn off_thread<T, E>(
    work: impl FnOnce() -> Result<T, E> + Send + 'static,
) -> Result<T, E>
where
    T: Send + 'static,
    E: From<String> + Send + 'static,
{
    let (sender, receiver) = futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let _ = sender.send(work());
    });
    receiver.await.unwrap_or_else(|_| {
        Err(E::from("the request stopped before it answered".to_string()))
    })
}

// ---- The instance's own folders -----------------------------------------

/// One world in `saves/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct World {
    /// Folder name, which is the world's name to Minecraft.
    pub name: String,
    /// Whether the world has been opened since it was created.
    pub played: bool,
    /// Seconds since the world folder was last written, or `None` if the clock
    /// could not be read.
    pub modified: Option<u64>,
}

/// One file in an instance's directory, for the Files tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Name as it is on disk.
    pub name: String,
    /// A directory rather than a file.
    pub directory: bool,
    /// Size in bytes, zero for a directory.
    pub bytes: u64,
}

/// The instances' mods, from the same reader the old interface uses.
pub fn content(instance_dir: &Path) -> Vec<ModEntry> {
    mods::list_mods(&instance_dir.join("mods"))
}

/// The worlds in `saves/`, name-sorted.
///
/// A world that has a `level.dat` has been opened at least once: that file is
/// written when the world is saved, and a folder with only `region/` in it is one
/// Minecraft created and never saved -- which is why `played` exists rather than
/// being guessed from the folder's contents.
pub fn worlds(instance_dir: &Path) -> Vec<World> {
    let mut worlds = Vec::new();
    let Ok(entries) = std::fs::read_dir(instance_dir.join("saves")) else {
        return worlds;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let modified = std::fs::metadata(path.join("level.dat"))
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|since| since.as_secs());
        worlds.push(World { name, played: modified.is_some(), modified });
    }
    worlds.sort_by(|left, right| left.name.cmp(&right.name));
    worlds
}

/// One level of an instance's directory, directories first then files, each
/// name-sorted.
pub fn files(directory: &Path) -> Vec<Entry> {
    let mut entries = Vec::new();
    let Ok(read) = std::fs::read_dir(directory) else {
        return entries;
    };
    for entry in read.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let directory = path.is_dir();
        let bytes = if directory {
            0
        } else {
            std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0)
        };
        entries.push(Entry { name, directory, bytes });
    }
    entries.sort_by(|left, right| {
        right.directory.cmp(&left.directory).then_with(|| left.name.cmp(&right.name))
    });
    entries
}

/// The screenshots in `screenshots/`, newest name first.
///
/// Names rather than pixels: decoding is the engine's image cache, and a page that
/// decoded a directory of 1080p PNGs on every frame would be worse than one that
/// lists them.
pub fn screenshots(instance_dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = files(&instance_dir.join("screenshots"))
        .into_iter()
        .filter(|entry| !entry.directory)
        .filter(|entry| {
            let extension = entry.name.rsplit('.').next().unwrap_or_default().to_ascii_lowercase();
            matches!(extension.as_str(), "png" | "jpg" | "jpeg")
        })
        .map(|entry| entry.name)
        .collect();
    names.sort();
    names.reverse();
    names
}

/// The tail of an instance's newest log, newest line last.
///
/// `logs/latest.log` is what both Mojang's launcher and Prism write, and it is the
/// file the reference's Logs tab reads. The tail rather than the whole file: a log
/// that has been appended to for a year is megabytes, and a console shows the end
/// of it.
pub fn log_tail(instance_dir: &Path, lines: usize) -> Option<String> {
    let text = std::fs::read_to_string(instance_dir.join("logs").join("latest.log")).ok()?;
    let all: Vec<&str> = text.lines().collect();
    let start = all.len().saturating_sub(lines);
    Some(all[start..].join("\n"))
}

/// A byte count as the reference's `formatBytes` writes it.
///
/// Binary units with the `KiB`/`MiB`/`GiB` labels the reference's own strings use
/// (`format.bytes.0`..`format.bytes.4`), one decimal place past the first unit.
pub fn bytes_label(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use palantir_net::engine::request::{MapFetch, Route};
    use palantir_net::PISTON_MANIFEST_URL;

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join("palantirmc-store-tests").join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a scratch directory");
        root
    }

    #[test]
    fn an_unanswered_page_names_what_is_missing_without_naming_a_stage() {
        // The sentence a reader sees has to be true about what is missing, and it
        // must not read like a failure of their machine.
        let reason = not_implemented("Discover's search");
        assert!(reason.contains("is not implemented yet"), "{reason}");
        assert!(!reason.contains("stage"), "a stage number is a word for developers: {reason}");
        assert!(reason.starts_with("Discover's search"), "{reason}");
        assert!(!reason.to_lowercase().contains("error"), "{reason}");
    }

    /// A search body with one hit in it, in the API's own shape.
    const SEARCH_BODY: &str = r#"{
        "hits": [{
            "project_id": "AANobbMI", "slug": "sodium", "title": "Sodium",
            "description": "Modern rendering engine", "author": "jellysquid",
            "downloads": 41000000, "follows": 9000,
            "icon_url": "https://cdn.modrinth.com/icon.png", "latest_version": "mc1.21.4-0.6.5",
            "versions": ["1.21.4"], "categories": ["fabric", "optimization"]
        }],
        "offset": 0, "limit": 20, "total_hits": 214
    }"#;

    /// A store whose only way out is a server that answers what a test scripted.
    fn store_over(name: &str, fetch: Arc<MapFetch>) -> Store {
        Store::default().with_engine(Engine::over(scratch(name), fetch))
    }

    /// A version manifest with one release in it, in Mojang's own shape.
    const MANIFEST_BODY: &str = r#"{
        "latest": { "release": "1.21.4", "snapshot": "25w02a" },
        "versions": [
            { "id": "1.21.4", "type": "release", "url": "https://piston.invalid/1.21.4.json",
              "time": "2024-12-03T10:00:00+00:00", "releaseTime": "2024-12-03T10:00:00+00:00",
              "sha1": "0000000000000000000000000000000000000000", "complianceLevel": 1 }
        ]
    }"#;

    #[test]
    fn creating_an_instance_asks_mojang_which_version_is_current() {
        // The first write-side seam: the version is Mojang's answer rather than a
        // number this launcher invented, the folder is written in the launcher's
        // own format, and the list the pages draw from shows it afterwards.
        let home = scratch("create");
        let paths = PalantirPaths::at(&home);
        let fetch = Arc::new(MapFetch::new());
        fetch.set_route(PISTON_MANIFEST_URL, Route::text(MANIFEST_BODY));
        let mut store = Store::load(&paths)
            .with_engine(Engine::over(scratch("create-cache"), fetch.clone()));
        assert!(matches!(store.instances(), Load::Empty), "a home with nothing in it");

        let id = store.create_instance("Scratch instance", None).expect("an instance");
        assert!(
            store.instance_dir(&id).join("instance.cfg").exists(),
            "the instance is written where the launcher keeps them"
        );
        assert_eq!(fetch.count(), 1, "the version came from Mojang's own list, once");
        // And what was written is what the library draws from, once it is read
        // again -- which is the half a create without a reload would not have.
        store.reload();
        let cards = store.instances().ready().expect("the reloaded list");
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].id, id);
    }

    #[test]
    fn a_create_with_nothing_behind_it_says_what_is_missing() {
        // A store with no launcher and no engine is a test's store, and it answers
        // with the sentence rather than writing an instance somewhere invented.
        let reason = Store::default()
            .create_instance("Nowhere", None)
            .expect_err("nothing to create it with");
        assert!(reason.contains("is not implemented yet"), "{reason}");
        let named = Store::default()
            .create_instance("Nowhere", Some("1.21.4"))
            .expect_err("nothing to create it with");
        assert!(named.contains("Creating an instance"), "{named}");
    }

    #[test]
    fn a_search_through_the_engine_comes_back_as_cards() {
        // The whole seam in one test: a page's question goes out as the URL the
        // engine builds, and what comes back is the page's own `Hit` -- so a
        // change to either end of the seam fails here rather than in a window.
        let fetch = Arc::new(MapFetch::new());
        let store = store_over("search", fetch.clone());
        let query = ApiSearch::new("sodium").of_type("mod").sorted_by("downloads").with_limit(20);
        fetch.set_route(&query.url(), Route::text(SEARCH_BODY));

        let hits = store.search(&query).expect("an answer");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "AANobbMI", "the stable id, not the slug");
        assert_eq!(hits[0].title, "Sodium");
        assert_eq!(hits[0].author, "jellysquid");
        assert_eq!(hits[0].summary, "Modern rendering engine");
        assert_eq!(hits[0].downloads, 41_000_000);
        assert_eq!(hits[0].follows, 9_000);
        // The tags come from the hit itself: no second request per card.
        assert_eq!(hits[0].game_versions, vec!["1.21.4"]);
        assert_eq!(hits[0].loaders, vec!["fabric", "optimization"]);
        assert_eq!(fetch.count(), 1);

        // And the answer is cached, so the same search again is no request.
        let again = store.search(&query).expect("the same answer");
        assert_eq!(again, hits);
        assert_eq!(fetch.count(), 1, "a fresh search is not asked for twice");
    }

    #[test]
    fn a_request_runs_off_the_thread_that_draws_and_comes_back_through_a_channel() {
        // The crossing itself: the engine's call is blocking, so it happens on a
        // thread that owns it, and what the caller awaits is the channel. A test
        // can await it here because the shell's only part in this is which thread
        // the future runs on.
        let fetch = Arc::new(MapFetch::new());
        let store = store_over("off-thread", fetch.clone());
        let query = ApiSearch::new("sodium");
        fetch.set_route(&query.url(), Route::text(SEARCH_BODY));
        let asked = query.clone();
        let hits =
            futures::executor::block_on(off_thread(move || store.search(&asked))).expect("an answer");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Sodium");
    }

    #[test]
    fn a_search_that_fails_is_reported_rather_than_answered_with_nothing() {
        // A request that never happened is not an empty result set: a service
        // that answers 404 comes back as a reason, and the page prints it.
        let fetch = Arc::new(MapFetch::new());
        let store = store_over("search-failure", fetch);
        let reason = store.search(&ApiSearch::new("sodium")).expect_err("a failure");
        assert!(reason.contains("404"), "{reason}");
        assert!(!reason.is_empty());
    }

    #[test]
    fn a_store_with_no_engine_says_so_rather_than_answering_with_nothing() {
        // The arm a test store takes, and the sentence a page opens with: the two
        // have to be the same sentence, because a reader cannot be told which of
        // them they are looking at.
        let plain = Store::default();
        assert!(plain.engine().is_none());
        let reason = plain.search(&ApiSearch::new("sodium")).expect_err("no engine");
        assert_eq!(reason, not_implemented("Discover's search"));
        assert!(Store::default().with_engine(Engine::over(scratch("engine"), Arc::new(MapFetch::new()))).engine().is_some());
    }

    #[test]
    fn a_store_with_no_instances_says_empty_rather_than_nothing_happened() {
        // `Empty` and `Ready(vec![])` are different answers, and the pages draw
        // them differently: one is "you have none yet", the other is a list.
        let paths = PalantirPaths::at(scratch("empty-store"));
        let store = Store::load(&paths);
        assert_eq!(store.instances(), &Load::Empty);
        // And an id that is not there is empty rather than a failure.
        assert_eq!(store.instance("nothing"), Load::Empty);
        assert_eq!(store.instance_dir("nothing"), store.instances_dir().join("nothing"));
    }

    #[test]
    fn the_worlds_are_read_from_saves_and_know_whether_they_were_played() {
        let dir = scratch("worlds");
        std::fs::create_dir_all(dir.join("saves").join("Played")).expect("a world");
        std::fs::write(dir.join("saves").join("Played").join("level.dat"), b"x").expect("level.dat");
        std::fs::create_dir_all(dir.join("saves").join("Never saved")).expect("a folder");
        std::fs::write(dir.join("saves").join("loose.txt"), b"x").expect("a loose file");
        let found = worlds(&dir);
        assert_eq!(found.len(), 2, "a loose file is not a world");
        assert_eq!(found[0].name, "Never saved");
        assert!(!found[0].played, "a folder with no level.dat was never saved");
        assert!(found[1].played);
        assert!(found[1].modified.is_some());
        // A directory that does not exist is no worlds, not a panic.
        assert!(worlds(&dir.join("nowhere")).is_empty());
    }

    #[test]
    fn the_file_list_puts_directories_first_then_names() {
        let dir = scratch("files");
        std::fs::create_dir_all(dir.join("mods")).expect("a directory");
        std::fs::write(dir.join("options.txt"), b"hello").expect("a file");
        std::fs::write(dir.join("instance.cfg"), b"x").expect("a file");
        let entries = files(&dir);
        assert_eq!(entries[0].name, "mods");
        assert!(entries[0].directory);
        assert_eq!(entries[0].bytes, 0);
        assert_eq!(
            entries.iter().map(|entry| entry.name.as_str()).collect::<Vec<_>>(),
            vec!["mods", "instance.cfg", "options.txt"]
        );
        assert_eq!(entries[2].bytes, 5);
        assert!(files(&dir.join("nowhere")).is_empty());
    }

    #[test]
    fn only_images_are_screenshots_and_the_newest_come_first() {
        let dir = scratch("shots");
        let shots = dir.join("screenshots");
        std::fs::create_dir_all(&shots).expect("the folder");
        for name in ["2026-01-01_12.00.00.png", "2026-02-02_12.00.00.png", "notes.txt"] {
            std::fs::write(shots.join(name), b"x").expect("a file");
        }
        let names = screenshots(&dir);
        assert_eq!(names, vec!["2026-02-02_12.00.00.png", "2026-01-01_12.00.00.png"]);
        assert!(screenshots(&dir.join("nowhere")).is_empty());
    }

    #[test]
    fn a_log_is_read_from_its_end() {
        let dir = scratch("logs");
        std::fs::create_dir_all(dir.join("logs")).expect("the folder");
        let lines: Vec<String> = (0..100).map(|index| format!("line {index}")).collect();
        std::fs::write(dir.join("logs").join("latest.log"), lines.join("\n")).expect("a log");
        let tail = log_tail(&dir, 3).expect("a tail");
        assert_eq!(tail, "line 97\nline 98\nline 99");
        // A short log is the whole log, and a missing one is nothing rather than a
        // failure.
        let short = log_tail(&dir, 500).expect("a tail");
        assert!(short.starts_with("line 0"));
        assert_eq!(log_tail(&dir.join("nowhere"), 10), None);
    }

    #[test]
    fn a_byte_count_uses_the_reference_s_own_units() {
        assert_eq!(bytes_label(0), "0 B");
        assert_eq!(bytes_label(1023), "1023 B");
        assert_eq!(bytes_label(1024), "1.0 KiB");
        assert_eq!(bytes_label(1536), "1.5 KiB");
        assert_eq!(bytes_label(1024 * 1024), "1.0 MiB");
        assert_eq!(bytes_label(3 * 1024 * 1024 * 1024), "3.0 GiB");
        assert_eq!(bytes_label(5 * 1024_u64.pow(4)), "5.0 TiB");
        // Past the last unit it stays in it rather than inventing one.
        assert!(bytes_label(u64::MAX).ends_with("TiB"));
    }
}
