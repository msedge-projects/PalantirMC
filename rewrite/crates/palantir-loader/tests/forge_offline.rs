//! The Forge-family vendor paths, offline: the URLs the installers are
//! published at, and what a vendor's version number means.
//!
//! Everything here is a pure function of a string, so it is checked without a
//! network: the live half (fetch the jar, run the processors, launch the
//! result) is `live.rs`, and it is the one that needs ~650 MB, and the fetch
//! itself is driven against a mock vendor Maven in
//! `forge_install_offline.rs`.
//!
//! The listings below are shaped from the vendors' real files rather than
//! invented: Maven writes metadata in *deploy* order, which is not a release
//! order and is not even monotonic — Forge's own file was checked on
//! 2026-10-12 and opens its 1.20.1 run descending while ending it ascending.
//! `FORGE_DESCENDING` is that first shape, because a listing whose newest
//! release comes first is exactly the case a "take the last entry" rule gets
//! wrong.

use palantir_loader::forge::{
    Vendor, game_version, installer_file_name, installer_path, installer_url, versions_for_game,
};

/// A listing as Maven writes it: oldest first, with the `<latest>` and
/// `<release>` elements that are *not* version entries.
const FORGE_LISTING: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<metadata>
  <groupId>net.minecraftforge</groupId>
  <artifactId>forge</artifactId>
  <versioning>
    <latest>1.20.1-47.10.0</latest>
    <release>1.20.1-47.10.0</release>
    <versions>
      <version>1.19.4-45.1.0</version>
      <version>1.20.1-47.1.0</version>
      <version>1.20.1-47.4.26</version>
      <version>1.20.1-47.10.0</version>
      <version>1.20.2-48.0.1</version>
    </versions>
    <lastUpdated>20260101000000</lastUpdated>
  </versioning>
</metadata>
"#;

const NEOFORGE_LISTING: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<metadata>
  <groupId>net.neoforged</groupId>
  <artifactId>neoforge</artifactId>
  <versioning>
    <latest>26.1.2.116</latest>
    <release>26.1.2.116</release>
    <versions>
      <version>20.2.88</version>
      <version>20.6.141</version>
      <version>21.0.167</version>
      <version>21.1.77</version>
      <version>26.1.0.0-alpha.1+snapshot-1</version>
      <version>26.1.2.116</version>
      <version>26.1.1.5</version>
    </versions>
  </versioning>
</metadata>
"#;

/// A listing whose newest release is the *first* entry, which is how Forge's
/// real 1.21 run is written.
const FORGE_DESCENDING: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<metadata>
  <versioning>
    <latest>1.21-51.0.33</latest>
    <release>1.21-51.0.33</release>
    <versions>
      <version>1.21-51.0.33</version>
      <version>1.21-51.0.32</version>
      <version>1.21-51.0.2</version>
    </versions>
  </versioning>
</metadata>
"#;

#[test]
fn installer_urls_are_the_vendors_own_published_paths() {
    assert_eq!(
        installer_url(Vendor::Forge, "1.20.1-47.4.26"),
        "https://maven.minecraftforge.net/net/minecraftforge/forge/1.20.1-47.4.26/\
         forge-1.20.1-47.4.26-installer.jar"
    );
    assert_eq!(
        installer_url(Vendor::NeoForge, "20.6.141"),
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/20.6.141/\
         neoforge-20.6.141-installer.jar"
    );
    assert_eq!(
        installer_file_name(Vendor::NeoForge, "20.6.141"),
        "neoforge-20.6.141-installer.jar"
    );
}

#[test]
fn both_vendors_publish_their_metadata_beside_their_artifacts() {
    let forge = Vendor::Forge.metadata_url();
    let neoforge = Vendor::NeoForge.metadata_url();
    assert_eq!(
        forge,
        "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml"
    );
    assert_eq!(
        neoforge,
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml"
    );
}

#[test]
fn the_jar_is_staged_under_the_name_it_is_published_with() {
    let into = std::path::Path::new("/data/versions/1.20.1");
    assert_eq!(
        installer_path(into, Vendor::Forge, "1.20.1-47.4.26"),
        into.join("forge-1.20.1-47.4.26-installer.jar")
    );
}

#[test]
fn the_forge_game_is_the_part_before_the_loader() {
    assert_eq!(
        game_version(Vendor::Forge, "1.20.1-47.4.26").as_deref(),
        Some("1.20.1")
    );
    assert_eq!(
        game_version(Vendor::Forge, "1.12.2-14.23.5.2859").as_deref(),
        Some("1.12.2")
    );
    // The pre-1.7 generation named both parts, one of them twice over.
    let legacy = game_version(Vendor::Forge, "1.7.10-10.13.4.1614-1.7.10");
    assert_eq!(legacy.as_deref(), Some("1.7.10"));
    // The year-and-release game ids are Forge's own left half too.
    assert_eq!(
        game_version(Vendor::Forge, "26.3-66.0.10").as_deref(),
        Some("26.3")
    );
    // A version number that names only the loader has no game to report; the
    // game came from the profile inside the jar instead.
    assert_eq!(game_version(Vendor::Forge, "9.11.1.1345"), None);
    assert_eq!(game_version(Vendor::Forge, "-47.4.26"), None);
    assert_eq!(game_version(Vendor::Forge, ""), None);
}

