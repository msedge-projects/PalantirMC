#![windows_subsystem = "windows"]
//! PalantirMC desktop entry point (iced 0.12).
//!
//! The whole interface lives in [`app::PrismApp`]; this file only holds the
//! two thin runtime shells over it:
//!
//! * [`State`] implements [`iced::Sandbox`] — the crate's original API, kept
//!   compiling and working. `Sandbox` cannot stream background output (its
//!   blanket `Application` impl hardcodes `Subscription::none()`), so a launch
//!   here performs an honest synchronous dry run and instances load during
//!   construction.
//! * [`App`] implements [`iced::Application`] from the same iced 0.12 crate and
//!   wires the subscriptions: instance scans, the version catalog, Modrinth
//!   searches and downloads, launcher scans and the launch stream.
//!
//! Window: the shell draws its own title bar, so the window is created without
//! decorations and carries the PalantirMC icon. `cargo build`/`cargo test`
//! stay headless; the binary is never executed by tests.
//!
//! Renderer: iced ships a `wgpu` compositor and a tiny-skia rasteriser. The
//! choice is made by asking wgpu what this machine would actually give it:
//! iced is pinned to the rasteriser only when nothing here offers an
//! accelerated backend, and its own order is left untouched otherwise — see
//! [`gpu`]. Nothing here hardcodes a backend.

mod accounts;
mod app;
mod brand;
mod browse;
mod catalog;
mod glyphs;
mod gpu;
mod icons;
mod instances;
mod launch;
mod mods;
mod native;
mod prefs;
mod screenshots;
mod scroll;
mod theme;

use app::{console_scroll_id, Message, PrismApp};
use iced::widget::scrollable::RelativeOffset;
use iced::{window, Application, Command, Element, Sandbox, Settings, Subscription, Theme};
use prism_core::paths::PrismPaths;

/// The size the shell would like, before the screen gets a say.
const PREFERRED_SIZE: (f32, f32) = (1280.0, 820.0);
/// Below this the layouts stop working, so it beats the screen size.
const MINIMUM_SIZE: (f32, f32) = (980.0, 640.0);
/// How much of the work area a window that has to shrink may take up. Leaves
/// enough gap that the window still reads as a window rather than a maximized
/// one, and keeps clear of rounded screen corners.
const WORK_AREA_FRACTION: f32 = 0.92;

/// The window size to open at, clamped so it fits the screen it is opening on.
fn opening_size() -> (f32, f32) {
    native::fit_to_work_area(
        PREFERRED_SIZE,
        native::primary_work_area(),
        MINIMUM_SIZE,
        WORK_AREA_FRACTION,
    )
}

/// Window settings: undecorated (the shell paints its own bar), branded icon,
/// and a size the screen can actually hold.
///
/// Opening larger than the display is what made the window feel broken: on a
/// 1366x768 screen the old fixed 1280x820 put the status bar and the bottom of
/// the sidebar off-screen, so they could neither be read nor grabbed. Sizing
/// from the work area fixes that at the source rather than asking the user to
/// resize a window that is already bigger than their screen.
fn window_settings() -> window::Settings {
    let (width, height) = opening_size();
    window::Settings {
        size: iced::Size::new(width, height),
        position: window::Position::Centered,
        min_size: Some(iced::Size::new(MINIMUM_SIZE.0, MINIMUM_SIZE.1)),
        decorations: false,
        icon: brand::window_icon(),
        ..Default::default()
    }
}

/// Hand the window's frame back to Windows, as soon as there is a window.
///
/// `install_hit_test` takes over `WM_NCHITTEST` on the shell's window, which is
/// what makes Windows run its own resize loop (with its own cursors, including
/// the diagonals iced has no way to ask for) and what makes the maximize button
/// a non-client region Windows 11 will put Snap Layouts on.
///
/// It cannot happen in `new`: iced runs `Application::new` *before* it builds
/// the window. The first dispatched message is the earliest safe moment, and
/// there is one before the window is ever painted — the constructor asks the
/// window whether it opened maximized. Once it has taken effect this is an
/// atomic load, so it is called from every update rather than tracked.
///
/// A failure is not reported and not retried forever: the shell's own resize
/// bands still work without it (see `PrismApp::view`), so the window stays
/// usable and the only losses are the native cursors, Aero Snap's edges and
/// the Snap Layouts flyout.
fn install_hit_test() {
    let _ = native::install_hit_test();
}

