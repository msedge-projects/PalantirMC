#![windows_subsystem = "windows"]
//! PalantirMC desktop entry point (iced 0.12).
//!
//! One shell draws this window: [`shell::Shell`], the reference's own
//! information architecture in iced. What this file holds is the runtime around
//! it — which renderer, which window, which fonts — and the tests that check the
//! window opens where the screen allows.
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
mod anim;
mod brand;
mod browse;
mod catalog;
/// The panel's getting-started checklist: its three steps, and the two rules the
/// reference draws them by -- the flag that shows the section, and the one the
/// friends list beside it is gated on.
mod checklist;
/// The metadata a launch resolves through: the loaders' own services, and
/// Prism's mirror only where a publisher serves nothing in the shape this
/// launcher reads.
mod meta;
/// The colour theme setting: the reference's own option list, its dev-mode rule
/// for retro, and labels read from the generated string table.
///
/// Its own module rather than a corner of [`theme`]: the setting outlives the
/// hand-written palette that the last stage of the rewrite deletes, and the list
/// of themes is the reference's, not a palette's.
mod color_theme;
mod gpu;
/// Reporting a pointer crossing as a message: the only place a hover tween
/// can be started from. See the module for why the view cannot start one.
mod hover;
/// Drawing one of the reference's icons: the scale and the centring, which are
/// the two things a caller cannot guess.
mod icon;
/// The reference client's icon set, compiled from its vendored SVGs by
/// `tools/gen_icons.py` into geometry the toolkit strokes.
///
/// Not `cfg(test)`, for the same reason [`theme_gen`] is not: this is the
/// intended runtime source — the shell's icons are these, and the last carved
/// bitmaps went with the shell they were drawn in.
mod icons_gen;
mod install;
mod instances;
mod java_runtime;
mod launch;
/// The language the interface is in: the tag, the table behind it, the fallback
/// when the table has nothing to say, and the writing direction.
///
/// It is what makes [`text_gen`]'s `Key::message` read a language rather than
/// always English, and it is where the CLDR plural rule lives -- the runtime half
/// of the table `tools/gen_locale.py` compiles against. The choice is persisted by
/// [`prefs`], which has carried a `locale` field since before there was a table
/// for it to name.
mod locale;
/// The reference client's other 33 locale trees, compiled by
/// `tools/gen_locale.py` into sparse `(index, template)` tables.
///
/// Not `cfg(test)`, for the same reason [`text_gen`] is not: this is where a
/// translated string comes from once the language setting is anything but
/// English. A key a table does not carry falls back to [`text_gen`], which is
/// what the reference's own `fallbackLocale: 'en-US'` does. The gate is the
/// command -- `python tools/gen_locale.py --check` -- rather than a test,
/// because what it compares is this file against the locales it came from.
mod locale_gen;
/// The view-model whose two readers are left: the instance list `instances.rs` is
/// built from, and the override-gated settings `launch.rs` resolves a run with.
///
/// It arrived as a crate of its own -- `palantir-gui`, whose only dependant was
/// the shell that has since been deleted and whose reason to exist was a CLI
/// that is gone -- and it lives here now so that the crate can go. What is
/// deliberately *not* done in the same move is renaming or reshaping it: the
/// model is Prism-shaped because `palantir_core::settings` is, and both go when
/// the flattening importer replaces the last of them. Moving it and redesigning
/// it in one step would make a broken launch impossible to tell from a changed
/// one.
mod model;
mod mods;
/// The reference's motion: its own durations and curves, and the cubic Bézier
/// solver a native toolkit does not have.
///
/// Every number comes from [`theme_gen`], so a duration is a citation rather
/// than a choice; the solver is checked against Chromium's own answers, which
/// `tools/curve_samples.html` measures.
mod motion;
mod native;
/// Opening a link in the browser the machine already has.
///
/// One function and one rule set -- `http`/`https` and nothing else -- because the
/// links this launcher draws include a stranger's feeds and a press is what hands
/// a string to the operating system. The panel's news section is what needed it
/// first (`NEXT_STEPS.md`, stage 3).
mod open;
/// The scaffold every page is built from: the state of what a page asked for, and
/// the blocks it draws when the answer is not ready yet.
///
/// Stage 3's pages are what use it; it is here, before them, because "every page
/// has a loading, an empty and a failed state" is a property of the scaffold or it
/// is a promise about nineteen files.
mod page;
/// The pages themselves: one module per route, and the `Screen` that is which of
/// them the shell is showing.
///
/// Stage 3 of the rewrite spec. A page owns its own state and its own messages
/// and reports the things only the shell can do (opening an instance, opening a
/// project) rather than performing them -- see `pages::Open`.
mod pages;
mod prefs;
/// The token gate: the reference's own source, compared with what this shell
/// paints. Compiled for tests only -- it reads files, and the shell never does.
#[cfg(test)]
mod reference_tokens;
/// The independence gate for the generated vocabulary: the sheets read a
/// second time, with a parser that shares nothing with the generator, and the
/// two readings compared row by row.
#[cfg(test)]
mod reference_vocabulary;
/// Where the shell can be: the reference's navigation, mirroring its
/// `routes.js` route for route and name for name.
///
/// Not the old shell's page list. That one put Mods, Worlds, Logs, Settings,
/// Accounts and About on the rail; the reference keeps the first four inside
/// an instance and Settings in a modal, and this is the module that says so.
mod route;
/// What the interface costs at size: the instance page's tabs against a `mods/`
/// folder of thousands, Discover against a hundred hits, and the interaction
/// clock against every control on a page. Compiled for tests only -- it builds
/// fixtures and asserts an envelope, and the shell never reads a number out of
/// it. The numbers themselves are recorded in `GATES.md`.
#[cfg(test)]
mod scale;
mod screenshots;
mod scroll;
/// The shell: the rail, the head, the page pane, the right panel and Settings as
/// a modal, on the reference's own information architecture.
///
/// **This is the only shell there is.** It replaced the one that preceded it,
/// which is deleted rather than kept behind a flag: its page names, its capture
/// flag and its `PalantirApp` state are gone, and the parts of it the product
/// still needed — the native frame, and a `--shot` capture — are here.
mod shell;
/// A Minecraft skin as a picture: the texture's own layout, cut into the front
/// view the Skins page draws.
///
/// Its own module rather than a corner of that page, because the arithmetic is
/// the format's rather than the page's -- and because it is the one place this
/// launcher draws something a *service* published rather than something it drew.
mod skin;
mod store;
/// The one way this launcher reaches the network: the engine's cache, its pool
/// and its queue, as the launch path uses them.
mod wire;
/// The vocabulary the interface paints with: which token an ink is, and which
/// face a piece of text is set in.
///
/// Shared by the shell and the pages, so "the token behind `text-primary`" has one
/// answer rather than two.
mod style;
/// The runtime behind [`text_gen`]: the plural value, the English plural rule and
/// the number formatting the reference's strings are filled in by.
mod text;
/// The widgets the pages are drawn from, each quoting the reference's own class or
/// rule rather than inventing a look.
///
/// Stage 3's pages are what use it; keeping them here rather than inside a page is
/// what makes two pages share a card instead of each drawing one.
mod ui;
/// The reference client's own strings, compiled from its vendored English locale
/// by `tools/gen_text.py`.
///
/// Not `cfg(test)`, for the same reason [`theme_gen`] and [`icons_gen`] are not:
/// this is where the interface's copy comes from, so a page has nothing of its
/// own to invent and every sentence can be traced back to the reference's key.
/// The gate is a command -- `python tools/gen_text.py --check` -- rather than a
/// test, because what it compares is this file against the locale it came from.
mod text_gen;
mod theme;
/// The reference client's own design system, compiled from its vendored
/// stylesheets by `tools/gen_theme.py`.
///
/// Deliberately **not** `cfg(test)`, unlike [`theme_tokens`]: that module was a
/// receipt for values transcribed by hand, and this one is the thing the shell
/// paints from. The gate is not a test but a command --
/// `python tools/gen_theme.py --check` fails if this file is not what the tool
/// emits -- and the palette it will replace is [`theme`], which carries a
/// second, hand-written copy of the same values today (NOTES 28).
mod theme_gen;
/// The reference's full token vocabulary, generated by `tools/gen_tokens.py`
/// from the vendored sheets. Compiled for tests only: the palette paints from
/// `theme.rs`, and this copy is what the vocabulary gate re-reads the sheets
/// against. When a page starts consuming a token at runtime, the token moves
/// into `theme.rs` and this row stays as the receipt.
#[cfg(test)]
mod theme_tokens;

