//! Component resolution — the `palantir-core` side of
//! `ComponentUpdateTask`: turn a [`crate::pack::PackProfile`] into a merged
//! [`LaunchProfile`] by loading one version file per enabled component and
//! applying them in patch order.
//!
//! Networking stays out of core: version files come from a [`MetaStore`]
//! (offline disk cache here; `palantir-net` adds the online implementation and
//! the network-timeout handling behind the same trait).
//!
//! Two rules here are not obvious from the profile format, and both decide
//! whether an instance can start at all:
//!
//! * A component may carry no version. The one to load then comes from what
//!   other components require of it (`equals` first, then `suggests`), or from
//!   the game version for the two mapping uids that follow it. See
//!   [`game_locked_version`].
//! * A component something requires but the profile does not list is resolved
//!   anyway, so the dependency reaches the classpath. Prism does the same when
//!   it loads a profile.

use crate::error::{Error, Result};
use crate::pack::{PackProfile, Require};
use crate::version::{LaunchProfile, ProblemSeverity, RuntimeContext, VersionFile};
use std::collections::{BTreeMap, BTreeSet};
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

/// Fold `req` into `map` the way Prism's `composeRequirement` does.
///
/// An exact `equals` outranks a `suggests` (it is a pin, not a preference),
/// two suggestions keep the higher version, and two `equals` for the same uid
/// keep the first one seen so the decision does not depend on hash order. The
/// caller reports a genuine conflict; this only has to be deterministic.
fn compose_into(map: &mut BTreeMap<String, Require>, req: &Require) {
    match map.get_mut(&req.uid) {
        None => {
            map.insert(req.uid.clone(), req.clone());
        }
        Some(existing) => {
            if existing.equals_version.is_empty() && !req.equals_version.is_empty() {
                existing.equals_version = req.equals_version.clone();
            }
            if existing.suggests.is_empty() {
                existing.suggests = req.suggests.clone();
            } else if !req.suggests.is_empty()
                && crate::version::PalantirVersion::parse(&existing.suggests)
                    < crate::version::PalantirVersion::parse(&req.suggests)
            {
                existing.suggests = req.suggests.clone();
            }
        }
    }
}

/// The version of a component no requirement pins.
///
/// Two uids are mappings that follow the game they belong to, and Prism
/// resolves both to the Minecraft version being launched. Everything else has
/// no defensible guess and is left empty, so the component fails with a reason
/// instead of loading an invented version.
fn game_locked_version(uid: &str, minecraft: &str) -> String {
    match uid {
        "net.fabricmc.intermediary" | "org.quiltmc.hashed" => minecraft.to_string(),
        _ => String::new(),
    }
}

/// Source of component metadata. Implemented offline here; the online
/// implementation (with HTTP caching and timeouts) lands in `palantir-net`.
pub trait MetaStore {
    /// Load the version file for a uid/version pair.
    fn version_file(&mut self, uid: &str, version: &str) -> Result<VersionFile>;

    /// List known versions of a uid (empty when unknown).
    fn version_list(&mut self, uid: &str) -> Result<Vec<VersionEntry>>;
}

