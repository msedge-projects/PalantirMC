use super::*;
use crate::{local, surfaces::AdditionKind};
use std::io::Write;
fn manifest(kind: &str, uuid: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({"format_version":2,"header":{"name":format!("Test {kind}"),"uuid":uuid,"version":[1,2,3]},"modules":[{"type":kind,"version":[1,2,3]}]})).unwrap()
}
fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "palantir-addon-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("minecraftWorlds/world")).unwrap();
        std::fs::write(
            root.join("minecraftWorlds/world/level.dat"),
            b"unchanged world metadata",
        )
        .unwrap();
        Self(root)
    }
    fn input(&self, bytes: &[u8], world: bool) -> local::Input {
        let source = self.0.join("test.mcaddon");
        std::fs::write(&source, bytes).unwrap();
        local::Input::Addition {
            target: local::Target::Bedrock(palantir_bedrock::Channel::Release),
            kind: AdditionKind::BedrockPack,
            source: source.display().to_string(),
            world: world.then(|| "world".into()),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
const RESOURCE: &str = "11111111-2222-3333-4444-555555555555";
const BEHAVIOUR: &str = "22222222-3333-4444-5555-666666666666";
#[test]
fn distinct_manifest_packs_land_in_their_own_folders_and_preserve_activation_entries() {
    for nested in [false, true] {
        let fixture = Fixture::new();
        let r = manifest("resources", RESOURCE);
        let b = manifest("data", BEHAVIOUR);
        let rpack = archive(&[("manifest.json", &r), ("textures/example.txt", b"texture")]);
        let bpack = archive(&[
            ("manifest.json", &b),
            ("functions/test.mcfunction", b"say hello"),
        ]);
        let bytes = if nested {
            archive(&[("resource.mcpack", &rpack), ("behavior.mcpack", &bpack)])
        } else {
            archive(&[
                ("resources/manifest.json", &r),
                ("resources/textures/example.txt", b"texture"),
                ("behavior/manifest.json", &b),
                ("behavior/functions/test.mcfunction", b"say hello"),
            ])
        };
        let world = fixture.0.join("minecraftWorlds/world");
        let previous = serde_json::json!([{"pack_id":"33333333-4444-5555-6666-777777777777","version":[0,1,0],"keep":"yes"}]);
        std::fs::write(
            world.join("world_resource_packs.json"),
            serde_json::to_vec(&previous).unwrap(),
        )
        .unwrap();
        let input = fixture.input(&bytes, true);
        let plan = local::review(&fixture.0, &input).unwrap();
        assert_eq!(plan.bundle.as_ref().unwrap().packs.len(), 2);
        assert_eq!(plan.expanded_files.len(), 4);
        assert!(!fixture.0.join("resource_packs").exists());
        local::apply(&fixture.0, &plan).unwrap();
        assert_eq!(
            std::fs::read(
                fixture
                    .0
                    .join(format!("resource_packs/{RESOURCE}/textures/example.txt"))
            )
            .unwrap(),
            b"texture"
        );
        assert_eq!(
            std::fs::read(fixture.0.join(format!(
                "behavior_packs/{BEHAVIOUR}/functions/test.mcfunction"
            )))
            .unwrap(),
            b"say hello"
        );
        let active = content::read_activation(&world.join("world_resource_packs.json")).unwrap();
        assert_eq!(active.len(), 2);
        assert_eq!(active[0], previous[0]);
        assert_eq!(
            content::read_activation(&world.join("world_behavior_packs.json"))
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            std::fs::read(world.join("level.dat")).unwrap(),
            b"unchanged world metadata"
        );
        assert!(local::apply(&fixture.0, &plan).is_err());
    }
}
#[test]
fn changed_addon_or_existing_member_refuses_before_installing_other_members() {
    let fixture = Fixture::new();
    let r = manifest("resources", RESOURCE);
    let b = manifest("data", BEHAVIOUR);
    let bytes = archive(&[("r/manifest.json", &r), ("b/manifest.json", &b)]);
    let input = fixture.input(&bytes, false);
    let plan = local::review(&fixture.0, &input).unwrap();
    std::fs::write(&plan.source, b"changed").unwrap();
    assert!(local::apply(&fixture.0, &plan).is_err());
    assert!(!fixture.0.join("resource_packs").exists());
    fixture.input(&bytes, false);
    std::fs::create_dir_all(fixture.0.join(format!("behavior_packs/{BEHAVIOUR}"))).unwrap();
    assert!(local::review(&fixture.0, &input).is_err());
    assert!(!fixture.0.join("resource_packs").exists());
}
#[test]
fn duplicate_id_unsafe_nested_member_and_stray_file_are_refused() {
    let r = manifest("resources", RESOURCE);
    let duplicate = manifest("data", RESOURCE);
    let unsafe_pack = archive(&[("manifest.json", &r), ("../outside", b"bad")]);
    for bytes in [
        archive(&[("r/manifest.json", &r), ("b/manifest.json", &duplicate)]),
        archive(&[("nested.mcpack", &unsafe_pack)]),
        archive(&[
            ("r/manifest.json", &r),
            ("b/manifest.json", &duplicate),
            ("stray", b"bad"),
        ]),
    ] {
        let fixture = Fixture::new();
        let input = fixture.input(&bytes, false);
        assert!(local::review(&fixture.0, &input).is_err());
        assert!(!fixture.0.join("resource_packs").exists());
    }
}
