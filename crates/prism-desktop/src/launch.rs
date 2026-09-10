//! Offline launch preparation and the background launch worker.
//!
//! The worker mirrors the `prism-cli launch-script` flow: resolve the
//! instance profile through [`OfflineMetaStore`] at `paths.meta_dir()`,
//! build the launch script with [`prism_core::launch`], probe java (instance
//! `JavaPath` else a `PATH` lookup of `javaw`/`java`) and — only when java
//! was found **and** the main jar exists on disk — spawn the child with
//! piped stdout/stderr. Anything missing produces an honest console message
//! and no process is started (never a faked success).
//!
//! Log lines travel back to the GUI through the `iced::subscription::channel`
//! sender owned by the worker thread (`Vec<String>` batches); the final
//! outcome arrives as a done message. See `app.rs` for the subscription.

use futures::channel::mpsc::Sender;
use prism_core::{
    instance::Instance,
    java::JavaVersion,
    launch,
    pack::PackProfile,
    paths::{PrismPaths, System},
    resolve::{resolve, OfflineMetaStore},
    settings::{defaults, Settings},
    version::{ProblemSeverity, RuntimeContext},
};
use prism_gui::SettingsModel;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use crate::app::Message;

/// Shared handle for the running game process (for the Kill button).
pub type ChildSlot = Arc<Mutex<Option<Child>>>;

/// Offline account identity used for the launch session.
#[derive(Debug, Clone)]
pub struct AccountRef {
    /// Player name.
    pub username: String,
    /// Account uuid (32 hex digits).
    pub uuid: String,
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
}

