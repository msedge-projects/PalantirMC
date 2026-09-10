//! Version file model — port of `OneSixVersionFormat.cpp` (OneSix/meta
//! format) plus the Mojang property block of `MojangVersionFormat.cpp`.
//!
//! Accepted JSON (meta v0/v1 or missing `formatVersion`):
//! `name`/`uid`|`fileId`/`version`, `order`, Mojang properties (`id`,
//! `mainClass`, `appletClass`, `minecraftArguments`, `type`, `assets`,
//! `assetIndex`, `releaseTime`, `time`, `minimumLauncherVersion`,
//! `compatibleJavaMajors`, `compatibleJavaName`, `downloads`),
//! `+tweakers`, `+traits`, `+jvmArgs`, `jarMods`/`+jarMods` (deprecated),
//! `mods`, `libraries`/`+libraries`, `mavenFiles`, `+agents`, `mainJar`,
//! `requires`, `conflicts`, `mcVersion`, `volatile`, `runtimes`.
//! Legacy elements `tweakers`, `-libraries`, `-tweakers`,
//! `-minecraftArguments`, `+minecraftArguments` raise `Error` problems.

use super::library::{download_info_from_json, Library};
use super::{Agent, AssetIndexInfo, Problem, ProblemSeverity};
use crate::error::{Error, Result};
use crate::json;
use crate::pack::Require;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Highest `minimumLauncherVersion` Prism accepts without a warning.
pub const CURRENT_MINIMUM_LAUNCHER_VERSION: i64 = 18;

/// A parsed version file (metadata component).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VersionFile {
    /// Patch order (0 when absent).
    pub order: i64,
    /// Whether an explicit `order` was present.
    pub has_order: bool,
    /// Display name.
    pub name: String,
    /// Component uid (`uid` or legacy `fileId`).
    pub uid: String,
    /// Component version.
    pub version: String,
    /// Minecraft version (`id`).
    pub minecraft_version: String,
    /// Main class override.
    pub main_class: String,
    /// Legacy applet class.
    pub applet_class: String,
    /// Version type (`release`, `snapshot`, ...).
    pub type_: String,
    /// Bare asset index name (`assets`).
    pub assets: String,
    /// Full asset index descriptor.
    pub asset_index: Option<AssetIndexInfo>,
    /// Legacy `minecraftArguments` string.
    pub minecraft_arguments: String,
    /// `releaseTime` (raw S3 timestamp string).
    pub release_time: String,
    /// `time` (raw S3 timestamp string).
    pub update_time: String,
    /// `minimumLauncherVersion`, -1 when absent.
    pub minimum_launcher_version: i64,
    /// `compatibleJavaMajors`.
    pub compatible_java_majors: Vec<i64>,
    /// `compatibleJavaName`.
    pub compatible_java_name: String,
    /// `downloads` classifier map.
    pub mojang_downloads: BTreeMap<String, super::library::DownloadInfo>,
    /// `libraries` entries.
    pub libraries: Vec<Library>,
    /// `mavenFiles` entries (downloaded, not on the classpath).
    pub maven_files: Vec<Library>,
    /// `jarMods` entries.
    pub jar_mods: Vec<Library>,
    /// `mods` entries.
    pub mods: Vec<Library>,
    /// `+agents` entries.
    pub agents: Vec<Agent>,
    /// `+tweakers` entries.
    pub add_tweakers: Vec<String>,
    /// `+traits` set.
    pub traits: BTreeSet<String>,
    /// `+jvmArgs` entries.
    pub addn_jvm_arguments: Vec<String>,
    /// Main jar override.
    pub main_jar: Option<Library>,
    /// `requires` set.
    pub requires: BTreeSet<Require>,
    /// `conflicts` set.
    pub conflicts: BTreeSet<Require>,
    /// `volatile` flag.
    pub volatile: bool,
    /// `runtimes` entries (raw; interpreted by the Java module later).
    pub runtimes: Vec<Value>,
    /// Problems found while parsing.
    pub problems: Vec<Problem>,
}