use iced::{window, Application, Settings};

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
///
/// Each entry relies on the declared element type to unsize
/// `&[u8; N]` into `&[u8]`. Writing that as `&include_bytes!(..)[..]` instead is
/// the same bytes and is what this was first written as — and it does not
/// build: slicing in a `static` initialiser needs `std::ops::Index` in const,
/// which is not stable, so the table failed to compile on CI's toolchain while
/// the identical code was fine on a slightly older one. The coercion is what
/// const evaluation actually allows.
static FONTS: [&[u8]; 5] = [
    include_bytes!("../assets/fonts/Inter-400.otf"),
    include_bytes!("../assets/fonts/Inter-500.otf"),
    include_bytes!("../assets/fonts/Inter-600.otf"),
    include_bytes!("../assets/fonts/Inter-700.otf"),
    include_bytes!("../assets/fonts/Inter-800.otf"),
];

fn main() -> iced::Result {
    // Decide the renderer from what this machine can actually provide, before
    // iced builds its compositor. See `gpu` for why this is a probe.
    let _ = gpu::select_renderer();
    run_shell(std::env::args().skip(1))
}

/// Run the shell: the window, the fonts, and the flags this run was given.
fn run_shell(args: impl Iterator<Item = String>) -> iced::Result {
    let flags = shell::Flags::from_args(args);
    // A capture states its size and is born off the desktop; both are decided
    // here because they are window settings, and read before `flags` is moved
    // into them.
    let capture = flags.shot.is_some();
    let mut settings = Settings::default();
    settings.window = shell_window_settings(flags.size, capture);
    settings.antialiasing = false;
    settings.fonts = FONTS
        .iter()
        .map(|bytes| std::borrow::Cow::Borrowed(*bytes))
        .collect();
    // Modrinth sets its entire interface at weight 500 (`--font-weight-text`),
    // so the shell's default is the medium face rather than the regular one.
    // Headings then ask for a heavier face of the same family (`theme::semibold()`
    // for a page title, `theme::bold()` for emphasis) and everything else inherits.
    settings.default_font = theme::medium();
    // `--page`, `--size` and `--shot` describe *this run*; nothing may remember
    // them once it is over.
    settings.flags = flags;
    shell::Shell::run(settings)
}

