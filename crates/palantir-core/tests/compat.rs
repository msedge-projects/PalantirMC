//! Golden-file compatibility tests (VERIFY COMPAT).
//!
//! Fixtures reproduce the exact bytes Prism Launcher writes; each test
//! proves that palantir-core loads them and re-serializes them unchanged (or
//! produces byte-identical output from the same inputs).

use palantir_core::instance::{groups::Groups, Instance};
use palantir_core::json;
use palantir_core::launch;
use palantir_core::pack::{Component, PackProfile, Require};
use palantir_core::paths::PalantirPaths;
use palantir_core::resolve::{resolve, OfflineMetaStore};
use palantir_core::settings::Settings;
use palantir_core::version::{ProblemSeverity, RuntimeContext};
use std::collections::BTreeMap;

/// A Prism-written instance.cfg for a Fabric instance (QSettings IniFormat:
/// case-insensitively sorted keys, ConfigVersion injected, quoted values
/// for specials).
const INSTANCE_CFG_GOLDEN: &str = "[General]\nConfigVersion=1.3\niconKey=default\nInstanceType=OneSix\nlastLaunchTime=1735689600123\nlinkedInstances=[]\nMaxMemAlloc=8192\nname=Fabric 1.21.1\nnotes=\"multi line\\nwith ; and = chars\"\nOverrideMemory=true\ntotalTimePlayed=3600\nuuid=550e8400e29b41d4b716446655440000\n";

/// A Prism-written mmc-pack.json (QJsonDocument::Indented: 4-space indent,
/// alphabetical keys, trailing newline).
const MMC_PACK_GOLDEN: &str = concat!(
    "{\n",
    "    \"components\": [\n",
    "        {\n",
    "            \"cachedName\": \"Minecraft\",\n",
    "            \"cachedVersion\": \"1.21.1\",\n",
    "            \"important\": true,\n",
    "            \"uid\": \"net.minecraft\"\n",
    "        },\n",
    "        {\n",
    "            \"cachedName\": \"Fabric Loader\",\n",
    "            \"cachedVersion\": \"0.16.5\",\n",
    "            \"important\": true,\n",
    "            \"uid\": \"net.fabricmc.fabric-loader\"\n",
    "        }\n",
    "    ],\n",
    "    \"formatVersion\": 1\n",
    "}\n"
);

#[test]
fn instance_cfg_reproduces_prism_bytes_from_the_same_settings() {
    let tmp = tempfile::tempdir().unwrap();
    let mut s = Settings::empty(tmp.path().join("instance.cfg"));
    s.set_str("name", "Fabric 1.21.1");
    s.set_str("iconKey", "default");
    s.set_str("InstanceType", "OneSix");
    s.set_str("uuid", "550e8400e29b41d4b716446655440000");
    s.set_i64("lastLaunchTime", 1735689600123);
    s.set_i64("totalTimePlayed", 3600);
    s.set_i64("MaxMemAlloc", 8192);
    s.set_bool("OverrideMemory", true);
    s.set_str("linkedInstances", "[]");
    s.set_str("notes", "multi line\nwith ; and = chars");
    s.save().unwrap();
    let written = std::fs::read_to_string(tmp.path().join("instance.cfg")).unwrap();
    assert_eq!(written, INSTANCE_CFG_GOLDEN);

    // and loading the golden file gives the same settings back
    let loaded = Settings::load(&tmp.path().join("instance.cfg")).unwrap();
    assert_eq!(loaded.get_str("name", ""), "Fabric 1.21.1");
    assert_eq!(loaded.get_str("notes", ""), "multi line\nwith ; and = chars");
    assert_eq!(loaded.get_i64("MaxMemAlloc", 0), 8192);
    assert!(loaded.get_bool("OverrideMemory", false));
}

#[test]
fn mmc_pack_reproduces_prism_bytes_and_parses_the_golden() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("mmc-pack.json");
    std::fs::write(&path, MMC_PACK_GOLDEN).unwrap();

    let profile = PackProfile::load(&path).unwrap();
    assert_eq!(profile.components().len(), 2);
    assert_eq!(profile.get("net.minecraft").unwrap().cached_version, "1.21.1");
    assert_eq!(profile.get("net.fabricmc.fabric-loader").unwrap().cached_name, "Fabric Loader");
    assert_eq!(profile.to_text(), MMC_PACK_GOLDEN);
}

#[test]
fn legacy_multimc_instance_cfg_migrates_like_prism() {
    // Old MultiMC format: '#'-comments, \n escapes, no ConfigVersion.
    let legacy = "name=Old MultiMC\nnotes=line1\\nline2 with \\# hash and ; and =\nInstanceType=OneSix\n";
    let map = palantir_core::ini::load_ini(legacy, std::path::Path::new("instance.cfg")).unwrap();
    assert_eq!(map.get("ConfigVersion"), Some("1.3"));
    assert_eq!(map.get("name"), Some("Old MultiMC"));
    assert_eq!(map.get("notes"), Some("line1\nline2 with # hash and ; and ="));
    // and it re-saves in the modern format
    let rewritten = palantir_core::ini::save_ini(&map);
    assert!(rewritten.starts_with("[General]\nConfigVersion=1.3\n"));
    assert!(rewritten.contains("InstanceType=OneSix\n"));
    assert!(rewritten.contains("notes=\"line1\\nline2 with # hash and ; and =\"\n"));
}

