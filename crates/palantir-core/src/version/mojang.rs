//! Mojang's version file, translated into the shape this launcher resolves.
//!
//! `meta.prismlauncher.org` serves a *rewrite* of Mojang's file, and this
//! launcher's model reads the rewrite: `VersionFile` has no reading of Mojang's
//! `arguments` object at all. Reading piston directly therefore means doing the
//! translation here, and this module is it -- measured against the mirror for
//! `1.21.4`, `1.19.4`, `1.12.2` and `1.6.4` rather than inferred from one
//! example.
//!
//! | What the model reads | Where Mojang puts it |
//! | --- | --- |
//! | `minecraftArguments` | `minecraftArguments` before 1.13, and the plain strings of `arguments.game` from 1.13 on |
//! | (dropped from both) | the `--clientId ${clientid} --xuid ${auth_xuid}` pair, whose tokens no launch of this launcher fills -- see `arguments_of` |
//! | `mainJar` | `downloads.client` |
//! | `compatibleJavaMajors`, `compatibleJavaName` | `javaVersion.majorVersion`, `javaVersion.component` |
//! | `assets`, `assetIndex` | the same keys, unchanged |
//! | `libraries` | the same key, unchanged -- including the `:natives-<os>` entries 1.19 introduced, which [`Library::is_native`](crate::version::Library::is_native) now reads as natives |
//! | `+traits` | nowhere: see [`traits`] |
//!
//! Three things are deliberately not translated, and each is measured:
//!
//! * **`arguments.jvm`.** Prism's own file for every one of those four versions
//!   carries `+jvmArgs: []` or nothing at all, because the launcher builds those
//!   arguments itself: `-Djava.library.path`, the classpath, and the two
//!   workarounds it keys off the platform. Flattening them here would duplicate
//!   what `launch.rs` already writes.
//! * **Conditional `arguments.game` entries.** The objects with `rules` are
//!   `--demo`, the quick-play flags and the window size; the launcher offers no
//!   way to ask for any of them, and their presence is recorded as the
//!   `feature:is_quick_play_*` traits instead -- which is exactly what the
//!   quick-play arguments are gated on in `launch.rs`.
//! * **`logging`, `complianceLevel`, `minimumLauncherVersion`.** The model has no
//!   field for them; `logging` is Mojang's own log4j configuration, which a
//!   launcher that passes no `-Dlog4j.configurationFile` does not use.

use serde_json::{Map, Value};

use crate::launch::FILLED_TOKENS;

/// Translate one of Mojang's version files into the meta shape this launcher
/// reads, ready for [`VersionFile::parse`](crate::version::VersionFile::parse).
///
/// The component id and the version are *not* set here: Mojang's file names the
/// version in `id` and carries no component id, and the caller is the one that
/// knows which uid and version it asked for.
pub fn from_mojang(value: &Value) -> Result<Value, String> {
    let obj = value
        .as_object()
        .ok_or_else(|| "a version file is an object".to_string())?;
    let mut out = Map::new();
    let id = string_of(obj, "id");
    copy(obj, &mut out, &["type", "releaseTime", "mainClass", "assets", "assetIndex"]);
    if !id.is_empty() {
        out.insert("name".into(), Value::String(id.clone()));
    }

    // The game's arguments, in whichever shape this version published them.
    let minecraft_arguments = arguments_of(obj);
    if !minecraft_arguments.is_empty() {
        out.insert("minecraftArguments".into(), Value::String(minecraft_arguments.clone()));
    }

    if let Some(java) = obj.get("javaVersion").and_then(Value::as_object) {
        if let Some(major) = java.get("majorVersion").and_then(Value::as_i64) {
            out.insert("compatibleJavaMajors".into(), Value::Array(vec![Value::from(major)]));
        }
        if let Some(component) = java.get("component").and_then(Value::as_str) {
            out.insert("compatibleJavaName".into(), Value::String(component.to_string()));
        }
    }

    // The client jar. Mojang names it under `downloads` with no Maven coordinate
    // at all, so one is written: the same `com.mojang:minecraft:<id>:client` the
    // mirror's file uses, with the digest the manifest does not publish.
    if let Some(client) = obj
        .get("downloads")
        .and_then(|downloads| downloads.get("client"))
        .and_then(Value::as_object)
    {
        let artifact = obj_of(client, &["sha1", "size", "url"]);
        let mut downloads = Map::new();
        downloads.insert("artifact".into(), Value::Object(artifact));
        let name = if id.is_empty() {
            "com.mojang:minecraft:client".to_string()
        } else {
            format!("com.mojang:minecraft:{id}:client")
        };
        out.insert(
            "mainJar".into(),
            Value::Object(Map::from_iter([
                ("name".to_string(), Value::String(name)),
                ("downloads".to_string(), Value::Object(downloads)),
            ])),
        );
    }

    if let Some(libraries) = obj.get("libraries") {
        out.insert("libraries".into(), libraries.clone());
    }
    let traits = traits(obj, &minecraft_arguments);
    if !traits.is_empty() {
        out.insert(
            "+traits".into(),
            Value::Array(traits.into_iter().map(Value::String).collect()),
        );
    }
    Ok(Value::Object(out))
}

