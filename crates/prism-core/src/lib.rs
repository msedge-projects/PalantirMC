//! # prism-core
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
//!   (offline disk cache now, online fetch in `prism-net`).
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
