//! Starting Minecraft, off the frame thread.
//!
//! The engine's launch path is a network resolve, a disk install and a process
//! wait. None of those can happen on egui's frame thread, so `start` hands the
//! instance to one worker thread and `poll` collects what that worker has said.
//! The shape is the one `search` and `images` already use here: a worker, an
//! `mpsc` channel, and a frame that only reads what is waiting.
//!
//! **The worker owns the paths.** Building `SiftPaths` here rather than in the
//! shell keeps every disk question on the thread that is already blocked on
//! disk. The root is the same `platform_data_root` that
//! [`crate::data::instances_dir`] uses, so the instance a card names is the
//! instance this launch opens.
//!
//! **The account is offline and fixed for now.** There is no account flow in
//! this shell yet, and an offline session is what lets a launch be tested end to
//! end: it needs no tokens and no sign-in, while it still exercises the resolve,
//! the install and the process spawn. It also keeps the player identity stable
//! (`Player`'s offline UUID), so a world made by one test launch is the same
//! player's world on the next.

use std::fmt;
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};

use sift_core::paths::{platform_data_root, SiftPaths, System};
use sift_game::install;
use sift_game::launch::{prepare_launch, LaunchDefaults, LaunchReadiness};
use sift_game::wire::Wire;
use sift_net::{OfflineSession, OnlineMetaStore, DEFAULT_META_BASE_URL};

/// What the title bar has to say about the one game this shell starts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Status {
    /// No launch has been asked for, or the shell has not started one yet.
    #[default]
    Idle,
    /// The worker is resolving, installing or probing Java.
    Preparing,
    /// A counted download phase is moving, with the engine's own totals.
    Downloading {
        /// Files finished so far.
        done: u32,
        /// Files this phase will fetch in total.
        total: u32,
    },
    /// The game process exists and is still running.
    Running,
    /// The game process ended with this exit code.
    Exited(i32),
    /// The launch could not start or could not be followed to an exit.
    Failed(String),
}

impl fmt::Display for Status {
    /// The exact string the title bar's running-instance slot shows.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Status::Idle => formatter.write_str("No instances running"),
            Status::Preparing => formatter.write_str("Preparing…"),
            Status::Downloading { done, total } => {
                write!(formatter, "Downloading {done}/{total}")
            }
            Status::Running => formatter.write_str("Running"),
            Status::Exited(code) => write!(formatter, "Exited ({code})"),
            Status::Failed(reason) => formatter.write_str(reason),
        }
    }
}

/// One worker's word, tagged with the launch it belongs to.
struct Reply {
    /// The `Game::launch` value that asked for this worker.
    launch: u64,
    status: Status,
}

/// The function one worker runs. A pointer rather than a trait object so a test
/// can substitute a worker without starting a network or a game.
type Worker = fn(String, u64, Option<(String, String, String)>, Option<String>, Sender<Reply>);

/// The launcher's single game process and the channel its worker answers on.
pub struct Game {
    status: Status,
    /// Incremented for every launch that is allowed to start. A reply carrying a
    /// different number belongs to a launch the reader has moved past.
    launch: u64,
    sender: Sender<Reply>,
    receiver: Receiver<Reply>,
    /// The real worker in the window; a probe in the tests.
    worker: Worker,
    /// The signed-in profile the next launch should start as, if there is one.
    account: Option<(String, String, String)>,
}

impl Default for Game {
    fn default() -> Game {
        Game::with_worker(run_launch)
    }
}

impl Game {
    /// Build a game holder around `worker`.
    fn with_worker(worker: Worker) -> Game {
        let (sender, receiver) = channel();
        Game {
            status: Status::Idle,
            launch: 0,
            account: None,
            sender,
            receiver,
            worker,
        }
    }

    /// Start `instance_id`, unless a launch is already preparing, downloading or
    /// running.
    ///
    /// The check is before the generation bump and before the spawn, so a second
    /// press while the first worker is out is a no-op rather than a second
    /// process. `Preparing` is set here, on the frame that asked, not when the
    /// worker's first message arrives: that is the frame the person who pressed
    /// Play sees.
    /// Point the next launch at a signed-in profile: its name, uuid and access token.
    ///
    /// `None` falls back to the offline player, which is what a reader who has not signed in
    /// gets and what every earlier round launched as.
    pub fn set_account(&mut self, account: Option<(String, String, String)>) {
        self.account = account;
    }

    pub fn start(&mut self, instance_id: &str) {
        self.start_with(instance_id, None);
    }

    /// Start `instance_id` with `world` opened, which the engine turns into
    /// `--quickPlaySingleplayer`.
    ///
    /// `// shortcut: a crash between the write and the clear leaves the world set; upgrade when
    /// the world is passed somewhere disposable instead.` The world travels through the
    /// instance's own settings because that is where the engine reads a join target from.
    pub fn start_world(&mut self, instance_id: &str, world: &str) {
        self.start_with(instance_id, Some(world.to_owned()));
    }