/// Offline metadata store reading Prism's cache layout:
/// `<meta_dir>/<uid>/index.json` (version list, with the flat
/// `<meta_dir>/<uid>.json` still read for caches written before the layout
/// changed) and `<meta_dir>/<uid>/<version>.json` (version file).
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

    /// Version-list paths, in the order they are tried: the current layout
    /// first, then the flat file older caches hold.
    fn list_paths(&self, uid: &str) -> [PathBuf; 2] {
        [
            self.meta_dir.join(uid).join("index.json"),
            self.meta_dir.join(format!("{uid}.json")),
        ]
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
        let Some(path) = self.list_paths(uid).into_iter().find(|p| p.exists()) else {
            return Ok(Vec::new());
        };
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

    // Pass 1: decide, per component, which version to load.
    //
    // A component does not have to name one. Prism writes a vanilla instance's
    // `org.lwjgl3` slot and a Fabric instance's `net.fabricmc.intermediary`
    // slot with nothing but a uid, and works out the version while loading the
    // component: the version something else *requires* of it (`equals` outranks
    // `suggests`), or -- for the two mapping uids that follow the game -- the
    // Minecraft version being launched. Prism also *adds* a component that
    // something requires while the profile does not list it at all, which is
    // where those two slots come from in the first place.
    //
    // Loading such a component with the empty version instead asks the metadata
    // for `<uid>/.json`, which is not a URL. The component is then reported
    // unresolvable, the profile is a hard error, and the launcher refuses to
    // start an instance that is perfectly sound -- while its own dependencies
    // (LWJGL, the Fabric mappings) never reach the classpath.
    struct Loaded {
        uid: String,
        file: VersionFile,
    }

    /// One component being resolved, in the order it will be reported.
    struct Slot {
        uid: String,
        /// Version pinned in `mmc-pack.json` (may be empty).
        pinned: String,
        /// Version a previous resolution cached (may be empty).
        cached: String,
        disabled: bool,
        /// Version to load, once one is known.
        target: String,
        /// The loaded file, once it is loaded.
        file: Option<VersionFile>,
        /// Whether the file came from `patches/<uid>.json`.
        custom: bool,
        /// Why it could not be loaded, if it could not.
        error: Option<String>,
    }

    let mut slots: Vec<Slot> = profile
        .components()
        .iter()
        .map(|c| Slot {
            uid: c.uid.clone(),
            pinned: c.version.clone(),
            cached: c.cached_version.clone(),
            disabled: !c.is_enabled(),
            target: String::new(),
            file: None,
            custom: false,
            error: None,
        })
        .collect();

    // What other components ask of each uid, seeded from the requirements a
    // previous resolution cached into `mmc-pack.json` (Prism's
    // `m_cachedRequires`). That seed is what lets an offline launch fill a
    // version without loading anything first.
    let mut wanted: BTreeMap<String, Require> = BTreeMap::new();
    for comp in profile.components() {
        for req in &comp.cached_requires {
            compose_into(&mut wanted, req);
        }
    }

    // The Minecraft version the mapping components follow. Taken from the
    // profile until the metadata confirms it.
    let mut game_version = profile
        .components()
        .iter()
        .find(|c| c.uid == "net.minecraft")
        .map(|c| {
            if c.version.is_empty() {
                c.cached_version.clone()
            } else {
                c.version.clone()
            }
        })
        .unwrap_or_default();

    // Round 1 loads everything that brings its own version: a patch file, a
    // pinned version, or a cached one. Later rounds load what the requirements
    // say, and one round can unlock the next (a dependency may itself require
    // something), so this repeats until a round changes nothing. Each round
    // either loads a component or appends one, both of which are finite, so the
    // bound is the number of rounds it could possibly need.
    let max_rounds = slots.len() + 8;
    for round in 0..max_rounds {
        let mut progress = false;
        let mut index = 0;
        while index < slots.len() {
            if slots[index].disabled || slots[index].file.is_some() || slots[index].error.is_some() {
                index += 1;
                continue;
            }
            let uid = slots[index].uid.clone();
            let patch_path = patches_dir.join(format!("{uid}.json"));
            let patched = patch_path.exists();
            let target = if patched {
                // The patch carries its own version; nothing to decide.
                String::new()
            } else if !slots[index].pinned.is_empty() {
                slots[index].pinned.clone()
            } else if !slots[index].cached.is_empty() {
                slots[index].cached.clone()
            } else if let Some(req) = wanted.get(&uid) {
                if req.equals_version.is_empty() {
                    req.suggests.clone()
                } else {
                    req.equals_version.clone()
                }
            } else {
                game_locked_version(&uid, &game_version)
            };
            if !patched && target.is_empty() {
                // Round 1 has no basis for a decision; a later round may have
                // one, because loading another component adds requirements.
                if round == 0 {
                    index += 1;
                    continue;
                }
                slots[index].error =
                    Some("no version is pinned and no other component requires one".to_string());
                index += 1;
                continue;
            }
            let outcome = if patched {
                crate::util::read_text(&patch_path).and_then(|text| {
                    let value: serde_json::Value = serde_json::from_str(&text)
                        .map_err(|e| Error::json(&patch_path, e.to_string()))?;
                    VersionFile::parse(&value, &patch_path, false)
                })
            } else {
                store.version_file(&uid, &target)
            };
            match outcome {
                Ok(file) => {
                    for req in &file.requires {
                        compose_into(&mut wanted, req);
                    }
                    if uid == "net.minecraft" {
                        game_version = if file.version.is_empty() {
                            target.clone()
                        } else {
                            file.version.clone()
                        };
                    }
                    slots[index].target = if file.version.is_empty() { target } else { file.version.clone() };
                    slots[index].file = Some(file);
                    slots[index].custom = patched;
                    progress = true;
                }
                Err(e) => slots[index].error = Some(e.to_string()),
            }
            index += 1;
        }

        // A component something requires but the profile never listed: Prism
        // adds it (as a dependency-only component) rather than failing.
        // Resolving it here, in memory, is what puts the Fabric mappings on the
        // classpath of an instance that only ever named the loader.
        for uid in wanted.keys() {
            if !slots.iter().any(|s| s.uid == *uid) {
                slots.push(Slot {
                    uid: uid.clone(),
                    pinned: String::new(),
                    cached: String::new(),
                    disabled: false,
                    target: String::new(),
                    file: None,
                    custom: false,
                    error: None,
                });
                progress = true;
            }
        }
        if !progress {
            break;
        }
    }

    let mut loaded: Vec<Loaded> = Vec::new();
    for slot in &mut slots {
        if slot.disabled {
            resolution.components.push(ResolvedComponent {
                uid: slot.uid.clone(),
                version: slot.pinned.clone(),
                disabled: true,
                custom: false,
                problems: Vec::new(),
            });
            continue;
        }
        match slot.file.take() {
            Some(file) => {
                resolution.components.push(ResolvedComponent {
                    uid: slot.uid.clone(),
                    version: slot.target.clone(),
                    disabled: false,
                    custom: slot.custom,
                    problems: file.problems.clone(),
                });
                loaded.push(Loaded { uid: slot.uid.clone(), file });
            }
            None => {
                let detail = slot.error.clone().unwrap_or_default();
                resolution.problems.push(crate::version::Problem {
                    severity: ProblemSeverity::Error,
                    message: format!("cannot resolve component '{}': {detail}", slot.uid),
                });
                resolution.components.push(ResolvedComponent {
                    uid: slot.uid.clone(),
                    version: if slot.target.is_empty() {
                        slot.pinned.clone()
                    } else {
                        slot.target.clone()
                    },
                    disabled: false,
                    custom: false,
                    problems: Vec::new(),
                });
            }
        }
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

    /// A vanilla instance writes an `org.lwjgl3` slot with no version of its
    /// own; `net.minecraft` is the component that says which one it wants. The
    /// empty version used to be looked up as `<uid>/.json`, which does not
    /// exist, so the whole instance failed to resolve and could not launch.
    #[test]
    fn a_versionless_component_takes_the_version_it_is_required_at() {
        let mut store = MapStore::with(
            "net.minecraft",
            "1.21.1",
            json!({
                "uid": "net.minecraft", "version": "1.21.1", "order": -2,
                "mainClass": "net.minecraft.client.main.Main",
                "requires": [{"uid": "org.lwjgl3", "suggests": "3.3.3"}]
            }),
        );
        store.files.insert(
            ("org.lwjgl3".into(), "3.3.3".into()),
            json!({"uid": "org.lwjgl3", "version": "3.3.3", "order": 3, "+traits": ["lwjgl3"]}),
        );
        let profile = profile_with(vec![
            Component { uid: "net.minecraft".into(), version: "1.21.1".into(), important: true, ..Default::default() },
            Component { uid: "org.lwjgl3".into(), important: true, ..Default::default() },
        ]);
        let r = resolve(&profile, Path::new("/no/patches"), &mut store, &RuntimeContext::current_host()).unwrap();
        assert_eq!(r.severity(), ProblemSeverity::None, "problems: {:?}", r.problems);
        assert!(r.profile.has_trait("lwjgl3"), "LWJGL never reached the profile");
        let lwjgl = r.components.iter().find(|c| c.uid == "org.lwjgl3").unwrap();
        assert_eq!(lwjgl.version, "3.3.3");
    }

    /// The Fabric case, and the reason a Fabric instance created by this
    /// launcher could not start: the loader requires the intermediary mappings
    /// and names no version, and the profile does not list the component at
    /// all. Prism adds it at the game version; so does this.
    #[test]
    fn a_required_component_that_is_not_listed_is_resolved_at_the_game_version() {
        let mut store = MapStore::with(
            "net.minecraft",
            "1.21.1",
            json!({"uid": "net.minecraft", "version": "1.21.1", "order": -2, "mainClass": "M"}),
        );
        store.files.insert(
            ("net.fabricmc.fabric-loader".into(), "0.16.5".into()),
            json!({
                "uid": "net.fabricmc.fabric-loader", "version": "0.16.5", "order": 10,
                "requires": [{"uid": "net.fabricmc.intermediary"}]
            }),
        );
        store.files.insert(
            ("net.fabricmc.intermediary".into(), "1.21.1".into()),
            json!({
                "uid": "net.fabricmc.intermediary", "version": "1.21.1", "order": 11,
                "+traits": ["intermediary"]
            }),
        );
        let profile = profile_with(vec![
            Component { uid: "net.minecraft".into(), version: "1.21.1".into(), important: true, ..Default::default() },
            Component { uid: "net.fabricmc.fabric-loader".into(), version: "0.16.5".into(), important: true, ..Default::default() },
        ]);
        let r = resolve(&profile, Path::new("/no/patches"), &mut store, &RuntimeContext::current_host()).unwrap();
        assert_eq!(r.severity(), ProblemSeverity::None, "problems: {:?}", r.problems);
        assert!(r.profile.has_trait("intermediary"), "mappings never reached the profile");
        let added = r.components.iter().find(|c| c.uid == "net.fabricmc.intermediary").unwrap();
        assert_eq!(added.version, "1.21.1");
    }

    /// The negative control: a component nobody asks about and no version for
    /// is still a refusal, so the fills above cannot have been bought by
    /// inventing a version out of nothing.
    #[test]
    fn a_versionless_component_nobody_requires_is_still_an_error() {
        let mut store = MapStore::with("net.minecraft", "1", json!({"uid": "net.minecraft"}));
        let profile = profile_with(vec![
            Component { uid: "net.minecraft".into(), version: "1".into(), ..Default::default() },
            Component { uid: "org.example.something".into(), ..Default::default() },
        ]);
        let r = resolve(&profile, Path::new("/no"), &mut store, &RuntimeContext::current_host()).unwrap();
        assert_eq!(r.severity(), ProblemSeverity::Error);
        assert!(r
            .problems
            .iter()
            .any(|p| p.message.contains("org.example.something")
                && p.message.contains("no version is pinned")));
    }

    #[test]
    fn an_exact_requirement_outranks_a_suggestion() {
        let mut map = BTreeMap::new();
        compose_into(&mut map, &Require { uid: "org.example".into(), equals_version: String::new(), suggests: "3.3.3".into() });
        compose_into(&mut map, &Require { uid: "org.example".into(), equals_version: "3.2.0".into(), suggests: String::new() });
        let composed = &map["org.example"];
        assert_eq!(composed.equals_version, "3.2.0");
        assert_eq!(composed.suggests, "3.3.3");
        // Two suggestions keep the higher version, in either order.
        compose_into(&mut map, &Require { uid: "org.example".into(), equals_version: String::new(), suggests: "3.4.1".into() });
        assert_eq!(map["org.example"].suggests, "3.4.1");
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
        // The flat list is the older layout and is still read.
        let list = store.version_list("net.minecraft").unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].recommended);

        // The current layout (`<uid>/index.json`) is what a cache written today
        // holds, and it wins when both are present.
        std::fs::create_dir_all(meta.join("net.fabricmc.fabric-loader")).unwrap();
        std::fs::write(
            meta.join("net.fabricmc.fabric-loader").join("index.json"),
            r#"{"formatVersion": 1, "uid": "net.fabricmc.fabric-loader", "versions": [
                {"version": "0.16.5", "type": "release", "recommended": true},
                {"version": "0.16.4", "type": "release"}
            ]}"#,
        )
        .unwrap();
        let modded = store.version_list("net.fabricmc.fabric-loader").unwrap();
        assert_eq!(modded.len(), 2);
        assert_eq!(modded[0].version, "0.16.5");
        assert!(modded[0].recommended);
        assert!(!modded[1].recommended);

        // unknown uid -> empty list, not an error
        assert!(store.version_list("does.not.exist").unwrap().is_empty());
        assert!(store.version_file("does.not.exist", "1").is_err());
    }
}
