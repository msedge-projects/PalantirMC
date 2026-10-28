//! Installing a Discover card's pack, off the frame thread.
//!
//! The engine's pack install is a version resolve, an archive download, an import into a new
//! instance and a fetch of every file the index lists. None of that can happen on egui's frame
//! thread, so this is the shape [`crate::game`] already uses for a launch: [`Install::start`]
//! hands one project to a worker thread, [`Install::poll`] collects what that worker said, and
//! a frame only reads what is already waiting.
//!
//! **The worker owns the paths**, exactly as the launch worker does. The root is the
//! `platform_data_root` the engine's own instance creation uses, so the instance this makes is
//! the one a launch opens and the Home page lists.
//!
//! **The state machine is the card's, not the title bar's.** A launch reports into the running
//! instance chip; an install has no such slot, so its states are the three busy phases the
//! button draws as `Installing…` and the two terminal sentences the shell turns into a toast.
//!
//! **Which version is not this module's choice.** [`install::newest_pack_version`] is the rule
//! -- releases, then betas, then alphas, keeping the API's publish-date order -- because the
//! engine already applies it to a pack, and a second ranking here would be a second answer to
//! one question.

use std::sync::mpsc::{channel, Receiver, Sender};

use sift_core::paths::{platform_data_root, SiftPaths, System};
// The engine's pack install: the rules this module only wraps in a frame's state machine.
use sift_game::install;
use sift_game::wire::Wire;

/// What a Discover card's Install button is doing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Status {
    /// No install has been asked for yet.
    #[default]
    Idle,
    /// The project's version list is being read and its newest pack chosen.
    Resolving,
    /// A phase is moving, with the engine's own counts.
    ///
    /// `done: 0, total: 0` is the archive's own transfer: the engine's `install_file` gives
    /// its download a no-op reporter, so that phase has no total to be a fraction of and says
    /// so rather than inventing one.
    ///
    /// The counts are kept because the engine reports them and a state machine that dropped
    /// them could not draw a bar without asking the engine again; this shell's button says
    /// `Installing…` in all three busy phases, so nothing reads them yet.
    #[allow(dead_code)]
    Downloading {
        /// Files finished so far.
        done: usize,
        /// Files this phase will fetch in total.
        total: usize,
    },
    /// The archive is on disk and the pack is being imported: the new instance, its
    /// `overrides/` tree, then the files its index lists.
    Installing,
    /// The pack is installed. The string is the sentence the toast shows, which names the
    /// instance that did not exist before.
    Done(String),
    /// The pack could not be installed, with the engine's own reason.
    Failed(String),
}

/// One project to install: what the card that asked for it knows.
#[derive(Debug, Clone)]
struct Job {
    /// The Modrinth project id, which is what `version_url` takes and what a card compares
    /// itself against.
    project: String,
    /// The project's name: the new instance's name, and the subject of both sentences.
    title: String,
}

/// One worker's word, tagged with the install it belongs to.
struct Reply {
    /// The `Install::generation` value that asked for this worker.
    generation: u64,
    status: Status,
}

/// The function one worker runs. A pointer rather than a trait object so a test can
/// substitute a worker without a network or a disk.
type Worker = fn(Job, u64, Sender<Reply>);

/// The pack install behind the Discover cards, and the channel its worker answers on.
pub struct Install {
    status: Status,
    /// The project the current status belongs to, so a card can ask whether the install it is
    /// drawing is its own and a failure can name the pack.
    job: Option<Job>,
    /// Incremented for every install that is allowed to start. A reply carrying a different
    /// number belongs to an install the reader has already moved past.
    generation: u64,
    sender: Sender<Reply>,
    receiver: Receiver<Reply>,
    /// The real worker in the window; a probe in the tests.
    worker: Worker,
}

impl Default for Install {
    fn default() -> Install {
        Install::with_worker(run_install)
    }
}

impl Install {
    /// Build an install holder around `worker`.
    fn with_worker(worker: Worker) -> Install {
        let (sender, receiver) = channel();
        Install {
            status: Status::Idle,
            job: None,
            generation: 0,
            sender,
            receiver,
            worker,
        }
    }

    /// Start installing `project`, under `title`, unless an install is already out.
    ///
    /// The check is before the generation bump and before the spawn, so a second press while
    /// the first worker is out is a no-op rather than a second import into the same instances
    /// folder. `Resolving` is set here, on the frame that asked, not when the worker's first
    /// word arrives: that is the frame the person who pressed the button sees.
    pub fn start(&mut self, project: &str, title: &str) {
        if self.busy() {
            return;
        }
        let job = Job {
            project: project.to_owned(),
            title: title.to_owned(),
        };
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.status = Status::Resolving;

        let sent = job.clone();
        let sender = self.sender.clone();
        let worker = self.worker;
        let spawned = std::thread::Builder::new()
            .name("sift-install".to_owned())
            .spawn(move || worker(sent, generation, sender));
        // Remembered even when the spawn failed: the card still has to read `Install` rather
        // than pass the press off as somebody else's.
        self.job = Some(job);
        if let Err(error) = spawned {
            self.status = Status::Failed(format!("could not start an install worker: {error}"));
        }
    }

