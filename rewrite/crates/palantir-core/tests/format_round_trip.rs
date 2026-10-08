//! The real samples must survive a round trip unchanged in value.
//!
//! The fixtures are downloaded public metadata documents (see
//! `THIRD_PARTY_NOTICES.md` for their origin and fetch date). A parser that
//! drops a field it does not model would corrupt a version document the
//! first time a launcher wrote one back -- loader installs do write them --
//! so the check is: parse, serialize, and require the JSON value to be
//! identical, then require re-parsing to be equal to the first parse.

use palantir_core::assets::{AssetDestination, AssetIndex};
use palantir_core::version::{Version, VersionManifest};
use serde_json::Value;

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

fn round_trip<T>(text: &str, parse: impl Fn(&str) -> palantir_core::Result<T>)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let before: Value = serde_json::from_str(text).unwrap();
    let parsed = parse(text).unwrap();
    let written = serde_json::to_string(&parsed).unwrap();
    let after: Value = serde_json::from_str(&written).unwrap();
    assert_eq!(
        before, after,
        "serialize(parse(x)) changed the document's value"
    );
    let reparsed = parse(&written).unwrap();
    assert_eq!(parsed, reparsed, "re-parsing our own output disagreed");
}

#[test]
fn manifest_round_trips_a_real_manifest() {
    let text = fixture("version_manifest_v2.json");
    round_trip(&text, VersionManifest::parse);
    let manifest = VersionManifest::parse(&text).unwrap();
    // As published on 2026-10-08: 918 versions, latest release 26.3.
    assert_eq!(manifest.versions.len(), 918);
    assert_eq!(manifest.latest.release, "26.3");
    assert_eq!(manifest.latest.snapshot, "26.4-snapshot-3");
    let entry = manifest.find("1.12.2").unwrap();
    assert_eq!(entry.kind, "release");
}

#[test]
fn modern_version_metadata_round_trips() {
    let text = fixture("version-26.3.json");
    round_trip(&text, Version::parse);
    let version = Version::parse(&text).unwrap();
    assert_eq!(version.id, "26.3");
    // The 2026 shape: three argument lists, including the user-overrideable
    // JVM tuning group that the format did not have in 2018.
    let arguments = version.arguments.as_ref().unwrap();
    assert!(!arguments.default_user_jvm.is_empty());
    assert!(!arguments.jvm.is_empty());
    assert!(!arguments.game.is_empty());
    assert!(version.minecraft_arguments.is_none());
    assert_eq!(version.asset_index.as_ref().unwrap().id, "34");
    assert_eq!(version.java_version.as_ref().unwrap().major_version, 25);
}

#[test]
fn legacy_version_metadata_round_trips() {
    let text = fixture("version-1.12.2.json");
    round_trip(&text, Version::parse);
    let version = Version::parse(&text).unwrap();
    assert_eq!(version.id, "1.12.2");
    // The 2017 shape: one argument string, no argument lists.
    assert!(version.arguments.is_none());
    assert!(
        version
            .minecraft_arguments
            .as_ref()
            .unwrap()
            .contains("${auth_player_name}")
    );
    // Natives and extract rules live on libraries only in this era.
    assert!(version.libraries.iter().any(|l| l.natives.is_some()));
    assert!(version.libraries.iter().any(|l| l.extract.is_some()));
}

#[test]
fn oldest_version_metadata_round_trips() {
    let text = fixture("version-1.5.2.json");
    round_trip(&text, Version::parse);
    let version = Version::parse(&text).unwrap();
    assert_eq!(version.asset_index.as_ref().unwrap().id, "pre-1.6");
    assert!(version.libraries.iter().all(|l| l.name.contains(':')));
}

#[test]
fn asset_indexes_round_trip_and_declare_their_layout() {
    let plain = fixture("asset-index-1.12.json");
    round_trip(&plain, AssetIndex::parse);
    let idx = AssetIndex::parse(&plain).unwrap();
    assert_eq!(idx.destination(), AssetDestination::ObjectStore);
    assert_eq!(idx.objects.len(), 1305);
    let (name, object) = idx.objects.iter().next().unwrap();
    assert_eq!(
        idx.rel_path("1.12", name).unwrap(),
        format!("objects/{}/{}", &object.hash[..2], object.hash)
    );

    let pre16 = fixture("asset-index-pre-1.6.json");
    round_trip(&pre16, AssetIndex::parse);
    let idx = AssetIndex::parse(&pre16).unwrap();
    assert_eq!(idx.destination(), AssetDestination::Resources);
    assert_eq!(idx.objects.len(), 749);
}
