//! The version manifest and the per-version metadata JSON.
//!
//! Two documents the metadata service publishes: the manifest listing every
//! version, and one JSON per version saying exactly how to launch it. Field
//! names follow the published format verbatim -- `serde` renames keep the
//! format's spelling, including its mixture of camelCase and snake_case.
//!
//! Two rules make the round trip honest, and `tests/format_round_trip.rs`
//! asserts both against real downloaded samples:
//!
//! - fields the format grew over the years are optional (`arguments` is the
//!   2018+ shape, `minecraftArguments` the older string);
//! - a field absent in the document stays absent when it is written back
//!   (`skip_serializing_if`), and anything unrecognised rides along in
//!   `extra`, so a document survives parse -> serialize -> parse unchanged
//!   in value.
//!
//! The formats are public specifications; nothing here is derived from
//! another launcher's implementation of them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};

/// `skip_serializing_if` helper for `bool` fields the format omits when false.
pub(crate) fn is_false(value: &bool) -> bool {
    !*value
}

/// The manifest: `latest` pointers plus one entry per published version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VersionManifest {
    pub latest: Latest,
    pub versions: Vec<ManifestEntry>,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, Value>,
}

/// The manifest's two pointers, which move as versions publish.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, Value>,
}

/// One row of the manifest: enough to fetch the version's full metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    pub time: String,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(
        rename = "complianceLevel",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub compliance_level: Option<u32>,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, Value>,
}

/// One version's full metadata: what to run, with what, from where.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Version {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(rename = "mainClass")]
    pub main_class: String,
    pub time: String,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
    #[serde(
        rename = "minimumLauncherVersion",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub minimum_launcher_version: Option<u32>,
    #[serde(
        rename = "complianceLevel",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub compliance_level: Option<u32>,
    /// The 2018+ argument lists (JVM and game), with rule-gated entries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Arguments>,
    /// The pre-2018 shape: one whitespace-separated game-argument string.
    #[serde(
        rename = "minecraftArguments",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub minecraft_arguments: Option<String>,
    /// Which asset index this version draws its sounds and art from.
    #[serde(
        rename = "assetIndex",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub asset_index: Option<AssetRef>,
    /// The asset index *name* older versions pass to the game as a flag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assets: Option<String>,
    #[serde(
        rename = "javaVersion",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub java_version: Option<JavaVersion>,
    /// Per-platform jars: at minimum the client, sometimes the server.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub downloads: BTreeMap<String, Download>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub libraries: Vec<Library>,
    /// Logging configuration downloads (the log4j configuration).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub logging: BTreeMap<String, Logging>,
    /// Mod-loader versions are overlays on a parent version, by id.
    #[serde(
        rename = "inheritsFrom",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub inherits_from: Option<String>,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, Value>,
}

/// A downloadable file named by hash and size.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Download {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(rename = "totalSize", default, skip_serializing_if = "Option::is_none")]
    pub total_size: Option<u64>,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, Value>,
}

/// A reference to an asset index document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetRef {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(rename = "totalSize", default, skip_serializing_if = "Option::is_none")]
    pub total_size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, Value>,
}

/// Which Java runtime a version wants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JavaVersion {
    /// The Mojang runtime component name, e.g. `java-runtime-delta`.
    pub component: String,
    #[serde(rename = "majorVersion")]
    pub major_version: u32,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, Value>,
}

/// One library: a jar on the classpath, natives to extract, or both.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Library {
    /// Maven coordinate: `group:artifact:version[:classifier][@ext]`.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downloads: Option<LibraryDownloads>,
    /// OS name -> classifier template, e.g. `windows` -> `natives-windows`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub natives: Option<BTreeMap<String, String>>,
    /// When this library applies. Absent means always.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<Rule>>,
    /// Paths to skip when a natives jar is extracted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extract: Option<Extract>,
    /// Older versions name a Maven repository base instead of exact URLs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LibraryDownloads {
    /// The plain jar. Some natives-only libraries omit it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<Artifact>,
    /// Per-platform natives jars, keyed by classifier name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classifiers: Option<BTreeMap<String, Artifact>>,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, Value>,
}

/// One downloadable jar: where it lives in the library tree and what it is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Artifact {
    /// Relative path under the library root (the Maven layout).
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(rename = "totalSize", default, skip_serializing_if = "Option::is_none")]
    pub total_size: Option<u64>,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Extract {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, Value>,
}

/// A logging configuration to download and pass to the JVM.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Logging {
    /// The JVM argument template; `${path}` becomes the downloaded file.
    pub argument: String,
    pub file: LoggingFile,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoggingFile {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, Value>,
}

/// The 2018+ argument lists. `default_user_jvm` appeared in the 2026 format:
/// JVM tuning the launcher may replace when the person has their own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Arguments {
    #[serde(
        rename = "default-user-jvm",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub default_user_jvm: Vec<Argument>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub jvm: Vec<Argument>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub game: Vec<Argument>,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, Value>,
}

