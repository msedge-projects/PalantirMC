//! The product identity the engine has to carry.
//!
//! A launch script names the launcher that started the game in its
//! `windowTitle`/`launcherBrand` lines, and the per-instance run log opens with
//! the same name and version. Those two facts are the engine's business because
//! the engine writes them; everything else about the brand -- the artwork, the
//! window icon, the About page -- stays in `sift-desktop/src/brand.rs`, which
//! re-exports these two items.
//!
//! [`APP_NAME`] is [`sift_core::PRODUCT_NAME`] rather than a second literal: the
//! name travels into the game, and two spellings of one product name is how a
//! window ends up labelled with a different launcher. [`version`] is this
//! workspace's version, which every member inherits from `[workspace.package]`,
//! so it is the same string the exe and the About page carry.

/// Product name shown in the title bar, the hero card and the About page, and
/// written into the launch script and the run log.
pub const APP_NAME: &str = sift_core::PRODUCT_NAME;

/// Version reported by the About page and written at the top of a run log.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
