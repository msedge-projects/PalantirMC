//! Rendering-backend seam: [`GuiBackend`], the [`App`] driver and a
//! headless backend for tests.
//!
//! A real window (a future `iced`/`egui` crate) implements [`GuiBackend`]
//! and pumps [`App::tick`]; tests use [`HeadlessBackend`], which records
//! rendered snapshots and replays scripted events without opening a window.

use crate::model::{InstanceEntry, InstanceListModel, Page, Route, SettingsModel};
use prism_core::{paths::PrismPaths, settings::Settings};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// One frame of GUI state: everything a backend needs to paint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSnapshot {
    /// Current route (page + instance context).
    pub route: Route,
    /// Active instance-list filter text.
    pub filter: String,
    /// Visible instances after filtering, in display order.
    pub instances: Vec<InstanceEntry>,
}

impl AppSnapshot {
    /// Serialize the snapshot (used by the headless backend tests and by
    /// future remote-debug tooling).
    pub fn to_json(&self) -> Result<String, crate::Error> {
        serde_json::to_string_pretty(self).map_err(crate::Error::Json)
    }
}

/// Input event polled from the backend (window system or test script).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuiEvent {
    /// Switch to another page.
    Navigate(Page),
    /// Replace the instance-list filter text.
    SetFilter(String),
    /// Select (or with `None`, deselect) the current instance.
    SelectInstance(Option<String>),
    /// Re-read instances and groups from disk.
    Refresh,
    /// Request application exit.
    Quit,
}

/// Rendering backend plug-in point. `render` paints one [`AppSnapshot`];
/// `poll_event` returns the next pending input event, if any.
pub trait GuiBackend {
    /// Paint one snapshot.
    fn render(&mut self, snapshot: &AppSnapshot);
    /// Return the next pending event, or `None` when idle.
    fn poll_event(&mut self) -> Option<GuiEvent>;
}

/// Test backend: records every rendered snapshot and replays queued events.
#[derive(Debug, Default)]
pub struct HeadlessBackend {
    renders: Vec<AppSnapshot>,
    events: VecDeque<GuiEvent>,
}

impl HeadlessBackend {
    /// Empty backend with no queued events.
    pub fn new() -> Self {
        HeadlessBackend::default()
    }

    /// Queue an event to be returned by a later [`GuiBackend::poll_event`].
    pub fn push_event(&mut self, event: GuiEvent) {
        self.events.push_back(event);
    }

    /// All snapshots rendered so far, in order.
    pub fn renders(&self) -> &[AppSnapshot] {
        &self.renders
    }

    /// Number of snapshots rendered so far.
    pub fn render_count(&self) -> usize {
        self.renders.len()
    }
}

impl GuiBackend for HeadlessBackend {
    fn render(&mut self, snapshot: &AppSnapshot) {
        self.renders.push(snapshot.clone());
    }

    fn poll_event(&mut self) -> Option<GuiEvent> {
        self.events.pop_front()
    }
}

/// View-model owner driving one [`GuiBackend`].
pub struct App<B: GuiBackend> {
    paths: PrismPaths,
    instances: InstanceListModel,
    settings: SettingsModel,
    route: Route,
    filter: String,
    backend: B,
    quit_requested: bool,
}

impl<B: GuiBackend> App<B> {
    /// Build an app over `paths` with an explicit backend. Ensures the
    /// standard data-root layout exists (like Prism's startup `mkpath`
    /// calls), loads the instance list, and loads the global settings
    /// best-effort (a missing `prismlauncher.cfg` starts empty).
    pub fn with_backend(paths: PrismPaths, backend: B) -> Result<Self, crate::Error> {
        paths.ensure_layout()?;
        let instances = InstanceListModel::load(&paths)?;
        let global_path = paths.global_config();
        let global = match Settings::load(&global_path) {
            Ok(settings) => settings,
            Err(_) => Settings::empty(&global_path),
        };
        Ok(App {
            paths,
            instances,
            settings: SettingsModel::new(global),
            route: Route::default(),
            filter: String::new(),
            backend,
            quit_requested: false,
        })
    }

    /// Data-root paths backing this app.
    pub fn paths(&self) -> &PrismPaths {
        &self.paths
    }

    /// Current instance list model.
    pub fn instances(&self) -> &InstanceListModel {
        &self.instances
    }

    /// Current settings model (global; bind an instance for overrides).
    pub fn settings(&self) -> &SettingsModel {
        &self.settings
    }

    /// Mutable access to the settings model.
    pub fn settings_mut(&mut self) -> &mut SettingsModel {
        &mut self.settings
    }

    /// Current route.
    pub fn route(&self) -> &Route {
        &self.route
    }

    /// Current filter text.
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// The backend (read-only; tests inspect [`HeadlessBackend::renders`]).
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Mutable access to the backend (tests queue events here).
    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    /// Whether [`GuiEvent::Quit`] has been processed.
    pub fn should_quit(&self) -> bool {
        self.quit_requested
    }

    /// Build the current snapshot (filtered instance list + route).
    pub fn snapshot(&self) -> AppSnapshot {
        AppSnapshot {
            route: self.route.clone(),
            filter: self.filter.clone(),
            instances: self.instances.filter(&self.filter).into_iter().cloned().collect(),
        }
    }

