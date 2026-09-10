//! Library entries — port of `minecraft/Library.cpp` + `MojangVersionFormat`
//! library (de)serialization, including the `MMC-*` extension keys.

use super::gradle::GradleSpecifier;
use super::rules::{Applied, Rule, RuntimeContext};
use super::{Problem, ProblemSeverity};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

/// A Mojang download descriptor (`MojangDownloadInfo`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DownloadInfo {
    /// Optional relative storage path.
    pub path: Option<String>,
    /// SHA-1 checksum.
    pub sha1: String,
    /// Download URL.
    pub url: String,
    /// Size in bytes.
    pub size: i64,
}

/// `downloads` object of a library (`MojangLibraryDownloadInfo`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LibraryDownloads {
    /// Main artifact.
    pub artifact: Option<DownloadInfo>,
    /// Native classifier artifacts.
    pub classifiers: BTreeMap<String, DownloadInfo>,
}

impl LibraryDownloads {
    /// Lookup a classifier download (`getDownloadInfo`).
    pub fn download(&self, classifier: &str) -> Option<&DownloadInfo> {
        self.classifiers.get(classifier)
    }
}

/// File lists produced by `Library::getApplicableFiles`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ApplicableFiles {
    /// Regular jars for the classpath.
    pub jar: Vec<String>,
    /// Native jars without `${arch}`.
    pub native: Vec<String>,
    /// Native jars whose classifier contains `${arch}` -> 32-bit variant.
    pub native32: Vec<String>,
    /// Native jars whose classifier contains `${arch}` -> 64-bit variant.
    pub native64: Vec<String>,
}

/// One library of a version file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Library {
    /// Maven coordinate (`name`).
    pub name: GradleSpecifier,
    /// Maven repository base URL (`url`).
    pub repository_url: String,
    /// MultiMC hint (`MMC-hint`), e.g. `local` or `always-stale`.
    pub hint: String,
    /// Absolute URL override (`MMC-absoluteUrl` / legacy `MMC-absulute_url`).
    pub absolute_url: String,
    /// File-name override (`MMC-filename`).
    pub filename_override: String,
    /// Display name override (`MMC-displayname`).
    pub displayname: String,
    /// `extract.exclude` entries.
    pub extract_excludes: Vec<String>,
    /// `natives` classifier map (OS -> classifier template).
    pub native_classifiers: BTreeMap<String, String>,
    /// `rules` list.
    pub rules: Vec<Rule>,
    /// `downloads` object.
    pub mojang_downloads: Option<LibraryDownloads>,
}

