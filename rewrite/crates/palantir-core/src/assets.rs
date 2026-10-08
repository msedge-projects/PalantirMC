//! The asset index: asset names, their hashes, and where each one lives.
//!
//! An index is a flat map from in-game path (`lang/en_us.json`,
//! `icons/icon_16x16.png`) to a content hash and size. The same object store
//! serves every version, addressed by hash; three layouts are in the wild and
//! which one applies is the index's own flags:
//!
//! - plain (the common case): `objects/<first two hash chars>/<hash>`;
//! - `map_to_resources: true` (the pre-1.6 index): copies go into the game
//!   directory's `resources/` tree, at their in-game path;
//! - `virtual: true`: copies go into a per-index `virtual/<index id>/` tree
//!   that the game reads as its asset root.
//!
//! Object URLs are the content service's hash-addressed layout:
//! `<base>/<first two hash chars>/<hash>`. The base is written down in one
//! constant so a service move edits one line.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::version::is_false;

/// Where the asset objects are served from.
pub const OBJECT_URL_BASE: &str = "https://resources.download.minecraft.net";

/// One version's (or era's) asset index document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AssetIndex {
    /// In-game path -> object.
    pub objects: BTreeMap<String, AssetObject>,
    /// Pre-1.6: assets are copied into the game dir's `resources/`.
    #[serde(rename = "map_to_resources", default, skip_serializing_if = "is_false")]
    pub map_to_resources: bool,
    /// Assets are copied into a per-index `virtual/` tree. (`virtual` is a
    /// reserved word in edition 2024, so the format's key is a rename.)
    #[serde(rename = "virtual", default, skip_serializing_if = "is_false")]
    pub is_virtual: bool,
    #[serde(
        rename = "min_launcher_version",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub min_launcher_version: Option<u32>,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetObject {
    /// SHA-1 of the object's contents, hex.
    pub hash: String,
    pub size: u64,
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, Value>,
}

/// Which of the three layouts an index asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetDestination {
    /// The shared hash-addressed object store.
    ObjectStore,
    /// The game directory's `resources/` tree.
    Resources,
    /// A per-index `virtual/<index id>/` tree.
    Virtual,
}

impl AssetIndex {
    /// Parse an index document.
    pub fn parse(text: &str) -> Result<Self> {
        serde_json::from_str(text).map_err(Error::parse("asset index"))
    }

    /// Write it back; parse -> serialize -> parse is value-stable.
    pub fn to_json_string(&self) -> Result<String> {
        serde_json::to_string(self).map_err(Error::parse("asset index"))
    }

    /// The layout this index asks for.
    pub fn destination(&self) -> AssetDestination {
        if self.map_to_resources {
            AssetDestination::Resources
        } else if self.is_virtual {
            AssetDestination::Virtual
        } else {
            AssetDestination::ObjectStore
        }
    }

    /// Where the named asset lives, relative to its root. `index_id` is the
    /// index's own id (the file is not self-describing), needed only by the
    /// virtual layout.
    pub fn rel_path(&self, index_id: &str, name: &str) -> Result<String> {
        match self.destination() {
            AssetDestination::ObjectStore => {
                let object = self.objects.get(name).ok_or_else(|| Error::Invalid {
                    what: "asset index",
                    why: format!("no object named {name}"),
                })?;
                object.rel_path()
            }
            AssetDestination::Resources => Ok(format!("resources/{name}")),
            AssetDestination::Virtual => Ok(format!("virtual/{index_id}/{name}")),
        }
    }
}

impl AssetObject {
    /// The object's path in the shared store: `objects/<hh>/<hash>`.
    pub fn rel_path(&self) -> Result<String> {
        let prefix = hash_prefix(&self.hash)?;
        Ok(format!("objects/{prefix}/{}", self.hash))
    }

    /// Where the content service serves this object from.
    pub fn url(&self) -> Result<String> {
        let prefix = hash_prefix(&self.hash)?;
        Ok(format!("{OBJECT_URL_BASE}/{prefix}/{}", self.hash))
    }
}

/// The two hex characters a hash is bucketed under.
fn hash_prefix(hash: &str) -> Result<&str> {
    hash.get(..2)
        .filter(|p| !p.is_empty())
        .ok_or_else(|| Error::Invalid {
            what: "asset hash",
            why: format!("{hash:?} is too short to bucket"),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index(json: &str) -> AssetIndex {
        AssetIndex::parse(json).unwrap()
    }

    #[test]
    fn plain_index_goes_to_the_object_store() {
        let idx = index(r#"{"objects": {"a/b.ogg": {"hash": "abcdef", "size": 3}}}"#);
        assert_eq!(idx.destination(), AssetDestination::ObjectStore);
        assert_eq!(
            idx.rel_path("1.12", "a/b.ogg").unwrap(),
            "objects/ab/abcdef"
        );
        assert_eq!(
            idx.objects["a/b.ogg"].url().unwrap(),
            "https://resources.download.minecraft.net/ab/abcdef"
        );
    }

    #[test]
    fn pre16_index_maps_to_resources() {
        let idx = index(
            r#"{"map_to_resources": true,
                "objects": {"a/b.ogg": {"hash": "abcdef", "size": 3}}}"#,
        );
        assert_eq!(idx.destination(), AssetDestination::Resources);
        assert_eq!(
            idx.rel_path("pre-1.6", "a/b.ogg").unwrap(),
            "resources/a/b.ogg"
        );
    }

    #[test]
    fn virtual_index_names_its_own_tree() {
        let idx = index(
            r#"{"virtual": true,
                "objects": {"a/b.ogg": {"hash": "abcdef", "size": 3}}}"#,
        );
        assert_eq!(idx.destination(), AssetDestination::Virtual);
        assert_eq!(
            idx.rel_path("realms", "a/b.ogg").unwrap(),
            "virtual/realms/a/b.ogg"
        );
    }

    #[test]
    fn a_short_hash_is_refused_not_panicking() {
        let idx = index(r#"{"objects": {"a": {"hash": "z", "size": 1}}}"#);
        assert!(idx.rel_path("x", "a").is_err());
        assert!(idx.objects["a"].url().is_err());
    }

    #[test]
    fn absent_flags_stay_absent() {
        let idx = index(r#"{"objects": {}}"#);
        let out = idx.to_json_string().unwrap();
        assert!(
            !out.contains("map_to_resources"),
            "false flag leaked: {out}"
        );
    }
}
