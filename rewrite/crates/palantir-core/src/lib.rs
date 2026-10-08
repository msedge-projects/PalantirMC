//! The Minecraft metadata formats, and how to turn them into a launch.
//!
//! This crate parses the documents the game's metadata service publishes --
//! the version manifest, the per-version metadata JSON, the asset index --
//! and derives what a launcher needs from them: which libraries the running
//! system gets, where every download lands, and the exact command line to
//! start the game.
//!
//! Everything here is written from the published formats themselves (and
//! tested against real downloaded samples in `tests/`), not from another
//! launcher's implementation of them. Formats and functionality are not
//! protected; implementations are.

// A launcher that panics is a crash with no message. Lint attributes apply
// in sequence and the last one wins, so the test allowance must come AFTER
// the deny: before it, the deny cancels the allowance and clippy rejects
// every `unwrap` in the unit tests too.
#![deny(clippy::expect_used, clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

pub mod assets;
pub mod error;
pub mod version;

pub use error::{Error, Result};
