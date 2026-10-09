//! The Forge-family installer documents, against the real ones.
//!
//! Both fixtures are extracted from the vendors' own installer jars
//! (`THIRD_PARTY_NOTICES.md`): Forge 1.20.1-47.4.26 and NeoForge
//! 20.6.141. They carry token shapes no synthetic document would
//! invent -- `'quoted literals'`, `:classifier@ext` artifact paths,
//! per-side data -- so the expansion rules are checked against exactly
//! those.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use palantir_loader::installer::{
    InstallProfile, PlannedProcessor, Processor, ProcessorContext, Side, plan_processors,
};

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

fn forge() -> InstallProfile {
    InstallProfile::parse(&fixture("forge-1.20.1-47.4.26-install-profile.json")).unwrap()
}

/// A vendor installer jar in miniature: the two packaged data files the
/// real documents' `BINPATCH` marker names. Planning an absolute data
/// value stages the entry out of the installer jar, so the context's jar
/// has to be a real one.
fn installer_jar() -> PathBuf {
    static JAR: OnceLock<PathBuf> = OnceLock::new();
    JAR.get_or_init(|| {
        let dir =
            std::env::temp_dir().join(format!("palantirmc-installer-jar-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let jar = dir.join("forge-installer.jar");
        let file = std::fs::File::create(&jar).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("data/client.lzma", options).unwrap();
        zip.write_all(b"client patch bytes").unwrap();
        zip.start_file("data/server.lzma", options).unwrap();
        zip.write_all(b"server patch bytes").unwrap();
        zip.finish().unwrap();
        jar
    })
    .clone()
}

/// The four context paths, as the real document will see them (its jar
/// standing in as the installer).
fn paths() -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    (
        PathBuf::from("/root"),
        PathBuf::from("/root/libraries"),
        PathBuf::from("/root/versions/1.20.1/1.20.1.jar"),
        installer_jar(),
    )
}

/// Path equality, not string equality: the expansion emits this
/// platform's separators, and the same file must compare equal however
/// the platform spells it.
fn same_path(planned: &str, expected: &str) -> bool {
    Path::new(planned) == Path::new(expected)
}

fn plan_for(profile: &InstallProfile, side: Side) -> Vec<PlannedProcessor> {
    let (root, library_dir, minecraft_jar, installer) = paths();
    let context = ProcessorContext {
        root: &root,
        library_dir: &library_dir,
        minecraft_jar: &minecraft_jar,
        installer: &installer,
        side,
    };
    plan_processors(profile, &context).unwrap()
}

#[test]
fn the_real_forge_profile_parses() {
    let profile = forge();
    assert_eq!(profile.spec, 1);
    assert_eq!(profile.minecraft, "1.20.1");
    assert_eq!(profile.version, "1.20.1-forge-47.4.26");
    assert_eq!(profile.json.as_deref(), Some("/version.json"));
    assert_eq!(profile.libraries.len(), 36);
    assert_eq!(profile.processors.len(), 10);
    // The data markers the whole pipeline is built on.
    assert_eq!(
        profile.data.get("PATCHED").unwrap().client.as_deref(),
        Some("[net.minecraftforge:forge:1.20.1-47.4.26:client]")
    );
    // MCP_VERSION is a quoted literal, not a path.
    assert_eq!(
        profile.data.get("MCP_VERSION").unwrap().client.as_deref(),
        Some("'20230612.114412'")
    );
}

#[test]
fn the_real_neoforge_profile_parses() {
    let profile =
        InstallProfile::parse(&fixture("neoforge-20.6.141-install-profile.json")).unwrap();
    assert_eq!(profile.spec, 1);
    assert_eq!(profile.minecraft, "1.20.6");
    assert_eq!(profile.version, "neoforge-20.6.141");
    assert_eq!(profile.json.as_deref(), Some("/version.json"));
    assert_eq!(profile.processors.len(), 10);
    assert_eq!(profile.libraries.len(), 89);
    assert!(profile.data.contains_key("PATCHED"));
}

#[test]
fn the_client_plan_skips_server_only_processors() {
    let profile = forge();
    let plan = plan_for(&profile, Side::Client);
    // As published: 10 processors -- 4 server-only, 5 both sides, 1
    // client-only -- so the client runs 6 of them.
    assert_eq!(plan.len(), 6);
    assert!(
        !plan
            .iter()
            .any(|p| p.args.iter().any(|a| a == "EXTRACT_FILES")),
        "the server-only EXTRACT_FILES processor leaked into the client plan"
    );
    let server_plan = plan_for(&profile, Side::Server);
    assert_eq!(server_plan.len(), 9, "4 server-only + 5 both");
}

#[test]
fn tokens_expand_to_the_real_paths() {
    let profile = forge();
    let plan = plan_for(&profile, Side::Client);
    let all: Vec<&String> = plan.iter().flat_map(|p| p.args.iter()).collect();

    // `[coord@ext]` is the artifact's Maven path under the library root.
    let mappings = "/root/libraries/de/oceanlabs/mcp/mcp_config/1.20.1-20230612.114412/mcp_config-1.20.1-20230612.114412-mappings.txt";
    assert!(
        all.iter().any(|arg| same_path(arg, mappings)),
        "{mappings} not planned"
    );

    // A data marker expands to its side's value: PATCHED is the artifact
    // the pipeline produces.
    let patched =
        "/root/libraries/net/minecraftforge/forge/1.20.1-47.4.26/forge-1.20.1-47.4.26-client.jar";
    assert!(
        all.iter().any(|arg| same_path(arg, patched)),
        "{patched} not planned"
    );
    // Built-ins as the real args use them: the game jar and the side name
    // stand alone; the BINPATCH marker names a file the installer jar
    // packages, so the plan points at the staged copy -- a path that
    // exists -- not at the vendor build tree's own `/data/client.lzma`.
    assert!(
        all.iter()
            .any(|arg| same_path(arg, "/root/versions/1.20.1/1.20.1.jar")),
        "MINECRAFT_JAR not planned"
    );
    assert!(all.contains(&&"client".to_string()), "SIDE not planned");
    let staged = installer_jar()
        .parent()
        .unwrap()
        .join("installer-data")
        .join("data")
        .join("client.lzma");
    assert!(
        all.iter()
            .any(|arg| same_path(arg, &staged.display().to_string())),
        "the packaged BINPATCH file was not staged to {}",
        staged.display()
    );
    assert!(staged.is_file(), "the staged file was not written");

    // And the skip receipts: the processor's output artifacts with their
    // promised hashes, both expanded. It produces two, slim and extra.
    let with_outputs = plan.iter().find(|p| !p.outputs.is_empty()).unwrap();
    let mut endings: Vec<String> = with_outputs
        .outputs
        .iter()
        .map(|(file, hash)| {
            assert_eq!(hash.len(), 40, "{hash}");
            file.to_string_lossy().into_owned()
        })
        .collect();
    endings.sort();
    assert_eq!(endings.len(), 2);
    assert!(endings[0].ends_with("-extra.jar"), "{}", endings[0]);
    assert!(endings[1].ends_with("-slim.jar"), "{}", endings[1]);
}

#[test]
fn literal_forms_expand_against_the_real_markers() {
    // The quoted and embedded forms appear in the real documents' data and
    // server-only args; run them through the real markers as a client
    // plan to pin each form.
    let mut profile = forge();
    profile.processors = vec![Processor {
        sides: None,
        jar: "a:b:1".into(),
        classpath: Vec::new(),
        args: vec![
            "{MCP_VERSION}".into(), // 'quoted literal'
            "{ROOT}/run.sh".into(), // embedded in a larger string
            "{INSTALLER}".into(),   // built-in
            "{PATCHED}".into(),     // marker -> bracketed artifact
        ],
        outputs: None,
    }];
    let plan = plan_for(&profile, Side::Client);
    let installer = installer_jar().display().to_string();
    let expected = [
        "20230612.114412",
        "/root/run.sh",
        installer.as_str(),
        "/root/libraries/net/minecraftforge/forge/1.20.1-47.4.26/forge-1.20.1-47.4.26-client.jar",
    ];
    assert_eq!(plan[0].args.len(), expected.len());
    for (planned, expected) in plan[0].args.iter().zip(expected) {
        assert!(
            same_path(planned, expected),
            "{planned} planned, {expected} expected"
        );
    }
}

#[test]
fn an_unknown_token_fails_naming_it() {
    let profile = InstallProfile::parse(
        r#"{"spec": 1, "minecraft": "1.20.1", "version": "x",
          "processors": [{"jar": "a:b:1", "args": ["{NOPE}"]}]}"#,
    )
    .unwrap();
    let (root, library_dir, minecraft_jar, installer) = paths();
    let error = plan_processors(
        &profile,
        &ProcessorContext {
            root: &root,
            library_dir: &library_dir,
            minecraft_jar: &minecraft_jar,
            installer: &installer,
            side: Side::Client,
        },
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("NOPE"), "{error}");
}

#[test]
fn omitted_sides_mean_both() {
    let both = Processor {
        sides: None,
        jar: "a:b:1".into(),
        classpath: Vec::new(),
        args: Vec::new(),
        outputs: None,
    };
    assert!(InstallProfile::runs_on(&both, Side::Client));
    assert!(InstallProfile::runs_on(&both, Side::Server));
    let server = Processor {
        sides: Some(vec!["server".into()]),
        ..both.clone()
    };
    assert!(!InstallProfile::runs_on(&server, Side::Client));
    assert!(InstallProfile::runs_on(&server, Side::Server));
}
