//! Re-export of [`sift_game::meta`]: the metadata a launch resolves through.
//!
//! [`sift_game::meta::PublisherMeta`] moved into the game crate because a launch
//! resolves against it; the store's repair path still builds one here, through
//! this re-export, and passes it back into the engine as a
//! `sift_core::resolve::MetaStore`.
pub use sift_game::meta::*;
