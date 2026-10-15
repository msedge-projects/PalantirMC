use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

use crate::model::{
    self, Instance, LaunchMode, Loader, Preferences, Project, ProjectDetail, ProjectType, Result,
    SearchResponse, Sort,
};
use palantir_core::assets::{AssetDestination, AssetIndex};
use palantir_core::paths::DataRoot;
use palantir_core::rules::{Arch, Os, Platform};
use palantir_core::version::Version;
use palantir_loader::{dependencies, forge, install, installer, java, launch, mrpack, profiles};
use palantir_net::{
    auth,
    cache::{MANIFEST_TTL, MetadataCache, cache_dir_for},
    client::Http,
    download::{DownloadOptions, download},
    scheduler::Scheduler,
    session,
    store::ContentStore,
    sync::Syncer,
};

#[derive(Debug)]
pub enum Request {
    Create(String, String, Loader, String),
    GameVersions,
    LoaderVersions(String, Loader),
    InspectSource(u64, String),
    Browse(String),
    Releases(String),
    OpenUrl(String),
    Authenticate,
    CancelAuthentication,
    BedrockDetect,
    BedrockLaunch(palantir_bedrock::Channel),
    BedrockInventory(palantir_bedrock::Channel),
    BedrockFolder(palantir_bedrock::Channel, String),
    /// The archive to install, and the optional client files the review screen
    /// decided on: `None` means the player has not been shown them yet, which is
    /// a different state from an empty choice and must not install silently.
    Import(
        String,
        String,
        String,
        Loader,
        Option<Vec<String>>,
        Option<String>,
    ),
    FetchPack(u64, Project, model::Release),
    Rename(String, String),
    /// Copy a collection byte for byte under a new name. The worker owns the
    /// copy; the source is read only.
    Duplicate(String, String),
    /// One collection's game window size: `None` returns it to the launcher
    /// preference. The worker validates and persists it.
    Window(String, Option<model::WindowOverride>),
    /// One collection's memory budget, Java runtime, and launch customs.
    /// `None` on the first two returns that side to the launcher preference.
    Runtime(
        String,
        Option<u32>,
        Option<String>,
        palantir_loader::hooks::LaunchCustom,
    ),
    Inventory(String),
    ToggleMod(String, String),
    RemoveContent(String, String, String),
    OpenFolder(Option<String>, String),
    Delete(String),
    Save(Preferences),
    /// Ask a candidate Java executable for its version. The answer joins the
    /// roster; nothing is installed or deleted.
    JavaProbe(String),
    Launch(String, Preferences, LaunchMode),
    Stop(String),
    Search(u64, String, ProjectType, Sort, usize, String),
    Project(String),
    ReviewContent(u64, String, Project, String),
    Content(String, Project, model::ContentPlan),
    ReviewLocal(u64, crate::local::Input),
    ApplyLocal(crate::local::Plan),
    Shutdown,
}
#[derive(Debug)]
pub enum Event {
    Loaded {
        instances: Vec<Instance>,
        preferences: Preferences,
        root: String,
    },
    Instances(Vec<Instance>),
    GameVersions(Result<Vec<model::GameChoice>>),
    LoaderVersions(String, Loader, Result<Vec<String>>),
    ImportReview(u64, Result<model::ImportReview>),
    FetchedPack(u64, String, String, Result<model::ImportReview>),
    ContentReview(u64, Result<model::ContentPlan>),
    LocalReview(u64, Result<crate::local::Plan>),
    Directory(String, Result<Vec<model::DirectoryEntry>>),
    Releases(String, Result<Vec<model::Release>>),
    Inventory(String, Result<Vec<crate::files::Entry>>),
    Status(String, String),
    Finished(String, Result<String>),
    /// A candidate runtime's self-reported version line, or why it is not a
    /// usable Java executable.
    JavaProbed(String, Result<String>),
    /// A confirmed reviewed local operation's outcome. Separate from
    /// `Finished` so the receipt cannot be confused with another task
    /// finishing under the same instance key.
    LocalApplied(String, Result<String>),
    Running(String, String),
    Exited(String, String),
    Log(String, String),
    Search(u64, Result<(Vec<Project>, usize)>),
    Project(String, Result<ProjectDetail>),
    Error(String),
    Authentication(model::Authentication),
    BedrockInstalls(Result<Vec<palantir_bedrock::BedrockInstall>>),
    BedrockInventory(palantir_bedrock::Channel, Result<crate::bedrock::Inventory>),
}

