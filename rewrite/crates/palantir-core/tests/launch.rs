//! Launch plans built from the real downloaded samples.
//!
//! The output is checked where the format is strangest: the 2026 argument
//! shape with its three lists, the pre-2018 single string with no JVM list
//! at all, and the platform rules that split one list across operating
//! systems. The invariant behind all of them: once a plan is built, no
//! placeholder may survive into the command.

use std::path::PathBuf;

use palantir_core::rules::{Arch, Os, Platform};
use palantir_core::version::Version;
use palantir_loader::launch::{LaunchContext, LaunchPlan, build_launch_plan};

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

fn version(name: &str) -> Version {
    Version::parse(&fixture(name)).unwrap()
}

fn context() -> LaunchContext {
    LaunchContext {
        player_name: "Steve".to_string(),
        player_uuid: "uuid-1".to_string(),
        access_token: "token-1".to_string(),
        clientid: "client-1".to_string(),
        auth_xuid: "xuid-1".to_string(),
        user_type: "msa".to_string(),
        launcher_name: "palantir-test".to_string(),
        launcher_version: "0.0.0".to_string(),
        java: PathBuf::from("/data/java/bin/java"),
        game_dir: PathBuf::from("/data/game"),
        assets_root: PathBuf::from("/data/assets"),
        natives_dir: PathBuf::from("/data/natives"),
        library_root: PathBuf::from("/data/libraries"),
        client_jar: PathBuf::from("/data/versions/26.3/26.3.jar"),
        ..LaunchContext::default()
    }
}

fn platform(os: Os) -> Platform {
    Platform::new(os, "10.0.17134".to_string(), Arch::X86_64)
}

/// The invariant: every entry is a real jar path and no placeholder survives
/// into any argument the process will see.
fn assert_fully_expanded(plan: &LaunchPlan) {
    assert!(!plan.classpath.is_empty(), "plan has no classpath");
    for arg in plan.jvm_args.iter().chain(&plan.game_args) {
        assert!(!arg.contains("${"), "placeholder survived into {arg:?}");
    }
    for entry in &plan.classpath {
        assert!(
            entry.extension().is_some_and(|e| e == "jar"),
            "classpath entry is not a jar: {entry:?}"
        );
    }
}

#[test]
fn windows_gets_the_windows_only_jvm_args() {
    let version = version("version-26.3.json");
    let plan = build_launch_plan(&version, &platform(Os::Windows), &context()).unwrap();
    assert_fully_expanded(&plan);
    assert!(
        plan.jvm_args.iter().any(|a| a.contains("HeapDumpPath")),
        "windows-only heapdump arg missing: {:?}",
        plan.jvm_args
    );
    assert!(
        !plan.jvm_args.contains(&"-XstartOnFirstThread".to_string()),
        "macOS-only arg leaked onto windows"
    );
    // The classpath separator is the platform's, not the host's.
    assert!(
        plan.jvm_args.iter().any(|a| a.contains(';')),
        "classpath was not joined with ';': {:?}",
        plan.jvm_args
    );
}

#[test]
fn macos_gets_the_macos_only_jvm_args() {
    let version = version("version-26.3.json");
    let plan = build_launch_plan(&version, &platform(Os::MacOs), &context()).unwrap();
    assert_fully_expanded(&plan);
    assert!(
        plan.jvm_args.contains(&"-XstartOnFirstThread".to_string()),
        "macOS-only arg missing: {:?}",
        plan.jvm_args
    );
    assert!(
        !plan.jvm_args.iter().any(|a| a.contains("HeapDumpPath")),
        "windows-only arg leaked onto macOS"
    );
}

#[test]
fn linux_26_3_rules_split() {
    let version = version("version-26.3.json");
    let plan = build_launch_plan(&version, &platform(Os::Linux), &context()).unwrap();
    assert_fully_expanded(&plan);
    // Neither OS's gated arg applies here, and the 32-bit one does not on
    // x86_64 -- the rules split one list by machine and this machine gets
    // only the unconditional entries.
    assert!(!plan.jvm_args.iter().any(|a| a.contains("HeapDumpPath")));
    assert!(!plan.jvm_args.contains(&"-XstartOnFirstThread".to_string()));
    assert!(!plan.jvm_args.contains(&"-Xss1M".to_string()));
    assert!(
        plan.jvm_args
            .contains(&"--enable-native-access=ALL-UNNAMED".to_string()),
        "unconditional JVM arg missing: {:?}",
        plan.jvm_args
    );
}

