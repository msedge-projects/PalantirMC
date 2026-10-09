//! The importers, against real packs.
//!
//! Every fixture is a document published by a pack or launcher author
//! (`THIRD_PARTY_NOTICES.md`): a Modrinth pack's own `modrinth.index.json`
//! and `.mrpack` zip, two Prism/MultiMC instances (Fabric and Forge
//! flavoured), a CurseForge export's `manifest.json`, and a vanilla
//! launcher's `launcher_profiles.json`. The shapes they carry are the
//! ones no synthetic document would invent: `env` values, `dependencyOnly`
//! components, catalogue ids, and a `gameDir` that points at someone's
//! actual home directory.

use palantir_loader::curseforge;
use palantir_loader::import::{FileSource, LoaderTarget, SideFilter, Support};
use palantir_loader::mrpack;
use palantir_loader::prism;
use palantir_loader::vanilla;

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

// ---------------------------------------------------------------- mrpack

#[test]
fn a_real_mrpack_index_imports() {
    let index =
        mrpack::MrpackIndex::parse(&fixture("sodiumplus-2.4.7-modrinth.index.json")).unwrap();
    let pack = mrpack::import(&index).unwrap();
    assert_eq!(pack.name, "2.4.7");
    assert_eq!(pack.game, "26.2");
    assert_eq!(
        pack.loader,
        LoaderTarget::Fabric {
            loader: "0.19.5".to_string()
        }
    );
    assert_eq!(pack.files.len(), 83);
    assert!(pack.unknown.is_empty(), "{:?}", pack.unknown);

    // The layers, base first: the side layers overwrite it by definition.
    let names: Vec<(&str, SideFilter)> = pack
        .overrides
        .iter()
        .map(|layer| (layer.prefix.as_str(), layer.side))
        .collect();
    assert_eq!(
        names,
        vec![
            ("overrides", SideFilter::Both),
            ("client-overrides", SideFilter::ClientOnly),
            ("server-overrides", SideFilter::ServerOnly),
        ]
    );

    // A client-required/server-unsupported file drops out of the server
    // view and stays in the client's.
    let client = pack.select(palantir_loader::installer::Side::Client);
    let server = pack.select(palantir_loader::installer::Side::Server);
    assert_eq!(client.required.len() + client.optional.len(), 83);
    assert!(server.required.len() + server.optional.len() < 83);

    // Downloads are https URLs with both hashes, as the format demands.
    for file in &pack.files {
        let FileSource::Download {
            urls,
            hashes,
            size: _,
        } = &file.source
        else {
            panic!("{:?} is not a download", file.path);
        };
        assert!(urls.iter().all(|u| u.starts_with("https://")));
        assert!(hashes.contains_key("sha1") && hashes.contains_key("sha512"));
    }
}

#[test]
fn an_optional_file_is_offered_never_installed() {
    let index = mrpack::MrpackIndex::parse(
        r#"{"formatVersion": 1, "game": "minecraft", "versionId": "x", "name": "x",
         "files": [{"path": "mods/x.jar", "downloads": ["https://e/x.jar"],
           "hashes": {"sha1": "a", "sha512": "b"},
           "env": {"client": "optional", "server": "unsupported"}}],
         "dependencies": {"minecraft": "1.20.1"}}"#,
    )
    .unwrap();
    let pack = mrpack::import(&index).unwrap();
    let client = pack.select(palantir_loader::installer::Side::Client);
    assert!(client.required.is_empty());
    assert_eq!(client.optional.len(), 1);
    let server = pack.select(palantir_loader::installer::Side::Server);
    assert!(server.required.is_empty() && server.optional.is_empty());
}

#[test]
fn a_path_that_escapes_the_game_directory_is_refused() {
    for evil in ["../evil.jar", "/etc/passwd", "C:\\evil.jar", "mods/../../x"] {
        let text = format!(
            r#"{{"formatVersion": 1, "game": "minecraft", "versionId": "x", "name": "x",
             "files": [{{"path": {evil:?}, "downloads": ["https://e/x"],
               "hashes": {{"sha1": "a", "sha512": "b"}}}}],
             "dependencies": {{"minecraft": "1.20.1"}}}}"#
        );
        let error = mrpack::MrpackIndex::parse(&text).unwrap_err().to_string();
        assert!(error.contains("escape"), "{evil}: {error}");
    }
}

