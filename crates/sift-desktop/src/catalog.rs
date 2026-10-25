//! Re-export of [`sift_game::catalog`]: Minecraft releases and loader builds.
//!
//! Moved with the engine because the Java search asks the same metadata service
//! for a runtime list. The Create Instance dialog and the store reach it through
//! this re-export, so their paths are unchanged.
pub use sift_game::catalog::*;
