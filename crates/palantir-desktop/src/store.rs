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

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use palantir_core::instance::Instance;
use palantir_core::pack::PackProfile;
use palantir_core::paths::PalantirPaths;
use palantir_core::settings::Settings;
use palantir_net::engine::{
    Backoff, Build as LoaderBuildSource, Cancel, Fetch, HttpPool, Loader, LoaderMeta, Manifest,
    MetadataCache, ModrinthApi, PistonMeta, Request,
};
use palantir_net::engine::Search as ApiSearch;
use palantir_net::modrinth::{ModrinthMember, ModrinthProject, ModrinthProjectVersion, NewsArticle};
use palantir_net::{
    MinecraftSkins, MicrosoftAuth, SkinChange, DEFAULT_LIMIT, DEFAULT_TIMEOUT, DEFAULT_TTL,
};
use serde::{Deserialize, Serialize};

use crate::catalog::LoaderKind;
use crate::install;
use crate::instances::{self, ImportCandidate, InstanceCard, NewInstance};
use crate::mods::{self, ModEntry};
use crate::page::Load;
use crate::route::InstanceTab;
use crate::pages::discover::Hit;
use crate::pages::project::Project;
use crate::pages::user::Profile;
use crate::route::ProjectType;
use crate::skin::Appearance;
use crate::wire::Wire;

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
    /// Every launch this session has been asked for, by the instance it is about.
    ///
    /// The one thing here that is not read from disk, and it is here for the same
    /// reason the rest is: a page cannot ask the shell, and "an instance is
    /// running" is a fact a page's own controls are drawn from. The reference
    /// delivers it the same way -- its process list is a query keyed by instance
    /// (`instanceKeys.processes(id)`), which is why this is a map and not the one
    /// run the bar was drawn from: several instances can be running at once, and
    /// the bar's chip is about one of them while the popover is about all of
    /// them.
    launches: BTreeMap<String, Launch>,
    /// Which of them the action bar's chip is about -- the reference's *selected
    /// process*, the one its stop control, its logs button and its name belong
    /// to.
    ///
    /// An id rather than a flag on the entry, because what it names can stop
    /// while other runs go on: [`Store::selected_launch`] decides which id the
    /// chip is about at the moment it is asked.
    selected_launch: Option<String>,
}

/// What a launch is doing, as a page sees it.
///
/// Four states rather than a flag, because the header's control is not a boolean:
/// the reference draws *Play* when nothing is running, *Starting…* while the
/// launcher is preparing, *Stop* once the game is up and *Stopping…* while it is
/// being taken down (`page-header/index.vue`'s four arms). An instance that was
/// only "running or not" could not draw three of those.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LaunchState {
    /// Nothing is running.
    #[default]
    Idle,
    /// The launcher is preparing: signing in, resolving a version, fetching what
    /// is missing, extracting natives.
    Starting,
    /// The game is up.
    Running,
    /// The game is being taken down.
    Stopping,
}

/// One instance's launch, as the pages are drawn from it.
///
/// An entry outlives the run it describes, and that is the point: the last thing
/// a run said is worth exactly as much as the run, and a page that lost it the
/// moment the process exited would show a user who navigated away and back that
/// nothing had happened. So [`LaunchState::Idle`] means both "not running" and
/// "ran, and this is how it ended".
///
/// There is no `instance` field: the map this is held in is keyed by that.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Launch {
    /// What it is doing.
    pub state: LaunchState,
    /// The launch's own last word: the progress line while it works, the note it
    /// ended with. `None` until it has said anything.
    pub line: Option<String>,
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
#[derive(Clone)]
pub struct Engine {
    /// The pool every request this engine makes goes through, kept so that work
    /// which is not a metadata read can use it too.
    ///
    /// A module install and a launch's own files are downloads rather than
    /// document reads, and they belong on the engine's pool for the reason the
    /// engine exists: one connection pool, one timeout, one process-wide ceiling.
    /// A second pool built from the same numbers would be a second ceiling, which
    /// is the thing the numbers are for.
    fetch: Arc<dyn Fetch>,
    /// Modrinth over the engine's cache. An `Arc` because it is handed to a
    /// worker thread per request rather than borrowed across one.
    api: Arc<ModrinthApi>,
    /// The four mod loaders' own build lists, over the same cache.
    ///
    /// The same directory again, for [`Engine::piston`]'s reason: a service's
    /// answer is a service's answer, and the entry is told apart by its URL. This
    /// is the source the create flow's loader picker reads -- *not* Prism's mirror
    /// of it, which is what the retirement of the old shell takes away.
    loaders: Arc<LoaderMeta>,
    /// Mojang's own metadata over the *same* cache and pool as Modrinth's.
    ///
    /// One directory rather than two, because the launcher asks both services the
    /// same kind of question -- "what does this document say right now" -- and a
    /// second cache is a second set of stale answers nobody knows about. An `Arc`
    /// for the same reason the API's is: the engine is cloned into a worker.
    piston: Arc<PistonMeta>,
}

impl std::fmt::Debug for Engine {
    /// Hand-written, and only because of one field: [`Engine::fetch`] is a trait
    /// object, which has no `Debug` unless every implementor is made to have one.
    /// What is worth reading in a debug dump of a store is which services it can
    /// reach rather than the pool's own fields, so the pool is named and not
    /// printed.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine").field("fetch", &"<the engine's own pool>").finish_non_exhaustive()
    }
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
            fetch: fetch.clone(),
            api: Arc::new(ModrinthApi::new(cache, fetch.clone())),
            piston: Arc::new(PistonMeta::new(MetadataCache::new(dir.clone(), DEFAULT_TTL), fetch.clone())),
            loaders: Arc::new(LoaderMeta::new(MetadataCache::new(dir, DEFAULT_TTL), fetch)),
        }
    }

    /// The pool under every request this engine makes, for the transfers that
    /// are files rather than documents.
    pub fn fetch(&self) -> Arc<dyn Fetch> {
        self.fetch.clone()
    }

    /// Modrinth over this engine's cache.
    pub fn api(&self) -> &ModrinthApi {
        &self.api
    }

    /// Mojang's metadata over the same cache and pool.
    pub fn piston(&self) -> &PistonMeta {
        &self.piston
    }

    /// The loaders' own build lists over the same cache and pool.
    pub fn loaders(&self) -> &LoaderMeta {
        &self.loaders
    }
}

/// One project installed into one instance: what landed, and what to call it.
///
/// The three names are carried rather than looked up again because the sentence a
/// page shows uses all three and none of them is the id it asked with: a reader
/// pressed *Install* on a title and chose an instance by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    /// The project's title, as Modrinth spells it.
    pub project: String,
    /// The version that was chosen, as published.
    pub version: String,
    /// The instance's display name.
    pub instance: String,
    /// The file that landed, and where.
    pub file: crate::install::InstalledFile,
}

impl Installed {
    /// The line the project page shows once it has worked, in the reference's own
    /// shape ("Installed" and then the thing, rather than a stage name).
    pub fn summary(&self) -> String {
        format!(
            "Installed {} {} into {}",
            self.project, self.version, self.instance
        )
    }
}

/// What an install came to, in the two shapes the shell treats differently.
///
/// Both carry the sentence the page draws; only a pack carries somewhere for the
/// reader to *be*, which is the whole difference: a file went into an instance
/// they were already looking at, and a pack made one they have never seen -- and
/// the reference leaves them in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// A file in an instance, and the line about it.
    File(String),
    /// A new instance, and the line about it.
    Pack {
        /// The instance's id (its folder name), which is where to go.
        id: String,
        /// The line the page shows.
        line: String,
    },
}

/// One instance's own settings, as the settings modal reads and writes them.
///
/// The values are the ones a *launch* would use rather than the instance file's
/// raw contents: where an override gate is on the instance's own number is shown,
/// and where it is off the value in force is this launcher's own
/// ([`crate::prefs`]), which is the rule `launch::heap_for_launch` already obeys.
/// The gates travel with the values because the modal draws them as switches,
/// and a switch drawn from a missing value would be a guess about the file.
///
/// The strings are what the reader typed, not split or normalized: this is the
/// value a form holds, and `save_instance_settings` is where it becomes a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceSettings {
    /// The Java this instance runs with, when it overrides the launcher's.
    pub java_path: String,
    /// Whether the instance's own Java is what a launch uses.
    pub override_java: bool,
    /// Heap floor, in MiB.
    pub memory_min: i64,
    /// Heap ceiling, in MiB.
    pub memory_max: i64,
    /// Whether the instance's own heap is what a launch uses.
    pub override_memory: bool,
    /// Extra JVM arguments, in the one string the file holds.
    pub jvm_args: String,
    /// Whether the instance's own JVM arguments are what a launch uses.
    pub override_java_args: bool,
}

/// One instance's installation, as its settings modal reads and writes it: the
/// three facts `mmc-pack.json` carries and a launch resolves.
///
/// The platform is this launcher's own vocabulary ([`LoaderKind`]) rather than a
/// uid, because the tab and the builds the shell reads are keyed by it; the
/// file's own uid is what [`LoaderKind::uid`] maps it to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceInstallation {
    /// Vanilla, or one of the four loaders this launcher models.
    pub platform: LoaderKind,
    /// The Minecraft version the instance runs.
    pub game_version: String,
    /// The build of `platform` in force; empty for vanilla.
    pub loader_build: String,
}

/// The file an instance's link to the project it came from is kept in, beside
/// `mmc-pack.json`.
///
/// The reference keeps this on the instance itself: its `InstanceLink`
/// (`{ type: 'modrinth_modpack', project_id, version_id }`) lives inside the
/// `link` field of its own `profile.json`, and its panel re-reads the project and
/// version through the API to name them. This launcher's instances are
/// Prism-shaped, and Prism has no such field -- so the link is a file of its own
/// rather than a rewrite of a file another launcher owns, with the reference's own
/// field names inside it, so a reader who knows its `InstanceLink` recognises
/// this one.
///
/// It is deliberately *not* in `mmc-pack.json`: that file is a component list a
/// launch resolves, a Modrinth link is not a component, and a launcher that put
/// its own bookkeeping there would be writing a shape Prism reads.
pub const LINK_FILE: &str = "modrinth-link.json";

/// The one link type this launcher writes, in the reference's own word for it.
const MODRINTH_MODPACK: &str = "modrinth_modpack";

/// Which Modrinth project and version an instance was installed from.
///
/// Two ids rather than a title and a version number: the reference stores the
/// pair and asks the API for the names, so a project that is renamed shows under
/// its new name, and this is the reference's shape rather than one of this
/// launcher's own invention. [`Store::linked_modpack`] is the read that turns it
/// into something a card can draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceLink {
    /// The project the instance was installed from.
    pub project_id: String,
    /// The version of that project which was installed.
    pub version_id: String,
}

/// The link, named: what the installation tab's modpack card draws.
///
/// The author and the version number are captions and both can be missing -- a
/// project's team can fail to read, and a version the author has since deleted is
/// gone from the API -- so both are empty strings rather than errors, and the card
/// draws what it has. What cannot be missing is the title: without it there is
/// nothing to say which pack this instance came from, and the read fails instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedModpack {
    /// The project's id, which is also what the card's link to the project page
    /// would be built from.
    pub project_id: String,
    /// The project's title, as Modrinth spells it now.
    pub title: String,
    /// The author's name, or empty when the team could not be read.
    pub author: String,
    /// The linked version's number, or empty when the version is gone.
    pub version: String,
}

/// The link file's own shape: the reference's field names, and its `type`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LinkFile {
    /// `modrinth_modpack`, which is the only kind this launcher writes.
    #[serde(rename = "type")]
    kind: String,
    project_id: String,
    version_id: String,
}