#[test]
fn full_instance_fixture_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let inst_dir = tmp.path().join("instances");
    let mut instance = Instance::create(&inst_dir, "Golden", "1.21.1").unwrap();
    instance.set_notes("created by palantir-core");
    instance.add_play_time_secs(42);
    instance.save().unwrap();

    let reopened = Instance::open(&instance.root()).unwrap();
    assert_eq!(reopened.name(), "Golden");
    assert_eq!(reopened.notes(), "created by palantir-core");
    assert_eq!(reopened.total_time_played_secs(), 42);
    assert_eq!(reopened.instance_type(), "OneSix");
    assert!(reopened.mmc_pack_path().is_file());

    // settings survive a full save/load cycle
    let mut settings = reopened.settings().clone();
    settings.set_str("extra", "value");
    settings.save().unwrap();
    let again = Instance::open(instance.root()).unwrap();
    assert_eq!(again.settings().get_str("extra", ""), "value");
}

#[test]
fn groups_file_matches_prism_layout_at_data_root() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = PalantirPaths::at(tmp.path());
    std::fs::create_dir_all(paths.instances_dir()).unwrap();
    let mut groups = Groups::load(&paths);
    groups.set_group("inst", Some("Modded"));
    groups.save(&paths).unwrap();
    let text = std::fs::read_to_string(paths.groups_file()).unwrap();
    assert!(text.contains("\"formatVersion\": \"1\""));
    let back = Groups::load(&paths);
    assert_eq!(back.group_of("inst"), Some("Modded"));
}

