//! Where Bedrock's package identities and its player files live.
//!
//! Bedrock moved its user data with 1.21.120, when the Windows client changed
//! from a UWP package to a GDK one. A launcher that knows a single location
//! reports "no worlds" on every machine that has not migrated, so both layouts
//! are derived here and the caller asks which one exists instead of assuming.

use std::fmt;
use std::path::{Path, PathBuf};

/// Which Microsoft Store channel an installed Bedrock package belongs to.
///
/// The channels are separate packages rather than two versions of one package,
/// so a machine can hold both at once and a launch has to name which one it
/// means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Channel {
    Release,
    Preview,
}

impl Channel {
    /// The package-identity prefix Microsoft ships this channel under.
    ///
    /// The preview client has an identity of its own, so the identity -- not a
    /// version string and not a licence flag -- is what tells the channels
    /// apart.
    pub fn package_prefix(self) -> &'static str {
        match self {
            Self::Release => "Microsoft.MinecraftUWP",
            Self::Preview => "Microsoft.MinecraftWindowsBeta",
        }
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Release => "Release",
            Self::Preview => "Preview",
        })
    }
}

/// The channel a package identity names, or `None` for anything else.
///
/// The match is a prefix because a full package family name carries a
/// publisher hash (`Microsoft.MinecraftUWP_8wekyb3d8bbwe`) that is not part of
/// the identity itself.
pub fn classify_package(name: &str) -> Option<Channel> {
    let lowered = name.trim().to_ascii_lowercase();
    [Channel::Release, Channel::Preview]
        .into_iter()
        .find(|channel| {
            let prefix = channel.package_prefix().to_ascii_lowercase();
            lowered.starts_with(&prefix)
        })
}

/// The two places Bedrock has kept a player's own files.
///
/// They are not interchangeable: the UWP package held them inside its own
/// package folder under the local profile, and the GDK client keeps them in a
/// shared folder under the roaming profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataLocations {
    pub gdk: PathBuf,
    pub uwp: PathBuf,
}

/// Both layouts, derived from the two profile roots the caller supplies.
///
/// The roots are parameters rather than reads of the process environment, so
/// the derivation stays a pure function that can be tested on any host.
pub fn data_locations(appdata: &Path, localappdata: &Path) -> DataLocations {
    DataLocations {
        gdk: appdata
            .join("Minecraft Bedrock")
            .join("users")
            .join("shared")
            .join("games")
            .join("com.mojang"),
        uwp: localappdata
            .join("Packages")
            .join("Microsoft.MinecraftUWP_8wekyb3d8bbwe")
            .join("LocalState")
            .join("games")
            .join("com.mojang"),
    }
}

/// The layout that actually exists, preferring the current one.
///
/// A machine that migrated keeps an empty or stale legacy folder, so GDK wins
/// when both are present. `None` means Bedrock has never run here, which is a
/// state to report rather than an error to raise.
pub fn existing_data_root(locations: &DataLocations) -> Option<PathBuf> {
    if locations.gdk.is_dir() {
        return Some(locations.gdk.clone());
    }
    if locations.uwp.is_dir() {
        return Some(locations.uwp.clone());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let name = format!("palantirmc-bedrock-{tag}-{}", std::process::id());
        let dir = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn both_data_locations_hang_off_the_roots_given() {
        let appdata = temp_root("gdk-root");
        let localappdata = temp_root("uwp-root");
        let locations = data_locations(&appdata, &localappdata);
        let expected_gdk = appdata
            .join("Minecraft Bedrock")
            .join("users")
            .join("shared")
            .join("games")
            .join("com.mojang");
        let expected_uwp = localappdata
            .join("Packages")
            .join("Microsoft.MinecraftUWP_8wekyb3d8bbwe")
            .join("LocalState")
            .join("games")
            .join("com.mojang");
        assert_eq!(locations.gdk, expected_gdk);
        assert_eq!(locations.uwp, expected_uwp);
    }

    #[test]
    fn existing_data_root_prefers_the_current_layout() {
        let appdata = temp_root("prefer-gdk");
        let localappdata = temp_root("prefer-uwp");
        let locations = data_locations(&appdata, &localappdata);
        std::fs::create_dir_all(&locations.gdk).unwrap();
        std::fs::create_dir_all(&locations.uwp).unwrap();
        assert_eq!(existing_data_root(&locations), Some(locations.gdk.clone()));
    }

    #[test]
    fn existing_data_root_falls_back_to_the_legacy_layout() {
        let appdata = temp_root("legacy-gdk");
        let localappdata = temp_root("legacy-uwp");
        let locations = data_locations(&appdata, &localappdata);
        std::fs::create_dir_all(&locations.uwp).unwrap();
        assert_eq!(existing_data_root(&locations), Some(locations.uwp.clone()));
    }

    #[test]
    fn existing_data_root_is_none_before_bedrock_has_run() {
        let appdata = temp_root("fresh-gdk");
        let localappdata = temp_root("fresh-uwp");
        let locations = data_locations(&appdata, &localappdata);
        assert_eq!(existing_data_root(&locations), None);
    }

    #[test]
    fn package_identities_classify_to_their_channel() {
        let release = "Microsoft.MinecraftUWP_8wekyb3d8bbwe";
        let preview = "Microsoft.MinecraftWindowsBeta_8wekyb3d8bbwe";
        assert_eq!(classify_package(release), Some(Channel::Release));
        assert_eq!(classify_package(preview), Some(Channel::Preview));
        let mixed_case = classify_package("microsoft.minecraftuwp");
        assert_eq!(mixed_case, Some(Channel::Release));
        assert_eq!(classify_package("Microsoft.WindowsCalculator"), None);
        assert_eq!(classify_package(""), None);
    }

    #[test]
    fn channel_prefixes_and_labels_are_the_documented_identities() {
        assert_eq!(Channel::Release.package_prefix(), "Microsoft.MinecraftUWP");
        let preview_prefix = Channel::Preview.package_prefix();
        assert_eq!(preview_prefix, "Microsoft.MinecraftWindowsBeta");
        assert_eq!(Channel::Release.to_string(), "Release");
        assert_eq!(Channel::Preview.to_string(), "Preview");
    }
}