/// The five Inter weights the shell draws with, carried in the binary.
///
/// Supplied through `Settings::fonts` rather than `iced::font::load`, and the
/// difference is not cosmetic: settings are in place before the first frame, so
/// the very first paint is already Inter, whereas a load command means a frame
/// or two of fallback text plus five messages -- and here a message rebuilds the
/// entire interface, so that would be five extra rebuilds during startup.
///
/// Subset by `tools/make_fonts.py` from Inter 3.19, the release Modrinth's own
/// stylesheet pins. All five together are ~292 KB.
static FONTS: [&[u8]; 5] = [
    &include_bytes!("../assets/fonts/Inter-400.otf")[..],
    &include_bytes!("../assets/fonts/Inter-500.otf")[..],
    &include_bytes!("../assets/fonts/Inter-600.otf")[..],
    &include_bytes!("../assets/fonts/Inter-700.otf")[..],
    &include_bytes!("../assets/fonts/Inter-800.otf")[..],
];

fn main() -> iced::Result {
    // Decide the renderer from what this machine can actually provide, before
    // iced builds its compositor. See `gpu` for why this is a probe.
    let _ = gpu::select_renderer();
    // The OS appearance feeds the "Sync with system" theme, so it is read once
    // here rather than in a paint path, and the saved theme is put in force
    // before the first frame. Both belong to the process rather than to
    // `PrismApp`, which is why they are applied by the entry point: constructing
    // an app has no business changing global state, and keeping it that way is
    // what stops the test suite's fifty-odd app constructions from racing each
    // other through the palette.
    let paths = PrismPaths::detect();
    theme::set_os_prefers_light(native::system_prefers_light());
    theme::set_color_theme(prefs::load(&paths).theme());
    let mut settings = Settings::default();
    settings.window = window_settings();
    settings.antialiasing = false;
    settings.fonts = FONTS
        .iter()
        .map(|bytes| std::borrow::Cow::Borrowed(*bytes))
        .collect();
    // Modrinth sets its entire interface at weight 500 (`--font-weight-text`),
    // so the shell's default is the medium face rather than the regular one.
    // Headings then ask for `theme::heading()` and everything else inherits.
    settings.default_font = theme::medium();
    App::run(settings)
}

/// Original `Sandbox` shell: fully interactive for everything synchronous.
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
        install_hit_test();
        let launched = matches!(message, Message::PlayInstance(_));
        let _ = self.app.update(message);
        if launched {
            // No subscription runtime here: resolve + report synchronously.
            self.app.sandbox_drain_launch();
        }
    }

    fn view(&self) -> Element<'_, Message> {
        self.app.view()
    }

    fn theme(&self) -> Theme {
        theme::app_theme()
    }
}

/// Full shell with background work and launch streaming via `Subscription`.
/// Starts from an instant placeholder ([`PrismApp::pending`]) so the window
/// paints before any disk IO finishes.
pub struct App {
    app: PrismApp,
}

impl Application for App {
    type Executor = iced::executor::Default;
    type Flags = ();
    type Message = Message;
    type Theme = Theme;

    fn new(_flags: ()) -> (Self, Command<Message>) {
        // Ask the window what state it is already in: without this the maximize
        // button cannot know whether to offer Maximize or Restore.
        let probe = window::fetch_maximized(window::Id::MAIN, Message::MaximizedChanged);
        (App { app: PrismApp::pending(PrismPaths::detect()) }, probe)
    }

    fn title(&self) -> String {
        self.app.title()
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        install_hit_test();
        let snap = matches!(message, Message::LaunchLog { .. }) && self.app.autoscroll_enabled();
        let command = self.app.update(message);
        if snap {
            iced::widget::scrollable::snap_to(console_scroll_id(), RelativeOffset::END)
        } else {
            command
        }
    }