/// Meta cache fixtures for the Fabric resolution test.
fn seed_meta_cache(meta_dir: &std::path::Path) {
    std::fs::create_dir_all(meta_dir.join("net.minecraft")).unwrap();
    std::fs::create_dir_all(meta_dir.join("net.fabricmc.fabric-loader")).unwrap();
    std::fs::create_dir_all(meta_dir.join("net.fabricmc.intermediary")).unwrap();
    std::fs::write(
        meta_dir.join("net.minecraft").join("1.21.1.json"),
        json::to_document_string(&serde_json::json!({
            "formatVersion": 1,
            "uid": "net.minecraft",
            "version": "1.21.1",
            "order": 0,
            "name": "Minecraft",
            "type": "release",
            "assets": "17",
            "mainClass": "net.minecraft.client.main.Main",
            "minecraftArguments": "--username ${auth_player_name} --version ${version_name}",
            // The real file names the LWJGL slot the pack never versions.
            "requires": [{"uid": "org.lwjgl3", "suggests": "3.3.3"}]
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::write(
        meta_dir.join("net.fabricmc.fabric-loader").join("0.16.5.json"),
        json::to_document_string(&serde_json::json!({
            "formatVersion": 1,
            "uid": "net.fabricmc.fabric-loader",
            "version": "0.16.5",
            "order": 1,
            "name": "Fabric Loader",
            "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "+traits": ["fabric"],
            "+jvmArgs": ["-Dfabric.gameJar=minecraft.jar"],
            // The real loader requires the mappings and names no version.
            "requires": [
                {"uid": "net.minecraft", "equals": "1.21.1"},
                {"uid": "net.fabricmc.intermediary"}
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::write(
        meta_dir.join("net.fabricmc.intermediary").join("1.21.1.json"),
        json::to_document_string(&serde_json::json!({
            "formatVersion": 1,
            "uid": "net.fabricmc.intermediary",
            "version": "1.21.1",
            "order": 11,
            "name": "Intermediary Mappings",
            "+traits": ["intermediary", "fabric"],
            "requires": [{"uid": "net.minecraft", "equals": "1.21.1"}]
        }))
        .unwrap(),
    )
    .unwrap();
    // The LWJGL slot: seeded, never versioned in `mmc-pack.json` -- exactly the
    // shape that used to make resolution fail.
    std::fs::create_dir_all(meta_dir.join("org.lwjgl3")).unwrap();
    std::fs::write(
        meta_dir.join("org.lwjgl3").join("3.3.3.json"),
        json::to_document_string(&serde_json::json!({
            "formatVersion": 1,
            "uid": "org.lwjgl3",
            "version": "3.3.3",
            "order": 2,
            "name": "LWJGL3"
        }))
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn resolve_fabric_instance_end_to_end() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = PalantirPaths::at(tmp.path());
    paths.ensure_layout().unwrap();
    seed_meta_cache(&paths.meta_dir());

    let instance = Instance::create(&paths.instances_dir(), "Fabric 1.21.1", "1.21.1").unwrap();
    let mut profile = PackProfile::load(&instance.mmc_pack_path()).unwrap();
    profile.set_version("net.fabricmc.fabric-loader", "0.16.5", true);
    // The LWJGL slot a profile written by Prism -- or by a build of this launcher
    // from before it read piston -- carries beside `net.minecraft`: no version, to
    // be filled from the `requires` the mirror-shaped fixture below publishes.
    // Neither launcher writes it against Mojang's own file, which keeps those
    // libraries itself, so it is added here rather than inherited from
    // `Instance::create`.
    profile.append(Component { uid: "org.lwjgl3".into(), important: true, ..Default::default() });
    profile.save(&instance.mmc_pack_path()).unwrap();

    let mut store = OfflineMetaStore::new(paths.meta_dir());
    let ctx = RuntimeContext::current_host();
    let saved = PackProfile::load(&instance.mmc_pack_path()).unwrap();
    // The `org.lwjgl3` slot carries no version. Leaving it that way is the point:
    // the dependency machinery has to fill it from `net.minecraft`'s requirement,
    // or the instance cannot start.
    assert!(
        saved
            .components()
            .iter()
            .any(|c| c.uid == "org.lwjgl3" && c.version.is_empty()),
        "the fixture stopped covering the versionless LWJGL slot"
    );
    let resolution = resolve(&saved, &instance.patches_dir(), &mut store, &ctx).unwrap();
    assert_eq!(resolution.severity(), ProblemSeverity::None, "problems: {:?}", resolution.problems);
    assert_eq!(resolution.profile.minecraft_version, "1.21.1");
    assert_eq!(resolution.profile.main_class, "net.fabricmc.loader.impl.launch.knot.KnotClient");
    assert!(resolution.profile.has_trait("fabric"));
    assert_eq!(resolution.profile.addn_jvm_arguments, vec!["-Dfabric.gameJar=minecraft.jar"]);

    // Both slots the pack does not version were decided during resolution: the
    // LWJGL slot from the requirement that named it, and the mappings from the
    // game version, added because the loader Requires them.
    let lwjgl = resolution.components.iter().find(|c| c.uid == "org.lwjgl3").unwrap();
    assert_eq!(lwjgl.version, "3.3.3");
    let mappings = resolution
        .components
        .iter()
        .find(|c| c.uid == "net.fabricmc.intermediary")
        .expect("the mappings dependency was never resolved");
    assert_eq!(mappings.version, "1.21.1");
    assert!(resolution.profile.has_trait("intermediary"));

    // The Fabric loader's `equals` requirement is satisfied by net.minecraft.
    assert!(!resolution.problems.iter().any(|p| p.message.contains("requires")));
}

#[test]
fn launch_script_golden_matches_protocol() {
    let profile = palantir_core::version::LaunchProfile {
        minecraft_version: "1.21.1".into(),
        minecraft_version_type: "release".into(),
        minecraft_assets: Some(palantir_core::version::AssetIndexInfo::bare("17")),
        minecraft_arguments: "--username ${auth_player_name} --version ${version_name} --assetsDir ${assets_root}".into(),
        main_class: "net.fabricmc.loader.impl.launch.knot.KnotClient".into(),
        ..Default::default()
    };
    let mut vars: BTreeMap<String, String> = BTreeMap::new();
    vars.insert("version_name".into(), "1.21.1".into());
    vars.insert("assets_root".into(), "/data/assets".into());
    vars.insert("auth_player_name".into(), "Player".into());
    let mc_args = launch::process_minecraft_args(&profile, None, None, &vars);
    let script = launch::create_launch_script(
        &profile,
        None,
        None,
        &mc_args,
        launch::WindowParams { width: 854, height: 480, maximized: false },
        palantir_core::PRODUCT_NAME,
        "9.0",
        "Fabric 1.21.1",
    );
    let expected = concat!(
        "mainClass net.fabricmc.loader.impl.launch.knot.KnotClient\n",
        "param --username\n",
        "param Player\n",
        "param --version\n",
        "param 1.21.1\n",
        "param --assetsDir\n",
        "param /data/assets\n",
        "windowTitle PalantirMC: Fabric 1.21.1\n",
        "windowParams 854x480\n",
        "launcherBrand PalantirMC\n",
        "launcherVersion 9.0\n",
        "instanceName Fabric 1.21.1\n",
        "instanceIconKey Fabric 1.21.1\n",
        "instanceIconPath icon.png\n",
        "launcher standard\n"
    );
    assert_eq!(script, expected);
}

#[test]
fn require_serialization_uses_equals_key_like_meta_format() {
    // Meta::serializeRequires writes "equals" (meta/JsonFormat.cpp), even
    // though the C++ field is named equalsVersion.
    let r = Require { uid: "net.minecraft".into(), equals_version: "1.21.1".into(), suggests: String::new() };
    let v = r.to_json();
    assert_eq!(v["equals"], "1.21.1");
    assert!(v.get("suggests").is_none());
    let back = Require::from_json(&v).unwrap();
    assert_eq!(back, r);
}