struct Engine {
    root: DataRoot,
    http: Http,
    cache: MetadataCache,
    store: ContentStore,
    scheduler: Scheduler,
    processes: Mutex<HashMap<String, Arc<Mutex<Child>>>>,
    // Shared version/cache staging paths require one installation writer at a time.
    install_lock: Mutex<()>,
    /// The stop flag of the sign-in attempt in flight. One attempt at a time: a
    /// second request stops the first, and cancelling takes the flag away so a
    /// stale one cannot stop the attempt that follows.
    signing_in: Mutex<Option<Arc<AtomicBool>>>,
    shutdown: AtomicBool,
}
impl Engine {
    fn new() -> Result<Self> {
        // An explicit override also makes interface smoke tests isolated from real user data.
        let root = match std::env::var_os("PALANTIR_DATA_DIR") {
            Some(path) => DataRoot::new(path),
            None => DataRoot::platform_default().map_err(|e| e.to_string())?,
        };
        Self::at(root)
    }
    /// The engine assembled over a root the caller names.
    ///
    /// `new` is the product's door -- the environment override, else the
    /// platform default -- and this is the body behind it. A test opens this
    /// one so it can point the engine at a directory it made: `PALANTIR_DATA_DIR`
    /// is process-global and this test binary runs multi-threaded, so setting it
    /// from a test would race every sibling in the same process.
    fn at(root: DataRoot) -> Result<Self> {
        std::fs::create_dir_all(&root.root).map_err(|e| e.to_string())?;
        Ok(Self {
            cache: MetadataCache::new(cache_dir_for(&root)),
            store: ContentStore::new(root.content_dir()),
            scheduler: Scheduler::with_default_limit(),
            http: Http::new().map_err(|e| e.to_string())?,
            root,
            processes: Mutex::new(HashMap::new()),
            install_lock: Mutex::new(()),
            signing_in: Mutex::new(None),
            shutdown: AtomicBool::new(false),
        })
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
    fn instance(&self, id: &str) -> Result<Instance> {
        model::load_instances(&self.root)?
            .into_iter()
            .find(|i| i.id == id)
            .ok_or_else(|| "Instance no longer exists. Refresh the library.".into())
    }
    fn busy(&self, id: &str) -> Result<bool> {
        Ok(self
            .processes
            .lock()
            .map_err(|e| e.to_string())?
            .contains_key(id))
    }
}
fn emit(events: &mpsc::Sender<Event>, event: Event) {
    if events.send(event).is_err() { /* The window closed; no receiver needs this event. */ }
}
fn refresh(engine: &Engine, events: &mpsc::Sender<Event>) {
    match model::load_instances(&engine.root) {
        Ok(items) => emit(events, Event::Instances(items)),
        Err(error) => emit(events, Event::Error(error)),
    }
}

pub fn start() -> (mpsc::Sender<Request>, mpsc::Receiver<Event>) {
    let (requests, receive) = mpsc::channel();
    let (events, output) = mpsc::channel();
    std::thread::spawn(move || {
        let engine = match Engine::new() {
            Ok(engine) => Arc::new(engine),
            Err(error) => {
                emit(
                    &events,
                    Event::Error(format!("Cannot open the launcher: {error}")),
                );
                return;
            }
        };
        match (
            model::load_instances(&engine.root),
            model::load_preferences(&engine.root),
        ) {
            (Ok(instances), Ok(preferences)) => emit(
                &events,
                Event::Loaded {
                    instances,
                    preferences,
                    root: engine.root.root.display().to_string(),
                },
            ),
            (instances, preferences) => {
                if let Err(error) = instances {
                    emit(&events, Event::Error(error));
                }
                if let Err(error) = preferences {
                    emit(&events, Event::Error(error));
                }
            }
        }
        let active: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        for request in receive {
            match request {
                Request::ReviewLocal(revision, input) => {
                    let engine = Arc::clone(&engine);
                    let events = events.clone();
                    std::thread::spawn(move || {
                        let result = local_root(&engine, input.target())
                            .and_then(|root| crate::local::review(&root, &input));
                        emit(&events, Event::LocalReview(revision, result));
                    });
                }
                Request::BedrockDetect => {
                    let events = events.clone();
                    std::thread::spawn(move || {
                        emit(
                            &events,
                            Event::BedrockInstalls(palantir_bedrock::windows::detect_installs()),
                        );
                    });
                }
                Request::BedrockInventory(channel) => {
                    let events = events.clone();
                    std::thread::spawn(move || {
                        let result = detected_bedrock(channel)
                            .and_then(|copy| crate::bedrock::inventory(&copy));
                        emit(&events, Event::BedrockInventory(channel, result));
                    });
                }
                Request::BedrockLaunch(channel) => {
                    let events = events.clone();
                    std::thread::spawn(move || {
                        let result = detected_bedrock(channel).and_then(|copy| {
                            palantir_bedrock::windows::launch(&copy)?;
                            Ok(format!("Windows was asked to open Bedrock {channel}. Check the game window; account and licence checks remain with Windows and the game."))
                        });
                        emit(&events, Event::Finished("bedrock-launch".into(), result));
                    });
                }
                Request::BedrockFolder(channel, folder) => {
                    let events = events.clone();
                    std::thread::spawn(move || {
                        let result = detected_bedrock(channel)
                            .and_then(|copy| crate::bedrock::open_folder(&copy, &folder));
                        if let Err(error) = result {
                            emit(&events, Event::Error(error));
                        }
                    });
                }
                Request::Authenticate => {
                    // No registered client id means there is nothing to call, so
                    // the feature is reported unavailable: a device code this
                    // launcher invented would be a lie the player acts on.
                    let client_id = std::env::var("PALANTIR_MSA_CLIENT_ID")
                        .unwrap_or_default()
                        .trim()
                        .to_string();
                    if client_id.is_empty() {
                        emit(
                            &events,
                            Event::Authentication(model::Authentication::Failed(
                                "Microsoft sign-in is unavailable in this build: no authentication client is registered. Set PALANTIR_MSA_CLIENT_ID to a registered native client to enable it.".into(),
                            )),
                        );
                    } else {
                        let cancel = Arc::new(AtomicBool::new(false));
                        if let Ok(mut slot) = engine.signing_in.lock() {
                            if let Some(previous) = slot.replace(Arc::clone(&cancel)) {
                                previous.store(true, Ordering::SeqCst);
                            }
                        }
                        let engine = Arc::clone(&engine);
                        let events = events.clone();
                        std::thread::spawn(move || sign_in(&engine, &events, &client_id, &cancel));
                    }
                }
                Request::CancelAuthentication => {
                    if let Ok(mut slot) = engine.signing_in.lock() {
                        if let Some(flag) = slot.take() {
                            flag.store(true, Ordering::SeqCst);
                        }
                    }
                    emit(
                        &events,
                        Event::Authentication(model::Authentication::Cancelled),
                    );
                }
                Request::OpenUrl(url) => {
                    if let Err(error) = open_url(&url) {
                        emit(&events, Event::Error(error));
                    }
                }
                Request::GameVersions => {
                    let engine = Arc::clone(&engine);
                    let events = events.clone();
                    std::thread::spawn(move || {
                        let result = (|| {
                            let text = engine
                                .cache
                                .fetch_text(
                                    &engine.http,
                                    "desktop-game-versions",
                                    install::VERSION_MANIFEST_URL,
                                    MANIFEST_TTL,
                                )
                                .map_err(|e| e.to_string())?;
                            let manifest = palantir_core::version::VersionManifest::parse(&text)
                                .map_err(|e| e.to_string())?;
                            Ok(manifest
                                .versions
                                .into_iter()
                                .map(|v| model::GameChoice {
                                    id: v.id,
                                    kind: v.kind,
                                })
                                .collect())
                        })();
                        emit(&events, Event::GameVersions(result));
                    });
                }
                Request::LoaderVersions(game, loader) => {
                    let engine = Arc::clone(&engine);
                    let events = events.clone();
                    std::thread::spawn(move || {
                        let result = if let Some(vendor) = loader.installer_vendor() {
                            vendor_releases(&engine, vendor, &game)
                        } else {
                            let sources =
                                profiles::LoaderSources::vendor(if loader == Loader::Fabric {
                                    profiles::LoaderKind::Fabric
                                } else {
                                    profiles::LoaderKind::Quilt
                                });
                            profiles::list_loaders(&engine.http, &engine.cache, &sources, &game)
                                .map(|v| v.into_iter().map(|r| r.version).collect())
                                .map_err(|e| e.to_string())
                        };
                        emit(&events, Event::LoaderVersions(game, loader, result));
                    });
                }
                Request::FetchPack(revision, project, release) => {
                    let engine = Arc::clone(&engine);
                    let events = events.clone();
                    std::thread::spawn(move || {
                        let result = fetch_catalogue_pack(&engine, &project, &release);
                        emit(
                            &events,
                            Event::FetchedPack(revision, project.project_id, release.id, result),
                        );
                    });
                }
                Request::InspectSource(id, path) => {
                    let events = events.clone();
                    std::thread::spawn(move || {
                        emit(
                            &events,
                            Event::ImportReview(
                                id,
                                crate::files::inspect_source(std::path::Path::new(&path)),
                            ),
                        )
                    });
                }
                Request::Browse(path) => {
                    let events = events.clone();
                    std::thread::spawn(move || {
                        let result = crate::files::browse(std::path::Path::new(&path));
                        emit(&events, Event::Directory(path, result));
                    });
                }
                Request::Releases(id) => {
                    let engine = Arc::clone(&engine);
                    let events = events.clone();
                    std::thread::spawn(move || {
                        let result = model::safe_component(&id).and_then(|()| {
                            get_json(
                                &engine.http,
                                &format!("https://api.modrinth.com/v2/project/{id}/version"),
                            )
                        });
                        emit(&events, Event::Releases(id, result));
                    });
                }
                Request::Inventory(id) => emit(
                    &events,
                    Event::Inventory(id.clone(), crate::files::inventory(&engine.root, &id)),
                ),
                Request::OpenFolder(id, folder) => {
                    if let Err(error) =
                        crate::files::open_folder(&engine.root, id.as_deref(), &folder)
                    {
                        emit(&events, Event::Error(error));
                    }
                }
                Request::RemoveContent(id, folder, name) => {
                    let result = (|| {
                        if engine.busy(&id)?
                            || active.lock().map_err(|e| e.to_string())?.contains(&id)
                        {
                            return Err(
                                "Stop the game and finish its task before removing content.".into(),
                            );
                        }
                        crate::files::remove_content(&engine.root, &id, &folder, &name)?;
                        Ok("Addition removed.".into())
                    })();
                    emit(&events, Event::Finished(id.clone(), result));
                    emit(
                        &events,
                        Event::Inventory(id.clone(), crate::files::inventory(&engine.root, &id)),
                    );
                }
                Request::ToggleMod(id, name) => {
                    let result = (|| {
                        if engine.busy(&id)?
                            || active.lock().map_err(|e| e.to_string())?.contains(&id)
                        {
                            return Err(
                                "Stop the game and finish its current operation first.".into()
                            );
                        }
                        crate::files::toggle_mod(&engine.root, &id, &name)?;
                        Ok("Mod state changed.".into())
                    })();
                    emit(&events, Event::Finished(id.clone(), result));
                    emit(
                        &events,
                        Event::Inventory(id.clone(), crate::files::inventory(&engine.root, &id)),
                    );
                }
                Request::Rename(id, name) => {
                    let result = (|| {
                        let name = name.trim();
                        if name.is_empty()
                            || name.chars().count() > 80
                            || name.chars().any(char::is_control)
                        {
                            return Err("Use a name between 1 and 80 characters.".into());
                        }
                        let mut item = engine.instance(&id)?;
                        item.name = name.into();
                        model::save_instance(&engine.root, &item)?;
                        Ok("Instance renamed.".into())
                    })();
                    emit(&events, Event::Finished(format!("rename:{id}"), result));
                    refresh(&engine, &events);
                }
                Request::Duplicate(id, name) => {
                    let engine = Arc::clone(&engine);
                    let events = events.clone();
                    std::thread::spawn(move || {
                        let result = model::duplicate(&engine.root, &id, &name).map(|copy| {
                            format!(
                                "Duplicated as {}. The original was left untouched.",
                                copy.name
                            )
                        });
                        emit(&events, Event::Finished(format!("duplicate:{id}"), result));
                        refresh(&engine, &events);
                    });
                }
                Request::Window(id, window) => {
                    let result = (|| {
                        if let Some(window) = window {
                            window.validate()?;
                        }
                        let mut item = engine.instance(&id)?;
                        item.window = window;
                        model::save_instance(&engine.root, &item)?;
                        Ok(match window {
                            Some(window) => format!(
                                "{} now launches at {}×{}.",
                                item.name, window.width, window.height
                            ),
                            None => "This collection now follows the launcher window size.".into(),
                        })
                    })();
                    emit(&events, Event::Finished(format!("window:{id}"), result));
                    refresh(&engine, &events);
                }
                Request::Runtime(id, memory_mb, java, custom) => {
                    let result = (|| {
                        if let Some(memory_mb) = memory_mb {
                            model::validate_memory_mb(memory_mb)?;
                        }
                        if let Some(java) = &java {
                            model::validate_java_override(java)?;
                        }
                        custom.validate().map_err(|error| error.to_string())?;
                        let mut item = engine.instance(&id)?;
                        item.memory_mb = memory_mb;
                        item.java = java;
                        item.custom = custom;
                        model::save_instance(&engine.root, &item)?;
                        Ok("Launch settings saved for this collection.".into())
                    })();
                    emit(&events, Event::Finished(format!("runtime:{id}"), result));
                    refresh(&engine, &events);
                }
                Request::Import(source, name, game, loader, selected, reviewed_hash) => {
                    let engine = Arc::clone(&engine);
                    let events = events.clone();
                    std::thread::spawn(move || {
                        // A pack archive is installed rather than copied: its
                        // index lists files that were never on this machine.
                        let from_archive = Path::new(&source)
                            .extension()
                            .is_some_and(|extension| extension == "mrpack");
                        let result = if from_archive {
                            (|| {
                                let expected = reviewed_hash
                                    .ok_or("Review this archive before installing it.")?;
                                crate::local::checked(Path::new(&source))?;
                                if palantir_net::download::sha1_file(Path::new(&source))
                                    .map_err(|e| e.to_string())?
                                    != expected
                                {
                                    return Err("Pack archive changed since review. Inspect it again; nothing was created.".into());
                                }
                                let _writer =
                                    engine.install_lock.lock().map_err(|e| e.to_string())?;
                                install_pack_archive(
                                    &engine,
                                    &events,
                                    &source,
                                    &name,
                                    &game,
                                    loader,
                                    selected.as_deref(),
                                )
                            })()
                        } else {
                            crate::files::import_directory(
                                &engine.root,
                                Path::new(&source),
                                &name,
                                &game,
                                loader,
                            )
                            .map(|item| {
                                format!("Imported {}. The source was left untouched.", item.name)
                            })
                        };
                        emit(&events, Event::Finished("import".into(), result));
                        refresh(&engine, &events);
                    });
                }
                Request::Create(name, game, loader, pin) => {
                    let result = (|| {
                        if !pin.is_empty() {
                            model::safe_component(&pin)?;
                        }
                        let mut item = model::create(&engine.root, &name, &game, loader)?;
                        if !pin.is_empty() {
                            item.loader_version = Some(pin);
                            model::save_instance(&engine.root, &item)?;
                        }
                        Ok(item)
                    })()
                    .map(|i: Instance| {
                        format!(
                            "Created {}. Your game files will be installed on first launch.",
                            i.name
                        )
                    });
                    emit(&events, Event::Finished("create".into(), result));
                    refresh(&engine, &events);
                }
                Request::JavaProbe(path) => {
                    let result = (|| {
                        model::ensure_java(&path)?;
                        let output = std::process::Command::new(&path)
                            .arg("-version")
                            .output()
                            .map_err(|error| format!("Cannot run {path}: {error}"))?;
                        if !output.status.success() {
                            return Err(format!(
                                "{path} did not answer -version ({}).",
                                output.status
                            ));
                        }
                        // `java -version` reports on stderr, by convention.
                        let text = String::from_utf8_lossy(&output.stderr);
                        Ok(text.lines().next().unwrap_or("").trim().to_string())
                    })();
                    emit(&events, Event::JavaProbed(path, result));
                }
                Request::Save(preferences) => emit(
                    &events,
                    Event::Finished(
                        "preferences".into(),
                        model::save_preferences(&engine.root, &preferences)
                            .map(|_| "Preferences saved.".into()),
                    ),
                ),
                Request::Delete(id) => {
                    let result = (|| {
                        if engine.busy(&id)?
                            || active.lock().map_err(|e| e.to_string())?.contains(&id)
                        {
                            return Err("Stop the instance and wait for its current operation before deleting it.".into());
                        }
                        let path = model::instance_path(&engine.root, &id)?;
                        std::fs::remove_dir_all(path).map_err(|e| e.to_string())?;
                        Ok("Instance deleted. Shared downloads were kept.".into())
                    })();
                    emit(&events, Event::Finished(id, result));
                    refresh(&engine, &events);
                }
                Request::Stop(id) => {
                    if let Err(error) = stop(&engine, &id) {
                        emit(&events, Event::Error(error));
                    }
                }
                Request::Project(id) => {
                    let engine = Arc::clone(&engine);
                    let events = events.clone();
                    std::thread::spawn(move || {
                        let result = model::safe_component(&id).and_then(|()| {
                            get_json(
                                &engine.http,
                                &format!("https://api.modrinth.com/v2/project/{id}"),
                            )
                        });
                        emit(&events, Event::Project(id, result));
                    });
                }
                Request::ReviewContent(review_id, id, project, release) => {
                    let engine = Arc::clone(&engine);
                    let events = events.clone();
                    std::thread::spawn(move || {
                        let result = resolve_content_plan(&engine, &id, &project, &release);
                        emit(&events, Event::ContentReview(review_id, result));
                    });
                }
                Request::Search(id, query, kind, sort, offset, game) => {
                    let engine = Arc::clone(&engine);
                    let events = events.clone();
                    std::thread::spawn(move || {
                        emit(
                            &events,
                            Event::Search(
                                id,
                                search(&engine.http, &query, kind, sort, offset, &game),
                            ),
                        )
                    });
                }
                request @ (Request::Launch(_, _, _)
                | Request::Content(_, _, _)
                | Request::ApplyLocal(_)) => {
                    let id = match &request {
                        Request::Launch(id, _, _) | Request::Content(id, _, _) => id.clone(),
                        Request::ApplyLocal(plan) => plan.input.target().key(),
                        _ => continue,
                    };
                    let accepted = match active.lock() {
                        Ok(mut active) => {
                            if active.contains(&id) {
                                false
                            } else {
                                active.push(id.clone());
                                true
                            }
                        }
                        Err(error) => {
                            emit(&events, Event::Error(error.to_string()));
                            false
                        }
                    };
                    if !accepted {
                        emit(
                            &events,
                            Event::Error(
                                "An operation is already running for this instance.".into(),
                            ),
                        );
                        continue;
                    }
                    let local_apply = matches!(request, Request::ApplyLocal(_));
                    let engine = Arc::clone(&engine);
                    let events = events.clone();
                    let active = Arc::clone(&active);
                    std::thread::spawn(move || {
                        let result = (|| {
                            let _writer = engine.install_lock.lock().map_err(|e| e.to_string())?;
                            if engine.shutdown.load(Ordering::SeqCst) {
                                return Err(
                                    "The launcher closed before this operation started.".into()
                                );
                            }
                            match request {
                                Request::Launch(id, prefs, mode) => {
                                    prepare_and_launch(&engine, &events, &id, prefs, mode)
                                }
                                Request::Content(id, project, release) => {
                                    install_content(&engine, &events, &id, &project, &release)
                                }
                                Request::ApplyLocal(plan) => {
                                    let root = local_root(&engine, plan.input.target())?;
                                    crate::local::apply(&root, &plan)
                                }
                                _ => Err("Unknown background operation.".into()),
                            }
                        })();
                        if let Ok(mut active) = active.lock() {
                            active.retain(|item| item != &id);
                        }
                        if local_apply {
                            emit(&events, Event::LocalApplied(id.clone(), result));
                        } else {
                            emit(&events, Event::Finished(id.clone(), result));
                        }
                        if id != "bedrock-local" {
                            emit(
                                &events,
                                Event::Inventory(
                                    id.clone(),
                                    crate::files::inventory(&engine.root, &id),
                                ),
                            );
                        }
                        refresh(&engine, &events);
                    });
                }
                Request::Shutdown => break,
            }
        }
        engine.shutdown.store(true, Ordering::SeqCst);
        let ids = engine
            .processes
            .lock()
            .map(|processes| processes.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        for id in ids {
            if let Err(error) = stop(&engine, &id) {
                emit(&events, Event::Error(error));
            }
        }
    });
    (requests, output)
}

// Refresh the OS answer at the action boundary: UI metadata must not become an
// arbitrary activation string or filesystem path, nor name an uninstalled copy.
fn detected_bedrock(
    channel: palantir_bedrock::Channel,
) -> Result<palantir_bedrock::BedrockInstall> {
    palantir_bedrock::windows::detect_installs()?
        .into_iter()
        .find(|copy| copy.channel == channel)
        .ok_or_else(|| "This Bedrock copy is no longer installed. Refresh installed copies.".into())
}

fn local_root(engine: &Engine, target: &crate::local::Target) -> Result<PathBuf> {
    match target {
        crate::local::Target::Java(id) => {
            if engine.busy(id)? {
                return Err("Close the game before changing content or copying a world.".into());
            }
            engine.instance(id)?;
            crate::files::game_directory(&engine.root, id)
        }
        crate::local::Target::Bedrock(channel) => {
            crate::bedrock::folder_path(&detected_bedrock(*channel)?, "")
        }
    }
}

fn platform() -> Platform {
    Platform::new(Os::host(), "", Arch::host())
}
/// Run the post-exit hook after the game's process has ended, however it
/// ended. A hook that fails is reported as an error event; it never hides
/// the exit itself and never stops the supervisor.
fn run_post_exit(
    hook: &Option<String>,
    dir: &std::path::Path,
    env: &[(String, String)],
    events: &mpsc::Sender<Event>,
) {
    let Some(hook) = hook else { return };
    match palantir_loader::hooks::run(hook, dir, env) {
        Ok(output) if output.status.success() => {}
        Ok(output) => emit(
            events,
            Event::Error(format!(
                "Post-exit hook failed ({}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )),
        ),
        Err(error) => emit(
            events,
            Event::Error(format!("Post-exit hook could not run: {error}")),
        ),
    }
}
fn prepare_and_launch(
    engine: &Arc<Engine>,
    events: &mpsc::Sender<Event>,
    id: &str,
    preferences: Preferences,
    mode: LaunchMode,
) -> Result<String> {
    preferences.validate()?;
    crate::files::game_directory(&engine.root, id)?;
    if engine.busy(id)? {
        return Err("This instance is already running.".into());
    }
    let mut instance = engine.instance(id)?;
    emit(
        events,
        Event::Status(id.into(), "Installing game and shared libraries…".into()),
    );
    // The processor plan travels beside the document: a Forge-family install is
    // not finished when its files land, and which Java has to run its
    // processors is only known once the installed document's own runtime
    // requirement can be read.
    let (version, processors) = match &instance.installed_version {
        Some(version) => (
            Version::parse(
                &std::fs::read_to_string(engine.root.version_json(version))
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?,
            Vec::new(),
        ),
        None => {
            let (version, processors) = install_version(engine, &instance, id)?;
            instance.installed_version = Some(version.id.clone());
            model::save_instance(&engine.root, &instance)?;
            (version, processors)
        }
    };
    // Repair missing files on subsequent launches instead of trusting only the descriptor.
    engine
        .syncer()
        .sync_version(&version, &platform(), Some(&engine.root.game_dir(id)))
        .map_err(|e| e.to_string())?;
    emit(
        events,
        Event::Status(id.into(), "Preparing Java and native libraries…".into()),
    );
    // A collection's own runtime wins over the launcher's; a blank anywhere
    // falls back to the managed runtime the game's own metadata names.
    let java = match model::effective_java(&instance, &preferences)? {
        Some(path) => {
            // A removed or renamed runtime refuses by name before anything
            // else happens; the message says where to go next.
            model::ensure_java(&path)?;
            PathBuf::from(path)
        }
        None => runtime(engine, &version)?,
    };
    // The loaders' own installer processors run before anything is extracted or
    // launched: their artifacts are what the overlay's libraries name, so a
    // launch that started first would run against a half-patched install. A
    // processor whose receipts already hold is skipped, which is exactly what a
    // second play of the same collection looks like.
    if !processors.is_empty() {
        emit(
            events,
            Event::Status(
                id.into(),
                format!(
                    "Running {} installer processor(s) for {}…",
                    processors.len(),
                    instance.loader
                ),
            ),
        );
        let report = launch::run_processors(&java, &engine.root.libraries_dir(), &processors)
            .map_err(|e| e.to_string())?;
        emit(
            events,
            Event::Status(
                id.into(),
                format!(
                    "{} processor(s) ran, {} skipped on their receipts.",
                    report.ran.len(),
                    report.skipped.len()
                ),
            ),
        );
    }
    let natives = engine.root.instance_dir(id).join("natives");
    std::fs::create_dir_all(&natives).map_err(|e| e.to_string())?;
    for library in &version.libraries {
        if let Some(resolved) = library.resolve(&platform()).map_err(|e| e.to_string())? {
            if let Some(native) = resolved.natives {
                launch::extract_natives(
                    &engine
                        .root
                        .library_file(&native.rel_path)
                        .map_err(|e| e.to_string())?,
                    &natives,
                    &resolved.extract_exclude,
                )
                .map_err(|e| e.to_string())?;
            }
        }
    }
    let game_assets = if let Some(index) = &version.asset_index {
        let index_data = AssetIndex::parse(
            &std::fs::read_to_string(engine.root.asset_index_file(&index.id))
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        match index_data.destination() {
            AssetDestination::ObjectStore => engine.root.assets_dir(),
            AssetDestination::Resources => engine.root.game_dir(id).join("resources"),
            AssetDestination::Virtual => engine.root.virtual_assets_dir(&index.id),
        }
    } else {
        engine.root.assets_dir()
    };
    // Which identity this launch carries is decided by the mode the player chose,
    // never by what happens to be available: a Microsoft launch with no usable
    // session is refused, because a launcher that quietly fell back to an offline
    // identity would be reporting a session it does not have.
    //
    // The offline identity comes from the selected offline account, so switching
    // accounts changes the name and uuid the game is handed and nothing else. The
    // derivation lives in the model so one name cannot mean two identifiers.
    let (player_name, player_uuid, access_token, user_type) = match mode {
        LaunchMode::Offline => {
            let (_, name, uuid) = preferences.active_identity();
            (name, uuid, "0".into(), "legacy".into())
        }
        LaunchMode::Microsoft => {
            let (name, uuid, token) = authenticated_identity(engine)?;
            (name, uuid, token, "msa".into())
        }
    };
    // The version format gates `--width/--height` behind a feature and names
    // the values as placeholders: the player's size switches the gate on and
    // fills it, and no size leaves the game's own default alone.
    let resolution = model::effective_resolution(&instance, &preferences)?;
    let mut launch_platform = platform();
    if resolution.is_some() {
        launch_platform
            .features
            .insert("has_custom_resolution".to_string(), true);
    }
    let context = launch::LaunchContext {
        player_name,
        player_uuid,
        access_token,
        user_type,
        java,
        game_dir: engine.root.game_dir(id),
        assets_root: engine.root.assets_dir(),
        natives_dir: natives,
        library_root: engine.root.libraries_dir(),
        client_jar: engine.root.version_jar(&version.id),
        game_assets,
        placeholders: resolution
            .map(|(width, height)| {
                let mut values = std::collections::BTreeMap::new();
                values.insert("resolution_width".to_string(), width.to_string());
                values.insert("resolution_height".to_string(), height.to_string());
                values
            })
            .unwrap_or_default(),
        ..Default::default()
    };
    let plan = launch::build_launch_plan(&version, &launch_platform, &context)
        .map_err(|e| e.to_string())?;
    // JVM flags must precede the main class, not follow game arguments.
    let memory_mb = model::effective_memory(&instance, &preferences)?;
    let custom = instance.custom.clone();
    custom.validate().map_err(|error| error.to_string())?;
    // A pre-launch hook runs to completion before anything spawns; a hook
    // that fails stops the launch by name rather than half-starting a game.
    if let Some(hook) = custom.hooks.pre_launch.as_deref() {
        let output = palantir_loader::hooks::run(hook, &engine.root.game_dir(id), &custom.env)
            .map_err(|error| format!("Pre-launch hook could not run: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "Pre-launch hook failed ({}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
    }
    let original = plan.command();
    let rest: Vec<String> = std::iter::once(format!("-Xmx{memory_mb}M"))
        .chain(
            original
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned()),
        )
        .collect();
    // The wrapper hook, when present, becomes the program and its prefix
    // arguments (`gamemoderun java ...`); extra arguments and environment
    // entries apply after the plan's own arguments, in their stored order.
    let mut command = match custom.hooks.wrapper.as_deref() {
        Some(wrapper) => {
            let (program, args) = palantir_loader::hooks::wrap(wrapper, &plan.java, &rest)
                .map_err(|error| format!("Wrapper hook could not run: {error}"))?;
            let mut command = std::process::Command::new(program);
            command.args(args);
            command
        }
        None => {
            let mut command = std::process::Command::new(&plan.java);
            command.args(&rest);
            command
        }
    };
    command.current_dir(&plan.dir);
    custom.apply(&mut command);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    if engine.shutdown.load(Ordering::SeqCst) {
        return Err("The launcher closed before the game started.".into());
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Cannot start Java: {e}. Check the executable in Settings."))?;
    if let Some(stdout) = child.stdout.take() {
        read_output(stdout, events.clone(), id.into());
    }
    if let Some(stderr) = child.stderr.take() {
        read_output(stderr, events.clone(), id.into());
    }
    let child = Arc::new(Mutex::new(child));
    engine
        .processes
        .lock()
        .map_err(|e| e.to_string())?
        .insert(id.into(), Arc::clone(&child));
    emit(events, Event::Running(id.into(), instance.name.clone()));
    let engine = Arc::clone(engine);
    let events = events.clone();
    let id = id.to_string();
    // The post-exit hook belongs to the exit itself, not to how the exit
    // happened: it fires on a clean exit, a crash, and a monitoring failure
    // alike, and a hook that fails is reported, never swallowed.
    let post_exit = custom.hooks.post_exit.clone();
    let post_env = custom.env.clone();
    let post_dir = engine.root.game_dir(&id);
    std::thread::spawn(move || {
        loop {
            let status = child
                .lock()
                .map_err(|e| e.to_string())
                .and_then(|mut child| child.try_wait().map_err(|e| e.to_string()));
            match status {
                Ok(Some(status)) => {
                    if let Ok(mut processes) = engine.processes.lock() {
                        processes.remove(&id);
                    }
                    run_post_exit(&post_exit, &post_dir, &post_env, &events);
                    emit(
                        &events,
                        Event::Exited(id, format!("{} exited: {status}", instance.name)),
                    );
                    break;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(150)),
                Err(error) => {
                    run_post_exit(&post_exit, &post_dir, &post_env, &events);
                    emit(
                        &events,
                        Event::Error(format!("Cannot monitor the game: {error}")),
                    );
                    break;
                }
            }
        }
    });
    Ok("Game started. Open Logs for its output.".into())
}
/// The identity a Microsoft-account launch injects, from the stored session.
///
/// An expired session is renewed once, because the player already signed in and
/// the refresh token is exactly the thing that avoids asking them for a device
/// code again. A refresh that cannot happen is refused by name: the alternative
/// is handing the game a *different* player than the one the screen said was
/// playing, which is the silent swap this launcher refuses everywhere else.
fn authenticated_identity(engine: &Engine) -> Result<(String, String, String)> {
    let path = session::session_file(&engine.root);
    let mut stored = session::load(&path)
        .map_err(|e| e.to_string())?
        .ok_or_else(no_stored_session_message)?;
    if let Some(refresh) = session_renewal(Some(&stored), unix_now())? {
        let client_id = if stored.client_id.trim().is_empty() {
            std::env::var("PALANTIR_MSA_CLIENT_ID").unwrap_or_default()
        } else {
            stored.client_id.clone()
        };
        if client_id.trim().is_empty() {
            return Err("The stored Microsoft session has expired and no client id is registered to renew it. Set PALANTIR_MSA_CLIENT_ID and sign in again.".into());
        }
        let tokens = auth::refresh_session(
            &engine.http,
            auth::endpoints::TOKEN,
            client_id.trim(),
            &refresh,
        )
        .map_err(|error| {
            format!("The stored Microsoft session could not be renewed: {error}. Sign in again.")
        })?;
        stored.access_token = tokens.access_token;
        // A rotated refresh token replaces the used one; an answer that offers
        // none keeps the token that just worked.
        if let Some(rotated) = tokens.refresh_token {
            stored.refresh_token = Some(rotated);
        }
        stored.client_id = client_id.trim().to_string();
        stored.expires_at = session::StoredSession::expires_at(unix_now(), tokens.expires_in);
        session::save(&path, &stored).map_err(|error| {
            format!("The renewed Microsoft session could not be stored: {error}")
        })?;
    }
    let (uuid, name) = stored.identity();
    Ok((name, uuid, stored.access_token))
}

/// What a Microsoft-account launch must do with the session it found: renew it,
/// use it, or refuse because there is nothing usable.
///
/// Pure, so all of it is testable without a network, a window or a clock: the
/// answers are genuinely different states, and merging any two of them is how a
/// launcher ends up launching with an identity it does not have.
fn session_renewal(stored: Option<&session::StoredSession>, now: u64) -> Result<Option<String>> {
    let Some(stored) = stored else {
        return Err(no_stored_session_message());
    };
    if !stored.is_expired(now) {
        return Ok(None);
    }
    match stored.refresh_token.as_deref() {
        Some(token) if !token.trim().is_empty() => Ok(Some(token.to_string())),
        _ => Err(expired_session_message()),
    }
}

/// What a Microsoft-account launch says when no session is stored.
fn no_stored_session_message() -> String {
    "No Microsoft account session is stored. Sign in first; this launcher never quietly falls back to an offline identity."
        .to_string()
}

/// What it says when the stored session is expired and cannot be renewed.
fn expired_session_message() -> String {
    "The stored Microsoft session has expired and carries no refresh token. Sign in again; this launcher never quietly falls back to an offline identity."
        .to_string()
}

/// Unix seconds, saturating: the launcher's own clock is only ever compared
/// against a service's lifetime, and a clock set before 1970 must not panic a
/// launch over a session that simply reads as expired.
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

/// Every release a Forge-family vendor publishes for one game version, newest
/// first, read from the vendor's own Maven metadata.
///
/// Maven's own document is used rather than either vendor's website because it
/// is the document the installer jars themselves are published under: a version
/// it names is a version whose installer can be fetched. It goes through the
/// metadata cache like every other small document the launcher reads twice.
fn vendor_releases(engine: &Engine, vendor: forge::Vendor, game: &str) -> Result<Vec<String>> {
    let key = format!(
        "desktop-loader-list-{}-{game}",
        vendor.name().to_lowercase()
    );
    let text = engine
        .cache
        .fetch_text(&engine.http, &key, &vendor.metadata_url(), MANIFEST_TTL)
        .map_err(|e| e.to_string())?;
    Ok(forge::versions_for_game(&text, vendor, game))
}

/// The release to install when a collection pins none: the newest the vendor
/// lists for its game.
///
/// A pin is a decision the player already made, so it is used as given. An
/// empty listing is refused by name -- installing "the latest" of nothing is not
/// a fallback this launcher may invent.
fn newest_vendor_release(engine: &Engine, vendor: forge::Vendor, game: &str) -> Result<String> {
    vendor_releases(engine, vendor, game)?
        .into_iter()
        .next()
        .ok_or_else(|| {
            format!(
                "{} publishes no release for Minecraft {game}. Choose a game version the loader supports.",
                vendor.name()
            )
        })
}

/// Install what a collection has not installed yet: the resolved document, and
/// the processor plan its loader's installer asked for.
///
/// The Forge family's installer jar is staged under the data root's `versions/`
/// directory and has to survive until the processors have run: two of the plan's
/// arguments name that file, and the profile's packaged `data` files are staged
/// beside it, so a directory that vanished between planning and running would
/// leave the plan pointing at nothing.
fn install_version(
    engine: &Arc<Engine>,
    instance: &Instance,
    id: &str,
) -> Result<(Version, Vec<installer::PlannedProcessor>)> {
    match instance.loader {
        Loader::Vanilla => {
            let version = install::resolve_version(
                &engine.http,
                &engine.cache,
                install::VERSION_MANIFEST_URL,
                &instance.game_version,
            )
            .map_err(|e| e.to_string())?;
            let installed = install::install_document(
                &engine.syncer(),
                &engine.cache,
                install::VERSION_MANIFEST_URL,
                &platform(),
                &version.to_json_string().map_err(|e| e.to_string())?,
                Some(&engine.root.game_dir(id)),
            )
            .map_err(|e| e.to_string())?;
            Ok((installed.version, Vec::new()))
        }
        Loader::Fabric | Loader::Quilt => {
            let sources = profiles::LoaderSources::vendor(if instance.loader == Loader::Fabric {
                profiles::LoaderKind::Fabric
            } else {
                profiles::LoaderKind::Quilt
            });
            let installed = profiles::install_loader(
                &engine.syncer(),
                &engine.cache,
                install::VERSION_MANIFEST_URL,
                profiles::LoaderInstall {
                    sources: &sources,
                    game: &instance.game_version,
                    loader: instance.loader_version.as_deref(),
                    game_dir: Some(&engine.root.game_dir(id)),
                },
                &platform(),
            )
            .map_err(|e| e.to_string())?;
            Ok((installed.version, Vec::new()))
        }
        Loader::Forge | Loader::NeoForge => {
            let vendor = instance
                .loader
                .installer_vendor()
                .ok_or("This loader has no vendor installer.")?;
            let release = match instance.loader_version.as_deref() {
                Some(pinned) if !pinned.trim().is_empty() => pinned.trim().to_string(),
                _ => newest_vendor_release(engine, vendor, &instance.game_version)?,
            };
            let staged = forge::install(
                &engine.syncer(),
                &engine.cache,
                install::VERSION_MANIFEST_URL,
                forge::VendorInstall {
                    vendor,
                    version: &release,
                    game_dir: Some(&engine.root.game_dir(id)),
                    // The vendor's own Maven; only a test substitutes a base.
                    base: None,
                },
                &platform(),
                &engine.root.versions_dir(),
            )
            .map_err(|e| e.to_string())?;
            Ok((staged.report.version, staged.processors))
        }
    }
}

fn read_output<R: std::io::Read + Send + 'static>(
    stream: R,
    events: mpsc::Sender<Event>,
    id: String,
) {
    std::thread::spawn(move || {
        for line in BufReader::new(stream).lines() {
            match line {
                Ok(line) => emit(&events, Event::Log(id.clone(), line)),
                Err(error) => {
                    emit(
                        &events,
                        Event::Log(id.clone(), format!("Log stream error: {error}")),
                    );
                    break;
                }
            }
        }
    });
}
fn stop(engine: &Engine, id: &str) -> Result<()> {
    let child = engine
        .processes
        .lock()
        .map_err(|e| e.to_string())?
        .get(id)
        .cloned();
    if let Some(child) = child {
        child
            .lock()
            .map_err(|e| e.to_string())?
            .kill()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn runtime(engine: &Engine, version: &Version) -> Result<PathBuf> {
    let component = version
        .java_version
        .as_ref()
        .map(|v| v.component.as_str())
        .unwrap_or("jre-legacy");
    let platform = platform();
    let choice = java::RuntimeChoice {
        platform: java::mojang_platform(&platform).map_err(|e| e.to_string())?,
        component,
    };
    let index = java::parse_index(
        &engine
            .cache
            .fetch_text(
                &engine.http,
                "java-runtime-all",
                java::JAVA_RUNTIME_ALL_URL,
                MANIFEST_TTL,
            )
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let entry = java::select(&index, &choice).map_err(|e| e.to_string())?;
    let manifest = java::RuntimeManifest::parse(
        &engine
            .cache
            .fetch_text(
                &engine.http,
                &format!("java-{}-{component}", choice.platform),
                &entry.manifest.url,
                MANIFEST_TTL,
            )
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let into = java::runtime_dir(&engine.root.java_dir(), &choice);
    java::fetch_runtime(&engine.http, &manifest, &into, DownloadOptions::default())
        .map_err(|e| e.to_string())?;
    Ok(into
        .join("bin")
        .join(if cfg!(windows) { "java.exe" } else { "java" }))
}

pub fn open_url(url: &str) -> Result<()> {
    if !url.starts_with("https://") || url.contains(['\r', '\n', '\0', '"']) || url.len() > 4096 {
        return Err("Only ordinary HTTPS links can be opened.".into());
    }
    let mut command = if cfg!(windows) {
        let mut c = std::process::Command::new("rundll32.exe");
        c.arg("url.dll,FileProtocolHandler");
        c
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else {
        std::process::Command::new("xdg-open")
    };
    command
        .arg(url)
        .spawn()
        .map_err(|e| format!("Cannot open the browser: {e}"))?;
    Ok(())
}
fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}
fn get_json<T: serde::de::DeserializeOwned>(http: &Http, url: &str) -> Result<T> {
    let response = http.get(url).map_err(|e| e.to_string())?;
    if !response.is_success() {
        return Err(format!("Service returned HTTP {}", response.status()));
    }
    serde_json::from_str(&response.text().map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
fn search(
    http: &Http,
    query: &str,
    kind: ProjectType,
    sort: Sort,
    offset: usize,
    game: &str,
) -> Result<(Vec<Project>, usize)> {
    let mut facets = vec![vec![format!("project_type:{}", kind.api_name())]];
    if !game.trim().is_empty() {
        model::safe_component(game.trim())?;
        facets.push(vec![format!("versions:{}", game.trim())]);
    }
    let facets = serde_json::to_string(&facets).map_err(|e| e.to_string())?;
    let url = format!(
        "https://api.modrinth.com/v2/search?query={}&facets={}&index={}&limit=12&offset={offset}",
        encode(query),
        encode(&facets),
        sort.api_name()
    );
    let response: SearchResponse = get_json(http, &url)?;
    Ok((response.hits, response.total_hits))
}
fn resolve_content_plan(
    engine: &Engine,
    id: &str,
    project: &Project,
    release_id: &str,
) -> Result<model::ContentPlan> {
    if engine.busy(id)? {
        return Err("Stop this instance before installing content.".into());
    }
    let instance = engine.instance(id)?;
    if project.project_type == "modpack" {
        return Err("A modpack becomes its own collection rather than being installed into this one: a pack must not overwrite an existing world's files. Configure it from Explore's pack setup, which imports the archive and lets you choose its optional files.".into());
    }
    if project.project_type == "mod" && instance.loader == Loader::Vanilla {
        return Err(
            "Mods require a Fabric or Quilt instance. Create one with the matching loader.".into(),
        );
    }
    model::safe_component(&project.project_id)?;
    let games = serde_json::to_string(&[&instance.game_version]).map_err(|e| e.to_string())?;
    let mut url = format!(
        "https://api.modrinth.com/v2/project/{}/version?game_versions={}",
        project.project_id,
        encode(&games)
    );
    if project.project_type == "mod" {
        let loaders =
            serde_json::to_string(&[instance.loader.api_name()]).map_err(|e| e.to_string())?;
        url.push_str(&format!("&loaders={}", encode(&loaders)));
    }
    let versions: Vec<model::Release> = get_json(&engine.http, &url)?;
    let version = versions.iter().find(|r| r.id == release_id).ok_or(
        "The selected release is not compatible with this collection. Choose another release.",
    )?;
    if version.project_id != project.project_id
        || !version.compatible(&instance, &project.project_type)
    {
        return Err("Release compatibility changed; review the game and loader again.".into());
    }
    let folder = match project.project_type.as_str() {
        "mod" => "mods",
        "resourcepack" => "resourcepacks",
        "shader" => "shaderpacks",
        _ => return Err("Unsupported content type.".into()),
    };
    // The release, then everything it requires. Both are planned before the
    // first fetch, so a release whose library has no compatible build for this
    // collection refuses the whole install instead of landing half of it.
    let mut lookup = |key: &str| lookup_versions(engine, key);
    let planned = plan_content_files(&instance, project, version, folder, &mut lookup)?;
    validate_content_destinations(&engine.root, id, planned).map(|files| model::ContentPlan {
        instance_id: id.into(),
        instance_name: instance.name,
        project_id: project.project_id.clone(),
        release_id: version.id.clone(),
        game_version: instance.game_version,
        loader: instance.loader,
        files,
    })
}

fn plan_content_files(
    instance: &Instance,
    project: &Project,
    version: &model::Release,
    folder: &str,
    lookup: &mut dyn FnMut(&str) -> Result<Vec<serde_json::Value>>,
) -> Result<Vec<model::ContentFile>> {
    let mut planned = vec![planned_primary(version, folder)?];
    if version.required_dependencies() {
        let root = release_document(project, version);
        let mut lookup = |key: &str| -> palantir_loader::error::Result<Vec<serde_json::Value>> {
            lookup(key).map_err(|why| palantir_loader::error::Error::Invalid {
                what: "dependency lookup",
                why,
            })
        };
        let graph = dependencies::resolve_plan(
            &root,
            &mut lookup,
            &instance.game_version,
            instance.loader.api_name(),
        )
        .map_err(|e| e.to_string())?;
        for file in graph {
            planned.push(planned_dependency(file, folder)?);
        }
    }
    let mut names = std::collections::BTreeSet::new();
    for item in &planned {
        if !names.insert((item.folder.clone(), item.file_name.to_lowercase())) {
            return Err(format!(
                "{} is named by more than one release; nothing was installed.",
                item.file_name
            ));
        }
    }
    Ok(planned)
}

fn validate_content_destinations(
    root: &DataRoot,
    id: &str,
    mut planned: Vec<model::ContentFile>,
) -> Result<Vec<model::ContentFile>> {
    let game = crate::files::game_directory(root, id)?;
    for item in &mut planned {
        item.already_present = false;
        let directory = game.join(&item.folder);
        match std::fs::symlink_metadata(&directory) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err("Refusing a linked or non-directory content folder.".into());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        let destination = directory.join(&item.file_name);
        match std::fs::symlink_metadata(&destination) {
            Ok(metadata) => {
                if !metadata.is_file() || metadata.file_type().is_symlink() {
                    return Err("Refusing a linked or non-file content destination.".into());
                }
                let hash =
                    palantir_net::download::sha1_file(&destination).map_err(|e| e.to_string())?;
                if hash != item.sha1 || item.size.is_some_and(|size| size != metadata.len()) {
                    return Err(format!(
                        "{} already exists with different bytes; nothing will overwrite it.",
                        item.file_name
                    ));
                }
                item.already_present = true;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(planned)
}

fn install_content(
    engine: &Engine,
    events: &mpsc::Sender<Event>,
    id: &str,
    project: &Project,
    reviewed: &model::ContentPlan,
) -> Result<String> {
    let current = resolve_content_plan(engine, id, project, &reviewed.release_id)?;
    require_reviewed_plan(reviewed, &current)?;
    let game = crate::files::game_directory(&engine.root, id)?;
    emit(
        events,
        Event::Status(
            id.into(),
            format!(
                "Downloading {} file(s) for {}…",
                current
                    .files
                    .iter()
                    .filter(|file| !file.already_present)
                    .count(),
                project.title
            ),
        ),
    );
    let mut staged: Vec<(PathBuf, PathBuf)> = Vec::new();
    for (index, item) in current
        .files
        .iter()
        .enumerate()
        .filter(|(_, file)| !file.already_present)
    {
        let destination = game.join(&item.folder).join(&item.file_name);
        let staging = engine
            .root
            .instance_dir(id)
            .join(format!("content-{index}-{}.download", item.sha1));
        if let Err(error) = download(
            &engine.http,
            &item.url,
            &staging,
            Some(item.sha1.as_str()),
            item.size,
            &DownloadOptions::default(),
        ) {
            // The transfer owns its resumable partial file; completed staging
            // files from this operation must not survive a failed set.
            remove_content_staging(&staged)?;
            return Err(error.to_string());
        }
        staged.push((destination, staging));
    }
    // Recheck after transfers, too: a user may have added/replaced a file while
    // the window was downloading. Installation never silently overwrites it.
    let fresh = validate_content_destinations(&engine.root, id, current.files.clone());
    match fresh {
        Ok(files) if files == current.files => {}
        result => {
            remove_content_staging(&staged)?;
            return Err(result.err().unwrap_or_else(|| {
                "Content changed during download. Review the installation again.".into()
            }));
        }
    }
    commit_content_files(&staged)?;
    Ok(install_note(
        &project.title,
        &current.instance_name,
        current.files.len(),
    ))
}

fn require_reviewed_plan(
    reviewed: &model::ContentPlan,
    current: &model::ContentPlan,
) -> Result<()> {
    if reviewed != current {
        return Err("The release, dependencies, or destination changed. Review the installation again; nothing was downloaded.".into());
    }
    Ok(())
}

// Exclusive creates avoid rename's silent overwrite semantics. On failure only
// files created by this operation are rolled back; existing content is retained.
fn commit_content_files(staged: &[(PathBuf, PathBuf)]) -> Result<()> {
    let mut created = Vec::new();
    let result = (|| -> Result<()> {
        for (destination, staging) in staged {
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut source = std::fs::File::open(staging).map_err(|e| e.to_string())?;
            let mut target = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(destination)
                .map_err(|e| format!("Cannot place {}: {e}", destination.display()))?;
            created.push(destination);
            std::io::copy(&mut source, &mut target).map_err(|e| e.to_string())?;
            target.sync_all().map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    if result.is_err() {
        for path in created {
            std::fs::remove_file(path).map_err(|e| format!("Install failed and could not remove {}: {e}. Inspect the content folder before retrying.", path.display()))?;
        }
    }
    remove_content_staging(staged)?;
    result
}
fn remove_content_staging(staged: &[(PathBuf, PathBuf)]) -> Result<()> {
    for (_, staging) in staged {
        std::fs::remove_file(staging).map_err(|e| {
            format!(
                "Could not remove download staging file {}: {e}",
                staging.display()
            )
        })?;
    }
    Ok(())
}

/// Emit a failed sign-in unless the player already cancelled. A cancellation
/// must not be answered with "try again": the state the player asked for is
/// the one they get.
fn report_auth_failure(events: &mpsc::Sender<Event>, cancel: &AtomicBool, why: String) {
    if cancel.load(Ordering::SeqCst) {
        return;
    }
    emit(
        events,
        Event::Authentication(model::Authentication::Failed(why)),
    );
}
/// What a failure in the sign-in chain means to the player.
///
/// Ownership is the one failure they can act on by buying the game, so it must
/// not read like the network failing -- and a network failure must not read
/// like a purchase problem, which would send them to a store page they do not
/// need.
fn sign_in_message(error: &palantir_net::Error) -> String {
    if auth::is_not_owned(error) {
        "This Microsoft account does not own Minecraft. Sign in with the account that bought the game, or buy it on this account.".into()
    } else {
        format!("Signing in did not finish: {error}")
    }
}
/// The blocking sign-in chain, on its own thread so the window keeps drawing.
///
/// The stop flag is read before every emission: a cancelled sign-in must not
/// report a result the player already walked away from.
fn sign_in(engine: &Engine, events: &mpsc::Sender<Event>, client_id: &str, cancel: &AtomicBool) {
    let stopped = || cancel.load(Ordering::SeqCst);
    emit(
        events,
        Event::Authentication(model::Authentication::Requesting),
    );
    // `offline_access` is what makes the token refreshable; without it the
    // session cannot outlive the access token's own lifetime.
    let code = match auth::request_device_code(
        &engine.http,
        auth::endpoints::DEVICE_CODE,
        client_id,
        "XboxLive.signin offline_access",
    ) {
        Ok(code) => code,
        Err(error) => {
            return report_auth_failure(
                events,
                cancel,
                format!("Sign-in could not start: {error}"),
            );
        }
    };
    if stopped() {
        return;
    }
    emit(
        events,
        Event::Authentication(model::Authentication::DeviceCode {
            code: code.user_code.clone(),
            url: code.verification_uri.clone(),
            expires_in: code.expires_in,
        }),
    );
    // The service's own interval, grown by five seconds whenever it asks for a
    // slower poll and bounded by the code's own lifetime.
    let mut wait = Duration::from_secs(code.interval.max(1));
    let deadline = Instant::now() + Duration::from_secs(code.expires_in.max(1));
    let msa_token = loop {
        if stopped() {
            return;
        }
        if Instant::now() >= deadline {
            emit(
                events,
                Event::Authentication(model::Authentication::Expired),
            );
            return;
        }
        std::thread::sleep(wait);
        if stopped() {
            return;
        }
        match auth::poll_token(
            &engine.http,
            auth::endpoints::TOKEN,
            client_id,
            &code.device_code,
        ) {
            Ok(auth::Poll::Token(tokens)) => break tokens,
            Ok(auth::Poll::Pending) => {}
            Ok(auth::Poll::SlowDown) => wait += Duration::from_secs(5),
            Ok(auth::Poll::Expired) => {
                emit(
                    events,
                    Event::Authentication(model::Authentication::Expired),
                );
                return;
            }
            Ok(auth::Poll::Denied) => {
                return report_auth_failure(
                    events,
                    cancel,
                    "The sign-in request was declined. No account was changed.".into(),
                );
            }
            Err(error) => {
                return report_auth_failure(
                    events,
                    cancel,
                    format!("Sign-in polling failed: {error}"),
                );
            }
        }
    };
    if stopped() {
        return;
    }
    emit(
        events,
        Event::Authentication(model::Authentication::Verifying),
    );
    let player = (|| -> std::result::Result<String, String> {
        let user = auth::xbox_user_token(
            &engine.http,
            &msa_token.access_token,
            auth::endpoints::XBL_USER,
        )
        .map_err(|error| sign_in_message(&error))?;
        let xsts = auth::xbox_xsts_token(&engine.http, &user, auth::endpoints::XSTS)
            .map_err(|error| sign_in_message(&error))?;
        let mc_session = auth::minecraft_login(&engine.http, &xsts, auth::endpoints::MC_LOGIN)
            .map_err(|error| sign_in_message(&error))?;
        let profile =
            auth::minecraft_profile(&engine.http, &mc_session, auth::endpoints::MC_PROFILE)
                .map_err(|error| sign_in_message(&error))?;
        // The session is stored before the player is told they are signed in: a
        // Microsoft-account launch reads this file, so reporting a sign-in whose
        // session did not land would be a promise the next click cannot keep.
        // The refresh token is kept because it is the only thing that lets the
        // session outlive the access token without asking for a new code.
        session::save(
            &session::session_file(&engine.root),
            &session::StoredSession {
                player_name: profile.name.clone(),
                player_uuid: profile.id.clone(),
                access_token: mc_session.access_token.clone(),
                refresh_token: msa_token.refresh_token.clone(),
                client_id: client_id.to_string(),
                expires_at: session::StoredSession::expires_at(unix_now(), mc_session.expires_in),
            },
        )
        .map_err(|error| format!("Signed in, but the session could not be stored: {error}"))?;
        Ok(profile.name)
    })();
    if stopped() {
        return;
    }
    match player {
        Ok(player) => emit(
            events,
            Event::Authentication(model::Authentication::Connected { player }),
        ),
        Err(why) => report_auth_failure(events, cancel, why),
    }
}
/// One file an install will place, with every check already made: an install
/// that discovers a problem while writing is an install that stops halfway.
fn fetch_catalogue_pack(
    engine: &Engine,
    project: &Project,
    release: &model::Release,
) -> Result<model::ImportReview> {
    if project.project_type != "modpack" || release.project_id != project.project_id {
        return Err("Choose a release belonging to this modpack.".into());
    }
    let file = planned_primary(release, "")?;
    if !file.file_name.to_ascii_lowercase().ends_with(".mrpack")
        || file
            .size
            .is_none_or(|size| size == 0 || size > 512 * 1024 * 1024)
    {
        return Err("Choose a .mrpack archive between 1 byte and 512 MiB.".into());
    }
    fetch_pack_file(engine, &file)
}
fn fetch_pack_file(engine: &Engine, file: &model::ContentFile) -> Result<model::ImportReview> {
    let _writer = engine.install_lock.lock().map_err(|e| e.to_string())?;
    let cache = engine.root.root.join("pack-archives");
    match std::fs::create_dir(&cache) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.to_string()),
    }
    if !crate::local::checked(&cache)?.is_dir() {
        return Err("Pack cache is not a directory.".into());
    }
    let destination = cache.join(format!("{}.mrpack", file.sha1));
    for path in [
        &destination,
        &palantir_net::download::part_path(&destination),
    ] {
        match std::fs::symlink_metadata(path) {
            Ok(_) => {
                if !crate::local::checked(path)?.is_file() {
                    return Err("Pack cache entry is not a regular file.".into());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    download(
        &engine.http,
        &file.url,
        &destination,
        Some(&file.sha1),
        file.size,
        &DownloadOptions::default(),
    )
    .map_err(|e| e.to_string())?;
    crate::files::inspect_source(&destination)
}
fn planned_primary(version: &model::Release, folder: &str) -> Result<model::ContentFile> {
    let file = version
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| version.files.first())
        .ok_or("This release contains no downloadable file.")?;
    crate::files::local_name(&file.filename)?;
    Ok(model::ContentFile {
        already_present: false,
        url: content_url(&file.url, &file.filename)?,
        sha1: content_hash(file.hashes.get("sha1").map(String::as_str), &file.filename)?,
        size: Some(file.size),
        file_name: file.filename.clone(),
        folder: folder.to_string(),
    })
}
fn planned_dependency(
    file: dependencies::ResolvedFile,
    folder: &str,
) -> Result<model::ContentFile> {
    crate::files::local_name(&file.file_name)?;
    let url = file.url.ok_or_else(|| {
        format!(
            "{} names no download; nothing was installed.",
            file.file_name
        )
    })?;
    Ok(model::ContentFile {
        already_present: false,
        url: content_url(&url, &file.file_name)?,
        sha1: content_hash(file.sha1.as_deref(), &file.file_name)?,
        size: file.size,
        file_name: file.file_name,
        folder: folder.to_string(),
    })
}
/// A content URL, refused unless it is the CDN the catalogue serves from: a
/// release that points somewhere else is not something this launcher fetches.
fn content_url(url: &str, file_name: &str) -> Result<String> {
    if !url.starts_with("https://cdn.modrinth.com/") {
        return Err(format!(
            "{file_name} points outside the supported secure content CDN."
        ));
    }
    Ok(url.to_string())
}
/// A verification hash, refused when absent or malformed: an unverified file is
/// the one thing a launcher must never call installed.
fn content_hash(hash: Option<&str>, file_name: &str) -> Result<String> {
    let hash = hash.ok_or_else(|| format!("{file_name} has no SHA-1 verification hash."))?;
    if hash.len() != 40 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("{file_name} has an invalid verification hash."));
    }
    Ok(hash.to_string())
}
/// The version document the dependency walk reads, rebuilt from the release the
/// catalogue already answered with: asking again for a document we hold would
/// be a second source of truth for the same metadata.
fn release_document(project: &Project, version: &model::Release) -> serde_json::Value {
    serde_json::json!({
        "project_id": project.project_id,
        "dependencies": version
            .dependencies
            .iter()
            .map(|dependency| serde_json::json!({
                "dependency_type": dependency.dependency_type,
                "project_id": dependency.project_id,
                "version_id": dependency.version_id,
                "file_name": dependency.file_name,
            }))
            .collect::<Vec<_>>(),
    })
}
/// Every version document the catalogue lists for the id a dependency named.
///
/// The project endpoint is asked first because a project is what a dependency
/// normally names; a dependency that named only a version is answered by the
/// version endpoint, and an id that is neither reports the project endpoint's
/// own failure rather than a made-up one.
fn lookup_versions(engine: &Engine, id: &str) -> Result<Vec<serde_json::Value>> {
    model::safe_component(id)?;
    match get_json::<Vec<serde_json::Value>>(
        &engine.http,
        &format!("https://api.modrinth.com/v2/project/{id}/version"),
    ) {
        Ok(versions) => Ok(versions),
        Err(project_error) => get_json::<serde_json::Value>(
            &engine.http,
            &format!("https://api.modrinth.com/v2/version/{id}"),
        )
        .map(|version| vec![version])
        .map_err(|_| project_error),
    }
}
/// What the player is told after an install.
///
/// The count is named when there is one: "installed" without saying that three
/// libraries arrived with it hides what is now in the folder, and a mod that
/// silently brought its library is a state the player cannot reason about
/// later.
fn install_note(title: &str, instance: &str, files: usize) -> String {
    let dependencies = files.saturating_sub(1);
    match dependencies {
        0 => format!("Installed {title} into {instance}. Restart the game to load it."),
        1 => format!(
            "Installed {title} and 1 required dependency into {instance}. Restart the game to load them."
        ),
        count => format!(
            "Installed {title} and {count} required dependencies into {instance}. Restart the game to load them."
        ),
    }
}
/// Whether a pack's index carries files marked optional for the client.
///
/// This is the fact; what to do about it is [`pack_selection`]'s decision.
fn optional_client_files(index: &serde_json::Value) -> bool {
    index
        .get("files")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|files| {
            files.iter().any(|file| {
                file.get("env")
                    .and_then(|env| env.get("client"))
                    .and_then(serde_json::Value::as_str)
                    == Some("optional")
            })
        })
}
/// The optional-file choice a pack install is allowed to use.
///
/// `selected` is the review screen's answer: `Some` means the pack's optional
/// files were shown and decided (an empty list being "none of them"), `None`
/// means they were never shown at all. Installing the required set under a
/// screen that never mentioned the optional ones is the silent-wrong-install
/// this refuses, so `None` is a refusal whenever the pack has any -- while an
/// explicit empty selection is a decision and installs.
fn pack_selection(has_optional: bool, selected: Option<&[String]>) -> Result<&[String]> {
    match (has_optional, selected) {
        (true, None) => Err(pack_optional_choice_message()),
        (_, Some(chosen)) => Ok(chosen),
        (false, None) => Ok(&[]),
    }
}

/// What a pack install says when its optional client files were never shown.
fn pack_optional_choice_message() -> String {
    "This pack carries optional client files. Review them and choose before installing; nothing was created."
        .to_string()
}

/// A pack path joined onto the game directory, refused when it would leave it.
///
/// The planner checked this already; checking again where the write happens is
/// the difference between a bug and a file outside the collection.
fn pack_destination(game_dir: &Path, path: &str) -> Result<PathBuf> {
    let candidate = Path::new(path);
    let leaves = candidate.is_absolute()
        || candidate
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir));
    if leaves {
        return Err(format!(
            "{path} would leave the game directory; nothing was installed."
        ));
    }
    Ok(game_dir.join(candidate))
}
/// The override files an archive actually carries, as (archive entry, path in
/// the game directory), in application order.
///
/// The layers are base-first and a later layer's file is the one that lands, so
/// a file carried in both layers is placed once, by the later one.
fn override_files(names: &[String], plan: &mrpack::PackPlan) -> Vec<(String, String)> {
    let mut placed: Vec<(String, String)> = Vec::new();
    for layer in &plan.overrides {
        let prefix = format!("{}/", layer.source);
        for name in names {
            let Some(rest) = name.strip_prefix(&prefix) else {
                continue;
            };
            if rest.is_empty() || rest.ends_with('/') {
                continue;
            }
            let relative = if layer.destination.is_empty() {
                rest.to_string()
            } else {
                format!("{}/{}", layer.destination, rest)
            };
            placed.retain(|(_, existing)| existing != &relative);
            placed.push((name.clone(), relative));
        }
    }
    placed
}
/// Install a `.mrpack` archive into a new collection: the index's own client
/// file list, fetched and verified, with the archive's override layers applied
/// over it.
///
/// The pack is refused rather than half-installed when any of its files cannot
/// be placed, because a collection holding half a pack is a state the player
/// cannot see. The game version and loader are the reviewed ones the request
/// carries, since the review already read this archive; a loader pin is
/// resolved on first play the way every other imported collection resolves one.
///
/// `selected` is the optional-file choice the review screen made, in the sense
/// [`pack_selection`] defines: the pack's optional client files are installed
/// exactly when the player was shown them and chose.
fn install_pack_archive(
    engine: &Engine,
    events: &mpsc::Sender<Event>,
    source: &str,
    name: &str,
    game: &str,
    loader: Loader,
    selected: Option<&[String]>,
) -> Result<String> {
    let file = std::fs::File::open(source).map_err(|e| format!("Pack archive: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Pack archive: {e}"))?;
    let mut index = archive
        .by_name("modrinth.index.json")
        .map_err(|e| format!("Pack archive: {e}"))?;
    if index.size() > 8 * 1024 * 1024 {
        return Err("Pack index exceeds the 8 MiB install limit.".into());
    }
    let mut text = String::new();
    std::io::Read::read_to_string(&mut index, &mut text).map_err(|e| e.to_string())?;
    drop(index);
    let index_value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Pack index: {e}"))?;
    let chosen = pack_selection(optional_client_files(&index_value), selected)?;
    let plan = mrpack::plan_files(&index_value, chosen).map_err(|e| e.to_string())?;
    let names = archive
        .file_names()
        .map(str::to_string)
        .collect::<Vec<String>>();
    let overrides = override_files(&names, &plan);
    // A record with no download is a file the pack expects to place from its own
    // overrides. A new collection cannot hold it otherwise, and saying so is
    // better than an install missing one of the pack's own files.
    for entry in &plan.files {
        if let mrpack::SelectedFile::Local { path } = entry {
            if !overrides.iter().any(|(_, placed)| placed == path) {
                return Err(format!(
                    "The pack lists {path} with no download and no override carries it; nothing was created."
                ));
            }
        }
    }
    let dependencies = index_value
        .get("dependencies")
        .and_then(serde_json::Value::as_object)
        .ok_or("Pack names no game dependencies.")?;
    let archive_game = dependencies
        .get("minecraft")
        .and_then(serde_json::Value::as_str)
        .ok_or("Pack names no Minecraft version.")?;
    if archive_game != game {
        return Err("Pack game version changed since review.".into());
    }
    let mut pins = Vec::new();
    for (key, value) in dependencies {
        if key == "minecraft" {
            continue;
        }
        let kind = match key.as_str() {
            "fabric-loader" => Loader::Fabric,
            "quilt-loader" => Loader::Quilt,
            "forge" => Loader::Forge,
            "neoforge" => Loader::NeoForge,
            _ => {
                return Err(format!(
                    "Unsupported pack dependency {key}; nothing was created."
                ));
            }
        };
        let pin = value.as_str().ok_or("Invalid loader pin.")?;
        model::safe_component(pin)?;
        pins.push((kind, pin.to_string()));
    }
    if pins.len() > 1
        || pins
            .first()
            .map(|(kind, _)| *kind)
            .unwrap_or(Loader::Vanilla)
            != loader
    {
        return Err("Pack loader differs from the reviewed selection.".into());
    }
    validate_pack_paths(&mut archive, &plan, &overrides)?;
    let mut item = model::create(&engine.root, name, game, loader)?;
    let outcome = (|| {
        item.loader_version = pins.first().map(|(_, pin)| pin.clone());
        model::save_instance(&engine.root, &item)?;
        let game_dir = engine.root.game_dir(&item.id);
        let mut staged: Vec<(PathBuf, PathBuf)> = Vec::new();
        for entry in &plan.files {
            let mrpack::SelectedFile::Download {
                path,
                url,
                sha1,
                size,
            } = entry
            else {
                continue;
            };
            let destination = pack_destination(&game_dir, path)?;
            if destination.exists() {
                return Err(format!(
                    "{path} would overwrite a file that is already there; nothing was installed."
                ));
            }
            emit(
                events,
                Event::Status(item.id.clone(), format!("Fetching {path}…")),
            );
            let staging = engine
                .root
                .instance_dir(&item.id)
                .join(format!("pack-{}.download", staged.len()));
            if let Err(error) = download(
                &engine.http,
                url,
                &staging,
                sha1.as_deref(),
                *size,
                &DownloadOptions::default(),
            ) {
                for (_, leftover) in &staged {
                    let _ = std::fs::remove_file(leftover);
                }
                return Err(error.to_string());
            }
            staged.push((destination, staging));
        }
        let files = staged.len();
        for (destination, staging) in staged {
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::rename(&staging, &destination).map_err(|e| e.to_string())?;
        }
        // The overrides come last and overwrite, which is the format's own
        // precedence: a file carried in both places must end as the pack's copy.
        let mut layers = 0usize;
        for (entry, relative) in &overrides {
            let destination = pack_destination(&game_dir, relative)?;
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut source_entry = archive.by_name(entry).map_err(|e| e.to_string())?;
            let mut out = std::fs::File::create(&destination).map_err(|e| e.to_string())?;
            std::io::copy(&mut source_entry, &mut out).map_err(|e| e.to_string())?;
            layers += 1;
        }
        Ok(format!(
            "Installed the pack as {}. {files} pack file(s) and {layers} override(s) are in place; the game itself installs on first play.",
            item.name
        ))
    })();
    if let Err(error) = outcome {
        std::fs::remove_dir_all(engine.root.instance_dir(&item.id)).map_err(|cleanup| {
            format!("{error}; incomplete collection cleanup failed: {cleanup}")
        })?;
        return Err(error);
    }
    outcome
}

fn validate_pack_paths(
    archive: &mut zip::ZipArchive<std::fs::File>,
    plan: &mrpack::PackPlan,
    overrides: &[(String, String)],
) -> Result<()> {
    let mut names = std::collections::BTreeSet::new();
    let mut bytes = 0u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|e| e.to_string())?;
        let name = entry.name().trim_end_matches('/');
        safe_pack_path(name)?;
        if !names.insert(name.to_lowercase())
            || entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err("Pack archive has duplicate or linked entries.".into());
        }
        bytes = bytes
            .checked_add(entry.size())
            .ok_or("Pack size overflow.")?;
    }
    let mut destinations = std::collections::BTreeMap::new();
    for file in &plan.files {
        let (path, size) = match file {
            mrpack::SelectedFile::Download {
                path, size, sha1, ..
            } => {
                content_hash(sha1.as_deref(), path)?;
                (path, size.unwrap_or(0))
            }
            mrpack::SelectedFile::Local { path } => (path, 0),
        };
        safe_pack_path(path)?;
        if destinations
            .insert(path.to_lowercase(), path.clone())
            .is_some()
        {
            return Err("Duplicate pack destinations.".into());
        }
        bytes = bytes.checked_add(size).ok_or("Pack size overflow.")?;
    }
    for (_, path) in overrides {
        safe_pack_path(path)?;
        if let Some(existing) = destinations.insert(path.to_lowercase(), path.clone()) {
            if existing != *path {
                return Err("Pack override has a case-conflicting destination.".into());
            }
        }
    }
    for key in destinations.keys() {
        let mut prefix = String::new();
        for part in key
            .split('/')
            .take(key.split('/').count().saturating_sub(1))
        {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            if destinations.contains_key(&prefix) {
                return Err("Pack file conflicts with a directory.".into());
            }
        }
    }
    if archive.len() > 100_000 || destinations.len() > 100_000 || bytes > 32 * 1024 * 1024 * 1024 {
        return Err("Pack exceeds 100,000 files or 32 GiB.".into());
    }
    Ok(())
}
fn safe_pack_path(path: &str) -> Result<()> {
    if path.is_empty()
        || path.contains('\\')
        || path.starts_with('/')
        || path.split('/').count() > 40
    {
        return Err("Unsafe pack path.".into());
    }
    for part in path.split('/') {
        crate::local::safe_name(part)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    // The pack tests below build real zips; `super::*` brings the reader half
    // of `std::io`, not the writer half.
    use std::io::Write as _;
    // The mock content service the byte-moving install tests fetch from.
    use test_support::{MockServer, Route};
    fn content_test_root(label: &str) -> DataRoot {
        let path = std::env::temp_dir().join(format!(
            "palantir-review-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        DataRoot::new(path)
    }
    fn content_file(name: &str, bytes: &[u8]) -> model::ContentFile {
        model::ContentFile {
            url: format!("https://cdn.modrinth.com/{name}"),
            file_name: name.into(),
            sha1: palantir_net::download::sha1_bytes(bytes),
            size: Some(bytes.len() as u64),
            folder: "mods".into(),
            already_present: false,
        }
    }
    #[test]
    fn catalogue_archive_download_reviews_optional_files_before_creating_a_collection() {
        let optional = b"optional content";
        let pack = sample_pack(
            "catalogue-review",
            &pack_index(vec![pack_file(
                "mods/extra.jar",
                optional,
                Some("optional"),
            )]),
            &[],
        );
        let bytes = std::fs::read(&pack.archive).unwrap();
        let server = MockServer::start(vec![("/pack.mrpack", Route::new(bytes.clone()))]);
        let engine = Engine::at(pack.root.clone()).unwrap();
        let file = model::ContentFile {
            url: server.url("/pack.mrpack"),
            file_name: "pack.mrpack".into(),
            sha1: palantir_net::download::sha1_bytes(&bytes),
            size: Some(bytes.len() as u64),
            folder: String::new(),
            already_present: false,
        };
        let review = fetch_pack_file(&engine, &file).unwrap();
        assert_eq!(review.optional, ["mods/extra.jar"]);
        assert_eq!(review.archive_sha1.as_deref(), Some(file.sha1.as_str()));
        assert_eq!(review.game, "1.20.1");
        assert!(model::load_instances(&pack.root).unwrap().is_empty());
        fetch_pack_file(&engine, &file).unwrap();
        assert_eq!(server.hits(), ["/pack.mrpack"]);
        std::fs::remove_dir_all(pack.root.root).unwrap();
    }
    #[test]
    fn pack_install_preserves_loader_pin_and_shared_hash_files() {
        let bytes = b"shared bytes";
        let server = MockServer::start(vec![("/same.jar", Route::new(bytes.to_vec()))]);
        let index = pack_index(vec![
            pack_file_from(&server.url("/same.jar"), "mods/a.jar", bytes, None),
            pack_file_from(&server.url("/same.jar"), "mods/b.jar", bytes, None),
        ]);
        let pack = sample_pack("shared-hash", &index, &[]);
        let installed = installed_pack(&pack, Some(&[]), &server, "Shared hash").unwrap();
        assert_eq!(
            std::fs::read(installed.game_dir.join("mods/a.jar")).unwrap(),
            bytes
        );
        assert_eq!(
            std::fs::read(installed.game_dir.join("mods/b.jar")).unwrap(),
            bytes
        );
        assert_eq!(
            model::load_instances(&pack.root).unwrap()[0]
                .loader_version
                .as_deref(),
            Some("0.15.0")
        );
        std::fs::remove_dir_all(pack.root.root).unwrap();
    }
    #[test]
    fn pack_overrides_cannot_escape_or_alias_a_destination() {
        for entries in [
            vec![("overrides/../escape", b"bad".as_slice())],
            vec![
                ("overrides/config/A", b"a".as_slice()),
                ("overrides/config/a", b"b".as_slice()),
            ],
        ] {
            let pack = sample_pack("bad-overrides", &pack_index(vec![]), &entries);
            let server = MockServer::start(vec![]);
            assert!(installed_pack(&pack, Some(&[]), &server, "Unsafe").is_err());
            assert!(model::load_instances(&pack.root).unwrap().is_empty());
            std::fs::remove_dir_all(pack.root.root).unwrap();
        }
    }
    #[test]
    fn content_plan_resolves_required_files_and_refuses_duplicate_destinations() {
        let item = Instance {
            id: "target".into(),
            name: "Target".into(),
            game_version: "1.20.1".into(),
            loader: Loader::Fabric,
            loader_version: None,
            window: None,
            memory_mb: None,
            java: None,
            custom: Default::default(),
            installed_version: None,
        };
        let project = Project {
            project_id: "demo".into(),
            title: "Demo".into(),
            description: String::new(),
            author: String::new(),
            downloads: 0,
            project_type: "mod".into(),
        };
        let mut release = a_release(vec![
            a_dependency("required", "api"),
            a_dependency("optional", "not-fetched"),
        ]);
        release.files.push(model::ReleaseFile {
            url: "https://cdn.modrinth.com/root.jar".into(),
            filename: "root.jar".into(),
            size: 4,
            primary: true,
            hashes: [("sha1".into(), "a".repeat(40))].into(),
        });
        let candidate = serde_json::json!({"id":"api-release","project_id":"api","game_versions":["1.20.1"],"loaders":["fabric"],"files":[{"filename":"api.jar","url":"https://cdn.modrinth.com/api.jar","hashes":{"sha1":"b".repeat(40)},"size":8}],"dependencies":[]});
        let mut calls = Vec::new();
        let plan = plan_content_files(&item, &project, &release, "mods", &mut |key| {
            calls.push(key.to_string());
            Ok(vec![candidate.clone()])
        })
        .unwrap();
        assert_eq!(calls, ["api"]);
        assert_eq!(
            plan.iter()
                .map(|file| file.file_name.as_str())
                .collect::<Vec<_>>(),
            ["root.jar", "api.jar"]
        );
        let mut duplicate = candidate.clone();
        duplicate["files"][0]["filename"] = "ROOT.jar".into();
        assert!(
            plan_content_files(&item, &project, &release, "mods", &mut |_| Ok(vec![
                duplicate.clone()
            ]))
            .unwrap_err()
            .contains("more than one")
        );
        assert!(
            plan_content_files(&item, &project, &release, "mods", &mut |_| Err(
                "service down".into()
            ))
            .unwrap_err()
            .contains("service down")
        );
    }
    #[test]
    fn existing_content_is_only_reused_when_its_bytes_match() {
        let root = content_test_root("existing");
        let item = model::create(&root, "Target", "1.20.1", Loader::Fabric).unwrap();
        let mods = root.game_dir(&item.id).join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        let file = content_file("api.jar", b"correct");
        std::fs::write(mods.join("api.jar"), b"correct").unwrap();
        let plan = validate_content_destinations(&root, &item.id, vec![file]).unwrap();
        assert!(plan[0].already_present);
        std::fs::remove_file(mods.join("api.jar")).unwrap();
        assert!(
            !validate_content_destinations(&root, &item.id, plan.clone()).unwrap()[0]
                .already_present
        );
        std::fs::write(mods.join("api.jar"), b"wrong").unwrap();
        assert!(
            validate_content_destinations(&root, &item.id, plan)
                .unwrap_err()
                .contains("different bytes")
        );
        assert_eq!(std::fs::read(mods.join("api.jar")).unwrap(), b"wrong");
        std::fs::remove_dir_all(root.root).unwrap();
    }
    #[test]
    fn committing_content_rolls_back_new_files_without_overwriting_existing_ones() {
        let root = content_test_root("rollback");
        let a = root.root.join("a.download");
        let b = root.root.join("b.download");
        std::fs::write(&a, b"new-a").unwrap();
        std::fs::write(&b, b"new-b").unwrap();
        let first = root.root.join("mods/a.jar");
        let existing = root.root.join("mods/b.jar");
        std::fs::create_dir_all(existing.parent().unwrap()).unwrap();
        std::fs::write(&existing, b"keep").unwrap();
        assert!(
            commit_content_files(&[(first.clone(), a.clone()), (existing.clone(), b.clone())])
                .is_err()
        );
        assert!(!first.exists());
        assert!(!a.exists());
        assert!(!b.exists());
        assert_eq!(std::fs::read(&existing).unwrap(), b"keep");
        std::fs::write(&a, b"new-a").unwrap();
        commit_content_files(&[(first.clone(), a.clone())]).unwrap();
        assert_eq!(std::fs::read(first).unwrap(), b"new-a");
        assert!(!a.exists());
        std::fs::remove_dir_all(root.root).unwrap();
    }
    #[test]
    fn changed_review_receipts_are_refused_before_installation() {
        let plan = model::ContentPlan {
            instance_id: "target".into(),
            instance_name: "Target".into(),
            project_id: "demo".into(),
            release_id: "release".into(),
            game_version: "1.20.1".into(),
            loader: Loader::Fabric,
            files: vec![content_file("a.jar", b"test")],
        };
        require_reviewed_plan(&plan, &plan).unwrap();
        let mut changed = plan.clone();
        changed.files[0].sha1 = "f".repeat(40);
        assert!(
            require_reviewed_plan(&plan, &changed)
                .unwrap_err()
                .contains("nothing was downloaded")
        );
        let mut changed = plan.clone();
        changed.files.push(content_file("dependency.jar", b"other"));
        assert!(require_reviewed_plan(&plan, &changed).is_err());
    }
    #[test]
    fn query_encoding_preserves_utf8_and_escapes_query_separators() {
        assert_eq!(encode("a & b/雪"), "a%20%26%20b%2F%E9%9B%AA");
    }
    #[test]
    fn public_search_contract_accepts_realistic_data() {
        let response: SearchResponse = serde_json::from_str(r#"{"hits":[{"project_id":"demo","title":"A project","description":"Description","author":"Author","downloads":42,"project_type":"mod"}],"total_hits":1}"#).unwrap();
        assert_eq!(response.hits.len(), response.total_hits);
    }
    fn a_release(dependencies: Vec<model::ReleaseDependency>) -> model::Release {
        model::Release {
            id: "version-1".into(),
            project_id: "demo".into(),
            name: "Demo 1.0".into(),
            version_number: "1.0".into(),
            version_type: "release".into(),
            date_published: String::new(),
            changelog: String::new(),
            game_versions: vec!["1.20.1".into()],
            loaders: vec!["fabric".into()],
            files: vec![],
            dependencies,
        }
    }
    fn a_dependency(kind: &str, project: &str) -> model::ReleaseDependency {
        model::ReleaseDependency {
            dependency_type: kind.into(),
            project_id: Some(project.into()),
            version_id: None,
            file_name: None,
        }
    }
    #[test]
    fn ownership_failures_do_not_read_like_network_failures() {
        let unowned = palantir_net::Error::Http {
            url: "https://api.minecraftservices.com/minecraft/profile".into(),
            status: Some(404),
            why: "this account owns no copy of Minecraft".into(),
        };
        assert!(sign_in_message(&unowned).contains("does not own Minecraft"));
        let network = palantir_net::Error::Http {
            url: "https://login.microsoftonline.com/consumers/oauth2/v2.0/token".into(),
            status: None,
            why: "connection refused".into(),
        };
        let message = sign_in_message(&network);
        assert!(message.contains("did not finish"));
        assert!(!message.contains("does not own"));
    }
    #[test]
    fn a_cancelled_sign_in_is_not_told_to_try_again() {
        let (events, received) = std::sync::mpsc::channel();
        let cancelled = AtomicBool::new(true);
        report_auth_failure(&events, &cancelled, "Sign-in polling failed".into());
        assert!(received.try_recv().is_err());
        let live = AtomicBool::new(false);
        report_auth_failure(&events, &live, "Sign-in polling failed".into());
        let reported = received.try_recv().is_ok_and(|event| {
            matches!(event, Event::Authentication(model::Authentication::Failed(why)) if why.contains("polling"))
        });
        assert!(reported);
    }
    #[test]
    fn an_install_note_says_how_many_dependencies_came_with_it() {
        assert_eq!(
            install_note("Demo", "World", 1),
            "Installed Demo into World. Restart the game to load it."
        );
        assert!(install_note("Demo", "World", 2).contains("1 required dependency into"));
        assert!(install_note("Demo", "World", 4).contains("3 required dependencies into"));
    }
    #[test]
    fn the_release_document_carries_the_dependencies_the_walk_reads() {
        let project = Project {
            project_id: "demo".into(),
            title: "Demo".into(),
            description: String::new(),
            author: String::new(),
            downloads: 0,
            project_type: "mod".into(),
        };
        let release = a_release(vec![
            a_dependency("required", "library"),
            a_dependency("optional", "extra"),
        ]);
        let document = release_document(&project, &release);
        let required = dependencies::required("demo", &document);
        assert_eq!(required.len(), 1);
        assert_eq!(required[0].project_id.as_deref(), Some("library"));
    }
    #[test]
    fn a_pack_with_optional_files_is_not_installed_without_a_choice() {
        let none: [String; 0] = [];
        let chosen = ["mods/extra.jar".to_string()];
        // A pack with nothing optional needs no decision at all...
        assert!(pack_selection(false, None).unwrap().is_empty());
        // ...and one whose optional files were shown installs with the choices
        // made, an explicit empty choice included.
        assert!(pack_selection(false, Some(&none)).unwrap().is_empty());
        assert!(pack_selection(true, Some(&none)).unwrap().is_empty());
        assert_eq!(pack_selection(true, Some(&chosen)).unwrap(), &chosen[..]);
        // Optional files the player was never shown: refused, in those words.
        let refused = pack_selection(true, None).unwrap_err();
        assert!(refused.contains("optional client files"), "{refused}");
        assert!(refused.contains("nothing was created"), "{refused}");
        assert_eq!(refused, pack_optional_choice_message());
        // And the index scan that decides whether there are any.
        assert!(optional_client_files(&serde_json::json!({
            "files": [{"path": "mods/a.jar", "env": {"client": "optional"}}]
        })));
        assert!(!optional_client_files(&serde_json::json!({
            "files": [{"path": "mods/a.jar", "env": {"client": "required"}}]
        })));
        assert!(!optional_client_files(
            &serde_json::json!({"files": [{"path": "mods/a.jar"}]})
        ));
    }
    fn a_stored_session() -> session::StoredSession {
        session::StoredSession {
            player_name: "Quiet".into(),
            player_uuid: "11111111-2222-3333-4444-555555555555".into(),
            access_token: "a-token".into(),
            refresh_token: Some("a-refresh-token".into()),
            client_id: "client-1".into(),
            expires_at: session::StoredSession::expires_at(1_000, 600),
        }
    }
    #[test]
    fn a_microsoft_launch_decides_from_the_stored_session_and_never_falls_back() {
        let stored = a_stored_session();
        assert_eq!(session_renewal(Some(&stored), 1_100).unwrap(), None);
        assert_eq!(
            session_renewal(Some(&stored), 1_600).unwrap(),
            Some("a-refresh-token".to_string()),
            "the boundary itself is expired, and the refresh token is what renews it"
        );
        let missing = session_renewal(None, 0).unwrap_err();
        assert!(missing.contains("never quietly falls back"), "{missing}");
        assert!(missing.contains("Sign in"), "{missing}");
        let mut no_refresh = stored.clone();
        no_refresh.refresh_token = None;
        let refused = session_renewal(Some(&no_refresh), 1_600).unwrap_err();
        assert!(refused.contains("expired"), "{refused}");
        assert!(refused.contains("never quietly falls back"), "{refused}");
        let mut blank = stored.clone();
        blank.refresh_token = Some("  ".into());
        assert!(session_renewal(Some(&blank), 1_600).is_err());
    }
    #[test]
    fn the_client_override_layer_is_the_copy_that_lands() {
        let plan = mrpack::PackPlan {
            files: vec![],
            overrides: mrpack::override_layers(),
        };
        let names = vec![
            "overrides/mods/shared.jar".to_string(),
            "client-overrides/mods/shared.jar".to_string(),
            "overrides/config/kept.toml".to_string(),
            "server-overrides/config/server.toml".to_string(),
        ];
        let placed = override_files(&names, &plan);
        assert_eq!(placed.len(), 2);
        let landing = placed
            .iter()
            .find(|(_, relative)| relative == "mods/shared.jar")
            .map(|(entry, _)| entry.as_str());
        assert_eq!(landing, Some("client-overrides/mods/shared.jar"));
        assert!(
            placed
                .iter()
                .any(|(_, relative)| relative == "config/kept.toml")
        );
        assert!(
            !placed
                .iter()
                .any(|(_, relative)| relative.ends_with("server.toml"))
        );
    }
    #[test]
    fn pack_paths_cannot_leave_the_game_directory() {
        let game = Path::new("/tmp/palantir-game");
        assert!(pack_destination(game, "mods/a.jar").is_ok());
        // A leading separator is absolute on Unix but a plain name on Windows,
        // so the portable cases to pin are parents that climb out.
        assert!(pack_destination(game, "../escape.jar").is_err());
        assert!(pack_destination(game, "mods/../../escape.jar").is_err());
    }
    #[test]
    fn a_verified_hash_is_required_and_well_formed() {
        let hash = "a".repeat(40);
        assert_eq!(content_hash(Some(&hash), "a.jar").unwrap(), hash);
        assert!(content_hash(None, "a.jar").is_err());
        assert!(content_hash(Some("abc"), "a.jar").is_err());
        assert!(content_url("https://example.com/a.jar", "a.jar").is_err());
        assert!(content_url("https://cdn.modrinth.com/a.jar", "a.jar").is_ok());
    }

    /// A real `.mrpack` on disk, plus the root it was built in so the test can
    /// clean up after itself.
    struct SamplePack {
        root: DataRoot,
        archive: PathBuf,
        /// Where a pack installed from this archive would put its files. The
        /// tests assert it is never created on a refused pack.
        game_dir: PathBuf,
    }

    /// One `modrinth.index.json` carrying `files`, shaped the way the format
    /// requires (a game, one loader, a version).
    fn pack_index(files: Vec<serde_json::Value>) -> serde_json::Value {
        serde_json::json!({
            "formatVersion": 1,
            "game": "minecraft",
            "versionId": "1.0.0",
            "name": "Sample pack",
            "files": files,
            "dependencies": {"minecraft": "1.20.1", "fabric-loader": "0.15.0"}
        })
    }

    /// One index file record. The hashes are the *true* SHA-1 and SHA-512 of
    /// `body`, so an archive built by these helpers is one a download step
    /// could genuinely verify -- the bytes are simply not fetched here, which
    /// is the half the mock-server seam would add.
    fn pack_file(path: &str, body: &[u8], client: Option<&str>) -> serde_json::Value {
        let mut record = serde_json::json!({
            "path": path,
            "hashes": {
                "sha1": palantir_net::download::sha1_bytes(body),
                "sha512": "1".repeat(128),
            },
            "downloads": [format!("https://cdn.modrinth.com/{path}")],
            "fileSize": body.len() as u64,
        });
        if let Some(client) = client {
            record["env"] = serde_json::json!({"client": client});
        }
        record
    }

    /// A record with no download at all: the file is expected to arrive from
    /// the archive's own overrides. `plan_files` accepts this at review time
    /// even though the strict importer refuses it, which is the difference the
    /// install's own check exists to close.
    fn local_pack_file(path: &str) -> serde_json::Value {
        serde_json::json!({"path": path, "hashes": {"sha1": "2".repeat(40)}, "downloads": []})
    }

    /// Write a zip carrying `index` plus `entries`, and return it with the root
    /// it lives in.
    fn sample_pack(
        label: &str,
        index: &serde_json::Value,
        entries: &[(&str, &[u8])],
    ) -> SamplePack {
        let root = content_test_root(label);
        let archive = root.root.join("sample.mrpack");
        let file = std::fs::File::create(&archive).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("modrinth.index.json", options).unwrap();
        writer
            .write_all(&serde_json::to_vec_pretty(index).unwrap())
            .unwrap();
        for (name, body) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(body).unwrap();
        }
        writer.finish().unwrap();
        SamplePack {
            game_dir: root.game_dir("would-be-instance"),
            root,
            archive,
        }
    }

    /// What `install_pack_archive` decides about an archive before it fetches
    /// anything, run over a real file.
    ///
    /// It cannot call `install_pack_archive` itself: that function takes an
    /// `&Engine`, and an `Engine` can only be built from the process
    /// environment (`Engine::new` reads `PALANTIR_DATA_DIR`), which a test in
    /// this binary must not set -- the variable is process-global and the
    /// test runner is multi-threaded, so setting it would race every sibling.
    /// So this runs the same production helpers over the same real archive, in
    /// the same order, and the tests below assert the facts they produce: the
    /// archive's own index decides the optional files, the plan is built from
    /// it, and the archive's own entry names decide which overrides land.
    fn pack_plan_from_archive(
        archive: &Path,
        selected: Option<&[String]>,
    ) -> Result<(mrpack::PackPlan, Vec<(String, String)>)> {
        let file = std::fs::File::open(archive).map_err(|e| format!("Pack archive: {e}"))?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("Pack archive: {e}"))?;
        let mut index = zip
            .by_name("modrinth.index.json")
            .map_err(|e| format!("Pack archive: {e}"))?;
        let mut text = String::new();
        std::io::Read::read_to_string(&mut index, &mut text).map_err(|e| e.to_string())?;
        drop(index);
        let index_value: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("Pack index: {e}"))?;
        let chosen = pack_selection(optional_client_files(&index_value), selected)?;
        let plan = mrpack::plan_files(&index_value, chosen).map_err(|e| e.to_string())?;
        let names = zip
            .file_names()
            .map(str::to_string)
            .collect::<Vec<String>>();
        let overrides = override_files(&names, &plan);
        Ok((plan, overrides))
    }

    fn plan_paths(plan: &mrpack::PackPlan) -> Vec<String> {
        plan.files
            .iter()
            .map(|file| match file {
                mrpack::SelectedFile::Download { path, .. }
                | mrpack::SelectedFile::Local { path } => path.clone(),
            })
            .collect()
    }

    /// The archive's own index decides which optional files a pack installs,
    /// and the same archive installs two different collections depending on the
    /// answer the review screen gave.
    ///
    /// This is the fact the pure `pack_selection` tests cannot reach: they hand
    /// in a hand-built boolean and a hand-built list, while what the install
    /// actually reads is the index inside the zip the player chose.
    #[test]
    fn a_real_pack_archive_decides_its_optional_files_from_the_index_it_carries() {
        let required = b"required-mod-bytes".as_slice();
        let extra = b"optional-mod-bytes".as_slice();
        let index = pack_index(vec![
            pack_file("mods/required.jar", required, None),
            pack_file("mods/extra.jar", extra, Some("optional")),
        ]);
        let pack = sample_pack(
            "pack-optional",
            &index,
            &[
                ("overrides/config/kept.toml", b"override-config".as_slice()),
                ("overrides/mods/required.jar", b"override-wins".as_slice()),
            ],
        );

        // Never shown: refused, in the words the player reads, and the refusal
        // comes before the collection exists -- `model::create` runs only after
        // a plan does.
        let refused = pack_plan_from_archive(&pack.archive, None).unwrap_err();
        assert!(refused.contains("optional client files"), "{refused}");
        assert!(
            !pack.root.instance_dir("would-be-instance").exists(),
            "a refused pack must not have created a collection"
        );

        // Shown and declined: the required set only, and the archive's own
        // entry names decide which overrides land where (base layer first, and
        // each at the game directory's own relative path).
        let none: [String; 0] = [];
        let (plan, overrides) = pack_plan_from_archive(&pack.archive, Some(&none)).unwrap();
        assert_eq!(plan_paths(&plan), ["mods/required.jar"]);
        assert_eq!(
            overrides,
            [
                (
                    "overrides/config/kept.toml".to_string(),
                    "config/kept.toml".to_string()
                ),
                (
                    "overrides/mods/required.jar".to_string(),
                    "mods/required.jar".to_string()
                ),
            ]
        );
        for (_, relative) in &overrides {
            let destination = pack_destination(&pack.game_dir, relative).unwrap();
            assert!(
                destination.starts_with(&pack.game_dir),
                "{destination:?} must stay inside the game directory"
            );
        }

        // Shown and taken: both files, carrying the index's own hash and size
        // for the optional one -- the values the download step verifies against.
        let chosen = ["mods/extra.jar".to_string()];
        let (plan, _) = pack_plan_from_archive(&pack.archive, Some(&chosen)).unwrap();
        assert_eq!(plan_paths(&plan), ["mods/required.jar", "mods/extra.jar"]);
        let expected = palantir_net::download::sha1_bytes(extra);
        let entry = plan
            .files
            .iter()
            .find(|file| matches!(file, mrpack::SelectedFile::Download { path, .. } if path == "mods/extra.jar"))
            .expect("the chosen optional file is planned");
        match entry {
            mrpack::SelectedFile::Download { sha1, size, .. } => {
                assert_eq!(sha1.as_deref(), Some(expected.as_str()));
                assert_eq!(*size, Some(extra.len() as u64));
            }
            other => panic!("expected a download record, got {other:?}"),
        }

        std::fs::remove_dir_all(pack.root.root).unwrap();
    }

    /// A pack path that leaves the game directory is refused at plan time, so a
    /// hostile index never reaches the write.
    #[test]
    fn a_real_pack_archive_cannot_place_a_file_outside_the_game_directory() {
        let none: [String; 0] = [];
        for path in ["../escape.jar", "mods/../../escape.jar"] {
            let index = pack_index(vec![pack_file(path, b"escape".as_slice(), None)]);
            let pack = sample_pack("pack-escape", &index, &[]);
            let refused = pack_plan_from_archive(&pack.archive, Some(&none)).unwrap_err();
            assert!(
                refused.contains("escape the game directory"),
                "{path}: {refused}"
            );
            assert!(
                !pack.root.instance_dir("would-be-instance").exists(),
                "{path} must be refused before a collection is created"
            );
            std::fs::remove_dir_all(pack.root.root).unwrap();
        }
    }

    /// A record with no download is only installable when the archive itself
    /// carries the bytes, and the archive's entry list is what says so.
    ///
    /// The install's refusal reads exactly this pair -- the plan's `Local`
    /// paths against the override list -- and that predicate lives inline in
    /// `install_pack_archive`, which needs an `Engine`. What is pinned here is
    /// the archive-level fact it reads: a real `.mrpack` can name such a file,
    /// and whether its overrides rescue it is visible in the archive.
    #[test]
    fn a_pack_record_with_no_download_is_rescued_only_by_the_archive_itself() {
        let index = pack_index(vec![
            local_pack_file("mods/hand-placed.jar"),
            pack_file("mods/downloaded.jar", b"downloaded".as_slice(), None),
        ]);
        let none: [String; 0] = [];

        // Carried by its own overrides: the file the plan calls `Local` has a
        // source after all.
        let carried = sample_pack(
            "pack-local-carried",
            &index,
            &[("overrides/mods/hand-placed.jar", b"carried".as_slice())],
        );
        let (plan, overrides) = pack_plan_from_archive(&carried.archive, Some(&none)).unwrap();
        let local = plan
            .files
            .iter()
            .find_map(|file| match file {
                mrpack::SelectedFile::Local { path } => Some(path.clone()),
                _ => None,
            })
            .expect("the index named a file with no download");
        assert_eq!(local, "mods/hand-placed.jar");
        assert!(
            overrides.iter().any(|(_, placed)| placed == &local),
            "the override layer carries the file the index lists with no download"
        );
        std::fs::remove_dir_all(carried.root.root).unwrap();

        // Nothing carries it: the record has no source at all, which is the
        // state the install refuses rather than installing a pack with one of
        // its own files missing.
        let orphaned = sample_pack(
            "pack-local-orphaned",
            &index,
            &[("overrides/mods/downloaded.jar", b"unrelated".as_slice())],
        );
        let (_, overrides) = pack_plan_from_archive(&orphaned.archive, Some(&none)).unwrap();
        assert!(
            !overrides.iter().any(|(_, placed)| placed == &local),
            "an archive that carries no such file leaves the record unsourced"
        );
        std::fs::remove_dir_all(orphaned.root.root).unwrap();
    }

    /// One index record whose download URL is given rather than derived.
    ///
    /// The real catalogue serves its files from one CDN, and a test that has to
    /// watch bytes move serves them from the mock server instead.
    fn pack_file_from(
        url: &str,
        path: &str,
        body: &[u8],
        client: Option<&str>,
    ) -> serde_json::Value {
        let mut record = serde_json::json!({
            "path": path,
            "hashes": {
                "sha1": palantir_net::download::sha1_bytes(body),
                // The format requires SHA-512 as well; nothing in the transfer
                // layer verifies it, so only its presence is asserted here.
                "sha512": "3".repeat(128),
            },
            "downloads": [url],
            "fileSize": body.len() as u64,
        });
        if let Some(client) = client {
            record["env"] = serde_json::json!({ "client": client });
        }
        record
    }

    /// What one real pack install produced, for the assertions to read.
    struct InstalledPack {
        root: DataRoot,
        game_dir: PathBuf,
        instance_dir: PathBuf,
        /// The paths the mock server was asked for, in arrival order.
        hits: Vec<String>,
    }

    /// Install `pack` for real, through the same function the window calls,
    /// over a mock server standing in for the content CDN.
    ///
    /// This is the half the review-only tests above cannot reach: the bytes
    /// move, the record's hash is checked against them, and the files land in
    /// the collection. It is only possible because the engine has a
    /// constructor that takes a root -- a test in a multi-threaded binary must
    /// not set `PALANTIR_DATA_DIR` to get one.
    fn installed_pack(
        pack: &SamplePack,
        selected: Option<&[String]>,
        server: &MockServer,
        name: &str,
    ) -> Result<InstalledPack> {
        let engine = Engine::at(pack.root.clone())?;
        let (events, _window) = mpsc::channel();
        let source = pack.archive.to_str().expect("a temp path is UTF-8");
        install_pack_archive(
            &engine,
            &events,
            source,
            name,
            "1.20.1",
            Loader::Fabric,
            selected,
        )?;
        let item = model::load_instances(&pack.root)?
            .into_iter()
            .find(|item| item.name == name)
            .expect("the install created the collection it was given");
        Ok(InstalledPack {
            root: pack.root.clone(),
            game_dir: pack.root.game_dir(&item.id),
            instance_dir: pack.root.instance_dir(&item.id),
            hits: server.hits(),
        })
    }

    /// Staging names the install leaves in a collection: none may survive, or
    /// the next install would find a half-file where a download goes.
    fn staging_leftovers(instance_dir: &Path) -> Vec<String> {
        std::fs::read_dir(instance_dir)
            .map(|entries| {
                entries
                    .filter_map(|entry| entry.ok())
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .filter(|name| name.starts_with("pack-"))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// A pack install moves real bytes: what the review kept lands verified in
    /// the collection, the archive's own override layer lands last, and what
    /// the review declined is never even fetched.
    #[test]
    fn a_pack_install_fetches_what_the_review_kept_and_never_fetches_what_it_declined() {
        let required = b"the required half of the pack".to_vec();
        let optional = b"the half a player may decline".to_vec();
        let carried = b"a file only the archive carries".to_vec();
        let server = MockServer::start(vec![
            ("/mods/required.jar", Route::new(required.clone())),
            ("/mods/optional.jar", Route::new(optional.clone())),
        ]);
        let required_record = pack_file_from(
            &server.url("/mods/required.jar"),
            "mods/required.jar",
            &required,
            None,
        );
        let optional_record = pack_file_from(
            &server.url("/mods/optional.jar"),
            "mods/optional.jar",
            &optional,
            Some("optional"),
        );
        let index = pack_index(vec![required_record, optional_record]);
        let pack = sample_pack(
            "pack-install-declined",
            &index,
            &[("overrides/mods/carried.jar", carried.as_slice())],
        );
        let declined: Vec<String> = Vec::new();

        let installed = installed_pack(&pack, Some(&declined), &server, "Declined pack").unwrap();

        let placed = std::fs::read(installed.game_dir.join("mods/required.jar")).unwrap();
        assert_eq!(placed, required);
        let carried_bytes = std::fs::read(installed.game_dir.join("mods/carried.jar")).unwrap();
        assert_eq!(carried_bytes, carried, "the override layer is what lands");
        let optional_placed = installed.game_dir.join("mods/optional.jar").exists();
        assert!(!optional_placed, "a declined optional file was installed");
        assert!(
            installed.hits.contains(&"/mods/required.jar".to_string()),
            "the required file was never fetched: {:?}",
            installed.hits
        );
        assert!(
            !installed.hits.contains(&"/mods/optional.jar".to_string()),
            "a declined optional file was fetched anyway: {:?}",
            installed.hits
        );
        let leftovers = staging_leftovers(&installed.instance_dir);
        assert!(leftovers.is_empty(), "staging survived: {leftovers:?}");
        std::fs::remove_dir_all(&installed.root.root).unwrap();
    }

    /// The other answer to the same review: a chosen optional file is fetched
    /// and verified exactly like a required one.
    #[test]
    fn a_pack_install_fetches_the_optional_file_the_review_chose() {
        let optional = b"the half the player asked for".to_vec();
        let server = MockServer::start(vec![("/mods/optional.jar", Route::new(optional.clone()))]);
        let record = pack_file_from(
            &server.url("/mods/optional.jar"),
            "mods/optional.jar",
            &optional,
            Some("optional"),
        );
        let index = pack_index(vec![record]);
        let pack = sample_pack("pack-install-chosen", &index, &[]);
        let chosen = vec!["mods/optional.jar".to_string()];

        let installed = installed_pack(&pack, Some(&chosen), &server, "Chosen pack").unwrap();

        let placed = std::fs::read(installed.game_dir.join("mods/optional.jar")).unwrap();
        assert_eq!(placed, optional);
        assert!(
            installed.hits.contains(&"/mods/optional.jar".to_string()),
            "the chosen file was never fetched: {:?}",
            installed.hits
        );
        std::fs::remove_dir_all(&installed.root.root).unwrap();
    }

    /// A transfer that arrives with the wrong bytes installs nothing at all.
    ///
    /// The record carries a hash precisely so this case is decidable, and the
    /// collection must be left holding none of the pack's files rather than a
    /// file the game would load as content.
    #[test]
    fn a_pack_download_that_fails_verification_places_nothing() {
        let promised = b"the bytes the index promises".to_vec();
        let served = b"the bytes the server actually sent".to_vec();
        let server = MockServer::start(vec![("/mods/required.jar", Route::new(served))]);
        let record = pack_file_from(
            &server.url("/mods/required.jar"),
            "mods/required.jar",
            &promised,
            None,
        );
        let index = pack_index(vec![record]);
        let pack = sample_pack("pack-install-tampered", &index, &[]);
        let none: Vec<String> = Vec::new();

        let refused = installed_pack(&pack, Some(&none), &server, "Tampered pack");
        assert!(refused.is_err(), "wrong bytes must refuse the install");

        // The collection exists -- it is what the pack was being installed into
        // -- but holds none of the pack's files, and no staging file survives.
        for item in model::load_instances(&pack.root).unwrap() {
            let placed = pack.root.game_dir(&item.id).join("mods/required.jar");
            assert!(!placed.exists(), "an unverified file was placed");
            let leftovers = staging_leftovers(&pack.root.instance_dir(&item.id));
            assert!(leftovers.is_empty(), "staging survived: {leftovers:?}");
        }
        std::fs::remove_dir_all(&pack.root.root).unwrap();
    }
}