impl Library {
    /// Parse a library JSON object. Missing `name` is a hard error (mirrors
    /// the `JSONValidationError` that fails the whole version file); an
    /// unparseable name is recorded as an `Error` problem instead
    /// (`MojangVersionFormat::libraryFromJson`).
    pub fn from_json(value: &Value, problems: &mut Vec<Problem>) -> crate::error::Result<Library> {
        let obj = value.as_object().ok_or_else(|| {
            crate::error::Error::json("<version>", "library entry must be an object")
        })?;
        let raw_name = obj
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::Error::json("<version>", "library that doesn't have a 'name' field"))?
            .to_string();
        let name = GradleSpecifier::parse(&raw_name);
        if !name.valid() {
            problems.push(super::Problem {
                severity: ProblemSeverity::Error,
                message: format!("Library {raw_name} name is broken and cannot be processed."),
            });
        }
        let mut lib = Library {
            name,
            repository_url: obj.get("url").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
            ..Default::default()
        };
        if let Some(extract) = obj.get("extract").and_then(|v| v.as_object()) {
            if let Some(exclude) = extract.get("exclude").and_then(|v| v.as_array()) {
                for e in exclude {
                    if let Some(s) = e.as_str() {
                        lib.extract_excludes.push(s.to_string());
                    }
                }
            }
        }
        if let Some(natives) = obj.get("natives").and_then(|v| v.as_object()) {
            for (k, v) in natives {
                // Non-string values are skipped with a warning, like Prism.
                if let Some(s) = v.as_str() {
                    lib.native_classifiers.insert(k.clone(), s.to_string());
                }
            }
        }
        if let Some(rules) = obj.get("rules").and_then(|v| v.as_array()) {
            for r in rules {
                lib.rules.push(Rule::from_json(r));
            }
        }
        if let Some(dl) = obj.get("downloads").and_then(|v| v.as_object()) {
            let mut info = LibraryDownloads::default();
            if let Some(artifact) = dl.get("artifact") {
                info.artifact = Some(download_info_from_json(artifact)?);
            }
            if let Some(classifiers) = dl.get("classifiers").and_then(|v| v.as_object()) {
                for (k, v) in classifiers {
                    info.classifiers.insert(k.clone(), download_info_from_json(v)?);
                }
            }
            lib.mojang_downloads = Some(info);
        }
        // MultiMC extensions
        lib.hint = obj.get("MMC-hint").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        lib.absolute_url = obj
            .get("MMC-absoluteUrl")
            .or_else(|| obj.get("MMC-absulute_url")) // legacy typo key
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        lib.filename_override = obj.get("MMC-filename").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        lib.displayname = obj.get("MMC-displayname").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        Ok(lib)
    }

    /// Serialize (`OneSixVersionFormat::libraryToJson` + Mojang base).
    pub fn to_json(&self) -> Value {
        let mut root = serde_json::Map::new();
        root.insert("name".into(), Value::String(self.name.serialize()));
        if !self.repository_url.is_empty() {
            root.insert("url".into(), Value::String(self.repository_url.clone()));
        }
        if self.is_native() {
            let natives: serde_json::Map<String, Value> = self
                .native_classifiers
                .iter()
                .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                .collect();
            root.insert("natives".into(), Value::Object(natives));
            if !self.extract_excludes.is_empty() {
                let mut extract = serde_json::Map::new();
                extract.insert(
                    "exclude".into(),
                    Value::Array(self.extract_excludes.iter().map(|e| Value::String(e.clone())).collect()),
                );
                root.insert("extract".into(), Value::Object(extract));
            }
        }
        if !self.rules.is_empty() {
            root.insert("rules".into(), Value::Array(self.rules.iter().map(Rule::to_json).collect()));
        }
        if let Some(dl) = &self.mojang_downloads {
            let mut dl_obj = serde_json::Map::new();
            if let Some(artifact) = &dl.artifact {
                dl_obj.insert("artifact".into(), download_info_to_json(artifact));
            }
            if !dl.classifiers.is_empty() {
                let classifiers: serde_json::Map<String, Value> =
                    dl.classifiers.iter().map(|(k, v)| (k.clone(), download_info_to_json(v))).collect();
                dl_obj.insert("classifiers".into(), Value::Object(classifiers));
            }
            root.insert("downloads".into(), Value::Object(dl_obj));
        }
        // MultiMC extensions (alphabetical placement is handled by the map)
        if !self.absolute_url.is_empty() {
            root.insert("MMC-absoluteUrl".into(), Value::String(self.absolute_url.clone()));
        }
        if !self.hint.is_empty() {
            root.insert("MMC-hint".into(), Value::String(self.hint.clone()));
        }
        if !self.filename_override.is_empty() {
            root.insert("MMC-filename".into(), Value::String(self.filename_override.clone()));
        }
        if !self.displayname.is_empty() {
            root.insert("MMC-displayname".into(), Value::String(self.displayname.clone()));
        }
        Value::Object(root)
    }

    /// Whether the library carries native classifiers.
    pub fn is_native(&self) -> bool {
        !self.native_classifiers.is_empty()
    }

    /// `MMC-hint == "local"` (stored inside the instance).
    pub fn is_local(&self) -> bool {
        self.hint == "local"
    }

    /// `MMC-hint == "always-stale"` (re-verified on every update).
    pub fn is_always_stale(&self) -> bool {
        self.hint == "always-stale"
    }

    /// Native classifier for the current context
    /// (`Library::getCompatibleNative`): precise `<os>-<arch>` first, bare
    /// `<os>` only on legacy (x86) architectures.
    pub fn compatible_native(&self, ctx: &RuntimeContext) -> Option<String> {
        let precise = ctx.classifier();
        if let Some(c) = self.native_classifiers.get(&precise) {
            return Some(c.clone());
        }
        if ctx.is_legacy_arch() {
            if let Some(c) = self.native_classifiers.get(&ctx.system) {
                return Some(c.clone());
            }
        }
        None
    }

    /// Rule + native evaluation (`Library::isActive`): default Disallow,
    /// last non-defer rule wins; natives additionally need a compatible
    /// classifier.
    pub fn is_active(&self, ctx: &RuntimeContext) -> bool {
        let result = if self.rules.is_empty() {
            true
        } else {
            let mut rule_result = Applied::Disallow;
            for rule in &self.rules {
                let applied = rule.apply(ctx);
                if applied != Applied::Defer {
                    rule_result = applied;
                }
            }
            rule_result == Applied::Allow
        };
        if self.is_native() {
            result && self.compatible_native(ctx).is_some()
        } else {
            result
        }
    }

    /// Maven-relative storage path for this context (`storageSuffix`),
    /// with native classifier substitution and `${arch}` left in place when
    /// unresolved (INVALID classifier otherwise).
    pub fn storage_suffix(&self, ctx: &RuntimeContext) -> String {
        if !self.is_native() {
            return self.name.to_path(&self.filename_override);
        }
        let mut spec = self.name.clone();
        match self.compatible_native(ctx) {
            Some(c) => spec.set_classifier(&c),
            None => spec.set_classifier("INVALID"),
        }
        spec.to_path(&self.filename_override)
    }

    /// Resolve applicable file paths (`Library::getApplicableFiles`).
    /// `override_path` is the instance-local library dir for `local` libs.
    pub fn applicable_files(&self, ctx: &RuntimeContext, override_path: Option<&Path>) -> ApplicableFiles {
        let mut out = ApplicableFiles::default();
        let actual_path = |rel: &str| -> String {
            let joined = format!("libraries/{}", rel);
            if self.is_local() {
                if let Some(ovr) = override_path {
                    // Qt-style path: forward slashes on every platform.
                    let file = rel.rsplit('/').next().unwrap_or(rel);
                    let owned = ovr.to_string_lossy().into_owned();
                    let base = owned.trim_end_matches(['/', '\\']);
                    return format!("{base}/{file}");
                }
            }
            joined
        };
        let raw = self.storage_suffix(ctx);
        if self.is_native() {
            if raw.contains("${arch}") {
                out.native32.push(actual_path(&raw.replace("${arch}", "32")));
                out.native64.push(actual_path(&raw.replace("${arch}", "64")));
            } else {
                out.native.push(actual_path(&raw));
            }
        } else {
            out.jar.push(actual_path(&raw));
        }
        out
    }
}

