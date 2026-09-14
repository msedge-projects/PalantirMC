//! Launch preparation and the background launch worker.
//!
//! The worker mirrors the `palantir-cli launch-script` flow, with the two halves
//! that used to be missing now in place:
//!
//! * **The game is installed, not assumed.** Metadata is resolved through
//!   [`OnlineMetaStore`], which fetches what is not cached and writes it in the
//!   offline store's layout; then [`crate::install`] fetches the libraries, the
//!   client jar, the asset index and its objects, and extracts the natives.
//!   Only after that does anything check for a main jar — the old flow demanded
//!   one up front and so could never bootstrap a fresh instance.
//! * **The session is real.** An offline account produces the legacy session it
//!   always did; a Microsoft account renews its tokens through
//!   [`palantir_net::MicrosoftAuth`] (the same device-code/refresh chain used to
//!   sign in) and launches with `user_type = "msa"`. A renewal that fails
//!   *blocks* the launch instead of quietly falling back to an offline session,
//!   because the fallback would look like a working launch and then fail on
//!   every server the user tries to join.
//!
//! `-Djava.library.path` points at the instance's `natives/` directory, which is
//! where [`crate::install`] extracts the natives and where Prism's own
//! `MinecraftInstance::getNativePath` points, so both launchers run the same
//! extracted libraries.
//!
//! Three things travel back to the GUI through the `iced::subscription::channel`
//! sender owned by the worker thread: `Vec<String>` batches of log lines, the
//! running progress of whatever phase is fetching (a level, deliberately
//! droppable — see `send_progress`), and the final outcome as a done message.
//! See `app.rs` for the subscription.

use futures::channel::mpsc::Sender;
use palantir_core::{
    instance::Instance,
    java::JavaVersion,
    launch,
    pack::PackProfile,
    paths::{PalantirPaths, System},
    resolve::{resolve, MetaStore, OfflineMetaStore},
    settings::{defaults, Settings},
    version::{ProblemSeverity, RuntimeContext},
};
use palantir_gui::SettingsModel;
use palantir_net::meta::Fetcher;
use palantir_net::{msa_auth_session, MicrosoftAuth, OfflineSession};
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use crate::accounts::{needs_refresh, AccountKind};
use crate::app::Message;
use crate::install;
use crate::java_runtime::{self, JavaPrefs};

/// Shared handle for the running game process (for the Kill button).
pub type ChildSlot = Arc<Mutex<Option<Child>>>;

/// The account identity a launch runs as.
///
/// Carries the Microsoft tokens as well as the name, because renewing them is
/// part of preparing a launch: the store is the source of truth, this is the
/// snapshot the worker was handed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountRef {
    /// Player name.
    pub username: String,
    /// Account uuid (32 hex digits).
    pub uuid: String,
    /// Offline or Microsoft.
    pub kind: AccountKind,
    /// Game access token, for a Microsoft account.
    pub access_token: Option<String>,
    /// Microsoft refresh token, for a Microsoft account.
    pub refresh_token: Option<String>,
    /// Unix milliseconds at which the access token expires.
    pub expires_at_ms: Option<i64>,
}

impl AccountRef {
    /// A name-only (offline) account.
    pub fn offline(username: impl Into<String>, uuid: impl Into<String>) -> AccountRef {
        AccountRef {
            username: username.into(),
            uuid: uuid.into(),
            kind: AccountKind::Offline,
            access_token: None,
            refresh_token: None,
            expires_at_ms: None,
        }
    }

    /// The anonymous fallback: an offline account for a fresh player name.
    ///
    /// The uuid is derived from the name rather than generated, so two launches
    /// without a selected account are the *same* player instead of a new one
    /// every time.
    pub fn anonymous() -> AccountRef {
        AccountRef {
            uuid: crate::accounts::offline_uuid("Player"),
            ..AccountRef::offline("Player", "")
        }
    }

    /// Snapshot a stored account.
    pub fn from_entry(entry: &crate::accounts::AccountEntry) -> AccountRef {
        AccountRef {
            username: entry.username.clone(),
            uuid: entry.uuid.clone(),
            kind: entry.kind,
            access_token: entry.access_token.clone(),
            refresh_token: entry.refresh_token.clone(),
            expires_at_ms: entry.expires_at_ms,
        }
    }

    /// Whether the stored Microsoft token must be renewed before use.
    pub fn needs_refresh(&self, now_ms: i64) -> bool {
        needs_refresh(
            self.kind,
            self.refresh_token.as_deref(),
            self.expires_at_ms,
            now_ms,
        )
    }
}

/// Tokens a launch renewal produced, to be written back to the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshedTokens {
    /// Profile uuid the tokens belong to.
    pub uuid: String,
    /// Profile name.
    pub name: String,
    /// Fresh game access token.
    pub access_token: String,
    /// Fresh Microsoft refresh token (Microsoft rotates these).
    pub refresh_token: Option<String>,
    /// Unix milliseconds at which the access token expires.
    pub expires_at_ms: i64,
}

/// What [`prepare_auth`] decided to launch with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedAuth {
    /// Session handed to the argument builders.
    pub session: launch::AuthSession,
    /// Tokens to persist when the session was renewed.
    pub refreshed: Option<RefreshedTokens>,
}

/// What this launcher's own settings say a launch should use.
///
/// Read at the moment a run is requested and carried with it, because the
/// worker has no business going back to a preferences file — or to another
/// launcher's — to find out what it was told to use. The settings live in this
/// product's own directory, which is not the data root and is not what the
/// worker was handed.
///
/// Both halves are here for the same reason: a Java the version may or may not
/// be able to run on, and a heap an instance may or may not override. A
/// preference this launcher shows but never passes on is a pane that lies, and
/// two launchers disagreeing about a number is the same failure spelled
/// differently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchDefaults {
    /// The Java the Java tab names.
    pub java: JavaPrefs,
    /// Heap floor, in MiB, for an instance that does not override memory.
    pub min_mem_mib: i64,
    /// Heap ceiling, in MiB, for an instance that does not override memory.
    pub max_mem_mib: i64,
}

impl LaunchDefaults {
    /// Read the defaults this launcher's settings hold.
    pub fn from_prefs(prefs: &crate::prefs::Prefs) -> LaunchDefaults {
        LaunchDefaults {
            java: JavaPrefs::from_prefs(prefs),
            min_mem_mib: i64::from(prefs.min_mem_mib()),
            max_mem_mib: i64::from(prefs.max_mem_mib()),
        }
    }
}

impl Default for LaunchDefaults {
    /// What a launch uses when no preference has been set: the Java search, and
    /// the numbers the game's own settings ship with — not zero, which is not a
    /// heap.
    fn default() -> LaunchDefaults {
        LaunchDefaults {
            java: JavaPrefs::default(),
            min_mem_mib: palantir_core::settings::defaults::MIN_MEM_ALLOC,
            max_mem_mib: palantir_core::settings::defaults::MAX_MEM_ALLOC,
        }
    }
}

/// Inputs for one launch run.
#[derive(Debug, Clone)]
pub struct LaunchParams {
    /// Data root the instance lives under.
    pub data_root: PathBuf,
    /// Instance folder id.
    pub instance_id: String,
    /// Account to launch with.
    pub account: AccountRef,
    /// Subscription id this run streams under.
    pub run_id: u64,
    /// The settings this launcher's own UI holds, read when the run was asked
    /// for.
    pub defaults: LaunchDefaults,
}

/// A recorded run: what the subscription streams under and what the worker
/// needs. (Separate from [`LaunchParams`] so the GUI never has to build
/// worker inputs before the subscription exists.)
#[derive(Debug, Clone)]
pub struct ActiveRunData {
    /// Subscription id this run streams under.
    pub run_id: u64,
    /// Instance folder id.
    pub instance_id: String,
    /// Data root the instance lives under.
    pub data_root: PathBuf,
    /// Account to launch with.
    pub account: AccountRef,
    /// The settings that were read when the run was requested.
    pub defaults: LaunchDefaults,
}

/// A fully planned, runnable launch.
#[derive(Debug, Clone)]
pub struct LaunchPlan {
    /// Java binary that was probed successfully.
    pub java_bin: String,
    /// Full java argument vector (JVM args, `-Djava.library.path`, `-cp`, main
    /// class, game args).
    pub argv: Vec<String>,
    /// Working directory (the game root).
    pub cwd: PathBuf,
    /// Resolved main jar (verified to exist).
    pub main_jar: PathBuf,
    /// `INST_*` environment additions.
    pub envs: Vec<(String, String)>,
}

/// Outcome of [`prepare_launch`]: either a runnable plan or a refusal. The
/// accompanying log lines always explain what happened.
#[derive(Debug, Clone)]
pub enum LaunchReadiness {
    /// The game can be started with this plan.
    Ready(LaunchPlan),
    /// Launch refused; see the log lines.
    Blocked,
}

// ---- pure helpers ---------------------------------------------------------

/// Java binary names to look up on `PATH` when no `JavaPath` is configured.
pub fn candidate_java_names() -> Vec<String> {
    if cfg!(windows) {
        vec!["javaw".to_string(), "java".to_string()]
    } else {
        vec!["java".to_string()]
    }
}

/// Find `name` inside `dirs` (regular files only). On Windows a missing
/// extension also tries `.exe`. Pure and headless-testable.
pub fn find_executable_in_dirs(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    if name.is_empty() || name.contains('/') || name.contains('\\') {
        return None;
    }
    let mut candidates = vec![name.to_string()];
    if cfg!(windows) && !name.contains('.') {
        candidates.push(format!("{name}.exe"));
    }
    for dir in dirs {
        for candidate in &candidates {
            let full = dir.join(candidate);
            if full.is_file() {
                return Some(full);
            }
        }
    }
    None
}

/// `PATH` directories of this process.
pub fn path_dirs() -> Vec<PathBuf> {
    match std::env::var_os("PATH") {
        Some(raw) => std::env::split_paths(&raw).collect(),
        None => Vec::new(),
    }
}

/// Look `name` up on `PATH`.
pub fn find_on_path(name: &str) -> Option<PathBuf> {
    find_executable_in_dirs(name, &path_dirs())
}

/// Parse a Prism join-server address (`host[:port]`, default port 25565).
/// Returns `None` for blank or malformed input.
pub fn parse_server_address(raw: &str) -> Option<(String, u16)> {
    let text = raw.trim();
    if text.is_empty() {
        return None;
    }
    match text.rsplit_once(':') {
        Some((host, port_text)) => {
            let host = host.trim();
            if host.is_empty() {
                return None;
            }
            match port_text.trim().parse::<u16>() {
                Ok(port) => Some((host.to_string(), port)),
                Err(_) => None,
            }
        }
        None => Some((text.to_string(), 25565)),
    }
}

/// Classpath entry separator for this platform.
pub fn classpath_separator() -> &'static str {
    if cfg!(windows) {
        ";"
    } else {
        ":"
    }
}

/// Join classpath entries with the platform separator.
pub fn join_classpath(entries: &[String]) -> String {
    entries.join(classpath_separator())
}

/// The main jar is the last jar on the classpath (Prism puts it there).
pub fn select_main_jar(jars: &[String]) -> Option<String> {
    jars.last().cloned()
}

/// Resolve a library path from [`palantir_core::version::Library`]
/// file lists: absolute entries (instance-local libs) stay as-is,
/// `libraries/...` relatives resolve under the data root.
pub fn resolve_library_path(data_root: &Path, rel: &str) -> PathBuf {
    let path = Path::new(rel);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        data_root.join(path)
    }
}

/// Whether `xrandr` handling applies: only relevant on Linux with LWJGL2,
/// where a missing `xrandr` needs the workaround flag.
pub fn xrandr_available() -> bool {
    if System::current() == System::Linux {
        find_on_path("xrandr").is_some()
    } else {
        true
    }
}

/// Best-effort LWJGL2 detection: any active library from the legacy
/// `org.lwjgl.lwjgl` group (LWJGL3 lives under `org.lwjgl`).
pub fn has_legacy_lwjgl(profile: &palantir_core::version::LaunchProfile) -> bool {
    profile.libraries.iter().any(|lib| lib.name.group() == "org.lwjgl.lwjgl")
}