#[test]
fn neoforge_versions_map_back_to_the_game_they_install_over() {
    assert_eq!(
        game_version(Vendor::NeoForge, "20.2.88").as_deref(),
        Some("1.20.2")
    );
    assert_eq!(
        game_version(Vendor::NeoForge, "20.6.141").as_deref(),
        Some("1.20.6")
    );
    // A minor of zero is the `.0` game release, not a missing component.
    assert_eq!(
        game_version(Vendor::NeoForge, "21.0.167").as_deref(),
        Some("1.21")
    );
    assert_eq!(
        game_version(Vendor::NeoForge, "21.1.77").as_deref(),
        Some("1.21.1")
    );
    assert_eq!(
        game_version(Vendor::NeoForge, "21.4.1").as_deref(),
        Some("1.21.4")
    );
    assert_eq!(
        game_version(Vendor::NeoForge, "21.10.3").as_deref(),
        Some("1.21.10")
    );
    // The game's ids moved to a year-and-release scheme and this vendor's
    // leading pair follows them: `26.1` is a real id and `1.26` is not one,
    // so the pair is the game rather than something to prefix with `1.`.
    assert_eq!(
        game_version(Vendor::NeoForge, "26.3.0.76").as_deref(),
        Some("26.3")
    );
    assert_eq!(
        game_version(Vendor::NeoForge, "26.1.2.116").as_deref(),
        Some("26.1")
    );
    assert_eq!(
        game_version(Vendor::NeoForge, "26.1.0.0-alpha.1+snapshot-1").as_deref(),
        Some("26.1")
    );
    // Not a release of theirs, or not a version at all.
    assert_eq!(game_version(Vendor::NeoForge, "19.0.1"), None);
    assert_eq!(game_version(Vendor::NeoForge, "not-a-version"), None);
    assert_eq!(game_version(Vendor::NeoForge, "20"), None);
    assert_eq!(game_version(Vendor::NeoForge, ""), None);
    // One vendor's version number is never read with the other's scheme.
    assert_eq!(game_version(Vendor::NeoForge, "1.20.1-47.4.26"), None);
}

#[test]
fn a_listing_keeps_only_the_game_asked_for_newest_first() {
    assert_eq!(
        versions_for_game(FORGE_LISTING, Vendor::Forge, "1.20.1"),
        vec![
            "1.20.1-47.10.0".to_string(),
            "1.20.1-47.4.26".to_string(),
            "1.20.1-47.1.0".to_string(),
        ]
    );
    // A loader patch version that sorts *before* a smaller one as text still
    // comes first as newest, because the numbers are compared.
    let newest = versions_for_game(FORGE_LISTING, Vendor::Forge, "1.20.2");
    assert_eq!(newest, vec!["1.20.2-48.0.1".to_string()]);
    assert!(versions_for_game(FORGE_LISTING, Vendor::Forge, "1.21").is_empty());
    // The case the document's own order gets wrong: its newest release is its
    // first entry, so "take the last one" would offer a player `51.0.2`.
    assert_eq!(
        versions_for_game(FORGE_DESCENDING, Vendor::Forge, "1.21"),
        vec![
            "1.21-51.0.33".to_string(),
            "1.21-51.0.32".to_string(),
            "1.21-51.0.2".to_string(),
        ]
    );

    assert_eq!(
        versions_for_game(NEOFORGE_LISTING, Vendor::NeoForge, "1.21.1"),
        vec!["21.1.77".to_string()]
    );
    assert_eq!(
        versions_for_game(NEOFORGE_LISTING, Vendor::NeoForge, "1.21"),
        vec!["21.0.167".to_string()]
    );
    let shipped = versions_for_game(NEOFORGE_LISTING, Vendor::NeoForge, "1.20.6");
    assert_eq!(shipped, vec!["20.6.141".to_string()]);
    // The year-and-release ids, listed out of order in the document: the two
    // release numbers decide, and a `-beta`/`+snapshot` label is not a number.
    assert_eq!(
        versions_for_game(NEOFORGE_LISTING, Vendor::NeoForge, "26.1"),
        vec![
            "26.1.2.116".to_string(),
            "26.1.1.5".to_string(),
            "26.1.0.0-alpha.1+snapshot-1".to_string(),
        ]
    );
    // A game NeoForge never shipped a release for.
    assert!(versions_for_game(NEOFORGE_LISTING, Vendor::NeoForge, "1.20.1").is_empty());
    assert!(versions_for_game(NEOFORGE_LISTING, Vendor::NeoForge, "1.26").is_empty());
}

#[test]
fn a_listing_that_carries_nothing_is_empty_rather_than_an_error() {
    assert!(versions_for_game("", Vendor::Forge, "1.20.1").is_empty());
    let empty_metadata = "<metadata/>";
    assert!(versions_for_game(empty_metadata, Vendor::NeoForge, "1.21").is_empty());
    // An element that never closes must not swallow the versions after it.
    let broken = "<version>1.20.1-47.1.0</version><version>1.20.1-47.4.26";
    let recovered = versions_for_game(broken, Vendor::Forge, "1.20.1");
    assert_eq!(recovered, vec!["1.20.1-47.1.0".to_string()]);
}