impl VersionFile {
    /// Highest problem severity recorded on this file.
    pub fn problem_severity(&self) -> ProblemSeverity {
        self.problems.iter().map(|p| p.severity).max().unwrap_or(ProblemSeverity::None)
    }

    /// Parse a OneSix/meta version file from JSON.
    ///
    /// `require_order` mirrors `OneSixVersionFormat::versionFileFromJson`:
    /// meta-driven files (which carry `order`) demand it, patch files do not.
    pub fn parse(value: &Value, path: &Path, require_order: bool) -> Result<VersionFile> {
        let obj = value.as_object().ok_or_else(|| {
            Error::json(path, "version file is not an object")
        })?;
        // formatVersion: absent ok (non-required parse), 0/1 ok, else invalid.
        if let Some(fv) = obj.get("formatVersion") {
            let n = fv.as_i64().ok_or_else(|| {
                Error::format(path, "does not contain a recognizable version of the metadata format")
            })?;
            if n != 0 && n != 1 {
                return Err(Error::format(
                    path,
                    "does not contain a recognizable version of the metadata format",
                ));
            }
        }

        let mut out = VersionFile::default();
        if require_order {
            if let Some(o) = obj.get("order") {
                out.has_order = true;
                out.order = o.as_i64().ok_or_else(|| Error::json(path, "'order' must be an integer"))?;
            } else {
                out.problems.push(Problem {
                    severity: ProblemSeverity::Warning,
                    message: "version file doesn't contain an order field".into(),
                });
            }
        }
        out.name = str_of(obj, "name");
        out.uid = match obj.get("uid").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => str_of(obj, "fileId"),
        };
        if !uid_valid(&out.uid) {
            out.problems.push(Problem {
                severity: ProblemSeverity::Error,
                message: format!("The component's 'uid' contains illegal characters! UID: {}", out.uid),
            });
        }
        out.version = str_of(obj, "version");
        read_version_properties(obj, &mut out, path)?;
        out.applet_class = str_of(obj, "appletClass");

