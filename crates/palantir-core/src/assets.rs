//! Asset index handling — port of the relevant parts of
//! `minecraft/AssetsUtils.cpp`: index JSON parsing and the object/virtual
//! directory mapping used for `${game_assets}`.
//!
//! The download pipeline itself belongs to `palantir-net` (phase 2).

use crate::error::{Error, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One object entry of an asset index.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssetObject {
    /// SHA-1 hash of the object (lowercase hex).
    pub hash: String,
    /// Size in bytes.
    pub size: i64,
}

/// A parsed asset index (`objects` map only; other keys are ignored).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssetIndex {
    /// Map of logical name -> object.
    pub objects: BTreeMap<String, AssetObject>,
}

impl AssetIndex {
    /// Parse an asset index JSON document.
    pub fn parse(text: &str) -> Result<AssetIndex> {
        let value: serde_json::Value =
            serde_json::from_str(text).map_err(|e| Error::json("<asset index>", e.to_string()))?;
        AssetIndex::from_value(&value)
    }

    /// Parse from a JSON value.
    pub fn from_value(value: &serde_json::Value) -> Result<AssetIndex> {
        let mut index = AssetIndex::default();
        let objects = value
            .get("objects")
            .and_then(|v| v.as_object())
            .ok_or_else(|| Error::json("<asset index>", "missing 'objects'"))?;
        for (name, obj) in objects {
            let hash = obj
                .get("hash")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::json("<asset index>", format!("missing 'hash' for '{name}'")))?
                .to_string();
            let size = obj.get("size").and_then(|v| v.as_i64()).unwrap_or(0);
            index.objects.insert(name.clone(), AssetObject { hash, size });
        }
        Ok(index)
    }
}

/// Storage-relative path of an object:
/// `objects/<first two hash chars>/<full hash>`
/// (`AssetsUtils::getAssetPath` layout).
///
/// This is the layout **on disk**, under the data root's `assets/` folder — the
/// one Prism writes too. It is not a URL: the CDN serves the same object at
/// [`object_cdn_path`], with no `objects/` segment, and asking it for this
/// string is a 404 (see `NEXT_STEPS.md` §19).
pub fn object_relative_path(hash: &str) -> String {
    let prefix = hash.get(0..2).unwrap_or("");
    format!("objects/{prefix}/{hash}")
}

/// URL-relative path of an object on Mojang's resource CDN:
/// `<first two hash chars>/<full hash>`, to be appended to
/// `https://resources.download.minecraft.net`.
///
/// Separate from [`object_relative_path`] on purpose. Both are derived from the
/// hash, which is exactly why one helper served both for so long and sent every
/// asset request to a path the CDN does not have: the storage layout has an
/// `objects/` segment and the CDN layout does not. Two functions with two names
/// make that difference something the compiler and a test can hold apart.
pub fn object_cdn_path(hash: &str) -> String {
    let prefix = hash.get(0..2).unwrap_or("");
    format!("{prefix}/{hash}")
}

/// Directory that `${game_assets}` points to (`AssetsUtils::getAssetsDir`).
///
/// Pre-1.6 asset indexes (`legacy`, `pre-1.6`) map onto the instance
/// `resources/` folder; everything else uses `assets/<id>`.
pub fn game_assets_dir(assets_root: &Path, index_id: &str, resources_dir: &Path) -> PathBuf {
    if index_id == "legacy" || index_id == "pre-1.6" {
        resources_dir.to_path_buf()
    } else {
        assets_root.join(index_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_objects_with_hash_and_size() {
        let index = AssetIndex::from_value(&json!({
            "objects": {
                "minecraft/sounds/random/click.ogg": {"hash": "9ea1b80ddb116f0355d5f9107ba8c6c4d20b44f5", "size": 3784},
                "pack.mcmeta": {"hash": "aa", "size": 1}
            }
        }))
        .unwrap();
        assert_eq!(index.objects.len(), 2);
        let click = index.objects.get("pack.mcmeta").unwrap();
        assert_eq!(click.hash, "aa");
        assert_eq!(click.size, 1);
    }

    #[test]
    fn missing_objects_or_hash_is_an_error() {
        assert!(AssetIndex::from_value(&json!({})).is_err());
        assert!(AssetIndex::from_value(&json!({"objects": {"a": {}}})).is_err());
        assert!(AssetIndex::parse("not json").is_err());
    }

    #[test]
    fn object_path_uses_two_char_prefix() {
        assert_eq!(
            object_relative_path("9ea1b80ddb116f0355d5f9107ba8c6c4d20b44f5"),
            "objects/9e/9ea1b80ddb116f0355d5f9107ba8c6c4d20b44f5"
        );
        assert_eq!(object_relative_path("a"), "objects//a"); // degenerate hash tolerated
    }

    /// The CDN path is written out as the literal Mojang's resource server
    /// serves, not re-derived through the storage helper. The bug this guards
    /// against was invisible precisely because every fixture built its expected
    /// URL the same way production built the real one, so both agreed on a path
    /// the server has never had.
    #[test]
    fn the_cdn_path_has_no_objects_segment() {
        let hash = "9ea1b80ddb116f0355d5f9107ba8c6c4d20b44f5";
        assert_eq!(object_cdn_path(hash), "9e/9ea1b80ddb116f0355d5f9107ba8c6c4d20b44f5");
        assert_eq!(object_cdn_path("a"), "/a"); // degenerate hash tolerated
        assert_eq!(
            object_relative_path(hash).strip_prefix("objects/").unwrap(),
            object_cdn_path(hash),
            "the two layouts differ by exactly the storage segment"
        );
        assert!(!object_cdn_path(hash).starts_with("objects/"));
    }

    #[test]
    fn legacy_indexes_map_to_resources_dir() {
        let assets = Path::new("/root/assets");
        let resources = Path::new("/inst/minecraft/resources");
        assert_eq!(game_assets_dir(assets, "legacy", resources), resources.to_path_buf());
        assert_eq!(game_assets_dir(assets, "pre-1.6", resources), resources.to_path_buf());
        assert_eq!(game_assets_dir(assets, "17", resources), assets.join("17"));
    }
}
