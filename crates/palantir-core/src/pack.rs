//! `mmc-pack.json` — the component (patch) list of an instance.
//!
//! Mirrors `launcher/minecraft/PackProfile.cpp` (develop):
//!
//! * Root object: `{"components": [...], "formatVersion": 1}`; any other
//!   `formatVersion` is a hard parse error.
//! * Component fields on **write** (present only when meaningful):
//!   `uid` (always), `version` (non-empty), `dependencyOnly`, `important`,
//!   `disabled` (true), `cachedVersion`, `cachedName` (non-empty),
//!   `cachedRequires` / `cachedConflicts` (non-empty), `cachedVolatile`
//!   (true).
//! * On **read**, `cachedVolatile` is taken from the key `volatile` — a
//!   live asymmetry in Prism's own code that we reproduce verbatim so both
//!   launchers behave identically on the same files.
//! * Duplicate `uid` entries: the first wins, later ones are ignored
//!   ("Ignoring duplicate component entry").
//! * Missing `uid` in a component: hard error (`Json::requireString`).
//! * Output uses `QJsonDocument::Indented` shape: 4-space indent,
//!   alphabetical keys, trailing newline (see [`crate::json`]).

use crate::error::{Error, Result};
use crate::json;
use std::path::Path;

/// A metadata requirement (`Meta::Require`): serialized as
/// `{"uid": ..., "equals": ..., "suggests": ...}` with empty members
/// omitted (`meta/JsonFormat.cpp`).
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Require {
    /// Required component uid.
    pub uid: String,
    /// Exact version requirement (`equals` key).
    pub equals_version: String,
    /// Suggested version (`suggests` key).
    pub suggests: String,
}

impl Require {
    /// Build a requirement that only names a uid.
    pub fn uid(uid: impl Into<String>) -> Require {
        Require { uid: uid.into(), ..Default::default() }
    }

    /// Parse from a JSON object; missing members are empty.
    pub fn from_json(value: &serde_json::Value) -> Option<Require> {
        let obj = value.as_object()?;
        let uid = obj.get("uid")?.as_str()?.to_string();
        let equals_version = obj.get("equals").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let suggests = obj.get("suggests").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        Some(Require { uid, equals_version, suggests })
    }

    /// Serialize per `Meta::serializeRequires` (empty members omitted).
    pub fn to_json(&self) -> serde_json::Value {
        let mut obj = serde_json::Map::new();
        obj.insert("uid".into(), serde_json::Value::String(self.uid.clone()));
        if !self.equals_version.is_empty() {
            obj.insert("equals".into(), serde_json::Value::String(self.equals_version.clone()));
        }
        if !self.suggests.is_empty() {
            obj.insert("suggests".into(), serde_json::Value::String(self.suggests.clone()));
        }
        serde_json::Value::Object(obj)
    }
}

/// Mod loader kinds recognized by Prism (`Component::KNOWN_MODLOADERS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModLoader {
    /// NeoForge (`net.neoforged`).
    NeoForge,
    /// Forge (`net.minecraftforge`).
    Forge,
    /// Fabric (`net.fabricmc.fabric-loader`).
    Fabric,
    /// Quilt (`org.quiltmc.quilt-loader`).
    Quilt,
    /// LiteLoader (`com.mumfrey.liteloader`).
    LiteLoader,
}

