//! `prism-cli` — command-line interface for prism-core.
//!
//! Subcommands:
//! * `dump-instance <dir>` — print a JSON summary of an instance.
//! * `create <instances-dir> <name> <mcversion>` — scaffold an instance.
//! * `launch-script <dir> [--meta <cache/meta>]` — print the launch script
//!   (offline; uses the metadata cache).
//! * `verify <dir>` — re-serialize `instance.cfg` and `mmc-pack.json` and
//!   byte-diff against the originals (the drop-in compat proof).
//! * `resolve-online <instance-dir> [--meta-url URL] [--cache DIR]` —
//!   resolve components through [`prism_net::OnlineMetaStore`] (disk
//!   write-through cache, network on miss) and print severity/problems.
//! * `download <url> <dest> [--sha256 HEX]` — fetch a URL to a file via
//!   `prism-net`, optionally verifying its SHA-256.
//! * `import-pack <zip> <instances-dir> <name>` — auto-detect a Modrinth
//!   (`.mrpack`) or CurseForge modpack zip via `prism-loader` and scaffold
//!   an instance from it (fully offline: remote files are not downloaded).
//!
//! Argument parsing is hand-rolled (no clap), like the original verify
//! harness this binary was ported from. Errors use `anyhow` at the binary
//! boundary, per the phase plan.

