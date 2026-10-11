//! Minecraft: Bedrock Edition, as far as a launcher may touch it.
//!
//! Bedrock is a packaged Windows app, not a program this launcher owns. What we
//! may do is therefore deliberately small: find the copy Microsoft installed,
//! launch it, and manage that install's content and worlds. We do not install
//! it, do not register versions, and do not stand in for its licence or sign-in
//! checks -- Windows and the game own those, and a launcher that produced them
//! itself would be shipping a bypass rather than a feature.
//!
//! Everything platform-independent lives in [`addons`], [`install`] and
//! [`paths`], which take their inputs as parameters and so compile and test on
//! any host. [`windows`] is the only module that talks to Windows, and it is
//! gated so the rest of the crate stays checkable off Windows.

// A launcher that panics is a crash with no message for the player, so `unwrap`
// and `expect` are denied crate-wide. Lint attributes apply in sequence and the
// last one wins, so the test allowance has to come AFTER the deny: before it,
// the deny would cancel the allowance and clippy would reject `unwrap` in the
// unit tests too.
#![deny(clippy::expect_used, clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

pub mod addons;
pub mod install;
pub mod paths;
pub mod windows;

pub use install::BedrockInstall;
pub use paths::Channel;