/// Spawn `command` with no console window of its own on Windows.
///
/// A GUI process that starts a console program — `java.exe`, or the `cmd /C
/// start` used to open a URL — gets a console allocated for the child, and the
/// player sees that as a command prompt appearing behind the launcher and then
/// behind the game, titled with the `java.exe` path and wearing Java's icon.
/// `CREATE_NO_WINDOW` gives the child no console at all.
///
/// This is half of the fix and [`crate::java_runtime::launcher_binary`] is the
/// other half: that one runs `javaw.exe` (no console to hide), which cannot
/// cover a `java.exe` a user configured by hand or the version probes below,
/// and this one covers everything but leaves the child on the console build.
/// Either alone leaves a path that flashes a console.
#[cfg(windows)]
fn hide_console(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

/// Nothing to hide where the console in question does not exist.
#[cfg(not(windows))]
fn hide_console(_command: &mut Command) {}

/// Open `path` in the platform file manager (best effort).
pub fn open_in_file_manager(path: &Path) -> Result<(), String> {
    let (tool, verb) = if cfg!(windows) {
        ("explorer", "opening")
    } else if cfg!(target_os = "macos") {
        ("open", "opening")
    } else {
        ("xdg-open", "opening")
    };
    match Command::new(tool).arg(path).spawn() {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("{verb} '{}' failed: {e}", path.display())),
    }
}

/// Open `url` in the default browser (best effort).
///
/// Windows needs `cmd /C start` rather than `explorer.exe` for a URL: passing a
/// URL to Explorer opens a *folder* search instead of the browser, which is the
/// kind of "it did nothing" failure worth avoiding in a sign-in flow.
pub fn open_url(url: &str) -> Result<(), String> {
    let result = if cfg!(windows) {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", "", url]);
        // `start` still opens the browser without a console of its own.
        hide_console(&mut command);
        command.spawn()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(url).spawn()
    } else {
        Command::new("xdg-open").arg(url).spawn()
    };
    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("opening '{url}' failed: {e}")),
    }
}

// ---- authentication -------------------------------------------------------

/// Build the launch session for `account`, renewing Microsoft tokens when they
/// are (or are about to be) stale.
///
/// Returns `Err` with a user-facing message when a Microsoft session cannot be
/// renewed; the caller must block the launch in that case.
pub fn prepare_auth(
    account: &AccountRef,
    auth: &MicrosoftAuth,
    log: &mut dyn FnMut(String),
) -> Result<PreparedAuth, String> {
    if !account.kind.is_online() {
        log(format!(
            "using offline account '{}' ({})",
            account.username, account.uuid
        ));
        return Ok(PreparedAuth {
            session: OfflineSession::new(account.username.clone(), account.uuid.clone())
                .into_auth_session(),
            refreshed: None,
        });
    }
    let name = account.username.clone();
    if !account.needs_refresh(palantir_core::util::now_millis()) {
        if let Some(token) = account.access_token.as_deref().filter(|token| !token.is_empty()) {
            log(format!("using the stored Microsoft session for '{name}'"));
            return Ok(PreparedAuth {
                session: msa_auth_session(&name, &account.uuid, token),
                refreshed: None,
            });
        }
    }
    let refresh = account
        .refresh_token
        .as_deref()
        .filter(|token| !token.is_empty())
        .ok_or_else(|| {
            format!(
                "'{name}' has to sign in to Microsoft again: no refresh token is stored"
            )
        })?;
    log(format!("renewing the Microsoft session for '{name}'…"));
    let msa = auth.refresh(refresh).map_err(|error| error.to_string())?;
    // A renewed MSA token still has to walk the Xbox → XSTS → launcher-login
    // chain: the game token is what the launch actually uses, and it is only
    // issued by that last hop.
    let session = auth.finish(&msa).map_err(|error| error.to_string())?;
    let expires_at_ms = palantir_core::util::now_millis() + session.expires_in.max(0) * 1000;
    log(format!(
        "signed in as '{}' ({}{})",
        session.name,
        session.uuid,
        if session.entitled { ", game owned" } else { "" }
    ));
    Ok(PreparedAuth {
        session: msa_auth_session(&session.name, &session.uuid, &session.access_token),
        refreshed: Some(RefreshedTokens {
            uuid: session.uuid,
            name: session.name,
            access_token: session.access_token,
            refresh_token: (!msa.refresh_token.is_empty()).then(|| msa.refresh_token.clone()),
            expires_at_ms,
        }),
    })
}

// ---- launch preparation ---------------------------------------------------

/// Resolve + install + plan a launch without starting anything.
///
/// The metadata store and the HTTP fetcher are injected so this is testable
/// offline: production passes an [`OnlineMetaStore`] (which caches what it
/// fetches) and a [`palantir_net::BlockingHttpFetcher`], while tests pass an
/// offline store and a map of canned bodies.
pub fn prepare_launch(
    paths: &PalantirPaths,
    instance_id: &str,
    session: &launch::AuthSession,
    defaults: &LaunchDefaults,
    store: &mut dyn MetaStore,
    fetcher: &(dyn Fetcher + Sync),
    log: &mut dyn FnMut(String),
    progress: &mut dyn FnMut(install::Progress),
) -> LaunchReadiness {
    // Resolved dir: honors the `InstanceDir` override in prismlauncher.cfg.
    let instance = match Instance::open(&paths.configured_instances_dir().join(instance_id)) {
        Ok(instance) => instance,
        Err(e) => {
            log(format!("cannot open instance '{instance_id}': {e}"));
            return LaunchReadiness::Blocked;
        }
    };
    let name = instance.name();
    let id = instance.id();
    log(format!("preparing launch of '{name}' ({id})"));

    let global = match Settings::load(&paths.global_config()) {
        Ok(settings) => settings,
        Err(_) => {
            log("no global prismlauncher.cfg found, using defaults".to_string());
            Settings::empty(paths.global_config())
        }
    };
    let model = SettingsModel::with_instance(global, instance.settings().clone());

    let profile = match PackProfile::load(&instance.mmc_pack_path()) {
        Ok(profile) => profile,
        Err(_) => {
            log("no mmc-pack.json found, using an empty component list".to_string());
            PackProfile::default()
        }
    };

    let ctx = RuntimeContext::current_host();
    let resolution = match resolve(&profile, &instance.patches_dir(), store, &ctx) {
        Ok(resolution) => resolution,
        Err(e) => {
            log(format!("resolution error: {e}"));
            return LaunchReadiness::Blocked;
        }
    };
    for problem in &resolution.problems {
        log(format!("resolve {:?}: {}", problem.severity, problem.message));
    }
    for component in &resolution.components {
        for problem in &component.problems {
            log(format!("resolve {} {:?}: {}", component.uid, problem.severity, problem.message));
        }
    }
    if resolution.severity() == ProblemSeverity::Error {
        log("resolution failed with errors — not launching".to_string());
        return LaunchReadiness::Blocked;
    }
    if resolution.profile.main_class.is_empty() {
        log("no main class resolved — metadata is incomplete, not launching".to_string());
        return LaunchReadiness::Blocked;
    }
    log(format!(
        "resolved {} component(s), main class {}",
        resolution.components.len(),
        resolution.profile.main_class
    ));

    // ---- install ---------------------------------------------------------
    let install_plan = install::plan(paths, instance.root(), &resolution.profile, &ctx);
    log(format!("install: {}", install_plan.summary()));
    for problem in &install_plan.problems {
        log(format!("install: {problem}"));
    }
    // The phases that fetch are handed a [`install::Reporter`], built from the
    // two taps this function was given: a phase that draws a bar and writes no
    // lines, or writes lines and reports no bar, is not something a caller can
    // ask for by accident.
    let report = {
        let mut reporter = install::Reporter::new(&mut *log, &mut *progress);
        install::run(&install_plan, fetcher, install::DEFAULT_THREADS, &mut reporter)
    };
    log(format!("install: {}", report.summary()));
    for failure in &report.failed {
        log(format!("install failed: {failure}"));
    }
    for problem in &report.problems {
        log(format!("install: {problem}"));
    }
    if !report.is_complete() {
        log("files are missing and could not be downloaded — not launching".to_string());
        return LaunchReadiness::Blocked;
    }

    let target = join_target(instance.settings(), log);

    let window = launch::WindowParams {
        width: model.get_i64("MinecraftWinWidth", Some("OverrideWindow"), defaults::MC_WIN_WIDTH),
        height: model.get_i64("MinecraftWinHeight", Some("OverrideWindow"), defaults::MC_WIN_HEIGHT),
        maximized: model.get_bool("LaunchMaximized", Some("OverrideWindow"), false),
    };
    let game_root = instance.game_root();
    // Every shared directory comes from the data root, in one place, because
    // this is the mapping the game reads `${assets_root}` and
    // `${library_directory}` out of: the assets are shared by every instance and
    // live at the root, not inside the instance folder.
    let mut vars = launch::instance_var_map(paths, &instance, &resolution.profile);
    vars.insert("version_name".to_string(), resolution.profile.minecraft_version.clone());
    let mc_args = launch::process_minecraft_args(&resolution.profile, Some(session), target.as_ref(), &vars);
    let script = launch::create_launch_script(
        &resolution.profile,
        Some(session),
        target.as_ref(),
        &mc_args,
        window,
        // The product's own name, not the reference client's: this line is what
        // Minecraft's title bar and crash report show, and the launcher that
        // started the game is the one that has to be named there.
        crate::brand::APP_NAME,
        env!("CARGO_PKG_VERSION"),
        &name,
    );
    log(format!("launch script ({} lines):", script.lines().count()));
    for line in script.lines() {
        log(line.to_string());
    }

    // Java: the instance's own path when it is usable, then the paths this
    // launcher's settings name, then a runtime this machine already has, and
    // only then a runtime fetched from the metadata service. A configured path
    // that is not there any more is a reason to look further rather than a
    // reason to refuse to start: the path belongs to a runtime that was
    // uninstalled or moved, and the user did not ask this launcher to stop
    // working because of it. The same goes for a path that is there but is the
    // wrong major — the Create dialog writes the settings' default Java into
    // every new instance, and a Java 21 default on a 1.12.2 instance is an
    // `UnsupportedClassVersionError` rather than an answer. Prism reads
    // `AutomaticJava` (on unless switched off) and picks a runtime it can find;
    // the same rule here means an instance keeps launching.
    let majors = &resolution.profile.compatible_java_majors;
    let configured = model.get_str("JavaPath", Some("OverrideJavaLocation"), "");
    let configured = configured.trim();
    let java_bin = if configured.is_empty() {
        String::new()
    } else if !Path::new(configured).is_file() {
        log(format!(
            "the configured JavaPath '{configured}' is not there — looking for another Java"
        ));
        String::new()
    } else {
        match java_fits_version(configured, majors) {
            Ok(()) => {
                log(format!("using the configured Java: {configured}"));
                configured.to_string()
            }
            Err(reason) => {
                log(format!("{reason} — looking for another Java"));
                String::new()
            }
        }
    };
    let java_bin = if java_bin.is_empty() {
        match pick_java(
            paths,
            &defaults.java,
            &resolution.profile.compatible_java_majors,
            &resolution.profile.compatible_java_name,
            fetcher,
            log,
            progress,
        ) {
            Some(found) => found,
            None => {
                log(
                    "no Java found: set a Java path in the Java tab of the settings, or install a Java runtime — looked under <data root>/java, in JAVA_HOME, in the standard install locations and on PATH".to_string(),
                );
                return LaunchReadiness::Blocked;
            }
        }
    } else {
        java_bin
    };
    match probe_java(&java_bin) {
        Ok(detail) => log(format!("using java '{java_bin}' ({detail})")),
        Err(e) => {
            log(e);
            return LaunchReadiness::Blocked;
        }
    }

    // Libraries: the main jar (last classpath entry) must exist. It was just
    // installed above, so a failure here means the install reported success and
    // was wrong — which is worth an explicit message rather than a spawn error.
    let files = resolution.profile.get_library_files(
        &ctx,
        Some(&instance.local_libraries_dir()),
        &instance.natives_dir(),
        false,
    );
    let main_rel = match select_main_jar(&files.jar) {
        Some(rel) => rel,
        None => {
            log("no libraries resolved — not launching".to_string());
            return LaunchReadiness::Blocked;
        }
    };
    let main_jar = resolve_library_path(&paths.root, &main_rel);
    if !main_jar.is_file() {
        log(format!(
            "main jar is missing at {} even after installing — not launching",
            main_jar.display()
        ));
        return LaunchReadiness::Blocked;
    }
    let mut missing = 0usize;
    for rel in &files.jar {
        if !resolve_library_path(&paths.root, rel).is_file() {
            missing += 1;
        }
    }
    if missing > 0 {
        log(format!(
            "warning: {missing} of {} classpath jar(s) are missing (continuing anyway)",
            files.jar.len()
        ));
    }

    let overrides_memory = instance.settings().get_bool("OverrideMemory", false);
    let (min_mem, max_mem) = heap_for_launch(&model, instance.settings(), defaults);
    log(format!(
        "heap: {min_mem} MiB to {max_mem} MiB ({})",
        if overrides_memory { "this instance's own numbers" } else { "this launcher's defaults" }
    ));
    let full_jars: Vec<String> = files
        .jar
        .iter()
        .map(|rel| resolve_library_path(&paths.root, rel).to_string_lossy().into_owned())
        .collect();
    let agents = agent_args(&resolution.profile, &ctx, &paths.root, &instance);
    let opts = launch::JavaArgsOptions {
        traits: resolution.profile.traits.clone(),
        jvm_args: model.get_str("JvmArgs", Some("OverrideJavaArgs"), ""),
        min_mem,
        max_mem,
        perm_gen: model.get_i64("PermGen", Some("OverrideMemory"), defaults::PERM_GEN),
        jar_mods_present: !resolution.profile.jar_mods.is_empty(),
        addn_jvm_arguments: resolution.profile.addn_jvm_arguments.clone(),
        agents,
        java_major: java_major(&model),
        online_fixes: resolution.profile.has_trait("legacyServices"),
        platform: System::current(),
        xrandr_available: xrandr_available(),
        has_lwjgl2: has_legacy_lwjgl(&resolution.profile),
        native_openal: None,
        native_glfw: None,
        native_sdl: None,
        window_title: name.clone(),
        token_map: vars,
    };
    let jvm_args = launch::java_arguments(&opts);
    let java_args_joined = jvm_args.join(" ");
    let mut argv = jvm_args;
    // Prism appends this in `LauncherPartLaunch`, right before the classpath:
    // it is the directory `crate::install` extracted the native jars into, and
    // without it every LWJGL native load fails at startup.
    argv.push(format!(
        "-Djava.library.path={}",
        instance.natives_dir().to_string_lossy()
    ));
    argv.push("-cp".to_string());
    argv.push(join_classpath(&full_jars));
    argv.push(resolution.profile.main_class.clone());
    argv.extend(mc_args);

    if std::fs::create_dir_all(&game_root).is_err() {
        log(format!("cannot create game directory {} — not launching", game_root.display()));
        return LaunchReadiness::Blocked;
    }
    let env_map = launch::instance_env_vars(
        &name,
        &id,
        instance.root(),
        &game_root,
        Path::new(&java_bin),
        &java_args_joined,
    );
    let mut envs: Vec<(String, String)> = Vec::new();
    for (key, value) in &env_map {
        envs.push((key.clone(), value.clone()));
    }
    log(format!("command: {java_bin} {}", argv.join(" ")));
    let plan = LaunchPlan { java_bin, argv, cwd: game_root, main_jar, envs };
    LaunchReadiness::Ready(plan)
}