#[test]
fn a_record_without_both_hashes_is_refused() {
    let text = r#"{"formatVersion": 1, "game": "minecraft", "versionId": "x", "name": "x",
     "files": [{"path": "mods/x.jar", "downloads": ["https://e/x"],
       "hashes": {"sha1": "a"}}],
     "dependencies": {"minecraft": "1.20.1"}}"#;
    let error = mrpack::MrpackIndex::parse(text).unwrap_err().to_string();
    assert!(error.contains("sha512"), "{error}");
}

#[test]
fn an_unknown_dependency_is_the_loader_not_vanilla() {
    let mut dependencies = std::collections::BTreeMap::new();
    dependencies.insert("minecraft".to_string(), "1.20.1".to_string());
    dependencies.insert("liteloader".to_string(), "1.2".to_string());
    let index = mrpack::MrpackIndex {
        format_version: 1,
        game: "minecraft".to_string(),
        version_id: "x".to_string(),
        name: "x".to_string(),
        summary: None,
        files: Vec::new(),
        dependencies,
    };
    let pack = mrpack::import(&index).unwrap();
    // Installing this as vanilla would be a silent wrong install.
    assert_eq!(
        pack.loader,
        LoaderTarget::Unknown {
            id: "liteloader".to_string(),
            version: "1.2".to_string()
        }
    );
    assert_eq!(
        pack.unknown.get("liteloader").map(String::as_str),
        Some("1.2")
    );
}

// ----------------------------------------------------------------- prism

#[test]
fn a_real_fabric_instance_imports() {
    let pack = prism::MmcPack::parse(&fixture("prism-fabulously-optimized-mmc-pack.json")).unwrap();
    let cfg = prism::InstanceCfg::parse(&fixture("prism-fabulously-optimized-instance.cfg"));
    let imported = prism::import(&pack, &cfg, "minecraft").unwrap();
    assert_eq!(imported.name, "Fabulously Optimized 15.0.0-alpha.5");
    assert_eq!(imported.game, "26.3");
    assert_eq!(
        imported.loader,
        LoaderTarget::Fabric {
            loader: "0.19.5".to_string()
        }
    );
    // lwjgl3 and intermediary are dependency-only: the game document and
    // the Fabric profile carry them, so an import must not claim them.
    assert!(imported.unknown.is_empty(), "{:?}", imported.unknown);
    assert_eq!(imported.overrides[0].prefix, "minecraft");
}

#[test]
fn a_real_forge_instance_imports() {
    let pack = prism::MmcPack::parse(&fixture("prism-gt-infinity-bakery-mmc-pack.json")).unwrap();
    let cfg = prism::InstanceCfg::default();
    let imported = prism::import(&pack, &cfg, "minecraft").unwrap();
    assert_eq!(imported.game, "1.20.1");
    assert_eq!(
        imported.loader,
        LoaderTarget::Forge {
            loader: "47.4.0".to_string()
        }
    );
    // No name in an empty instance.cfg: the game version names it.
    assert_eq!(imported.name, "1.20.1");
}

#[test]
fn an_unknown_component_is_the_loader_not_vanilla() {
    let pack = prism::MmcPack::parse(
        r#"{"formatVersion": 1, "components": [
      {"uid": "net.minecraft", "version": "1.20.1", "important": true},
      {"uid": "com.mumfrey.liteloader", "version": "1.7.2"}]}"#,
    )
    .unwrap();
    let imported = prism::import(&pack, &prism::InstanceCfg::default(), "minecraft").unwrap();
    assert_eq!(
        imported.loader,
        LoaderTarget::Unknown {
            id: "com.mumfrey.liteloader".to_string(),
            version: "1.7.2".to_string()
        }
    );
}

#[test]
fn an_instance_cfg_splits_on_the_first_equals() {
    let cfg = prism::InstanceCfg::parse(
        "[General]\nname=Pack with = in name\njavaArgs=-Xmx4G -Dfoo=bar\niconKey=pack\n",
    );
    assert_eq!(cfg.name.as_deref(), Some("Pack with = in name"));
    assert_eq!(cfg.icon_key.as_deref(), Some("pack"));
    assert_eq!(
        cfg.entries.get("javaArgs").map(String::as_str),
        Some("-Xmx4G -Dfoo=bar")
    );
}

