//! Embedded instance + toolbar icons (zero runtime IO).
//!
//! Every PNG below is `include_bytes!`'d, so the GUI never touches the disk
//! to paint an icon. See `assets/ATTRIBUTION`: the PNGs were carved from the
//! Prism Launcher binary (`prismlauncher.exe`), (c) Prism Launcher
//! contributors, GPL-3.0-only (this workspace is already GPL-3.0-only).
//!
//! # Instance-icon table ([`instance_icon`])
//!
//! Resolution order for an `iconKey`:
//!
//! 1. Exact match against the canonical set: `chicken_legacy`,
//!    `enderman_legacy`, `enderpearl_legacy`, `default`, `grass`, `dirt`,
//!    `steve`, `creeper`, `tnt`, `gear`. (`default` and `grass` are
//!    byte-identical: Prism's default icon IS the grass block.)
//! 2. Strip a single trailing `"_legacy"` and retry the canonical set, so
//!    e.g. `grass_legacy` hits `grass` (only one suffix is stripped:
//!    `grass_legacy_legacy` does NOT resolve to grass).
//! 3. Alias table for other common Prism keys, mapped to the nearest
//!    available art:
//!    * bare mob names that name an available icon without its suffix —
//!      `chicken` -> `chicken_legacy`, `enderman` -> `enderman_legacy`,
//!      `enderpearl` -> `enderpearl_legacy`, `creeper` -> `creeper`
//!      (applies after step 2, so `diamond_legacy` also reaches the
//!      mineral aliases via its `diamond` base);
//!    * ores/gems/minerals — `diamond`, `gold`, `iron`, `coal`,
//!      `redstone`, `emerald`, `lapis`, `quartz`, `copper`, `netherite`,
//!      `amethyst`, `glowstone` — -> `dirt` (closest earthy/mineral look
//!      among the carved art);
//! 4. Anything else (unknown or empty) -> `default` grass bytes. There is
//!    no letter-tile fallback anymore.
//!
//! # Toolbar-icon table ([`ui_icon`])
//!
//! Exact match over `play`, `kill`, `minus`, `check`, `help`, `about`,
//! `folder`, `update`, `refresh`, `gear`; unknown/empty -> `help` (the blue
//! `?` is the natural generic toolbar glyph).
//!
//! # Handle caching (why the grid does not re-decode PNGs every frame)
//!
//! Verified against the iced 0.12 sources in the local cargo registry:
//!
//! * `iced_core::image::Handle::from_memory` hashes the byte content into
//!   `Handle::id`, so equal bytes always yield equal ids.
//! * The tiny-skia backend (`iced_tiny_skia::raster::Pipeline`) keeps a
//!   `Cache: FxHashMap<u64, Option<Entry>>` keyed by that id: the PNG is
//!   decoded (via `graphics::image::load`) only on first sight of an id,
//!   and `trim_cache` retains every id used since the last trim — our icons
//!   are drawn every frame, so they stay cached. Building a fresh handle
//!   per `view()` call would therefore already avoid re-decoding.
//! * Per-frame `from_memory` would still re-hash the PNG bytes on every
//!   frame just to recompute the id, so [`instance_handle`]/[`ui_handle`]
//!   go one step further: each canonical icon's `Handle` is built exactly
//!   once behind a `OnceLock` map and then cheaply cloned (`Handle` clone
//!   is an `Arc` bump; the `Bytes` payload is the `&'static` embedded
//!   slice, never copied). The grid does no hashing, no decoding and no IO
//!   per frame.

use std::collections::HashMap;
use std::sync::OnceLock;

use iced::widget::image::Handle;

// ---- Embedded bytes (one `include_bytes!` per canonical file). ----

const CHICKEN_LEGACY_BYTES: &[u8] = include_bytes!("../assets/icons/chicken_legacy.png");
const ENDERMAN_LEGACY_BYTES: &[u8] = include_bytes!("../assets/icons/enderman_legacy.png");
const ENDERPEARL_LEGACY_BYTES: &[u8] = include_bytes!("../assets/icons/enderpearl_legacy.png");
const DEFAULT_BYTES: &[u8] = include_bytes!("../assets/icons/default.png");
const GRASS_BYTES: &[u8] = include_bytes!("../assets/icons/grass.png");
const DIRT_BYTES: &[u8] = include_bytes!("../assets/icons/dirt.png");
const STEVE_BYTES: &[u8] = include_bytes!("../assets/icons/steve.png");
const CREEPER_BYTES: &[u8] = include_bytes!("../assets/icons/creeper.png");
const TNT_BYTES: &[u8] = include_bytes!("../assets/icons/tnt.png");
const GEAR_BYTES: &[u8] = include_bytes!("../assets/icons/gear.png");