    /// Take every reply that is ready, and answer the sentence a finished install owes the
    /// shell. Called once per frame.
    pub fn poll(&mut self) -> Option<String> {
        let mut line = None;
        while let Ok(reply) = self.receiver.try_recv() {
            // A reply for an install the reader has moved past is dropped: showing it would
            // put an old state under the new one, the same rule the search and launch
            // workers apply to an abandoned request.
            if reply.generation != self.generation {
                continue;
            }
            if let Some(text) = toast_line(&reply.status, self.job.as_ref()) {
                line = Some(text);
            }
            self.status = reply.status;
        }
        line
    }

    /// Whether an install is still alive. Exactly resolving, downloading or installing keeps
    /// the shell drawing; every terminal state owes no frame.
    pub fn busy(&self) -> bool {
        matches!(
            self.status,
            Status::Resolving | Status::Downloading { .. } | Status::Installing
        )
    }

    /// What the install button on `project`'s card shows.
    ///
    /// Compared by project id rather than by title: the same project can legitimately appear
    /// twice in a list (two sources, a repeat) while its id is the key the version URL takes,
    /// and another card's button stays an offer even while one install is out.
    pub fn label(&self, project: &str) -> &'static str {
        let mine = self.job.as_ref().map(|job| job.project.as_str()) == Some(project);
        match (&self.status, mine) {
            (Status::Done(_), true) => "Installed",
            (
                Status::Resolving | Status::Downloading { .. } | Status::Installing,
                true,
            ) => "Installing…",
            // A failure reads `Install` again on purpose: the engine's sentence is in the
            // toast, and the button is the retry.
            _ => "Install",
        }
    }
}

/// One worker: run the engine and always leave a terminal status behind.
fn run_install(job: Job, generation: u64, sender: Sender<Reply>) {
    let status = match install_pack(&job, generation, &sender) {
        Ok(summary) => Status::Done(summary),
        Err(reason) => Status::Failed(reason),
    };
    // A closed receiver means the window is gone; dropping the last word is right then, and
    // there is no one left to report a failure to anyway.
    let _ = sender.send(Reply { generation, status });
}

/// Resolve the newest pack, fetch its archive, import it as an instance, fill it.
fn install_pack(job: &Job, generation: u64, sender: &Sender<Reply>) -> Result<String, String> {
    // This thread owns the paths because it owns every disk and network call an install
    // makes, and it is the same root a launch opens its instance from.
    let paths = SiftPaths::at(platform_data_root(System::current()));
    let wire = Wire::new(paths.meta_dir());

    // Resolving. The URL is the client crate's own builder rather than a format string here,
    // and the document goes through the wire's cached request -- the same one client and the
    // same cache as every other metadata question this launcher asks.
    let url = sift_net::modrinth::version_url(&job.project);
    // shortcut: the list is believed for the metadata cache's TTL (30 minutes), so a release
    // published within it is not seen -- upgrade when a reader needs the very newest.
    let body = wire.document(&url)?;
    let versions: Vec<sift_net::modrinth::ModrinthProjectVersion> =
        serde_json::from_str(&body).map_err(|error| format!("{url}: {error}"))?;
    let version = install::newest_pack_version(&versions)
        .ok_or_else(|| format!("{} has no version with a file to download", job.title))?;

    // Downloading, uncounted: the archive itself. `cache/meta/packs/` is the directory the
    // engine's own documentation names, so a second install of the same pack version costs
    // nothing.
    report(
        sender,
        generation,
        Status::Downloading {
            done: 0,
            total: 0,
        },
    );
    let archive = install::fetch_pack_archive(&wire, &paths.meta_dir().join("packs"), version)?;

    // Installing: the import that creates the instance, then the pack's files, which are the
    // phase that reports a count.
    report(sender, generation, Status::Installing);
    let mut progress = |level: install::Progress| {
        report(
            sender,
            generation,
            Status::Downloading {
                done: level.done,
                total: level.total,
            },
        );
    };
    // A project that is not a pack fails here with the engine's own words ("neither a
    // .mrpack nor a CurseForge pack"), which is the honest answer for the Mod and Resource
    // Pack tabs: this button installs packs into instances of their own.
    let installed =
        install::install_pack_archive(&wire, &paths, &archive, &job.title, &mut progress)?;
    // The engine's sentence, because it is the one that names both the pack and the instance.
    Ok(installed.summary(&job.title))
}

/// Send one word to the frame, ignoring a window that has gone.
fn report(sender: &Sender<Reply>, generation: u64, status: Status) {
    let _ = sender.send(Reply { generation, status });
}

/// The sentence a status owes the shell.
///
/// A finished install names the instance it created, a failed one names the pack the engine
/// could not install, and progress owes nothing: the button already says it.
fn toast_line(status: &Status, job: Option<&Job>) -> Option<String> {
    match status {
        Status::Done(summary) => Some(summary.clone()),
        Status::Failed(reason) => Some(failure_line(
            job.map(|job| job.title.as_str()).unwrap_or_default(),
            reason,
        )),
        Status::Idle | Status::Resolving | Status::Downloading { .. } | Status::Installing => None,
    }
}