#[test]
fn pre_2018_single_string_version() {
    let version = version("version-1.12.2.json");
    let plan = build_launch_plan(&version, &platform(Os::Windows), &context()).unwrap();
    assert_fully_expanded(&plan);
    // The pre-2018 game string is split on whitespace and expanded in place.
    assert_eq!(plan.game_args[0], "--username");
    assert_eq!(plan.game_args[1], "Steve");
    // That shape names no JVM list at all: the command supplies the classpath.
    assert!(plan.jvm_args.is_empty());
    let command = plan.command();
    let argv: Vec<String> = command
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let cp = argv
        .iter()
        .position(|a| a == "-cp")
        .expect("command has no -cp");
    assert!(
        argv[cp + 1].contains("26.3.jar"),
        "client jar not on the classpath: {argv:?}"
    );
    // The other thing pre-2018 launchers supplied by hand: where the
    // natives live. The document names no JVM list, so the command carries
    // it without pretending the document did.
    assert!(
        argv.iter().any(|a| a.starts_with("-Djava.library.path=")),
        "launcher-supplied natives path missing: {argv:?}"
    );
}

#[test]
fn inherits_from_loader_profile() {
    // A loader profile is an overlay: it names `inheritsFrom` and carries no
    // client download of its own. Merged over its game -- as an install does
    // -- it plans like any other version.
    let profile = version("fabric-loader-profile-1.20.1.json");
    let game = version("version-1.20.1.json");
    let version = profile.merged_with(&game).unwrap();
    let plan = build_launch_plan(&version, &platform(Os::Windows), &context()).unwrap();
    assert_fully_expanded(&plan);
    // The overlay's own main class wins, and its library entries land on the
    // classpath after the client jar.
    assert_eq!(
        plan.main_class,
        "net.fabricmc.loader.impl.launch.knot.KnotClient"
    );
    assert!(
        plan.classpath
            .iter()
            .any(|p| p.to_string_lossy().contains("asm")),
        "loader library missing from classpath: {:?}",
        plan.classpath
    );
    assert!(
        plan.jvm_args.iter().any(|a| a.contains("FabricMcEmu")),
        "loader JVM arg missing: {:?}",
        plan.jvm_args
    );
}

#[test]
fn game_arguments_carry_the_identity_values() {
    let version = version("version-26.3.json");
    let plan = build_launch_plan(&version, &platform(Os::Linux), &context()).unwrap();
    let value = |flag: &str| {
        let at = plan
            .game_args
            .iter()
            .position(|a| a == flag)
            .unwrap_or_else(|| panic!("{flag} missing from {:?}", plan.game_args));
        plan.game_args[at + 1].clone()
    };
    assert_eq!(value("--username"), "Steve");
    assert_eq!(value("--uuid"), "uuid-1");
    assert_eq!(value("--accessToken"), "token-1");
    assert_eq!(value("--version"), "26.3");
}

#[test]
fn one_file_appears_on_the_classpath_once() {
    // Documents repeat entries -- overlays re-list their game's libraries,
    // sometimes the same coordinate twice -- and the same file twice on
    // the classpath is one too many: the game's own bootstrap reads the
    // list as a set of jars and refuses a duplicate.
    let json = r#"{"id": "dup", "type": "release", "mainClass": "game.Main",
        "time": "t", "releaseTime": "r",
        "downloads": {"client": {"sha1": "00", "size": 1,
            "url": "http://host/client.jar"}},
        "libraries": [
            {"name": "com.example:dup:1.0",
             "downloads": {"artifact": {"path": "com/example/dup/1.0/dup-1.0.jar",
                "sha1": "11", "size": 1, "url": "http://host/dup.jar"}}},
            {"name": "com.example:dup:1.0",
             "downloads": {"artifact": {"path": "com/example/dup/1.0/dup-1.0.jar",
                "sha1": "11", "size": 1, "url": "http://host/dup.jar"}}}
        ]}"#;
    let version = Version::parse(json).unwrap();
    let plan = build_launch_plan(&version, &platform(Os::Linux), &context()).unwrap();
    let repeated: Vec<&PathBuf> = plan
        .classpath
        .iter()
        .filter(|p| p.to_string_lossy().contains("dup-1.0.jar"))
        .collect();
    assert_eq!(
        repeated.len(),
        1,
        "the repeated jar landed twice: {:?}",
        plan.classpath
    );
}