const PLAY_BYTES: &[u8] = include_bytes!("../assets/icons/play.png");
const KILL_BYTES: &[u8] = include_bytes!("../assets/icons/kill.png");
const MINUS_BYTES: &[u8] = include_bytes!("../assets/icons/minus.png");
const CHECK_BYTES: &[u8] = include_bytes!("../assets/icons/check.png");
const HELP_BYTES: &[u8] = include_bytes!("../assets/icons/help.png");
const ABOUT_BYTES: &[u8] = include_bytes!("../assets/icons/about.png");
const FOLDER_BYTES: &[u8] = include_bytes!("../assets/icons/folder.png");
const UPDATE_BYTES: &[u8] = include_bytes!("../assets/icons/update.png");
const REFRESH_BYTES: &[u8] = include_bytes!("../assets/icons/refresh.png");

/// Canonical instance-icon names (file stems under `assets/icons/`).
pub const INSTANCE_CANONICAL: [&str; 10] = [
    "chicken_legacy",
    "enderman_legacy",
    "enderpearl_legacy",
    "default",
    "grass",
    "dirt",
    "steve",
    "creeper",
    "tnt",
    "gear",
];

/// Canonical toolbar-icon names (file stems under `assets/icons/`).
pub const UI_CANONICAL: [&str; 10] =
    ["play", "kill", "minus", "check", "help", "about", "folder", "update", "refresh", "gear"];

/// Bytes for a canonical instance-icon name (always known-good input).
fn instance_bytes(canonical: &str) -> &'static [u8] {
    match canonical {
        "chicken_legacy" => CHICKEN_LEGACY_BYTES,
        "enderman_legacy" => ENDERMAN_LEGACY_BYTES,
        "enderpearl_legacy" => ENDERPEARL_LEGACY_BYTES,
        "grass" => GRASS_BYTES,
        "dirt" => DIRT_BYTES,
        "steve" => STEVE_BYTES,
        "creeper" => CREEPER_BYTES,
        "tnt" => TNT_BYTES,
        "gear" => GEAR_BYTES,
        // "default" and anything unexpected: Prism's default grass block.
        _ => DEFAULT_BYTES,
    }
}

/// Bytes for a canonical toolbar-icon name (always known-good input).
fn ui_bytes(canonical: &str) -> &'static [u8] {
    match canonical {
        "play" => PLAY_BYTES,
        "kill" => KILL_BYTES,
        "minus" => MINUS_BYTES,
        "check" => CHECK_BYTES,
        "about" => ABOUT_BYTES,
        "folder" => FOLDER_BYTES,
        "update" => UPDATE_BYTES,
        "refresh" => REFRESH_BYTES,
        "gear" => GEAR_BYTES,
        // "help" and anything unexpected: the blue `?`.
        _ => HELP_BYTES,
    }
}

/// Resolve an `iconKey` to a canonical instance-icon name.
fn resolve_instance(key: &str) -> &'static str {
    // 1. Exact canonical match.
    if INSTANCE_CANONICAL.contains(&key) {
        return canonical_or_default(key);
    }
    // 2./3. Strip one trailing "_legacy", then retry canonical + aliases.
    let base = key.strip_suffix("_legacy").unwrap_or(key);
    if base != key && INSTANCE_CANONICAL.contains(&base) {
        return canonical_or_default(base);
    }
    if let Some(canonical) = alias_name(base) {
        return canonical;
    }
    "default"
}

/// Identity map for known canonical names, `"default"` otherwise.
/// Keeps [`resolve_instance`] total without panicking paths.
fn canonical_or_default(name: &str) -> &'static str {
    match name {
        "chicken_legacy" => "chicken_legacy",
        "enderman_legacy" => "enderman_legacy",
        "enderpearl_legacy" => "enderpearl_legacy",
        "grass" => "grass",
        "dirt" => "dirt",
        "steve" => "steve",
        "creeper" => "creeper",
        "tnt" => "tnt",
        "gear" => "gear",
        _ => "default",
    }
}

/// Canonical name for an aliasable base key, if it has a near match.
fn alias_name(base: &str) -> Option<&'static str> {
    match base {
        "chicken" => Some("chicken_legacy"),
        "enderman" => Some("enderman_legacy"),
        "enderpearl" => Some("enderpearl_legacy"),
        "creeper" => Some("creeper"),
        "diamond" | "gold" | "iron" | "coal" | "redstone" | "emerald" | "lapis" | "quartz"
        | "copper" | "netherite" | "amethyst" | "glowstone" => Some("dirt"),
        _ => None,
    }
}