use anyhow::{bail, Context, Result};
use prism_core::{
    instance::{groups::Groups, Instance},
    java::JavaVersion,
    launch,
    pack::PackProfile,
    paths::PrismPaths,
    resolve::resolve,
    version::RuntimeContext,
};
use prism_loader::{detect_format, import_curseforge, import_mrpack, PackFormat};
use prism_net::{download_file, verify_sha256, OnlineMetaStore, DEFAULT_META_BASE_URL};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Per-request network timeout for `download`.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);

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
        Some("resolve-online") => resolve_online(
            &arg(&args, 1)?,
            flag_value(&args, "--meta-url"),
            flag_value(&args, "--cache"),
        ),
        Some("download") => download(&arg(&args, 1)?, &arg(&args, 2)?, flag_value(&args, "--sha256")),
        Some("import-pack") => import_pack(&arg(&args, 1)?, &arg(&args, 2)?, arg(&args, 3)?.as_str()),
        Some(other) => {
            bail!("unknown subcommand '{other}' (expected dump-instance|create|launch-script|verify|resolve-online|download|import-pack)")
        }
        None => {
            bail!("usage: prism-cli <dump-instance|create|launch-script|verify|resolve-online|download|import-pack> ...");
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
    let mut store = prism_core::resolve::OfflineMetaStore::new(meta);
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

// ---- resolve-online -------------------------------------------------------

fn resolve_online(instance_dir: &str, meta_url: Option<String>, cache: Option<String>) -> Result<()> {
    let instance = Instance::open(Path::new(instance_dir)).context("opening instance")?;
    let profile = PackProfile::load(&instance.mmc_pack_path()).unwrap_or_default();
    let cache_dir = match cache {
        Some(c) => PathBuf::from(c),
        None => PrismPaths::detect().meta_dir(),
    };
    let base = meta_url.unwrap_or_else(|| DEFAULT_META_BASE_URL.to_string());
    let mut store = OnlineMetaStore::new(base, &cache_dir);
    let ctx = RuntimeContext::current_host();
    let resolution =
        resolve(&profile, &instance.patches_dir(), &mut store, &ctx).context("resolving components")?;
    for component in &resolution.components {
        println!(
            "component {} @ {} (disabled: {}, custom: {})",
            component.uid, component.version, component.disabled, component.custom
        );
        for problem in &component.problems {
            println!("  problem [{:?}]: {}", problem.severity, problem.message);
        }
    }
    println!("severity: {:?}", resolution.severity());
    for problem in &resolution.problems {
        println!("problem: {}", problem.message);
    }
    if resolution.severity() == prism_core::version::ProblemSeverity::Error {
        bail!("resolution failed with errors");
    }
    println!("resolve-online: OK");
    Ok(())
}

// ---- download -------------------------------------------------------------

fn download(url: &str, dest: &str, sha256: Option<String>) -> Result<()> {
    let dest_path = PathBuf::from(dest);
    let bytes = download_file(url, &dest_path, DOWNLOAD_TIMEOUT)
        .with_context(|| format!("downloading {url}"))?;
    if let Some(hex) = sha256 {
        verify_sha256(&dest_path, &hex).with_context(|| format!("verifying {dest}"))?;
        println!("sha256: OK");
    }
    println!("downloaded {url} -> {dest} ({bytes} bytes)");
    Ok(())
}

// ---- import-pack ----------------------------------------------------------

fn import_pack(zip: &str, instances_dir: &str, name: &str) -> Result<()> {
    let archive = Path::new(zip);
    let bytes = std::fs::read(archive).with_context(|| format!("reading {}", archive.display()))?;
    let root = match detect_format(&bytes) {
        PackFormat::MrPack => import_mrpack(&bytes, instances_dir, name).context("importing mrpack")?,
        PackFormat::CurseForge => {
            import_curseforge(&bytes, instances_dir, name).context("importing curseforge pack")?
        }
        PackFormat::Unknown => {
            bail!("unknown modpack format for {} (no modrinth.index.json or manifest.json)", archive.display())
        }
    };
    println!("imported pack '{name}' at {}", root.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal CRC-32 (IEEE) for building test zips with valid checksums.
    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for &byte in data {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                if crc & 1 == 1 {
                    crc = (crc >> 1) ^ 0xEDB8_8320;
                } else {
                    crc >>= 1;
                }
            }
        }
        !crc
    }

    /// Build a minimal stored-entry zip from `(name, contents)` pairs.
    fn minimal_zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut data: Vec<u8> = Vec::new();
        let mut central: Vec<u8> = Vec::new();
        for (name, contents) in entries {
            let local_offset = data.len() as u32;
            let crc = crc32(contents);
            let size = contents.len() as u32;
            data.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
            data.extend_from_slice(&20u16.to_le_bytes());
            data.extend_from_slice(&0u16.to_le_bytes());
            data.extend_from_slice(&0u16.to_le_bytes()); // stored
            data.extend_from_slice(&0u16.to_le_bytes());
            data.extend_from_slice(&0u16.to_le_bytes());
            data.extend_from_slice(&crc.to_le_bytes());
            data.extend_from_slice(&size.to_le_bytes());
            data.extend_from_slice(&size.to_le_bytes());
            data.extend_from_slice(&(name.len() as u16).to_le_bytes());
            data.extend_from_slice(&0u16.to_le_bytes());
            data.extend_from_slice(name.as_bytes());
            data.extend_from_slice(contents);
            central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
            central.extend_from_slice(&20u16.to_le_bytes());
            central.extend_from_slice(&20u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&crc.to_le_bytes());
            central.extend_from_slice(&size.to_le_bytes());
            central.extend_from_slice(&size.to_le_bytes());
            central.extend_from_slice(&(name.len() as u16).to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u32.to_le_bytes());
            central.extend_from_slice(&local_offset.to_le_bytes());
            central.extend_from_slice(name.as_bytes());
        }
        let cd_offset = data.len() as u32;
        let cd_size = central.len() as u32;
        data.extend_from_slice(&central);
        data.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        data.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        data.extend_from_slice(&cd_size.to_le_bytes());
        data.extend_from_slice(&cd_offset.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data
    }

    fn temp_subdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("prism-cli-test-{}-{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn detect_format_covers_mrpack_curseforge_and_unknown() {
        let mrpack = minimal_zip_bytes(&[
            ("modrinth.index.json", br#"{"formatVersion": 1}"#),
            ("overrides/x.cfg", b"x"),
        ]);
        assert_eq!(detect_format(&mrpack), PackFormat::MrPack);
        let curse = minimal_zip_bytes(&[("manifest.json", br#"{}"#)]);
        assert_eq!(detect_format(&curse), PackFormat::CurseForge);
        // Modrinth wins when both markers are present.
        let both = minimal_zip_bytes(&[("manifest.json", br#"{}"#), ("modrinth.index.json", br#"{}"#)]);
        assert_eq!(detect_format(&both), PackFormat::MrPack);
        assert_eq!(detect_format(b"definitely not a zip"), PackFormat::Unknown);
    }

    #[test]
    fn import_mrpack_scaffolds_instance_offline() {
        let dir = temp_subdir("mrpack");
        let bytes = minimal_zip_bytes(&[
            (
                "modrinth.index.json",
                br#"{"formatVersion": 1, "game": "minecraft", "versionId": "x",
                     "name": "t", "dependencies": {"minecraft": "1.21.1", "fabric-loader": "0.16.14"}}"#,
            ),
            ("overrides/config/test.cfg", b"hello"),
        ]);
        let root = import_mrpack(&bytes, dir.join("instances"), "mr-test").unwrap();
        assert!(root.join("mmc-pack.json").is_file());
        let profile = PackProfile::load(&root.join("mmc-pack.json")).unwrap();
        assert!(profile.components().iter().any(|c| c.uid == "net.fabricmc.fabric-loader"
            && c.version == "0.16.14"));
        assert_eq!(std::fs::read(root.join("config/test.cfg")).unwrap(), b"hello");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn import_curseforge_scaffolds_instance_offline() {
        let dir = temp_subdir("curse");
        let bytes = minimal_zip_bytes(&[
            (
                "manifest.json",
                br#"{"manifestType": "minecraftModpack", "overrides": "overrides",
                     "minecraft": {"version": "1.20.1",
                      "modLoaders": [{"id": "forge-47.2.0", "primary": true}]}}"#,
            ),
            ("overrides/mods/a.jar", b"jar-bytes"),
        ]);
        let root = import_curseforge(&bytes, dir.join("instances"), "cf-test").unwrap();
        let profile = PackProfile::load(&root.join("mmc-pack.json")).unwrap();
        assert!(profile.components().iter().any(|c| c.uid == "net.minecraftforge"
            && c.version == "47.2.0"));
        assert!(root.join("mods/a.jar").is_file());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn verify_sha256_accepts_correct_digest_offline() {
        let dir = temp_subdir("sha");
        let path = dir.join("a.bin");
        std::fs::write(&path, b"abc").unwrap();
        verify_sha256(
            &path,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        )
        .unwrap();
        assert!(verify_sha256(&path, "00").is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
