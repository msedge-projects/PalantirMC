//! Component resolution — the `prism-core` side of
//! `ComponentUpdateTask`: turn a [`crate::pack::PackProfile`] into a merged
//! [`LaunchProfile`] by loading one version file per enabled component and
//! applying them in patch order.
//!
//! Networking stays out of core: version files come from a [`MetaStore`]
//! (offline disk cache here; `prism-net` adds the online implementation and
//! the network-timeout handling behind the same trait).

use crate::error::{Error, Result};
use crate::pack::{PackProfile, Require};
use crate::version::{LaunchProfile, ProblemSeverity, RuntimeContext, VersionFile};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// One entry of a metadata version list (`Meta::Version` common fields).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VersionEntry {
    /// Component uid.
    pub uid: String,
    /// Version string.
    pub version: String,
    /// `release` / `snapshot` / ...
    pub type_: String,
    /// `recommended` flag.
    pub recommended: bool,
    /// `volatile` flag.
    pub volatile: bool,
    /// Release time (raw S3 string).
    pub release_time: String,
    /// Requirements.
    pub requires: Vec<Require>,
    /// Conflicts.
    pub conflicts: Vec<Require>,
    /// Optional sha256 of the version file.
    pub sha256: String,
}

/// Source of component metadata. Implemented offline here; the online
/// implementation (with HTTP caching and timeouts) lands in `prism-net`.
pub trait MetaStore {
    /// Load the version file for a uid/version pair.
    fn version_file(&mut self, uid: &str, version: &str) -> Result<VersionFile>;

    /// List known versions of a uid (empty when unknown).
    fn version_list(&mut self, uid: &str) -> Result<Vec<VersionEntry>>;
}

/// Offline metadata store reading Prism's cache layout:
/// `<meta_dir>/<uid>.json` (version list) and
/// `<meta_dir>/<uid>/<version>.json` (version file).
#[derive(Debug, Clone)]
pub struct OfflineMetaStore {
    meta_dir: PathBuf,
}

impl OfflineMetaStore {
    /// Store rooted at `meta_dir` (usually `paths.meta_dir()`).
    pub fn new(meta_dir: impl Into<PathBuf>) -> OfflineMetaStore {
        OfflineMetaStore { meta_dir: meta_dir.into() }
    }

    fn version_path(&self, uid: &str, version: &str) -> PathBuf {
        self.meta_dir.join(uid).join(format!("{version}.json"))
    }

    fn list_path(&self, uid: &str) -> PathBuf {
        self.meta_dir.join(format!("{uid}.json"))
    }
}

impl MetaStore for OfflineMetaStore {
    fn version_file(&mut self, uid: &str, version: &str) -> Result<VersionFile> {
        let path = self.version_path(uid, version);
        let text = crate::util::read_text(&path)?;
        let value: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| Error::json(&path, e.to_string()))?;
        // meta files carry "order"; require it exactly like the meta parser
        VersionFile::parse(&value, &path, value.get("order").is_some())
    }

    fn version_list(&mut self, uid: &str) -> Result<Vec<VersionEntry>> {
        let path = self.list_path(uid);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let text = crate::util::read_text(&path)?;
        let value: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| Error::json(&path, e.to_string()))?;
        let obj = value
            .as_object()
            .ok_or_else(|| Error::json(&path, "version list root must be an object"))?;
        let fv = obj.get("formatVersion").and_then(|v| v.as_i64()).unwrap_or(0);
        if fv != 0 && fv != 1 {
            return Err(Error::format(&path, format!("unknown metadata format version {fv}")));
        }
        let mut out = Vec::new();
        if let Some(items) = obj.get("versions").and_then(|v| v.as_array()) {
            for item in items {
                let Some(o) = item.as_object() else { continue };
                let requires = o
                    .get("requires")
                    .and_then(|v| v.as_array())
                    .map(|a| a.iter().filter_map(Require::from_json).collect())
                    .unwrap_or_default();
                let conflicts = o
                    .get("conflicts")
                    .and_then(|v| v.as_array())
                    .map(|a| a.iter().filter_map(Require::from_json).collect())
                    .unwrap_or_default();
                out.push(VersionEntry {
                    uid: uid.to_string(),
                    version: o.get("version").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                    type_: o.get("type").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                    recommended: o.get("recommended").and_then(|v| v.as_bool()).unwrap_or(false),
                    volatile: o.get("volatile").and_then(|v| v.as_bool()).unwrap_or(false),
                    release_time: o.get("releaseTime").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                    requires,
                    conflicts,
                    sha256: o.get("sha256").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
                });
            }
        }
        Ok(out)
    }
}

