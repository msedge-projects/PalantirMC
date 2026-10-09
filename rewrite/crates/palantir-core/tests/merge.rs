//! Version inheritance against the real loader profiles.
//!
//! Both fixtures are the loader vendors' own launcher profiles for game
//! 1.20.1 (see `THIRD_PARTY_NOTICES.md`), each carrying `inheritsFrom` and
//! nothing of the game's own metadata. The real pairs contain *no*
//! coordinate collision between child and parent, so the conflict rules
//! are pinned synthetically below -- the real samples fix the shape, the
//! synthetic ones fix the semantics we chose where the samples are silent.

use palantir_core::version::{Argument, Version};

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

fn parse(name: &str) -> Version {
    Version::parse(&fixture(name)).unwrap()
}

#[test]
fn fabric_merges_over_its_real_parent() {
    let parent = parse("version-1.20.1.json");
    let child = parse("fabric-loader-profile-1.20.1.json");
    let merged = child.merged_with(&parent).unwrap();

    // The child decides who this version is and how it starts.
    assert_eq!(merged.id, "fabric-loader-0.19.5-1.20.1");
    assert_eq!(
        merged.main_class,
        "net.fabricmc.loader.impl.launch.knot.KnotClient"
    );
    assert!(
        merged.inherits_from.is_none(),
        "the result must be standalone"
    );

    // Everything the child never mentions comes from the parent: the game
    // jar, its asset index, its java requirement.
    assert_eq!(
        merged.downloads.get("client").unwrap().sha1,
        parent.downloads.get("client").unwrap().sha1
    );
    assert_eq!(merged.asset_index, parent.asset_index);
    assert_eq!(merged.java_version, parent.java_version);

    // The real pair collides on no coordinate, so both lists survive whole:
    // the loader's 8 libraries lead, the game's 88 follow.
    assert_eq!(merged.libraries.len(), 8 + 88);
    assert_eq!(merged.libraries[0].name, "org.ow2.asm:asm:9.10.1");
    assert_eq!(merged.libraries[8].name, parent.libraries[0].name);

    // Arguments concatenate parent-first, child-last: the loader's one JVM
    // property is the final word on the command line.
    let args = merged.arguments.as_ref().unwrap();
    let parent_args = parent.arguments.as_ref().unwrap();
    // The child declares `"game": []` -- present, saying nothing -- and the
    // distinction from the parent's game list is the parent's whole list.
    let game = args.game.as_deref().unwrap();
    assert_eq!(game, parent_args.game.as_deref().unwrap());
    let jvm = args.jvm.as_deref().unwrap();
    assert_eq!(jvm.len(), parent_args.jvm.as_deref().unwrap().len() + 1);
    assert!(matches!(
        jvm.last(),
        Some(Argument::Plain(text))
            if text == "-DFabricMcEmu= net.minecraft.client.main.Main "
    ));
}

#[test]
fn quilt_merges_over_its_real_parent() {
    let parent = parse("version-1.20.1.json");
    let child = parse("quilt-loader-profile-1.20.1.json");
    let merged = child.merged_with(&parent).unwrap();

    assert_eq!(merged.id, "quilt-loader-0.31.0-beta.4-1.20.1");
    assert_eq!(
        merged.main_class,
        "org.quiltmc.loader.impl.launch.knot.KnotClient"
    );
    assert_eq!(merged.libraries.len(), 11 + 88);
    // Quilt ships Fabric's sponge-mixin and its own loader; both lead the
    // classpath, ahead of every game library.
    assert!(
        merged.libraries[0]
            .name
            .starts_with("net.fabricmc:sponge-mixin")
    );
    assert_eq!(merged.asset_index, parent.asset_index);
}

