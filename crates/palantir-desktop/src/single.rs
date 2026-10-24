//! One launcher at a time, and a lock a crash cannot make permanent.
//!
//! Two windows on one data root write the same instance files: a create in one
//! and a delete in the other, two installs into one instance, two launches
//! fighting over the same log. The lock file is the obvious answer, and the
//! obvious version of it is a trap -- a launcher that is killed, or that panics, or
//! that is ended from the task manager leaves the file behind, and every later
//! start refuses to open against a file nobody owns. A launcher that will not
//! start until a file is deleted by hand is worse than two windows.
//!
//! So the lock is a **claim by process id**: whoever holds it wrote their own pid
//! into it, and a start that finds a claim asks the OS whether that process is
//! still alive. A live one is a second window and this start declines; a dead one
//! is a stale file, and the new start takes it over. The liveness question is the
//! one thing each OS answers its own way -- `OpenProcess` and `GetExitCodeProcess`
//! on Windows, `/proc/<pid>` on Linux -- and both are asked with the cheapest call
//! that is definitive rather than with a timeout somebody would have to choose.
//!
//! **The claim is taken by exclusive create**, not by reading then writing: two
//! starts a millisecond apart both see "no lock" that way. `create_new` decides
//! which one wins, and the loser re-reads to find out who did.
//!
//! **A capture does not claim.** `--shot` renders one frame and exits and touches
//! no instance state, and the tools in `tools/` drive it while a launcher may well
//! be open on the same desk; refusing that would break the capture loop rather
//! than protect anything.
//!
//! What this deliberately does not do is *focus the window that is already open*.
//! That needs a channel between the two processes -- a named pipe, or a registered
//! window message -- and is a change of its own; what a second start does here is
//! say which process is running and leave.

use std::path::{Path, PathBuf};

/// The claim file, under the launcher's own home.
///
/// Beside the preferences rather than in the data root: the data root can be an
/// install another launcher created and can change, and a lock that moved with it
/// would not be a lock on this launcher.
pub const LOCK_FILE: &str = "palantir.pid";

/// What a start found when it asked for the lock.
#[derive(Debug)]
pub enum Claim {
    /// This process holds it, for as long as the value is kept.
    ///
    /// Dropping it releases the claim, which is why the caller binds it to a
    /// named local rather than to `_`.
    Held(Lock),
    /// Another process holds it and is alive.
    AlreadyRunning(u32),
}

impl Claim {
    /// The lock, when this process got it.
    pub fn held(self) -> Option<Lock> {
        match self {
            Claim::Held(lock) => Some(lock),
            Claim::AlreadyRunning(_) => None,
        }
    }
}

/// A held claim: the file exists until this is dropped.
#[derive(Debug)]
pub struct Lock {
    path: PathBuf,
}

impl Lock {
    /// The file this claim is kept in.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Lock {
    /// Remove the claim, best effort.
    ///
    /// A file that cannot be removed is stale, and the next start's liveness
    /// check is what makes a stale file harmless -- which is the whole point of
    /// the pid being in it.
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// The claim file for a launcher's home.
pub fn lock_path(home: &palantir_core::paths::PalantirPaths) -> PathBuf {
    home.root.join(LOCK_FILE)
}

/// Tell whoever tried to start a second window that one is already open.
///
/// A message box rather than a log line: this build is `windows_subsystem =
/// "windows"` and may well have been started from a shortcut, so a console line
/// would go where nobody can read it and the click would look like nothing
/// happening at all. The pid is in the sentence because it is the one fact that
/// makes the running process findable in the task manager.
#[cfg(windows)]
pub fn tell_the_user(pid: u32) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OK};

    let sentence = format!(
        "PalantirMC is already running (process {pid}).\n\n\
         Use the window that is open, or close it and start again."
    );
    let text: Vec<u16> = sentence.encode_utf16().chain(std::iter::once(0)).collect();
    let title: Vec<u16> = "PalantirMC"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    // Both pointers are to buffers this function owns, alive across the call, and
    // the dialog is modal until it is answered; there is no pre-condition left
    // over, which is all the `unsafe` covers.
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

/// The same sentence where there is no dialog to put it in.
#[cfg(not(windows))]
pub fn tell_the_user(pid: u32) {
    eprintln!("PalantirMC is already running (process {pid}).");
}

/// Take the lock, or find out who has it.
///
/// The exclusive create is what decides a race: the winner writes its pid, and
/// the loser reads it. A lock whose owner is dead is removed and created again,
/// once -- a second failure to create is reported rather than retried in a loop,
/// because a lock file that cannot be created is a broken home directory rather
/// than a busy launcher.
pub fn claim(path: &Path) -> Result<Claim, String> {
    match create(path) {
        Ok(()) => Ok(Claim::Held(Lock {
            path: path.to_path_buf(),
        })),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            match read_pid(path) {
                Some(pid) if alive(pid) => Ok(Claim::AlreadyRunning(pid)),
                // No pid, or a pid whose process is gone: the file is stale and
                // this start owns it from here.
                _ => {
                    let _ = std::fs::remove_file(path);
                    match create(path) {
                        Ok(()) => Ok(Claim::Held(Lock {
                            path: path.to_path_buf(),
                        })),
                        Err(error) => Err(format!(
                            "the lock at {} could not be taken over: {error}",
                            path.display()
                        )),
                    }
                }
            }
        }
        Err(error) => Err(format!(
            "the lock at {} could not be written: {error}",
            path.display()
        )),
    }
}

/// Create the claim file if nobody else has, with this process's id in it.
fn create(path: &Path) -> std::io::Result<()> {
    use std::io::Write as _;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    // Written and flushed before the claim is treated as held: a file with no
    // pid in it would be read as stale by the next start, which is the one
    // reading this cannot afford to lose.
    file.write_all(std::process::id().to_string().as_bytes())?;
    file.flush()
}

