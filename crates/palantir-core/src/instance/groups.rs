//! `instgroups.json` — instance groups.
//!
//! Mirrors `InstanceList::saveGroupList` / `loadGroupList`:
//! `{"formatVersion": "1", "groups": {"Name": {"hidden": bool,
//! "instances": [id, ...]}}, "ungrouped": {"hidden": true}?}`.
//! `formatVersion` is written as the *string* `"1"` but accepted as string
//! or integer on load. The file lives at the data root; a legacy copy inside
//! the instances dir is loaded as fallback and re-saved to the new location.

use crate::error::Result;
use crate::json;
use crate::paths::PalantirPaths;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Group membership index plus collapsed ("hidden") groups. Empty-string
/// group means ungrouped (only used for the collapsed flag).
#[derive(Debug, Clone, Default)]
pub struct Groups {
    index: BTreeMap<String, String>,
    collapsed: BTreeSet<String>,
}

impl Groups {
    /// Load groups from the data root, falling back to the legacy location
    /// inside the instances dir (which is then re-saved at the new location,
    /// like `loadGroupList`). Corrupt or wrong-version files yield an empty
    /// index (Prism logs and returns).
    pub fn load(paths: &PalantirPaths) -> Groups {
        let primary = paths.groups_file();
        let mut migrated = false;
        let file = if primary.exists() {
            primary
        } else {
            let legacy = paths.legacy_groups_file();
            if !legacy.exists() {
                return Groups::default();
            }
            migrated = true;
            legacy
        };
        let groups = Groups::from_file(&file);
        if migrated {
            let _ = groups.save(paths);
        }
        groups
    }