impl ModLoader {
    /// The component uid for this loader.
    pub fn uid(self) -> &'static str {
        match self {
            ModLoader::NeoForge => "net.neoforged",
            ModLoader::Forge => "net.minecraftforge",
            ModLoader::Fabric => "net.fabricmc.fabric-loader",
            ModLoader::Quilt => "org.quiltmc.quilt-loader",
            ModLoader::LiteLoader => "com.mumfrey.liteloader",
        }
    }

    /// Recognize a loader by uid.
    pub fn from_uid(uid: &str) -> Option<ModLoader> {
        match uid {
            "net.neoforged" => Some(ModLoader::NeoForge),
            "net.minecraftforge" => Some(ModLoader::Forge),
            "net.fabricmc.fabric-loader" => Some(ModLoader::Fabric),
            "org.quiltmc.quilt-loader" => Some(ModLoader::Quilt),
            "com.mumfrey.liteloader" => Some(ModLoader::LiteLoader),
            _ => None,
        }
    }

    /// Uids that conflict with this loader (`knownConflictingComponents`).
    pub fn conflicting_uids(self) -> &'static [&'static str] {
        match self {
            ModLoader::NeoForge => &["net.minecraftforge", "net.fabricmc.fabric-loader", "org.quiltmc.quilt-loader"],
            ModLoader::Forge => &["net.neoforged", "net.fabricmc.fabric-loader", "org.quiltmc.quilt-loader"],
            ModLoader::Fabric => &["net.minecraftforge", "net.neoforged", "org.quiltmc.quilt-loader"],
            ModLoader::Quilt => &["net.minecraftforge", "net.neoforged", "net.fabricmc.fabric-loader"],
            ModLoader::LiteLoader => &[],
        }
    }
}

/// One entry of `mmc-pack.json` (`Component`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Component {
    /// Component uid (required).
    pub uid: String,
    /// Requested version (empty = whatever the metadata provides).
    pub version: String,
    /// `dependencyOnly`: pulled in by another component, cannot be disabled.
    pub dependency_only: bool,
    /// `important`: user-requested core component (cannot be removed).
    pub important: bool,
    /// `disabled`: excluded from resolution and launch.
    pub disabled: bool,
    /// Cached resolved version (`cachedVersion`).
    pub cached_version: String,
    /// Cached display name (`cachedName`).
    pub cached_name: String,
    /// Cached requirements (`cachedRequires`).
    pub cached_requires: Vec<Require>,
    /// Cached conflicts (`cachedConflicts`).
    pub cached_conflicts: Vec<Require>,
    /// Cached volatility. NOTE: read from the `volatile` key and written as
    /// `cachedVolatile`, exactly like Prism.
    pub cached_volatile: bool,
}

impl Component {
    /// `important` components are non-removable (`Component::isRemovable`).
    pub fn is_removable(&self) -> bool {
        !self.important
    }

    /// `dependencyOnly` components cannot be disabled. Note `important`
    /// only controls removability: an important component can still be
    /// toggled (but disabling is ineffective, see `is_enabled`).
    pub fn can_be_disabled(&self) -> bool {
        !self.dependency_only
    }

    /// Effective enabled state (`Component::isEnabled`): disabling an
    /// `important` or `dependencyOnly` component has no effect.
    pub fn is_enabled(&self) -> bool {
        !self.disabled || self.important || self.dependency_only
    }

    /// Serialize per `componentToJsonV1` (field presence rules above).
    pub fn to_json(&self) -> serde_json::Value {
        let mut obj = serde_json::Map::new();
        obj.insert("uid".into(), serde_json::Value::String(self.uid.clone()));
        if !self.version.is_empty() {
            obj.insert("version".into(), serde_json::Value::String(self.version.clone()));
        }
        if self.dependency_only {
            obj.insert("dependencyOnly".into(), serde_json::Value::Bool(true));
        }
        if self.important {
            obj.insert("important".into(), serde_json::Value::Bool(true));
        }
        if self.disabled {
            obj.insert("disabled".into(), serde_json::Value::Bool(true));
        }
        if !self.cached_version.is_empty() {
            obj.insert("cachedVersion".into(), serde_json::Value::String(self.cached_version.clone()));
        }
        if !self.cached_name.is_empty() {
            obj.insert("cachedName".into(), serde_json::Value::String(self.cached_name.clone()));
        }
        if !self.cached_requires.is_empty() {
            // Prism stores requirements in a `RequireSet` ordered by uid, so
            // serialization is sorted by uid regardless of insertion order.
            let mut reqs = self.cached_requires.clone();
            reqs.sort_by(|a, b| a.uid.cmp(&b.uid));
            obj.insert(
                "cachedRequires".into(),
                serde_json::Value::Array(reqs.iter().map(Require::to_json).collect()),
            );
        }
        if !self.cached_conflicts.is_empty() {
            let mut confs = self.cached_conflicts.clone();
            confs.sort_by(|a, b| a.uid.cmp(&b.uid));
            obj.insert(
                "cachedConflicts".into(),
                serde_json::Value::Array(confs.iter().map(Require::to_json).collect()),
            );
        }
        if self.cached_volatile {
            obj.insert("cachedVolatile".into(), serde_json::Value::Bool(true));
        }
        serde_json::Value::Object(obj)
    }