/// The game's arguments, in whichever shape the file published them, with the
/// ones this launcher cannot pass on removed.
///
/// The modern shape is a list where the plain strings are the arguments and the
/// objects carry `rules`: those are `--demo`, the quick-play destinations and the
/// window size, which the launcher turns into a trait rather than into an
/// argument here (`launch.rs` adds the flag itself when the instance asks for a
/// server or a world). A version whose arguments are all conditional has none, and
/// the launcher then starts the game with no arguments rather than with the wrong
/// ones.
///
/// The filter is one rule, applied to both shapes: an argument whose value is a
/// `${...}` token outside [`FILLED_TOKENS`] is dropped, and the flag in front of
/// it with it. Measured against the mirror for `1.21.4`, `1.21.1`, `1.19.4`,
/// `1.16.5`, `1.12.2` and `1.6.4`: the two documents carry the same argument
/// string, except that Prism drops that pair for every version that publishes it,
/// which is 1.19 and later. `clientid` and `auth_xuid` are the only tokens in
/// piston's list the launcher has no value for, and `launch.rs` substitutes an
/// unknown token with itself -- so keeping them would call the game with
/// `--clientId ${clientid}` as a literal string, and would read as a secret to
/// the launcher's own log redaction.
fn arguments_of(obj: &Map<String, Value>) -> String {
    if let Some(legacy) = obj.get("minecraftArguments").and_then(Value::as_str) {
        // One published string, split the way the launch path splits it.
        return fillable(legacy.split(' '));
    }
    let Some(args) = obj
        .get("arguments")
        .and_then(|arguments| arguments.get("game"))
        .and_then(Value::as_array)
    else {
        return String::new();
    };
    fillable(args.iter().filter_map(Value::as_str))
}

/// `args` as one string, without the arguments whose value is a token no launch
/// of this launcher fills.
fn fillable<'a>(args: impl Iterator<Item = &'a str>) -> String {
    let mut kept: Vec<&str> = Vec::new();
    for arg in args.filter(|arg| !arg.is_empty()) {
        if let Some(token) = placeholder(arg) {
            if !FILLED_TOKENS.contains(&token) {
                // The flag the value belongs to goes with it, so the game never
                // sees a `--clientId` with nothing after it. A value with no flag
                // in front is still dropped on its own.
                if kept.last().is_some_and(|last| last.starts_with("--")) {
                    kept.pop();
                }
                continue;
            }
        }
        kept.push(arg);
    }
    kept.join(" ")
}

/// The token an argument *is*, when the argument is nothing but one.
fn placeholder(arg: &str) -> Option<&str> {
    arg.strip_prefix("${")?.strip_suffix('}')
}