        if let Some(arr) = opt_array(obj, "+tweakers") {
            out.add_tweakers = string_array(arr, path)?;
        }
        if let Some(arr) = opt_array(obj, "+traits") {
            for t in string_array(arr, path)? {
                out.traits.insert(t);
            }
        }
        if let Some(arr) = opt_array(obj, "+jvmArgs") {
            out.addn_jvm_arguments = string_array(arr, path)?;
        }
        if let Some(arr) = opt_array(obj, "jarMods") {
            for v in arr {
                out.jar_mods.push(Library::from_json(v, &mut out.problems)?);
            }
        } else if let Some(arr) = opt_array(obj, "+jarMods") {
            // Deprecated style: {"name": <filename>, "originalName": ...?}
            for v in arr {
                let o = v
                    .as_object()
                    .ok_or_else(|| Error::json(path, "contains a jarmod that isn't an object"))?;
                let file_name = o
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::json(path, "contains a jarmod that doesn't have a 'name' field"))?
                    .to_string();
                let mut lib = Library::default();
                lib.name = super::GradleSpecifier::parse(&format!(
                    "org.multimc.jarmods:{}:1",
                    uuid::Uuid::new_v4().simple()
                ));
                lib.filename_override = file_name.clone();
                lib.hint = "local".into();
                let original = str_of(o, "originalName");
                lib.displayname = if original.is_empty() {
                    out.name.trim_end_matches(" (jar mod)").to_string()
                } else {
                    original
                };
                out.jar_mods.push(lib);
            }
        }
        if let Some(arr) = opt_array(obj, "mods") {
            for v in arr {
                out.mods.push(Library::from_json(v, &mut out.problems)?);
            }
        }
        let has_plus_libs = obj.contains_key("+libraries");
        let has_libs = obj.contains_key("libraries");
        if has_plus_libs && has_libs {
            out.problems.push(Problem {
                severity: ProblemSeverity::Warning,
                message: "Version file has both '+libraries' and 'libraries'. This is no longer supported.".into(),
            });
            for v in opt_array(obj, "libraries").into_iter().flatten() {
                out.libraries.push(Library::from_json(v, &mut out.problems)?);
            }
            for v in opt_array(obj, "+libraries").into_iter().flatten() {
                out.libraries.push(Library::from_json(v, &mut out.problems)?);
            }
        } else if has_libs {
            for v in opt_array(obj, "libraries").into_iter().flatten() {
                out.libraries.push(Library::from_json(v, &mut out.problems)?);
            }
        } else if has_plus_libs {
            for v in opt_array(obj, "+libraries").into_iter().flatten() {
                out.libraries.push(Library::from_json(v, &mut out.problems)?);
            }
        }
        if let Some(arr) = opt_array(obj, "mavenFiles") {
            for v in arr {
                out.maven_files.push(Library::from_json(v, &mut out.problems)?);
            }
        }
        if let Some(arr) = opt_array(obj, "+agents") {
            for v in arr {
                let o = v
                    .as_object()
                    .ok_or_else(|| Error::json(path, "agent entry must be an object"))?;
                let library = Library::from_json(v, &mut out.problems)?;
                let argument = str_of(o, "argument");
                out.agents.push(Agent { library, argument });
            }
        }
        if let Some(v) = obj.get("mainJar") {
            out.main_jar = Some(Library::from_json(v, &mut out.problems)?);
        } else if !out.minecraft_version.is_empty() {
            let mut lib = Library::default();
            lib.name = super::GradleSpecifier::parse(&format!(
                "com.mojang:minecraft:{}:client",
                out.minecraft_version
            ));
            if let Some(client) = out.mojang_downloads.get("client") {
                let mut downloads = super::library::LibraryDownloads::default();
                downloads.artifact = Some(client.clone());
                lib.mojang_downloads = Some(downloads);
            } else {
                out.problems.push(Problem {
                    severity: ProblemSeverity::Error,
                    message: "URL for the main jar could not be determined - Mojang removed the server that we used as fallback.".into(),
                });
            }
            out.main_jar = Some(lib);
        }
        if let Some(arr) = opt_array(obj, "requires") {
            for v in arr {
                if let Some(r) = Require::from_json(v) {
                    out.requires.insert(r);
                }
            }
        }
        let mc_version_req = str_of(obj, "mcVersion");
        if !mc_version_req.is_empty() {
            out.requires.insert(Require {
                uid: "net.minecraft".into(),
                equals_version: mc_version_req,
                suggests: String::new(),
            });
        }
        if let Some(arr) = opt_array(obj, "conflicts") {
            for v in arr {
                if let Some(r) = Require::from_json(v) {
                    out.conflicts.insert(r);
                }
            }
        }
        if let Some(v) = obj.get("volatile") {
            out.volatile = v.as_bool().ok_or_else(|| Error::json(path, "'volatile' must be a boolean"))?;
        }
        if let Some(arr) = opt_array(obj, "runtimes") {
            out.runtimes = arr.clone();
        }
        // Unsupported legacy elements -> Error problems.
        for banned in ["tweakers", "-libraries", "-tweakers", "-minecraftArguments", "+minecraftArguments"] {
            if obj.contains_key(banned) {
                out.problems.push(Problem {
                    severity: ProblemSeverity::Error,
                    message: format!("Version file contains unsupported element '{banned}'"),
                });
            }
        }
        Ok(out)
    }

    /// Parse a raw Mojang client.json (`MojangVersionFormat::versionFileFromJson`).
    pub fn parse_mojang(value: &Value, path: &Path) -> Result<VersionFile> {
        let mut out = VersionFile::parse(value, path, false)?;
        out.name = "Minecraft".into();
        out.uid = "net.minecraft".into();
        out.version = out.minecraft_version.clone();
        let obj = value.as_object().ok_or_else(|| Error::json(path, "not an object"))?;
        if let Some(arr) = opt_array(obj, "libraries") {
            for v in arr {
                out.libraries.push(Library::from_json(v, &mut out.problems)?);
            }
        }
        Ok(out)
    }

    /// Serialize as a patch file (`OneSixVersionFormat::versionFileToJson`).
    /// Trait ordering is sorted here; Prism iterates a `QSet` (unspecified
    /// order).
    pub fn to_json(&self) -> Value {
        let mut root = serde_json::Map::new();
        put_str(&mut root, "name", &self.name);
        put_str(&mut root, "uid", &self.uid);
        put_str(&mut root, "version", &self.version);
        root.insert("formatVersion".into(), Value::from(1));
        write_version_properties(&mut root, self);
        if let Some(jar) = &self.main_jar {
            root.insert("mainJar".into(), jar.to_json());
        }
        put_str(&mut root, "appletClass", &self.applet_class);
        put_str_list(&mut root, "+tweakers", &self.add_tweakers);
        let traits: Vec<String> = self.traits.iter().cloned().collect();
        put_str_list(&mut root, "+traits", &traits);
        put_str_list(&mut root, "+jvmArgs", &self.addn_jvm_arguments);
        if !self.agents.is_empty() {
            let arr: Vec<Value> = self
                .agents
                .iter()
                .map(|a| {
                    let mut o = match a.library.to_json() {
                        Value::Object(o) => o,
                        _ => serde_json::Map::new(),
                    };
                    if !a.argument.is_empty() {
                        o.insert("argument".into(), Value::String(a.argument.clone()));
                    }
                    Value::Object(o)
                })
                .collect();
            root.insert("+agents".into(), Value::Array(arr));
        }
        if !self.libraries.is_empty() {
            root.insert(
                "libraries".into(),
                Value::Array(self.libraries.iter().map(Library::to_json).collect()),
            );
        }
        if !self.maven_files.is_empty() {
            root.insert(
                "mavenFiles".into(),
                Value::Array(self.maven_files.iter().map(Library::to_json).collect()),
            );
        }
        if !self.jar_mods.is_empty() {
            root.insert(
                "jarMods".into(),
                Value::Array(self.jar_mods.iter().map(Library::to_json).collect()),
            );
        }
        if !self.mods.is_empty() {
            root.insert(
                "mods".into(),
                Value::Array(self.mods.iter().map(Library::to_json).collect()),
            );
        }
        if !self.requires.is_empty() {
            root.insert(
                "requires".into(),
                Value::Array(self.requires.iter().map(Require::to_json).collect()),
            );
        }
        if !self.conflicts.is_empty() {
            root.insert(
                "conflicts".into(),
                Value::Array(self.conflicts.iter().map(Require::to_json).collect()),
            );
        }
        if self.volatile {
            root.insert("volatile".into(), Value::Bool(true));
        }
        Value::Object(root)
    }

    /// Serialize to the QJsonDocument byte format (patch files).
    pub fn to_document_text(&self) -> Result<String> {
        json::to_document_string(&self.to_json())
    }
}

