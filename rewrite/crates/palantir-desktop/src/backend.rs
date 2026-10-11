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
    self, Instance, Loader, Preferences, Project, ProjectDetail, ProjectType, Result,
    SearchResponse, Sort,
};
use palantir_core::assets::{AssetDestination, AssetIndex};
use palantir_core::paths::DataRoot;
use palantir_core::rules::{Arch, Os, Platform};
use palantir_core::version::Version;
use palantir_loader::{dependencies, install, java, launch, mrpack, profiles};
use palantir_net::{
    auth,
    cache::{MANIFEST_TTL, MetadataCache, cache_dir_for},
    client::Http,
    download::{DownloadOptions, download},
    scheduler::Scheduler,
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
    Import(String, String, String, Loader),
    Rename(String, String),
    Inventory(String),
    ToggleMod(String, String),
    RemoveContent(String, String, String),
    OpenFolder(Option<String>, String),
    Delete(String),
    Save(Preferences),
    Launch(String, Preferences),
    Stop(String),
    Search(u64, String, ProjectType, Sort, usize, String),
    Project(String),
    Content(String, Project, Option<String>),
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
    Directory(String, Result<Vec<model::DirectoryEntry>>),
    Releases(String, Result<Vec<model::Release>>),
    Inventory(String, Result<Vec<crate::files::Entry>>),
    Status(String, String),
    Finished(String, Result<String>),
    Running(String, String),
    Exited(String, String),
    Log(String, String),
    Search(u64, Result<(Vec<Project>, usize)>),
    Project(String, Result<ProjectDetail>),
    Error(String),
    Authentication(model::Authentication),
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
                        let result = if matches!(loader, Loader::Fabric | Loader::Quilt) {
                            let sources =
                                profiles::LoaderSources::vendor(if loader == Loader::Fabric {
                                    profiles::LoaderKind::Fabric
                                } else {
                                    profiles::LoaderKind::Quilt
                                });
                            profiles::list_loaders(&engine.http, &engine.cache, &sources, &game)
                                .map(|v| v.into_iter().map(|r| r.version).collect())
                                .map_err(|e| e.to_string())
                        } else {
                            Ok(Vec::new())
                        };
                        emit(&events, Event::LoaderVersions(game, loader, result));
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
                Request::Import(source, name, game, loader) => {
                    let engine = Arc::clone(&engine);
                    let events = events.clone();
                    std::thread::spawn(move || {
                        // A pack archive is installed rather than copied: its
                        // index lists files that were never on this machine.
                        let from_archive = Path::new(&source)
                            .extension()
                            .is_some_and(|extension| extension == "mrpack");
                        let result = if from_archive {
                            install_pack_archive(&engine, &events, &source, &name, &game, loader)
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
                request @ (Request::Launch(_, _) | Request::Content(_, _, _)) => {
                    let id = match &request {
                        Request::Launch(id, _) | Request::Content(id, _, _) => id.clone(),
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
                                Request::Launch(id, prefs) => {
                                    prepare_and_launch(&engine, &events, &id, prefs)
                                }
                                Request::Content(id, project, release) => install_content(
                                    &engine,
                                    &events,
                                    &id,
                                    &project,
                                    release.as_deref(),
                                ),
                                _ => Err("Unknown background operation.".into()),
                            }
                        })();
                        if let Ok(mut active) = active.lock() {
                            active.retain(|item| item != &id);
                        }
                        emit(&events, Event::Finished(id, result));
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

fn platform() -> Platform {
    Platform::new(Os::host(), "", Arch::host())
}
fn prepare_and_launch(
    engine: &Arc<Engine>,
    events: &mpsc::Sender<Event>,
    id: &str,
    preferences: Preferences,
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
    let version = match &instance.installed_version {
        Some(version) => Version::parse(
            &std::fs::read_to_string(engine.root.version_json(version))
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?,
        None => {
            let version = match instance.loader {
                Loader::Forge | Loader::NeoForge => return Err("The selected loader needs desktop processor coordination. The collection is saved, but this backend cannot launch it yet.".into()),
                Loader::Vanilla => {
                    let version = install::resolve_version(
                        &engine.http,
                        &engine.cache,
                        install::VERSION_MANIFEST_URL,
                        &instance.game_version,
                    )
                    .map_err(|e| e.to_string())?;
                    install::install_document(
                        &engine.syncer(),
                        &engine.cache,
                        install::VERSION_MANIFEST_URL,
                        &platform(),
                        &version.to_json_string().map_err(|e| e.to_string())?,
                        Some(&engine.root.game_dir(id)),
                    )
                    .map_err(|e| e.to_string())?
                    .version
                }
                Loader::Fabric | Loader::Quilt => {
                    let sources =
                        profiles::LoaderSources::vendor(if instance.loader == Loader::Fabric {
                            profiles::LoaderKind::Fabric
                        } else {
                            profiles::LoaderKind::Quilt
                        });
                    profiles::install_loader(
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
                    .map_err(|e| e.to_string())?
                    .version
                }
            };
            instance.installed_version = Some(version.id.clone());
            model::save_instance(&engine.root, &instance)?;
            version
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
    let java = if preferences.java.trim().is_empty() {
        runtime(engine, &version)?
    } else {
        PathBuf::from(&preferences.java)
    };
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
    // This preview is explicitly offline; the stable identifier is not an authenticated identity.
    // The identity comes from the selected offline account, so switching accounts
    // changes the name and uuid the game is handed and nothing else. The
    // derivation lives in the model so one name cannot mean two identifiers.
    let (_, player_name, player_uuid) = preferences.active_identity();
    let context = launch::LaunchContext {
        player_name,
        player_uuid,
        access_token: "0".into(),
        user_type: "legacy".into(),
        java,
        game_dir: engine.root.game_dir(id),
        assets_root: engine.root.assets_dir(),
        natives_dir: natives,
        library_root: engine.root.libraries_dir(),
        client_jar: engine.root.version_jar(&version.id),
        game_assets,
        ..Default::default()
    };
    let plan =
        launch::build_launch_plan(&version, &platform(), &context).map_err(|e| e.to_string())?;
    // JVM flags must precede the main class, not follow game arguments.
    let mut command = std::process::Command::new(&plan.java);
    command
        .current_dir(&plan.dir)
        .arg(format!("-Xmx{}M", preferences.memory_mb));
    let original = plan.command();
    command.args(original.get_args());
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
                    emit(
                        &events,
                        Event::Exited(id, format!("{} exited: {status}", instance.name)),
                    );
                    break;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(150)),
                Err(error) => {
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
fn install_content(
    engine: &Engine,
    events: &mpsc::Sender<Event>,
    id: &str,
    project: &Project,
    release_id: Option<&str>,
) -> Result<String> {
    if engine.busy(id)? {
        return Err("Stop this instance before installing content.".into());
    }
    let instance = engine.instance(id)?;
    if project.project_type == "modpack" {
        return Err("Modpack import is not connected in this preview. Use a separate instance; a pack must not overwrite an existing world's files.".into());
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
    let version = if let Some(release_id) = release_id {
        versions.iter().find(|r| r.id == release_id)
    } else {
        versions.first()
    }
    .ok_or(
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
    let mut planned = vec![planned_primary(version, folder)?];
    if version.required_dependencies() {
        let root = release_document(project, version);
        let mut lookup = |key: &str| -> palantir_loader::error::Result<Vec<serde_json::Value>> {
            lookup_versions(engine, key).map_err(|why| palantir_loader::error::Error::Invalid {
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
    let game = crate::files::game_directory(&engine.root, id)?;
    // Every destination is decided before anything is fetched: an install that
    // stops at the fifth file because it already existed is the partial state
    // this refuses.
    let mut destinations = Vec::new();
    for item in &planned {
        let directory = game.join(&item.folder);
        if directory.exists()
            && std::fs::symlink_metadata(&directory)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_symlink()
        {
            return Err("Refusing a linked content folder.".into());
        }
        let destination = directory.join(&item.file_name);
        if destination.exists() {
            return Err(format!(
                "{} is already installed; remove it before replacing it.",
                item.file_name
            ));
        }
        destinations.push(destination);
    }
    emit(
        events,
        Event::Status(
            id.into(),
            format!(
                "Downloading {} file(s) for {}…",
                planned.len(),
                project.title
            ),
        ),
    );
    let mut staged: Vec<(PathBuf, PathBuf)> = Vec::new();
    for (item, destination) in planned.iter().zip(destinations) {
        let staging = engine
            .root
            .instance_dir(id)
            .join(format!("content-{}.download", item.sha1));
        if let Err(error) = download(
            &engine.http,
            &item.url,
            &staging,
            Some(item.sha1.as_str()),
            item.size,
            &DownloadOptions::default(),
        ) {
            // The transfer deletes bytes that failed their hash; the files this
            // install already staged are ours to remove, because a
            // half-downloaded set is a state the player cannot see.
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
    Ok(install_note(&project.title, &instance.name, files))
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
            Ok(auth::Poll::Token(token)) => break token,
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
        let user = auth::xbox_user_token(&engine.http, &msa_token, auth::endpoints::XBL_USER)
            .map_err(|error| sign_in_message(&error))?;
        let xsts = auth::xbox_xsts_token(&engine.http, &user, auth::endpoints::XSTS)
            .map_err(|error| sign_in_message(&error))?;
        let session = auth::minecraft_login(&engine.http, &xsts, auth::endpoints::MC_LOGIN)
            .map_err(|error| sign_in_message(&error))?;
        auth::minecraft_profile(&engine.http, &session, auth::endpoints::MC_PROFILE)
            .map(|profile| profile.name)
            .map_err(|error| sign_in_message(&error))
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
struct PlannedContent {
    url: String,
    file_name: String,
    sha1: String,
    size: Option<u64>,
    folder: String,
}
fn planned_primary(version: &model::Release, folder: &str) -> Result<PlannedContent> {
    let file = version
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| version.files.first())
        .ok_or("This release contains no downloadable file.")?;
    crate::files::local_name(&file.filename)?;
    Ok(PlannedContent {
        url: content_url(&file.url, &file.filename)?,
        sha1: content_hash(file.hashes.get("sha1").map(String::as_str), &file.filename)?,
        size: Some(file.size),
        file_name: file.filename.clone(),
        folder: folder.to_string(),
    })
}
fn planned_dependency(file: dependencies::ResolvedFile, folder: &str) -> Result<PlannedContent> {
    crate::files::local_name(&file.file_name)?;
    let url = file.url.ok_or_else(|| {
        format!(
            "{} names no download; nothing was installed.",
            file.file_name
        )
    })?;
    Ok(PlannedContent {
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
/// The choice of those files belongs to the review screen, and the import
/// request carries no selection: installing the required set and calling it the
/// pack is the silent-wrong-install this refuses.
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
fn install_pack_archive(
    engine: &Engine,
    events: &mpsc::Sender<Event>,
    source: &str,
    name: &str,
    game: &str,
    loader: Loader,
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
    if optional_client_files(&index_value) {
        return Err("This pack carries optional client files. Review them and choose before installing; nothing was created.".into());
    }
    let plan = mrpack::plan_files(&index_value, &[]).map_err(|e| e.to_string())?;
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
    let item = model::create(&engine.root, name, game, loader)?;
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
        let staging = engine.root.instance_dir(&item.id).join(format!(
            "pack-{}.download",
            sha1.as_deref().unwrap_or("unverified")
        ));
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
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