/// The heap a launch uses: the instance's own numbers when it overrides memory,
/// and this launcher's defaults otherwise.
///
/// Prism's `prismlauncher.cfg` is deliberately not consulted here. The numbers on
/// the Synced settings pane are what this launcher shows, so they are what a
/// launch has to obey — a machine whose numbers began as Prism's read them once
/// when the preferences loaded ([`crate::prefs::Prefs::min_mem_mib`]), so the
/// pane and the JVM agree there too.
fn heap_for_launch(
    model: &SettingsModel,
    instance: &Settings,
    defaults: &LaunchDefaults,
) -> (i64, i64) {
    if instance.get_bool("OverrideMemory", false) {
        model.effective_memory()
    } else {
        (defaults.min_mem_mib, defaults.max_mem_mib)
    }
}

/// Build the join-server target from instance settings, logging problems.
fn join_target(settings: &Settings, log: &mut dyn FnMut(String)) -> Option<launch::LaunchTarget> {
    if !settings.get_bool("JoinServerOnLaunch", false) {
        return None;
    }
    let raw = settings.get_str("JoinServerOnLaunchAddress", "");
    match parse_server_address(&raw) {
        Some((address, port)) => {
            log(format!("will join server {address}:{port}"));
            Some(launch::LaunchTarget { address, port, world: String::new() })
        }
        None => {
            log(format!("ignoring malformed join-server address '{raw}'"));
            None
        }
    }
}

/// First PATH hit for the platform java names.
fn find_java_on_path() -> Option<String> {
    for name in candidate_java_names() {
        if let Some(found) = find_on_path(&name) {
            return Some(found.to_string_lossy().into_owned());
        }
    }
    None
}

// ---- java selection -------------------------------------------------------

/// Where a Java binary was found, for the log line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JavaOrigin {
    /// A runtime managed under the data root's `java/` folder.
    Managed,
    /// `JAVA_HOME`.
    JavaHome,
    /// A standard install location for this platform.
    Installed,
    /// Something on `PATH`.
    Path,
}

impl JavaOrigin {
    /// How the origin reads in the console.
    pub fn label(self) -> &'static str {
        match self {
            JavaOrigin::Managed => "the data root's java folder",
            JavaOrigin::JavaHome => "JAVA_HOME",
            JavaOrigin::Installed => "an installed JDK",
            JavaOrigin::Path => "PATH",
        }
    }
}

/// A Java binary worth trying, with the version it reports about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaCandidate {
    /// The executable.
    pub bin: String,
    /// Major version, when a `release` file said so. `None` means "ask it".
    pub major: Option<i64>,
    /// Where it was found.
    pub origin: JavaOrigin,
}

/// Every Java binary on this machine worth trying, best-first.
///
/// * runtimes under the data root's `java/` — both the ones this launcher
///   manages and the ones the other launcher sharing the root downloaded, in
///   Prism's two layouts (`java/<runtime>` and `java/<vendor>/<runtime>`);
/// * `JAVA_HOME`;
/// * the standard install locations ([`palantir_core::java::scan_installs`]);
/// * whatever `PATH` offers.
///
/// The version comes from each home's `release` file rather than from running
/// it, so a machine with six JDKs does not pay six processes to answer "which
/// Java is this". Anything that reports nothing is asked once, later, only if it
/// turns out to matter.
pub fn java_candidates(paths: &PalantirPaths) -> Vec<JavaCandidate> {
    let mut out: Vec<JavaCandidate> = Vec::new();
    let exe = if cfg!(windows) { "java.exe" } else { "java" };

    // `<root>/java/<runtime>` and `<root>/java/<vendor>/<runtime>`, newest
    // major first: when several are installed the newest is the one most
    // versions want, and the exact-match pass below overrides that anyway.
    let managed_root = paths.root.join("java");
    let mut homes: Vec<PathBuf> = Vec::new();
    for first in directories_in(&managed_root) {
        if first.join("bin").is_dir() {
            homes.push(first.clone());
        }
        for second in directories_in(&first) {
            if second.join("bin").is_dir() {
                homes.push(second);
            }
        }
    }
    let mut managed: Vec<JavaCandidate> = homes
        .into_iter()
        .filter_map(|home| {
            let bin = home.join("bin").join(exe);
            bin.is_file().then(|| JavaCandidate {
                bin: bin.to_string_lossy().into_owned(),
                major: home_version(&home),
                origin: JavaOrigin::Managed,
            })
        })
        .collect();
    managed.sort_by_key(|candidate| std::cmp::Reverse(candidate.major));
    out.extend(managed);

    if let Some(java_home) = std::env::var_os("JAVA_HOME").map(PathBuf::from) {
        let bin = java_home.join("bin").join(exe);
        if bin.is_file() {
            out.push(JavaCandidate {
                bin: bin.to_string_lossy().into_owned(),
                major: home_version(&java_home),
                origin: JavaOrigin::JavaHome,
            });
        }
    }

    let mut installed: Vec<JavaCandidate> = palantir_core::java::scan_installs(System::current())
        .into_iter()
        .filter_map(|install| {
            let bin = install.home.join("bin").join(exe);
            bin.is_file().then(|| JavaCandidate {
                bin: bin.to_string_lossy().into_owned(),
                major: install.version.as_ref().map(|version| version.major()),
                origin: JavaOrigin::Installed,
            })
        })
        .collect();
    installed.sort_by_key(|candidate| std::cmp::Reverse(candidate.major));
    out.extend(installed);

    if let Some(found) = find_java_on_path() {
        out.push(JavaCandidate { bin: found, major: None, origin: JavaOrigin::Path });
    }
    out
}

/// Directories directly inside `dir`, sorted, ignoring anything unreadable.
fn directories_in(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    found.sort();
    found
}

/// The major version a Java home's `release` file reports, without running it.
fn home_version(home: &Path) -> Option<i64> {
    palantir_core::java::probe_home(home)
        .and_then(|install| install.version)
        .map(|version| version.major())
}

/// The major version in a `java -version` first line.
///
/// The line is `openjdk version "21.0.6" 2025-01-21` (or `java version
/// "1.8.0_402"` for the last of the 1.x line), so the number is the quoted
/// token — not the first number in the line, which for some vendors is a build
/// date.
pub fn major_from_version_output(text: &str) -> Option<i64> {
    let start = text.find('"')? + 1;
    let rest = &text[start..];
    let end = rest.find('"')?;
    let quoted = &rest[..end];
    (!quoted.is_empty()).then(|| palantir_core::java::JavaVersion::parse(quoted).major())
}

/// The major a Java binary is: from its home's `release` file when there is
/// one, and from running it when there is not.
///
/// The `release` file is preferred because the answer decides a launch: a
/// machine with six JDKs should not pay six processes to say which is which.
fn java_major_of(bin: &str) -> Option<i64> {
    let home = Path::new(bin).parent().and_then(Path::parent);
    home.and_then(home_version).or_else(|| probe_java_major(bin))
}

/// Why the Java at `bin` may not be used for a version that accepts `majors`.
///
/// A runtime the user named outranks automatic selection — that is what naming
/// it means — but it does not outrank the version's own requirement, because
/// then naming one Java breaks every instance that needs another. So a Java
/// that is there but is the wrong major is looked past, and the search carries
/// on.
///
/// Two cases are deliberately *not* refusals: a version that declares no majors
/// has nothing to disagree with, and a Java whose version could not be read is
/// left alone rather than second-guessed — it may be a wrapper or a launcher
/// script that answers perfectly well when it is run.
fn java_fits_version(bin: &str, majors: &[i64]) -> Result<(), String> {
    if majors.is_empty() {
        return Ok(());
    }
    let Some(major) = java_major_of(bin) else {
        return Ok(());
    };
    if majors.contains(&major) {
        return Ok(());
    }
    let wants = majors
        .iter()
        .map(|major| major.to_string())
        .collect::<Vec<_>>()
        .join(" or ");
    Err(format!(
        "the configured Java '{bin}' is Java {major}, but this version wants Java {wants}"
    ))
}

/// Ask a binary its version. `None` when it cannot be run at all.
fn probe_java_major(bin: &str) -> Option<i64> {
    let mut command = Command::new(bin);
    command.arg("-version");
    // A probe is a whole `java.exe` start; without this the console it would
    // allocate blinks on screen every time a launch picks a Java.
    hide_console(&mut command);
    let output = command.output().ok()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    major_from_version_output(&stderr).or_else(|| major_from_version_output(&stdout))
}

/// The candidate to run with, given what the version asked for.
///
/// Pure, so the rule is testable without a machine that happens to have the
/// right JDK on it: a candidate whose major is in `want_majors` wins, in list
/// order; failing that the highest known major; failing that the first
/// candidate, whose version is unknown.
pub fn best_java(candidates: &[JavaCandidate], want_majors: &[i64]) -> Option<JavaCandidate> {
    candidates
        .iter()
        .find(|candidate| {
            candidate.major.map(|major| want_majors.contains(&major)).unwrap_or(false)
        })
        .or_else(|| {
            candidates
                .iter()
                .filter(|candidate| candidate.major.is_some())
                .max_by_key(|candidate| candidate.major.unwrap_or(0))
        })
        .or_else(|| candidates.first())
        .cloned()
}