/// Prism uid pattern: `[a-zA-Z0-9-_]+(\.[a-zA-Z0-9-_]+)*`.
pub fn uid_valid(uid: &str) -> bool {
    if uid.is_empty() {
        return false;
    }
    for segment in uid.split('.') {
        if segment.is_empty() {
            return false;
        }
        if !segment.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return false;
        }
    }
    true
}

fn str_of(obj: &Map<String, Value>, key: &str) -> String {
    obj.get(key).and_then(|v| v.as_str()).unwrap_or_default().to_string()
}

fn put_str(map: &mut serde_json::Map<String, Value>, key: &str, value: &str) {
    if !value.is_empty() {
        map.insert(key.to_string(), Value::String(value.to_string()));
    }
}

fn put_str_list(map: &mut serde_json::Map<String, Value>, key: &str, list: &[String]) {
    if !list.is_empty() {
        map.insert(key.to_string(), Value::Array(list.iter().map(|s| Value::String(s.clone())).collect()));
    }
}

fn opt_array<'a>(obj: &'a Map<String, Value>, key: &str) -> Option<&'a Vec<Value>> {
    obj.get(key).and_then(|v| v.as_array())
}

fn string_array(arr: &[Value], path: &Path) -> Result<Vec<String>> {
    arr.iter()
        .map(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| Error::json(path, "expected a string in array"))
        })
        .collect()
}