    fn start_with(&mut self, instance_id: &str, world: Option<String>) {
        if self.busy() {
            return;
        }
        self.launch = self.launch.wrapping_add(1);
        let launch = self.launch;
        self.status = Status::Preparing;

        let instance_id = instance_id.to_owned();
        let sender = self.sender.clone();
        let worker = self.worker;
        let account = self.account.clone();
        let spawned = std::thread::Builder::new()
            .name("sift-launch".to_owned())
            .spawn(move || worker(instance_id, launch, account, world, sender));
        if let Err(error) = spawned {
            self.status = Status::Failed(format!("could not start a launch worker: {error}"));
        }
    }

    /// Take every reply that is ready. Called once per frame.
    pub fn poll(&mut self) {
        while let Ok(reply) = self.receiver.try_recv() {
            // A reply for a launch the reader has moved past is dropped: showing
            // it would put an old state under the new launch, the same rule the
            // search module applies to a reply for an abandoned query.
            if reply.launch != self.launch {
                continue;
            }
            self.status = reply.status;
        }
    }

    /// The state the title bar draws.
    pub fn status(&self) -> &Status {
        &self.status
    }

    /// Whether a launch is still alive. Exactly preparing, downloading or
    /// running keeps the shell drawing; every terminal state owes no frame.
    pub fn busy(&self) -> bool {
        matches!(
            self.status,
            Status::Preparing | Status::Downloading { .. } | Status::Running
        )
    }
}

/// One worker: run the engine and always leave a terminal status behind.
fn run_launch(
    instance_id: String,
    launch: u64,
    account: Option<(String, String, String)>,
    world: Option<String>,
    sender: Sender<Reply>,
) {
    let status = match launch_instance(&instance_id, launch, account, world, &sender) {
        Ok(status) => status,
        Err(reason) => Status::Failed(reason),
    };
    // A closed receiver means the window is gone; dropping the last word is
    // right then, and there is no one left to report a failure to anyway.
    let _ = sender.send(Reply { launch, status });
}

/// Resolve, install, spawn and wait. Every stage is reported as it happens.
fn launch_instance(
    instance_id: &str,
    launch: u64,
    account: Option<(String, String, String)>,
    world: Option<String>,
    sender: &Sender<Reply>,
) -> Result<Status, String> {
    // This thread owns the paths because it owns every disk and network call a
    // launch makes. `platform_data_root` is the root `data::instances_dir` reads
    // from, so the id came from the same tree this opens.
    let paths = SiftPaths::at(platform_data_root(System::current()));

    // A world named by the caller is a setting for the length of this launch: `join_target` in
    // the engine reads it and builds `--quickPlaySingleplayer` from it. Written before the
    // instance is prepared, removed once the answer is in.
    let mut world_written = false;
    if let Some(world) = &world {
        let root = paths.configured_instances_dir().join(&instance_id);
        if let Ok(mut instance) = sift_core::instance::Instance::open(&root) {
            instance
                .settings_mut()
                .set_str("QuickPlayWorld", world.clone());
            if instance.save().is_ok() {
                world_written = true;
            }
        }
    }
    let wire = Wire::new(paths.meta_dir());
    // The same cache directory the engine's mirror store has always used. The
    // wire owns the download queue; this store owns the version documents.
    let mut store = OnlineMetaStore::new(DEFAULT_META_BASE_URL, paths.meta_dir());

    // No account flow exists in this shell yet. A fixed offline session is enough
    // to prove the engine path (resolve, install, spawn) and needs no sign-in;
    // the derived UUID makes every such launch the same player.
    let player = "Player";
    let session =
        OfflineSession::new(player, sift_game::accounts::offline_uuid(player)).into_auth_session();

    // The engine explains a refusal through its log sink, not through its return
    // value. Keep the last relevant line so `Blocked` can carry the engine's own
    // reason into `Failed` rather than a sentence invented here.
    let mut refusal = String::new();
    let mut last_line = String::new();
    let mut last_download: Option<(u32, u32)> = None;
    let readiness = {
        let mut log = |line: String| {
            if line.contains("not launching") {
                refusal = line.clone();
            }
            last_line = line;
        };
        let levels = sender.clone();
        let mut progress = |level: install::Progress| {
            let counts = download_counts(&level);
            // Progress arrives once per finishing file per phase. A report with
            // the same counts is not a delta and would only repaint the same
            // string.
            if counts == last_download {
                return;
            }
            last_download = counts;
            let status = match counts {
                Some((done, total)) => Status::Downloading { done, total },
                None => Status::Preparing,
            };
            let _ = levels.send(Reply { launch, status });
        };
        // `None` is the work-area probe this shell does not have yet; the engine
        // falls back to the instance's configured size, and says so in its log.
        prepare_launch(
            &paths,
            instance_id,
            &session,
            &LaunchDefaults::default(),
            &mut store,
            &wire,
            None,
            &mut log,
            &mut progress,
        )
    };

    // The setting has done its job: the plan above already holds the argument. Removed before
    // anything else can fail, so a launch that goes wrong does not leave the next one pointing
    // at a world nobody asked for.
    if world_written {
        let root = paths.configured_instances_dir().join(&instance_id);
        if let Ok(mut instance) = sift_core::instance::Instance::open(&root) {
            if instance.settings_mut().remove("QuickPlayWorld") {
                let _ = instance.save();
            }
        }
    }

    let plan = match readiness {
        LaunchReadiness::Ready(plan) => plan,
        LaunchReadiness::Blocked => {
            let reason = if refusal.trim().is_empty() {
                last_line
            } else {
                refusal
            };
            return Err(if reason.trim().is_empty() {
                "launch preparation was blocked".to_owned()
            } else {
                reason
            });
        }
    };

    let mut command = Command::new(&plan.java_bin);
    command
        .args(&plan.argv)
        .current_dir(&plan.cwd)
        // There is no log window yet, and an unread piped stdout/stderr is a pipe
        // that fills and stops the game. Discarding them is the honest choice
        // until a run-log view exists.
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for (key, value) in &plan.envs {
        command.env(key, value);
    }
    let mut child = command.spawn().map_err(|error| {
        format!(
            "could not start Minecraft with '{}': {error}",
            plan.java_bin
        )
    })?;

    // Only after the spawn succeeded: a `Running` sent before this point would be
    // a claim about a process that might not exist.
    let _ = sender.send(Reply {
        launch,
        status: Status::Running,
    });
    let pid = child.id();
    let exit = child
        .wait()
        .map_err(|error| format!("waiting for Minecraft (pid {pid}) failed: {error}"))?;
    match exit.code() {
        Some(code) => Ok(Status::Exited(code)),
        // `ExitStatus::code` is `None` when a signal ended the process; this
        // shell's state has an integer exit code, so that case is a failed
        // observation rather than an invented code such as `-1`.
        None => Err(format!("Minecraft ended without an exit code ({exit})")),
    }
}

