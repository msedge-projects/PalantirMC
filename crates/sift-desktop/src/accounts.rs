//! Re-export of [`sift_game::accounts`]: the account store a launch signs in from.
//!
//! The store moved with the launch code because a Microsoft launch renews its
//! tokens from it; the shell's account pane reads the same file through this
//! re-export, so no path in `shell.rs` changed.
pub use sift_game::accounts::*;