/// Set a string setting behind its instance override gate, or take both away.
///
/// A blank string is not a value here: an empty Java path or argument string is
/// the same answer as the gate being off, so both keys go rather than a gate
/// pointing at nothing.
fn set_gated_string(settings: &mut Settings, gate: &str, key: &str, on: bool, value: &str) {
    if on && !value.is_empty() {
        settings.set_bool(gate, true);
        settings.set_str(key, value);
    } else {
        settings.set_bool(gate, false);
        settings.remove(key);
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
            launches: BTreeMap::new(),
            selected_launch: None,
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

    /// Mojang's manifest, or the reason there is none.
    ///
    /// One reader for the two questions the create flow asks it, so that a change
    /// to how the list is fetched or how a failure is worded is in one place.
    /// **Blocking**, like every engine call: the shell runs it off the frame
    /// thread.
    fn manifest(&self) -> Result<Manifest, String> {
        let Some(engine) = &self.engine else {
            return Err(not_implemented("Minecraft's version list"));
        };
        engine
            .piston()
            .manifest(&Cancel::new(), &Backoff::default())
            .map_err(|error| format!("Mojang's version list could not be read: {error}"))
    }

    /// The newest release Mojang publishes, as the launcher's own metadata
    /// reader sees it.
    ///
    /// This is what a create flow opens on when it has no version chosen: the
    /// reference asks for a version, and a launcher that has to be told which
    /// Minecraft exists is a launcher that will be wrong the week a release lands.
    pub fn current_release(&self) -> Result<String, String> {
        self.manifest()?
            .newest_release()
            .map(|version| version.id.clone())
            .ok_or_else(|| "Mojang's version list names no release".to_string())
    }

    /// Every version Mojang publishes, with the one a picker opens on.
    ///
    /// **Blocking**, and the one request the creation dialog makes: the list is a
    /// thousand entries, and a picker that asked per frame would make a request
    /// per frame. Both facts come out of the same document on purpose -- a picker
    /// that asked twice could be told about a list and about "current" at two
    /// different moments, and then open on a version that is in neither.
    pub fn versions(&self) -> Result<VersionList, String> {
        let manifest = self.manifest()?;
        Ok(VersionList {
            latest_release: manifest.latest_release,
            versions: manifest
                .versions
                .iter()
                .map(|version| GameVersion {
                    id: version.id.clone(),
                    release: version.is_release(),
                })
                .collect(),
        })
    }

    /// Create an instance in this launcher's own format.
    ///
    /// **Blocking**, for [`Store::search`]'s reason: it writes a folder, a config
    /// and a version profile, and the shell runs it on the thread it runs requests
    /// on. `game` is the version the instance is for; `None` asks Mojang which one
    /// is current, which is what the create flow does when nothing was chosen.
    ///
    /// `loader` and `build` are what the dialog's chips chose. A loader with no
    /// build is a create that writes a vanilla instance with a warning rather than
    /// a component with no version in it -- `instances::create` says so in the
    /// warning it returns -- because a component with nothing to resolve is an
    /// instance that cannot launch at all.
    ///
    /// The answer is the new instance's id -- what the address to open it with is
    /// made of -- and the shell reloads the list it draws from.
    pub fn create_instance(
        &self,
        name: &str,
        game: Option<&str>,
        loader: LoaderKind,
        build: Option<&str>,
    ) -> Result<String, String> {
        let Some(paths) = &self.paths else {
            return Err(not_implemented("Creating an instance"));
        };
        let game = match game.map(str::trim).filter(|game| !game.is_empty()) {
            Some(game) => game.to_string(),
            None => self.current_release()?,
        };
        let spec = NewInstance {
            loader,
            loader_build: build.map(str::to_string),
            ..NewInstance::vanilla(name, &game)
        };
        instances::create(paths, &spec).map(|created| created.id)
    }

    /// Every build of `loader` that can run `game`, newest first.
    ///
    /// **Blocking**, and the other request the creation dialog makes: the list is
    /// a service's answer rather than a field read, and the eight loader chips a
    /// dialog draws do not each cost a request -- only the one that was chosen
    /// does.
    ///
    /// Vanilla asks nothing and answers with nothing, because there is no such
    /// service and no such build: the picker draws no build list for it.
    pub fn loader_builds(&self, loader: LoaderKind, game: &str) -> Result<Vec<LoaderBuild>, String> {
        let Some(loader) = Loader::from_name(loader.modrinth_name()) else {
            return Ok(Vec::new());
        };
        let Some(engine) = &self.engine else {
            return Err(not_implemented("The loader's build list"));
        };
        engine
            .loaders()
            .builds(loader, game, &Cancel::new(), &Backoff::default())
            .map(|builds| builds.into_iter().map(LoaderBuild::from).collect())
            .map_err(|error| format!("{}'s build list could not be read: {error}", loader.name()))
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

    /// One instance's launch, if it has ever been asked to run.
    pub fn launch(&self, id: &str) -> Option<&Launch> {
        self.launches.get(id)
    }

    /// Every run that is not idle, in instance order: what the action bar's
    /// popover lists, and what decides whether it is drawn at all.
    pub fn running_launches(&self) -> Vec<(&str, &Launch)> {
        self.launches
            .iter()
            .filter(|(_, launch)| launch.state != LaunchState::Idle)
            .map(|(id, launch)| (id.as_str(), launch))
            .collect()
    }

    /// Which run the bar's chip is about, if any.
    ///
    /// Derived rather than stored, because the id this is drawn from may name a
    /// run that has since ended: the selected instance while it is running, and
    /// otherwise the first of the runs that are. A chip that went on naming a run
    /// which stopped while another was still going would be the bar lying about
    /// what its own stop control is attached to. `None` is the chip's *No
    /// instances running* face.
    pub fn selected_launch(&self) -> Option<&str> {
        if let Some(id) = &self.selected_launch {
            if self.launch_state(id) != LaunchState::Idle {
                return Some(id);
            }
        }
        self.launches
            .iter()
            .find(|(_, launch)| launch.state != LaunchState::Idle)
            .map(|(id, _)| id.as_str())
    }

    /// Make `id` the run the bar's chip is about.
    ///
    /// What the reference's popover does when one of its rows is pressed: the
    /// chip becomes that process, and with it the stop control and the name.
    pub fn select_launch(&mut self, id: &str) {
        self.selected_launch = Some(id.to_string());
    }

    /// Record what `id`'s launch is doing.
    ///
    /// Called by the shell and by nothing else: it is the one object that knows
    /// which instance was asked to run, and the pages read the answer rather than
    /// deciding it.
    ///
    /// A *new* entry becomes the selected one, so the chip follows the press that
    /// started a run and a second press moves it to the second game. Every later
    /// word about that same run -- its lines, its level, the note it ends with --
    /// arrives as an entry that already exists and moves nothing.
    pub fn set_launch(&mut self, id: &str, launch: Launch) {
        let first = self.launches.insert(id.to_string(), launch).is_none();
        if first {
            self.selected_launch = Some(id.to_string());
        }
    }

    /// What `id`'s launch is doing.
    ///
    /// [`LaunchState::Idle`] for every instance that is not running, so a page
    /// asks this with its own id and never has to compare anything itself.
    pub fn launch_state(&self, id: &str) -> LaunchState {
        self.launches.get(id).map_or(LaunchState::Idle, |launch| launch.state)
    }

    /// The launch's own last word, when it is about `id`.
    pub fn launch_line(&self, id: &str) -> Option<&str> {
        self.launches.get(id).and_then(|launch| launch.line.as_deref())
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

    /// This launcher's own directory, when the store was read from one.
    ///
    /// Where the skins this launcher keeps for the reader live
    /// (`crate::saved_skins`): the *launcher's* own folder, and not the data root,
    /// for the reason `prefs` gives -- the data root can be an install another
    /// launcher created and can change, while what this product stores under its
    /// own name cannot. `None` for a store that was built over a fetch double
    /// rather than read from a directory, which is how the tests build one.
    pub fn paths(&self) -> Option<&PalantirPaths> {
        self.paths.as_ref()
    }

    /// One instance's own settings, read back for the settings modal.
    ///
    /// A read of two small files (its `instance.cfg` and this launcher's own
    /// preferences), which is why it is synchronous where a page's tab listing is
    /// not: the shell asks for it once, when the modal opens.
    pub fn instance_settings(&self, id: &str) -> Result<InstanceSettings, String> {
        let instance = Instance::open(&self.instance_dir(id))
            .map_err(|error| format!("cannot open '{id}': {error}"))?;
        let settings = instance.settings();
        let defaults = self.launch_defaults();
        let override_memory = settings.get_bool("OverrideMemory", false);
        let override_java = settings.get_bool("OverrideJavaLocation", false);
        let override_java_args = settings.get_bool("OverrideJavaArgs", false);
        Ok(InstanceSettings {
            java_path: if override_java {
                settings.get_str("JavaPath", "")
            } else {
                defaults.java.default_binary().unwrap_or_default().to_string()
            },
            override_java,
            memory_min: if override_memory {
                settings.get_i64("MinMemAlloc", defaults.min_mem_mib)
            } else {
                defaults.min_mem_mib
            },
            memory_max: if override_memory {
                settings.get_i64("MaxMemAlloc", defaults.max_mem_mib)
            } else {
                defaults.max_mem_mib
            },
            override_memory,
            // There are no launcher-wide JVM arguments to fall back to
            // (`launch::LaunchDefaults` holds Java and the two numbers and
            // nothing else), so an instance that does not override them runs
            // with none -- and the field draws empty rather than showing a
            // string a launch would drop on the floor.
            jvm_args: if override_java_args {
                settings.get_str("JvmArgs", "")
            } else {
                String::new()
            },
            override_java_args,
        })
    }

    /// The numbers and the Java an instance inherits while it does not override
    /// them: this launcher's own preferences, read the way a launch reads them.
    fn launch_defaults(&self) -> crate::launch::LaunchDefaults {
        match &self.paths {
            Some(paths) => crate::launch::LaunchDefaults::from_prefs(&crate::prefs::load(paths)),
            None => crate::launch::LaunchDefaults::default(),
        }
    }

    /// Write one instance's own settings back to its file.
    ///
    /// A gate that is off *takes the instance's key away* rather than leaving
    /// the number in the file: the modal reads the value in force back every time
    /// it opens, so a stale number would be a value nothing reads and nothing
    /// shows. A heap that cannot be one -- below the game's own floor, or upside
    /// down -- is a sentence returned to the form rather than a written line.
    pub fn save_instance_settings(&self, id: &str, edit: &InstanceSettings) -> Result<(), String> {
        if edit.override_memory {
            let floor = palantir_core::settings::defaults::MIN_MEM_ALLOC;
            if edit.memory_min < floor {
                return Err(format!(
                    "the minimum heap is {} MiB; {floor} MiB is the floor",
                    edit.memory_min
                ));
            }
            if edit.memory_max < edit.memory_min {
                return Err(format!(
                    "the maximum heap ({} MiB) is below the minimum ({} MiB)",
                    edit.memory_max, edit.memory_min
                ));
            }
        }
        let mut instance = Instance::open(&self.instance_dir(id))
            .map_err(|error| format!("cannot open '{id}': {error}"))?;
        let settings = instance.settings_mut();
        set_gated_string(
            settings,
            "OverrideJavaLocation",
            "JavaPath",
            edit.override_java,
            edit.java_path.trim(),
        );
        set_gated_string(
            settings,
            "OverrideJavaArgs",
            "JvmArgs",
            edit.override_java_args,
            edit.jvm_args.trim(),
        );
        settings.set_bool("OverrideMemory", edit.override_memory);
        if edit.override_memory {
            settings.set_i64("MinMemAlloc", edit.memory_min);
            settings.set_i64("MaxMemAlloc", edit.memory_max);
        } else {
            settings.remove("MinMemAlloc");
            settings.remove("MaxMemAlloc");
        }
        instance
            .save()
            .map_err(|error| format!("saving '{id}' failed: {error}"))
    }

    /// One instance's installation, read from its own `mmc-pack.json`.
    ///
    /// A loader the file names but this launcher does not model (Prism's
    /// LiteLoader) answers as vanilla here and is *refused on the write side*:
    /// the tab draws what it knows, and a save that would have to take out a
    /// component it cannot name is a sentence instead.
    pub fn instance_installation(&self, id: &str) -> Result<InstanceInstallation, String> {
        let instance = Instance::open(&self.instance_dir(id))
            .map_err(|error| format!("cannot open '{id}': {error}"))?;
        let path = instance.mmc_pack_path();
        let profile = PackProfile::load(&path)
            .map_err(|error| format!("reading {} failed: {error}", path.display()))?;
        let platform = LoaderKind::all()
            .into_iter()
            .find(|kind| {
                kind.uid()
                    .and_then(|uid| profile.get(uid))
                    .map(|component| component.is_enabled())
                    .unwrap_or(false)
            })
            .unwrap_or(LoaderKind::Vanilla);
        Ok(InstanceInstallation {
            platform,
            game_version: profile
                .get(crate::catalog::MINECRAFT_UID)
                .map(|component| component.version.clone())
                .unwrap_or_default(),
            loader_build: platform
                .uid()
                .and_then(|uid| profile.get(uid))
                .map(|component| component.version.clone())
                .unwrap_or_default(),
        })
    }

    /// Write an instance's installation back to its `mmc-pack.json`.
    ///
    /// The two components are the whole write: `net.minecraft` carries the game
    /// version, the platform's own component carries its build, and every other
    /// loader is *taken out*, because two loader components in one profile is a
    /// profile nothing can resolve -- `ModLoader::conflicting_uids` is the same
    /// rule stated by the model. A component this launcher does not model is a
    /// refusal rather than a deletion, because a save that silently dropped one
    /// would be this tab editing a file it cannot read.
    ///
    /// What happens to a changed version or loader is the next launch's: the run
    /// resolves the profile it finds and fetches what is not here yet, which is
    /// the same path a first launch takes ([`crate::launch`]).
    pub fn save_instance_installation(
        &self,
        id: &str,
        edit: &InstanceInstallation,
    ) -> Result<(), String> {
        let game = edit.game_version.trim();
        if game.is_empty() {
            return Err("pick a game version".to_string());
        }
        let build = edit.loader_build.trim();
        if edit.platform.loads_mods() && build.is_empty() {
            return Err(format!(
                "pick a {} build: an empty one leaves a loader to resolve with no version to \
                 resolve it to",
                edit.platform.label()
            ));
        }
        let instance = Instance::open(&self.instance_dir(id))
            .map_err(|error| format!("cannot open '{id}': {error}"))?;
        let path = instance.mmc_pack_path();
        let mut profile = PackProfile::load(&path)
            .map_err(|error| format!("reading {} failed: {error}", path.display()))?;
        for loader in profile.mod_loaders() {
            if LoaderKind::from_uid(loader.uid()).is_none() {
                return Err(format!(
                    "this instance runs {}, a loader this launcher does not model; changing \
                     the platform here would take that component out",
                    loader.uid()
                ));
            }
        }
        profile.set_version(crate::catalog::MINECRAFT_UID, game, true);
        for kind in LoaderKind::all() {
            let Some(uid) = kind.uid() else { continue };
            if Some(uid) == edit.platform.uid() {
                continue;
            }
            // Our own writes mark a loader `important`, and `remove` refuses an
            // important component -- Prism's rule, because the reader asked for
            // it. The flag is cleared first, because this *is* the reader asking
            // for it to go.
            if let Some(component) = profile.get_mut(uid) {
                component.important = false;
            }
            profile.remove(uid);
        }
        if let Some(uid) = edit.platform.uid() {
            profile.set_version(uid, build, true);
        }
        profile
            .save(&path)
            .map_err(|error| format!("writing {} failed: {error}", path.display()))
    }

    /// The project an instance was installed from, if it came from one.
    ///
    /// `Ok(None)` is an instance nobody linked -- every instance this launcher
    /// makes by hand, and every instance it imported from another launcher. A
    /// link file that is there and cannot be believed is an `Err` rather than a
    /// `None`, because the difference matters to the reader: one means "this
    /// instance did not come from a pack", the other means "it did, and this
    /// launcher cannot read the file that says so" -- and only the second is a
    /// thing they could fix.
    pub fn instance_link(&self, id: &str) -> Result<Option<InstanceLink>, String> {
        let path = self.instance_dir(id).join(LINK_FILE);
        if !path.is_file() {
            return Ok(None);
        }
        let text = palantir_core::util::read_text(&path)
            .map_err(|error| format!("reading {} failed: {error}", path.display()))?;
        let file: LinkFile = serde_json::from_str(&text)
            .map_err(|error| format!("{} is not a link this launcher wrote: {error}", path.display()))?;
        if file.kind != MODRINTH_MODPACK {
            return Err(format!(
                "{} is linked as a {}, a kind of link this launcher does not draw yet",
                path.display(),
                file.kind
            ));
        }
        if file.project_id.trim().is_empty() || file.version_id.trim().is_empty() {
            return Err(format!("{} names no project or version", path.display()));
        }
        Ok(Some(InstanceLink { project_id: file.project_id, version_id: file.version_id }))
    }

    /// Write an instance's link, atomically.
    ///
    /// `prefs`' rule and `saved_skins`', for the same reason: the file is read by
    /// the next run, and a half-written one is an instance whose pack cannot be
    /// named.
    pub fn save_instance_link(&self, id: &str, link: &InstanceLink) -> Result<(), String> {
        let path = self.instance_dir(id).join(LINK_FILE);
        let file = LinkFile {
            kind: MODRINTH_MODPACK.to_string(),
            project_id: link.project_id.clone(),
            version_id: link.version_id.clone(),
        };
        let mut text = serde_json::to_string_pretty(&file).map_err(|error| error.to_string())?;
        text.push('\n');
        palantir_core::util::atomic_write(&path, text.as_bytes())
            .map_err(|error| format!("writing {} failed: {error}", path.display()))
    }

    /// Forget an instance's link, and say whether there was one.
    ///
    /// What unlinks an instance is exactly this: the *file* goes and everything
    /// else stays, because the instance and the files a pack install put in it are
    /// not the link's. That is what the reference says unlink does ("permanently
    /// disconnects this instance from the pack project, allowing you to change the
    /// loader and Minecraft version, but you won't receive future updates"), and it
    /// is why this is a deletion rather than a flag: there is no half-linked state
    /// for the rest of the launcher to check.
    pub fn clear_instance_link(&self, id: &str) -> Result<bool, String> {
        let path = self.instance_dir(id).join(LINK_FILE);
        if !path.exists() {
            return Ok(false);
        }
        std::fs::remove_file(&path)
            .map_err(|error| format!("removing {} failed: {error}", path.display()))?;
        Ok(true)
    }

    /// The pack an instance came from, named for the installation tab's card.
    ///
    /// **Blocking**, for [`Store::project`]'s reason, and three requests for one
    /// card for the same reason: the project document names the title and the
    /// team, the team names the author, and the version number lives in the
    /// versions list rather than in a document of its own. All three are cached
    /// under their own URLs by the engine, so a modal reopened is free.
    ///
    /// Two of the three failures are deliberately not this call's: an author is a
    /// caption, and a version the author has deleted is a version this card can no
    /// longer name. Both are drawn as nothing rather than as a broken card, which
    /// is the reference's own arm (`modpackInfo.value.version?.version_number`).
    pub fn linked_modpack(&self, id: &str) -> Result<Option<LinkedModpack>, String> {
        let Some(link) = self.instance_link(id)? else {
            return Ok(None);
        };
        let Some(engine) = &self.engine else {
            return Err(not_implemented("The project this instance came from"));
        };
        let cancel = Cancel::new();
        let backoff = Backoff::default();
        let api = engine.api();
        let project = api
            .project(&link.project_id, &cancel, &backoff)
            .map_err(|error| error.to_string())?;
        let members = api.members(&link.project_id, &cancel, &backoff).unwrap_or_default();
        let versions = api.versions(&link.project_id, &cancel, &backoff).unwrap_or_default();
        Ok(Some(LinkedModpack {
            project_id: link.project_id,
            title: project.title,
            author: author_of(&members).to_string(),
            version: versions
                .iter()
                .find(|version| version.id == link.version_id)
                .map(|version| version.version_number.clone())
                .unwrap_or_default(),
        }))
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

    /// Modrinth's news feed, for the panel's news section.
    ///
    /// **Blocking**, like every engine call, so the shell runs it off the frame
    /// thread -- and asked for *once*, at startup, because the panel is drawn on
    /// every route: a section that waited for a page to ask for it would blank out
    /// whenever the reader moved. The engine holds the answer for the metadata
    /// default, and the panel draws the newest four of it.
    ///
    /// A store with no engine says so in the same sentence every other unbuilt
    /// thing does; the panel draws no section for either, because there is nothing
    /// here a reader can act on.
    pub fn news(&self) -> Result<Vec<NewsArticle>, String> {
        let Some(engine) = &self.engine else {
            return Err(not_implemented("The news feed"));
        };
        let cancel = Cancel::new();
        let backoff = Backoff::default();
        engine
            .api()
            .news(&cancel, &backoff)
            .map_err(|error| error.to_string())
    }

    /// The account's own appearance, ready for the Skins page to draw.
    ///
    /// Two reads in one place: Minecraft's profile document -- which skins and
    /// capes the account owns, and which of them is in force -- and then the
    /// texture of the skin in force, over the engine's own pool. The reference's
    /// Skins page is the same document through a Tauri plugin this tree does not
    /// have; see [`crate::skin`] for what is drawn from it and what that costs.
    ///
    /// The texture's failure is deliberately *not* this call's failure: an account
    /// on a machine with no connection still owns every skin it owns, and a page
    /// that threw the lists away because a picture would not come back would be
    /// showing less than it knows. [`Appearance::of`] is where that split is made.
    ///
    /// **Blocking**, like every network call here, so the shell runs it off the
    /// frame thread. The auth path owns its transport the way a launch's does: this
    /// launcher has one `MicrosoftAuth`, and the alternative would be a second place
    /// that knows how to talk to Microsoft.
    pub fn appearance(&self, username: &str, token: &str) -> Result<Appearance, String> {
        let owned = self.skins(token)?;
        // Which skin is in force is the document's own answer, and a document that
        // does not name one is a reason rather than an empty picture: the page
        // says so instead of drawing the first skin in the list.
        let texture = match owned.equipped() {
            Some(skin) => self.skin_texture(&skin.url),
            None => Err("Minecraft does not say which skin is in force.".to_string()),
        };
        Ok(Appearance::of(username, owned, texture))
    }

    /// The skins and capes the account a launch would sign in as owns.
    ///
    /// `palantir_net`'s read of Minecraft's own profile document, in the crate that
    /// already signs this launcher in: the token is the account's game token, which
    /// is the same one `--accessToken` carries into the game.
    pub fn skins(&self, token: &str) -> Result<MinecraftSkins, String> {
        MicrosoftAuth::with_public_client_id()
            .skins(token)
            .map_err(|error| error.to_string())
    }

    /// Change what the account is wearing, through Minecraft's own skin service.
    ///
    /// The write half of [`Self::appearance`], and the same account and token: what
    /// the page drew a moment ago is what this changes, which is why the two share a
    /// caller rather than a client of their own. Four of the five changes name
    /// something the account already owns; the fifth *uploads* the bytes behind one
    /// ([`SkinChange::Upload`]), which the desktop has already read, padded and turned
    /// into a PNG by the time a change reaches here -- so this stays one pass-through
    /// rather than growing a second parameter for a file.
    ///
    /// **Blocking**, for [`Self::appearance`]'s reason.
    pub fn wear(&self, token: &str, change: SkinChange) -> Result<(), String> {
        MicrosoftAuth::with_public_client_id()
            .wear(token, change)
            .map_err(|error| error.to_string())
    }

    /// One skin texture, as the bytes of the PNG it is.
    ///
    /// Over the engine's pool, for [`Self::news`]'s reason: every request this
    /// launcher makes goes through the one client and the one ceiling, and a texture
    /// is a request like any other. Deliberately *not* through the metadata cache: a
    /// texture is a file rather than a document, nothing here revalidates it, and
    /// what names it is a URL the account's own document handed out a moment ago.
    pub fn skin_texture(&self, url: &str) -> Result<Vec<u8>, String> {
        let Some(engine) = &self.engine else {
            return Err(not_implemented("This skin's texture"));
        };
        let cancel = Cancel::new();
        engine
            .fetch()
            .get(&Request::get(url), &cancel)
            .map_err(|error| format!("fetching the skin's texture failed: {error}"))
    }

    /// Read one project: its own document, its team, and its versions.
    ///
    /// **Blocking**, for [`Store::search`]'s reason, and three requests for the
    /// one answer because Modrinth splits a project three ways: the project
    /// document names the team but not the person on it, and the versions are a
    /// list endpoint of their own. All three are cached by the engine under their
    /// own URLs, so a page revisited is free.
    ///
    /// A team that cannot be read is the one failure that does not fail the page:
    /// the author's name is a caption under the title, and a project with no
    /// caption is a project. Everything else is the answer or its reason.
    pub fn project(&self, id: &str) -> Result<Project, String> {
        let Some(engine) = &self.engine else {
            return Err(not_implemented("This project"));
        };
        let cancel = Cancel::new();
        let backoff = Backoff::default();
        let api = engine.api();
        let project = api
            .project(id, &cancel, &backoff)
            .map_err(|error| error.to_string())?;
        let members = api.members(id, &cancel, &backoff).unwrap_or_default();
        let versions = api
            .versions(id, &cancel, &backoff)
            .map_err(|error| error.to_string())?;
        Ok(Project::from_api(&project, author_of(&members), &versions))
    }

    /// One user's profile: their own document, the projects they own, and their
    /// avatar.
    ///
    /// **Blocking**, for [`Store::project`]'s reason, and three requests for the one
    /// answer for the same reason Modrinth splits a project three ways: the profile
    /// document is asked for by the *name* an address spells, and the projects are a
    /// second endpoint keyed by the id only that document carries -- which is why
    /// the id is read out of the first answer rather than guessed from the second.
    /// A user who owns nothing is an empty list and a real profile, not a failure:
    /// the reference draws its own empty sentence for that, and a launcher that
    /// called it an error would show a broken page for every new account.
    ///
    /// The avatar's failure is deliberately *not* this call's failure, and the
    /// profile carries the reason instead: an account on a machine with no
    /// connection still has a name, a bio and a list of projects, and a picture is
    /// the one thing here that is decoration (see [`Profile::of`]). An account with
    /// no avatar at all is not asked for one and is not told about it either -- the
    /// service publishes an empty `avatar_url` for it, not a missing one.
    pub fn user(&self, username: &str) -> Result<Profile, String> {
        let Some(engine) = &self.engine else {
            return Err(not_implemented("This profile"));
        };
        let cancel = Cancel::new();
        let backoff = Backoff::default();
        let api = engine.api();
        let user = api
            .user(username, &cancel, &backoff)
            .map_err(|error| error.to_string())?;
        let projects = api
            .user_projects(&user.id, &cancel, &backoff)
            .map_err(|error| error.to_string())?;
        let avatar = if user.avatar_url.is_empty() {
            Err("This account has no avatar.".to_string())
        } else {
            engine
                .fetch()
                .get(&Request::get(&user.avatar_url), &cancel)
                .map_err(|error| format!("fetching the avatar failed: {error}"))
        };
        Ok(Profile::of(user, projects, avatar))
    }

    /// Install one project into one instance.
    ///
    /// **Blocking**, like every engine call: the shell runs it off the frame
    /// thread.
    ///
    /// The whole path is one decision -- *which version, and where does it go* --
    /// and both halves of it come from the instance the reader picked: the version
    /// is the newest one that matches that instance's game version and loader
    /// ([`install::preferred_version`]), and the folder is what the project *is*
    /// ([`ProjectType::target_folder`]). Neither is the page's to decide, which is
    /// why the button reports a press and this does the work.
    ///
    /// The two documents the version comes out of are read again rather than taken
    /// from the page: the page has a title and a version list to draw, not a
    /// `project_type`, and a launcher that let a page tell it where to write would
    /// be trusting the page's copy of an answer the service has since replaced.
    /// Both reads are cached, so a press right after a page load costs nothing.
    pub fn install_project(&self, project_id: &str, instance_id: &str) -> Result<Installed, String> {
        let Some(engine) = &self.engine else {
            return Err(not_implemented("Installing a project"));
        };
        let Some(paths) = &self.paths else {
            // A store with no launcher behind it has no cache directory to
            // download through and no instances directory to write into.
            return Err(not_implemented("Installing a project"));
        };
        let Load::Ready(card) = self.instance(instance_id) else {
            return Err(format!("there is no instance called '{instance_id}'"));
        };
        let cancel = Cancel::new();
        let backoff = Backoff::default();
        let api = engine.api();
        let project = api
            .project(project_id, &cancel, &backoff)
            .map_err(|error| error.to_string())?;
        let versions = api
            .versions(project_id, &cancel, &backoff)
            .map_err(|error| error.to_string())?;
        let target = format!("{} {}", card.loader.label(), card.mc_version);
        let version = install::preferred_version(
            &versions,
            &project.project_type,
            &card.mc_version,
            card.loader.modrinth_name(),
        )
        .ok_or_else(|| format!("{} has no version for {target}", project.title))?;
        let kind = ProjectType::from_token(&project.project_type);
        let Some(folder) = kind.and_then(ProjectType::target_folder) else {
            // A pack is not a file that goes in a folder, and saying which kind it
            // is is more use than saying "no".
            return Err(format!(
                "{} is a {}, and one of those becomes an instance of its own rather than a folder \
                 inside one",
                project.title,
                kind.map(|kind| kind.sentence(1)).unwrap_or("project")
            ));
        };
        // The engine's own pool, not a second one built from the same numbers: a
        // mod and the launch that will load it are one ceiling apart.
        let wire = Wire::over(paths.meta_dir(), engine.fetch());
        let dir = self.instance_dir(instance_id).join(folder);
        let file = install::install_file(&wire, &dir, version)?;
        Ok(Installed {
            project: project.title,
            version: version.version_number.clone(),
            instance: card.name,
            file,
        })
    }

    /// Install a modpack as an instance of its own.
    ///
    /// **Blocking**, for [`Store::install_project`]'s reason and a bigger one: this
    /// is a document read, a pack download, an unpack and a download per file the
    /// pack lists, which is the longest thing this launcher does that is not a
    /// launch.
    ///
    /// The version is chosen by [`install::newest_pack_version`] rather than by the
    /// instance-matching rule, because there is no instance yet to match: a pack
    /// carries its own Minecraft version and its own loaders in its index, and
    /// *that* is what the instance it makes is made of. The instance's name is the
    /// project's title, which is what the reader pressed and what the reference
    /// names it.
    pub fn install_pack(&self, project_id: &str) -> Result<Outcome, String> {
        let Some(engine) = &self.engine else {
            return Err(not_implemented("Installing a modpack"));
        };
        let Some(paths) = &self.paths else {
            return Err(not_implemented("Installing a modpack"));
        };
        let cancel = Cancel::new();
        let backoff = Backoff::default();
        let api = engine.api();
        let project = api
            .project(project_id, &cancel, &backoff)
            .map_err(|error| error.to_string())?;
        let versions = api
            .versions(project_id, &cancel, &backoff)
            .map_err(|error| error.to_string())?;
        let version = install::newest_pack_version(&versions)
            .ok_or_else(|| format!("{} has published nothing to install", project.title))?;
        // The archive is cached outside every instance, under the digest the API
        // published, so a second install of the same pack version is a read of a
        // file this launcher already checked rather than another 300 MB.
        let wire = Wire::over(paths.meta_dir(), engine.fetch());
        let archive = install::fetch_pack_archive(&wire, &paths.meta_dir().join("packs"), version)?;
        let installed = install::install_pack_archive(
            &wire,
            paths,
            &archive,
            project.title.as_str(),
            &mut |_| {},
        )?;
        // The link is written here, where the project and version are still in
        // hand, and before the answer: an instance that appears in the library is
        // an instance whose own settings modal can say what it came from
        // ([`Store::linked_modpack`]). A link that would not write is *not* an
        // install that failed -- the files are on disk and the instance is
        // playable -- so the line the page draws carries the reason instead, which
        // is the one place a reader would see it.
        let summary = installed.summary(&project.title);
        let line = match self.save_instance_link(
            &installed.id,
            &InstanceLink {
                project_id: project.id.clone(),
                version_id: version.id.clone(),
            },
        ) {
            Ok(()) => summary,
            Err(error) => format!("{summary} (the link to it could not be recorded: {error})"),
        };
        Ok(Outcome::Pack { id: installed.id.clone(), line })
    }

    /// Re-install one instance's files, checking what is already on disk.
    ///
    /// **Blocking**, for [`Store::install_project`]'s reason and a longer one than
    /// it has: this is a metadata read, a resolve, a plan and however many files
    /// the check finds wrong, so the shell runs it off the frame thread.
    ///
    /// The work is [`crate::launch::repair_instance`]'s, because a repair *is* the
    /// half of a launch that puts files on disk -- the loader's own installer, the
    /// libraries, the client jar, the asset index and its objects -- with the one
    /// difference that a file already present is hashed against the digest its
    /// metadata publishes instead of being trusted by its size. What this method
    /// owns is the way out: the metadata store and the wire, built the way every
    /// other install builds them.
    ///
    /// The answer is the sentence the installation tab draws under its button, and
    /// a repair that could not finish answers with one too, so a reader is told
    /// which file failed rather than only that something did.
    pub fn repair_instance(&self, instance_id: &str) -> Result<String, String> {
        let Some(engine) = &self.engine else {
            return Err(not_implemented("Repairing an instance"));
        };
        let Some(paths) = &self.paths else {
            return Err(not_implemented("Repairing an instance"));
        };
        // The engine's own pool, not a second one built from the same numbers:
        // [`Store::install_project`]'s rule, and the reason there is a fetch here
        // at all.
        let wire = Wire::over(paths.meta_dir(), engine.fetch());
        let mut store = crate::meta::PublisherMeta::for_instance(&wire, paths, instance_id);
        let mut quiet = |_line: String| {};
        // The launch's own Java, read the way a launch reads it: a repair runs the
        // loader's installer, and the processors have to run on the same runtime
        // the next launch will use rather than on whichever one a second rule
        // happens to find (G131).
        let defaults = self.launch_defaults();
        crate::launch::repair_instance(paths, instance_id, &defaults, &mut store, &wire, &mut quiet)
    }

    /// Lay the pack this instance came from over it again.
    ///
    /// The reference's *Re-install modpack* (`installation-settings.vue`'s
    /// `reinstallModpack`, which is `update_repair_modrinth` for a pack the service
    /// knows and a file picker for one imported from disk): the version the link
    /// names is fetched again -- through the launcher's own cache, where a pack
    /// archive is filed by digest, so a second press costs a read rather than 300
    /// MB -- and laid over the instance by [`crate::install::reapply_pack`].
    ///
    /// **Blocking**, for a repair's reason and a longer one: a cached archive
    /// read, an unpack, and however many files the pack lists.
    ///
    /// The sentence is [`crate::install::PackReapply::summary`]'s, and it counts
    /// files rather than promising the reference's own reset of the instance's
    /// content, because this launcher's install path deletes nothing: what the
    /// reader added to the instance stays where it is.
    pub fn reinstall_modpack(&self, instance_id: &str) -> Result<String, String> {
        let (paths, engine) = self.pack_engine("Re-installing a modpack")?;
        let link = self.instance_link(instance_id)?.ok_or_else(|| {
            "this instance is not linked to a modpack, so there is nothing to re-install"
                .to_string()
        })?;
        let (project, version) = self.linked_version(&link, "Re-installing a modpack")?;
        let wire = Wire::over(paths.meta_dir(), engine.fetch());
        let archive =
            install::fetch_pack_archive(&wire, &paths.meta_dir().join("packs"), &version)?;
        let instance = Instance::open(&self.instance_dir(instance_id))
            .map_err(|error| format!("cannot open instance '{instance_id}': {error}"))?;
        let reapply = install::reapply_pack(&wire, &instance, &archive, &mut |_| {})?;
        Ok(reapply.summary(&project.title, version.version_number.as_str()))
    }

    /// The versions of the linked pack this instance could take instead.
    ///
    /// The filter is the reference's own: its `ContentUpdaterModal` is given the
    /// instance's `current-game-version` and `current-loader` and lists only the
    /// versions that name them. The pair is read from the instance's own
    /// `mmc-pack.json` rather than from anything a page holds, because the page's
    /// form may be holding edits that have not been saved.
    ///
    /// An instance with no link answers with the empty list rather than a refusal:
    /// the list is drawn inside the card that only a linked instance has, so there
    /// is no page that can ask this question and then have to draw the refusal.
    pub fn pack_versions(&self, instance_id: &str) -> Result<Vec<PackVersion>, String> {
        let Some(link) = self.instance_link(instance_id)? else {
            return Ok(Vec::new());
        };
        let (_, engine) = self.pack_engine("The versions of a modpack")?;
        let installation = self.instance_installation(instance_id)?;
        let cancel = Cancel::new();
        let backoff = Backoff::default();
        let versions = engine
            .api()
            .versions(&link.project_id, &cancel, &backoff)
            .map_err(|error| error.to_string())?;
        Ok(install::versions_for(
            &versions,
            installation.game_version.as_str(),
            installation.platform.modrinth_name(),
        )
        .into_iter()
        .map(|version| PackVersion {
            id: version.id.clone(),
            number: version_number(version),
        })
        .collect())
    }

    /// Lay another version of the linked pack over the instance, and remember it.
    ///
    /// One version of one project, so the link is rewritten in the same call the
    /// files land: an instance that had taken a new version and still pointed at
    /// the old one would draw the old number over files the new one wrote.
    ///
    /// **Blocking**, for [`Store::reinstall_modpack`]'s reason.
    pub fn change_pack_version(
        &self,
        instance_id: &str,
        version_id: &str,
    ) -> Result<String, String> {
        let (paths, engine) = self.pack_engine("Changing a modpack's version")?;
        let link = self.instance_link(instance_id)?.ok_or_else(|| {
            "this instance is not linked to a modpack, so there is no other version to take"
                .to_string()
        })?;
        // A link with the version the reader pressed, read through the same helper
        // the re-install uses: the project document names the pack and the version
        // is where the archive comes from, and a version the author has deleted is
        // a refusal with a sentence rather than a silent nothing.
        let wanted = InstanceLink {
            project_id: link.project_id.clone(),
            version_id: version_id.to_string(),
        };
        let (project, version) = self.linked_version(&wanted, "Changing a modpack's version")?;
        let wire = Wire::over(paths.meta_dir(), engine.fetch());
        let archive =
            install::fetch_pack_archive(&wire, &paths.meta_dir().join("packs"), &version)?;
        let instance = Instance::open(&self.instance_dir(instance_id))
            .map_err(|error| format!("cannot open instance '{instance_id}': {error}"))?;
        let reapply = install::reapply_pack(&wire, &instance, &archive, &mut |_| {})?;
        let summary = reapply.summary(&project.title, version.version_number.as_str());
        // The link follows the files, and a link that would not write is not an
        // install that failed -- the files are on disk and the instance is playable
        // -- so the reason travels in the sentence, the way [`Store::install_pack`]
        // does it.
        match self.save_instance_link(instance_id, &wanted) {
            Ok(()) => Ok(summary),
            Err(error) => Ok(format!("{summary} (the link could not be updated: {error})")),
        }
    }

    /// The paths and the way out, for the operations that need both.
    ///
    /// One place for the two refusals, because the pack actions all ask the same
    /// question first and a store with no launcher behind it has to answer each of
    /// them with its own word rather than with an empty answer.
    fn pack_engine(&self, what: &str) -> Result<(&PalantirPaths, &Engine), String> {
        let Some(engine) = &self.engine else {
            return Err(not_implemented(what));
        };
        let Some(paths) = &self.paths else {
            return Err(not_implemented(what));
        };
        Ok((paths, engine))
    }

    /// The project and the version a link names, read from the service.
    ///
    /// One helper for the two actions that need both, and the reason each of them
    /// is a request rather than a guess: the project document is the name the card
    /// draws beside the sentence, and the version is where the archive comes from.
    /// A version the author has deleted is a refusal with a sentence -- the pack is
    /// still installed, but there is nothing left to lay over it.
    fn linked_version(
        &self,
        link: &InstanceLink,
        what: &str,
    ) -> Result<(ModrinthProject, ModrinthProjectVersion), String> {
        let (_, engine) = self.pack_engine(what)?;
        let cancel = Cancel::new();
        let backoff = Backoff::default();
        let api = engine.api();
        let project = api
            .project(&link.project_id, &cancel, &backoff)
            .map_err(|error| error.to_string())?;
        let versions = api
            .versions(&link.project_id, &cancel, &backoff)
            .map_err(|error| error.to_string())?;
        let version = versions
            .into_iter()
            .find(|version| version.id == link.version_id)
            .ok_or_else(|| {
                format!(
                    "{} no longer publishes the version this instance came from",
                    project.title
                )
            })?;
        Ok((project, version))
    }
}

/// One version of a linked pack, as the *Change version* list draws it.
///
/// Two fields rather than the API's whole `ModrinthProjectVersion`: the list needs
/// something to press (`id`) and something to read (`number`), and a page holding
/// a service type is a page that could ask the service itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackVersion {
    /// The version's id, which is what a press installs.
    pub id: String,
    /// The number the author gave it (`1.6.1`), or its name when there is none.
    pub number: String,
}

/// What a version is called on a card: its number, or its name when the author
/// gave it none.
fn version_number(version: &ModrinthProjectVersion) -> String {
    if version.version_number.trim().is_empty() {
        version.name.clone()
    } else {
        version.version_number.clone()
    }
}

/// The name a project page draws under the title, out of a team list.
///
/// Measured against the live service rather than assumed, because the first
/// guess was wrong in a way a fixture would have agreed with: **there is no
/// `Owner` role in this API.** Sodium's team comes back as three members whose
/// roles are `Maintainer`, `Project Lead` and `Maintainer`, and every one of
/// them carries `ordering: 0`, so neither the role nor the order spells out the
/// project's owner. What the vocabulary does have is the rank Modrinth assigns
/// to the account that owns the project, so that is what is credited: the
/// `Project Lead` when the team has one, then the first member -- the list is in
/// the service's own order, and its first entry is the one Modrinth's own page
/// puts at the top. An empty team is the empty name rather than a place held in
/// the layout.
fn author_of(members: &[ModrinthMember]) -> &str {
    members
        .iter()
        .find(|member| member.role == "Project Lead")
        .or_else(|| members.first())
        .map(|member| member.user.username.as_str())
        .unwrap_or_default()
}

/// One version Mojang publishes, as a picker needs it.
///
/// The manifest says four things a version can be -- `release`, `snapshot`,
/// `old_beta`, `old_alpha` -- and the reference's picker draws one of them
/// differently: its snapshots toggle keeps the releases and hides the rest. So
/// the type travels as that one bit plus the id, which is what a picker draws and
/// what the created instance is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameVersion {
    /// The id a user types and a folder is named after: `1.21.4`, `25w02a`.
    pub id: String,
    /// Whether this is a full release rather than a snapshot or an old build.
    pub release: bool,
}

/// One build of a mod loader, as the picker draws it.
///
/// The store's own copy of `palantir_net::engine::Build`, because the shell paints
/// from this module and a page has no business knowing which crate a service is
/// reached through -- the same reason `GameVersion` is not Mojang's own type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoaderBuild {
    /// The build's version string: `0.19.5`, `21.4.5`, `54.1.0`.
    pub version: String,
    /// Whether the loader itself calls this build stable. What the picker opens
    /// on is the newest one of these, not the newest of all.
    pub stable: bool,
}

impl From<LoaderBuildSource> for LoaderBuild {
    fn from(build: LoaderBuildSource) -> LoaderBuild {
        LoaderBuild { version: build.version, stable: build.stable }
    }
}

/// Mojang's whole version list, with the answer a picker opens on.
///
/// Two fields rather than one because the picker needs both before it can draw
/// anything: the list to choose from, and the version to be on when the user has
/// chosen nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionList {
    /// Mojang's own `latest.release`, which the picker opens on.
    pub latest_release: String,
    /// Every version, in the order published: newest first.
    pub versions: Vec<GameVersion>,
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

/// The sentence for a surface this launcher does not have *on purpose*.
///
/// The distinction [`not_implemented`] cannot make, and the one a reader deserves:
/// "not yet" promises a slice, and four of the reference's surfaces are not owed
/// one. Modrinth Hosting and its billing, an instance's Share tab, the skin store
/// and the signed-in half of the panel's friends list are all Modrinth *account*
/// services, and this launcher does not hold a Modrinth credential -- a decision
/// rather than a gap (G118; the measurements behind it are G105, G109 and G111).
/// Everything else this launcher reads from Modrinth is the published, anonymous
/// API: Discover's search, project documents, version lists and the installs that
/// use them need no account and stay.
pub fn needs_account(what: &str) -> String {
    format!("{what} needs a Modrinth account, which this launcher does not have.")
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

/// How many lines of an instance's newest log the Logs tab draws.
///
/// The same 500 the interface asked for when it read the tail on every frame.
pub const LOG_TAIL_LINES: usize = 500;

/// One tab's own listing, as the shell's worker hands it back to the page.
///
/// **This is the answer to "read the page's data", and it exists because those
/// reads used to happen where the page was drawn.** A view is called once per
/// frame, so a view that walked a directory made the frame cost a directory walk:
/// `crate::scale` measured the Files tab at 2,304 ms of frame at five thousand
/// entries, and the Content tab at 46.8 ms -- 18.5 ms of it the read and 4.9 ms
/// the per-row clock name -- after that read was made linear. The read is a load
/// now, made once when a tab is entered and off the frame thread, so the view
/// only ever draws what is already here.
///
/// Five variants rather than a `Vec<String>`: a row is not a name. The Content
/// tab draws an enabled state and a toggle per row, the Files tab a glyph and a
/// size, the Worlds tab whether a world has been played, the Screenshots tab a
/// name, and the Logs tab one block of monospace. Flattening those into strings
/// would put each tab's own decision back in the view, which is the thing being
/// taken out of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Listing {
    /// The instance's `mods/`, as [`content`] reads it.
    Content(Vec<ModEntry>),
    /// One level of the instance's directory, as [`files`] reads it.
    Files(Vec<Entry>),
    /// The instance's `saves/`, as [`worlds`] reads it.
    Worlds(Vec<World>),
    /// The names in `screenshots/`, as [`screenshots`] reads them.
    Screenshots(Vec<String>),
    /// The last [`LOG_TAIL_LINES`] lines of the newest log, or nothing.
    Log(String),
}

impl Listing {
    /// Whether the tab that asked for this has nothing to draw.
    ///
    /// Asked here rather than by the view, because the view's four states are
    /// [`crate::page::Load`]'s: an answer with no rows in it is `Load::Empty`,
    /// which draws the reference's own "there is nothing here" card, and a page
    /// that had to look inside its data to tell the two apart would eventually
    /// draw the wrong one.
    pub fn is_empty(&self) -> bool {
        match self {
            Listing::Content(rows) => rows.is_empty(),
            Listing::Files(rows) => rows.is_empty(),
            Listing::Worlds(rows) => rows.is_empty(),
            Listing::Screenshots(names) => names.is_empty(),
            Listing::Log(tail) => tail.is_empty(),
        }
    }
}

/// Read one tab's own listing.
///
/// Takes a directory rather than an instance id because that is what the readers
/// below it take, and because the shell is the one that knows how an id becomes a
/// directory ([`Store::instance_dir`]). The `Share` tab answers with a sentence:
/// it is a service's page with no local listing to read, and its card says so
/// rather than pretending to be empty.
pub fn listing(instance_dir: &Path, tab: &InstanceTab) -> Result<Listing, String> {
    Ok(match tab {
        InstanceTab::Content | InstanceTab::ContentFilter(_) => {
            Listing::Content(content(instance_dir))
        }
        InstanceTab::Files => Listing::Files(files(instance_dir)),
        InstanceTab::Worlds => Listing::Worlds(worlds(instance_dir)),
        InstanceTab::Screenshots => Listing::Screenshots(screenshots(instance_dir)),
        // A missing log is an empty one rather than a failure: an instance that
        // has never been launched has no log, which is the same thing to the tab
        // that draws it.
        InstanceTab::Logs => {
            Listing::Log(log_tail(instance_dir, LOG_TAIL_LINES).unwrap_or_default())
        }
        InstanceTab::Share => return Err(not_implemented("Sharing an instance")),
    })
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
///
/// **The type and the size come from the scan, not from a fresh lookup.**
/// `path.is_dir()` and `fs::metadata(&path)` are each a lookup of a name the
/// directory read has just returned, and in a folder of five thousand that
/// lookup is not free: this function measured **2,311 ms** for 5,000 entries on
/// this machine, against 12 ms for `mods::list_mods` at the same count -- which
/// is the same walk reading `DirEntry::file_type` instead. The cost per entry
/// *grew* with the folder (0.147 ms at a hundred, 0.462 ms at five thousand),
/// which is worse than linear -- 50 times the entries cost 157 times the time --
/// and is what a lookup that has to search the directory index looks like.
/// `DirEntry` already carries what the scan returned, so the same list costs
/// milliseconds and is linear again; `crate::scale` prints both numbers.
///
/// A symlink is still *followed*, which is the one thing the cheap calls change:
/// `file_type` reports the link itself, and the name-based calls this replaced
/// resolved it. An instance whose `mods` folder is a junction is a real layout on
/// Windows, so the fallback for a link keeps the old behaviour and pays the old
/// price for it -- for a link, not for every file.
pub fn files(directory: &Path) -> Vec<Entry> {
    let mut entries = Vec::new();
    let Ok(read) = std::fs::read_dir(directory) else {
        return entries;
    };
    for entry in read.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let (directory, bytes) = match entry.file_type() {
            Ok(kind) if !kind.is_symlink() => (
                kind.is_dir(),
                if kind.is_dir() {
                    0
                } else {
                    entry.metadata().map(|meta| meta.len()).unwrap_or(0)
                },
            ),
            _ => {
                let directory = path.is_dir();
                let bytes = if directory {
                    0
                } else {
                    std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0)
                };
                (directory, bytes)
            }
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
///
/// The second file is this launcher's own account of the run
/// (`logs/launcher.log`, written by `launch::LaunchLogFile`), read when the game
/// never wrote one: a launch that failed before Minecraft started is exactly the
/// case where a reader wants the log most, and the game's file is the one thing a
/// failed launch does not leave behind.
pub fn log_tail(instance_dir: &Path, lines: usize) -> Option<String> {
    let logs = instance_dir.join("logs");
    let text = std::fs::read_to_string(logs.join("latest.log"))
        .or_else(|_| std::fs::read_to_string(logs.join("launcher.log")))
        .ok()?;
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
    use palantir_net::modrinth::{project_members_url, project_url, version_url, NEWS_URL};
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

    /// A version manifest with one of each kind of thing Mojang publishes, in
    /// Mojang's own shape and its own order: newest first.
    const MIXED_MANIFEST_BODY: &str = r#"{
        "latest": { "release": "1.21.4", "snapshot": "25w02a" },
        "versions": [
            { "id": "25w02a", "type": "snapshot", "url": "https://piston.invalid/25w02a.json",
              "time": "2025-01-08T10:00:00+00:00", "releaseTime": "2025-01-08T10:00:00+00:00",
              "sha1": "0000000000000000000000000000000000000000", "complianceLevel": 1 },
            { "id": "1.21.4", "type": "release", "url": "https://piston.invalid/1.21.4.json",
              "time": "2024-12-03T10:00:00+00:00", "releaseTime": "2024-12-03T10:00:00+00:00",
              "sha1": "0000000000000000000000000000000000000000", "complianceLevel": 1 },
            { "id": "b1.7.3", "type": "old_beta", "url": "https://piston.invalid/b1.7.3.json",
              "time": "2011-07-01T10:00:00+00:00", "releaseTime": "2011-07-01T10:00:00+00:00",
              "sha1": "0000000000000000000000000000000000000000", "complianceLevel": 1 }
        ]
    }"#;

    /// One `GET /v2/project/{id}` body, in the API's own shape. A two-hash raw
    /// string, because a markdown body's heading opens with `"#`.
    const PROJECT_BODY: &str = r##"{
        "id": "AANobbMI", "slug": "sodium", "project_type": "mod",
        "title": "Sodium", "description": "Modern rendering engine",
        "body": "# Sodium\n\nFaster.\n", "downloads": 41000000, "followers": 9000,
        "game_versions": ["1.21.4"], "loaders": ["fabric"],
        "gallery": [{"url": "https://cdn.modrinth.com/shot.png", "title": "In the nether"}]
    }"##;

    /// One `GET /v2/project/{id}/members` body, with the roles the live service
    /// actually publishes: a maintainer first, then the project's lead. Every
    /// member's `ordering` is zero, which is why the role decides who is
    /// credited -- see `author_of`.
    const MEMBERS_BODY: &str = r#"[
        {"role": "Maintainer", "ordering": 0, "user": {"username": "IMS"}},
        {"role": "Project Lead", "ordering": 0, "user": {"username": "jellysquid3"}},
        {"role": "Maintainer", "ordering": 0, "user": {"username": "douira"}}
    ]"#;

    /// One `GET /v2/project/{id}/version` body.
    const MODRINTH_VERSIONS_BODY: &str = r#"[{
        "id": "abc123", "project_id": "AANobbMI", "name": "Sodium 0.6.5",
        "version_number": "mc1.21.4-0.6.5", "version_type": "release", "downloads": 1234,
        "changelog": "Fixed a thing.", "game_versions": ["1.21.4"], "loaders": ["fabric"],
        "files": [], "dependencies": []
    }]"#;

    #[test]
    fn the_project_page_reads_three_documents_and_credits_the_team() {
        // Modrinth splits a project three ways -- the document, the team, the
        // versions -- and the page is one `Load`, so the store is where the three
        // become one answer. The author is the part that is not in the project
        // document at all.
        let fetch = Arc::new(MapFetch::new());
        fetch.set_route(&project_url("AANobbMI"), Route::text(PROJECT_BODY));
        fetch.set_route(&project_members_url("AANobbMI"), Route::text(MEMBERS_BODY));
        fetch.set_route(&version_url("AANobbMI"), Route::text(MODRINTH_VERSIONS_BODY));
        let store = store_over("project", fetch.clone());

        let project = store.project("AANobbMI").expect("the project");
        assert_eq!(project.id, "AANobbMI");
        assert_eq!(project.title, "Sodium");
        assert_eq!(
            project.author, "jellysquid3",
            "the Project Lead, not the first member, out of the same list"
        );
        assert_eq!(project.summary, "Modern rendering engine");
        assert_eq!(project.loaders, vec!["fabric"]);
        assert_eq!(project.gallery, vec!["In the nether"]);
        assert_eq!(project.versions.len(), 1);
        assert_eq!(project.versions[0].number, "mc1.21.4-0.6.5");
        assert_eq!(fetch.count(), 3, "one request per document, and no more");

        // Asked again, the three are answered from the cache: a page revisited
        // costs nothing at all.
        let again = store.project("AANobbMI").expect("the project, again");
        assert_eq!(again, project);
        assert_eq!(fetch.count(), 3);
    }

    #[test]
    fn a_team_that_cannot_be_read_does_not_fail_the_project_page() {
        // The author is a caption under the title. A service that will not name
        // the team leaves the caption empty rather than leaving the page blank --
        // and the two reads around it are still asked for.
        let fetch = Arc::new(MapFetch::new());
        fetch.set_route(&project_url("AANobbMI"), Route::text(PROJECT_BODY));
        // No route for the members: `MapFetch` fails a URL nobody scripted.
        fetch.set_route(&version_url("AANobbMI"), Route::text(MODRINTH_VERSIONS_BODY));
        let store = store_over("project-no-team", fetch.clone());

        let project = store.project("AANobbMI").expect("the project");
        assert_eq!(project.title, "Sodium");
        assert_eq!(project.author, "");
        assert_eq!(project.versions.len(), 1);
        assert_eq!(fetch.count(), 3, "the members were asked for and failed");
    }

    /// One `GET /v2/project/{id}/version` body with a file in it, in the API's
    /// own shape: the newest matching version first, and one that is not for this
    /// instance after it.
    ///
    /// The digest is a parameter because it is what decides whether a second
    /// install transfers anything: the queue skips a file that is already there
    /// and matches the `sha1` the API published, and fetches one it cannot check
    /// (which is why the caller still measures the length).
    fn versions_with_files(sha1: &str) -> String {
        format!(
            r#"[{{
        "id": "new", "project_id": "AANobbMI", "name": "Sodium 0.6.5",
        "version_number": "mc1.21.4-0.6.5", "version_type": "beta", "downloads": 10,
        "changelog": "", "game_versions": ["1.21.4"], "loaders": ["fabric"],
        "files": [{{"url": "https://cdn.modrinth.com/data/sodium.jar",
                   "filename": "sodium-fabric-0.6.5.jar", "primary": true, "size": 9,
                   "hashes": {{"sha1": "{sha1}"}}}}], "dependencies": []
    }}, {{
        "id": "old", "project_id": "AANobbMI", "name": "Sodium 0.5.0",
        "version_number": "mc1.20.1-0.5.0", "version_type": "release", "downloads": 900,
        "changelog": "", "game_versions": ["1.20.1"], "loaders": ["fabric"],
        "files": [{{"url": "https://cdn.modrinth.com/data/old.jar",
                   "filename": "sodium-fabric-0.5.0.jar", "primary": true, "size": 10,
                   "hashes": {{}}}}], "dependencies": []
    }}]"#
        )
    }

    /// A store with one Fabric instance in it, whose only way out is the engine.
    fn store_with_instance(name: &str, fetch: Arc<MapFetch>) -> Store {
        let root = scratch(name);
        let paths = PalantirPaths::at(&root);
        std::fs::create_dir_all(paths.instances_dir()).expect("an instances directory");
        crate::instances::create(
            &paths,
            &NewInstance {
                name: "atm10".to_string(),
                loader: LoaderKind::Fabric,
                game: "1.21.4".to_string(),
                loader_build: Some("0.16.9".to_string()),
                icon_key: None,
                icon_source: None,
                max_mem_mb: None,
                java_path: None,
            },
        )
        .expect("a fabric instance");
        Store::load(&paths).with_engine(Engine::over(paths.meta_dir(), fetch))
    }

    #[test]
    fn installing_a_project_puts_the_version_that_matches_into_the_right_folder() {
        // The whole path of one press: the project document says what the thing
        // is, its version list says what fits the instance, and the file lands in
        // the folder that kind belongs in -- over the engine's own queue, so the
        // counts here are of requests that really went through it.
        let body = b"a mod jar";
        let fetch = Arc::new(MapFetch::new());
        fetch.set_route(&project_url("AANobbMI"), Route::text(PROJECT_BODY));
        fetch.set_route(
            &version_url("AANobbMI"),
            Route::text(&versions_with_files(&install::sha1_hex(body))),
        );
        fetch.set_route("https://cdn.modrinth.com/data/sodium.jar", Route::body(body.to_vec()));
        let store = store_with_instance("install-project", fetch.clone());

        let installed = store.install_project("AANobbMI", "atm10").expect("an install");
        assert_eq!(installed.project, "Sodium");
        assert_eq!(
            installed.version, "mc1.21.4-0.6.5",
            "the newest version for this instance, not the newest release"
        );
        assert_eq!(installed.file.filename, "sodium-fabric-0.6.5.jar");
        assert_eq!(installed.summary(), "Installed Sodium mc1.21.4-0.6.5 into atm10");
        let path = store.instance_dir("atm10").join("mods").join("sodium-fabric-0.6.5.jar");
        assert_eq!(installed.file.path, path);
        assert_eq!(std::fs::read(&path).expect("the file"), b"a mod jar");
        assert!(installed.file.verified, "the published sha1 was checked");
        assert_eq!(installed.file.bytes, body.len() as u64);
        assert_eq!(fetch.count(), 3, "the document, the version list and the file");

        // Pressed again, nothing is transferred: the file is already there and
        // matched the digest the API published, and the two documents are cached.
        let again = store.install_project("AANobbMI", "atm10").expect("the same install");
        assert_eq!(again.version, installed.version);
        assert_eq!(fetch.count(), 3, "a second press costs nothing");
    }

    #[test]
    fn installing_says_what_it_cannot_do_instead_of_writing_somewhere_invented() {
        let fetch = Arc::new(MapFetch::new());
        fetch.set_route(&project_url("AANobbMI"), Route::text(PROJECT_BODY));
        fetch.set_route(&version_url("AANobbMI"), Route::text("[]"));
        let store = store_with_instance("install-refusals", fetch.clone());

        // No such instance: named rather than guessed at.
        let reason = store.install_project("AANobbMI", "gone").expect_err("no instance");
        assert!(reason.contains("gone"), "{reason}");
        // A version list with nothing for this instance: the sentence names what
        // the instance *is*, which is what the reader can act on.
        let bare = Arc::new(MapFetch::new());
        bare.set_route(&project_url("AANobbMI"), Route::text(PROJECT_BODY));
        bare.set_route(&version_url("AANobbMI"), Route::text("[]"));
        let store = store_with_instance("install-no-version", bare.clone());
        let reason = store.install_project("AANobbMI", "atm10").expect_err("no version");
        assert!(reason.contains("Fabric 1.21.4"), "the target is named: {reason}");
        assert_eq!(bare.count(), 2, "and no file was asked for");
        // A store with no engine, and one with no launcher behind it: the two
        // ways this cannot be attempted at all.
        let reason = Store::default().install_project("AANobbMI", "atm10").expect_err("no engine");
        assert!(reason.contains("is not implemented yet"), "{reason}");
        assert!(store.install_project("AANobbMI", "gone").is_err());
    }

    #[test]
    fn a_pack_is_refused_by_name_because_it_becomes_an_instance_of_its_own() {
        // The one project type whose install is not a folder, and the one command
        // that cannot be pointed at an instance at all: `install_pack` is where it
        // goes instead. The refusal names the kind rather than saying "there is no
        // folder for that", because a pack *is* installable.
        let fetch = Arc::new(MapFetch::new());
        fetch.set_route(&project_url("pack"), Route::text(PACK_PROJECT_BODY));
        fetch.set_route(&version_url("pack"), Route::text(&versions_with_files("0")));
        let store = store_with_instance("install-pack", fetch.clone());
        let reason = store.install_project("pack", "atm10").expect_err("a pack");
        assert!(reason.contains("modpack"), "{reason}");
        assert!(reason.contains("instance of its own"), "{reason}");
        assert_eq!(fetch.count(), 2, "and nothing was downloaded");
    }

    /// A news feed body, in the live feed's own shape: one envelope, five fields
    /// per article, newest first.
    const NEWS_FEED_BODY: &str = r#"{
        "articles": [
            {"title": "Sync settings across instances",
             "summary": "Keep game options the same across your instances.",
             "thumbnail": "https://modrinth.com/news/article/sync-settings/thumbnail.webp",
             "date": "2026-09-07T19:00:00.000Z",
             "link": "https://modrinth.com/news/article/sync-settings"},
            {"title": "An older one", "date": "2026-08-01T10:00:00.000Z",
             "link": "https://modrinth.com/news/article/older"}
        ]
    }"#;

    #[test]
    fn the_news_feed_is_read_once_and_then_answered_from_the_engine_s_own_cache() {
        let fetch = Arc::new(MapFetch::new());
        fetch.set_route(NEWS_URL, Route::text(NEWS_FEED_BODY));
        let store = store_over("news", fetch.clone());

        let news = store.news().expect("the feed");
        assert_eq!(news.len(), 2, "the whole feed; the panel takes four of it");
        assert_eq!(news[0].title, "Sync settings across instances");
        assert_eq!(news[0].date_label(), "September 7, 2026");
        assert_eq!(news[1].summary, "", "an article with no summary is still an article");
        assert_eq!(fetch.count(), 1);
        // The feed is a document, so the second read is the engine's own cache:
        // the panel is drawn on every frame and must not ask again.
        assert_eq!(store.news().expect("again").len(), 2);
        assert_eq!(fetch.count(), 1);
        // A store with no engine says so in the same sentence every unbuilt thing
        // does, and the panel draws no section for either.
        let reason = Store::default().news().expect_err("no engine");
        assert!(reason.contains("is not implemented yet"), "{reason}");
    }

    /// One `GET /v2/project/{id}` body for a pack: the same shape as
    /// [`PROJECT_BODY`] with the type that makes it one, under a title that is not
    /// Sodium's -- the instance a pack makes is named after it.
    const PACK_PROJECT_BODY: &str = r##"{
        "id": "cobblemon", "slug": "cobblemon", "project_type": "modpack",
        "title": "Cobblemon", "description": "Pokemon, in Minecraft",
        "body": "", "downloads": 41000000, "followers": 9000,
        "game_versions": ["1.21.4"], "loaders": ["fabric"], "gallery": []
    }"##;

    /// One `GET /v2/project/{id}/version` body for a pack: the archive itself is
    /// the version's primary file, which is the whole of what this rule reads.
    fn pack_versions_body(sha1: &str, size: usize) -> String {
        format!(
            r#"[{{
        "id": "pack-1", "project_id": "cobblemon", "name": "Cobblemon 1.6.1",
        "version_number": "1.6.1", "version_type": "release", "downloads": 10,
        "changelog": "", "game_versions": ["1.21.4"], "loaders": ["fabric"],
        "files": [{{"url": "https://cdn.modrinth.com/data/cobblemon.mrpack",
                   "filename": "Cobblemon-1.6.1.mrpack", "primary": true, "size": {size},
                   "hashes": {{"sha1": "{sha1}"}}}}], "dependencies": []
    }}]"#
        )
    }

    /// A `.mrpack` in memory: an index that names a Minecraft version, a loader and
    /// one remote file, plus one overrides file. The smallest thing that is a pack.
    fn pack_archive(index: &serde_json::Value) -> Vec<u8> {
        use std::io::Write as _;
        let buf = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(buf);
        let options =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        writer.start_file("modrinth.index.json", options).unwrap();
        writer.write_all(index.to_string().as_bytes()).unwrap();
        writer.start_file("overrides/config/cobblemon.json", options).unwrap();
        writer.write_all(b"{}").unwrap();
        writer.finish().unwrap().into_inner()
    }

    /// A store over a launcher with no instances in it: what installing a pack
    /// needs, and the thing that makes the pack's own instance the first one.
    fn store_without_instances(name: &str, fetch: Arc<MapFetch>) -> Store {
        let root = scratch(name);
        let paths = PalantirPaths::at(&root);
        std::fs::create_dir_all(paths.instances_dir()).expect("an instances directory");
        Store::load(&paths).with_engine(Engine::over(paths.meta_dir(), fetch))
    }

    #[test]
    fn installing_a_modpack_makes_an_instance_of_its_own_and_fetches_what_it_lists() {
        // The whole path of a pack press: the project document says it is a pack,
        // the version list gives the archive, the archive's index names the game
        // version and the loader the *instance* is made of, and the files it lists
        // land in that instance -- including the mods it names.
        let mod_bytes = b"a pack mod";
        let index = serde_json::json!({
            "formatVersion": 1,
            "game": "minecraft",
            "versionId": "1.6.1",
            "name": "Cobblemon",
            "files": [{
                "path": "mods/cobblemon.jar",
                "hashes": { "sha1": install::sha1_hex(mod_bytes) },
                "downloads": ["https://cdn.modrinth.com/data/cobblemon.jar"],
                "fileSize": mod_bytes.len(),
            }],
            "dependencies": { "minecraft": "1.21.4", "fabric-loader": "0.16.9" },
        });
        let archive = pack_archive(&index);
        let fetch = Arc::new(MapFetch::new());
        fetch.set_route(&project_url("cobblemon"), Route::text(PACK_PROJECT_BODY));
        fetch.set_route(
            &version_url("cobblemon"),
            Route::text(&pack_versions_body(&install::sha1_hex(&archive), archive.len())),
        );
        fetch.set_route(
            "https://cdn.modrinth.com/data/cobblemon.mrpack",
            Route::body(archive.clone()),
        );
        fetch.set_route(
            "https://cdn.modrinth.com/data/cobblemon.jar",
            Route::body(mod_bytes.to_vec()),
        );
        let store = store_without_instances("install-modpack", fetch.clone());

        let Outcome::Pack { id, line } = store.install_pack("cobblemon").expect("a pack") else {
            panic!("a pack install is the shape that makes an instance")
        };
        assert_eq!(id, "Cobblemon", "named after the project the reader pressed");
        assert_eq!(line, "Installed Cobblemon as Cobblemon, 1 file");
        assert_eq!(fetch.count(), 4, "the document, the versions, the archive, one file");
        let root = store.instance_dir(&id);
        assert_eq!(
            std::fs::read(root.join("mods").join("cobblemon.jar")).expect("the pack mod"),
            mod_bytes
        );
        assert!(
            root.join("config").join("cobblemon.json").is_file(),
            "the overrides tree is written at the instance root, with `overrides/` stripped"
        );
        let pack = palantir_core::util::read_text(&root.join("mmc-pack.json")).expect("a profile");
        assert!(pack.contains("1.21.4"), "the instance is the pack's game: {pack}");
        assert!(pack.contains("net.fabricmc.fabric-loader"), "and its loader: {pack}");

        // And the instance remembers which project and version it came from,
        // which is the whole of what the installation tab's modpack card draws.
        assert_eq!(
            store.instance_link(&id).expect("a link"),
            Some(InstanceLink {
                project_id: "cobblemon".to_string(),
                version_id: "pack-1".to_string(),
            })
        );
        assert_eq!(fetch.count(), 4, "and a link read is a file read, not a request");
    }

    #[test]
    fn a_link_is_a_file_of_its_own_and_one_that_cannot_be_read_is_a_refusal() {
        // The link is the one thing this launcher knows about an instance that
        // Prism's format has no field for, so it lives in a file of its own beside
        // `mmc-pack.json` -- with the reference's own `type` inside it, because the
        // ids alone do not say what they are ids *of*.
        //
        // Over a real instances directory rather than `Store::default()`: a default
        // store's `instances_dir` is the empty path, which makes this instance a
        // *relative* `atm10` shared with every other test in the process -- and two
        // tests writing one link file is a race, not a fixture.
        let store = store_without_instances("link", Arc::new(MapFetch::new()));
        let dir = store.instance_dir("atm10");
        std::fs::create_dir_all(&dir).expect("an instance folder");
        assert_eq!(
            store.instance_link("atm10").expect("no link"),
            None,
            "an instance nobody linked is not an error"
        );

        let link = InstanceLink {
            project_id: "cobblemon".to_string(),
            version_id: "pack-1".to_string(),
        };
        store.save_instance_link("atm10", &link).expect("a write");
        let text = palantir_core::util::read_text(&dir.join(LINK_FILE)).expect("the file");
        assert!(text.contains("modrinth_modpack"), "the reference's own type: {text}");
        assert_eq!(store.instance_link("atm10").expect("a link"), Some(link.clone()));

        // A file this build cannot believe is an error rather than a `None`: one
        // says "this instance came from nothing", the other says "it came from
        // something, and this launcher cannot read the file that says what" -- and
        // only the second is a thing the reader could act on.
        std::fs::write(
            dir.join(LINK_FILE),
            r#"{"type":"shared_instance","project_id":"x","version_id":"y"}"#,
        )
        .expect("a hand-written link");
        let reason = store.instance_link("atm10").expect_err("a kind this launcher does not draw");
        assert!(reason.contains("shared_instance"), "the refusal names the kind: {reason}");
        std::fs::write(dir.join(LINK_FILE), "{ not json").expect("a broken link");
        let reason = store.instance_link("atm10").expect_err("junk");
        assert!(reason.contains(LINK_FILE), "the refusal names the file: {reason}");

        // Unlinking takes the link and leaves everything else: the instance, its
        // `mmc-pack.json` and every file the pack install put in it.
        store.save_instance_link("atm10", &link).expect("a write");
        assert!(store.clear_instance_link("atm10").expect("a clear"));
        assert_eq!(store.instance_link("atm10").expect("no link"), None);
        assert!(
            !store.clear_instance_link("atm10").expect("a second clear"),
            "unlinking an unlinked instance says there was nothing to take"
        );
        assert!(dir.join("mmc-pack.json").is_file() || dir.is_dir(), "the instance stays");
    }

    #[test]
    fn the_linked_modpack_is_named_from_the_service_and_not_from_the_file() {
        // Two ids is what the file holds, and two ids are not a card: the title,
        // the author and the version number are the service's, which is why a
        // rename shows here and why this read is a request rather than a parse.
        let fetch = Arc::new(MapFetch::new());
        fetch.set_route(&project_url("cobblemon"), Route::text(PACK_PROJECT_BODY));
        fetch.set_route(&project_members_url("cobblemon"), Route::text(MEMBERS_BODY));
        fetch.set_route(&version_url("cobblemon"), Route::text(&pack_versions_body("0", 1)));
        let store = store_without_instances("linked-modpack", fetch.clone());
        std::fs::create_dir_all(store.instance_dir("atm10")).expect("an instance folder");
        assert_eq!(store.linked_modpack("atm10").expect("no link"), None);
        assert_eq!(fetch.count(), 0, "an instance with no link asks nobody anything");

        let linked = |version_id: &str| InstanceLink {
            project_id: "cobblemon".to_string(),
            version_id: version_id.to_string(),
        };
        store.save_instance_link("atm10", &linked("pack-1")).expect("a write");
        let named = store.linked_modpack("atm10").expect("a read").expect("a link");
        assert_eq!(named.project_id, "cobblemon");
        assert_eq!(named.title, "Cobblemon", "the project document's own title");
        assert_eq!(named.author, "jellysquid3", "the team's Project Lead");
        assert_eq!(named.version, "1.6.1", "found in the version list by its id");
        assert_eq!(fetch.count(), 3, "the document, the team and the versions list");

        // A version its author has deleted is a caption this card cannot draw, and
        // the card is still a card -- the reference's own `version?.version_number`.
        store.save_instance_link("atm10", &linked("pack-gone")).expect("a write");
        let named = store.linked_modpack("atm10").expect("a read").expect("a link");
        assert_eq!(named.version, "", "no version number, and no failure");
        assert_eq!(named.title, "Cobblemon");

        // A launcher with no way out says so rather than drawing a card with a
        // title it made up.
        let bare = Store::load(&PalantirPaths::at(scratch("linked-no-engine")));
        std::fs::create_dir_all(bare.instance_dir("atm10")).expect("an instance folder");
        bare.save_instance_link("atm10", &linked("pack-1")).expect("a write");
        assert_eq!(
            bare.linked_modpack("atm10").expect_err("no engine"),
            not_implemented("The project this instance came from")
        );
    }

    #[test]
    fn the_version_picker_asks_mojang_once_for_the_list_and_what_is_current() {
        // The creation dialog has to offer every version Mojang publishes and open
        // on the current one, and it has to do it in one request: two reads are two
        // chances to be told about a list and about "current" at different moments,
        // and then the picker opens on a version that is in neither.
        let fetch = Arc::new(MapFetch::new());
        fetch.set_route(PISTON_MANIFEST_URL, Route::text(MIXED_MANIFEST_BODY));
        let store = store_over("versions", fetch.clone());
        let list = store.versions().expect("a version list");
        assert_eq!(
            list.latest_release, "1.21.4",
            "Mojang's own `latest.release`, not a re-derived one"
        );
        assert_eq!(
            list.versions.iter().map(|version| version.id.as_str()).collect::<Vec<_>>(),
            vec!["25w02a", "1.21.4", "b1.7.3"],
            "the manifest's own order, newest first"
        );
        assert!(!list.versions[0].release, "a snapshot is not a release");
        assert!(list.versions[1].release);
        assert!(!list.versions[2].release, "an old beta is not a release either");
        assert_eq!(fetch.count(), 1);
        // And the create flow's own fallback reads the same document, which is
        // cached: a second request here would be the second answer this test
        // exists to prevent.
        assert_eq!(store.current_release().expect("a release"), "1.21.4");
        assert_eq!(fetch.count(), 1);
    }

    #[test]
    fn a_version_list_with_nothing_behind_it_says_what_is_missing() {
        // The arm a store with no engine takes, which is the sentence the picker
        // shows in place of a list.
        let reason = Store::default().versions().expect_err("no engine");
        assert_eq!(reason, not_implemented("Minecraft's version list"));
    }

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

        let id = store
            .create_instance("Scratch instance", None, LoaderKind::Vanilla, None)
            .expect("an instance");
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
            .create_instance("Nowhere", None, LoaderKind::Vanilla, None)
            .expect_err("nothing to create it with");
        assert!(reason.contains("is not implemented yet"), "{reason}");
        let named = Store::default()
            .create_instance("Nowhere", Some("1.21.4"), LoaderKind::Vanilla, None)
            .expect_err("nothing to create it with");
        assert!(named.contains("Creating an instance"), "{named}");
    }

    /// A trimmed Fabric build list: two builds, one of each kind.
    const FABRIC_BUILDS: &str = r#"[
      { "loader": { "version": "0.19.5", "stable": true } },
      { "loader": { "version": "0.19.4", "stable": false } }
    ]"#;

    #[test]
    fn the_loader_picker_reads_the_loader_s_own_service_and_vanilla_asks_nothing() {
        use palantir_net::engine::Loader;

        let fetch = Arc::new(MapFetch::new());
        fetch.set_route(&Loader::Fabric.list_url("1.21.4"), Route::text(FABRIC_BUILDS));
        let store = store_over("loader-builds", fetch.clone());

        let builds = store.loader_builds(LoaderKind::Fabric, "1.21.4").expect("a build list");
        assert_eq!(
            builds.iter().map(|build| build.version.as_str()).collect::<Vec<_>>(),
            vec!["0.19.5", "0.19.4"]
        );
        assert!(builds[0].stable, "Fabric's own flag, carried through");
        assert_eq!(fetch.count(), 1);

        // Vanilla is not a service, so it is not a request: a dialog that opened
        // on it must not send eight chips' worth of traffic to draw one list.
        assert!(store
            .loader_builds(LoaderKind::Vanilla, "1.21.4")
            .expect("vanilla")
            .is_empty());
        assert_eq!(fetch.count(), 1, "vanilla asked nothing");

        // And a store with no way out says so rather than drawing an empty list,
        // which is the same distinction the version picker makes.
        let reason = Store::default()
            .loader_builds(LoaderKind::Quilt, "1.21.4")
            .expect_err("no engine");
        assert_eq!(reason, not_implemented("The loader's build list"));
    }

    #[test]
    fn a_create_writes_the_loader_and_the_build_the_dialog_chose() {
        use palantir_core::pack::PackProfile;

        let home = scratch("create-loader");
        let paths = PalantirPaths::at(&home);
        let fetch = Arc::new(MapFetch::new());
        fetch.set_route(PISTON_MANIFEST_URL, Route::text(MANIFEST_BODY));
        let store = Store::load(&paths)
            .with_engine(Engine::over(scratch("create-loader-cache"), fetch.clone()));

        let id = store
            .create_instance("Fabric instance", Some("1.21.4"), LoaderKind::Fabric, Some("0.19.5"))
            .expect("an instance");
        // The loader is a *component with a version* in the pack profile, which is
        // what the launch resolves: a chip that changed nothing on disk would be a
        // dialog that lied about what it made.
        let profile = PackProfile::load(&store.instance_dir(&id).join("mmc-pack.json"))
            .expect("the instance's pack profile");
        let component = profile
            .get("net.fabricmc.fabric-loader")
            .expect("the loader the dialog chose");
        assert_eq!(component.version, "0.19.5");

        // A loader with no build chosen is a vanilla instance rather than a
        // component with an empty version, which is an instance nothing can
        // resolve.
        let plain = store
            .create_instance("No build", Some("1.21.4"), LoaderKind::Fabric, None)
            .expect("an instance");
        let profile = PackProfile::load(&store.instance_dir(&plain).join("mmc-pack.json"))
            .expect("the second instance's pack profile");
        assert!(profile.get("net.fabricmc.fabric-loader").is_none());
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
        // The game's own output wins, and the launcher's account of the run is
        // what there is when the game never wrote one -- the case a failed launch
        // leaves behind.
        std::fs::write(dir.join("logs").join("launcher.log"), "preparing\nnot launching")
            .expect("a launcher log");
        let with_both = log_tail(&dir, 10).expect("a tail");
        assert!(
            with_both.ends_with("line 99") && !with_both.contains("preparing"),
            "the game's own log wins while there is one: {with_both}"
        );
        std::fs::remove_file(dir.join("logs").join("latest.log")).expect("remove the game log");
        assert_eq!(log_tail(&dir, 10).as_deref(), Some("preparing\nnot launching"));
    }

    #[test]
    fn the_launch_state_is_only_about_the_instance_it_names() {
        // The pages ask this with their own id, so the answer for every other
        // instance has to be `Idle` without the page comparing anything -- and the
        // line keeps naming the instance after the run ended, because the last
        // thing a run said is still about it.
        let mut store = Store::default();
        assert_eq!(store.launch_state("atm10"), LaunchState::Idle);
        assert_eq!(store.launch_line("atm10"), None);
        store.set_launch(
            "atm10",
            Launch {
                state: LaunchState::Running,
                line: Some("process started, streaming output…".to_string()),
            },
        );
        assert_eq!(store.launch_state("atm10"), LaunchState::Running);
        assert_eq!(store.launch_state("sodium"), LaunchState::Idle);
        assert_eq!(store.launch_line("sodium"), None);
        assert_eq!(store.launch_line("atm10"), Some("process started, streaming output…"));
        store.set_launch(
            "atm10",
            Launch {
                state: LaunchState::Idle,
                line: Some("process exited (exit status: 0)".to_string()),
            },
        );
        assert_eq!(store.launch_state("atm10"), LaunchState::Idle);
        assert!(store.launch_line("atm10").is_some(), "the last word outlives the run");
    }

    #[test]
    fn several_runs_are_held_at_once_and_the_chip_follows_one_of_them() {
        // Two facts, because the bar needs both: the popover is drawn from the
        // list of runs, and the chip's name, stop control and logs button are the
        // selected one's.
        let mut store = Store::default();
        store.set_launch("atm10", Launch { state: LaunchState::Running, line: None });
        assert_eq!(store.selected_launch(), Some("atm10"), "the run that was started holds the chip");
        store.set_launch("sodium", Launch { state: LaunchState::Starting, line: None });
        assert_eq!(store.selected_launch(), Some("sodium"), "and the next press moves it");
        let running: Vec<&str> = store.running_launches().iter().map(|(id, _)| *id).collect();
        assert_eq!(running, vec!["atm10", "sodium"], "both of them are running, in instance order");

        // The selection is a name, and the name can stop running: the chip is
        // then about another run rather than about a process that is gone.
        store.set_launch("sodium", Launch { state: LaunchState::Idle, line: None });
        assert_eq!(store.selected_launch(), Some("atm10"));
        store.select_launch("sodium");
        assert_eq!(store.selected_launch(), Some("atm10"), "a stopped run cannot be its subject");
        store.set_launch("sodium", Launch { state: LaunchState::Running, line: None });
        assert_eq!(store.selected_launch(), Some("sodium"), "and a pressed row wins while it goes");

        // An entry that has ended keeps its last word and leaves the list, which
        // is what lets the header of an instance nobody is running still say what
        // happened to it.
        store.set_launch("sodium", Launch { state: LaunchState::Idle, line: Some("bye".to_string()) });
        assert_eq!(store.launch_line("sodium"), Some("bye"));
        assert_eq!(store.running_launches().len(), 1);
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

    #[test]
    fn instance_settings_come_back_the_way_they_went_in() {
        // One round trip through the door the modal uses, with every gate on: the
        // value read back has to be the value a launch would use, or the form is
        // showing the reader numbers nobody else believes.
        let fetch = Arc::new(MapFetch::new());
        let store = store_with_instance("settings-round-trip", fetch);
        let fresh = store.instance_settings("atm10").expect("the settings");
        assert!(!fresh.override_memory && !fresh.override_java && !fresh.override_java_args);
        assert!(fresh.memory_min > 0 && fresh.memory_max >= fresh.memory_min,
            "an instance that overrides nothing still shows a heap: {fresh:?}");

        let edit = InstanceSettings {
            java_path: "C:/jdk21/bin/javaw.exe".to_string(),
            override_java: true,
            memory_min: 2048,
            memory_max: 8192,
            override_memory: true,
            jvm_args: "-XX:+UseG1GC".to_string(),
            override_java_args: true,
        };
        store.save_instance_settings("atm10", &edit).expect("a save");
        assert_eq!(store.instance_settings("atm10").expect("read back"), edit);
    }

    #[test]
    fn a_gate_that_is_off_takes_the_instance_s_own_key_out_of_the_file() {
        // Turning an override off is not "write the launcher's number and mark it
        // unchecked": it is "this instance has no opinion", and the file has to
        // say that rather than hold a number nothing reads.
        let fetch = Arc::new(MapFetch::new());
        let store = store_with_instance("settings-gates", fetch);
        let on = InstanceSettings {
            java_path: "C:/jdk21/bin/javaw.exe".to_string(),
            override_java: true,
            memory_min: 2048,
            memory_max: 8192,
            override_memory: true,
            jvm_args: "-XX:+UseG1GC".to_string(),
            override_java_args: true,
        };
        store.save_instance_settings("atm10", &on).expect("a save with every gate on");
        let off = InstanceSettings {
            override_java: false,
            override_memory: false,
            override_java_args: false,
            ..on.clone()
        };
        store.save_instance_settings("atm10", &off).expect("a save with every gate off");

        let file = std::fs::read_to_string(store.instance_dir("atm10").join("instance.cfg"))
            .expect("the instance file");
        for key in ["JavaPath", "MinMemAlloc", "MaxMemAlloc", "JvmArgs"] {
            assert!(!file.contains(key), "{key} is still in the file:\n{file}");
        }
        let back = store.instance_settings("atm10").expect("read back");
        assert!(!back.override_memory && !back.override_java && !back.override_java_args);
        assert_ne!(back.memory_max, 8192, "the launcher's own ceiling is what is in force now");
    }

    #[test]
    fn a_heap_that_cannot_be_one_is_a_sentence_rather_than_a_written_line() {
        let fetch = Arc::new(MapFetch::new());
        let store = store_with_instance("settings-heaps", fetch);
        let mut edit = store.instance_settings("atm10").expect("the settings");
        edit.override_memory = true;

        edit.memory_min = 64;
        edit.memory_max = 4096;
        let below = store.save_instance_settings("atm10", &edit).expect_err("under the floor");
        assert!(below.contains("floor"), "{below}");

        edit.memory_min = 4096;
        edit.memory_max = 2048;
        let upside_down = store.save_instance_settings("atm10", &edit).expect_err("upside down");
        assert!(upside_down.contains("below the minimum"), "{upside_down}");

        // Neither refusal wrote anything: the file still says what it said.
        let back = store.instance_settings("atm10").expect("read back");
        assert!(!back.override_memory);
    }

    #[test]
    fn an_instance_s_installation_is_the_platform_and_the_two_versions_it_runs() {
        // The values the creation flow wrote are the values the tab reads back:
        // one profile, no second copy of the facts anywhere.
        let fetch = Arc::new(MapFetch::new());
        let store = store_with_instance("installation-read", fetch);
        let installation = store.instance_installation("atm10").expect("the installation");
        assert_eq!(installation.platform, LoaderKind::Fabric);
        assert_eq!(installation.game_version, "1.21.4");
        assert_eq!(installation.loader_build, "0.16.9");
    }

    #[test]
    fn changing_the_platform_takes_the_loader_it_replaces_out() {
        // Two loader components in one profile is a profile nothing can resolve,
        // so a switch is a write *and* a removal -- and neither is allowed to
        // touch the components this tab does not own.
        let fetch = Arc::new(MapFetch::new());
        let store = store_with_instance("installation-write", fetch);
        let mut edit = store.instance_installation("atm10").expect("the installation");
        edit.platform = LoaderKind::NeoForge;
        edit.loader_build = "21.4.157".to_string();
        edit.game_version = "1.21.1".to_string();
        store.save_instance_installation("atm10", &edit).expect("a save");

        let back = store.instance_installation("atm10").expect("read back");
        assert_eq!(back, edit);
        let file = std::fs::read_to_string(store.instance_dir("atm10").join("mmc-pack.json"))
            .expect("the profile");
        assert!(!file.contains("net.fabricmc.fabric-loader"), "the old loader is gone:\n{file}");
        assert!(file.contains("net.neoforged"), "{file}");

        // And back to vanilla: the loader goes, the game version stays where the
        // tab left it.
        let vanilla = InstanceInstallation {
            platform: LoaderKind::Vanilla,
            game_version: "1.21.1".to_string(),
            loader_build: String::new(),
        };
        store.save_instance_installation("atm10", &vanilla).expect("a vanilla save");
        let back = store.instance_installation("atm10").expect("read back");
        assert_eq!(back, vanilla);
        let file = std::fs::read_to_string(store.instance_dir("atm10").join("mmc-pack.json"))
            .expect("the profile");
        assert!(!file.contains("net.neoforged"), "{file}");
        assert!(file.contains("net.minecraft"), "{file}");
    }

    #[test]
    fn an_installation_that_could_not_be_resolved_is_a_sentence_rather_than_a_written_line() {
        let fetch = Arc::new(MapFetch::new());
        let store = store_with_instance("installation-refusals", fetch);
        let mut edit = store.instance_installation("atm10").expect("the installation");

        edit.game_version = "   ".to_string();
        let nothing_to_run = store.save_instance_installation("atm10", &edit).expect_err("no game");
        assert!(nothing_to_run.contains("game version"), "{nothing_to_run}");

        edit.game_version = "1.21.4".to_string();
        edit.loader_build = String::new();
        let nothing_to_load =
            store.save_instance_installation("atm10", &edit).expect_err("no build");
        assert!(nothing_to_load.contains("Fabric"), "{nothing_to_load}");

        // A loader this launcher does not model is refused rather than dropped:
        // writing one by hand is what a profile edited elsewhere looks like, and
        // a save that carried on would be this tab editing a file it cannot read.
        let path = store.instance_dir("atm10").join("mmc-pack.json");
        let text = std::fs::read_to_string(&path).expect("the profile");
        let mut profile = PackProfile::from_text(&text, &path).expect("a profile");
        profile.append(palantir_core::pack::Component {
            uid: "com.mumfrey.liteloader".to_string(),
            version: "1.12.2".to_string(),
            ..Default::default()
        });
        profile.save(&path).expect("the profile saved");
        let edit = store.instance_installation("atm10").expect("the installation");
        let unmodelled =
            store.save_instance_installation("atm10", &edit).expect_err("not modelled");
        assert!(unmodelled.contains("com.mumfrey.liteloader"), "{unmodelled}");
    }
}
