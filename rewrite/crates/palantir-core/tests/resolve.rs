//! Rule and library resolution against the real downloaded samples.
//!
//! The fixtures carry rule shapes no synthetic test would have invented --
//! an unconditional allow followed by a platform disallow, a boundary pair
//! of version ranges sharing one version number -- so the resolution logic
//! is checked against exactly those.

use palantir_core::library::ResolvedLibrary;
use palantir_core::rules::{Arch, Os, Platform, rules_allow};
use palantir_core::version::{Argument, Version};

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

fn version(name: &str) -> Version {
    Version::parse(&fixture(name)).unwrap()
}

fn platform(os: Os, os_version: &str) -> Platform {
    Platform::new(os, os_version, Arch::X86_64)
}

/// The strings a platform would get from one argument list. `launch` will own
/// this expansion in the next slice; here it is the test's lens onto which
/// rules fire.
fn selected_strings(version: &Version, list: &[Argument], platform: &Platform) -> Vec<String> {
    let _ = version;
    let mut out = Vec::new();
    for argument in list {
        match argument {
            Argument::Plain(text) => out.push(text.clone()),
            Argument::Conditional { rules, value } => {
                if rules_allow(rules, platform).unwrap() {
                    out.extend(value.strings().iter().cloned());
                }
            }
        }
    }
    out
}

fn resolve_all(version: &Version, platform: &Platform) -> Vec<ResolvedLibrary> {
    version
        .libraries
        .iter()
        .filter_map(|lib| lib.resolve(platform).unwrap())
        .collect()
}

#[test]
fn lwjgl_versions_split_by_platform() {
    // 1.12.2 ships lwjgl twice: `2.9.4` as `[{allow}, {disallow osx}]` and
    // `2.9.2` as `[{allow osx}]` -- macOS runs the older build, everyone
    // else the newer one. Both rule shapes are the format's own.
    let version = version("version-1.12.2.json");
    let lwjgl_versions = |platform: &Platform| {
        let mut found: Vec<String> = resolve_all(&version, platform)
            .iter()
            .filter(|lib| lib.coord.group == "org.lwjgl.lwjgl" && lib.coord.artifact == "lwjgl")
            .map(|lib| lib.coord.version.clone())
            .collect();
        found.sort();
        found
    };
    let old = "2.9.2-nightly-20140822".to_string();
    let new = "2.9.4-nightly-20150209".to_string();

    for (os, version_string) in [(Os::Windows, "10.0.19045"), (Os::Linux, "6.8.0")] {
        assert_eq!(
            lwjgl_versions(&platform(os, version_string)),
            vec![new.clone()],
            "{os:?}"
        );
    }
    assert_eq!(
        lwjgl_versions(&platform(Os::MacOs, "14.0")),
        vec![old],
        "MacOs"
    );
}

#[test]
fn natives_classifiers_land_on_every_desktop_platform() {
    let version = version("version-1.12.2.json");
    for (os, version_string, token) in [
        (Os::Windows, "10.0.19045", "windows"),
        (Os::Linux, "6.8.0", "linux"),
        (Os::MacOs, "14.0", "osx"),
    ] {
        let resolved = resolve_all(&version, &platform(os, version_string));
        let natives: Vec<&str> = resolved
            .iter()
            .filter_map(|lib| lib.natives.as_ref().map(|n| n.rel_path.as_str()))
            .collect();
        assert!(
            natives.iter().any(|path| path.contains(token)),
            "no {token} natives jar selected on {os:?}: {natives:?}"
        );
        // The base jar appears exactly when the metadata names one. This is
        // not a tautology: `jinput-platform` is classifiers-only in the real
        // document (its base jar is not on the repository at all -- asking
        // gets a 404), so it contributes its natives and no base jar, while
        // `text2speech` contributes both.
        for lib in &resolved {
            let name = format!(
                "{}:{}:{}",
                lib.coord.group, lib.coord.artifact, lib.coord.version
            );
            let source = version
                .libraries
                .iter()
                .find(|l| l.name == name)
                .unwrap_or_else(|| panic!("no metadata record for {name}"));
            let named_artifact = match &source.downloads {
                None => true, // name-only documents derive one
                Some(downloads) => downloads.artifact.is_some(),
            };
            assert_eq!(
                lib.artifact.is_some(),
                named_artifact,
                "{}: base jar does not follow its download records",
                source.name
            );
        }
        // Named outright so this split cannot go vacuous again.
        let jinput = resolved
            .iter()
            .find(|lib| lib.coord.artifact == "jinput-platform")
            .expect("jinput-platform resolved");
        assert!(
            jinput.artifact.is_none(),
            "jinput-platform gained a base jar"
        );
        assert!(jinput.natives.is_some());
    }
}

#[test]
fn an_osx_only_library_stays_on_macos() {
    let version = version("version-26.3.json");
    let has_bridge = |platform: &Platform| {
        resolve_all(&version, platform)
            .iter()
            .any(|lib| lib.coord.artifact == "java-objc-bridge")
    };
    assert!(has_bridge(&platform(Os::MacOs, "14.0")));
    assert!(!has_bridge(&platform(Os::Windows, "10.0.19045")));
    assert!(!has_bridge(&platform(Os::Linux, "6.8.0")));
}

#[test]
fn the_zgc_boundary_partitions_windows_versions() {
    // The 26.3 sample tunes the JVM two ways around one boundary: ZGC at
    // `min: 10.0.17134`, the G1 set at `max: 10.0.17134`. Exactly at the
    // boundary only the min side may fire -- read `max` as exclusive, or
    // both would claim it.
    let version = version("version-26.3.json");
    let list = &version.arguments.as_ref().unwrap().default_user_jvm;
    let tuning = |platform: &Platform| selected_strings(&version, list, platform);

    let old = tuning(&platform(Os::Windows, "10.0.17133"));
    assert!(!old.contains(&"-XX:+UseZGC".to_string()), "{old:?}");
    assert!(old.contains(&"-XX:+UseG1GC".to_string()), "{old:?}");

    let boundary = tuning(&platform(Os::Windows, "10.0.17134"));
    assert!(
        boundary.contains(&"-XX:+UseZGC".to_string()),
        "{boundary:?}"
    );
    assert!(
        !boundary.contains(&"-XX:+UseG1GC".to_string()),
        "{boundary:?}"
    );

    let new = tuning(&platform(Os::Windows, "10.0.19045"));
    assert!(new.contains(&"-XX:+UseZGC".to_string()), "{new:?}");

    // The unconditional tuning entry travels everywhere.
    assert!(new.contains(&"-Xmx4G".to_string()), "{new:?}");
    // And macOS and Linux take the ZGC arm by name.
    assert!(tuning(&platform(Os::MacOs, "14.0")).contains(&"-XX:+UseZGC".to_string()));
    assert!(tuning(&platform(Os::Linux, "6.8.0")).contains(&"-XX:+UseZGC".to_string()));
}
