//! `prism-cli` — verification harness for prism-core (dev tool).
//!
//! Subcommands:
//! * `dump-instance <dir>` — print a JSON summary of an instance.
//! * `create <instances-dir> <name> <mcversion>` — scaffold an instance.
//! * `launch-script <dir> [--meta <cache/meta>]` — print the launch script
//!   (offline; uses the metadata cache).
//! * `verify <dir>` — re-serialize `instance.cfg` and `mmc-pack.json` and
//!   byte-diff against the originals (the drop-in compat proof).
//!
//! Errors use `anyhow` at the binary boundary, per the phase plan.

use anyhow::{bail, Context, Result};
use prism_core::{
    instance::{groups::Groups, Instance},
    java::JavaVersion,
    launch,
    pack::PackProfile,
    paths::PrismPaths,
    resolve::{resolve, OfflineMetaStore},
    version::RuntimeContext,
};
use std::path::{Path, PathBuf};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("dump-instance") => dump_instance(&arg(&args, 1)?),
        Some("create") => create(
            &arg(&args, 1)?,
            arg(&args, 2)?.as_str(),
            arg(&args, 3)?.as_str(),
        ),
        Some("launch-script") => launch_script(&arg(&args, 1)?, flag_value(&args, "--meta")),
        Some("verify") => verify(&arg(&args, 1)?),
        Some(other) => {
            bail!("unknown subcommand '{other}' (expected dump-instance|create|launch-script|verify)")
        }
        None => {
            bail!("usage: prism-cli <dump-instance|create|launch-script|verify> ...");
        }
    }
}

fn arg(args: &[String], index: usize) -> Result<String> {
    args.get(index).cloned().context(format!("missing argument {index}"))
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1).cloned())
}

fn dump_instance(dir: &String) -> Result<()> {
    let instance = Instance::open(Path::new(dir)).context("opening instance")?;
    let profile = PackProfile::load(&instance.mmc_pack_path()).unwrap_or_default();
    let summary = serde_json::json!({
        "id": instance.id(),
        "name": instance.name(),
        "iconKey": instance.icon_key(),
        "instanceType": instance.instance_type(),
        "uuid": instance.uuid(),
        "lastLaunchTime": instance.last_launch_millis(),
        "totalTimePlayed": instance.total_time_played_secs(),
        "gameRoot": instance.game_root(),
        "components": profile
            .components()
            .iter()
            .map(|c| {
                serde_json::json!({
                    "uid": c.uid,
                    "version": c.version,
                    "dependencyOnly": c.dependency_only,
                    "important": c.important,
                    "disabled": c.disabled,
                    "cachedVersion": c.cached_version,
                })
            })
            .collect::<Vec<_>>(),
        "linkedInstances": instance.linked_instances(),
    });
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

fn create(instances_dir: &str, name: &str, mc_version: &str) -> Result<()> {
    let instance = Instance::create(Path::new(instances_dir), name, mc_version)
        .context("creating instance")?;
    println!("created instance '{}' at {}", instance.name(), instance.root().display());
    Ok(())
}

fn launch_script(dir: &String, meta_dir: Option<String>) -> Result<()> {
    let instance = Instance::open(Path::new(dir)).context("opening instance")?;
    let meta = match meta_dir {
        Some(m) => PathBuf::from(m),
        None => {
            let paths = PrismPaths::detect();
            paths.meta_dir()
        }
    };
    let mut store = OfflineMetaStore::new(meta);
    let profile = PackProfile::load(&instance.mmc_pack_path()).unwrap_or_default();
    let ctx = RuntimeContext::current_host();
    let resolution =
        resolve(&profile, &instance.patches_dir(), &mut store, &ctx).context("resolving components")?;
    if resolution.severity() == prism_core::version::ProblemSeverity::Error {
        for p in &resolution.problems {
            eprintln!("problem: {}", p.message);
        }
        bail!("resolution failed with errors");
    }
    let mut vars = launch::profile_var_map(
        &resolution.profile,
        &instance.name(),
        &instance.id(),
        instance.root(),
        &instance.game_root(),
        &instance.game_root().join("resources"),
        &instance.root().join("assets"),
        &instance.root().join("libraries"),
    );
    vars.insert("version_name".into(), resolution.profile.minecraft_version.clone());
    let mc_args = launch::process_minecraft_args(&resolution.profile, None, None, &vars);
    print!(
        "{}",
        launch::create_launch_script(
            &resolution.profile,
            None,
            None,
            &mc_args,
            launch::WindowParams { width: 854, height: 480, maximized: false },
            "Prism Launcher",
            "9.0",
            &instance.name(),
        )
    );
    Ok(())
}

fn verify(dir: &String) -> Result<()> {
    let root = Path::new(dir);
    let instance = Instance::open(root).context("opening instance")?;
    let mut all_identical = true;

    // instance.cfg round trip: parse -> re-serialize -> compare.
    let cfg_path = root.join("instance.cfg");
    let original_cfg = std::fs::read(&cfg_path).context("reading instance.cfg")?;
    let settings = prism_core::settings::Settings::load(&cfg_path)?;
    let rewritten = prism_core::ini::save_ini(settings.map());
    if rewritten.as_bytes() == original_cfg.as_slice() {
        println!("instance.cfg: byte-identical after round trip");
    } else {
        all_identical = false;
        println!("instance.cfg: DIFFERS after round trip");
        print_diff(&String::from_utf8_lossy(&original_cfg), &rewritten);
    }

    // mmc-pack.json round trip.
    let pack_path = instance.mmc_pack_path();
    if pack_path.exists() {
        let original = std::fs::read(&pack_path).context("reading mmc-pack.json")?;
        let profile = PackProfile::load(&pack_path)?;
        let rewritten = profile.to_text();
        if rewritten.as_bytes() == original.as_slice() {
            println!("mmc-pack.json: byte-identical after round trip");
        } else {
            all_identical = false;
            println!("mmc-pack.json: DIFFERS after round trip");
            print_diff(&String::from_utf8_lossy(&original), &rewritten);
        }
    }

    // instgroups.json presence check at the data root.
    let paths = PrismPaths::detect();
    let groups = Groups::load(&paths);
    if let Some(group) = groups.group_of(&instance.id()) {
        println!("group: {group}");
    }

    if !all_identical {
        bail!("round trip produced differences (see above)");
    }
    println!("verify: OK");
    Ok(())
}

fn print_diff(original: &str, rewritten: &str) {
    for (i, (a, b)) in original.lines().zip(rewritten.lines()).enumerate() {
        if a != b {
            println!("  line {}: -{a:?}\n            +{b:?}", i + 1);
        }
    }
    let o_lines = original.lines().count();
    let r_lines = rewritten.lines().count();
    if o_lines != r_lines {
        println!("  line count: {o_lines} -> {r_lines}");
    }
}

/// Unused-but-documented helper showing how a resolved profile reports its
/// Java requirements (kept here so the CLI surfaces them in future versions).
#[allow(dead_code)]
fn java_major_for(resolved_java_version: &str) -> i64 {
    JavaVersion::parse(resolved_java_version).major()
}