    /// Parse per `componentFromJsonV1` (read `volatile`, not
    /// `cachedVolatile`). Missing `uid` is an error, matching
    /// `Json::requireString`.
    pub fn from_json(value: &serde_json::Value, path: &Path) -> Result<Component> {
        let obj = value
            .as_object()
            .ok_or_else(|| Error::json(path, "Component must be an object"))?;
        let uid = obj
            .get("uid")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::MissingField { path: path.to_path_buf(), field: "uid" })?
            .to_string();
        let get_bool = |key: &str| obj.get(key).and_then(|v| v.as_bool()).unwrap_or(false);
        let get_str = |key: &str| {
            obj.get(key).and_then(|v| v.as_str()).unwrap_or_default().to_string()
        };
        let get_reqs = |key: &str| {
            obj.get(key)
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(Require::from_json).collect::<Vec<_>>())
                .unwrap_or_default()
        };
        Ok(Component {
            uid,
            version: get_str("version"),
            dependency_only: get_bool("dependencyOnly"),
            important: get_bool("important"),
            disabled: get_bool("disabled"),
            cached_version: get_str("cachedVersion"),
            cached_name: get_str("cachedName"),
            cached_requires: get_reqs("cachedRequires"),
            cached_conflicts: get_reqs("cachedConflicts"),
            // Prism quirk: reads "volatile", writes "cachedVolatile".
            cached_volatile: get_bool("volatile"),
        })
    }
}

/// The component list of an instance (`PackProfile`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackProfile {
    components: Vec<Component>,
}