    /// Parse a group file.
    pub fn from_file(path: &Path) -> Groups {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(_) => return Groups::default(),
        };
        Groups::from_text(&text)
    }

    /// Parse group JSON text.
    pub fn from_text(text: &str) -> Groups {
        let mut out = Groups::default();
        let Ok(root) = serde_json::from_str::<serde_json::Value>(text) else {
            return out;
        };
        let Some(obj) = root.as_object() else { return out };
        // formatVersion: string "1" or number 1 (toVariant().toInt()).
        let version_ok = match obj.get("formatVersion") {
            Some(serde_json::Value::String(s)) => s == "1",
            Some(serde_json::Value::Number(n)) => n.as_i64() == Some(1),
            _ => false,
        };
        if !version_ok {
            return out;
        }
        if let Some(groups) = obj.get("groups").and_then(|g| g.as_object()) {
            for (name, value) in groups {
                if name.is_empty() {
                    continue; // "Redundant empty group found"
                }
                let Some(gobj) = value.as_object() else { continue };
                if !gobj.get("instances").is_some_and(|i| i.is_array()) {
                    continue;
                }
                if gobj.get("hidden").and_then(|h| h.as_bool()).unwrap_or(false) {
                    out.collapsed.insert(name.clone());
                }
                if let Some(items) = gobj.get("instances").and_then(|i| i.as_array()) {
                    for item in items {
                        if let Some(id) = item.as_str() {
                            out.index.insert(id.to_string(), name.clone());
                        }
                    }
                }
            }
        }
        if obj
            .get("ungrouped")
            .and_then(|u| u.get("hidden"))
            .and_then(|h| h.as_bool())
            .unwrap_or(false)
        {
            out.collapsed.insert(String::new());
        }
        out
    }

    /// Serialize exactly like `saveGroupList` (alphabetical keys via
    /// `BTreeMap`, 4-space indent, trailing newline; `formatVersion` as a
    /// string; instance lists sorted for determinism where Prism's QSet
    /// order is unspecified).
    pub fn to_text(&self) -> String {
        let mut reverse: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (id, group) in &self.index {
            if group.is_empty() {
                continue;
            }
            reverse.entry(group).or_default().push(id);
        }
        for ids in reverse.values_mut() {
            ids.sort_unstable();
            ids.dedup();
        }
        let mut groups = serde_json::Map::new();
        for (name, ids) in &reverse {
            let instances = serde_json::Value::Array(ids.iter().map(|i| serde_json::Value::String((*i).to_string())).collect());
            let hidden = serde_json::Value::Bool(self.collapsed.contains(*name));
            let mut gobj = serde_json::Map::new();
            gobj.insert("hidden".to_string(), hidden);
            gobj.insert("instances".to_string(), instances);
            groups.insert((*name).to_string(), serde_json::Value::Object(gobj));
        }
        let mut root = serde_json::Map::new();
        root.insert("formatVersion".to_string(), serde_json::Value::String("1".to_string()));
        root.insert("groups".to_string(), serde_json::Value::Object(groups));
        if self.collapsed.contains("") {
            let mut ungrouped = serde_json::Map::new();
            ungrouped.insert("hidden".to_string(), serde_json::Value::Bool(true));
            root.insert("ungrouped".to_string(), serde_json::Value::Object(ungrouped));
        }
        json::to_document_string(&serde_json::Value::Object(root)).unwrap_or_else(|_| "{}\n".to_string())
    }

    /// Save to the data root.
    pub fn save(&self, paths: &PalantirPaths) -> Result<()> {
        crate::util::atomic_write(&paths.groups_file(), self.to_text().as_bytes())
    }

    /// Group of an instance id, if any.
    pub fn group_of(&self, id: &str) -> Option<&str> {
        self.index.get(id).map(|s| s.as_str())
    }

    /// Assign (or clear with `None`) an instance's group.
    pub fn set_group(&mut self, id: &str, group: Option<&str>) {
        match group {
            Some(g) if !g.is_empty() => {
                self.index.insert(id.to_string(), g.to_string());
            }
            _ => {
                self.index.remove(id);
            }
        }
    }

    /// All group names with at least one member.
    pub fn names(&self) -> Vec<&str> {
        let mut out: Vec<&str> = self.index.values().map(|s| s.as_str()).collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Mark a group (or the empty string for "ungrouped") collapsed.
    pub fn set_collapsed(&mut self, group: &str, collapsed: bool) {
        if collapsed {
            self.collapsed.insert(group.to_string());
        } else {
            self.collapsed.remove(group);
        }
    }

    /// Whether a group is collapsed.
    pub fn is_collapsed(&self, group: &str) -> bool {
        self.collapsed.contains(group)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::PalantirPaths;

    #[test]
    fn round_trip_matches_prism_shape() {
        let mut g = Groups::default();
        g.set_group("a", Some("Packs"));
        g.set_group("b", Some("Packs"));
        g.set_group("c", Some("Vanilla"));
        g.set_collapsed("Packs", true);
        g.set_collapsed("", true);
        let text = g.to_text();
        let expected = concat!(
            "{\n",
            "    \"formatVersion\": \"1\",\n",
            "    \"groups\": {\n",
            "        \"Packs\": {\n",
            "            \"hidden\": true,\n",
            "            \"instances\": [\n",
            "                \"a\",\n",
            "                \"b\"\n",
            "            ]\n",
            "        },\n",
            "        \"Vanilla\": {\n",
            "            \"hidden\": false,\n",
            "            \"instances\": [\n",
            "                \"c\"\n",
            "            ]\n",
            "        }\n",
            "    },\n",
            "    \"ungrouped\": {\n",
            "        \"hidden\": true\n",
            "    }\n",
            "}\n"
        );
        assert_eq!(text, expected);
        let back = Groups::from_text(&text);
        assert_eq!(back.group_of("a"), Some("Packs"));
        assert_eq!(back.group_of("c"), Some("Vanilla"));
        assert!(back.is_collapsed("Packs"));
        assert!(back.is_collapsed(""));
        assert_eq!(back.names(), vec!["Packs", "Vanilla"]);
    }

    #[test]
    fn load_accepts_integer_format_version_and_skips_bad_groups() {
        let text = concat!(
            "{\n \"formatVersion\": 1,\n",
            " \"groups\": {\n",
            "   \"\": {\"hidden\": false, \"instances\": [\"x\"]},\n",
            "   \"Bad\": {\"instances\": \"nope\"},\n",
            "   \"Ok\": {\"hidden\": true, \"instances\": [\"i1\", 42, null]},\n",
            "   \"NotObj\": []\n",
            " }\n}\n"
        );
        let g = Groups::from_text(text);
        assert_eq!(g.group_of("i1"), Some("Ok"));
        assert!(g.group_of("x").is_none());
        assert!(g.is_collapsed("Ok"));
        assert_eq!(g.names(), vec!["Ok"]);
    }

    #[test]
    fn corrupt_or_wrong_version_yields_empty() {
        assert!(Groups::from_text("{not json").index.is_empty());
        assert!(Groups::from_text("{\"formatVersion\": \"2\", \"groups\": {}}").index.is_empty());
        assert!(Groups::from_text("[]").index.is_empty());
    }

    #[test]
    fn save_and_load_file_round_trip_with_legacy_migration() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = PalantirPaths::at(tmp.path());
        std::fs::create_dir_all(paths.instances_dir()).unwrap();
        // legacy file inside instances dir
        std::fs::write(paths.legacy_groups_file(), "{\"formatVersion\": \"1\", \"groups\": {\"Old\": {\"hidden\": false, \"instances\": [\"inst\"]}}}").unwrap();
        let g = Groups::load(&paths);
        assert_eq!(g.group_of("inst"), Some("Old"));
        // migrated to the data root
        assert!(paths.groups_file().exists());
        let g2 = Groups::load(&paths);
        assert_eq!(g2.group_of("inst"), Some("Old"));
    }

    #[test]
    fn missing_files_yield_empty_index() {
        let tmp = tempfile::tempdir().unwrap();
        let g = Groups::load(&PalantirPaths::at(tmp.path()));
        assert!(g.index.is_empty() && g.collapsed.is_empty());
    }
}
