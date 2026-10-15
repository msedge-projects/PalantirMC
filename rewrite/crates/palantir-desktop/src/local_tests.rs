use super::*;
use std::io::Read;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "palantir-local-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
    fn world(&self, bedrock: bool) -> PathBuf {
        let path = self
            .0
            .join(if bedrock { "minecraftWorlds" } else { "saves" })
            .join("world");
        std::fs::create_dir_all(path.join("db")).unwrap();
        std::fs::write(path.join("level.dat"), b"raw world metadata").unwrap();
        std::fs::write(path.join("db/000001.ldb"), b"opaque database bytes").unwrap();
        path
    }
    fn pack(&self, entries: &[(&str, &[u8])]) -> PathBuf {
        let path = self.0.join("sample.mcpack");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        for (name, bytes) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
const MANIFEST: &[u8] = br#"{"format_version":2,"header":{"name":"Original test pack","uuid":"11111111-2222-3333-4444-555555555555","version":[1,0,0]},"modules":[{"type":"resources","version":[1,0,0]}]}"#;
fn world_input(bedrock: bool, root: &Path) -> Input {
    Input::World {
        target: if bedrock {
            Target::Bedrock(Channel::Release)
        } else {
            Target::Java("collection".into())
        },
        folder: "world".into(),
        output: root
            .join(if bedrock { "copy.mcworld" } else { "copy.zip" })
            .display()
            .to_string(),
    }
}
fn pack_input(source: &Path, world: Option<&str>) -> Input {
    Input::Addition {
        target: Target::Bedrock(Channel::Release),
        kind: AdditionKind::BedrockPack,
        source: source.display().to_string(),
        world: world.map(str::to_string),
    }
}
#[test]
fn complete_world_copies_match_every_source_byte_for_both_editions() {
    for bedrock in [false, true] {
        let fixture = Fixture::new();
        let world = fixture.world(bedrock);
        let input = world_input(bedrock, &fixture.0);
        let plan = review(&fixture.0, &input).unwrap();
        assert_eq!(plan.files.len(), 2);
        assert!(!plan.destination.exists());
        apply(&fixture.0, &plan).unwrap();
        let mut zip =
            zip::ZipArchive::new(std::fs::File::open(&plan.destination).unwrap()).unwrap();
        for file in &plan.files {
            let mut bytes = Vec::new();
            zip.by_name(&file.relative)
                .unwrap()
                .read_to_end(&mut bytes)
                .unwrap();
            assert_eq!(bytes, std::fs::read(world.join(&file.relative)).unwrap());
        }
        let before = std::fs::read(&plan.destination).unwrap();
        assert!(apply(&fixture.0, &plan).is_err());
        assert_eq!(std::fs::read(&plan.destination).unwrap(), before);
    }
}
#[test]
fn changed_world_or_output_refuses_before_writing() {
    let fixture = Fixture::new();
    let world = fixture.world(false);
    let plan = review(&fixture.0, &world_input(false, &fixture.0)).unwrap();
    std::fs::write(world.join("level.dat"), b"new bytes").unwrap();
    assert!(apply(&fixture.0, &plan).unwrap_err().contains("changed"));
    assert!(!plan.destination.exists());
    let mut input = world_input(false, &fixture.0);
    if let Input::World { output, .. } = &mut input {
        *output = world.join("inside.zip").display().to_string();
    }
    assert!(review(&fixture.0, &input).unwrap_err().contains("inside"));
    if let Input::World { output, .. } = &mut input {
        *output = "relative.zip".into();
    }
    assert!(review(&fixture.0, &input).is_err());
}
#[test]
fn local_java_file_is_reviewed_copied_and_never_overwritten() {
    let fixture = Fixture::new();
    let source = fixture.0.join("example.jar");
    std::fs::write(&source, b"test jar bytes").unwrap();
    let input = Input::Addition {
        target: Target::Java("collection".into()),
        kind: AdditionKind::Mod,
        source: source.display().to_string(),
        world: None,
    };
    let plan = review(&fixture.0, &input).unwrap();
    assert!(!fixture.0.join("mods").exists());
    apply(&fixture.0, &plan).unwrap();
    assert_eq!(std::fs::read(&plan.destination).unwrap(), b"test jar bytes");
    assert!(review(&fixture.0, &input).is_err());
}
#[test]
fn changed_local_source_does_not_create_a_destination() {
    let fixture = Fixture::new();
    let source = fixture.0.join("example.zip");
    std::fs::write(&source, b"first").unwrap();
    let input = Input::Addition {
        target: Target::Java("collection".into()),
        kind: AdditionKind::Resource,
        source: source.display().to_string(),
        world: None,
    };
    let plan = review(&fixture.0, &input).unwrap();
    std::fs::write(source, b"second").unwrap();
    assert!(apply(&fixture.0, &plan).unwrap_err().contains("changed"));
    assert!(!plan.destination.exists());
}
#[test]
fn bedrock_pack_extracts_under_its_uuid_and_only_activates_the_chosen_world() {
    let fixture = Fixture::new();
    let world = fixture.world(true);
    let list = world.join("world_resource_packs.json");
    let previous = serde_json::json!([{"pack_id":"22222222-3333-4444-5555-666666666666","version":[2,0,0],"preserve":"yes"}]);
    std::fs::write(&list, serde_json::to_vec(&previous).unwrap()).unwrap();
    let source = fixture.pack(&[
        ("wrapped/manifest.json", MANIFEST),
        ("wrapped/textures/test.txt", b"pack texture"),
    ]);
    let plan = review(&fixture.0, &pack_input(&source, Some("world"))).unwrap();
    assert_eq!(
        plan.activation.as_ref().unwrap().before,
        previous.as_array().unwrap().clone()
    );
    assert!(!plan.destination.exists());
    assert_eq!(plan.expanded_files.len(), 2);
    assert_eq!(plan.expanded_files[0].0, "manifest.json");
    apply(&fixture.0, &plan).unwrap();
    assert_eq!(
        std::fs::read(plan.destination.join("manifest.json")).unwrap(),
        MANIFEST
    );
    assert_eq!(
        std::fs::read(plan.destination.join("textures/test.txt")).unwrap(),
        b"pack texture"
    );
    let active = content::read_activation(&list).unwrap();
    assert_eq!(active.len(), 2);
    assert_eq!(active[0], previous[0]);
    assert_eq!(
        std::fs::read(world.join("level.dat")).unwrap(),
        b"raw world metadata"
    );
    assert_eq!(
        std::fs::read(world.join("db/000001.ldb")).unwrap(),
        b"opaque database bytes"
    );
}
#[test]
fn changed_activation_refuses_the_entire_pack_import() {
    let fixture = Fixture::new();
    let world = fixture.world(true);
    let source = fixture.pack(&[("manifest.json", MANIFEST)]);
    let plan = review(&fixture.0, &pack_input(&source, Some("world"))).unwrap();
    std::fs::write(world.join("world_resource_packs.json"), b"[]").unwrap();
    assert!(apply(&fixture.0, &plan).unwrap_err().contains("changed"));
    assert!(!plan.destination.exists());
}
#[test]
fn pack_import_without_activation_never_writes_world_lists() {
    let fixture = Fixture::new();
    let world = fixture.world(true);
    let source = fixture.pack(&[("manifest.json", MANIFEST)]);
    let plan = review(&fixture.0, &pack_input(&source, None)).unwrap();
    apply(&fixture.0, &plan).unwrap();
    assert!(!world.join("world_resource_packs.json").exists());
}
#[test]
fn unsafe_duplicate_and_multi_pack_archives_are_refused() {
    for entries in [
        vec![
            ("manifest.json", MANIFEST),
            ("../escape", b"bad".as_slice()),
        ],
        vec![
            ("manifest.json", MANIFEST),
            ("Textures/x", b"a".as_slice()),
            ("textures/x", b"b".as_slice()),
        ],
        // Two manifests with the same UUID are still ambiguous, even though
        // multi-pack add-ons with distinct identities are now supported.
        vec![
            ("first/manifest.json", MANIFEST),
            ("second/manifest.json", MANIFEST),
        ],
        vec![
            ("wrapped/manifest.json", MANIFEST),
            ("outside.txt", b"bad".as_slice()),
        ],
    ] {
        let fixture = Fixture::new();
        let source = fixture.pack(&entries);
        assert!(review(&fixture.0, &pack_input(&source, None)).is_err());
        assert!(!fixture.0.join("resource_packs").exists());
    }
}
#[cfg(unix)]
#[test]
fn linked_sources_world_entries_and_staging_are_refused() {
    let fixture = Fixture::new();
    let world = fixture.world(true);
    std::os::unix::fs::symlink(world.join("level.dat"), world.join("linked")).unwrap();
    assert!(review(&fixture.0, &world_input(true, &fixture.0)).is_err());
    let source = fixture.pack(&[("manifest.json", MANIFEST)]);
    std::os::unix::fs::symlink(
        world.join("level.dat"),
        world.join("world_resource_packs.tmp"),
    )
    .unwrap();
    assert!(review(&fixture.0, &pack_input(&source, Some("world"))).is_err());
}