/// Embedded PNG bytes for an instance `iconKey` (never empty, always a
/// valid PNG; unknown/empty keys yield the default grass block).
pub fn instance_icon(key: &str) -> &'static [u8] {
    instance_bytes(resolve_instance(key))
}

/// Embedded PNG bytes for a toolbar icon name (unknown/empty yields the
/// blue `?` help glyph).
pub fn ui_icon(name: &str) -> &'static [u8] {
    if UI_CANONICAL.contains(&name) {
        ui_bytes(name)
    } else {
        HELP_BYTES
    }
}

/// Build the once-per-process handle cache for the given canonical set.
fn build_handles(names: &[&'static str], bytes: fn(&str) -> &'static [u8]) -> HashMap<&'static str, Handle> {
    let mut map = HashMap::with_capacity(names.len());
    for name in names {
        map.insert(*name, Handle::from_memory(bytes(name)));
    }
    map
}

/// Cached image handles for the canonical instance icons.
fn instance_handles() -> &'static HashMap<&'static str, Handle> {
    static CACHE: OnceLock<HashMap<&'static str, Handle>> = OnceLock::new();
    // Built through the public byte table, so every canonical name resolves
    // to exactly the bytes [`instance_icon`] reports.
    CACHE.get_or_init(|| build_handles(&INSTANCE_CANONICAL, instance_icon))
}

/// Cached image handles for the canonical toolbar icons.
fn ui_handles() -> &'static HashMap<&'static str, Handle> {
    static CACHE: OnceLock<HashMap<&'static str, Handle>> = OnceLock::new();
    CACHE.get_or_init(|| build_handles(&UI_CANONICAL, ui_icon))
}

/// Cached [`Handle`] for an instance `iconKey`: built once per canonical
/// icon and cloned afterwards, so grid frames do no hashing, decoding or
/// IO (see the module docs for the renderer-side caching analysis).
pub fn instance_handle(key: &str) -> Handle {
    let canonical = resolve_instance(key);
    match instance_handles().get(canonical) {
        Some(handle) => handle.clone(),
        None => Handle::from_memory(DEFAULT_BYTES),
    }
}