/// A failure with the pack it was about in front of the engine's own reason.
///
/// The engine's sentence stays whole -- it is the one thing that says what actually went
/// wrong -- and the name goes in front because a toast on a page of cards would otherwise not
/// say which card it is about.
fn failure_line(title: &str, reason: &str) -> String {
    let title = if title.is_empty() { "that pack" } else { title };
    format!("Could not install {title}: {reason}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    /// The one fake-worker count this module's start test reads.
    static WORKERS_STARTED: AtomicUsize = AtomicUsize::new(0);

    fn fake_worker(_job: Job, _generation: u64, _sender: Sender<Reply>) {
        WORKERS_STARTED.fetch_add(1, Ordering::SeqCst);
    }

    fn job(project: &str) -> Job {
        Job {
            project: project.to_owned(),
            title: "Cobblemon".to_owned(),
        }
    }

    /// `busy` is the frame loop's rule: only work that is still happening keeps the window
    /// drawing. Every terminal state must owe no frame.
    #[test]
    fn only_the_working_states_are_busy() {
        let cases = [
            (Status::Idle, false),
            (Status::Resolving, true),
            (
                Status::Downloading {
                    done: 3,
                    total: 40,
                },
                true,
            ),
            // The archive's own uncounted transfer is still work.
            (
                Status::Downloading {
                    done: 0,
                    total: 0,
                },
                true,
            ),
            (Status::Installing, true),
            (Status::Done("Installed X as X, 4 files".to_owned()), false),
            (Status::Failed("no version".to_owned()), false),
        ];
        let mut install = Install::default();
        for (status, expected) in cases {
            install.status = status;
            assert_eq!(install.busy(), expected);
        }
    }

    /// A second press while the first worker is out must not start a second worker. The fake
    /// worker never replies, so the first install stays busy for the whole test without a
    /// network or a disk.
    #[test]
    fn a_start_while_busy_does_not_start_a_second_worker() {
        WORKERS_STARTED.store(0, Ordering::SeqCst);
        let mut install = Install::with_worker(fake_worker);
        install.start("first", "First");
        install.start("second", "Second");

        let deadline = Instant::now() + Duration::from_secs(1);
        while WORKERS_STARTED.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(install.busy(), "the first worker is still the current install");
        assert_eq!(
            WORKERS_STARTED.load(Ordering::SeqCst),
            1,
            "the second press was ignored"
        );
        assert_eq!(
            install.label("first"),
            "Installing…",
            "and the press that was taken is the one the button reports"
        );
        assert_eq!(install.label("second"), "Install", "the refused card is untouched");
    }

    /// The button's three words, per state and per card: the card whose project is being
    /// installed says so, and no other card claims it.
    #[test]
    fn the_button_says_what_the_install_is_doing() {
        let mut install = Install::default();
        assert_eq!(install.label("A"), "Install", "nothing asked yet");

        install.job = Some(job("A"));
        for status in [
            Status::Resolving,
            Status::Downloading {
                done: 2,
                total: 9,
            },
            Status::Installing,
        ] {
            install.status = status;
            assert_eq!(install.label("A"), "Installing…");
            assert_eq!(install.label("B"), "Install", "another card is still an offer");
        }

        install.status = Status::Done("Installed A as A, 9 files".to_owned());
        assert_eq!(install.label("A"), "Installed");
        assert_eq!(install.label("B"), "Install");

        // A failure puts the button back to a retry rather than leaving it reading as done.
        install.status = Status::Failed("no version with a file".to_owned());
        assert_eq!(install.label("A"), "Install");
    }

    /// The toast's sentence: a finished install is the engine's own summary, a failure names
    /// the pack in front of the engine's reason, and progress owes nothing.
    #[test]
    fn a_terminal_status_owes_the_shell_one_sentence() {
        let summary = "Installed Cobblemon as Cobblemon, 42 files".to_owned();
        assert_eq!(
            toast_line(&Status::Done(summary.clone()), Some(&job("A"))),
            Some(summary)
        );
        assert_eq!(
            toast_line(&Status::Failed("no direct download".to_owned()), Some(&job("A"))),
            Some("Could not install Cobblemon: no direct download".to_owned())
        );
        for status in [
            Status::Idle,
            Status::Resolving,
            Status::Downloading {
                done: 1,
                total: 2,
            },
            Status::Installing,
        ] {
            assert_eq!(toast_line(&status, Some(&job("A"))), None);
        }
    }

    /// A failure with no title behind it still reads as a sentence: the title comes from the
    /// API and a project may publish none.
    #[test]
    fn a_failure_without_a_title_still_names_the_pack() {
        assert_eq!(
            failure_line("", "connection reset"),
            "Could not install that pack: connection reset"
        );
        assert_eq!(
            failure_line("Cobblemon", "connection reset"),
            "Could not install Cobblemon: connection reset"
        );
    }
}
