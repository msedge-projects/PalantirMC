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
                .to_ascii_lowercase();
            // Both fields become filesystem paths during legacy reconstruction.
            // Refuse malformed indexes instead of skipping missing objects or
            // writing an asset outside its designated directory.
            if hash.len() != 40 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(Error::json("<asset index>", format!("invalid SHA-1 for '{name}'")));
            }
            if name.is_empty() || name.starts_with('/') || name.contains(['\u{005c}', ':'])
                || name.split('/').any(|part| part.is_empty() || part == "." || part == "..")
                || name.chars().any(char::is_control)
            {
                return Err(Error::json("<asset index>", format!("unsafe asset name '{name}'")));
            }
            let size = obj.get("size").and_then(|v| v.as_i64()).unwrap_or(0);
            if size < 0 {
                return Err(Error::json("<asset index>", format!("negative size for '{name}'")));
            }
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
                "pack.mcmeta": {"hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "size": 1}
            }
        }))
        .unwrap();
        assert_eq!(index.objects.len(), 2);
        let click = index.objects.get("pack.mcmeta").unwrap();
        assert_eq!(click.hash, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert_eq!(click.size, 1);
    }

    #[test]
    fn missing_objects_or_hash_is_an_error() {
        assert!(AssetIndex::from_value(&json!({})).is_err());
        assert!(AssetIndex::from_value(&json!({"objects": {"a": {}}})).is_err());
        assert!(AssetIndex::parse("not json").is_err());
    }

    #[test]
    fn malformed_indexes_cannot_escape_asset_directories() {
        let hash = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        for name in ["../outside", "/absolute", "nested/../outside", "C:\\outside", "a\\b", "", "a//b"] {
            let mut objects = serde_json::Map::new();
            objects.insert(name.into(), json!({ "hash": hash, "size": 1 }));
            assert!(AssetIndex::from_value(&json!({ "objects": objects })).is_err(), "{name}");
        }
        for bad_hash in ["", "a", "../outside", "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz"] {
            assert!(AssetIndex::from_value(&json!({ "objects": { "safe": { "hash": bad_hash } } })).is_err());
        }
        assert!(AssetIndex::from_value(&json!({ "objects": { "safe": { "hash": hash, "size": -1 } } })).is_err());
        let index = AssetIndex::from_value(&json!({ "objects": { "safe": { "hash": hash.to_uppercase() } } })).unwrap();
        assert_eq!(index.objects["safe"].hash, hash);
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