/// Choose a Java for this version, saying what was chosen and why.
///
/// The order is five answers to one question, from the most specific promise to
/// the last resort:
///
/// 1. the binary the Java tab names for a major this version accepts;
/// 2. the tab's default binary;
/// 3. a runtime this machine already has at an accepted major;
/// 4. the runtime the version names, fetched from the metadata service;
/// 5. the best runtime there is, with the mismatch said out loud.
///
/// Steps 1 and 2 are what makes the Java tab real: a user with a JDK 8 and a
/// JDK 21 has already answered "which one", and scanning for an answer anyway
/// would ignore them. Step 4 is the difference between "install a Java first"
/// and a game that starts on a machine that never had one.
fn pick_java(
    paths: &PalantirPaths,
    java: &JavaPrefs,
    want_majors: &[i64],
    want_name: &str,
    fetcher: &(dyn Fetcher + Sync),
    log: &mut dyn FnMut(String),
    progress: &mut dyn FnMut(install::Progress),
) -> Option<String> {
    // 1 and 2: what this launcher's own settings say.
    for (major, path) in java.for_majors(want_majors) {
        if Path::new(path).is_file() {
            log(format!("using Java {major} from the Java settings: {path}"));
            return Some(path.to_string());
        }
        log(format!(
            "the Java the settings name for major {major} is not there: {path}"
        ));
    }
    if let Some(path) = java.default_binary() {
        if Path::new(path).is_file() {
            log(format!("using the default Java from the settings: {path}"));
            return Some(path.to_string());
        }
        log(format!("the default Java in the settings is not there: {path}"));
    }

    // 3: what the machine has. A candidate that never told us its version is
    // asked now, once — but only here, where the answer decides whether it is
    // used.
    let candidates: Vec<JavaCandidate> = java_candidates(paths)
        .into_iter()
        .map(|mut candidate| {
            if candidate.major.is_none() {
                candidate.major = probe_java_major(&candidate.bin);
            }
            candidate
        })
        .collect();
    let compatible = candidates.iter().find(|candidate| {
        candidate
            .major
            .map(|major| want_majors.contains(&major))
            .unwrap_or(false)
    });
    if let Some(found) = compatible {
        log(format!(
            "using {} (Java {}, from {})",
            found.bin,
            found.major.unwrap_or(0),
            found.origin.label()
        ));
        return Some(found.bin.clone());
    }

    // 4: nothing here fits, so ask the service for the runtime the version
    // names. A failure is logged rather than fatal — step 5 still has an answer,
    // and a machine with the wrong Java that explains itself beats no launch at
    // all.
    let host = java_runtime::host_runtime_os(&RuntimeContext::current_host());
    let request = java_runtime::RuntimeRequest {
        base_url: palantir_net::DEFAULT_META_BASE_URL,
        majors: want_majors,
        name: want_name,
        os: &host,
    };
    let fetched = {
        let mut reporter = install::Reporter::new(&mut *log, &mut *progress);
        java_runtime::ensure_runtime(
            paths,
            &request,
            fetcher,
            java_runtime::DOWNLOAD_THREADS,
            &mut reporter,
        )
    };
    match fetched {
        Ok(bin) => return Some(bin),
        Err(reason) => log(format!(
            "no Java for this version could be fetched: {reason}"
        )),
    }

    // 5: the best there is, with the mismatch named.
    let chosen = best_java(&candidates, &[])?;
    match chosen.major {
        Some(major) if !want_majors.is_empty() => log(format!(
            "using {} (Java {major}, from {}), but this version wants Java {} — expect an UnsupportedClassVersionError if it does not start",
            chosen.bin,
            chosen.origin.label(),
            want_majors
                .iter()
                .map(|major| major.to_string())
                .collect::<Vec<_>>()
                .join(" or ")
        )),
        Some(major) => log(format!(
            "using {} (Java {major}, from {})",
            chosen.bin,
            chosen.origin.label()
        )),
        None => log(format!(
            "using {} (version unknown, from {})",
            chosen.bin,
            chosen.origin.label()
        )),
    }
    Some(chosen.bin)
}

/// Probe a java binary with `java -version`; returns the first output line.
fn probe_java(java_bin: &str) -> Result<String, String> {
    let mut command = Command::new(java_bin);
    command.arg("-version");
    hide_console(&mut command);
    match command.output() {
        Ok(output) => {
            if !output.status.success() {
                return Err(format!("java probe failed for '{java_bin}' (exit {})", output.status));
            }
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let first = match stderr.lines().next() {
                Some(line) => line.to_string(),
                None => match stdout.lines().next() {
                    Some(line) => line.to_string(),
                    None => "version output empty".to_string(),
                },
            };
            Ok(first.trim().to_string())
        }
        Err(e) => Err(format!("java probe failed for '{java_bin}': {e}")),
    }
}

/// Effective Java major for JVM flag selection (17 when unknown).
fn java_major(model: &SettingsModel) -> i64 {
    let raw = model.get_str("JavaVersion", Some("OverrideJavaLocation"), "");
    if raw.trim().is_empty() {
        return 17;
    }
    let major = JavaVersion::parse(raw.trim()).major();
    if major <= 0 {
        17
    } else {
        major
    }
}

/// `-javaagent` arguments with resolved jar paths.
fn agent_args(
    profile: &palantir_core::version::LaunchProfile,
    ctx: &RuntimeContext,
    data_root: &Path,
    instance: &Instance,
) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for agent in &profile.agents {
        let files = agent.library.applicable_files(ctx, Some(&instance.local_libraries_dir()));
        let first = match files.jar.first() {
            Some(first) => first.clone(),
            None => continue,
        };
        let full = resolve_library_path(data_root, &first).to_string_lossy().into_owned();
        out.push((full, agent.argument.clone()));
    }
    out
}

// ---- playtime -------------------------------------------------------------

/// Stamp `lastLaunchTime` the way Prism does when the game starts.
///
/// A failure is reported but never blocks a launch: the game is more important
/// than its statistics.
pub fn record_launch_start(paths: &PalantirPaths, instance_id: &str, log: &mut dyn FnMut(String)) {
    match Instance::open(&paths.configured_instances_dir().join(instance_id)) {
        Ok(mut instance) => {
            instance.mark_launched();
            if let Err(error) = instance.save() {
                log(format!("could not record the launch time: {error}"));
            }
        }
        Err(error) => log(format!("could not record the launch time: {error}")),
    }
}

/// Add elapsed seconds to `totalTimePlayed` when the game exits.
pub fn record_play_time(
    paths: &PalantirPaths,
    instance_id: &str,
    elapsed: Duration,
    log: &mut dyn FnMut(String),
) {
    let seconds = elapsed.as_secs().min(i64::MAX as u64) as i64;
    if seconds <= 0 {
        return;
    }
    match Instance::open(&paths.configured_instances_dir().join(instance_id)) {
        Ok(mut instance) => {
            instance.add_play_time_secs(seconds);
            let total = instance.total_time_played_secs();
            if let Err(error) = instance.save() {
                log(format!("could not save the play time: {error}"));
            } else {
                log(format!("played for {seconds}s ({total}s in total)"));
            }
        }
        Err(error) => log(format!("could not save the play time: {error}")),
    }
}

/// The metadata store and HTTP client a real launch uses.
pub fn online_backend(paths: &PalantirPaths) -> (palantir_net::OnlineMetaStore, palantir_net::BlockingHttpFetcher) {
    (
        palantir_net::OnlineMetaStore::new(palantir_net::DEFAULT_META_BASE_URL, paths.meta_dir()),
        palantir_net::BlockingHttpFetcher::new(Duration::from_secs(30)),
    )
}

/// The offline counterpart, for the sandbox shell and for tests: everything
/// must already be cached and installed.
#[allow(dead_code)]
pub fn offline_backend(paths: &PalantirPaths) -> (OfflineMetaStore, palantir_net::MapFetcher) {
    (OfflineMetaStore::new(paths.meta_dir()), palantir_net::MapFetcher::new())
}

// ---- worker ---------------------------------------------------------------