/// Cached [`Handle`] for a toolbar icon name (unknown/empty -> help `?`).
pub fn ui_handle(name: &str) -> Handle {
    let canonical = if UI_CANONICAL.contains(&name) { name } else { "help" };
    match ui_handles().get(canonical) {
        Some(handle) => handle.clone(),
        None => Handle::from_memory(HELP_BYTES),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// PNG magic every embedded file must start with.
    const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

    fn assert_png(bytes: &[u8]) {
        assert!(!bytes.is_empty(), "embedded icon must not be empty");
        assert!(
            bytes.len() >= PNG_MAGIC.len() && bytes[..PNG_MAGIC.len()] == PNG_MAGIC,
            "embedded icon must start with PNG magic"
        );
    }

    #[test]
    fn exact_keys_map_to_their_own_art() {
        assert_png(instance_icon("chicken_legacy"));
        assert_png(instance_icon("enderman_legacy"));
        assert_png(instance_icon("enderpearl_legacy"));
        assert_png(instance_icon("default"));
        assert_eq!(instance_icon("chicken_legacy"), CHICKEN_LEGACY_BYTES);
        assert_eq!(instance_icon("enderman_legacy"), ENDERMAN_LEGACY_BYTES);
        assert_eq!(instance_icon("enderpearl_legacy"), ENDERPEARL_LEGACY_BYTES);
        assert_eq!(instance_icon("default"), DEFAULT_BYTES);
        assert_eq!(instance_icon("grass"), GRASS_BYTES);
        assert_eq!(instance_icon("dirt"), DIRT_BYTES);
        assert_eq!(instance_icon("steve"), STEVE_BYTES);
        assert_eq!(instance_icon("creeper"), CREEPER_BYTES);
        assert_eq!(instance_icon("tnt"), TNT_BYTES);
        assert_eq!(instance_icon("gear"), GEAR_BYTES);
        // The three mob heads and the default block are distinct files.
        assert_ne!(instance_icon("chicken_legacy"), instance_icon("enderman_legacy"));
        assert_ne!(instance_icon("chicken_legacy"), instance_icon("enderpearl_legacy"));
        assert_ne!(instance_icon("enderman_legacy"), instance_icon("enderpearl_legacy"));
        assert_ne!(instance_icon("chicken_legacy"), instance_icon("default"));
    }

    #[test]
    fn default_and_grass_are_the_same_block() {
        // Prism's default icon IS the grass block (byte-identical copies).
        assert_eq!(instance_icon("default"), instance_icon("grass"));
        assert_eq!(DEFAULT_BYTES, GRASS_BYTES);
    }

    #[test]
    fn single_legacy_suffix_is_stripped_once() {
        assert_eq!(instance_icon("grass_legacy"), GRASS_BYTES);
        assert_eq!(instance_icon("dirt_legacy"), DIRT_BYTES);
        assert_eq!(instance_icon("steve_legacy"), STEVE_BYTES);
        assert_eq!(instance_icon("creeper_legacy"), CREEPER_BYTES);
        assert_eq!(instance_icon("tnt_legacy"), TNT_BYTES);
        assert_eq!(instance_icon("gear_legacy"), GEAR_BYTES);
        assert_eq!(instance_icon("default_legacy"), DEFAULT_BYTES);
        // Only ONE suffix is stripped: this still ends in "_legacy" after
        // stripping, which matches nothing, so it falls back to grass.
        assert_eq!(instance_icon("grass_legacy_legacy"), DEFAULT_BYTES);
    }

    #[test]
    fn common_prism_keys_map_to_nearest_art() {
        // Bare mob names hit their legacy art.
        assert_eq!(instance_icon("chicken"), CHICKEN_LEGACY_BYTES);
        assert_eq!(instance_icon("enderman"), ENDERMAN_LEGACY_BYTES);
        assert_eq!(instance_icon("enderpearl"), ENDERPEARL_LEGACY_BYTES);
        assert_eq!(instance_icon("creeper"), CREEPER_BYTES);
        // Ores/gems/minerals hit dirt (nearest carved art).
        for key in [
            "diamond", "gold", "iron", "coal", "redstone", "emerald", "lapis", "quartz", "copper",
            "netherite", "amethyst", "glowstone",
        ] {
            assert_eq!(instance_icon(key), DIRT_BYTES, "ore key '{key}'");
        }
        // Stripping composes with aliases: diamond_legacy -> diamond -> dirt.
        assert_eq!(instance_icon("diamond_legacy"), DIRT_BYTES);
        assert_eq!(instance_icon("gold_legacy"), DIRT_BYTES);
    }

    #[test]
    fn unknown_and_empty_keys_fall_back_to_grass() {
        for key in ["", "   ", "not-a-real-icon", "bedrock", "zombie", "stone", "_legacy"] {
            assert_eq!(instance_icon(key), DEFAULT_BYTES, "key '{key}'");
        }
        assert_png(instance_icon(""));
        assert_png(instance_icon("mystery"));
    }

    #[test]
    fn every_canonical_instance_icon_is_a_valid_png() {
        for name in INSTANCE_CANONICAL {
            assert_png(instance_bytes(name));
            assert_png(instance_icon(name));
        }
    }

    #[test]
    fn toolbar_table_covers_all_names_with_valid_pngs() {
        for name in UI_CANONICAL {
            assert_png(ui_bytes(name));
            assert_png(ui_icon(name));
        }
        assert_eq!(ui_icon("play"), PLAY_BYTES);
        assert_eq!(ui_icon("kill"), KILL_BYTES);
        assert_eq!(ui_icon("minus"), MINUS_BYTES);
        assert_eq!(ui_icon("check"), CHECK_BYTES);
        assert_eq!(ui_icon("help"), HELP_BYTES);
        assert_eq!(ui_icon("about"), ABOUT_BYTES);
        assert_eq!(ui_icon("folder"), FOLDER_BYTES);
        assert_eq!(ui_icon("update"), UPDATE_BYTES);
        assert_eq!(ui_icon("refresh"), REFRESH_BYTES);
        assert_eq!(ui_icon("gear"), GEAR_BYTES);
        // Unknown/empty toolbar names fall back to the blue `?`.
        assert_eq!(ui_icon(""), HELP_BYTES);
        assert_eq!(ui_icon("nope"), HELP_BYTES);
    }

    #[test]
    fn handles_are_stable_and_alias_shared_bytes() {
        // Same key -> same content-hash id, without touching a renderer.
        assert_eq!(instance_handle("chicken_legacy").id(), instance_handle("chicken_legacy").id());
        assert_eq!(ui_handle("play").id(), ui_handle("play").id());
        // Aliases share the target's bytes, hence its id.
        assert_eq!(instance_handle("grass_legacy").id(), instance_handle("grass").id());
        assert_eq!(instance_handle("diamond").id(), instance_handle("dirt").id());
        assert_eq!(instance_handle("bogus").id(), instance_handle("default").id());
        assert_eq!(ui_handle("bogus").id(), ui_handle("help").id());
    }
}