#[test]
fn a_child_library_replaces_the_parents_by_coordinate() {
    // The real pairs never collide, so pin the rule itself: same
    // `group:artifact` at any version means one artifact, and the child's
    // entry is the one wanted -- leading the list so it also shadows.
    let parent = Version::parse(
        r#"{"id": "game", "type": "release", "mainClass": "a.Main",
            "time": "t", "releaseTime": "r",
            "libraries": [
                {"name": "org.ow2.asm:asm:9.3"},
                {"name": "com.example:kept:1.0"}]}"#,
    )
    .unwrap();
    let child = Version::parse(
        r#"{"id": "mod", "type": "release", "mainClass": "b.Main",
            "time": "t", "releaseTime": "r", "inheritsFrom": "game",
            "libraries": [{"name": "org.ow2.asm:asm:9.10.1"}]}"#,
    )
    .unwrap();
    let merged = child.merged_with(&parent).unwrap();
    let names: Vec<&str> = merged.libraries.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["org.ow2.asm:asm:9.10.1", "com.example:kept:1.0"]
    );
}

#[test]
fn a_classifier_is_part_of_a_librarys_identity() {
    // `g:a:v` and `g:a:v:natives` are different artifacts: replacing one
    // must not drop the other.
    let parent = Version::parse(
        r#"{"id": "game", "type": "release", "mainClass": "a.Main",
            "time": "t", "releaseTime": "r",
            "libraries": [
                {"name": "com.example:nat:1.0"},
                {"name": "com.example:nat:1.0:natives-linux"}]}"#,
    )
    .unwrap();
    let child = Version::parse(
        r#"{"id": "mod", "type": "release", "mainClass": "b.Main",
            "time": "t", "releaseTime": "r", "inheritsFrom": "game",
            "libraries": [{"name": "com.example:nat:2.0"}]}"#,
    )
    .unwrap();
    let merged = child.merged_with(&parent).unwrap();
    let names: Vec<&str> = merged.libraries.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["com.example:nat:2.0", "com.example:nat:1.0:natives-linux"]
    );
}

#[test]
fn jar_is_the_default_extension_and_not_an_identity() {
    // NeoForge spells its libraries `group:artifact:version@jar` where the
    // game spells the same coordinate bare: one file, so the child
    // replaces the parent's entry instead of leaving both to land on the
    // classpath (the game's own bootstrap refuses a duplicate jar).
    let parent = Version::parse(
        r#"{"id": "game", "type": "release", "mainClass": "a.Main",
            "time": "t", "releaseTime": "r",
            "libraries": [
                {"name": "org.slf4j:slf4j-api:2.0.9"},
                {"name": "com.example:kept:1.0"}]}"#,
    )
    .unwrap();
    let child = Version::parse(
        r#"{"id": "mod", "type": "release", "mainClass": "b.Main",
            "time": "t", "releaseTime": "r", "inheritsFrom": "game",
            "libraries": [{"name": "org.slf4j:slf4j-api:2.0.9@jar"}]}"#,
    )
    .unwrap();
    let merged = child.merged_with(&parent).unwrap();
    let names: Vec<&str> = merged.libraries.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["org.slf4j:slf4j-api:2.0.9@jar", "com.example:kept:1.0"]
    );
}

#[test]
fn inheriting_from_the_wrong_parent_is_an_error_naming_both() {
    let parent = Version::parse(
        r#"{"id": "game", "type": "release", "mainClass": "a.Main",
            "time": "t", "releaseTime": "r"}"#,
    )
    .unwrap();
    let child = Version::parse(
        r#"{"id": "mod", "type": "release", "mainClass": "b.Main",
            "time": "t", "releaseTime": "r", "inheritsFrom": "other"}"#,
    )
    .unwrap();
    let error = child.merged_with(&parent).unwrap_err().to_string();
    assert!(error.contains("mod"), "{error}");
    assert!(error.contains("other"), "{error}");
    assert!(error.contains("game"), "{error}");
}

#[test]
fn launcher_minimums_merge_to_the_larger() {
    // The combined document must satisfy both halves, so the stricter
    // minimum wins no matter which side states it.
    let parent = Version::parse(
        r#"{"id": "game", "type": "release", "mainClass": "a.Main",
            "time": "t", "releaseTime": "r", "minimumLauncherVersion": 21}"#,
    )
    .unwrap();
    let child = Version::parse(
        r#"{"id": "mod", "type": "release", "mainClass": "b.Main",
            "time": "t", "releaseTime": "r", "inheritsFrom": "game",
            "minimumLauncherVersion": 24}"#,
    )
    .unwrap();
    assert_eq!(
        child.merged_with(&parent).unwrap().minimum_launcher_version,
        Some(24)
    );
}