/// `MojangVersionFormat::readVersionProperties`.
fn read_version_properties(obj: &Map<String, Value>, out: &mut VersionFile, path: &Path) -> Result<()> {
    out.minecraft_version = str_of(obj, "id");
    out.main_class = str_of(obj, "mainClass");
    out.minecraft_arguments = str_of(obj, "minecraftArguments");
    out.type_ = str_of(obj, "type");
    out.assets = str_of(obj, "assets");
    if let Some(ai) = obj.get("assetIndex") {
        let o = ai
            .as_object()
            .ok_or_else(|| Error::json(path, "'assetIndex' must be an object"))?;
        let require = |key: &str| -> Result<String> {
            o.get(key)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| Error::json(path, format!("missing '{key}' in assetIndex")))
        };
        let total_size = o
            .get("totalSize")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| Error::json(path, "missing 'totalSize' in assetIndex"))?;
        out.asset_index = Some(AssetIndexInfo {
            path: o.get("path").and_then(|v| v.as_str()).map(|s| s.to_string()),
            sha1: require("sha1")?,
            size: o
                .get("size")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| Error::json(path, "missing 'size' in assetIndex"))?,
            url: require("url")?,
            total_size,
            id: require("id")?,
            known: true,
        });
    } else if !out.assets.is_empty() {
        out.asset_index = Some(AssetIndexInfo::bare(&out.assets));
    }
    out.release_time = str_of(obj, "releaseTime");
    out.update_time = str_of(obj, "time");
    if let Some(v) = obj.get("minimumLauncherVersion") {
        out.minimum_launcher_version = v.as_i64().ok_or_else(|| Error::json(path, "'minimumLauncherVersion' must be an integer"))?;
        if out.minimum_launcher_version > CURRENT_MINIMUM_LAUNCHER_VERSION {
            out.problems.push(Problem {
                severity: ProblemSeverity::Warning,
                message: format!(
                    "The 'minimumLauncherVersion' value of this version ({}) is higher than supported ({}). It might not work properly!",
                    out.minimum_launcher_version, CURRENT_MINIMUM_LAUNCHER_VERSION
                ),
            });
        }
    } else {
        out.minimum_launcher_version = -1;
    }
    if let Some(arr) = opt_array(obj, "compatibleJavaMajors") {
        for v in arr {
            out.compatible_java_majors
                .push(v.as_i64().ok_or_else(|| Error::json(path, "'compatibleJavaMajors' must contain integers"))?);
        }
    }
    out.compatible_java_name = str_of(obj, "compatibleJavaName");
    if let Some(dl) = obj.get("downloads").and_then(|v| v.as_object()) {
        for (k, v) in dl {
            out.mojang_downloads.insert(k.clone(), download_info_from_json(v)?);
        }
    }
    Ok(())
}

