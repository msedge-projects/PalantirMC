//! # palantir-core
//!
//! Prism Launcher-compatible backend core. This crate mirrors the on-disk
//! formats of [Prism Launcher](https://github.com/PrismLauncher/PrismLauncher)
//! so instances created by one launcher can be opened by the other:
//!
//! * [`ini`] — `instance.cfg` / `prismlauncher.cfg` codec (QSettings-IniFormat
//!   emulation, including the legacy pre-`ConfigVersion` format).
//! * [`settings`] — typed settings view over an INI file with Prism defaults.
//! * [`paths`] — data-root discovery and the standard folder layout.
//! * [`instance`] — instance discovery, metadata and folder scaffolding.
//! * [`pack`] — `mmc-pack.json` component profiles (`PackProfile`).
//! * [`version`] — version-file / library / rule model and the merge engine
//!   that turns a component list into a [`version::LaunchProfile`].
//! * [`resolve`] — component resolution over a pluggable [`resolve::MetaStore`]
//!   (offline disk cache now, online fetch in `palantir-net`).
//! * [`launch`] — pure launch-plan builders (JVM args, Minecraft args, env,
//!   and the Prism launch-script line protocol).
//! * [`assets`] / [`java`] — asset-index paths and Java version handling.
//!
//! Format facts were verified against the Prism `develop` sources; each module
//! doc lists the C++ files it mirrors. Deviations are documented per item.
//!
//! Quality policy: no `unsafe`, no `unwrap`/`expect` outside tests, all
//! fallible operations return [`Error`] with path context.

#![deny(clippy::unwrap_used, clippy::expect_used)]
// The policy above is "no `unwrap`/`expect` outside tests", so the tests have
// to be exempt or the crate cannot compile under clippy at all. `palantir-loader`
// already carries this line; the crates without it were failing 153 and 16
// lints respectively the first time CI ran clippy.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
#![warn(missing_docs)]

pub mod assets;
pub mod error;
pub mod ini;
pub mod json;
pub mod instance;
pub mod java;
pub mod launch;
pub mod pack;
pub mod paths;
pub mod resolve;
pub mod settings;
pub mod util;
pub mod version;

pub use error::{Error, Result};

/// The product name, in one place.
///
/// It travels into the game: the launch script's `windowTitle` and
/// `launcherBrand` lines carry it, so Minecraft's own title bar and the crash
/// report say which launcher started the game. A name that is spelled out at
/// each call site is a name that eventually says two different things — which
/// is exactly what happened here, with a hardcoded second launcher's name in
/// the script's title line.
pub const PRODUCT_NAME: &str = "PalantirMC";
