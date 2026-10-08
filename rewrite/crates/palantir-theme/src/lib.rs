//! The design tokens of PalantirMC 2.0.
//!
//! One crate, one file: see [`theme`]. It exists separately from the shell so
//! the tokens are compile-checked and testable before any window exists, and so
//! every later crate draws from one source of truth.

// A GUI that panics is a crash with no message, so `unwrap` and `expect` are
// denied crate-wide. The allowance below must come first: inner attributes
// apply in sequence, and the later `deny` would otherwise cancel it.
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]
#![deny(clippy::expect_used, clippy::unwrap_used)]

pub mod theme;

pub use theme::*;