/// `MojangVersionFormat::writeVersionProperties`.
fn write_version_properties(root: &mut serde_json::Map<String, Value>, file: &VersionFile) {
    put_str(root, "id", &file.minecraft_version);
    put_str(root, "mainClass", &file.main_class);
    put_str(root, "minecraftArguments", &file.minecraft_arguments);
    put_str(root, "type", &file.type_);
    if !file.release_time.is_empty() {
        root.insert("releaseTime".into(), Value::String(file.release_time.clone()));
    }
    if !file.update_time.is_empty() {
        root.insert("time".into(), Value::String(file.update_time.clone()));
    }
    if file.minimum_launcher_version != -1 {
        root.insert("minimumLauncherVersion".into(), Value::from(file.minimum_launcher_version));
    }
    put_str(root, "assets", &file.assets);
    if let Some(ai) = &file.asset_index {
        if ai.known {
            let mut o = serde_json::Map::new();
            if let Some(p) = &ai.path {
                o.insert("path".into(), Value::String(p.clone()));
            }
            o.insert("sha1".into(), Value::String(ai.sha1.clone()));
            o.insert("size".into(), Value::from(ai.size));
            o.insert("url".into(), Value::String(ai.url.clone()));
            o.insert("totalSize".into(), Value::from(ai.total_size));
            o.insert("id".into(), Value::String(ai.id.clone()));
            root.insert("assetIndex".into(), Value::Object(o));
        }
    }
    if !file.mojang_downloads.is_empty() {
        let mut o = serde_json::Map::new();
        for (k, v) in &file.mojang_downloads {
            o.insert(k.clone(), super::library::download_info_to_json(v));
        }
        root.insert("downloads".into(), Value::Object(o));
    }
    if !file.compatible_java_majors.is_empty() {
        root.insert(
            "compatibleJavaMajors".into(),
            Value::Array(file.compatible_java_majors.iter().map(|m| Value::from(*m)).collect()),
        );
    }
    put_str(root, "compatibleJavaName", &file.compatible_java_name);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse_value(v: Value) -> Result<VersionFile> {
        VersionFile::parse(&v, Path::new("test.json"), false)
    }

    #[test]
    fn parses_meta_minecraft_version_file() {
        let v = json!({
            "assets": "17",
            "assetIndex": {"id": "17", "sha1": "s", "size": 1, "totalSize": 2, "url": "https://u/17.json"},
            "formatVersion": 1,
            "libraries": [{"name": "org.lwjgl3:lwjgl:3.3.2"}],
            "mainClass": "net.minecraft.client.main.Main",
            "mainJar": {"name": "com.mojang:minecraft:1.20.4:client",
                         "downloads": {"artifact": {"sha1": "a", "url": "https://j", "size": 9}}},
            "minecraftArguments": "--username ${auth_player_name}",
            "name": "Minecraft",
            "releaseTime": "2023-12-07T13:14:44+00:00",
            "requires": [{"uid": "org.lwjgl3", "suggests": "3.3.2"}],
            "type": "release",
            "uid": "net.minecraft",
            "version": "1.20.4"
        });
        let f = parse_value(v).unwrap();
        assert_eq!(f.uid, "net.minecraft");
        assert_eq!(f.version, "1.20.4");
        assert_eq!(f.main_class, "net.minecraft.client.main.Main");
        assert_eq!(f.minecraft_arguments, "--username ${auth_player_name}");
        assert_eq!(f.assets, "17");
        let ai = f.asset_index.as_ref().unwrap();
        assert!(ai.known && ai.id == "17" && ai.total_size == 2);
        assert_eq!(f.requires.len(), 1);
        assert!(f.main_jar.is_some());
        assert_eq!(f.problem_severity(), ProblemSeverity::None);
    }

    #[test]
    fn mc_version_implies_net_minecraft_equals_requirement() {
        let f = parse_value(json!({"uid": "net.minecraftforge", "version": "47.2.0", "mcVersion": "1.20.1"}))
            .unwrap();
        let req = f.requires.iter().find(|r| r.uid == "net.minecraft").unwrap();
        assert_eq!(req.equals_version, "1.20.1");
    }

    #[test]
    fn file_id_fallback_and_uid_validation() {
        let f = parse_value(json!({"fileId": "custom.patch", "name": "P"})).unwrap();
        assert_eq!(f.uid, "custom.patch");
        assert!(uid_valid("custom.patch"));
        assert!(uid_valid("net.fabricmc.fabric-loader"));
        assert!(!uid_valid("has space"));
        assert!(!uid_valid("slash/inside"));
        assert!(!uid_valid(""));
        let bad = parse_value(json!({"uid": "bad uid"})).unwrap();
        assert_eq!(bad.problem_severity(), ProblemSeverity::Error);
    }

    #[test]
    fn banned_legacy_elements_produce_error_problems() {
        let f = parse_value(json!({"uid": "x", "tweakers": ["a"], "-libraries": []})).unwrap();
        assert_eq!(f.problem_severity(), ProblemSeverity::Error);
        assert_eq!(f.problems.len(), 2);
    }

    #[test]
    fn both_libraries_forms_warn_and_both_are_read() {
        let f = parse_value(json!({
            "uid": "x",
            "libraries": [{"name": "a:b:1"}],
            "+libraries": [{"name": "c:d:2"}]
        }))
        .unwrap();
        assert_eq!(f.problem_severity(), ProblemSeverity::Warning);
        assert_eq!(f.libraries.len(), 2);
    }

    #[test]
    fn main_jar_reconstructed_from_mojang_downloads() {
        let f = parse_value(json!({
            "id": "1.20.4",
            "downloads": {"client": {"sha1": "s", "url": "https://client", "size": 1}}
        }))
        .unwrap();
        let jar = f.main_jar.unwrap();
        assert_eq!(jar.name.serialize(), "com.mojang:minecraft:1.20.4:client");
        assert_eq!(jar.mojang_downloads.unwrap().artifact.unwrap().url, "https://client");
    }

    #[test]
    fn main_jar_reconstruction_without_client_download_errors() {
        let f = parse_value(json!({"id": "1.20.4"})).unwrap();
        assert_eq!(f.problem_severity(), ProblemSeverity::Error);
    }

    #[test]
    fn assets_without_asset_index_yields_bare_index() {
        let f = parse_value(json!({"id": "b1.7.3", "assets": "legacy"})).unwrap();
        let ai = f.asset_index.unwrap();
        assert!(!ai.known);
        assert_eq!(ai.id, "legacy");
    }

    #[test]
    fn format_version_enforced() {
        assert!(parse_value(json!({"formatVersion": 2})).is_err());
        assert!(parse_value(json!({"formatVersion": "1"})).is_err());
        assert!(parse_value(json!({"formatVersion": 0})).is_ok());
        assert!(parse_value(json!({})).is_ok()); // absent = current
    }

    #[test]
    fn minimum_launcher_version_warns_above_supported() {
        let f = parse_value(json!({"uid": "x", "minimumLauncherVersion": 19})).unwrap();
        assert_eq!(f.problem_severity(), ProblemSeverity::Warning);
        let f = parse_value(json!({"uid": "x", "minimumLauncherVersion": 18})).unwrap();
        assert_eq!(f.problem_severity(), ProblemSeverity::None);
    }

    #[test]
    fn patch_serialization_round_trips() {
        let f = parse_value(json!({
            "name": "Fabric Loader",
            "uid": "net.fabricmc.fabric-loader",
            "version": "0.16.5",
            "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "+traits": ["fabric"],
            "+tweakers": [],
            "requires": [{"uid": "net.minecraft", "equals": "1.21.1"}]
        }))
        .unwrap();
        let text = f.to_document_text().unwrap();
        assert!(text.contains("\"+traits\": [\n        \"fabric\"\n    ]"));
        let back = VersionFile::parse(&json::parse(&text).unwrap(), Path::new("t"), false).unwrap();
        assert_eq!(back.traits, f.traits);
        assert_eq!(back.uid, f.uid);
        assert_eq!(back.main_class, f.main_class);
    }

    #[test]
    fn mojang_parse_sets_canonical_identity() {
        let f = VersionFile::parse_mojang(
            &json!({"id": "1.20.4", "mainClass": "net.minecraft.client.main.Main", "libraries": []}),
            Path::new("client.json"),
        )
        .unwrap();
        assert_eq!(f.name, "Minecraft");
        assert_eq!(f.uid, "net.minecraft");
        assert_eq!(f.version, "1.20.4");
    }

    #[test]
    fn plus_jar_mods_deprecated_style_parses() {
        let mut f = parse_value(json!({"uid": "x", "name": "Some Patch (jar mod)", "+jarMods": [{"name": "mod.jar"}]}))
            .unwrap();
        let jm = f.jar_mods.pop().unwrap();
        assert_eq!(jm.filename_override, "mod.jar");
        assert_eq!(jm.hint, "local");
        assert_eq!(jm.displayname, "Some Patch");
        assert!(jm.name.valid());
    }

    #[test]
    fn volatile_must_be_boolean() {
        assert!(parse_value(json!({"uid": "x", "volatile": true})).unwrap().volatile);
        assert!(parse_value(json!({"uid": "x", "volatile": "yes"})).is_err());
    }
}