// ------------------------------------------------------------- curseforge

#[test]
fn a_real_curseforge_manifest_imports() {
    let manifest =
        curseforge::Manifest::parse(&fixture("curseforge-fabulously-optimized-manifest.json"))
            .unwrap();
    let pack = curseforge::import(&manifest).unwrap();
    assert_eq!(pack.name, "Fabulously Optimized");
    assert_eq!(pack.game, "26.3");
    assert_eq!(
        pack.loader,
        LoaderTarget::Fabric {
            loader: "0.19.5".to_string()
        }
    );
    assert!(!pack.files.is_empty());
    assert_eq!(pack.overrides[0].prefix, "overrides");
    // Files are catalogue ids, not URLs: nothing is guessed before the
    // catalogue answers.
    for file in &pack.files {
        assert!(matches!(file.source, FileSource::Catalogue { .. }));
    }
}

#[test]
fn a_curseforge_loader_id_names_its_kind_and_version() {
    let text = r#"{"minecraft": {"version": "1.20.1",
      "modLoaders": [{"id": "forge-47.4.26", "primary": true}]},
     "manifestType": "minecraftModpack", "manifestVersion": 1,
     "name": "x", "files": []}"#;
    let manifest = curseforge::Manifest::parse(text).unwrap();
    let pack = curseforge::import(&manifest).unwrap();
    assert_eq!(
        pack.loader,
        LoaderTarget::Forge {
            loader: "47.4.26".to_string()
        }
    );
}

#[test]
fn a_not_modpack_manifest_is_refused_naming_it() {
    let text = r#"{"minecraft": {"version": "1.20.1"},
     "manifestType": "somethingElse", "manifestVersion": 1,
     "name": "x", "files": []}"#;
    let error = curseforge::Manifest::parse(text).unwrap_err().to_string();
    assert!(error.contains("somethingElse"), "{error}");
}

// --------------------------------------------------------------- vanilla

#[test]
fn a_real_launcher_profiles_file_imports() {
    let profiles =
        vanilla::LauncherProfiles::parse(&fixture("vanilla-launcher-profiles.json")).unwrap();
    assert_eq!(profiles.profiles.len(), 2);
    let (id, _) = profiles.selected().unwrap();
    assert_eq!(id, "MCPatcher");
    let migration = vanilla::migration(&profiles, "MCPatcher").unwrap();
    assert_eq!(migration.instance_name, "MCPatcher");
    assert_eq!(migration.game, "1.8-mcpatcher");
    // A profile's gameDir is where its worlds and options live; the
    // migration carries it as local content to copy.
    assert!(migration.game_dir.contains(".minecraft"));
    let pack = vanilla::import(&migration);
    assert_eq!(pack.game, "1.8-mcpatcher");
    assert!(matches!(pack.files[0].source, FileSource::Local { .. }));
}

#[test]
fn a_profile_with_no_version_is_refused_naming_it() {
    let profiles =
        vanilla::LauncherProfiles::parse(r#"{"profiles": {"p": {"name": "p", "gameDir": "/x"}}}"#)
            .unwrap();
    let error = vanilla::migration(&profiles, "p").unwrap_err().to_string();
    assert!(error.contains("lastVersionId"), "{error}");
    let error = vanilla::migration(&profiles, "missing")
        .unwrap_err()
        .to_string();
    assert!(error.contains("missing"), "{error}");
}

// ----------------------------------------------------------------- select

#[test]
fn optional_support_defaults_to_required() {
    // A file that says nothing about sides exists on both of them.
    let index = mrpack::MrpackIndex::parse(
        r#"{"formatVersion": 1, "game": "minecraft", "versionId": "x", "name": "x",
         "files": [{"path": "mods/x.jar", "downloads": ["https://e/x"],
           "hashes": {"sha1": "a", "sha512": "b"}}],
         "dependencies": {"minecraft": "1.20.1"}}"#,
    )
    .unwrap();
    let pack = mrpack::import(&index).unwrap();
    for side in [
        palantir_loader::installer::Side::Client,
        palantir_loader::installer::Side::Server,
    ] {
        let selection = pack.select(side);
        assert_eq!(selection.required.len(), 1);
        assert!(selection.optional.is_empty());
        assert_eq!(selection.required[0].env.client, Support::Required);
    }
}