/// One entry of an argument list: a literal, or literals possibly behind
/// rules. `rules` is optional in the format -- the 26.3 sample carries a
/// `value`-only entry for its JVM tuning group -- and omitted when empty so
/// a round trip does not invent a key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Argument {
    Plain(String),
    Conditional {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        rules: Vec<Rule>,
        value: ArgValue,
    },
}

/// A conditional argument's payload: one string or several.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ArgValue {
    One(String),
    Many(Vec<String>),
}

impl ArgValue {
    /// The literal strings, whichever shape they arrived in.
    pub fn strings(&self) -> &[String] {
        match self {
            ArgValue::One(one) => std::slice::from_ref(one),
            ArgValue::Many(many) => many,
        }
    }
}

/// A condition on an argument or a library.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    pub action: RuleAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<OsRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub features: Option<BTreeMap<String, bool>>,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    Allow,
    Disallow,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct OsRule {
    /// `windows`, `osx` or `linux` -- the format's names, not ours.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// A regular expression matched against the OS version string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// `x86`, seen on rules gating 32-bit Java.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
    /// The newer bound form: `min` inclusive, `max` exclusive. (The boundary
    /// pairing in the 26.3 sample -- ZGC at `min: 10.0.17134`, G1 at
    /// `max: 10.0.17134` -- only partitions cleanly with an exclusive max,
    /// which is how this is read; swap it if the specification says else.)
    #[serde(
        rename = "versionRange",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub version_range: Option<VersionRange>,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct VersionRange {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<String>,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, Value>,
}

impl VersionManifest {
    /// Parse the manifest document.
    pub fn parse(text: &str) -> Result<Self> {
        serde_json::from_str(text).map_err(Error::parse("version manifest"))
    }

    /// Write it back; parse -> serialize -> parse is value-stable.
    pub fn to_json_string(&self) -> Result<String> {
        serde_json::to_string(self).map_err(Error::parse("version manifest"))
    }

    /// Find one version's row by id.
    pub fn find(&self, id: &str) -> Option<&ManifestEntry> {
        self.versions.iter().find(|entry| entry.id == id)
    }
}

impl Version {
    /// Parse one version's metadata document.
    pub fn parse(text: &str) -> Result<Self> {
        serde_json::from_str(text).map_err(Error::parse("version metadata"))
    }

    /// Write it back; parse -> serialize -> parse is value-stable.
    pub fn to_json_string(&self) -> Result<String> {
        serde_json::to_string(self).map_err(Error::parse("version metadata"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_serde_untagged_shapes() {
        // The format mixes literals with rule-gated entries in one list, and
        // a gated entry's value is one string or several. All four shapes
        // must survive a round trip through our own writer.
        let json = r#"[
            "-plain",
            {"rules": [{"action": "allow"}], "value": "-one"},
            {"rules": [{"action": "disallow", "os": {"name": "osx"}}],
             "value": ["-many", "${classpath}"]},
            {"value": ["-no-rules-at-all"]}
        ]"#;
        let parsed: Vec<Argument> = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.len(), 4);
        let out = serde_json::to_string(&parsed).unwrap();
        let reparsed: Vec<Argument> = serde_json::from_str(&out).unwrap();
        let before: Value = serde_json::from_str(json).unwrap();
        let after: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(before, after);
        assert_eq!(parsed, reparsed);
    }

    #[test]
    fn absent_fields_stay_absent() {
        // A field the document omits must not appear as null on the way out,
        // or a round trip would not be equal in value.
        let json = r#"{"id": "x", "type": "release", "mainClass": "m",
                       "time": "t", "releaseTime": "r"}"#;
        let version = Version::parse(json).unwrap();
        let out = version.to_json_string().unwrap();
        assert!(
            !out.contains("null"),
            "omitted fields leaked as null: {out}"
        );
        let before: Value = serde_json::from_str(json).unwrap();
        let after: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn unknown_fields_ride_along() {
        let json = r#"{"id": "x", "type": "release", "mainClass": "m",
                       "time": "t", "releaseTime": "r", "fromTheFuture": 7}"#;
        let version = Version::parse(json).unwrap();
        let out = version.to_json_string().unwrap();
        let after: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(after["fromTheFuture"], Value::from(7));
    }

    #[test]
    fn manifest_lookup_finds_by_id() {
        let json = r#"{"latest": {"release": "a", "snapshot": "b"},
                       "versions": [{"id": "a", "type": "release", "url": "u",
                       "time": "t", "releaseTime": "r"}]}"#;
        let manifest = VersionManifest::parse(json).unwrap();
        assert!(manifest.find("a").is_some());
        assert!(manifest.find("b").is_none());
    }
}