/// A fully planned, runnable launch.
#[derive(Debug, Clone)]
pub struct LaunchPlan {
    /// Java binary that was probed successfully.
    pub java_bin: String,
    /// Full java argument vector (JVM args, `-cp`, main class, game args).
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

/// Resolve a library path from [`prism_core::version::Library`]
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
pub fn has_legacy_lwjgl(profile: &prism_core::version::LaunchProfile) -> bool {
    profile.libraries.iter().any(|lib| lib.name.group() == "org.lwjgl.lwjgl")
}

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

// ---- launch preparation ---------------------------------------------------

/// Resolve + plan a launch without starting anything.
///
/// Returns console lines narrating every step plus either a runnable
/// [`LaunchPlan`] or [`LaunchReadiness::Blocked`]. Used by the background
/// worker and (synchronously, as a dry run) by the `Sandbox` entrypoint.
pub fn prepare_launch(
    paths: &PrismPaths,
    instance_id: &str,
    account: &AccountRef,
) -> (Vec<String>, LaunchReadiness) {
    let mut log: Vec<String> = Vec::new();
    let instance = match Instance::open(&paths.instances_dir().join(instance_id)) {
        Ok(instance) => instance,
        Err(e) => {
            log.push(format!("cannot open instance '{instance_id}': {e}"));
            return (log, LaunchReadiness::Blocked);
        }
    };
    let name = instance.name();
    let id = instance.id();
    log.push(format!("preparing launch of '{name}' ({id})"));

    let global = match Settings::load(&paths.global_config()) {
        Ok(settings) => settings,
        Err(_) => {
            log.push("no global prismlauncher.cfg found, using defaults".to_string());
            Settings::empty(paths.global_config())
        }
    };
    let model = SettingsModel::with_instance(global, instance.settings().clone());

    let profile = match PackProfile::load(&instance.mmc_pack_path()) {
        Ok(profile) => profile,
        Err(_) => {
            log.push("no mmc-pack.json found, using an empty component list".to_string());
            PackProfile::default()
        }
    };

    // Honest per-component cache check before resolving, so a cold cache
    // names the missing files instead of failing opaquely.
    let ctx = RuntimeContext::current_host();
    let mut missing_cache: Vec<(String, String)> = Vec::new();
    for comp in profile.components() {
        if !comp.is_enabled() {
            continue;
        }
        let version = if comp.version.is_empty() {
            comp.cached_version.clone()
        } else {
            comp.version.clone()
        };
        if instance.patches_dir().join(format!("{}.json", comp.uid)).is_file() {
            continue;
        }
        if !paths.meta_dir().join(&comp.uid).join(format!("{version}.json")).is_file() {
            missing_cache.push((comp.uid.clone(), version));
        }
    }
    for (uid, version) in &missing_cache {
        log.push(format!("missing meta cache for {uid} {version} — run resolve-online first"));
    }

    let mut store = OfflineMetaStore::new(paths.meta_dir());
    let resolution = match resolve(&profile, &instance.patches_dir(), &mut store, &ctx) {
        Ok(resolution) => resolution,
        Err(e) => {
            log.push(format!("resolution error: {e}"));
            return (log, LaunchReadiness::Blocked);
        }
    };
    for problem in &resolution.problems {
        log.push(format!("resolve {:?}: {}", problem.severity, problem.message));
    }
    for component in &resolution.components {
        for problem in &component.problems {
            log.push(format!("resolve {} {:?}: {}", component.uid, problem.severity, problem.message));
        }
    }
    if resolution.severity() == ProblemSeverity::Error {
        log.push("resolution failed with errors — not launching".to_string());
        return (log, LaunchReadiness::Blocked);
    }
    if resolution.profile.main_class.is_empty() {
        log.push("no main class resolved — metadata is incomplete, not launching".to_string());
        return (log, LaunchReadiness::Blocked);
    }
    log.push(format!(
        "resolved {} component(s), main class {}",
        resolution.components.len(),
        resolution.profile.main_class
    ));

    // Offline auth session (mirrors the CLI's launch-script flow).
    let session = launch::AuthSession {
        player_name: account.username.clone(),
        uuid: account.uuid.clone(),
        access_token: "0".to_string(),
        session: "-".to_string(),
        user_type: "legacy".to_string(),
        user_properties: "{}".to_string(),
        demo: false,
    };
    let target = join_target(instance.settings(), &mut log);

    let window = launch::WindowParams {
        width: model.get_i64("MinecraftWinWidth", Some("OverrideWindow"), defaults::MC_WIN_WIDTH),
        height: model.get_i64("MinecraftWinHeight", Some("OverrideWindow"), defaults::MC_WIN_HEIGHT),
        maximized: model.get_bool("LaunchMaximized", Some("OverrideWindow"), false),
    };
    let game_root = instance.game_root();
    let mut vars = launch::profile_var_map(
        &resolution.profile,
        &name,
        &id,
        instance.root(),
        &game_root,
        &game_root.join("resources"),
        &instance.root().join("assets"),
        &instance.root().join("libraries"),
    );
    vars.insert("version_name".to_string(), resolution.profile.minecraft_version.clone());
    let mc_args = launch::process_minecraft_args(&resolution.profile, Some(&session), target.as_ref(), &vars);
    let script = launch::create_launch_script(
        &resolution.profile,
        Some(&session),
        target.as_ref(),
        &mc_args,
        window,
        "Prism Launcher",
        env!("CARGO_PKG_VERSION"),
        &name,
    );
    log.push(format!("launch script ({} lines):", script.lines().count()));
    for line in script.lines() {
        log.push(line.to_string());
    }

    // Java: configured path first, else PATH lookup.
    let configured = model.get_str("JavaPath", Some("OverrideJavaLocation"), "");
    let java_bin = if configured.trim().is_empty() {
        log.push("no JavaPath configured, searching PATH".to_string());
        match find_java_on_path() {
            Some(found) => found,
            None => {
                log.push(
                    "java not found: set JavaPath in the instance/global settings or install a Java runtime (looked for javaw/java on PATH)".to_string(),
                );
                return (log, LaunchReadiness::Blocked);
            }
        }
    } else {
        let trimmed = configured.trim().to_string();
        if !Path::new(&trimmed).is_file() {
            log.push(format!("configured JavaPath '{trimmed}' does not exist — not launching"));
            return (log, LaunchReadiness::Blocked);
        }
        trimmed
    };
    match probe_java(&java_bin) {
        Ok(detail) => log.push(format!("using java '{java_bin}' ({detail})")),
        Err(e) => {
            log.push(e);
            return (log, LaunchReadiness::Blocked);
        }
    }

    // Libraries: the main jar (last classpath entry) must exist.
    let files = resolution.profile.get_library_files(
        &ctx,
        Some(&instance.local_libraries_dir()),
        &instance.natives_dir(),
        false,
    );
    let main_rel = match select_main_jar(&files.jar) {
        Some(rel) => rel,
        None => {
            log.push("no libraries resolved — not launching".to_string());
            return (log, LaunchReadiness::Blocked);
        }
    };
    let main_jar = resolve_library_path(&paths.root, &main_rel);
    if !main_jar.is_file() {
        log.push(format!(
            "main jar not found at {} — libraries are not downloaded yet, not launching",
            main_jar.display()
        ));
        return (log, LaunchReadiness::Blocked);
    }
    let mut missing = 0usize;
    for rel in &files.jar {
        if !resolve_library_path(&paths.root, rel).is_file() {
            missing += 1;
        }
    }
    if missing > 0 {
        log.push(format!(
            "warning: {missing} of {} classpath jar(s) are missing (continuing anyway)",
            files.jar.len()
        ));
    }

    let (min_mem, max_mem) = model.effective_memory();
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
    argv.push("-cp".to_string());
    argv.push(join_classpath(&full_jars));
    argv.push(resolution.profile.main_class.clone());
    argv.extend(mc_args);

    if std::fs::create_dir_all(&game_root).is_err() {
        log.push(format!("cannot create game directory {} — not launching", game_root.display()));
        return (log, LaunchReadiness::Blocked);
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
    log.push(format!("command: {java_bin} {}", argv.join(" ")));
    let plan = LaunchPlan { java_bin, argv, cwd: game_root, main_jar, envs };
    (log, LaunchReadiness::Ready(plan))
}

/// Build the join-server target from instance settings, logging problems.
fn join_target(settings: &Settings, log: &mut Vec<String>) -> Option<launch::LaunchTarget> {
    if !settings.get_bool("JoinServerOnLaunch", false) {
        return None;
    }
    let raw = settings.get_str("JoinServerOnLaunchAddress", "");
    match parse_server_address(&raw) {
        Some((address, port)) => {
            log.push(format!("will join server {address}:{port}"));
            Some(launch::LaunchTarget { address, port, world: String::new() })
        }
        None => {
            log.push(format!("ignoring malformed join-server address '{raw}'"));
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

/// Probe a java binary with `java -version`; returns the first output line.
fn probe_java(java_bin: &str) -> Result<String, String> {
    match Command::new(java_bin).arg("-version").output() {
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
    profile: &prism_core::version::LaunchProfile,
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

// ---- worker ---------------------------------------------------------------

/// Run one launch in a background thread: prepare, optionally spawn the
/// child with piped output, stream batches + a final done message through
/// the subscription sender. Never reports success it did not observe.
pub fn run_launch_worker(params: LaunchParams, slot: ChildSlot, sender: Sender<Message>) {
    let paths = PrismPaths::at(&params.data_root);
    let run_id = params.run_id;
    let mut sender = sender;
    let (lines, readiness) = prepare_launch(&paths, &params.instance_id, &params.account);
    if !send_batch(&mut sender, run_id, lines) {
        return;
    }
    let plan = match readiness {
        LaunchReadiness::Blocked => {
            send_done(&mut sender, run_id, "launch blocked (see console)".to_string());
            return;
        }
        LaunchReadiness::Ready(plan) => plan,
    };
    send_batch(&mut sender, run_id, vec![format!("spawning '{}' (main jar {})", plan.java_bin, plan.main_jar.display())]);
    let mut command = Command::new(&plan.java_bin);
    command
        .args(&plan.argv)
        .current_dir(&plan.cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in &plan.envs {
        command.env(key, value);
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            send_batch(
                &mut sender,
                run_id,
                vec![format!("failed to start '{}': {e} — not launching", plan.java_bin)],
            );
            send_done(&mut sender, run_id, "failed to start".to_string());
            return;
        }
    };
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    match slot.lock() {
        Ok(mut guard) => {
            *guard = Some(child);
        }
        Err(_) => {
            send_batch(&mut sender, run_id, vec!["internal lock error: cannot track child".to_string()]);
            send_done(&mut sender, run_id, "internal lock error".to_string());
            let _ = child.kill();
            let _ = child.wait();
            return;
        }
    }
    send_batch(&mut sender, run_id, vec!["process started, streaming output…".to_string()]);

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
                    if !send_batch(&mut sender, run_id, batch) {
                        kill_slot(&slot);
                        return;
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if !buf.is_empty() {
                    let batch = std::mem::take(&mut buf);
                    if !send_batch(&mut sender, run_id, batch) {
                        kill_slot(&slot);
                        return;
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if !buf.is_empty() {
                    let batch = std::mem::take(&mut buf);
                    if !send_batch(&mut sender, run_id, batch) {
                        kill_slot(&slot);
                        return;
                    }
                }
                break;
            }
        }
    }
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
    send_batch(&mut sender, run_id, vec![outcome.clone()]);
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

/// Forward one batch; `false` means the GUI is gone and the worker should stop.
fn send_batch(sender: &mut Sender<Message>, run_id: u64, lines: Vec<String>) -> bool {
    if lines.is_empty() {
        return true;
    }
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
    fn prepare_launch_reports_cold_cache_honestly() {
        let dir = tempfile::tempdir().unwrap();
        let paths = PrismPaths::at(dir.path());
        std::fs::create_dir_all(paths.instances_dir()).unwrap();
        std::fs::create_dir_all(paths.meta_dir()).unwrap();
        let instance = Instance::create(&paths.instances_dir(), "Cold", "1.21.1").unwrap();
        let account = AccountRef { username: "Steve".to_string(), uuid: "0".repeat(32) };
        let (lines, readiness) = prepare_launch(&paths, &instance.id(), &account);
        assert!(matches!(readiness, LaunchReadiness::Blocked));
        let text = lines.join("\n");
        assert!(text.contains("missing meta cache for net.minecraft"), "got: {text}");
        assert!(text.contains("run resolve-online first"), "got: {text}");
    }

    #[test]
    fn prepare_launch_reports_missing_instance() {
        let dir = tempfile::tempdir().unwrap();
        let paths = PrismPaths::at(dir.path());
        std::fs::create_dir_all(paths.instances_dir()).unwrap();
        let account = AccountRef { username: "Steve".to_string(), uuid: "0".repeat(32) };
        let (lines, readiness) = prepare_launch(&paths, "nope", &account);
        assert!(matches!(readiness, LaunchReadiness::Blocked));
        assert!(lines.iter().any(|l| l.contains("cannot open instance")));
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
}