/// The engine's count for a phase, or `None` when the phase is indeterminate.
fn download_counts(level: &install::Progress) -> Option<(u32, u32)> {
    if level.is_indeterminate() || level.total == 0 {
        return None;
    }
    Some((count(level.done), count(level.total)))
}

/// `usize` to the title's `u32` without truncation. A total past `u32::MAX` is
/// not a real download; saturating is still a truthful "at least this many".
fn count(value: usize) -> u32 {
    match u32::try_from(value) {
        Ok(value) => value,
        Err(_) => u32::MAX,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    /// The one fake-worker count this module's start test reads.
    static WORKERS_STARTED: AtomicUsize = AtomicUsize::new(0);

    fn fake_worker(
        _instance: String,
        _launch: u64,
        _account: Option<(String, String, String)>,
        _world: Option<String>,
        _sender: Sender<Reply>,
    ) {
        WORKERS_STARTED.fetch_add(1, Ordering::SeqCst);
    }

    /// `busy` is the frame loop's rule: only work that is still happening keeps
    /// the window drawing. Every terminal state must owe no frame.
    #[test]
    fn only_the_live_states_are_busy() {
        let cases = [
            (Status::Idle, false),
            (Status::Preparing, true),
            (
                Status::Downloading {
                    done: 34,
                    total: 120,
                },
                true,
            ),
            (Status::Running, true),
            (Status::Exited(0), false),
            (Status::Failed("no Java".to_owned()), false),
        ];
        let mut game = Game::default();
        for (status, expected) in cases {
            game.status = status;
            assert_eq!(game.busy(), expected);
        }
    }

    /// A second press while the first worker is out must not start a second
    /// worker. The fake worker never replies, so the first launch stays busy for
    /// the whole test without a network, a JVM or a game.
    #[test]
    fn a_start_while_busy_does_not_start_a_second_worker() {
        WORKERS_STARTED.store(0, Ordering::SeqCst);
        let mut game = Game::with_worker(fake_worker);
        game.start("first");
        game.start("second");

        let deadline = Instant::now() + Duration::from_secs(1);
        while WORKERS_STARTED.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(game.busy(), "the first worker is still the current launch");
        assert_eq!(
            WORKERS_STARTED.load(Ordering::SeqCst),
            1,
            "the second press was ignored"
        );
    }
}
