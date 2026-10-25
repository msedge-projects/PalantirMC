//! Re-export of [`sift_game::install`]: the install half of a launch.
//!
//! The transfer engine moved into `sift-game` so the egui shell can install a
//! version too. This module keeps `crate::install::…` working for the shell's
//! pages and its store; the implementation, its documentation and every test are
//! the ones that used to live here -- nothing is reimplemented.
//!
//! One thing this crate still owns: the machine-awake guard that used to be
//! inside `install::run`. The engine is safe Rust, so the callers that own a
//! thread (`shell.rs`'s launch worker and `Store::repair_instance`) hold it
//! instead. See `awake.rs`.
pub use sift_game::install::*;