/// Run one launch in a background thread: sign in, resolve, install, spawn the
/// child with piped output, stream batches + a final done message through the
/// subscription sender. Never reports success it did not observe.
pub fn run_launch_worker(params: LaunchParams, slot: ChildSlot, sender: Sender<Message>) {
    let paths = PalantirPaths::at(&params.data_root);
    let run_id = params.run_id;
    let mut sender = sender;
    // Two things happen to every line before the console sees it: secrets are
    // masked, and the line is written to the instance. One object, because a line
    // that reached the screen without passing through both is the bug this
    // exists to prevent.
    let journal = LaunchLogFile::open(&paths, &params.instance_id);
    if !send_batch(
        &mut sender,
        run_id,
        vec![format!("writing this run to {}", journal.path().display())],
        &journal,
    ) {
        return;
    }
    // Sent after the preparation block: the log closure below owns the sender
    // for the duration of that block.
    let mut tokens_message: Option<Message> = None;

    // Each line of the preparation block goes out as it happens rather than
    // being held back into batches. Batching is right for the game's own output,
    // which arrives hundreds of lines at a time, but this phase is I/O-bound and
    // now reports progress while it works — and a buffer that only left the
    // closure every 25 lines is exactly what made a long asset download look
    // frozen instead of busy.
    //
    // The two taps are separate closures over separate *handles* rather than one
    // closure over `sender`: both need to send, and `Sender::try_send` wants a
    // `&mut`, which only one borrower can hold at a time. Cloning a channel
    // handle is how a futures mpsc sender is shared, and neither closure outlives
    // this block.
    let readiness = {
        let mut lines_sender = sender.clone();
        let mut progress_sender = sender.clone();
        let mut log = |line: String| {
            let _ = send_batch(&mut lines_sender, run_id, vec![line], &journal);
        };
        let mut progress = |level: install::Progress| {
            send_progress(&mut progress_sender, run_id, level);
        };
        let auth = MicrosoftAuth::with_public_client_id();
        let prepared = match prepare_auth(&params.account, &auth, &mut log) {
            Ok(prepared) => Some(prepared),
            Err(error) => {
                log(format!("sign-in failed: {error} — not launching"));
                None
            }
        };
        if let Some(refreshed) = prepared.as_ref().and_then(|prepared| prepared.refreshed.clone()) {
            tokens_message = Some(Message::AccountTokens {
                uuid: refreshed.uuid,
                name: refreshed.name,
                access_token: refreshed.access_token,
                refresh_token: refreshed.refresh_token,
                expires_at_ms: refreshed.expires_at_ms,
            });
        }
        match prepared {
            Some(prepared) => {
                let (mut store, fetcher) = online_backend(&paths);
                prepare_launch(
                    &paths,
                    &params.instance_id,
                    &prepared.session,
                    &params.defaults,
                    &mut store,
                    &fetcher,
                    &mut log,
                    &mut progress,
                )
            }
            None => LaunchReadiness::Blocked,
        }
    };
    if let Some(message) = tokens_message {
        if !send_message(&mut sender, message) {
            return;
        }
    }
    let plan = match readiness {
        LaunchReadiness::Blocked => {
            // The console already explains why; this is the terminal line.
            send_done(&mut sender, run_id, "launch blocked (see console)".to_string());
            return;
        }
        LaunchReadiness::Ready(plan) => plan,
    };
    {
        let mut log = |line: String| {
            let _ = send_batch(&mut sender, run_id, vec![line], &journal);
        };
        record_launch_start(&paths, &params.instance_id, &mut log);
    }
    send_batch(
        &mut sender,
        run_id,
        vec![format!(
            "spawning '{}' (main jar {})",
            plan.java_bin,
            plan.main_jar.display()
        )],
        &journal,
    );
    let mut command = Command::new(&plan.java_bin);
    command
        .args(&plan.argv)
        .current_dir(&plan.cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in &plan.envs {
        command.env(key, value);
    }
    // The game must not be born with a console behind it.
    hide_console(&mut command);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            send_batch(
                &mut sender,
                run_id,
                vec![format!("failed to start '{}': {e} — not launching", plan.java_bin)],
                &journal,
            );
            send_done(&mut sender, run_id, "failed to start".to_string());
            return;
        }
    };
    let started = Instant::now();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    match slot.lock() {
        Ok(mut guard) => {
            *guard = Some(child);
        }
        Err(_) => {
            send_batch(
                &mut sender,
                run_id,
                vec!["internal lock error: cannot track child".to_string()],
                &journal,
            );
            send_done(&mut sender, run_id, "internal lock error".to_string());
            let _ = child.kill();
            let _ = child.wait();
            return;
        }
    }
    send_batch(
        &mut sender,
        run_id,
        vec!["process started, streaming output…".to_string()],
        &journal,
    );

    let (tx, rx) = mpsc::channel::<(bool, String)>();
    spawn_reader(stdout, false, tx.clone());
    spawn_reader(stderr, true, tx);
    // Worker loop: batch whatever the readers deliver.
    let mut buf: Vec<String> = Vec::new();
    loop {
        match rx.recv_timeout(Duration::from_millis(60)) {
            Ok((is_stderr, line)) => {
                if is_stderr {
                    buf.push(format!("[stderr] {line}"));
                } else {
                    buf.push(line);
                }
                if buf.len() >= 100 {
                    let batch = std::mem::take(&mut buf);
                    if !send_batch(&mut sender, run_id, batch, &journal) {
                        kill_slot(&slot);
                        return;
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if !buf.is_empty() {
                    let batch = std::mem::take(&mut buf);
                    if !send_batch(&mut sender, run_id, batch, &journal) {
                        kill_slot(&slot);
                        return;
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if !buf.is_empty() {
                    let batch = std::mem::take(&mut buf);
                    if !send_batch(&mut sender, run_id, batch, &journal) {
                        kill_slot(&slot);
                        return;
                    }
                }
                break;
            }
        }
    }
    let elapsed = started.elapsed();
    let outcome = match slot.lock() {
        Ok(mut guard) => match guard.take() {
            Some(mut child) => match child.wait() {
                Ok(status) => format!("process exited ({status})"),
                Err(e) => format!("waiting for process failed: {e}"),
            },
            None => "process handle already gone".to_string(),
        },
        Err(_) => "internal lock error while reaping process".to_string(),
    };
    {
        let mut log = |line: String| {
            let _ = send_batch(&mut sender, run_id, vec![line], &journal);
        };
        record_play_time(&paths, &params.instance_id, elapsed, &mut log);
    }
    send_batch(&mut sender, run_id, vec![outcome.clone()], &journal);
    send_done(&mut sender, run_id, outcome);
}

/// Best-effort kill of whatever the slot currently holds.
fn kill_slot(slot: &ChildSlot) {
    if let Ok(mut guard) = slot.lock() {
        if let Some(child) = guard.as_mut() {
            let _ = child.kill();
        }
    }
}

/// Read one pipe to EOF, forwarding lines. Ends when the pipe closes or the
/// GUI is gone.
fn spawn_reader<R: std::io::Read + Send + 'static>(
    pipe: Option<R>,
    is_stderr: bool,
    tx: mpsc::Sender<(bool, String)>,
) {
    let _ = std::thread::spawn(move || {
        let pipe = match pipe {
            Some(pipe) => pipe,
            None => return,
        };
        let reader = std::io::BufReader::new(pipe);
        for line in reader.lines() {
            match line {
                Ok(text) => {
                    if tx.send((is_stderr, text)).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
}

/// Send one message, waiting while the channel is full; `false` means the GUI is
/// gone.
fn send_message(sender: &mut Sender<Message>, message: Message) -> bool {
    let mut pending = Some(message);
    loop {
        let message = match pending.take() {
            Some(message) => message,
            None => return true,
        };
        match sender.try_send(message) {
            Ok(()) => return true,
            Err(error) if error.is_full() => {
                pending = Some(error.into_inner());
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(_) => return false,
        }
    }
}

/// What a launch line carries that must not be written down.
///
/// A launch command is `java … --accessToken <token> … --clientId <id>`, and
/// those tokens are not only printed: the game echoes its own command line in
/// crash reports, and a player pasting a log into a bug report is the normal way
/// these leak. Masking the value that follows the flag covers all three shapes —
/// the command, the launch script, and the game's echo of them — because all
/// three spell the flags the same way.
const SECRET_FLAGS: [&str; 4] = ["--accessToken", "--clientId", "--xuid", "--session"];

/// What a masked secret is replaced with.
pub const REDACTED: &str = "<redacted>";

/// Hide the credentials a line can carry, keeping everything else byte for byte.
///
/// Both spellings of a flag are handled (`--accessToken value` and
/// `--accessToken=value`) and the separator is kept, so a redacted line still
/// reads as the command that ran. A flag with nothing after it is left alone:
/// naming the flag says nothing secret.
pub fn redact(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut cursor = 0usize;
    while cursor < line.len() {
        let found = SECRET_FLAGS
            .iter()
            .filter_map(|flag| line[cursor..].find(flag).map(|at| (cursor + at, flag.len())))
            .min_by_key(|(at, _)| *at);
        let Some((start, flag_len)) = found else {
            out.push_str(&line[cursor..]);
            break;
        };
        let after = &line[start + flag_len..];
        // A value has to be *separated* from its flag: whitespace or `=`.
        // Anything else means the flag is part of a longer token and carries
        // nothing — `--accessToken,` names a flag without a secret, and masking
        // the punctuation would corrupt the line while protecting nobody.
        if !matches!(after.chars().next(), Some(c) if c.is_whitespace() || c == '=') {
            out.push_str(&line[cursor..start + flag_len]);
            cursor = start + flag_len;
            continue;
        }
        // Everything up to the value is kept verbatim: whitespace, or an `=`
        // with whitespace after it.
        let value_at = after
            .find(|c: char| !c.is_whitespace() && c != '=')
            .unwrap_or(after.len());
        let value = &after[value_at..];
        let value_len = value.find(char::is_whitespace).unwrap_or(value.len());
        out.push_str(&line[cursor..start + flag_len + value_at]);
        if value_len > 0 {
            out.push_str(REDACTED);
        }
        cursor = start + flag_len + value_at + value_len;
    }
    out
}

/// The on-disk copy of a run's console.
///
/// One file per instance, next to the game's own logs, because "why did this
/// instance not start" is answered by the lines that scrolled past while it was
/// failing — and until this existed they were gone the moment the window closed
/// or the process was killed, which is exactly when somebody is looking for
/// them.
///
/// The previous run is rotated to `.1` rather than appended to: retrying a
/// launch is the moment the log of the failed attempt matters most, and a single
/// growing file would have overwritten the evidence with the retry.
pub struct LaunchLogFile {
    path: PathBuf,
}

impl LaunchLogFile {
    /// Open (creating) the log for `instance_id`, rotating the previous run.
    ///
    /// Best effort throughout: a log that cannot be written is a missing
    /// convenience, never a reason to refuse to launch the game.
    pub fn open(paths: &PalantirPaths, instance_id: &str) -> LaunchLogFile {
        let path = paths
            .configured_instances_dir()
            .join(instance_id)
            .join("logs")
            .join("launcher.log");
        if path.is_file() {
            let previous = path.with_extension("log.1");
            let _ = std::fs::remove_file(&previous);
            let _ = std::fs::rename(&path, &previous);
        }
        let log = LaunchLogFile { path };
        log.append(&[format!(
            "{} {} — launching '{instance_id}'",
            crate::brand::APP_NAME,
            crate::brand::version()
        )]);
        log
    }

    /// Where the log is, whether or not anything could be written to it yet.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append lines, creating the directory and file when they are missing.
    pub fn append(&self, lines: &[String]) {
        use std::io::Write;
        let Some(parent) = self.path.parent() else {
            return;
        };
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
        let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&self.path)
        else {
            return;
        };
        for line in lines {
            let _ = writeln!(file, "{line}");
        }
    }

}

/// Forward one progress report; the GUI keeps only the latest.
///
/// Deliberately unlike [`send_batch`]: a report that cannot be delivered right
/// now is *dropped* rather than retried. A progress level is not an event — the
/// next one supersedes it, and every one of them is a fact the window already
/// knows how to lose — so blocking the downloader on a window that is a frame
/// behind would be the wrong trade twice over. Nothing here touches the journal:
/// a level is not a line, and a log that grows by a line per 2% of a download is
/// a log nobody reads.
fn send_progress(sender: &mut Sender<Message>, run_id: u64, progress: install::Progress) {
    let _ = sender.try_send(Message::LaunchProgress { run_id, progress });
}

/// Forward one batch; `false` means the GUI is gone and the worker should stop.
///
/// Every line passes through here — the launcher's own account of the launch and
/// the game's output alike — so this is the one place that has to redact, and the
/// one place that writes the log to disk.
fn send_batch(
    sender: &mut Sender<Message>,
    run_id: u64,
    lines: Vec<String>,
    journal: &LaunchLogFile,
) -> bool {
    if lines.is_empty() {
        return true;
    }
    let lines: Vec<String> = lines.into_iter().map(|line| redact(&line)).collect();
    journal.append(&lines);
    let mut pending: Option<Vec<String>> = Some(lines);
    loop {
        let batch = match pending.take() {
            Some(batch) => batch,
            None => return true,
        };
        match sender.try_send(Message::LaunchLog { run_id, lines: batch }) {
            Ok(()) => return true,
            Err(e) => {
                if e.is_full() {
                    match e.into_inner() {
                        Message::LaunchLog { lines, .. } => {
                            pending = Some(lines);
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        _ => return false,
                    }
                } else {
                    return false;
                }
            }
        }
    }
}

/// Deliver the final outcome (retries while the channel is full).
fn send_done(sender: &mut Sender<Message>, run_id: u64, note: String) {
    let mut pending: Option<String> = Some(note);
    loop {
        let note = match pending.take() {
            Some(note) => note,
            None => return,
        };
        match sender.try_send(Message::LaunchDone { run_id, note }) {
            Ok(()) => return,
            Err(e) => {
                if e.is_full() {
                    match e.into_inner() {
                        Message::LaunchDone { note, .. } => {
                            pending = Some(note);
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        _ => return,
                    }
                } else {
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use palantir_core::pack::Component;
    use palantir_net::{MapFetcher, MapTransport, MicrosoftOAuth};

    #[test]
    fn server_addresses_parse_with_default_port() {
        assert_eq!(parse_server_address("mc.example.com"), Some(("mc.example.com".to_string(), 25565)));
        assert_eq!(parse_server_address("mc.example.com:25570"), Some(("mc.example.com".to_string(), 25570)));
        assert_eq!(parse_server_address("  mc.example.com : 1234 "), Some(("mc.example.com".to_string(), 1234)));
        assert_eq!(parse_server_address(""), None);
        assert_eq!(parse_server_address("   "), None);
        assert_eq!(parse_server_address("host:notaport"), None);
        assert_eq!(parse_server_address(":25565"), None);
    }

    /// A data root holding managed runtimes in both of Prism's layouts.
    fn root_with_managed_java(specs: &[(&str, &str)]) -> (tempfile::TempDir, PalantirPaths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = PalantirPaths::at(dir.path());
        let exe = if cfg!(windows) { "java.exe" } else { "java" };
        for (relative, version) in specs {
            let home = paths.root.join("java").join(relative);
            std::fs::create_dir_all(home.join("bin")).unwrap();
            std::fs::write(home.join("bin").join(exe), b"").unwrap();
            std::fs::write(
                home.join("release"),
                format!("JAVA_VERSION=\"{version}\"\nJAVA_VENDOR=\"Test\"\nOS_ARCH=\"amd64\"\n"),
            )
            .unwrap();
        }
        (dir, paths)
    }

    /// A JDK home with the `release` file a real one carries, holding a binary
    /// that is a file but not a JVM.
    ///
    /// Writing `release` is what lets the java-selection tests be about
    /// selection instead of about which JDK the machine running them happens to
    /// have: the version is read from the home, and nothing is executed until a
    /// test deliberately launches with one of these.
    fn fake_java_home(dir: &Path, name: &str, version: &str) -> String {
        let home = dir.join(name);
        std::fs::create_dir_all(home.join("bin")).unwrap();
        let exe = if cfg!(windows) { "java.exe" } else { "java" };
        let bin = home.join("bin").join(exe);
        std::fs::write(&bin, b"").unwrap();
        std::fs::write(home.join("release"), format!("JAVA_VERSION=\"{version}\"\n")).unwrap();
        bin.to_string_lossy().into_owned()
    }

    #[test]
    fn a_java_the_version_cannot_run_on_is_not_used_just_because_it_was_named() {
        let dir = tempfile::tempdir().unwrap();
        let eight = fake_java_home(dir.path(), "jdk8", "1.8.0_402");
        let twenty_one = fake_java_home(dir.path(), "jdk21", "21.0.6");

        // The major comes from the home's own file, without running anything.
        assert_eq!(java_major_of(&eight), Some(8));
        assert_eq!(java_major_of(&twenty_one), Some(21));

        // Naming a Java is a promise about that Java, and it is kept: it is used
        // for the major it is, and a version that accepts either of two keeps
        // it. So does a version that declares nothing at all.
        assert!(java_fits_version(&twenty_one, &[21]).is_ok());
        assert!(java_fits_version(&eight, &[8, 17]).is_ok());
        assert!(java_fits_version(&eight, &[]).is_ok());

        // What it does not outrank is the version's own requirement: a Java 21
        // named for a version that runs on 8 is exactly what an
        // UnsupportedClassVersionError looks like from here, so it is refused
        // with both numbers in the reason.
        let reason = java_fits_version(&twenty_one, &[8]).unwrap_err();
        assert!(reason.contains("Java 21"), "{reason}");
        assert!(reason.contains("wants Java 8"), "{reason}");
        let reason = java_fits_version(&eight, &[17, 21]).unwrap_err();
        assert!(reason.contains("wants Java 17 or 21"), "{reason}");

        // A binary whose version cannot be read is not refused — it may be a
        // wrapper, and refusing a Java the user named by hand would be this
        // launcher overruling them on a question it could not answer.
        let bare = dir.path().join("wrapper").join(if cfg!(windows) { "java.exe" } else { "java" });
        std::fs::create_dir_all(bare.parent().unwrap()).unwrap();
        std::fs::write(&bare, b"not a JVM").unwrap();
        assert_eq!(java_major_of(&bare.to_string_lossy()), None);
        assert!(java_fits_version(&bare.to_string_lossy(), &[21]).is_ok());
    }

    /// The Synced settings pane is the answer for the heap, not Prism's file.
    ///
    /// A machine where both exist is the case that matters: the pane shows one
    /// number, so the JVM has to get that number, or the pane is wrong about the
    /// machine it describes.
    #[test]
    fn the_heap_a_launch_uses_is_the_one_the_pane_shows() {
        let prism = || {
            let mut settings = Settings::empty("prismlauncher.cfg");
            settings.set_i64("MinMemAlloc", 128);
            settings.set_i64("MaxMemAlloc", 2048);
            settings
        };
        let defaults = LaunchDefaults {
            min_mem_mib: 1024,
            max_mem_mib: 8192,
            ..LaunchDefaults::default()
        };

        // No override: this launcher's numbers, even though the global file
        // names different ones.
        let instance = Settings::empty("instance.cfg");
        let model = SettingsModel::with_instance(prism(), instance.clone());
        assert_eq!(heap_for_launch(&model, &instance, &defaults), (1024, 8192));

        // Override on: the instance's numbers win, which is what the gate means.
        let mut instance = Settings::empty("instance.cfg");
        instance.set_bool("OverrideMemory", true);
        instance.set_i64("MinMemAlloc", 256);
        instance.set_i64("MaxMemAlloc", 4096);
        let model = SettingsModel::with_instance(prism(), instance.clone());
        assert_eq!(heap_for_launch(&model, &instance, &defaults), (256, 4096));

        // A gate with nothing behind it falls to the global file, which is what
        // a half-written instance.cfg gets in Prism too.
        let mut bare = Settings::empty("instance.cfg");
        bare.set_bool("OverrideMemory", true);
        let model = SettingsModel::with_instance(prism(), bare.clone());
        assert_eq!(heap_for_launch(&model, &bare, &defaults), (128, 2048));
    }

    #[test]
    fn the_defaults_are_read_from_the_preferences_rather_than_left_at_zero() {
        let mut prefs = crate::prefs::Prefs::default();
        prefs.default_min_mem_mib = Some(1024);
        prefs.default_max_mem_mib = Some(8192);
        prefs.default_java_path = Some("C:/jdk21/bin/javaw.exe".to_string());
        let defaults = LaunchDefaults::from_prefs(&prefs);
        assert_eq!((defaults.min_mem_mib, defaults.max_mem_mib), (1024, 8192));
        assert_eq!(defaults.java.default_binary(), Some("C:/jdk21/bin/javaw.exe"));

        // Nothing set anywhere: the shipped numbers, because a heap of zero
        // megabytes is a launch that dies in the JVM rather than a default.
        let shipping = LaunchDefaults::from_prefs(&crate::prefs::Prefs::default());
        assert_eq!(
            (shipping.min_mem_mib, shipping.max_mem_mib),
            (
                i64::from(crate::prefs::DEFAULT_MIN_MEM_MIB),
                i64::from(crate::prefs::DEFAULT_MAX_MEM_MIB)
            )
        );
    }

    #[test]
    fn a_managed_runtime_is_found_in_both_layouts_and_reads_its_own_version() {
        // Prism keeps downloaded runtimes in `java/<runtime>` on newer builds
        // and `java/<vendor>/<runtime>` in others, and the instance that was
        // configured against one keeps naming it. Both have to be found — the
        // version comes from the runtime's own `release` file, without running
        // six JDKs to ask them.
        let (_dir, paths) = root_with_managed_java(&[
            ("java-runtime-delta", "21.0.6"),
            ("adoptium/java-runtime-gamma", "17.0.12"),
        ]);
        let managed: Vec<JavaCandidate> = java_candidates(&paths)
            .into_iter()
            .filter(|candidate| candidate.origin == JavaOrigin::Managed)
            .collect();
        assert_eq!(managed.len(), 2, "got {managed:#?}");
        // Newest major first, so a tie is broken toward the newer runtime.
        assert_eq!(managed[0].major, Some(21));
        assert_eq!(managed[1].major, Some(17));
        // Compared as path components rather than as a string: the separator is
        // not the same on every platform these tests run on.
        let expected = if cfg!(windows) {
            "java-runtime-delta/bin/java.exe"
        } else {
            "java-runtime-delta/bin/java"
        };
        assert!(
            Path::new(&managed[0].bin).ends_with(expected),
            "got {} for the newest managed runtime",
            managed[0].bin
        );
    }

    #[test]
    fn java_that_does_not_exist_is_not_a_candidate() {
        // A `java` folder with no executable in it — the shape a half-finished
        // download leaves behind — must not be offered.
        let (_dir, paths) = root_with_managed_java(&[("broken", "21.0.6")]);
        let broken = paths.root.join("java").join("broken").join("bin");
        let exe = if cfg!(windows) { "java.exe" } else { "java" };
        std::fs::remove_file(broken.join(exe)).unwrap();
        assert!(java_candidates(&paths)
            .iter()
            .all(|candidate| candidate.origin != JavaOrigin::Managed));
    }

    #[test]
    fn the_version_a_java_reports_is_read_from_the_quoted_token() {
        // `java -version` prints the version in quotes, and for some vendors
        // the first digits on the line are a build date.
        assert_eq!(major_from_version_output("openjdk version \"21.0.6\" 2025-01-21"), Some(21));
        assert_eq!(major_from_version_output("java version \"1.8.0_402\""), Some(8));
        assert_eq!(major_from_version_output("openjdk 2025-01-21 version 21"), None);
        assert_eq!(major_from_version_output(""), None);
        assert_eq!(major_from_version_output("\"\""), None);
    }

    #[test]
    fn the_version_asked_for_wins_over_a_newer_one() {
        let candidate = |major: Option<i64>, origin, bin: &str| JavaCandidate {
            bin: bin.to_string(),
            major,
            origin,
        };
        let candidates = vec![
            candidate(Some(23), JavaOrigin::Managed, "newest"),
            candidate(Some(21), JavaOrigin::Installed, "wanted"),
            candidate(Some(17), JavaOrigin::Managed, "older"),
        ];
        // The version's own requirement decides, not "the newest" — that is
        // how a 1.21 instance ends up on a Java 23 that its mods reject.
        assert_eq!(best_java(&candidates, &[21]).unwrap().bin, "wanted");
        // Two acceptable majors: the earlier candidate in the list wins, and
        // the list is already ordered by how good the runtime is.
        assert_eq!(best_java(&candidates, &[23, 17]).unwrap().bin, "newest");
        // Nothing matches: the newest there is, rather than nothing at all.
        assert_eq!(best_java(&candidates, &[8]).unwrap().bin, "newest");
        // No requirement: the newest.
        assert_eq!(best_java(&candidates, &[]).unwrap().bin, "newest");

        // A candidate whose version could not be read is used only when there
        // is nothing else — an unknown runtime should never beat a known one.
        let unknown = vec![
            candidate(None, JavaOrigin::Path, "unknown"),
            candidate(Some(17), JavaOrigin::Installed, "known"),
        ];
        assert_eq!(best_java(&unknown, &[]).unwrap().bin, "known");
        assert_eq!(best_java(&unknown, &[17]).unwrap().bin, "known");
        let only_unknown = vec![candidate(None, JavaOrigin::Path, "unknown")];
        assert_eq!(best_java(&only_unknown, &[21]).unwrap().bin, "unknown");
        // And no candidates is no answer, not a panic.
        assert!(best_java(&[], &[21]).is_none());
    }

    /// The Java tab is consulted before the machine is, which is the whole
    /// point of the tab: a user with a JDK 8 and a JDK 21 has already answered
    /// "which one", and a scan that ran first would answer for them.
    #[test]
    fn the_java_the_settings_name_answers_before_the_machine_is_scanned() {
        let (_dir, paths) = test_root();
        // Files rather than working runtimes: `pick_java` names a binary and the
        // probe happens in its caller, so this test does not depend on what
        // this machine happens to have installed.
        let exe = if cfg!(windows) { "java.exe" } else { "java" };
        let named = paths.root.join("jdk21").join("bin").join(exe);
        let fallback = paths.root.join("jdk").join("bin").join(exe);
        for path in [&named, &fallback] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"").unwrap();
        }
        let named = named.to_string_lossy().into_owned();
        let fallback = fallback.to_string_lossy().into_owned();

        let java = JavaPrefs {
            default_path: fallback.clone(),
            by_major: [(21, named.clone())].into_iter().collect(),
        };

        // The version wants 21, and 21 is what was named for it.
        let mut lines = Vec::new();
        let found = pick_java(
            &paths,
            &java,
            &[21],
            "java-runtime-delta",
            &MapFetcher::new(),
            &mut |line| lines.push(line),
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(found, named);
        assert!(lines.iter().any(|line| line.contains("from the Java settings")), "{lines:?}");

        // A version whose majors nothing was named for falls to the default
        // rather than to the scan.
        let mut lines = Vec::new();
        let found = pick_java(
            &paths,
            &java,
            &[17],
            "java-runtime-gamma",
            &MapFetcher::new(),
            &mut |line| lines.push(line),
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(found, fallback);
        assert!(
            lines.iter().any(|line| line.contains("the default Java from the settings")),
            "{lines:?}"
        );

        // A named path that is gone is said out loud and looked past rather
        // than used: the runtime was uninstalled, and the user did not ask for
        // the launcher to stop working because of it.
        let mut stale = java.clone();
        let gone = paths.root.join("gone").join(exe).to_string_lossy().into_owned();
        stale.by_major.insert(21, gone.clone());
        stale.default_path = paths.root.join("gone-too").join(exe).to_string_lossy().into_owned();
        let mut lines = Vec::new();
        let found = pick_java(
            &paths,
            &stale,
            &[21],
            "java-runtime-delta",
            &MapFetcher::new(),
            &mut |line| lines.push(line),
            &mut |_| {},
        );
        let text = lines.join("\n");
        assert!(text.contains("not there"), "{text}");
        assert_ne!(found.as_deref(), Some(gone.as_str()), "a path that is gone is not a Java");
    }

    #[test]
    fn executable_lookup_searches_dirs_and_rejects_paths() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("java"), b"x").unwrap();
        let dirs = vec![dir.path().join("missing"), dir.path().to_path_buf()];
        assert_eq!(find_executable_in_dirs("java", &dirs), Some(dir.path().join("java")));
        assert_eq!(find_executable_in_dirs("nope", &dirs), None);
        assert_eq!(find_executable_in_dirs("", &dirs), None);
        assert_eq!(find_executable_in_dirs("a/b", &dirs), None);
        // Directories do not count.
        let sub = dir.path().join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        assert_eq!(find_executable_in_dirs("sub", &[dir.path().to_path_buf()]), None);
    }

    #[test]
    fn classpath_helpers_select_last_jar_and_join() {
        let jars = vec!["a.jar".to_string(), "main.jar".to_string()];
        assert_eq!(select_main_jar(&jars).as_deref(), Some("main.jar"));
        assert_eq!(select_main_jar(&[]), None);
        let joined = join_classpath(&jars);
        if cfg!(windows) {
            assert_eq!(joined, "a.jar;main.jar");
        } else {
            assert_eq!(joined, "a.jar:main.jar");
        }
        let root = Path::new("/data");
        assert_eq!(resolve_library_path(root, "libraries/x/y.jar"), PathBuf::from("/data/libraries/x/y.jar"));
    }

    #[test]
    fn candidate_names_match_platform() {
        let names = candidate_java_names();
        if cfg!(windows) {
            assert!(names.contains(&"javaw".to_string()));
        } else {
            assert_eq!(names, vec!["java".to_string()]);
        }
    }

    // ---- accounts ---------------------------------------------------------

    #[test]
    fn the_anonymous_account_is_stable_and_no_longer_random() {
        let first = AccountRef::anonymous();
        let second = AccountRef::anonymous();
        assert_eq!(first.uuid, second.uuid, "the same player every time");
        assert_eq!(first.uuid, crate::accounts::offline_uuid("Player"));
        assert_eq!(first.username, "Player");
        assert!(!first.kind.is_online());
    }

    #[test]
    fn an_account_entry_snapshots_into_a_launch_reference() {
        let entry = crate::accounts::AccountEntry::microsoft(
            "Steve",
            "AABB",
            "token",
            "refresh",
            1234,
            Some(true),
        );
        let reference = AccountRef::from_entry(&entry);
        assert_eq!(reference.username, "Steve");
        // Normalized on the way in: Mojang's profile ids are lowercase hex, and
        // this uuid is passed to the game as `uuid`, so a stored "AABB" must not
        // reach a server as one.
        assert_eq!(reference.uuid, "aabb");
        assert_eq!(reference.access_token.as_deref(), Some("token"));
        assert_eq!(reference.refresh_token.as_deref(), Some("refresh"));
        assert_eq!(reference.expires_at_ms, Some(1234));
        assert!(reference.kind.is_online());
    }

    #[test]
    fn an_offline_account_never_needs_a_refresh() {
        let account = AccountRef::offline("Steve", "abc");
        assert!(!account.needs_refresh(0));
        let mut log = Vec::new();
        let prepared =
            prepare_auth(&account, &MicrosoftAuth::with_public_client_id(), &mut |line| log.push(line))
                .unwrap();
        assert_eq!(prepared.session.user_type, "legacy");
        assert_eq!(prepared.session.access_token, "0");
        assert_eq!(prepared.session.session, "token:0:abc");
        assert!(prepared.refreshed.is_none());
        assert!(log.iter().any(|line| line.contains("offline account")));
    }

    #[test]
    fn a_fresh_microsoft_token_is_used_without_touching_the_network() {
        let now = palantir_core::util::now_millis();
        let account = AccountRef {
            username: "Steve".into(),
            uuid: "aabb".into(),
            kind: AccountKind::Msa,
            access_token: Some("stored-token".into()),
            refresh_token: Some("refresh".into()),
            expires_at_ms: Some(now + 24 * 60 * 60 * 1000),
        };
        assert!(!account.needs_refresh(now));
        // A transport with nothing in it: any request would fail, so this test
        // only passes if no request is made.
        let auth = MicrosoftAuth::with_transport(
            MicrosoftOAuth::public_client_id(),
            Box::new(MapTransport::new()),
        );
        let mut log = Vec::new();
        let prepared = prepare_auth(&account, &auth, &mut |line| log.push(line)).unwrap();
        assert_eq!(prepared.session.user_type, "msa");
        assert_eq!(prepared.session.access_token, "stored-token");
        assert_eq!(prepared.session.session, "token:stored-token:aabb");
        assert!(prepared.refreshed.is_none(), "nothing was renewed");
        assert!(log.iter().any(|line| line.contains("stored Microsoft session")));
    }

    #[test]
    fn a_stale_microsoft_token_is_renewed_and_the_new_tokens_come_back() {
        let now = palantir_core::util::now_millis();
        let account = AccountRef {
            username: "Steve".into(),
            uuid: "aabb".into(),
            kind: AccountKind::Msa,
            access_token: Some("old-token".into()),
            refresh_token: Some("refresh-token".into()),
            // Inside the twelve-hour window.
            expires_at_ms: Some(now + 60_000),
        };
        assert!(account.needs_refresh(now));

        let mut transport = MapTransport::new();
        transport.insert_form(
            palantir_net::auth::MICROSOFT_TOKEN_URL,
            200,
            r#"{"access_token":"msa-2","refresh_token":"refresh-2","expires_in":3600}"#,
        );
        transport.insert_json(
            palantir_net::auth::XBOX_USER_AUTH_URL,
            200,
            r#"{"Token":"user-token","DisplayClaims":{"xui":[{"uhs":"uhs-1"}]}}"#,
        );
        transport.insert_json(
            palantir_net::auth::XBOX_XSTS_AUTH_URL,
            200,
            r#"{"Token":"xsts-token","DisplayClaims":{"xui":[{"uhs":"uhs-1"}]}}"#,
        );
        transport.insert_json(
            palantir_net::auth::MINECRAFT_LAUNCHER_LOGIN_URL,
            200,
            r#"{"access_token":"game-token-2","expires_in":86400}"#,
        );
        transport.insert_get(
            palantir_net::auth::MINECRAFT_PROFILE_URL,
            200,
            r#"{"id":"aabbccdd","name":"Steve"}"#,
        );
        let auth = MicrosoftAuth::with_transport(
            MicrosoftOAuth::public_client_id(),
            Box::new(transport),
        );
        let mut log = Vec::new();
        let prepared = prepare_auth(&account, &auth, &mut |line| log.push(line)).unwrap();
        assert_eq!(prepared.session.access_token, "game-token-2");
        assert_eq!(prepared.session.user_type, "msa");
        let refreshed = prepared.refreshed.expect("renewed tokens are reported");
        assert_eq!(refreshed.access_token, "game-token-2");
        assert_eq!(refreshed.refresh_token.as_deref(), Some("refresh-2"));
        assert_eq!(refreshed.name, "Steve");
        assert_eq!(refreshed.uuid, "aabbccdd");
        assert!(refreshed.expires_at_ms > now);
        assert!(log.iter().any(|line| line.contains("renewing the Microsoft session")));
    }

    #[test]
    fn a_microsoft_account_with_no_refresh_token_blocks_rather_than_pretending() {
        let account = AccountRef {
            username: "Steve".into(),
            uuid: "aabb".into(),
            kind: AccountKind::Msa,
            access_token: None,
            refresh_token: None,
            expires_at_ms: None,
        };
        let auth = MicrosoftAuth::with_transport(
            MicrosoftOAuth::public_client_id(),
            Box::new(MapTransport::new()),
        );
        let error = prepare_auth(&account, &auth, &mut |_| {}).unwrap_err();
        assert!(error.contains("sign in to Microsoft again"), "got {error}");
    }

    #[test]
    fn a_refresh_that_fails_reports_why() {
        let now = palantir_core::util::now_millis();
        let account = AccountRef {
            username: "Steve".into(),
            uuid: "aabb".into(),
            kind: AccountKind::Msa,
            access_token: None,
            refresh_token: Some("dead".into()),
            expires_at_ms: Some(now - 1),
        };
        let mut transport = MapTransport::new();
        transport.insert_form(
            palantir_net::auth::MICROSOFT_TOKEN_URL,
            400,
            r#"{"error":"invalid_grant","error_description":"The refresh token has expired."}"#,
        );
        let auth = MicrosoftAuth::with_transport(
            MicrosoftOAuth::public_client_id(),
            Box::new(transport),
        );
        let error = prepare_auth(&account, &auth, &mut |_| {}).unwrap_err();
        assert!(error.contains("refresh token has expired"), "got {error}");
    }

    // ---- launch preparation ----------------------------------------------

    fn test_root() -> (tempfile::TempDir, PalantirPaths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = PalantirPaths::at(dir.path());
        std::fs::create_dir_all(paths.instances_dir()).unwrap();
        std::fs::create_dir_all(paths.meta_dir()).unwrap();
        (dir, paths)
    }

    fn session() -> launch::AuthSession {
        OfflineSession::new("Steve", "0".repeat(32)).into_auth_session()
    }

    #[test]
    fn prepare_launch_reports_missing_instance() {
        let (_dir, paths) = test_root();
        let mut store = OfflineMetaStore::new(paths.meta_dir());
        let fetcher = MapFetcher::new();
        let mut lines = Vec::new();
        let readiness = prepare_launch(
            &paths,
            "nope",
            &session(),
            &LaunchDefaults::default(),
            &mut store,
            &fetcher,
            &mut |line| lines.push(line),
            &mut |_| {},
        );
        assert!(matches!(readiness, LaunchReadiness::Blocked));
        assert!(lines.iter().any(|l| l.contains("cannot open instance")));
    }

    #[test]
    fn a_cold_cache_blocks_with_the_resolution_error_named() {
        // An instance whose version metadata is not cached and cannot be
        // fetched (an empty fetcher): the honest outcome is a blocked launch
        // that says which component failed, not a spawn of a game that is not
        // there.
        let (_dir, paths) = test_root();
        let instance = Instance::create(&paths.instances_dir(), "Cold", "1.21.1").unwrap();
        let mut store = OfflineMetaStore::new(paths.meta_dir());
        let fetcher = MapFetcher::new();
        let mut lines = Vec::new();
        let readiness = prepare_launch(
            &paths,
            &instance.id(),
            &session(),
            &LaunchDefaults::default(),
            &mut store,
            &fetcher,
            &mut |line| lines.push(line),
            &mut |_| {},
        );
        assert!(matches!(readiness, LaunchReadiness::Blocked));
        let text = lines.join("\n");
        assert!(text.contains("cannot resolve component 'net.minecraft'"), "got: {text}");
        assert!(text.contains("resolution failed with errors"), "got: {text}");
    }

    #[test]
    fn a_fully_installed_instance_produces_a_runnable_plan() {
        // A minimal but complete instance: metadata cached, every file present,
        // and a fake `java` on PATH... which cannot be faked, so the java probe
        // is what this asserts *around*: the resolver and installer succeed and
        // the launch only stops at the missing runtime.
        let (_dir, paths) = test_root();
        let instance = Instance::create(&paths.instances_dir(), "Ready", "1.21.1").unwrap();
        seed_meta(&paths, &instance, "1.21.1", &[]);
        // `test.lib:lib:1.0` stores as `test/lib/lib/1.0/lib-1.0.jar` — the group
        // contributes `test/lib` and the artifact name is `lib`, so `lib` appears
        // twice. Seeding the single-`lib` path left the planner one file short and
        // this test was measuring that, not the launch plan.
        seed_library(
            paths
                .root
                .join("libraries")
                .join("test/lib/lib/1.0/lib-1.0.jar"),
        );
        seed_library(paths.root.join("libraries").join("com/mojang/minecraft/1.21.1/minecraft-1.21.1-client.jar"));

        let mut store = OfflineMetaStore::new(paths.meta_dir());
        let fetcher = MapFetcher::new();
        let mut lines = Vec::new();
        // Levels as well as lines: this is the only path through
        // `prepare_launch` that reaches the install phase with nothing missing,
        // so it is the one that can show the two taps arriving separately — a
        // phase that says nothing and a phase that draws nothing are both
        // silent, and only one of them is a bug.
        let mut levels: Vec<install::Progress> = Vec::new();
        let readiness = prepare_launch(
            &paths,
            &instance.id(),
            &session(),
            &LaunchDefaults::default(),
            &mut store,
            &fetcher,
            &mut |line| lines.push(line),
            &mut |level| levels.push(level),
        );
        let text = lines.join("\n");
        // Everything about the install phase must be clean…
        assert!(text.contains("install:"), "got: {text}");
        assert!(!text.contains("install failed"), "got: {text}");
        assert!(!text.contains("files are missing"), "got: {text}");
        // …and nothing missing means no phase fetched anything, so no level was
        // reported. A bar drawn for a phase with no work would be a claim that
        // something is happening, which is the opposite of what this phase did.
        assert!(
            levels.is_empty(),
            "a fully installed instance has nothing to report: {levels:?}"
        );
        // …and the only acceptable blocker is the runtime that is not
        // installed on every machine, or a real plan if one is.
        match readiness {
            LaunchReadiness::Blocked => {
                assert!(text.contains("java"), "blocked for a reason that is not java: {text}");
            }
            LaunchReadiness::Ready(plan) => {
                assert!(plan.main_jar.is_file());
                assert!(
                    plan.argv.iter().any(|arg| arg.starts_with("-Djava.library.path=")),
                    "the natives directory must be on the library path"
                );
                assert!(plan.argv.iter().any(|arg| arg == "-cp"));
                assert!(plan.argv.iter().any(|arg| arg == "com.example.Main"));
            }
        }
    }

    /// The Create dialog writes the settings' default Java into a new instance.
    /// That pin must not strand an instance whose version needs another major:
    /// the search carries on from the settings, which is what the Java tab is
    /// for.
    #[test]
    fn an_instance_pinned_to_the_wrong_java_keeps_looking() {
        let (_dir, paths) = test_root();
        let instance = Instance::create(&paths.instances_dir(), "Old", "1.12.2").unwrap();
        // 1.12.2 runs on Java 8, and the instance is about to be pinned to 21 —
        // the shape a machine whose default Java is 21 leaves behind.
        seed_meta(&paths, &instance, "1.12.2", &[8]);
        seed_library(paths.root.join("libraries").join("test/lib/lib/1.0/lib-1.0.jar"));
        seed_library(
            paths
                .root
                .join("libraries")
                .join("com/mojang/minecraft/1.12.2/minecraft-1.12.2-client.jar"),
        );

        let jdks = tempfile::tempdir().unwrap();
        let pinned = fake_java_home(jdks.path(), "jdk21", "21.0.6");
        let wanted = fake_java_home(jdks.path(), "jdk8", "1.8.0_402");
        let mut instance = Instance::open(&paths.instances_dir().join(instance.id())).unwrap();
        instance.settings_mut().set_bool("OverrideJavaLocation", true);
        instance.settings_mut().set_str("JavaPath", &pinned);
        instance.save().unwrap();

        let defaults = LaunchDefaults {
            java: JavaPrefs { default_path: wanted.clone(), ..JavaPrefs::default() },
            ..LaunchDefaults::default()
        };

        let mut store = OfflineMetaStore::new(paths.meta_dir());
        let fetcher = MapFetcher::new();
        let mut lines = Vec::new();
        let readiness = prepare_launch(
            &paths,
            &instance.id(),
            &session(),
            &defaults,
            &mut store,
            &fetcher,
            &mut |line| lines.push(line),
            &mut |_| {},
        );
        let text = lines.join("\n");
        assert!(
            text.contains("is Java 21, but this version wants Java 8"),
            "the wrong-major pin has to be named and looked past: {text}"
        );
        assert!(
            text.contains("using the default Java from the settings"),
            "the settings' Java is the next answer: {text}"
        );
        // Both files are placeholders rather than JVMs, so the launch stops at
        // the probe — and that is the only thing it stops at: naming a Java the
        // version cannot run on cost one log line, not a launch.
        assert!(matches!(readiness, LaunchReadiness::Blocked));
        assert!(
            text.contains(&format!("java probe failed for '{wanted}'")),
            "the settings' Java is the one that was tried: {text}"
        );
    }

    /// Write a version file into the metadata cache for `uid`/`version`.
    fn write_meta(paths: &PalantirPaths, uid: &str, version: &str, value: serde_json::Value) {
        let dir = paths.meta_dir().join(uid);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(format!("{version}.json")), value.to_string()).unwrap();
    }

    fn seed_library(path: PathBuf) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"jar").unwrap();
    }

    /// Cache the two version files a minimal 1.21.1 instance resolves: the game
    /// itself and one library, plus the client jar descriptor.
    ///
    /// `majors` is what the version declares it will run on
    /// (`compatibleJavaMajors`), which is the field a launcher picks a Java
    /// from; an empty slice means the version does not say.
    fn seed_meta(paths: &PalantirPaths, instance: &Instance, game: &str, majors: &[i64]) {
        write_meta(
            paths,
            "net.minecraft",
            game,
            serde_json::json!({
                "uid": "net.minecraft",
                "version": game,
                "order": 0,
                "mainClass": "com.example.Main",
                "compatibleJavaMajors": majors,
                "assets": "17",
                "assetIndex": {
                    "id": "17",
                    "sha1": "0000000000000000000000000000000000000000",
                    "size": 2,
                    "totalSize": 0,
                    "url": "https://example.invalid/17.json"
                },
                "libraries": [
                    {
                        "name": "test.lib:lib:1.0",
                        "downloads": {
                            "artifact": {
                                "path": "test/lib/lib/1.0/lib-1.0.jar",
                                "sha1": "",
                                "size": 3,
                                "url": "https://example.invalid/lib-1.0.jar"
                            }
                        }
                    }
                ],
                "downloads": {
                    "client": {
                        "path": format!("com/mojang/minecraft/{game}/minecraft-{game}-client.jar"),
                        "sha1": "",
                        "size": 3,
                        "url": "https://example.invalid/client.jar"
                    }
                }
            }),
        );
        // The index the profile names has to be on disk, or the install phase
        // legitimately tries to download it and this test would be measuring a
        // network failure instead of the launch plan.
        let indexes = paths.assets_dir().join("indexes");
        std::fs::create_dir_all(&indexes).unwrap();
        std::fs::write(indexes.join("17.json"), br#"{"objects":{}}"#).unwrap();

        // The pack must reference it, exactly as `Instance::create` writes it.
        let mut profile = PackProfile::default();
        profile.append(Component {
            uid: "net.minecraft".into(),
            version: game.into(),
            important: true,
            ..Default::default()
        });
        profile.save(&instance.mmc_pack_path()).unwrap();
    }

    #[test]
    fn playtime_helpers_write_the_instance_metadata() {
        let (_dir, paths) = test_root();
        let instance = Instance::create(&paths.instances_dir(), "Timed", "1.21.1").unwrap();
        let mut lines = Vec::new();
        record_launch_start(&paths, &instance.id(), &mut |line| lines.push(line));
        record_play_time(
            &paths,
            &instance.id(),
            Duration::from_secs(125),
            &mut |line| lines.push(line),
        );
        let back = Instance::open(&paths.instances_dir().join(instance.id())).unwrap();
        assert!(back.last_launch_millis() > 0, "LastLaunchTime is stamped");
        assert_eq!(back.total_time_played_secs(), 125);
        assert!(lines.iter().any(|line| line.contains("played for 125s")));

        // A zero-length session adds nothing but is not an error.
        record_play_time(&paths, &instance.id(), Duration::from_millis(10), &mut |_| {});
        assert_eq!(
            Instance::open(&paths.instances_dir().join(instance.id()))
                .unwrap()
                .total_time_played_secs(),
            125
        );
        // A missing instance is reported, not fatal.
        let mut lines = Vec::new();
        record_launch_start(&paths, "gone", &mut |line| lines.push(line));
        record_play_time(&paths, "gone", Duration::from_secs(3), &mut |line| lines.push(line));
        assert!(lines.iter().any(|line| line.contains("could not")));
    }

    // ---- the run log -------------------------------------------------------

    #[test]
    fn a_launch_line_loses_its_tokens_but_keeps_its_shape() {
        // The shapes that actually occur: the command line, the same thing written
        // into the launch script, and the game echoing its own arguments in a
        // crash report.
        assert_eq!(
            redact(
                "command: java -cp a.jar net.minecraft.client.main.Main --accessToken \
                 eyJhbG.abc.def --uuid aabb"
            ),
            "command: java -cp a.jar net.minecraft.client.main.Main --accessToken <redacted> \
             --uuid aabb"
        );
        assert_eq!(
            redact("\"$JAVA\" --accessToken=eyJhbG --version 21"),
            "\"$JAVA\" --accessToken=<redacted> --version 21"
        );
        // Several secrets on one line, with the whitespace left as it was found.
        assert_eq!(
            redact("--clientId\t1234   --xuid deadbeef --username Steve"),
            "--clientId\t<redacted>   --xuid <redacted> --username Steve"
        );
        // A flag with nothing after it names a secret without carrying one, and a
        // line without secrets comes back byte for byte.
        assert_eq!(
            redact("expected --accessToken, got nothing"),
            "expected --accessToken, got nothing"
        );
        let plain = "install: 5057 asset object(s) to fetch";
        assert_eq!(redact(plain), plain);
    }

    #[test]
    fn the_run_log_lives_with_the_instance_and_rotates_once() {
        let (_dir, paths) = test_root();
        let instance = Instance::create(&paths.instances_dir(), "Logged", "1.21.1").unwrap();

        let first = LaunchLogFile::open(&paths, &instance.id());
        assert!(first.path().ends_with(Path::new("logs").join("launcher.log")));
        first.append(&["first run".to_string()]);
        let text = std::fs::read_to_string(first.path()).unwrap();
        assert!(text.contains("Logged"), "the header names the instance: {text}");
        assert!(text.contains("first run"), "{text}");

        // A second launch moves the first aside instead of appending to it:
        // retrying is exactly when the log of the failed attempt matters.
        let second = LaunchLogFile::open(&paths, &instance.id());
        second.append(&["second run".to_string()]);
        let text = std::fs::read_to_string(second.path()).unwrap();
        assert!(text.contains("second run"), "{text}");
        assert!(!text.contains("first run"), "{text}");
        let previous = std::fs::read_to_string(second.path().with_extension("log.1")).unwrap();
        assert!(previous.contains("first run"), "{previous}");
    }

    /// Every line the console shows — the launcher's own account and the game's
    /// output alike — goes out redacted and lands in the instance's log.
    #[test]
    fn nothing_reaches_the_console_without_passing_the_redactor_and_the_log() {
        let (_dir, paths) = test_root();
        let instance = Instance::create(&paths.instances_dir(), "Streamed", "1.21.1").unwrap();
        let journal = LaunchLogFile::open(&paths, &instance.id());

        let (mut tx, mut rx) = futures::channel::mpsc::channel::<Message>(4);
        assert!(send_batch(
            &mut tx,
            7,
            vec![
                "command: java --accessToken eyJhbG --clientId 1234".to_string(),
                "Starting Minecraft 1.21.1".to_string(),
            ],
            &journal,
        ));
        let Ok(Message::LaunchLog { run_id, lines }) = rx.try_recv() else {
            panic!("expected a log batch");
        };
        assert_eq!(run_id, 7);
        assert_eq!(
            lines,
            vec![
                "command: java --accessToken <redacted> --clientId <redacted>",
                "Starting Minecraft 1.21.1"
            ]
        );

        let text = std::fs::read_to_string(journal.path()).unwrap();
        assert!(!text.contains("eyJhbG"), "the token reached the disk: {text}");
        assert!(text.contains("--accessToken <redacted>"), "{text}");
    }

    /// A progress report is a *level*, not an event: when the window is a frame
    /// behind, the newest report is the only one worth having, so a full channel
    /// drops it instead of retrying the way [`send_batch`] does. That matters
    /// because the alternative is a downloader waiting on a window — and because
    /// a level must never reach the run log as a line.
    ///
    /// The exact buffer accounting is `futures`' own (a sender buffers up to its
    /// capacity), so what is asserted is the outcome rather than the boundary:
    /// sixteen reports down a channel of one all *return*, the ones that arrive
    /// arrive in order, and fewer arrive than were sent.
    #[test]
    fn a_progress_report_is_dropped_rather_than_waited_for() {
        let (_dir, paths) = test_root();
        let instance = Instance::create(&paths.instances_dir(), "Busy", "1.21.1").unwrap();
        let journal = LaunchLogFile::open(&paths, &instance.id());
        let (mut tx, mut rx) = futures::channel::mpsc::channel::<Message>(1);

        const SENT: usize = 16;
        for done in 1..=SENT {
            send_progress(
                &mut tx,
                7,
                install::Progress::new("asset objects", done, 5057, done as u64 * 1024),
            );
        }

        let mut arrived: Vec<usize> = Vec::new();
        while let Ok(message) = rx.try_recv() {
            match message {
                Message::LaunchProgress { run_id, progress } => {
                    assert_eq!(run_id, 7);
                    arrived.push(progress.done);
                }
                other => panic!("expected a progress report, got {other:?}"),
            }
        }
        assert!(!arrived.is_empty(), "the first report has to be delivered");
        assert!(
            arrived.windows(2).all(|pair| pair[0] < pair[1]),
            "the reports that arrived are the ones that were sent, in order: {arrived:?}"
        );
        assert!(
            arrived.len() < SENT,
            "every report travelling a channel of one would mean nothing was \
             dropped, so the downloader would be waiting on the window: {arrived:?}"
        );
        let logged = std::fs::read_to_string(journal.path()).unwrap_or_default();
        assert!(
            !logged.contains("asset objects"),
            "a level is not a line in the run log: {logged}"
        );
    }

    #[test]
    fn open_url_reports_a_browser_it_cannot_launch() {
        // A URL is only opened through the platform opener, which on every
        // supported system is an absolute command; a nonsense URL still has to
        // produce success from the spawn itself, so what is asserted here is
        // that the call does not panic and reports a real error shape when it
        // fails.
        let result = open_url("not-a-url");
        if let Err(message) = result {
            assert!(message.contains("opening"), "got {message}");
        }
    }
}
