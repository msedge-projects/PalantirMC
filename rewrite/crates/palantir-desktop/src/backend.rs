use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;

use crate::model::{
    self, Instance, Loader, Preferences, Project, ProjectDetail, ProjectType, Result,
    SearchResponse, Sort,
};
use palantir_core::assets::{AssetDestination, AssetIndex};
use palantir_core::paths::DataRoot;
use palantir_core::rules::{Arch, Os, Platform};
use palantir_core::version::Version;
use palantir_loader::{install, java, launch, profiles};
use palantir_net::{
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
                Request::Authenticate=>emit(&events,Event::Authentication(model::Authentication::Failed("Microsoft authentication backend is not registered. No device code or token was issued.".into()))),
                Request::CancelAuthentication=>emit(&events,Event::Authentication(model::Authentication::Cancelled)),
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
                Request::RemoveContent(id,folder,name)=>{
                    let result=(||{if engine.busy(&id)?||active.lock().map_err(|e|e.to_string())?.contains(&id){return Err("Stop the game and finish its task before removing content.".into());}crate::files::remove_content(&engine.root,&id,&folder,&name)?;Ok("Addition removed.".into())})();
                    emit(&events,Event::Finished(id.clone(),result));emit(&events,Event::Inventory(id.clone(),crate::files::inventory(&engine.root,&id)));
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
                        let result = crate::files::import_directory(
                            &engine.root,
                            std::path::Path::new(&source),
                            &name,
                            &game,
                            loader,
                        )
                        .map(|item| {
                            format!("Imported {}. The source was left untouched.", item.name)
                        });
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
    if version
        .dependencies
        .iter()
        .any(|dependency| dependency.dependency_type == "required")
    {
        return Err("This release needs additional dependencies. Dependency resolution is not yet implemented; no files were installed.".into());
    }
    let file = version
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| version.files.first())
        .ok_or("This release contains no downloadable file.")?;
    crate::files::local_name(&file.filename)?;
    let hash = file
        .hashes
        .get("sha1")
        .ok_or("The release has no SHA-1 verification hash.")?;
    if hash.len() != 40 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("The release has an invalid verification hash.".into());
    }
    if !file.url.starts_with("https://cdn.modrinth.com/") {
        return Err("This release points outside the supported secure content CDN.".into());
    }
    let folder = match project.project_type.as_str() {
        "mod" => "mods",
        "resourcepack" => "resourcepacks",
        "shader" => "shaderpacks",
        _ => return Err("Unsupported content type.".into()),
    };
    let game = crate::files::game_directory(&engine.root, id)?;
    let content_dir = game.join(folder);
    if content_dir.exists()
        && std::fs::symlink_metadata(&content_dir)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
    {
        return Err("Refusing a linked content folder.".into());
    }
    let destination = content_dir.join(&file.filename);
    if destination.exists() {
        return Err(
            "A file with this name is already installed; remove it before replacing it.".into(),
        );
    }
    emit(
        events,
        Event::Status(id.into(), format!("Downloading {}…", project.title)),
    );
    let staging = engine
        .root
        .instance_dir(id)
        .join(format!("content-{hash}.download"));
    download(
        &engine.http,
        &file.url,
        &staging,
        Some(hash),
        Some(file.size),
        &DownloadOptions::default(),
    )
    .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(engine.root.game_dir(id).join(folder)).map_err(|e| e.to_string())?;
    std::fs::rename(staging, destination).map_err(|e| e.to_string())?;
    Ok(format!(
        "Installed {} into {}. Restart the game to load it.",
        project.title, instance.name
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
}