    /// Current snapshot serialized as pretty JSON.
    pub fn snapshot_json(&self) -> Result<String, crate::Error> {
        self.snapshot().to_json()
    }

    /// Re-read instances and groups from disk (keeps route/filter).
    pub fn reload(&mut self) -> Result<(), crate::Error> {
        self.instances = InstanceListModel::load(&self.paths)?;
        Ok(())
    }

    /// Drive model to backend once: render the current snapshot, then apply
    /// at most one pending event. Returns `Ok(false)` once quit was
    /// requested, `Ok(true)` otherwise.
    pub fn tick(&mut self) -> Result<bool, crate::Error> {
        let snapshot = self.snapshot();
        self.backend.render(&snapshot);
        match self.backend.poll_event() {
            None => {}
            Some(GuiEvent::Navigate(page)) => {
                self.route.page = page;
            }
            Some(GuiEvent::SetFilter(query)) => {
                self.filter = query;
            }
            Some(GuiEvent::SelectInstance(id)) => {
                self.route.instance_id = id;
            }
            Some(GuiEvent::Refresh) => {
                self.reload()?;
            }
            Some(GuiEvent::Quit) => {
                self.quit_requested = true;
            }
        }
        Ok(!self.quit_requested)
    }
}

impl<B: GuiBackend + Default> App<B> {
    /// Build an app over `paths` with a default-constructed backend.
    pub fn new(paths: PrismPaths) -> Result<Self, crate::Error> {
        Self::with_backend(paths, B::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prism_core::instance::Instance;

    fn test_app() -> (tempfile::TempDir, App<HeadlessBackend>) {
        let dir = tempfile::tempdir().unwrap();
        let paths = PrismPaths::at(dir.path());
        std::fs::create_dir_all(paths.instances_dir()).unwrap();
        Instance::create(&paths.instances_dir(), "Beta", "1.21.1").unwrap();
        Instance::create(&paths.instances_dir(), "alpha", "1.20.4").unwrap();
        let app = App::new(paths).unwrap();
        (dir, app)
    }

    #[test]
    fn tick_renders_once_with_no_events() {
        let (_dir, mut app) = test_app();
        assert!(app.tick().unwrap());
        assert_eq!(app.backend().render_count(), 1);
        let snapshot = &app.backend().renders()[0];
        // Sorted by name, unfiltered.
        let names: Vec<&str> = snapshot.instances.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "Beta"]);
        assert_eq!(snapshot.route.page, Page::Instances);
        assert!(!app.should_quit());
    }

    #[test]
    fn tick_applies_navigation_filter_and_selection() {
        let (_dir, mut app) = test_app();
        app.backend_mut().push_event(GuiEvent::Navigate(Page::Settings));
        assert!(app.tick().unwrap());
        assert_eq!(app.route().page, Page::Settings);

        app.backend_mut().push_event(GuiEvent::SetFilter("alp".to_string()));
        assert!(app.tick().unwrap());
        // The render for this tick still shows the previous filter; the next
        // tick shows the filtered list.
        assert!(app.tick().unwrap());
        let snapshot = app.backend().renders().last().unwrap();
        assert_eq!(snapshot.instances.len(), 1);
        assert_eq!(snapshot.instances[0].name, "alpha");

        app.backend_mut().push_event(GuiEvent::SelectInstance(Some("alpha".to_string())));
        assert!(app.tick().unwrap());
        assert_eq!(app.route().instance_id.as_deref(), Some("alpha"));
        app.backend_mut().push_event(GuiEvent::SelectInstance(None));
        assert!(app.tick().unwrap());
        assert_eq!(app.route().instance_id, None);
    }

    #[test]
    fn tick_refresh_picks_up_new_instances_and_quit_stops() {
        let (dir, mut app) = test_app();
        let paths = PrismPaths::at(dir.path());
        Instance::create(&paths.instances_dir(), "Gamma", "1.21.1").unwrap();
        // Not visible until refresh.
        assert!(app.tick().unwrap());
        assert_eq!(app.backend().renders().last().unwrap().instances.len(), 2);

        app.backend_mut().push_event(GuiEvent::Refresh);
        assert!(app.tick().unwrap());
        // The refresh tick renders the pre-refresh snapshot, then reloads;
        // the next tick paints the reloaded list.
        assert_eq!(app.backend().renders().last().unwrap().instances.len(), 2);
        assert!(app.tick().unwrap());
        assert_eq!(app.backend().renders().last().unwrap().instances.len(), 3);

        app.backend_mut().push_event(GuiEvent::Quit);
        assert!(!app.tick().unwrap());
        assert!(app.should_quit());
    }

    #[test]
    fn snapshot_json_round_trips() {
        let (_dir, app) = test_app();
        let json = app.snapshot_json().unwrap();
        let back: AppSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(back.instances.len(), 2);
        assert_eq!(back.route.page, Page::Instances);
    }

    #[test]
    fn new_on_fresh_root_starts_empty() {
        let dir = tempfile::tempdir().unwrap();
        let app: App<HeadlessBackend> = App::new(PrismPaths::at(dir.path())).unwrap();
        assert!(app.instances().is_empty());
        // Layout was created as a side effect.
        assert!(app.paths().instances_dir().is_dir());
    }
}
