//! # prism-gui — backend-agnostic view-model (phase 4)
//!
//! This crate owns the data a future `iced`/`egui` frontend will paint and
//! deliberately takes on zero heavy GUI dependencies: a rendering backend
//! plugs in later via the [`GuiBackend`] trait.
//!
//! * [`model::InstanceListModel`] — sorted/filterable instance rows loaded
//!   from [`prism_core::paths::PrismPaths`] via [`prism_core::instance::Instance`]
//!   discovery plus [`prism_core::instance::groups::Groups`].
//! * [`model::SettingsModel`] — typed settings with Prism's override-gate
//!   semantics (global value vs instance override flag).
//! * [`model::Page`] / [`model::Route`] — sidebar pages and navigation state.
//! * [`backend::App`] — owns the models and drives one [`GuiBackend`] via
//!   [`backend::App::tick`]; [`backend::HeadlessBackend`] records renders
//!   for tests so no window is ever needed.

#![deny(clippy::unwrap_used, clippy::expect_used)]
#![warn(missing_docs)]

mod backend;
mod model;

pub use backend::{App, AppSnapshot, GuiBackend, GuiEvent, HeadlessBackend};
pub use model::{InstanceEntry, InstanceListModel, Page, Route, SettingsModel};

/// View-model errors.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A `prism-core` operation failed.
    #[error("core error: {0}")]
    Core(#[from] prism_core::Error),
    /// Snapshot JSON serialization failed.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    /// An instance-bound operation was requested without a bound instance.
    #[error("no instance bound to the settings model")]
    NoInstanceBound,
}