/// The launcher behaviours this version needs, derived from the file itself.
///
/// Each one is Prism's own trait, kept under its own name because the shell reads
/// them: they are what the launch path keys platform workarounds and the
/// quick-play arguments off.
///
/// Measured against the mirror for `1.21.4`, `1.21.1`, `1.19.4`, `1.16.5`,
/// `1.12.2` and `1.6.4`, and asserted against it live for `1.21.1` -- the traits
/// derived here are the mirror's own list with one entry missing: `XR:Initial`,
/// which is about a launcher feature this one does not offer. Everything else it
/// sets is derived: the quick-play *features* of the conditional arguments arrive
/// as traits rather than as arguments, because `launch.rs` writes those flags
/// itself when a server or a world is asked for.
fn traits(obj: &Map<String, Value>, minecraft_arguments: &str) -> Vec<String> {
    let mut traits = Vec::new();
    if has_lwjgl3(obj) {
        // Prism carries this for every version that loads LWJGL 3 -- 1.13 and on
        // -- and not for the ones that load LWJGL 2. It is what makes the
        // launcher start the game on the first thread, which macOS requires.
        traits.push("FirstThreadOnMacOS".to_string());
    }
    if minecraft_arguments.contains("${auth_session}") {
        // The legacy session argument is the mark of a version that predates the
        // access token: those need `onlineFixes` to be told apart from an offline
        // session.
        traits.push("legacyServices".to_string());
    }
    for feature in game_features(obj) {
        match feature.as_str() {
            "is_quick_play_multiplayer" => traits.push("feature:is_quick_play_multiplayer".to_string()),
            "is_quick_play_singleplayer" => {
                traits.push("feature:is_quick_play_singleplayer".to_string())
            }
            _ => {}
        }
    }
    traits
}

/// Whether the version loads LWJGL 3, which is a fact about its libraries.
fn has_lwjgl3(obj: &Map<String, Value>) -> bool {
    let Some(libraries) = obj.get("libraries").and_then(Value::as_array) else {
        return false;
    };
    libraries.iter().any(|library| {
        library
            .get("name")
            .and_then(Value::as_str)
            .map(|name| name.starts_with("org.lwjgl:"))
            .unwrap_or(false)
    })
}

