//! The launch customs, offline: arguments, environment, and the hooks.
//!
//! Every claim here is checked against a real spawned process whose argv or
//! environment a mock script writes down -- the acceptance criteria of the
//! slice, not inspection. The quoting trap the reference migrated away from
//! (paths with spaces split on whitespace) is a regression test here.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

// A sibling thread's fork can inherit a writable mock-script descriptor until
// exec closes it; Linux then refuses the script with ETXTBSY. Serialize mock
// creation through process exit (the same rule as launch_offline.rs).
// A panic in one test poisons no other: the mutex is only a turn-taking
// token, so a poisoned lock is taken over rather than cascading into
// failures that mask the real assertion.
static MOCK_PROCESS: Mutex<()> = Mutex::new(());

fn turn() -> std::sync::MutexGuard<'static, ()> {
    MOCK_PROCESS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

use palantir_loader::hooks::{LaunchCustom, command_words, env_pairs, run, wrap};

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("palantirmc-hooks-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A mock program that writes its argv (and one environment value) to files
/// beside itself. Cross-platform because CI runs windows-latest.
fn mock_script(dir: &Path, name: &str, body: &str) -> PathBuf {
    #[cfg(windows)]
    let (name, script) = (
        format!("{name}.bat"),
        format!("@echo off\r\n{body}\r\nexit /b %errorlevel%\r\n"),
    );
    #[cfg(not(windows))]
    let (name, script) = (
        format!("{name}.sh"),
        format!("#!/bin/sh\n{body}\nexit $?\n"),
    );
    let path = dir.join(name);
    std::fs::write(&path, script).unwrap();
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    path
}

fn dump_argv_body() -> &'static str {
    #[cfg(windows)]
    {
        "(for %%a in (%*) do @echo %%a) > argv.txt"
    }
    #[cfg(not(windows))]
    {
        r#"printf '%s\n' "$@" > argv.txt"#
    }
}

#[test]
fn extra_args_reach_the_process_in_order_after_the_plan() {
    let _guard = turn();
    let dir = temp_dir("args");
    // The mock stands in for the whole java command line; LaunchCustom only
    // ever appends, so the plan's own words come first.
    let mock = mock_script(
        &dir,
        "game",
        &format!("{}\necho done > ran.txt", dump_argv_body()),
    );
    let custom = LaunchCustom {
        extra_args: command_words(r#"--quickPlaySingleplayer "My World" --extra"#).unwrap(),
        ..LaunchCustom::default()
    };
    let mut command = std::process::Command::new(&mock);
    command.current_dir(&dir);
    command.arg("--plan-arg");
    custom.apply(&mut command);
    let status = command.status().unwrap();
    assert!(status.success());
    let argv = std::fs::read_to_string(dir.join("argv.txt")).unwrap();
    let lines: Vec<&str> = argv.lines().collect();
    // In order, quoted words intact, after the plan's own argument.
    assert_eq!(
        lines,
        vec![
            "--plan-arg",
            "--quickPlaySingleplayer",
            "My World",
            "--extra"
        ]
    );
}

#[test]
fn an_env_value_with_an_equals_survives_intact() {
    let _guard = turn();
    let dir = temp_dir("env");
    #[cfg(windows)]
    let body = "echo %PALANTIR_TEST_VALUE% > env.txt";
    #[cfg(not(windows))]
    let body = r#"printf '%s' "$PALANTIR_TEST_VALUE" > env.txt"#;
    let mock = mock_script(&dir, "game", body);
    let custom = LaunchCustom {
        env: env_pairs(&command_words("PALANTIR_TEST_VALUE=a=b=c").unwrap()).unwrap(),
        ..LaunchCustom::default()
    };
    let mut command = std::process::Command::new(&mock);
    command.current_dir(&dir);
    custom.apply(&mut command);
    assert!(command.status().unwrap().success());
    assert_eq!(
        std::fs::read_to_string(dir.join("env.txt")).unwrap().trim(),
        "a=b=c"
    );
}

#[test]
fn a_pre_launch_hook_with_a_space_in_its_path_runs() {
    let _guard = turn();
    // The v1 quoting trap: a hook whose path contains a space, written
    // quoted the way a shell accepts it. Whitespace-splitting would look for
    // `/tmp/.../hook` and fail.
    let dir = temp_dir("hook");
    let spaced = dir.join("dir with space");
    std::fs::create_dir_all(&spaced).unwrap();
    let hook = mock_script(&spaced, "hook", "echo ran > marker.txt");
    let command = format!("\"{}\"", hook.display());
    let output = run(&command, &dir, &[]).unwrap();
    assert!(
        output.status.success(),
        "hook failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(dir.join("marker.txt").exists(), "the hook never ran");
}

#[test]
fn the_wrapper_becomes_the_program_and_keeps_the_order() {
    let _guard = turn();
    let dir = temp_dir("wrap");
    let mock = mock_script(&dir, "wrapper", dump_argv_body());
    let java = PathBuf::from("/opt/java with space/bin/java");
    let (program, args) = wrap(
        &format!("\"{}\" --wrapper-flag", mock.display()),
        &java,
        &[
            "-Xmx1024M".to_string(),
            "MainClass".to_string(),
            "--game".to_string(),
        ],
    )
    .unwrap();
    assert_eq!(program, mock);
    let mut command = std::process::Command::new(program);
    command.current_dir(&dir);
    command.args(&args);
    assert!(command.status().unwrap().success());
    let argv = std::fs::read_to_string(dir.join("argv.txt")).unwrap();
    let lines: Vec<&str> = argv.lines().collect();
    assert_eq!(
        lines,
        vec![
            "--wrapper-flag",
            "/opt/java with space/bin/java",
            "-Xmx1024M",
            "MainClass",
            "--game",
        ]
    );
}

#[test]
fn hook_failures_are_visible_to_the_caller() {
    let _guard = turn();
    let dir = temp_dir("hooks");
    let ok = mock_script(&spaced_dir(&dir), "ok", "echo fine > ok.txt");
    let output = run(&format!("\"{}\"", ok.display()), &dir, &[]).unwrap();
    assert!(output.status.success());
    assert!(dir.join("ok.txt").exists());

    // A hook that exits badly reports it: the caller decides what that means
    // (pre-launch stops the launch; post-exit is noted and the launcher
    // carries on), but the failure itself is never invisible.
    #[cfg(windows)]
    let bad = mock_script(&dir, "bad", "echo bad hook 1>&2\r\nexit /b 3");
    #[cfg(not(windows))]
    let bad = mock_script(&dir, "bad", "echo bad hook 1>&2\nexit 3");
    let output = run(&format!("\"{}\"", bad.display()), &dir, &[]).unwrap();
    assert!(
        !output.status.success(),
        "a bad hook must not look successful"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("bad hook"),
        "the hook's own diagnosis must reach the caller"
    );

    // An empty or unparseable hook refuses instead of running something else.
    assert!(run("", &dir, &[]).is_err());
    assert!(run("\"unterminated", &dir, &[]).is_err());
}

fn spaced_dir(dir: &Path) -> PathBuf {
    let spaced = dir.join("dir with space");
    std::fs::create_dir_all(&spaced).unwrap();
    spaced
}