/// The pid a claim file names, if it names one.
fn read_pid(path: &Path) -> Option<u32> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| text.trim().parse::<u32>().ok())
        .filter(|pid| *pid > 0)
}

/// Whether `pid` is a process that exists and has not exited.
///
/// **Windows**: `OpenProcess` with the least right that can answer the question,
/// then `GetExitCodeProcess`, which reports `STILL_ACTIVE` for a process that is
/// still running and the exit status for one that has ended but not been reaped.
/// A pid that cannot be opened at all is not this launcher's (its own previous
/// instance runs as the same user), and is read as gone.
#[cfg(windows)]
fn alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    // Both calls take a handle this function owns and closes; neither has a
    // pre-condition this code does not satisfy, which is the whole of the
    // `unsafe` here.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        let mut code: u32 = 0;
        let asked = GetExitCodeProcess(handle, &mut code);
        CloseHandle(handle);
        asked != 0 && code == STILL_ACTIVE as u32
    }
}

/// Whether `pid` is a process that still exists, on Linux: its `/proc` entry is
/// the answer, and an entry that cannot be read is a process that is gone.
#[cfg(target_os = "linux")]
fn alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

/// There is no portable way to ask this question here, so a claim is treated as
/// stale and the launcher starts.
///
/// That is the failure this module is willing to have on a host it does not ship
/// to: the Linux and macOS builds exist to run tests and the build VM, and a
/// launcher that refuses to start because of a file it cannot ask about is the
/// trap the liveness check exists to avoid.
#[cfg(not(any(windows, target_os = "linux")))]
fn alive(_pid: u32) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir()
            .join("palantirmc-single-tests")
            .join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a scratch directory");
        root.join(LOCK_FILE)
    }

    /// A pid that cannot be running: no process table has this one.
    const DEAD: u32 = u32::MAX;

    #[test]
    fn a_claim_is_written_with_this_process_in_it() {
        let path = scratch("claim");
        let held = claim(&path).expect("a claim");
        let Claim::Held(lock) = &held else {
            panic!("a fresh file is a claim, not a refusal: {held:?}");
        };
        assert_eq!(lock.path(), path.as_path());
        assert_eq!(read_pid(&path), Some(std::process::id()));
        // A second claim from the same process is refused, because the process it
        // names is alive -- which is what a second window is.
        match claim(&path).expect("a second answer") {
            Claim::AlreadyRunning(pid) => assert_eq!(pid, std::process::id()),
            other => panic!("the same process should be refused: {other:?}"),
        }
    }

    #[test]
    fn dropping_the_claim_releases_it() {
        let path = scratch("release");
        let held = claim(&path).expect("a claim");
        assert!(path.exists());
        drop(held);
        assert!(!path.exists(), "the claim file outlived the claim");
        assert!(matches!(
            claim(&path).expect("a fresh claim"),
            Claim::Held(_)
        ));
    }

    /// The whole reason the pid is in the file: a launcher that was killed leaves
    /// the file behind, and the next start must take it rather than refuse.
    #[test]
    fn a_lock_left_by_a_dead_process_is_taken_over() {
        let path = scratch("stale");
        std::fs::write(&path, DEAD.to_string()).expect("a stale lock");
        assert!(!alive(DEAD), "the dead pid is somehow alive");
        let held = claim(&path).expect("a claim");
        assert!(
            matches!(held, Claim::Held(_)),
            "a stale lock must be taken over: {held:?}"
        );
        // While the claim is *held*: dropping it removes the file, which the
        // release test covers and this one would otherwise measure by accident.
        assert_eq!(read_pid(&path), Some(std::process::id()));
    }

    /// A lock file that does not hold a pid at all -- truncated by a crash mid
    /// write, or hand-edited -- is stale rather than a reason to refuse.
    #[test]
    fn a_lock_file_with_no_pid_in_it_is_stale() {
        let path = scratch("empty");
        for contents in ["", "   ", "not a pid", "0"] {
            std::fs::write(&path, contents).expect("a broken lock");
            assert!(
                matches!(claim(&path).expect("a claim"), Claim::Held(_)),
                "{contents:?} should be stale"
            );
        }
    }

    /// The live-but-not-us branch, with a process the test really started.
    #[cfg(unix)]
    #[test]
    fn a_lock_held_by_a_live_process_is_respected() {
        let path = scratch("live");
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("a child process");
        std::fs::write(&path, child.id().to_string()).expect("a lock naming the child");
        match claim(&path).expect("an answer") {
            Claim::AlreadyRunning(pid) => assert_eq!(pid, child.id()),
            other => panic!("a live process's lock must be respected: {other:?}"),
        }
        let _ = child.kill();
        let _ = child.wait();
        // And once it is gone the same file is stale, which is the crash case
        // arriving through the real liveness check rather than through DEAD.
        assert!(matches!(
            claim(&path).expect("a claim after the child"),
            Claim::Held(_)
        ));
    }

    /// The start has to ask before it opens a window, and a source gate is the
    /// only way to hold that: `run_shell` cannot be run in a test.
    #[test]
    fn the_shell_claims_the_lock_before_it_opens_a_window() {
        let main = include_str!("main.rs");
        let run_shell = main
            .split("fn run_shell(")
            .nth(1)
            .expect("run_shell is gone from main.rs");
        assert!(
            run_shell.contains("single::claim("),
            "run_shell no longer claims the lock"
        );
        assert!(
            run_shell.find("single::claim(") < run_shell.find("Shell::run"),
            "the lock is claimed after the window opens"
        );
    }
}