/// The feature names the conditional `arguments.game` entries ask about.
fn game_features(obj: &Map<String, Value>) -> Vec<String> {
    let Some(args) = obj
        .get("arguments")
        .and_then(|arguments| arguments.get("game"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    let mut features = Vec::new();
    for entry in args.iter().filter_map(Value::as_object) {
        let Some(rules) = entry.get("rules").and_then(Value::as_array) else {
            continue;
        };
        for rule in rules {
            let Some(features_in_rule) = rule.get("features").and_then(Value::as_object) else {
                continue;
            };
            for (name, enabled) in features_in_rule {
                if enabled.as_bool().unwrap_or(false) && !features.contains(name) {
                    features.push(name.clone());
                }
            }
        }
    }
    features
}

/// An object holding only the named keys of `from`, in the order given.
fn obj_of(from: &Map<String, Value>, keys: &[&str]) -> Map<String, Value> {
    let mut out = Map::new();
    for key in keys {
        if let Some(value) = from.get(*key) {
            out.insert((*key).to_string(), value.clone());
        }
    }
    out
}

/// Copy whichever of `keys` are present.
fn copy(from: &Map<String, Value>, to: &mut Map<String, Value>, keys: &[&str]) {
    for key in keys {
        if let Some(value) = from.get(*key) {
            to.insert((*key).to_string(), value.clone());
        }
    }
}

/// A string field, or empty when it is absent or not a string.
fn string_of(obj: &Map<String, Value>, key: &str) -> String {
    obj.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::version::VersionFile;
    use std::path::Path;

    /// Trimmed from piston's `1.21.4.json`: the shape 1.13 and on publish, where
    /// the game's arguments are an object, the natives are their own entries, and
    /// the client jar is a `downloads` entry rather than a library. The
    /// `--clientId` pair is the real file's, and is what the translation drops.
    const MODERN: &str = r#"{
      "id": "1.21.4", "type": "release", "releaseTime": "2024-12-03T10:12:57+00:00",
      "mainClass": "net.minecraft.client.main.Main",
      "assets": "19",
      "assetIndex": {"id": "19", "sha1": "aa", "size": 10, "totalSize": 20,
                     "url": "https://piston-meta.mojang.com/19.json"},
      "javaVersion": {"component": "java-runtime-delta", "majorVersion": 21},
      "downloads": {"client": {"sha1": "bb", "size": 28335587,
                               "url": "https://piston-data.mojang.com/client.jar"}},
      "arguments": {"jvm": ["-Djava.library.path=${natives_directory}", "-cp", "${classpath}"],
                    "game": ["--username", "${auth_player_name}", "--version", "${version_name}",
                             "--clientId", "${clientid}", "--xuid", "${auth_xuid}",
                             {"rules": [{"action": "allow", "features": {"is_demo_user": true}}],
                              "value": "--demo"},
                             {"rules": [{"action": "allow",
                                         "features": {"is_quick_play_multiplayer": true}}],
                              "value": "--quickPlayMultiplayer"}]},
      "libraries": [
        {"name": "com.mojang:brigadier:1.3.10",
         "downloads": {"artifact": {"path": "com/mojang/brigadier/1.3.10/brigadier-1.3.10.jar",
                                    "sha1": "cc", "size": 5, "url": "https://libraries/brigadier.jar"}}},
        {"name": "org.lwjgl:lwjgl:3.3.3",
         "downloads": {"artifact": {"path": "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3.jar",
                                    "sha1": "dd", "size": 6, "url": "https://libraries/lwjgl.jar"}}},
        {"name": "org.lwjgl:lwjgl:3.3.3:natives-windows",
         "rules": [{"action": "allow", "os": {"name": "windows"}}],
         "downloads": {"artifact": {"path": "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar",
                                    "sha1": "ee", "size": 7, "url": "https://libraries/lwjgl-natives.jar"}}}
      ]
    }"#;

    /// Trimmed from piston's `1.6.4.json`: the shape before 1.13, where the
    /// arguments are one legacy string, the java version is not stated, and the
    /// client jar is a library with a `natives` map beside it.
    const LEGACY: &str = r#"{
      "id": "1.6.4", "type": "release", "releaseTime": "2013-09-19T00:00:00+00:00",
      "mainClass": "net.minecraft.client.main.Main",
      "assets": "legacy",
      "assetIndex": {"id": "legacy", "sha1": "ff", "size": 8, "totalSize": 9,
                     "url": "https://piston-meta.mojang.com/legacy.json"},
      "downloads": {"client": {"sha1": "gg", "size": 100,
                               "url": "https://piston-data.mojang.com/1.6.4.jar"}},                    "minecraftArguments": "--username ${auth_player_name} --session ${auth_session} --gameDir ${game_directory}",
      "libraries": [
        {"name": "org.lwjgl.lwjgl:lwjgl:2.9.0",
         "downloads": {"artifact": {"path": "org/lwjgl/lwjgl/2.9.0/lwjgl-2.9.0.jar",
                                    "sha1": "hh", "size": 11, "url": "https://libraries/lwjgl2.jar"},
                       "classifiers": {"natives-windows": {"path": "org/lwjgl/lwjgl/2.9.0/lwjgl-2.9.0-natives-windows.jar",
                                                           "sha1": "ii", "size": 12,
                                                           "url": "https://libraries/lwjgl2-natives.jar"}}},
         "natives": {"windows": "natives-windows", "linux": "natives-linux"}}
      ]
    }"#;

    fn parsed(body: &str) -> VersionFile {
        let value: Value = serde_json::from_str(body).expect("the fixture parses");
        let translated = from_mojang(&value).expect("it translates");
        VersionFile::parse(&translated, Path::new("1.21.4.json"), false).expect("it parses")
    }

    #[test]
    fn a_modern_file_keeps_its_arguments_main_class_and_client_jar() {
        let file = parsed(MODERN);
        assert_eq!(file.main_class, "net.minecraft.client.main.Main");
        assert_eq!(
            file.minecraft_arguments,
            "--username ${auth_player_name} --version ${version_name}",
            "the plain strings, in order; the conditional entries are not arguments, \
             and the `--clientId` pair is not a pair this launcher can fill"
        );
        assert!(!file.minecraft_arguments.contains("--demo"));
        let jar = file.main_jar.as_ref().expect("a client jar");
        assert_eq!(
            format!("{}:{}:{}", jar.name.group(), jar.name.artifact(), jar.name.version()),
            "com.mojang:minecraft:1.21.4"
        );
        assert_eq!(jar.name.classifier(), "client");
        let artifact = jar.mojang_downloads.as_ref().and_then(|d| d.artifact.as_ref()).expect("its artifact");
        assert_eq!(artifact.url, "https://piston-data.mojang.com/client.jar");
        assert_eq!(artifact.sha1, "bb");
        assert_eq!(artifact.size, 28335587);
    }

    #[test]
    fn a_modern_file_states_the_java_it_needs_and_the_assets_it_wants() {
        let file = parsed(MODERN);
        assert_eq!(file.compatible_java_majors, vec![21]);
        assert_eq!(file.compatible_java_name, "java-runtime-delta");
        assert_eq!(file.assets, "19");
        let index = file.asset_index.as_ref().expect("an asset index");
        assert_eq!(index.id, "19");
        assert_eq!(index.url, "https://piston-meta.mojang.com/19.json");
    }

    #[test]
    fn the_traits_are_derived_from_the_file_rather_than_guessed() {
        let modern = parsed(MODERN);
        assert!(modern.traits.contains("FirstThreadOnMacOS"), "{:?}", modern.traits);
        assert!(
            modern.traits.contains("feature:is_quick_play_multiplayer"),
            "the conditional argument's feature is kept as a trait: {:?}",
            modern.traits
        );
        assert!(!modern.traits.contains("legacyServices"));
        assert!(!modern.traits.contains("feature:is_demo_user"), "the demo flag is not offered");

        let legacy = parsed(LEGACY);
        assert!(legacy.traits.contains("legacyServices"), "{:?}", legacy.traits);
        assert!(
            !legacy.traits.contains("FirstThreadOnMacOS"),
            "1.6.4 loads LWJGL 2, which is what the mirror says too: {:?}",
            legacy.traits
        );
    }

    #[test]
    fn a_legacy_file_keeps_its_own_argument_string_untouched() {
        let file = parsed(LEGACY);
        assert!(file.minecraft_arguments.contains("--session ${auth_session}"));
        assert_eq!(file.assets, "legacy");
        assert!(file.compatible_java_majors.is_empty(), "1.6.4 states no java version");
        assert_eq!(file.libraries.len(), 1);
        // The classic natives map is untouched, so the classifier still names the
        // native for this host.
        let natives = &file.libraries[0].native_classifiers;
        assert_eq!(natives.get("windows").map(String::as_str), Some("natives-windows"));
    }

    #[test]
    fn an_argument_this_launcher_cannot_fill_is_dropped_with_its_flag() {
        // The pair 1.19 and later publish, against the same prices `launch.rs`
        // pays: `${clientid}` and `${auth_xuid}` are not tokens a launch of this
        // launcher substitutes, and `replace_tokens` leaves an unknown token
        // alone, so the pair would go to the game as literal text -- and would
        // read to the log redaction as two secrets it has to hide. Prism's file
        // for every version that publishes it has neither the flag nor the value.
        let file = parsed(MODERN);
        for gone in ["--clientId", "${clientid}", "--xuid", "${auth_xuid}"] {
            assert!(
                !file.minecraft_arguments.contains(gone),
                "{gone} survived: {}",
                file.minecraft_arguments
            );
        }
        // The arguments on either side of the pair are still there, in order: it
        // is two arguments that go, not the tail of the string.
        assert!(file.minecraft_arguments.starts_with("--username ${auth_player_name}"));
        assert!(file.minecraft_arguments.contains("--version ${version_name}"));
    }

    #[test]
    fn a_native_that_is_its_own_entry_is_a_native() {
        // The shape 1.19 introduced, and the reason `is_native` has two rules:
        // without the classifier rule this entry is an ordinary jar and its
        // shared objects are never extracted.
        let file = parsed(MODERN);
        let natives: Vec<&crate::version::Library> = file
            .libraries
            .iter()
            .filter(|library| library.is_native())
            .collect();
        assert_eq!(natives.len(), 1, "{:?}", file.libraries.iter().map(|l| l.name.serialize()).collect::<Vec<_>>());
        assert_eq!(natives[0].name.classifier(), "natives-windows");
        assert!(file.libraries.iter().any(|library| !library.is_native()));
    }

    #[test]
    fn a_body_that_is_not_an_object_is_refused() {
        assert!(from_mojang(&Value::Array(Vec::new())).is_err());
    }
}