/// Parse a Mojang download descriptor (`readDownloadInfo`: sha1/url/size
/// required, path optional).
pub fn download_info_from_json(value: &Value) -> crate::error::Result<DownloadInfo> {
    let obj = value
        .as_object()
        .ok_or_else(|| crate::error::Error::json("<version>", "download info must be an object"))?;
    let require_str = |key: &str| {
        obj.get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| crate::error::Error::json("<version>", format!("missing '{key}' in download info")))
    };
    Ok(DownloadInfo {
        path: obj.get("path").and_then(|v| v.as_str()).map(|s| s.to_string()),
        sha1: require_str("sha1")?,
        url: require_str("url")?,
        size: obj
            .get("size")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| crate::error::Error::json("<version>", "missing 'size' in download info"))?,
    })
}

/// Serialize a download descriptor (`downloadInfoToJson`).
pub fn download_info_to_json(info: &DownloadInfo) -> Value {
    let mut obj = serde_json::Map::new();
    if let Some(p) = &info.path {
        obj.insert("path".into(), Value::String(p.clone()));
    }
    obj.insert("sha1".into(), Value::String(info.sha1.clone()));
    obj.insert("size".into(), Value::from(info.size));
    obj.insert("url".into(), Value::String(info.url.clone()));
    Value::Object(obj)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx(system: &str, arch: &str, java_arch: &str) -> RuntimeContext {
        RuntimeContext {
            system: system.into(),
            java_real_architecture: arch.into(),
            java_architecture: java_arch.into(),
        }
    }

    fn parse_lib(v: Value) -> (Library, Vec<super::Problem>) {
        let mut problems = Vec::new();
        let lib = Library::from_json(&v, &mut problems).unwrap();
        (lib, problems)
    }

    #[test]
    fn parses_mojang_library_with_downloads_and_rules() {
        let (lib, problems) = parse_lib(json!({
            "name": "org.lwjgl.lwjgl:lwjgl:2.9.3",
            "url": "https://libraries.minecraft.net/",
            "rules": [{"action": "allow", "os": {"name": "linux"}}],
            "downloads": {"artifact": {"sha1": "abc", "url": "https://x/lwjgl.jar", "size": 100}}
        }));
        assert!(problems.is_empty());
        assert_eq!(lib.name.version(), "2.9.3");
        assert_eq!(lib.repository_url, "https://libraries.minecraft.net/");
        assert_eq!(lib.rules.len(), 1);
        assert!(!lib.is_native());
        assert!(lib.mojang_downloads.as_ref().unwrap().artifact.is_some());
        assert!(lib.is_active(&ctx("linux", "x86_64", "64")));
        assert!(!lib.is_active(&ctx("windows", "x86_64", "64")));
    }

    #[test]
    fn invalid_gradle_name_yields_error_problem_not_panic() {
        let (lib, problems) = parse_lib(json!({"name": "not a coordinate"}));
        assert!(!lib.name.valid());
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].severity, ProblemSeverity::Error);
    }

    #[test]
    fn missing_name_is_a_hard_error() {
        let mut problems = Vec::new();
        let err = Library::from_json(&json!({"url": "x"}), &mut problems).unwrap_err();
        assert!(err.to_string().contains("name"));
    }

    #[test]
    fn natives_selection_precise_then_legacy() {
        let (lib, _) = parse_lib(json!({
            "name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.3:natives",
            "natives": {"linux": "natives-linux", "windows": "natives-windows-${arch}", "osx": "natives-osx"}
        }));
        assert!(lib.is_native());
        assert_eq!(lib.compatible_native(&ctx("linux", "x86_64", "64")).as_deref(), Some("natives-linux"));
        assert_eq!(
            lib.compatible_native(&ctx("windows", "amd64", "64")).as_deref(),
            Some("natives-windows-${arch}")
        );
        // arm mac: no precise osx-arm64, and osx bare matches only on legacy
        assert_eq!(lib.compatible_native(&ctx("osx", "aarch64", "64")), None);
        assert_eq!(lib.compatible_native(&ctx("osx", "x86_64", "64")).as_deref(), Some("natives-osx"));
    }

    #[test]
    fn native_library_inactive_without_compatible_classifier() {
        let (lib, _) = parse_lib(json!({
            "name": "x:y:1:natives",
            "natives": {"windows": "natives-windows"},
            "rules": [{"action": "allow"}]
        }));
        assert!(!lib.is_active(&ctx("linux", "x86_64", "64")));
        assert!(lib.is_active(&ctx("windows", "x86_64", "64")));
    }

    #[test]
    fn rule_disallow_default_and_last_wins() {
        let (lib, _) = parse_lib(json!({
            "name": "x:y:1",
            "rules": [
                {"action": "disallow"},
                {"action": "allow", "os": {"name": "linux"}}
            ]
        }));
        // on linux: disallow (no os) then allow(osx? no: linux) -> allow
        assert!(lib.is_active(&ctx("linux", "x86_64", "64")));
        // on windows: disallow then defer -> disallow
        assert!(!lib.is_active(&ctx("windows", "x86_64", "64")));
    }

    #[test]
    fn applicable_files_split_jar_native_and_arch() {
        let (jar_lib, _) = parse_lib(json!({"name": "a:b:1"}));
        let files = jar_lib.applicable_files(&ctx("linux", "x86_64", "64"), None);
        assert_eq!(files.jar, vec!["libraries/a/b/1/b-1.jar"]);

        let (native_lib, _) = parse_lib(json!({
            "name": "c:d:1:natives",
            "natives": {"windows": "natives-windows-${arch}"}
        }));
        let files = native_lib.applicable_files(&ctx("windows", "x86_64", "64"), None);
        assert!(files.jar.is_empty());
        assert_eq!(
            files.native32,
            vec!["libraries/c/d/1/d-1-natives-windows-32.jar"]
        );
        assert_eq!(
            files.native64,
            vec!["libraries/c/d/1/d-1-natives-windows-64.jar"]
        );
    }

    #[test]
    fn local_libraries_resolve_against_override_path() {
        let (mut lib, _) = parse_lib(json!({"name": "custom:customjar:1"}));
        lib.hint = "local".into();
        lib.filename_override = "custom.jar".into();
        let files = lib.applicable_files(&ctx("linux", "x86_64", "64"), Some(Path::new("/inst/libraries")));
        assert_eq!(files.jar, vec!["/inst/libraries/custom.jar"]);
    }

    #[test]
    fn json_round_trip_including_mmc_extensions() {
        let v = json!({
            "name": "custom.jarmods:abc:1",
            "MMC-hint": "local",
            "MMC-filename": "abc.jar",
            "MMC-displayname": "My Mod (jar mod)",
            "MMC-absoluteUrl": "https://x/y.jar",
            "extract": {"exclude": ["META-INF/"]},
            "natives": {"linux": "natives-linux"},
            "rules": [{"action": "disallow", "os": {"name": "osx"}}],
            "downloads": {"artifact": {"sha1": "s", "url": "u", "size": 1}, "classifiers": {"n": {"sha1": "s2", "url": "u2", "size": 2, "path": "p"}}}
        });
        let (lib, problems) = parse_lib(v.clone());
        assert!(problems.is_empty());
        let back = lib.to_json();
        assert_eq!(back, v);
        // legacy typo key also reads
        let mut problems = Vec::new();
        let lib2 = Library::from_json(&json!({"name": "a:b:1", "MMC-absulute_url": "legacy"}), &mut problems).unwrap();
        assert_eq!(lib2.absolute_url, "legacy");
    }

    #[test]
    fn download_info_requires_fields() {
        assert!(download_info_from_json(&json!({"sha1": "s", "url": "u"})).is_err());
        assert!(download_info_from_json(&json!({"sha1": "s", "url": "u", "size": 1})).is_ok());
        let info = download_info_from_json(&json!({"sha1": "s", "url": "u", "size": 1, "path": "p"})).unwrap();
        assert_eq!(download_info_to_json(&info), json!({"sha1": "s", "size": 1, "url": "u", "path": "p"}));
    }
}
