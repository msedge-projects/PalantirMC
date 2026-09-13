//! # prism-gui — shared view-model for the desktop shell
//!
//! The data the iced frontend reads, in a crate that takes on no GUI
//! dependency of its own: `prism-desktop`'s shell and the CLI both go through
//! these models.
//!
//! * [`model::InstanceListModel`] — sorted/filterable instance rows loaded
//!   from [`prism_core::paths::PrismPaths`] via [`prism_core::instance::Instance`]
//!   discovery plus [`prism_core::instance::groups::Groups`].
//! * [`model::SettingsModel`] — typed settings with Prism's override-gate
//!   semantics (global value vs instance override flag).
//!
//! There is deliberately no rendering seam here. A `GuiBackend` trait and an
//! `App` driver were written here before the frontend was chosen; iced won, and
//! the shell drives these models directly, so the seam was removed rather than
//! kept as a second, unreachable path.

// Permitted in tests only: the crate's own rule is about the models, and the
// tests assert by unwrapping. See `prism-core/src/lib.rs` for the long version.
//
// Order matters and is not cosmetic: lint attributes are applied in sequence,
// so this `allow` has to come *after* the `deny` below or the deny wins in the
// test build. It was written the other way round first and the lint job caught
// it -- prism-core, with the correct order, went green in the same run.
#![deny(clippy::unwrap_used, clippy::expect_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
#![warn(missing_docs)]

mod model;

pub use model::{InstanceEntry, InstanceListModel, SettingsModel};

/// View-model errors.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A `prism-core` operation failed.
    #[error("core error: {0}")]
    Core(#[from] prism_core::Error),
    /// An instance-bound operation was requested without a bound instance.
    #[error("no instance bound to the settings model")]
    NoInstanceBound,
}