    fn view(&self) -> Element<'_, Message> {
        self.app.view()
    }

    fn theme(&self) -> Theme {
        theme::app_theme()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([self.app.subscription(), self.app.keyboard()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shells_share_title_and_theme() {
        // Hermetic on purpose: both shells wrap a launcher pointed at the same
        // empty data root, so their titles must agree exactly instead of
        // depending on whatever the developer's real data root has selected.
        let dir = tempfile::tempdir().unwrap();
        let paths = PrismPaths::at(dir.path());
        std::fs::create_dir_all(paths.configured_instances_dir()).unwrap();
        let state = State { app: PrismApp::with_paths(paths.clone()) };
        let app = App { app: PrismApp::with_paths(paths) };
        assert_eq!(<State as Sandbox>::title(&state), app.title());
        assert_eq!(<State as Sandbox>::title(&state), brand::APP_NAME);
        assert!(format!("{:?}", <State as Sandbox>::theme(&state)).contains("PalantirMC"));
        assert!(format!("{:?}", app.theme()).contains("PalantirMC"));
    }

    #[test]
    fn window_is_undecorated_and_branded() {
        let settings = window_settings();
        assert!(!settings.decorations, "the shell paints its own title bar");
        assert!(settings.icon.is_some(), "the window carries the brand icon");
        assert_eq!(settings.min_size, Some(iced::Size::new(980.0, 640.0)));
        // The size is now the screen's business, but it must never be larger
        // than what was asked for, and never smaller than the floor.
        assert!(settings.size.width <= PREFERRED_SIZE.0);
        assert!(settings.size.height <= PREFERRED_SIZE.1);
        assert!(settings.size.width >= MINIMUM_SIZE.0);
        assert!(settings.size.height >= MINIMUM_SIZE.1);
        assert_eq!(
            settings.position,
            window::Position::Centered,
            "the window centres itself instead of landing wherever Windows decides"
        );
    }

    #[test]
    fn the_opening_size_always_fits_the_screen_it_is_given() {
        // Table-driven over real displays, including the 1366x768 laptop this
        // launcher was reported broken on.
        let displays = [
            (1366.0, 768.0),
            (1920.0, 1080.0),
            (2560.0, 1440.0),
            (3840.0, 2160.0),
            (800.0, 600.0),
        ];
        for (width, height) in displays {
            let work = native::WorkArea { x: 0.0, y: 0.0, width, height };
            let (opened_width, opened_height) = native::fit_to_work_area(
                PREFERRED_SIZE,
                Some(work),
                MINIMUM_SIZE,
                WORK_AREA_FRACTION,
            );
            assert!(opened_width <= PREFERRED_SIZE.0);
            assert!(opened_height <= PREFERRED_SIZE.1);
            if width >= MINIMUM_SIZE.0 / WORK_AREA_FRACTION {
                assert!(
                    opened_width <= width,
                    "{opened_width} does not fit a {width}px-wide screen"
                );
            }
            if height >= MINIMUM_SIZE.1 / WORK_AREA_FRACTION {
                assert!(
                    opened_height <= height,
                    "{opened_height} does not fit a {height}px-tall screen"
                );
            }
        }
    }

    #[test]
    fn sandbox_launch_without_selection_is_honest() {
        // Hermetic on purpose: the real data root may pre-select an instance,
        // so this builds against an empty temp root instead.
        let dir = tempfile::tempdir().unwrap();
        let paths = PrismPaths::at(dir.path());
        std::fs::create_dir_all(paths.configured_instances_dir()).unwrap();
        let mut state = State { app: PrismApp::with_paths(paths) };
        <State as Sandbox>::update(&mut state, Message::PlayInstance(String::new()));
        assert!(state.app.take_active_run().is_none());
        assert!(state.app.status().contains("Pick an instance"));
    }

    #[test]
    fn view_builds_without_a_window() {
        // A full `view()` pass over the default state: catches layout builder
        // regressions (bad lengths, mismatched widget types) in CI.
        let dir = tempfile::tempdir().unwrap();
        let paths = PrismPaths::at(dir.path());
        std::fs::create_dir_all(paths.configured_instances_dir()).unwrap();
        let state = State { app: PrismApp::with_paths(paths) };
        // Building the tree is the test: every widget is constructed, every
        // style function runs, and the element is dropped again.
        let element: Element<'_, Message> = <State as Sandbox>::view(&state);
        let _ = element;
    }

}