/// One resolved component and how it went.
#[derive(Debug, Clone)]
pub struct ResolvedComponent {
    /// Component uid.
    pub uid: String,
    /// Version that was resolved (may fall back to the cached version).
    pub version: String,
    /// Whether the component was disabled (skipped during merge).
    pub disabled: bool,
    /// Whether a custom patch file from `patches/` was used.
    pub custom: bool,
    /// Problems attached to this component.
    pub problems: Vec<crate::version::Problem>,
}

/// The outcome of resolution.
#[derive(Debug, Default)]
pub struct Resolution {
    /// Merged launch profile.
    pub profile: LaunchProfile,
    /// Per-component results in patch order.
    pub components: Vec<ResolvedComponent>,
    /// Global problems (ordering/conflict issues).
    pub problems: Vec<crate::version::Problem>,
}

impl Resolution {
    /// Highest problem severity across profile and components.
    pub fn severity(&self) -> ProblemSeverity {
        let mut s = self.profile.problem_severity;
        for c in &self.components {
            for p in &c.problems {
                if p.severity > s {
                    s = p.severity;
                }
            }
        }
        for p in &self.problems {
            if p.severity > s {
                s = p.severity;
            }
        }
        s
    }
}

/// Resolve a pack profile into a [`Resolution`].
///
/// Custom components (a `patches/<uid>.json` file) take precedence over the
/// metadata store, mirroring `Component::getVersionFile`. Components are
/// applied in their mmc-pack order (Prism orders patches by the file's
/// `order` field when present).
pub fn resolve(
    profile: &PackProfile,
    patches_dir: &Path,
    store: &mut dyn MetaStore,
    ctx: &RuntimeContext,
) -> Result<Resolution> {
    let mut resolution = Resolution::default();

    // Pass 1: load the version file of every enabled component.
    struct Loaded {
        uid: String,
        file: VersionFile,
    }
    let mut loaded: Vec<Loaded> = Vec::new();
    for comp in profile.components() {
        if !comp.is_enabled() {
            resolution.components.push(ResolvedComponent {
                uid: comp.uid.clone(),
                version: comp.version.clone(),
                disabled: true,
                custom: false,
                problems: Vec::new(),
            });
            continue;
        }
        let patch_path = patches_dir.join(format!("{}.json", comp.uid));
        let (file, custom) = if patch_path.exists() {
            let text = crate::util::read_text(&patch_path)?;
            let value: serde_json::Value =
                serde_json::from_str(&text).map_err(|e| Error::json(&patch_path, e.to_string()))?;
            (VersionFile::parse(&value, &patch_path, false)?, true)
        } else {
            let version = if comp.version.is_empty() { comp.cached_version.as_str() } else { comp.version.as_str() };
            match store.version_file(&comp.uid, version) {
                Ok(f) => (f, false),
                Err(e) => {
                    resolution.problems.push(crate::version::Problem {
                        severity: ProblemSeverity::Error,
                        message: format!("cannot resolve component '{}': {e}", comp.uid),
                    });
                    resolution.components.push(ResolvedComponent {
                        uid: comp.uid.clone(),
                        version: version.to_string(),
                        disabled: false,
                        custom: false,
                        problems: vec![],
                    });
                    continue;
                }
            }
        };
        resolution.components.push(ResolvedComponent {
            uid: comp.uid.clone(),
            version: if file.version.is_empty() { comp.version.clone() } else { file.version.clone() },
            disabled: false,
            custom,
            problems: file.problems.clone(),
        });
        loaded.push(Loaded { uid: comp.uid.clone(), file });
    }

    // Pass 2: conflict + requirement checks over enabled components.
    let enabled_uids: BTreeSet<&str> = loaded.iter().map(|l| l.uid.as_str()).collect();
    for item in &loaded {
        for conflict in &item.file.conflicts {
            if enabled_uids.contains(conflict.uid.as_str()) {
                resolution.problems.push(crate::version::Problem {
                    severity: ProblemSeverity::Error,
                    message: format!(
                        "component '{}' conflicts with enabled component '{}'",
                        item.uid, conflict.uid
                    ),
                });
            }
        }
        for require in &item.file.requires {
            if !enabled_uids.contains(require.uid.as_str()) {
                resolution.problems.push(crate::version::Problem {
                    severity: ProblemSeverity::Warning,
                    message: format!("component '{}' requires '{}', which is not present", item.uid, require.uid),
                });
            } else if !require.equals_version.is_empty() {
                let other = loaded
                    .iter()
                    .find(|l| l.uid == require.uid)
                    .map(|l| l.file.version.clone())
                    .unwrap_or_default();
                if !other.is_empty() && other != require.equals_version {
                    resolution.problems.push(crate::version::Problem {
                        severity: ProblemSeverity::Warning,
                        message: format!(
                            "component '{}' requires '{}' version '{}', but '{}' is installed",
                            item.uid, require.uid, require.equals_version, other
                        ),
                    });
                }
            }
        }
    }

    // Pass 3: apply in patch order (stable sort keeps mmc-pack order on ties).
    loaded.sort_by_key(|l| (l.file.order, 0usize));
    for item in &loaded {
        resolution.profile.apply_version_file(&item.file, ctx);
    }

    Ok(resolution)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::Component;
    use serde_json::json;
    use std::collections::BTreeMap;

    struct MapStore {
        files: BTreeMap<(String, String), serde_json::Value>,
    }

    impl MapStore {
        fn with(uid: &str, version: &str, value: serde_json::Value) -> MapStore {
            let mut files = BTreeMap::new();
            files.insert((uid.to_string(), version.to_string()), value);
            MapStore { files }
        }
    }

    impl MetaStore for MapStore {
        fn version_file(&mut self, uid: &str, version: &str) -> Result<VersionFile> {
            let v = self.files.get(&(uid.to_string(), version.to_string())).cloned().ok_or_else(|| {
                Error::Resolve { uid: uid.into(), version: version.into(), detail: "not in store".into() }
            })?;
            VersionFile::parse(&v, Path::new("meta.json"), false)
        }

        fn version_list(&mut self, _uid: &str) -> Result<Vec<VersionEntry>> {
            Ok(Vec::new())
        }
    }

    fn profile_with(components: Vec<Component>) -> PackProfile {
        let mut p = PackProfile::default();
        for c in components {
            p.append(c);
        }
        p
    }

    #[test]
    fn resolves_vanilla_profile_in_order() {
        let mut store = MapStore::with(
            "net.minecraft",
            "1.20.4",
            json!({
                "uid": "net.minecraft", "version": "1.20.4", "order": 0,
                "mainClass": "net.minecraft.client.main.Main",
                "minecraftArguments": "--username ${auth_player_name}",
                "assets": "17"
            }),
        );
        store.files.insert(
            ("org.lwjgl3".into(), "3.3.2".into()),
            json!({"uid": "org.lwjgl3", "version": "3.3.2", "order": 1, "+traits": ["lwjgl3"]}),
        );
        let profile = profile_with(vec![
            Component { uid: "net.minecraft".into(), version: "1.20.4".into(), important: true, ..Default::default() },
            Component { uid: "org.lwjgl3".into(), version: "3.3.2".into(), important: true, ..Default::default() },
        ]);
        let r = resolve(&profile, Path::new("/no/patches"), &mut store, &RuntimeContext::current_host()).unwrap();
        assert_eq!(r.severity(), ProblemSeverity::None);
        assert_eq!(r.profile.minecraft_version, "1.20.4");
        assert_eq!(r.profile.main_class, "net.minecraft.client.main.Main");
        assert!(r.profile.has_trait("lwjgl3"));
        assert_eq!(r.components.len(), 2);
    }

    #[test]
    fn custom_patch_overrides_metadata() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("custom.thing.json"),
            r#"{"uid": "custom.thing", "version": "1", "order": 5, "mainClass": "custom.Main"}"#,
        )
        .unwrap();
        let mut store = MapStore::with("custom.thing", "1", json!({"uid": "custom.thing", "mainClass": "meta.Main"}));
        let profile = profile_with(vec![Component { uid: "custom.thing".into(), version: "1".into(), ..Default::default() }]);
        let r = resolve(&profile, tmp.path(), &mut store, &RuntimeContext::current_host()).unwrap();
        assert!(r.components[0].custom);
        assert_eq!(r.profile.main_class, "custom.Main");
    }

    #[test]
    fn disabled_components_are_skipped() {
        let mut store = MapStore::with("net.minecraft", "1", json!({"uid": "net.minecraft", "mainClass": "M"}));
        let profile = profile_with(vec![
            Component { uid: "net.minecraft".into(), version: "1".into(), ..Default::default() },
            Component { uid: "org.quiltmc.quilt-loader".into(), version: "1".into(), disabled: true, ..Default::default() },
        ]);
        let r = resolve(&profile, Path::new("/no"), &mut store, &RuntimeContext::current_host()).unwrap();
        assert_eq!(r.profile.main_class, "M");
        assert!(r.components.iter().find(|c| c.uid == "org.quiltmc.quilt-loader").unwrap().disabled);
        // disabled loader's absence does not produce conflict problems
        assert!(r.problems.is_empty());
    }

    #[test]
    fn unresolvable_component_is_reported_not_fatal() {
        let mut store = MapStore::with("net.minecraft", "1", json!({"uid": "net.minecraft"}));
        let profile = profile_with(vec![
            Component { uid: "net.minecraft".into(), version: "1".into(), ..Default::default() },
            Component { uid: "net.minecraftforge".into(), version: "99.0".into(), ..Default::default() },
        ]);
        let r = resolve(&profile, Path::new("/no"), &mut store, &RuntimeContext::current_host()).unwrap();
        assert_eq!(r.severity(), ProblemSeverity::Error);
        assert!(r.problems.iter().any(|p| p.message.contains("net.minecraftforge")));
        // the rest still merged
        assert_eq!(r.profile.minecraft_version, "");
    }

    #[test]
    fn conflicting_modloaders_are_detected() {
        let mut store = MapStore::with("net.minecraft", "1", json!({"uid": "net.minecraft"}));
        store.files.insert(
            ("net.minecraftforge".into(), "1".into()),
            json!({"uid": "net.minecraftforge", "conflicts": [{"uid": "net.fabricmc.fabric-loader"}]}),
        );
        store
            .files
            .insert(("net.fabricmc.fabric-loader".into(), "1".into()), json!({"uid": "net.fabricmc.fabric-loader"}));
        let profile = profile_with(vec![
            Component { uid: "net.minecraft".into(), version: "1".into(), ..Default::default() },
            Component { uid: "net.minecraftforge".into(), version: "1".into(), ..Default::default() },
            Component { uid: "net.fabricmc.fabric-loader".into(), version: "1".into(), ..Default::default() },
        ]);
        let r = resolve(&profile, Path::new("/no"), &mut store, &RuntimeContext::current_host()).unwrap();
        assert!(r.problems.iter().any(|p| p.severity == ProblemSeverity::Error && p.message.contains("conflicts")));
    }

    #[test]
    fn unmet_requirement_warns() {
        let mut store = MapStore::with(
            "net.minecraftforge",
            "1",
            json!({"uid": "net.minecraftforge", "requires": [{"uid": "net.minecraft", "equals": "1.20.1"}]}),
        );
        store.files.insert(("net.minecraft".into(), "1.20.4".into()), json!({"uid": "net.minecraft", "version": "1.20.4"}));
        let profile = profile_with(vec![
            Component { uid: "net.minecraft".into(), version: "1.20.4".into(), ..Default::default() },
            Component { uid: "net.minecraftforge".into(), version: "1".into(), ..Default::default() },
        ]);
        let r = resolve(&profile, Path::new("/no"), &mut store, &RuntimeContext::current_host()).unwrap();
        assert!(r.problems.iter().any(|p| p.message.contains("requires 'net.minecraft' version '1.20.1'")));
    }

    #[test]
    fn offline_store_reads_cache_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let meta = tmp.path().join("meta");
        std::fs::create_dir_all(meta.join("net.minecraft")).unwrap();
        std::fs::write(
            meta.join("net.minecraft").join("1.20.4.json"),
            r#"{"formatVersion": 1, "uid": "net.minecraft", "version": "1.20.4", "order": 0, "mainClass": "M"}"#,
        )
        .unwrap();
        std::fs::write(
            meta.join("net.minecraft.json"),
            r#"{"formatVersion": 1, "uid": "net.minecraft", "versions": [{"version": "1.20.4", "type": "release", "recommended": true}]}"#,
        )
        .unwrap();
        let mut store = OfflineMetaStore::new(&meta);
        let f = store.version_file("net.minecraft", "1.20.4").unwrap();
        assert_eq!(f.main_class, "M");
        assert!(f.has_order);
        let list = store.version_list("net.minecraft").unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].recommended);
        // unknown uid -> empty list, not an error
        assert!(store.version_list("does.not.exist").unwrap().is_empty());
        assert!(store.version_file("does.not.exist", "1").is_err());
    }
}