/// Window settings: undecorated (the shell paints its own bar), branded icon,
/// and a size the screen can actually hold.
///
/// Opening larger than the display is what made the window feel broken: on a
/// 1366x768 screen a fixed 1280x820 put the status bar and the bottom of the
/// sidebar off-screen, so they could neither be read nor grabbed. Sizing from
/// the work area fixes that at the source rather than asking the user to resize
/// a window that is already bigger than their screen.
fn shell_window_settings(size: Option<(u32, u32)>, capture: bool) -> window::Settings {
    // A capture states its size outright rather than letting the screen decide:
    // the numbers it is checked against are client pixels of another window at
    // an exact size, and a capture that opened at 92% of the work area would make
    // every one of them wrong by a scale factor instead of failing.
    let (width, height) = size
        .map(|(width, height)| (width as f32, height as f32))
        .unwrap_or_else(opening_size);
    // A capture is born off the desktop, so it neither takes the focus nor
    // flashes a window at whoever asked for it; everything else centres itself,
    // which is what a window is for.
    let position = if capture {
        window::Position::Specific(iced::Point::new(native::beyond_every_monitor_x(), 8.0))
    } else {
        window::Position::Centered
    };
    window::Settings {
        size: iced::Size::new(width, height),
        position,
        min_size: Some(iced::Size::new(MINIMUM_SIZE.0, MINIMUM_SIZE.1)),
        decorations: false,
        icon: brand::window_icon(),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_is_undecorated_and_branded() {
        let settings = shell_window_settings(None, false);
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
    fn a_capture_states_its_size_and_hides_itself_off_the_desktop() {
        // Both halves of the `--shot` contract the page gates are run through:
        // the size is the caller's, so a capture is the pixels of a window that
        // size, and the position is off the desktop, so taking one does not
        // interrupt whoever is at the machine.
        let settings = shell_window_settings(Some((1280, 720)), true);
        assert_eq!(settings.size, iced::Size::new(1280.0, 720.0));
        match settings.position {
            window::Position::Specific(point) => {
                assert!(point.x > 0.0, "a capture parks itself past the desktop's right edge");
            }
            other => panic!("a capture must state its position, got {other:?}"),
        }
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
}
