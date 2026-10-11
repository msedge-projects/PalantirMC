//! Recognising the archives a Bedrock player actually downloads.
//!
//! These extensions name *containers*, not contents. `.mcpack` is a zip that
//! holds either a resource pack or a behaviour pack, and only the
//! `manifest.json` inside says which; the extension alone cannot. Inventing
//! that split from the file name would file half the packs in the wrong folder,
//! so the classification stops where the name stops and the destination of a
//! pack is left to the archive's own manifest.

/// What a downloaded Bedrock archive says it is, as far as its name can say it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AddonKind {
    /// A single pack: resource or behaviour, decided by the manifest inside.
    Pack,
    /// Several packs in one archive.
    Addon,
    /// A world, importable as it stands.
    World,
    /// A skin pack.
    SkinPack,
    /// Not a Bedrock container at all.
    Unknown,
}

/// Classify by final extension, case-insensitively.
///
/// Trailing dots and spaces are trimmed first because that is how a name gets
/// past a check that compares only the tail of a string.
pub fn classify(name: &str) -> AddonKind {
    let lowered = name.trim().to_ascii_lowercase();
    let trimmed = lowered.trim_end_matches(['.', ' ']);
    if trimmed.ends_with(".mcworld") {
        AddonKind::World
    } else if trimmed.ends_with(".mcskin") {
        AddonKind::SkinPack
    } else if trimmed.ends_with(".mcaddon") {
        AddonKind::Addon
    } else if trimmed.ends_with(".mcpack") {
        AddonKind::Pack
    } else {
        AddonKind::Unknown
    }
}

/// The folder under `com.mojang` a kind belongs in, when the name decides it.
///
/// A pack's folder depends on the manifest inside it, and an `.mcaddon` may
/// carry several packs that do not share one, so those answer `None` rather
/// than guessing a destination the launcher would be writing into.
pub fn install_destination(kind: AddonKind) -> Option<&'static str> {
    match kind {
        AddonKind::World => Some("minecraftWorlds"),
        AddonKind::SkinPack => Some("skin_packs"),
        AddonKind::Pack | AddonKind::Addon | AddonKind::Unknown => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_container_extension_classifies() {
        assert_eq!(classify("clouds.mcpack"), AddonKind::Pack);
        assert_eq!(classify("island.mcaddon"), AddonKind::Addon);
        assert_eq!(classify("valley.mcworld"), AddonKind::World);
        assert_eq!(classify("heroes.mcskin"), AddonKind::SkinPack);
    }

    #[test]
    fn classification_ignores_case_and_trailing_decoration() {
        assert_eq!(classify("Clouds.MCPACK"), AddonKind::Pack);
        assert_eq!(classify("  valley.mcworld  "), AddonKind::World);
        assert_eq!(classify("valley.mcworld."), AddonKind::World);
        assert_eq!(classify("island.McAddon"), AddonKind::Addon);
    }

    #[test]
    fn a_zip_wrapping_a_world_is_not_a_world() {
        assert_eq!(classify("valley.mcworld.zip"), AddonKind::Unknown);
    }

    #[test]
    fn unrelated_names_are_unknown() {
        assert_eq!(classify("readme.txt"), AddonKind::Unknown);
        assert_eq!(classify(""), AddonKind::Unknown);
        assert_eq!(classify("mcworld"), AddonKind::Unknown);
    }

    #[test]
    fn destinations_are_named_only_where_the_name_decides_them() {
        assert_eq!(
            install_destination(AddonKind::World),
            Some("minecraftWorlds")
        );
        assert_eq!(install_destination(AddonKind::SkinPack), Some("skin_packs"));
        assert_eq!(install_destination(AddonKind::Pack), None);
        assert_eq!(install_destination(AddonKind::Addon), None);
        assert_eq!(install_destination(AddonKind::Unknown), None);
    }
}
