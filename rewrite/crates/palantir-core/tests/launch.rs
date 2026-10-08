//! Launch plans built from the real downloaded samples.
//!
//! The output is checked where the format is strangest: the 2026 argument
//! shape with its three lists, the pre-2018 single string with no JVM list
//! at all, and the platform rules that split one list across operating
//! systems. The invariant behind all of them: once a plan is built, no
//! placeholder may survive into the command.

use std::path::{Path, PathBuf};

use palantir_core::launch::{LaunchContext, LaunchPlan, build_launch_plan};
use palantir_core::rules::{Arch, Os, Platform};
use palantir_core::version::Version;

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
        user_type: "msa".to_string(),
        game_dir: PathBuf::from("/data/instances/a/game"),
        assets_root: PathBuf::from("/data/assets"),
        assets_index_name: "34".to_string(),
        version_name: "26.3".to_string(),
        version_type: "release".to_string(),
        natives_dir: PathBuf::from("/data/versions/26.3/natives"),
        launcher_name: "PalantirMC".to_string(),
        launcher_version: "2.0".to_string(),
        library_dir: PathBuf::from("/data/libraries"),
        classpath: vec![PathBuf::from("/data/libraries/a.jar")],
        ..LaunchContext::default()
    }
}

fn platform(os: Os) -> Platform {
    Platform::new(os, "10.0.19045", Arch::X86_64)
}

/// No placeholder may survive into a built plan.
fn assert_fully_expanded(plan: &LaunchPlan) {
    for argument in plan
        .default_jvm_args
        .iter()
        .chain(&plan.jvm_args)
        .chain(&plan.game_args)
    {
        assert!(
            !argument.contains("${"),
            "unexpanded placeholder: {argument}"
        );
    }
}

#[test]
fn modern_metadata_produces_a_complete_command() {
    let version = version("version-26.3.json");
    let plan = build_launch_plan(&version, &platform(Os::Windows), &context()).unwrap();
    assert_fully_expanded(&plan);

    // The tuning group is kept separate so settings can override it.
    assert!(plan.default_jvm_args.contains(&"-Xmx4G".to_string()));
    assert!(plan.default_jvm_args.contains(&"-XX:+UseZGC".to_string()));

    // Game arguments arrive in the format's order, expanded.
    assert_eq!(
        &plan.game_args[..4],
        ["--username", "Steve", "--version", "26.3"]
    );

    let command = plan.command(Path::new("C:\\java\\bin\\java.exe"));
    assert_eq!(command[0], "C:\\java\\bin\\java.exe");
    assert_eq!(command.last().unwrap(), plan.game_args.last().unwrap());
    assert_eq!(command[1], "-Xms2G"); // the tuning group's first flag
    assert_eq!(
        command[command.len() - plan.game_args.len() - 1],
        "net.minecraft.client.main.Main"
    );
}

#[test]
fn jvm_arguments_split_by_platform() {
    let version = version("version-26.3.json");
    let windows = build_launch_plan(&version, &platform(Os::Windows), &context()).unwrap();
    let mac = build_launch_plan(
        &version,
        &Platform::new(Os::MacOs, "14.0", Arch::X86_64),
        &context(),
    )
    .unwrap();
    assert!(
        windows
            .jvm_args
            .iter()
            .any(|a| a.starts_with("-XX:HeapDumpPath="))
    );
    assert!(
        !mac.jvm_args
            .iter()
            .any(|a| a.starts_with("-XX:HeapDumpPath="))
    );
    assert!(mac.jvm_args.contains(&"-XstartOnFirstThread".to_string()));
    assert!(
        !windows
            .jvm_args
            .contains(&"-XstartOnFirstThread".to_string())
    );
}

#[test]
fn legacy_metadata_gets_the_launcher_supplied_jvm_arguments() {
    let mut context = context();
    context.assets_index_name = "1.12".to_string();
    context.version_name = "1.12.2".to_string();
    let version = version("version-1.12.2.json");
    let plan = build_launch_plan(&version, &platform(Os::Windows), &context).unwrap();
    assert_fully_expanded(&plan);

    // The pre-2018 shape carries no JVM list: the launcher owes the natives
    // directory and the classpath, and nothing else.
    assert_eq!(
        plan.jvm_args[0],
        "-Djava.library.path=/data/versions/26.3/natives"
    );
    assert_eq!(plan.jvm_args[1], "-cp");
    assert!(plan.jvm_args[2].ends_with("a.jar"));

    // The old string splits on whitespace and its flags carry literal values.
    assert_eq!(&plan.game_args[..2], ["--username", "Steve"]);
    let index = plan
        .game_args
        .iter()
        .position(|a| a == "--assetIndex")
        .unwrap();
    assert_eq!(plan.game_args[index + 1], "1.12");
}

#[test]
fn the_oldest_sample_launches_the_same_way() {
    let version = version("version-1.5.2.json");
    let plan = build_launch_plan(&version, &platform(Os::Linux), &context()).unwrap();
    assert_fully_expanded(&plan);
    // 1.5.2 boots through LaunchWrapper, not the modern main class.
    assert_eq!(plan.main_class, "net.minecraft.launchwrapper.Launch");
    assert!(!plan.game_args.is_empty());
}
