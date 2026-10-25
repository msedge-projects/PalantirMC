//! Re-export of [`sift_game::instances`]: instance discovery, summaries and
//! creation.
//!
//! Moved with the engine because the metadata and launch code resolve one
//! instance's pack and because its tests build real instance folders. The
//! library, the Create dialog and the store read it through this re-export.
pub use sift_game::instances::*;
