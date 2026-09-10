#![windows_subsystem = "windows"]
//! Prism desktop window (iced 0.12).
//!
//! The full Prism Launcher-style interface lives in [`app::PrismApp`]
//! (toolbar, instance grid, sidebar, pages, status bar, add-instance panel,
//! offline launch flow); this file only holds the two thin runtime shells
//! over it:
//!
//! * [`State`] implements [`iced::Sandbox`] — the original API of this
//!   crate, kept compiling and working. `Sandbox` cannot stream background
//!   output (its blanket `Application` impl hardcodes
//!   `Subscription::none()`), so a Launch here runs an honest synchronous
//!   dry run instead of spawning, and instances load synchronously.
//! * [`App`] implements [`iced::Application`] from the same iced 0.12 crate
//!   (no upgrade) and additionally wires subscriptions: the
//!   `iced::subscription::channel` keyed by the launch run id streams the
//!   worker thread in `launch.rs`, and a second channel fills the instance
//!   grid from a background thread so the window paints instantly.
//!
//! Renderer choice (see `main`): the tiny-skia software renderer is
//! preferred via `ICED_BACKEND`, with `antialiasing: false` for the fastest
//! first paint. `cargo build` and `cargo test` stay headless; the binary is
//! never executed by tests.

mod accounts;
mod app;
mod icons;
mod launch;
mod mods;

use app::{console_scroll_id, Message, PrismApp};
use iced::widget::scrollable::RelativeOffset;
use iced::{Application, Command, Element, Sandbox, Settings, Subscription, Theme};
use prism_core::paths::PrismPaths;

fn main() -> iced::Result {
    // Prefer the tiny-skia software renderer. iced 0.12 ships both backends
    // (`iced_renderer::Renderer::{TinySkia, Wgpu}`) and its compositor tries
    // wgpu first, falling back to tiny-skia only on failure. On this Windows
    // box the wgpu path stalls startup (adapter enumeration plus first-use
    // shader compilation before the first frame can present), which is the
    // visible "stutter": the window appears late even though our own IO is
    // tiny. tiny-skia rasterizes on the CPU and presents the first frame
    // immediately; for a mostly-static launcher grid the per-frame cost is
    // negligible. This only sets the default: `ICED_BACKEND=wgpu` (or
    // `tiny-skia`) in the environment still wins, so machines with good GPU
    // drivers can opt back into hardware rendering without recompiling.
    if std::env::var_os("ICED_BACKEND").is_none() {
        std::env::set_var("ICED_BACKEND", "tiny-skia");
    }
    // `antialiasing` defaults to false in iced 0.12; set it explicitly so the
    // fastest-first-paint choice is visible (no MSAA pipeline setup).
    let mut settings = Settings::default();
    settings.antialiasing = false;
    App::run(settings)
}

/// Original `Sandbox` shell: fully interactive for everything synchronous.
/// Kept so the pre-existing API (`State::run`) keeps working.
pub struct State {
    app: PrismApp,
}

impl Sandbox for State {
    type Message = Message;

    fn new() -> Self {
        State { app: PrismApp::new() }
    }

    fn title(&self) -> String {
        self.app.title()
    }

    fn update(&mut self, message: Message) {
        let launched = matches!(message, Message::LaunchPressed);
        self.app.update(message);
        if launched {
            // No subscription runtime here: resolve + report synchronously.
            self.app.sandbox_drain_launch();
        }
    }

    fn view(&self) -> Element<'_, Message> {
        self.app.view()
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }
}

/// Full shell with background instance loading + launch streaming via
/// `Subscription`. Starts from an instant placeholder ([`PrismApp::pending`])
/// so the window paints before any disk IO finishes.
pub struct App {
    app: PrismApp,
}

impl Application for App {
    type Executor = iced::executor::Default;
    type Flags = ();
    type Message = Message;
    type Theme = Theme;

    fn new(_flags: ()) -> (Self, Command<Message>) {
        (App { app: PrismApp::pending(PrismPaths::detect()) }, Command::none())
    }

    fn title(&self) -> String {
        self.app.title()
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        let snap = matches!(message, Message::LaunchLog { .. }) && self.app.autoscroll_enabled();
        self.app.update(message);
        if snap {
            iced::widget::scrollable::snap_to(console_scroll_id(), RelativeOffset::END)
        } else {
            Command::none()
        }
    }

    fn view(&self) -> Element<'_, Message> {
        self.app.view()
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn subscription(&self) -> Subscription<Message> {
        self.app.subscription()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shells_share_title_and_theme() {
        let state = <State as Sandbox>::new();
        let (app, _) = App::new(());
        assert_eq!(<State as Sandbox>::title(&state), app.title());
        assert_eq!(<State as Sandbox>::title(&state), "Prism Launcher (Rust)");
        assert!(matches!(<State as Sandbox>::theme(&state), Theme::Dark));
        assert!(matches!(app.theme(), Theme::Dark));
    }

    #[test]
    fn sandbox_launch_without_selection_is_honest() {
        // Hermetic on purpose: the real data root may pre-select
        // `SelectedInstance`, which would give this shell a selection. The
        // point here is only that launching with *no* selection records no
        // run and spawns nothing.
        let dir = tempfile::tempdir().unwrap();
        let paths = PrismPaths::at(dir.path());
        std::fs::create_dir_all(paths.configured_instances_dir()).unwrap();
        let mut state = State { app: PrismApp::with_paths(paths) };
        <State as Sandbox>::update(&mut state, Message::LaunchPressed);
        // Nothing to stream and nothing spawned: the status says so.
        assert!(state.app.take_active_run().is_none());
    }
}