impl PackProfile {
    /// Parse `mmc-pack.json` text.
    pub fn from_text(text: &str, path: &Path) -> Result<PackProfile> {
        let root: serde_json::Value =
            serde_json::from_str(text).map_err(|e| Error::json(path, e.to_string()))?;
        let obj = root
            .as_object()
            .ok_or_else(|| Error::json(path, "root must be an object"))?;
        let version = obj
            .get("formatVersion")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| Error::MissingField { path: path.to_path_buf(), field: "formatVersion" })?;
        if version != 1 {
            return Err(Error::format(path, format!("Invalid component file version, expected 1 (found {version})")));
        }
        let items = obj
            .get("components")
            .and_then(|v| v.as_array())
            .ok_or_else(|| Error::MissingField { path: path.to_path_buf(), field: "components" })?;
        let mut components: Vec<Component> = Vec::with_capacity(items.len());
        for item in items {
            let comp = Component::from_json(item, path)?;
            if components.iter().any(|c| c.uid == comp.uid) {
                continue; // "Ignoring duplicate component entry"
            }
            components.push(comp);
        }
        Ok(PackProfile { components })
    }

    /// Load from disk.
    pub fn load(path: &Path) -> Result<PackProfile> {
        let text = crate::util::read_text(path)?;
        PackProfile::from_text(&text, path)
    }

    /// Serialize exactly like `savePackProfile`.
    pub fn to_text(&self) -> String {
        let mut root = serde_json::Map::new();
        root.insert(
            "components".into(),
            serde_json::Value::Array(self.components.iter().map(Component::to_json).collect()),
        );
        root.insert("formatVersion".into(), serde_json::Value::from(1));
        json::to_document_string(&serde_json::Value::Object(root))
            .unwrap_or_else(|_| "{\"components\":[],\"formatVersion\":1}\n".to_string())
    }

    /// Save to disk atomically.
    pub fn save(&self, path: &Path) -> Result<()> {
        crate::util::atomic_write(path, self.to_text().as_bytes())
    }

    /// Components in file order (this is the resolution order basis).
    pub fn components(&self) -> &[Component] {
        &self.components
    }

    /// Mutable components in file order.
    pub fn components_mut(&mut self) -> &mut [Component] {
        &mut self.components
    }

    /// Lookup by uid.
    pub fn get(&self, uid: &str) -> Option<&Component> {
        self.components.iter().find(|c| c.uid == uid)
    }

    /// Mutable lookup by uid.
    pub fn get_mut(&mut self, uid: &str) -> Option<&mut Component> {
        self.components.iter_mut().find(|c| c.uid == uid)
    }

    /// Insert at `index`; silently ignored when the uid already exists
    /// (`PackProfile::insertComponent`) or when the uid is empty.
    pub fn insert(&mut self, index: usize, component: Component) {
        if component.uid.is_empty() || self.components.iter().any(|c| c.uid == component.uid) {
            return;
        }
        let index = index.min(self.components.len());
        self.components.insert(index, component);
    }

    /// Append a component.
    pub fn append(&mut self, component: Component) {
        self.insert(self.components.len(), component);
    }

    /// Remove a component by uid (only when removable, like
    /// `PackProfile::remove`). Returns whether anything was removed.
    pub fn remove(&mut self, uid: &str) -> bool {
        match self.get(uid) {
            Some(c) if c.is_removable() => {
                self.components.retain(|c| c.uid != uid);
                true
            }
            _ => false,
        }
    }

    /// Set (or create) a component version (`PackProfile::setComponentVersion`).
    /// Returns true when the profile changed.
    pub fn set_version(&mut self, uid: &str, version: &str, important: bool) -> bool {
        if let Some(c) = self.get_mut(uid) {
            c.version = version.to_string();
            c.important = important;
            true
        } else {
            self.append(Component {
                uid: uid.to_string(),
                version: version.to_string(),
                important,
                ..Default::default()
            });
            true
        }
    }

    /// Recognized mod loader for a uid, if any.
    pub fn modloader_of(uid: &str) -> Option<ModLoader> {
        ModLoader::from_uid(uid)
    }

    /// Enabled mod loaders present in the profile
    /// (`PackProfile::getModLoaders`).
    pub fn mod_loaders(&self) -> Vec<ModLoader> {
        self.components
            .iter()
            .filter(|c| c.is_enabled())
            .filter_map(|c| ModLoader::from_uid(&c.uid))
            .collect()
    }

    /// A minimal vanilla profile: one component, `net.minecraft` pinned to
    /// `version`, marked important.
    ///
    /// Prism writes a second slot beside it -- `org.lwjgl3` -- because its own
    /// `net.minecraft` file has no LWJGL entries at all: it serves them as a
    /// component of their own and names that component in a `requires`. Mojang's
    /// file keeps the libraries (56 of them for 1.21.4, plain and natives both),
    /// so a reader of piston has nothing to put in that slot -- it would name the
    /// same jars a second time, under the two naming schemes. An instance that
    /// already carries the slot, written by Prism or by a build of this launcher
    /// from before it read piston, resolves it as a component with nothing behind
    /// it instead of failing ([`crate::resolve`]), and Prism adds the slot back
    /// for itself when it opens a profile whose `net.minecraft` requires one.
    pub fn vanilla(version: &str) -> PackProfile {
        let mut p = PackProfile::default();
        p.append(Component {
            uid: "net.minecraft".into(),
            version: version.to_string(),
            important: true,
            ..Default::default()
        });
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOLDEN: &str = concat!(
        "{\n",
        "    \"components\": [\n",
        "        {\n",
        "            \"cachedName\": \"Minecraft\",\n",
        "            \"cachedVersion\": \"1.21.1\",\n",
        "            \"important\": true,\n",
        "            \"uid\": \"net.minecraft\"\n",
        "        },\n",
        "        {\n",
        "            \"cachedName\": \"Fabric Loader\",\n",
        "            \"cachedVersion\": \"0.16.5\",\n",
        "            \"important\": true,\n",
        "            \"uid\": \"net.fabricmc.fabric-loader\"\n",
        "        }\n",
        "    ],\n",
        "    \"formatVersion\": 1\n",
        "}\n"
    );

    #[test]
    fn save_matches_prism_byte_for_byte() {
        let mut p = PackProfile::default();
        p.append(Component {
            uid: "net.minecraft".into(),
            important: true,
            cached_version: "1.21.1".into(),
            cached_name: "Minecraft".into(),
            ..Default::default()
        });
        p.append(Component {
            uid: "net.fabricmc.fabric-loader".into(),
            important: true,
            cached_version: "0.16.5".into(),
            cached_name: "Fabric Loader".into(),
            ..Default::default()
        });
        assert_eq!(p.to_text(), GOLDEN);
    }

    #[test]
    fn load_round_trips_and_enforces_presence_rules() {
        let p = PackProfile::from_text(GOLDEN, Path::new("t")).unwrap();
        assert_eq!(p.components().len(), 2);
        let mc = p.get("net.minecraft").unwrap();
        assert_eq!(mc.version, "");
        assert_eq!(mc.cached_version, "1.21.1");
        assert!(mc.important && !mc.disabled && !mc.dependency_only && !mc.cached_volatile);

        // save -> load identity
        let again = PackProfile::from_text(&p.to_text(), Path::new("t")).unwrap();
        assert_eq!(p, again);
    }

    #[test]
    fn volatile_read_quirk_is_reproduced() {
        // Prism writes "cachedVolatile" but READS "volatile": a file with
        // "cachedVolatile": true must parse as false, and one with
        // "volatile": true must parse as true.
        let written = PackProfile::from_text(
            "{\"formatVersion\":1,\"components\":[{\"uid\":\"a\",\"cachedVolatile\":true}]}",
            Path::new("t"),
        )
        .unwrap();
        assert!(!written.get("a").unwrap().cached_volatile);
        let legacy = PackProfile::from_text(
            "{\"formatVersion\":1,\"components\":[{\"uid\":\"a\",\"volatile\":true}]}",
            Path::new("t"),
        )
        .unwrap();
        assert!(legacy.get("a").unwrap().cached_volatile);
        // ...and the written form uses cachedVolatile again
        assert!(legacy.to_text().contains("\"cachedVolatile\": true"));
        assert!(!legacy.to_text().contains("\"volatile\": true"));
    }

    #[test]
    fn duplicate_uid_first_wins() {
        let p = PackProfile::from_text(
            "{\"formatVersion\":1,\"components\":[{\"uid\":\"a\",\"version\":\"1\"},{\"uid\":\"a\",\"version\":\"2\"}]}",
            Path::new("t"),
        )
        .unwrap();
        assert_eq!(p.components().len(), 1);
        assert_eq!(p.get("a").unwrap().version, "1");
    }

    #[test]
    fn wrong_format_version_and_missing_uid_are_errors() {
        let err = PackProfile::from_text(
            "{\"formatVersion\":2,\"components\":[]}",
            Path::new("t"),
        )
        .unwrap_err();
        assert!(matches!(err, Error::Format { .. }));
        let err = PackProfile::from_text(
            "{\"formatVersion\":1,\"components\":[{\"version\":\"1\"}]}",
            Path::new("t"),
        )
        .unwrap_err();
        assert!(matches!(err, Error::MissingField { field: "uid", .. }));
        let err = PackProfile::from_text("not json", Path::new("t")).unwrap_err();
        assert!(matches!(err, Error::Json { .. }));
        assert!(matches!(PackProfile::from_text("[]", Path::new("t")), Err(Error::Json { .. })));
    }

    #[test]
    fn requires_serialize_with_equals_and_suggests_keys() {
        let mut c = Component { uid: "x".into(), ..Default::default() };
        c.cached_requires = vec![
            Require { uid: "org.lwjgl3".into(), suggests: "3.3.2".into(), equals_version: String::new() },
            Require { uid: "net.minecraft".into(), equals_version: "1.21.1".into(), suggests: String::new() },
        ];
        let json = c.to_json();
        let reqs = json["cachedRequires"].as_array().unwrap();
        // sorted because serde_json Map is a BTreeMap: keys alphabetical per object
        assert_eq!(reqs[0]["uid"], "net.minecraft");
        assert_eq!(reqs[0]["equals"], "1.21.1");
        assert!(reqs[0].get("suggests").is_none());
        assert_eq!(reqs[1]["uid"], "org.lwjgl3");
        assert_eq!(reqs[1]["suggests"], "3.3.2");
        assert!(reqs[1].get("equals").is_none());
    }

    #[test]
    fn component_semantics_match_prism() {
        let mut c = Component { uid: "x".into(), important: true, ..Default::default() };
        assert!(!c.is_removable());
        assert!(c.can_be_disabled()); // important but not dependencyOnly
        c.disabled = true;
        assert!(c.is_enabled()); // important => cannot be disabled effectively
        c.important = false;
        assert!(!c.is_enabled());
        c.dependency_only = true;
        assert!(c.is_enabled()); // dependencyOnly => cannot be disabled
    }

    #[test]
    fn profile_mutations_follow_prism_rules() {
        let mut p = PackProfile::vanilla("1.21.1");
        assert_eq!(p.components().len(), 1);
        assert_eq!(p.components()[0].uid, "net.minecraft");
        // important components are not removable
        assert!(!p.remove("net.minecraft"));
        // a non-important component is removable
        p.append(Component { uid: "custom.thing".into(), ..Default::default() });
        assert!(p.remove("custom.thing"));
    }

    #[test]
    fn insert_ignores_duplicates_and_empty_uids() {
        let mut p = PackProfile::default();
        p.append(Component { uid: "a".into(), ..Default::default() });
        p.append(Component { uid: "a".into(), version: "2".into(), ..Default::default() });
        assert_eq!(p.components().len(), 1);
        p.insert(0, Component { uid: String::new(), ..Default::default() });
        assert_eq!(p.components().len(), 1);
        p.insert(99, Component { uid: "b".into(), ..Default::default() }); // clamps to end
        assert!(p.get("b").is_some());
        p.insert(0, Component { uid: "c".into(), ..Default::default() });
        assert_eq!(p.components()[0].uid, "c");
    }

    #[test]
    fn set_version_creates_or_updates() {
        let mut p = PackProfile::default();
        assert!(p.set_version("net.minecraft", "1.20.4", true));
        assert_eq!(p.get("net.minecraft").unwrap().version, "1.20.4");
        assert!(p.set_version("net.minecraft", "1.21.1", false));
        let c = p.get("net.minecraft").unwrap();
        assert_eq!(c.version, "1.21.1");
        assert!(!c.important);
    }

    #[test]
    fn mod_loaders_and_conflicts_match_known_table() {
        assert_eq!(ModLoader::from_uid("net.fabricmc.fabric-loader"), Some(ModLoader::Fabric));
        assert_eq!(ModLoader::from_uid("unknown"), None);
        assert_eq!(
            ModLoader::NeoForge.conflicting_uids(),
            &["net.minecraftforge", "net.fabricmc.fabric-loader", "org.quiltmc.quilt-loader"]
        );
        assert!(ModLoader::LiteLoader.conflicting_uids().is_empty());
        let mut p = PackProfile::default();
        p.append(Component { uid: "net.fabricmc.fabric-loader".into(), ..Default::default() });
        p.append(Component { uid: "net.minecraftforge".into(), disabled: true, ..Default::default() });
        assert_eq!(p.mod_loaders(), vec![ModLoader::Fabric]);
    }
}
