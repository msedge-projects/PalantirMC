//! Prism desktop window (iced 0.12).
//!
//! The full Prism Launcher-style interface lives in [`app::PrismApp`]
//! (toolbar, sidebar, pages, status bar, add-instance panel, offline launch
//! flow); this file only holds the two thin runtime shells over it:
//!
//! * [`State`] implements [`iced::Sandbox`] — the original API of this
//!   crate, kept compiling and working. `Sandbox` cannot stream background
//!   output (its blanket `Application` impl hardcodes
//!   `Subscription::none()`), so a Launch here runs an honest synchronous
//!   dry run instead of spawning.
//! * [`App`] implements [`iced::Application`] from the same iced 0.12 crate
//!   (no upgrade) and additionally wires
//!   `iced::subscription::channel` — keyed by the launch run id — to the
//!   worker thread in `launch.rs`, which `try_send`s console batches back.
//!
//! `main` runs [`App`] so launching actually streams. `cargo build` and
//! `cargo test` stay headless; the binary is never executed by tests.

mod accounts;
mod app;
mod launch;
mod mods;

use app::{console_scroll_id, Message, PrismApp};
use iced::widget::scrollable::RelativeOffset;
use iced::{Application, Command, Element, Sandbox, Settings, Subscription, Theme};

fn main() -> iced::Result {
    App::run(Settings::default())
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

/// Full shell with background launch streaming via `Subscription`.
pub struct App {
    app: PrismApp,
}

impl Application for App {
    type Executor = iced::executor::Default;
    type Flags = ();
    type Message = Message;
    type Theme = Theme;

    fn new(_flags: ()) -> (Self, Command<Message>) {
        (App { app: PrismApp::new() }, Command::none())
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
        let mut state = <State as Sandbox>::new();
        <State as Sandbox>::update(&mut state, Message::LaunchPressed);
        // Nothing to stream and nothing spawned: the status says so.
        assert!(state.app.take_active_run().is_none());
    }
}
