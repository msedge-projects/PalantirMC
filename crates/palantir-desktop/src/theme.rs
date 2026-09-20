//! PalantirMC look & feel.
//!
//! One place for the palette, the radii and every widget style, so pages stay
//! declarative. The reference is the Modrinth launcher: near-black chrome
//! (`#16181C`), slightly lighter content cards (`#26292F`), a single saturated
//! green accent (`#1BD96A`) and lots of quiet grey text — with its light and
//! OLED counterparts available as a real choice ([`ColorTheme`]) rather than a
//! second stylesheet.
//!
//! **The palette is a value, not a constant.** [`palette`] resolves the theme in
//! force into a [`Palette`], and the `palette_accessors!` block below turns each
//! of its fields into a function (`text()`, `accent()`, …) so widget styles read
//! [`palette`] at *paint* time. That is what lets Settings switch the look of
//! the whole window without a single widget style being duplicated per theme.
//!
//! iced 0.12 style plumbing notes (verified against the vendored crate
//! sources, see `README`-level comments in `app.rs` for the layout story):
//!
//! * containers/rules accept plain closures (`impl Fn(&Theme) -> Appearance`),
//!   which is how the functions below are used (`container(..).style(theme::card)`);
//! * buttons do **not** — `Theme::Style` is an enum, so [`Btn`] implements
//!   [`button::StyleSheet`] and converts into it ([`Role`] picks the look);
//! * text inputs and scrollables need explicit `StyleSheet` impls, which is
//!   [`Field`] and [`Thin`];
//! * checkboxes and pick lists are *also* enums, and their `Custom` arms carry
//!   a boxed/Rc'd `StyleSheet` — [`Tick`] and [`Dropdown`]. A pick list needs
//!   two stylesheets (field and menu), which is why `Dropdown` implements both.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use iced::gradient::Linear;
use iced::overlay::menu;
use iced::theme::Palette as IcedPalette;
use iced::widget::{
    button, checkbox, container, pick_list, progress_bar, scrollable, text_input,
};
use iced::{Background, Border, Color, Font, Gradient, Radians, Theme};

/// Build a color from sRGB bytes (const-friendly, readable hex in the source).
pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

/// Same color with a different alpha (const-friendly).
pub const fn alpha(color: Color, a: f32) -> Color {
    Color { a, ..color }
}

/// A color multiplied channel-wise, which is what CSS `filter: brightness()`
/// does.
///
/// Written out because the reference's hover for a `size="lg"` button is
/// *exactly* that filter — `[&:hover]:brightness-[--hover-brightness]`, and
/// `--hover-brightness` is 1.25 in dark — and applying the rule to the surface it
/// applies to (`surface-4`, `#34363c`) gives `#41434b`, which is not a rung of
/// any ladder. The rule is the token here, so the rule is what is written.
/// `--hover-brightness` in dark, the factor the reference's buttons, rows and
/// tabs hover by. Light is `0.9` (it darkens instead), which is a fact about the
/// reference this palette does not yet carry over.
const HOVER_BRIGHTNESS: f32 = 1.25;

fn lighten(color: Color, factor: f32) -> Color {
    Color::from_rgba(
        (color.r * factor).min(1.0),
        (color.g * factor).min(1.0),
        (color.b * factor).min(1.0),
        color.a,
    )
}

// ---- Palette -----------------------------------------------------------

/// Every color the shell paints with, resolved for one look.
///
/// A struct rather than a pile of constants, because the look is a *choice*:
/// [`palette`] returns the colors of the theme in force, and every widget style
/// reads them from there, so nothing has to know which theme that is.
pub struct Palette {
    /// Window chrome (deepest surface).
    pub bg: Color,
    /// Rails and sidebars.
    pub bg_rail: Color,
    /// Cards, modals and other raised surfaces.
    pub surface: Color,
    /// Hovered raised surface.
    pub surface_hover: Color,
    /// Inset fields (search boxes, modal inner panels).
    pub surface_input: Color,
    /// Hairline borders.
    pub border: Color,
    /// Border on hover/selection.
    pub border_strong: Color,
    /// Primary text.
    pub text: Color,
    /// Secondary text.
    pub text_muted: Color,
    /// Tertiary text / inactive rail icons.
    pub text_dim: Color,
    /// The accent (Modrinth green).
    pub accent: Color,
    /// The accent at 25% over the chrome, as a solid.
    ///
    /// `--color-brand-highlight`, which is what an active rail entry and the
    /// selected tab in the reference's own tab strip are filled with. It is a
    /// solid here rather than an alpha because the surfaces it lands on are not
    /// all the chrome: compositing `accent` at 25% over the *page* answers
    /// `#104832`, while the reference's pill measures `#1d5540` -- the answer you
    /// get over `#27292e`. Measured both places it appears: `#1d5540` in the tab
    /// strip, `#1d563f` under the rail's active entry.
    pub brand_highlight: Color,
    /// Accent, hovered.
    pub accent_hover: Color,
    /// Accent, pressed.
    pub accent_dim: Color,
    /// Text on top of the accent.
    pub on_accent: Color,
    /// Destructive actions.
    pub danger: Color,
    /// Destructive, hovered.
    pub danger_hover: Color,
    /// The welcome logo tile's own surface.
    pub hero: Color,
    /// The dimmed wash behind a dialog.
    pub backdrop: Color,
    /// The dialog surface, a shade off the cards.
    pub modal: Color,
    /// Top of the right panel's brand wash.
    pub sidebar_top: Color,
    /// Bottom of the right panel's brand wash.
    pub sidebar_bottom: Color,
    /// A card inside the right panel.
    pub sidebar_surface: Color,
    /// An interactive row inside one of those cards.
    pub sidebar_row: Color,
    /// The panel's section divider and its left edge.
    pub sidebar_border: Color,
}

impl Palette {
    /// The dark look: Omorphia's dark theme, verbatim, and the default.
    ///
    /// Every value below is a token from
    /// `modrinth/code`'s `packages/assets/styles/variables.scss` -- the sheet
    /// the Modrinth App itself is painted from -- so this is a transcription
    /// rather than an approximation of it. Each line names the token it came
    /// from, because the mapping is not one-to-one (the palette has one
    /// `border` where the design system has three).
    pub const fn dark() -> Palette {
        Palette {
            bg: rgb(0x16, 0x18, 0x1C),              // surface-1
            bg_rail: rgb(0x1D, 0x1F, 0x23),         // surface-2
            surface: rgb(0x27, 0x29, 0x2E),         // surface-3, what a card is
            surface_hover: rgb(0x34, 0x36, 0x3C),   // surface-4, and the hover of one
            surface_input: rgb(0x34, 0x36, 0x3C),   // --color-button-bg
            border: alpha(rgb(0xC1, 0xBE, 0xD1), 0.12), // --color-button-border
            border_strong: rgb(0x42, 0x44, 0x4A),   // surface-5
            text: rgb(0xFF, 0xFF, 0xFF),            // --color-text-primary
            text_muted: rgb(0xB0, 0xBA, 0xC5),      // --color-text-default
            text_dim: rgb(0x96, 0xA2, 0xB0),        // --color-text-tertiary
            // Measured, not transcribed. The stylesheet's ladder says green-500
            // (`#1bd96a`) is dark `--color-brand`, and the app as installed is
            // not painted that: `#00da75` is what the call-to-action button, the
            // logo mark and the active rail icon all measure, flat, off a
            // capture of the running window (see `REFERENCE.md`). A palette
            // transcribed from a sheet the app no longer obeys is a palette
            // that is wrong in a way no gate can see, so the measurement wins.
            accent: rgb(0x00, 0xDA, 0x75),
            brand_highlight: rgb(0x1D, 0x55, 0x40),
            // Derived by the rule this palette already documents rather than
            // taken from a ladder rung: hover is `brightness(1.25)` and pressed
            // `brightness(0.8)`, applied to the accent above (0, 218, 117), which
            // is `#00ff92` and `#00ae5e`. The ladder's green-400/green-600 were
            // the same rule applied to green-500, which is the colour this file
            // no longer uses.
            accent_hover: rgb(0x00, 0xFF, 0x92),
            accent_dim: rgb(0x00, 0xAE, 0x5E),
            on_accent: rgb(0x00, 0x00, 0x00),       // --color-accent-contrast is black in dark
            danger: rgb(0xFF, 0x49, 0x6E),          // red-500
            danger_hover: rgb(0xFF, 0x69, 0x84),    // red-400
            hero: rgb(0x13, 0x1F, 0x17),            // --brand-gradient-strong-bg, light end
            backdrop: alpha(rgb(0x16, 0x18, 0x1C), 0.64), // --splash-overlay
            modal: rgb(0x27, 0x29, 0x2E),           // surface-3: a modal is a card that floats
            // The right panel is not a flat raised strip in the reference --
            // `.app-sidebar` paints `--brand-gradient-bg`, which resolves to a
            // brand wash over the page colour. Measured down its own empty
            // gutter it ramps `#182524` at the top to `#131a1a` at the bottom,
            // dead straight (the midpoint predicts within one level), so a
            // two-stop ramp is the whole of it. Without this the panel read as
            // a lighter strip beside the page instead of as the page tinted.
            sidebar_top: rgb(0x18, 0x25, 0x24),
            sidebar_bottom: rgb(0x13, 0x1A, 0x1A),
            // Inside the panel, `--surface-4` and `--surface-5` are overridden
            // to `--brand-gradient-button` and `--brand-gradient-border`, so
            // cards and rows there are brand-tinted rather than neutral grey.
            // Both measured flat: a card is `#2a3633`, a row inside it `#3a4341`
            // (a step *lighter*, which is why rows keep their own token).
            sidebar_surface: rgb(0x2A, 0x36, 0x33),
            sidebar_row: rgb(0x3A, 0x43, 0x41),
            sidebar_border: rgb(0x30, 0x3E, 0x38), // the section's 1px divider
        }
    }

    /// The light look.
    ///
    /// Not an inversion: the accent is darkened until it reads on white -- the
    /// dark theme's green is 1.9:1 against a white card, which is not text --
    /// and the chrome sits *below* the cards rather than above them.
    ///
    /// The accent is the reference's green-700 rather than the green-600 that
    /// light mode calls `--color-brand`, and the measurement is the reason:
    /// on this theme's own card (#F8F8F8) green-600 is 2.71:1, under the 3:1
    /// floor for a UI component, and white on it is 2.88:1 where the shell puts
    /// a button label. green-700 is 3.83:1 on the card and 4.06:1 under white.
    /// Taking the next rung of the same ladder keeps this a transcription while
    /// still clearing the floor the rest of the palette is held to.
    pub const fn light() -> Palette {
        Palette {
            bg: rgb(0xEB, 0xEB, 0xEB),              // surface-1
            bg_rail: rgb(0xED, 0xED, 0xED),         // surface-1-5
            surface: rgb(0xF8, 0xF8, 0xF8),         // surface-3
            surface_hover: rgb(0xDD, 0xDD, 0xDD),   // surface-5
            surface_input: rgb(0xFF, 0xFF, 0xFF),   // surface-4
            border: alpha(rgb(0xA1, 0xA1, 0xA1), 0.35), // --color-button-border
            border_strong: rgb(0xDD, 0xDD, 0xDD),   // surface-5
            text: rgb(0x1A, 0x20, 0x2C),            // --color-text-primary
            text_muted: rgb(0x2C, 0x2E, 0x31),      // --color-text-default
            text_dim: rgb(0x48, 0x4D, 0x54),        // --color-text-tertiary
            accent: rgb(0x04, 0x91, 0x4F),          // green-700, the readable rung (see above)
            // Derived by the rule the dark theme's is measured by -- 25% of the
            // accent over that theme's chrome -- because the reference's light
            // theme was not measured. 160,214,197 is that composite over
            // `#EDEDED`.
            brand_highlight: rgb(0xB3, 0xD6, 0xC5),
            accent_hover: rgb(0x00, 0xAF, 0x5C),    // green-600, light --color-brand: hover is brighter
            accent_dim: rgb(0x03, 0x74, 0x3F),      // green-700 at brightness(0.8), the pressed rule
            on_accent: rgb(0xFF, 0xFF, 0xFF),       // --color-accent-contrast is white in light
            danger: rgb(0xCB, 0x22, 0x45),          // red-600, light --color-red
            danger_hover: rgb(0xED, 0x46, 0x61),    // red-500
            hero: rgb(0xE6, 0xF4, 0xEC),            // brand gradient over a light surface
            backdrop: alpha(rgb(0xEB, 0xEB, 0xEB), 0.7),
            modal: rgb(0xF8, 0xF8, 0xF8),           // surface-3
            // The dark panel's wash, re-expressed over this theme's page: green
            // at ~6% fading to ~2%, which is what the dark stops measure as.
            // Light `--color-brand` is green-600, so the tint follows it before
            // being lightened toward the page.
            sidebar_top: rgb(0xDD, 0xE7, 0xE2),
            sidebar_bottom: rgb(0xE6, 0xEA, 0xE8),
            // A light card is already near-white, so the tint moves the other
            // way: the panel's cards step *down* from `surface` rather than up.
            sidebar_surface: rgb(0xF2, 0xF8, 0xF5),
            sidebar_row: rgb(0xE8, 0xF0, 0xEB),
            sidebar_border: rgb(0xC9, 0xDB, 0xD2),
        }
    }

    /// True black for OLED panels.
    ///
    /// Identical to the dark look in hue, but the two deepest surfaces are
    /// actually `#000000` so an OLED panel can switch those pixels off, and the
    /// text-on-accent is black because the accent never moves.
    pub const fn oled() -> Palette {
        Palette {
            // Omorphia's `.oled-mode` overrides only the seven surfaces; every
            // other token is inherited from dark, which is why only those
            // differ here.
            bg: rgb(0x00, 0x00, 0x00),              // surface-1
            bg_rail: rgb(0x05, 0x05, 0x06),         // surface-1-5
            surface: rgb(0x10, 0x10, 0x13),         // surface-3
            surface_hover: rgb(0x1B, 0x1B, 0x20),   // surface-4
            surface_input: rgb(0x1B, 0x1B, 0x20),   // surface-4
            border: alpha(rgb(0xC1, 0xBE, 0xD1), 0.12),
            border_strong: rgb(0x25, 0x26, 0x2B),   // surface-5
            text: rgb(0xFF, 0xFF, 0xFF),
            text_muted: rgb(0xB0, 0xBA, 0xC5),
            text_dim: rgb(0x96, 0xA2, 0xB0),
            // The dark theme's measured accent and its two derived states, on
            // this theme's own chrome for the highlight: OLED differs from dark
            // only in how dark its surfaces are, and it carries the same brand.
            accent: rgb(0x00, 0xDA, 0x75),
            accent_hover: rgb(0x00, 0xFF, 0x92),
            accent_dim: rgb(0x00, 0xAE, 0x5E),
            brand_highlight: rgb(0x0C, 0x42, 0x2B),
            on_accent: rgb(0x00, 0x00, 0x00),
            danger: rgb(0xFF, 0x49, 0x6E),
            danger_hover: rgb(0xFF, 0x69, 0x84),
            hero: rgb(0x13, 0x1F, 0x17),
            backdrop: alpha(rgb(0x00, 0x00, 0x00), 0.7),
            modal: rgb(0x10, 0x10, 0x13),           // surface-3
            // The dark panel's wash and surfaces, each moved by the same step
            // that separates this theme's `surface-3` from the dark one, so the
            // panel keeps its relationship to the cards rather than going grey.
            sidebar_top: rgb(0x01, 0x0C, 0x09),
            sidebar_bottom: rgb(0x00, 0x01, 0x00),
            sidebar_surface: rgb(0x13, 0x1D, 0x18),
            sidebar_row: rgb(0x23, 0x2A, 0x26),
            sidebar_border: rgb(0x19, 0x25, 0x1D),
        }
    }
}

// ---- Color theme -------------------------------------------------------

/// A color theme, as offered in Settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorTheme {
    /// The dark look, and the default.
    #[default]
    Dark,
    /// The light look.
    Light,
    /// Black surfaces, for OLED displays.
    Oled,
    /// Follow the operating system's app appearance.
    System,
}

impl ColorTheme {
    /// Every theme, in the order Settings shows them: the two the reference
    /// client offers first, then OLED, then the OS-following one.
    pub const ALL: [ColorTheme; 4] =
        [ColorTheme::Dark, ColorTheme::Light, ColorTheme::Oled, ColorTheme::System];

    /// The label on the card.
    pub const fn label(self) -> &'static str {
        match self {
            ColorTheme::Dark => "Dark",
            ColorTheme::Light => "Light",
            ColorTheme::Oled => "OLED",
            ColorTheme::System => "Sync with system",
        }
    }

    /// Stable id, for the settings file. Never localized and never reordered;
    /// renaming one silently resets the theme of anyone who picked it.
    pub const fn id(self) -> &'static str {
        match self {
            ColorTheme::Dark => "dark",
            ColorTheme::Light => "light",
            ColorTheme::Oled => "oled",
            ColorTheme::System => "system",
        }
    }

    /// Parse a stored id. Anything unrecognized is the default rather than an
    /// error: a hand-edited or future settings file must still open.
    pub fn from_id(id: &str) -> ColorTheme {
        Self::ALL
            .into_iter()
            .find(|theme| theme.id() == id.trim().to_ascii_lowercase())
            .unwrap_or_default()
    }

    /// The concrete look this theme means.
    ///
    /// `System` is the only theme that depends on the machine, and it resolves
    /// to OLED when the OS is dark — an explicit OLED choice is the display's
    /// business, not the OS's, so "system dark" means the ordinary dark look
    /// and OLED stays something you ask for.
    pub const fn resolve(self, system_prefers_light: bool) -> ColorTheme {
        match self {
            ColorTheme::System if system_prefers_light => ColorTheme::Light,
            ColorTheme::System => ColorTheme::Dark,
            other => other,
        }
    }

    /// The colors this theme paints with, following the OS for [`ColorTheme::System`].
    pub fn palette(self) -> Palette {
        match self.resolve(os_prefers_light()) {
            ColorTheme::Light => Palette::light(),
            ColorTheme::Oled => Palette::oled(),
            _ => Palette::dark(),
        }
    }
}

/// The theme in force. A process-wide choice, like the reference client's, and
/// read on every style call — an `AtomicU8` load is cheaper than threading a
/// theme through several hundred widget builders.
static COLOR_THEME: AtomicU8 = AtomicU8::new(0);

/// Whether the OS is set to a light appearance, cached at startup.
static OS_PREFERS_LIGHT: AtomicBool = AtomicBool::new(false);

/// Serializes the tests that change the theme in force.
///
/// The palette is process-wide, so a test that switches it has to restore it
/// before another test paints with it. Shared with the app's own tests, which
/// reach the same global through `Message::SetColorTheme`.
#[cfg(test)]
pub(crate) static THEME_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The theme in force.
pub fn color_theme() -> ColorTheme {
    match COLOR_THEME.load(Ordering::Relaxed) {
        1 => ColorTheme::Light,
        2 => ColorTheme::Oled,
        3 => ColorTheme::System,
        _ => ColorTheme::Dark,
    }
}

/// Put a theme in force. The next `view()` paints with it.
pub fn set_color_theme(theme: ColorTheme) {
    let raw = match theme {
        ColorTheme::Dark => 0,
        ColorTheme::Light => 1,
        ColorTheme::Oled => 2,
        ColorTheme::System => 3,
    };
    COLOR_THEME.store(raw, Ordering::Relaxed);
}

/// Record the OS appearance, so [`ColorTheme::System`] can resolve to it.
pub fn set_os_prefers_light(light: bool) {
    OS_PREFERS_LIGHT.store(light, Ordering::Relaxed);
}

/// Whether the OS is set to a light appearance.
pub fn os_prefers_light() -> bool {
    OS_PREFERS_LIGHT.load(Ordering::Relaxed)
}

/// The colors for the theme in force.
pub fn palette() -> Palette {
    color_theme().palette()
}

/// One accessor per color.
///
/// Generated so the twenty colors cannot drift apart: each is a single field
/// read of [`palette`], and a new color is one line here rather than a
/// hand-written function that might read the wrong field.
macro_rules! palette_accessors {
    ($($name:ident($field:ident): $doc:literal,)*) => {
        $(#[doc = $doc] pub fn $name() -> Color { palette().$field })*
    };
}

palette_accessors! {
    bg(bg): "Window chrome (deepest surface).",
    bg_rail(bg_rail): "Rails and sidebars.",
    surface(surface): "Cards and other raised surfaces.",
    surface_hover(surface_hover): "Hovered raised surface.",
    surface_input(surface_input): "Inset fields (search boxes, modal inner panels).",
    border(border): "Hairline borders.",
    border_strong(border_strong): "Border on hover/selection.",
    text(text): "Primary text.",
    text_muted(text_muted): "Secondary text.",
    text_dim(text_dim): "Tertiary text / inactive rail icons.",
    accent(accent): "The accent.",
    brand_highlight(brand_highlight): "The accent at 25% over the chrome, as a solid.",
    accent_hover(accent_hover): "Accent, hovered.",
    accent_dim(accent_dim): "Accent, pressed.",
    on_accent(on_accent): "Text on top of the accent.",
    danger(danger): "Destructive actions.",
    danger_hover(danger_hover): "Destructive, hovered.",
    sidebar_top(sidebar_top): "Top of the right panel's brand wash.",
    sidebar_bottom(sidebar_bottom): "Bottom of the right panel's brand wash.",
    sidebar_surface(sidebar_surface): "A card inside the right panel.",
    sidebar_row(sidebar_row): "An interactive row inside one of those cards.",
    sidebar_border(sidebar_border): "The panel's section divider and its left edge.",
    // `hero`, `backdrop` and `modal` are deliberately absent: their only
    // consumers are the three container styles a few lines below, which read
    // them through `palette()`, and two functions of the same name would
    // collide with those styles.
}

// Corner radii, from Omorphia's scale (`--radius-xs` 4, `sm` 8, `md` 12,
// `lg` 16, `xl` 20). Each constant names the component it styles, and the
// component is what decided the value: `.base-card` is `--radius-lg`, `.btn`
// is `--radius-md`, a tooltip is `--radius-sm`.

/// Card corner radius: Omorphia's `--radius-lg`, what `.base-card` uses.
pub const R_CARD: f32 = 16.0;
/// Button corner radius: `--radius-md`, what `.btn` uses.
pub const R_BUTTON: f32 = 12.0;
/// Chip/pill corner radius: `--radius-sm`.
pub const R_CHIP: f32 = 8.0;
/// Corner radius of the rail's active plate.
///
/// The reference's rail entries are `rounded-full` in its stylesheet, which at
/// 48px square is 24 -- and the app does not draw that. Measured down the plate's
/// own top-left corner in a capture of the running window, the inset reaches 11
/// at its widest, i.e. a 12px radius: `--radius-md`, the same rung the buttons
/// use. The measurement is what this is, the same way the accent above is.
pub const R_RAIL: f32 = 12.0;
/// Modal corner radius: `--radius-lg`, as cards.
pub const R_MODAL: f32 = 16.0;
/// Corner radius on the page pane's top-left, and only there.
///
/// The reference's `.app-contents` sets `border-top-left-radius:
/// var(--radius-xl)`, so the page is a panel whose one rounded corner notches
/// into the chrome. It is `--radius-xl` (20), one rung above a card.
pub const R_PANE: f32 = 20.0;
/// Corner radius of a `size="lg"` button.
///
/// The reference gives every button size its own radius rather than one for the
/// component: `xs` is `rounded-lg` (8), `sm` `rounded-[10px]`, `md` `rounded-xl`
/// (12, which is [`R_BUTTON`]), `lg` `rounded-[14px]` and `xl` `rounded-2xl`
/// (16). 14 is what the welcome screen's two buttons draw, and the capture
/// agrees with the stylesheet: at 1.5px in from the brand button's left edge its
/// fill starts 7px down from its top, which is a 14px corner to the pixel.
pub const R_BUTTON_LG: f32 = 14.0;
/// Corner radius of the welcome screen's key cap.
///
/// Six, from `rounded-md` in the `<kbd>`'s own class list — Tailwind's rung,
/// *not* Omorphia's: this design system's `--radius-md` is 12, and reading the
/// class names off the stylesheet's scale instead of the utility's is how a key
/// cap ends up as rounded as a button.
pub const R_KEYCAP: f32 = 6.0;

/// The family name the five bundled Inter faces register under.
///
/// fontdb reads the *typographic* family (name ID 16) before the plain family
/// (ID 1), which is what makes this work: Inter's intermediate weights are
/// named "Inter Medium" and friends in ID 1, and only ID 16 says "Inter".
/// Without that, weight selection would find two faces out of five.
pub const FAMILY: &str = "Inter";

/// One of the five shipped weights, by iced's name for it.
const fn inter(weight: iced::font::Weight) -> Font {
    Font { family: iced::font::Family::Name(FAMILY), weight, ..Font::DEFAULT }
}

/// Regular (400).
pub const fn regular() -> Font {
    inter(iced::font::Weight::Normal)
}

/// Medium (500) -- the weight Modrinth sets its whole interface in
/// (`--font-weight-text: 500`), and the default this shell boots with.
pub const fn medium() -> Font {
    inter(iced::font::Weight::Medium)
}

/// Semibold (600).
pub const fn semibold() -> Font {
    inter(iced::font::Weight::Semibold)
}

/// Bold (700), for emphasis on a label that is already medium.
pub const fn bold() -> Font {
    inter(iced::font::Weight::Bold)
}

/// Extrabold (800): `--font-weight-heading` and `--font-weight-title`, so this
/// is what a page title is drawn in.
pub const fn heading() -> Font {
    inter(iced::font::Weight::ExtraBold)
}

/// The application theme: `Theme::custom` derives the whole extended palette
/// (inputs, pick lists, scrollbars, checked boxes) from these five colors, so
/// stock widgets already match the shell without every one of them being
/// restyled by hand.
///
/// Reads the theme in force, so this changes with the choice in Settings:
/// iced re-reads it whenever it repaints, and the shell repaints after the
/// message that changes the theme.
pub fn app_theme() -> Theme {
    Theme::custom(
        "PalantirMC".to_string(),
        IcedPalette {
            background: bg(),
            text: text(),
            primary: accent(),
            success: accent(),
            danger: danger(),
        },
    )
}

// ---- Container styles (plain closures) ---------------------------------

/// Root window background.
pub fn app_bg(_: &Theme) -> container::Appearance {
    container::Appearance { background: Some(bg().into()), ..Default::default() }
}

/// Left icon rail, title bar and right panel: the raised chrome.
///
/// All three are `bg-bg-raised` in the reference, which in its palette is
/// `surface-3` -- the same surface a card is -- and the page sits *inside* that
/// chrome rather than beside it. Read off its own window: `#27292e` across the
/// rail, the bar and the panel, against `#16181c` for the content. Ours was
/// `surface-2`, which read as a slightly lighter page rather than as a frame
/// around one.
pub fn rail(_: &Theme) -> container::Appearance {
    container::Appearance { background: Some(surface().into()), ..Default::default() }
}

/// The page pane, inside the chrome.
///
/// This is the reference's `.app-contents`: the page is a *panel* the chrome
/// wraps, not a strip beside it, and its top-left corner is `--radius-xl` so
/// the rail and the bar meet in a notch. The radius needs the background to be
/// painted here rather than left to the window, because a container's radius
/// only cuts what that container draws -- behind it the chrome would show
/// through the corner instead of the corner being cut out of the pane.
pub fn pane(_: &Theme) -> container::Appearance {
    container::Appearance {
        background: Some(bg().into()),
        border: Border {
            radius: [R_PANE, 0.0, 0.0, 0.0].into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// The right panel: the reference's `.app-sidebar`, painted with
/// `--brand-gradient-bg`.
///
/// A gradient rather than a colour because that is what the token is, and the
/// tiny-skia backend rasterises one in the same pass as a solid fill (`fill_quad`
/// builds a `Shader`, so a linear ramp costs a gradient object per frame, not
/// per pixel). The angle is `PI`, which in iced is a gradient running straight
/// down: `to_distance` subtracts a quarter turn, so `PI` puts stop 0 at the top.
pub fn sidebar(_: &Theme) -> container::Appearance {
    container::Appearance {
        background: Some(Background::Gradient(Gradient::Linear(
            Linear::new(Radians(std::f32::consts::PI))
                .add_stop(0.0, sidebar_top())
                .add_stop(1.0, sidebar_bottom()),
        ))),
        ..Default::default()
    }
}

/// A card inside the right panel: brand-tinted, not the neutral raised grey.
pub fn sidebar_card(_: &Theme) -> container::Appearance {
    container::Appearance {
        background: Some(sidebar_surface().into()),
        border: Border { radius: R_CARD.into(), width: 1.0, color: sidebar_border() },
        ..Default::default()
    }
}

/// One row inside such a card (`bg-button-bg` + `border-button-border` in the
/// reference), used for the "Getting started" steps.
///
/// Where the reference draws a 40px row, the height is set by the caller -- this
/// style is the fill, the hairline border and the 12px corner.
pub fn sidebar_step(_: &Theme) -> container::Appearance {
    container::Appearance {
        background: Some(sidebar_row().into()),
        border: Border { radius: R_BUTTON.into(), width: 1.0, color: sidebar_border() },
        text_color: Some(text()),
        ..Default::default()
    }
}

/// A hairline between the raised chrome and the page.
///
/// A 1px fill rather than `horizontal_rule`, because the rule widget takes its
/// colour from the theme's own rule style while these two lines have a measured
/// value: `#42444a`, the reference's `surface-5`, on the bar's bottom edge and
/// on the rail's right edge. iced paints a container's border on all four edges,
/// so a line that exists on exactly one edge has to be its own widget.
pub fn separator(_: &Theme) -> container::Appearance {
    container::Appearance { background: Some(border_strong().into()), ..Default::default() }
}

/// Raised card with a hairline border.
pub fn card(_: &Theme) -> container::Appearance {
    container::Appearance {
        background: Some(surface().into()),
        border: Border { radius: R_CARD.into(), width: 1.0, color: border() },
        ..Default::default()
    }
}

/// Inset panel: search boxes and modal inner sections.
pub fn inset(_: &Theme) -> container::Appearance {
    container::Appearance {
        background: Some(surface_input().into()),
        border: Border { radius: R_BUTTON.into(), width: 1.0, color: border() },
        ..Default::default()
    }
}

/// The big rounded square behind the welcome logo.
///
/// Flat, like the tooltip and the dialog, and for a reason that was measured
/// rather than assumed: iced's tiny-skia backend renders a `Shadow` by
/// computing a signed-distance field for every pixel of the shadow's bounds and
/// building a fresh premultiplied pixmap from it, with no cache anywhere in the
/// path. A blurred glow on this tile is therefore re-blurred on every frame the
/// page paints -- every frame of a scroll included -- which at this size is
/// tens of thousands of `sqrt` calls and two heap allocations per frame, spent
/// on a halo the logo's own artwork already carries. The accent hairline does
/// the same job for one fill.
pub fn hero_tile(_: &Theme) -> container::Appearance {
    container::Appearance {
        background: Some(palette().hero.into()),
        border: Border { radius: 22.0.into(), width: 1.0, color: alpha(accent(), 0.25) },
        ..Default::default()
    }
}

/// Dimmed backdrop behind a modal (the main area is replaced, so this reads as
/// the launcher receding rather than a separate window).
pub fn backdrop(_: &Theme) -> container::Appearance {
    container::Appearance {
        background: Some(palette().backdrop.into()),
        ..Default::default()
    }
}

/// The modal dialog itself.
///
/// Deliberately a flat surface with a hairline border rather than a blurred
/// shadow. iced's tiny-skia backend renders a `Shadow` as a per-pixel blurred
/// bitmap, so a dialog-sized shadow costs ~400k SDF evaluations *per frame*,
/// and on this backend a large opaque one composites over the dialog's own
/// contents and paints the whole card near-black. The border plus the darker
/// backdrop already separate the dialog from the launcher behind it.
/// The selected row in the Settings section list: a tinted pill, so the current
/// pane is obvious without its label having to shout.
pub fn nav_active(_: &Theme) -> container::Appearance {
    container::Appearance {
        background: Some(alpha(accent(), 0.14).into()),
        border: Border { radius: R_BUTTON.into(), width: 1.0, color: alpha(accent(), 0.32) },
        ..Default::default()
    }
}

/// A section row that is listed but not yet editable: no surface at all, so it
/// reads as a label rather than as a button that does nothing.
pub fn nav_idle(_: &Theme) -> container::Appearance {
    container::Appearance::default()
}

pub fn modal(_: &Theme) -> container::Appearance {
    container::Appearance {
        background: Some(palette().modal.into()),
        border: Border { radius: R_MODAL.into(), width: 1.0, color: border_strong() },
        ..Default::default()
    }
}

/// A dot/pill surface in an arbitrary color (status chips, run indicators).
pub fn pill(color: Color) -> impl Fn(&Theme) -> container::Appearance {
    move |_: &Theme| container::Appearance {
        background: Some(color.into()),
        border: Border { radius: 4.0.into(), ..Default::default() },
        ..Default::default()
    }
}

/// A filled circle of an arbitrary color.
///
/// Distinct from [`pill`] on purpose: a pill is a 4px-radius chip whose size is
/// set by its label, a circle is a fixed dot. The reference's completed-step
/// marker is the latter -- an 18px `rounded-full` disc -- and a pill-shaped one
/// would read as a rounded square at that size.
pub fn circle(color: Color) -> impl Fn(&Theme) -> container::Appearance {
    move |_: &Theme| container::Appearance {
        background: Some(color.into()),
        // A radius past half the box is clamped, so this is a circle at any size
        // without the caller having to know the box.
        border: Border { radius: 999.0.into(), ..Default::default() },
        ..Default::default()
    }
}

/// The empty ring of a checklist step that is not done yet: the reference's
/// `RadioButtonIcon`, which is a 1.5px outline rather than a filled dot.
pub fn step_ring(_: &Theme) -> container::Appearance {
    container::Appearance {
        background: None,
        border: Border { radius: 999.0.into(), width: 1.5, color: text_dim() },
        ..Default::default()
    }
}

/// Small tinted capsule used next to the product name and for status text.
pub fn token_pill(_: &Theme) -> container::Appearance {
    container::Appearance {
        background: Some(alpha(text(), 0.05).into()),
        border: Border { radius: 999.0.into(), width: 1.0, color: alpha(text(), 0.08) },
        text_color: Some(text_muted()),
        ..Default::default()
    }
}

/// Card variant for the selected instance tile.
pub fn card_selected(_: &Theme) -> container::Appearance {
    container::Appearance {
        background: Some(alpha(accent(), 0.08).into()),
        border: Border { radius: R_CARD.into(), width: 1.0, color: alpha(accent(), 0.55) },
        ..Default::default()
    }
}

/// Bottom status strip.
pub fn toast(_: &Theme) -> container::Appearance {
    container::Appearance {
        background: Some(surface().into()),
        border: Border { radius: R_BUTTON.into(), width: 1.0, color: border() },
        text_color: Some(text_muted()),
        ..Default::default()
    }
}

/// Small filled pill used for loader/version metadata next to a name.
pub fn chip(_: &Theme) -> container::Appearance {
    container::Appearance {
        background: Some(alpha(accent(), 0.14).into()),
        border: Border { radius: R_CHIP.into(), width: 1.0, color: alpha(accent(), 0.32) },
        text_color: Some(accent()),
        ..Default::default()
    }
}

/// The reference's `<kbd>`: the key cap in the welcome screen's hint row.
///
/// `inline-flex h-5 min-w-5 items-center justify-center rounded-md border
/// border-solid border-surface-5 bg-button-bg px-1 text-xs font-normal
/// text-primary` — so 20x20 at its smallest, `#34363c` inside a `#42444a` ring,
/// at 12px regular. The capture agrees box for box: the reference's cap measures
/// exactly 20x20 (x 436..455, y 491..510) with that fill and that ring.
///
/// Its label is `text-primary`, which in this design system is
/// `--color-text-default` (`#b0bac5`) — the same token the description above it
/// uses, and the reason a 12px glyph never reaches it: the cap's own "N" peaks
/// at `#848c95` in the capture, which is what antialiasing does to a thin
/// twelve-pixel stem, not a second color.
pub fn keycap(_: &Theme) -> container::Appearance {
    container::Appearance {
        background: Some(surface_input().into()),
        border: Border { radius: R_KEYCAP.into(), width: 1.0, color: border_strong() },
        text_color: Some(text_muted()),
        ..Default::default()
    }
}

/// Neutral variant of [`chip`] (no accent: used for game versions).
pub fn chip_neutral(_: &Theme) -> container::Appearance {
    container::Appearance {
        background: Some(alpha(text(), 0.07).into()),
        border: Border { radius: R_CHIP.into(), width: 1.0, color: border() },
        text_color: Some(text_muted()),
        ..Default::default()
    }
}

/// Clickable settings/type row inside the Create Instance dialog.
pub fn option_row(_: &Theme) -> container::Appearance {
    container::Appearance {
        background: Some(alpha(text(), 0.04).into()),
        border: Border { radius: R_CARD.into(), width: 1.0, color: alpha(text(), 0.06) },
        text_color: Some(text()),
        ..Default::default()
    }
}

/// A solid rounded block of one colour.
///
/// The switch is built from two of these rather than from a canvas: the track
/// and the knob are both plain rounded rectangles, and painting them as
/// containers means the colour that moves is computed by a pure function (see
/// `settings::switch_colors`) instead of inside a draw call. Unlike a canvas
/// this also keeps the widget tree diffable, so a switch that is not moving
/// costs nothing at all.
pub fn plate(color: Color, radius: f32) -> impl Fn(&Theme) -> container::Appearance {
    move |_: &Theme| container::Appearance {
        background: Some(color.into()),
        border: Border { radius: radius.into(), ..Default::default() },
        ..Default::default()
    }
}

/// Square behind an instance icon in a card/grid.
pub fn icon_tile(background: Color) -> impl Fn(&Theme) -> container::Appearance {
    move |_: &Theme| container::Appearance {
        background: Some(background.into()),
        border: Border { radius: 12.0.into(), ..Default::default() },
        ..Default::default()
    }
}

// ---- Tooltip -----------------------------------------------------------

/// The floating label the icon rail shows on hover.
///
/// Deliberately flat and shadowless. This file's note on [`modal`] applies here
/// twice over: iced's tiny-skia backend renders a `Shadow` as a per-pixel
/// blurred bitmap, and a tooltip appears *while the pointer is moving*, which is
/// the worst possible moment to add per-frame blur work. The 1px border carries
/// the separation instead, which is what the reference does too.
#[derive(Debug, Clone, Copy, Default)]
pub struct Tooltip;

impl container::StyleSheet for Tooltip {
    type Style = Theme;

    fn appearance(&self, _: &Theme) -> container::Appearance {
        container::Appearance {
            background: Some(surface_hover().into()),
            border: Border { radius: R_CHIP.into(), width: 1.0, color: border_strong() },
            text_color: Some(text()),
            ..Default::default()
        }
    }
}

impl From<Tooltip> for iced::theme::Container {
    fn from(tooltip: Tooltip) -> Self {
        Self::Custom(Box::new(tooltip))
    }
}

// ---- Button styles -----------------------------------------------------

/// Which of the launcher's button looks to paint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Role {
    /// Filled accent button ("Create an instance", "Play").
    Primary,
    /// Raised grey button (secondary actions).
    Secondary,
    /// Borderless button that only tints on hover (rail, links, window controls).
    Ghost,
    /// Icon-rail entry; `active` paints the accent pill behind it.
    Rail { active: bool },
    /// Selectable pill (loader / loader-version / snapshot switches).
    Chip { active: bool },
    /// The clickable body of an instance card.
    CardArea { selected: bool },
    /// Destructive tinted button.
    Danger,
    /// One color-theme choice in Settings: the whole card is the target, and
    /// the selected one is ringed in the accent.
    ThemeCard { selected: bool },
    /// Title-bar control.
    Window,
    /// Title-bar control whose hover is decided *outside* iced.
    ///
    /// The window's maximize button is a non-client region — Windows has to
    /// own it for Snap Layouts to appear — so iced never sees the pointer over
    /// it and never reports it hovered. The hit test that took the region over
    /// is the only thing that knows, and it passes the answer in here.
    WindowExternallyHovered { hovered: bool },
    /// Title-bar close control (red on hover).
    WindowClose,
    /// One entry in the Settings dialog's section list.
    ///
    /// The reference's `rounded-xl px-4 py-2` row: `--color-button-bg-selected`
    /// behind the open pane, `--color-button-bg` on hover, on a 20px radius.
    NavItem { active: bool },
    /// One tab in a page's own tab strip (Discover's content types).
    ///
    /// Not a [`Role::Chip`], which is a bordered pill: measured off the
    /// reference's strip, the selected tab has no border at all, is filled with
    /// `--color-brand-highlight`, stands 36px tall and keeps a white label in
    /// both states. A chip and a tab looked similar enough to be the same
    /// component that they were one until the capture disagreed.
    Tab { active: bool },
    /// A `size="lg"` button in the brand color: the welcome screen's "Create an
    /// instance".
    BrandLarge,
    /// A `size="lg"` button on the basic surface: the welcome screen's "Import
    /// from launcher".
    BaseLarge,
}

/// A [`button::StyleSheet`] wrapper so call sites can write
/// `.style(theme::primary())`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Btn(pub Role);

/// Filled accent button.
pub fn primary() -> Btn {
    Btn(Role::Primary)
}
/// Raised grey button.
pub fn secondary() -> Btn {
    Btn(Role::Secondary)
}
/// A `size="lg"` brand button.
pub fn brand_large() -> Btn {
    Btn(Role::BrandLarge)
}
/// A `size="lg"` button on the basic surface.
pub fn base_large() -> Btn {
    Btn(Role::BaseLarge)
}
/// Borderless button.
pub fn ghost() -> Btn {
    Btn(Role::Ghost)
}
/// Icon-rail entry.
pub fn rail_button(active: bool) -> Btn {
    Btn(Role::Rail { active })
}
/// Selectable pill.
pub fn chip_button(active: bool) -> Btn {
    Btn(Role::Chip { active })
}
/// One tab in a page's own tab strip.
pub fn tab_button(active: bool) -> Btn {
    Btn(Role::Tab { active })
}
/// The clickable body of an instance card (the card itself is a container).
pub fn card_area(selected: bool) -> Btn {
    Btn(Role::CardArea { selected })
}
/// Destructive tinted button.
///
/// Named `destructive` rather than `danger` because [`danger`] is already the
/// red itself — one name for the color, one for the button that uses it.
pub fn destructive() -> Btn {
    Btn(Role::Danger)
}
/// One color-theme choice in Settings.
pub fn theme_card(selected: bool) -> Btn {
    Btn(Role::ThemeCard { selected })
}

/// One row of the Settings dialog's section list.
pub fn nav_item(active: bool) -> Btn {
    Btn(Role::NavItem { active })
}
/// Title-bar control.
pub fn window_button() -> Btn {
    Btn(Role::Window)
}
/// Title-bar control drawn from a hover state iced did not observe — see
/// [`Role::WindowExternallyHovered`].
pub fn caption_button(hovered: bool) -> Btn {
    Btn(Role::WindowExternallyHovered { hovered })
}
/// Title-bar close control.
pub fn close_button() -> Btn {
    Btn(Role::WindowClose)
}

impl Btn {
    fn appearance(&self, hovered: bool, pressed: bool) -> button::Appearance {
        let base = match self.0 {
            Role::Primary => button::Appearance {
                background: Some(if pressed { accent_dim() } else if hovered { accent_hover() } else { accent() }.into()),
                text_color: on_accent(),
                border: Border { radius: R_BUTTON.into(), ..Default::default() },
                ..Default::default()
            },
            // `ButtonFrame.vue`: `type="colored" color="brand" size="lg"` is
            // `bg-[--button-color] text-[var(--color-accent-contrast)]` on
            // `h-10 rounded-[14px] px-4 gap-2 text-base font-semibold`, with a
            // `::before` that paints a 1px ring of `linear-gradient(180deg,
            // rgba(255,255,255,0.3), rgba(255,255,255,0))` inside its edge.
            //
            // Both ends of that ramp are measured on the reference's own button:
            // its top edge reads `#4ce59e`, which is white at 30% over the fill,
            // and its left edge at mid-height reads `#26e08a`, which is white at
            // 15%. iced draws a border in one color, so the ramp is flattened to
            // the 15% that most of its length is.
            //
            // What is deliberately not drawn is the *outer* `0 0 0 1px
            // color-mix(in srgb, var(--button-color) 30%, transparent)` ring,
            // because the reference as installed does not draw one: the pixel
            // beside the button's left edge is the page, not a third of the
            // brand over it. A stylesheet the app has outgrown is not a spec.
            Role::BrandLarge => button::Appearance {
                background: Some(if pressed { accent_dim() } else if hovered { accent_hover() } else { accent() }.into()),
                text_color: on_accent(),
                border: Border {
                    radius: R_BUTTON_LG.into(),
                    width: 1.0,
                    color: alpha(text(), 0.15),
                },
                ..Default::default()
            },
            // The other half of the pair, `type="base" size="lg"`:
            // `bg-surface-4 text-contrast [&>svg]:text-primary` with an
            // `inset 0 0 0 1px var(--surface-5)` ring. Measured on the
            // reference's Import button, all four of its colors are this
            // palette's: fill `#34363c`, ring `#42444a`, label `#ffffff`, icon
            // `#b0bac5`. Hover is the base class's shared `brightness(1.25)`,
            // the same rule the accent's own hover is derived by, and the press
            // is that same brightness rather than the stylesheet's
            // `active:scale-[0.97]`: iced's button cannot transform, so a press
            // that did nothing would be worse than one that lights up.
            Role::BaseLarge => button::Appearance {
                background: Some(
                    if pressed { lighten(surface_input(), HOVER_BRIGHTNESS).into() }
                    else if hovered { lighten(surface_input(), HOVER_BRIGHTNESS).into() }
                    else { surface_input().into() },
                ),
                text_color: text(),
                border: Border {
                    radius: R_BUTTON_LG.into(),
                    width: 1.0,
                    color: border_strong(),
                },
                ..Default::default()
            },
            Role::Secondary => button::Appearance {
                background: Some(if hovered { surface_hover() } else { surface() }.into()),
                text_color: text(),
                border: Border {
                    radius: R_BUTTON.into(),
                    width: 1.0,
                    color: if hovered { border_strong() } else { border() },
                },
                ..Default::default()
            },
            Role::Ghost => button::Appearance {
                background: if hovered { Some(alpha(text(), 0.08).into()) } else { None },
                text_color: if hovered { text() } else { text_muted() },
                border: Border { radius: R_BUTTON.into(), ..Default::default() },
                ..Default::default()
            },
            Role::Rail { active } => button::Appearance {
                // `--color-brand-highlight` (the accent at 25%) over the chrome,
                // which is what the reference's active entry is; hover is
                // `--color-button-bg`, `surface-4`. The composite was measured
                // rather than assumed: the accent at 25% over `#27292e` is
                // `#1d5540`, and the reference's own plate samples `#1d5540` in
                // its tab strip and `#1d563f` in its rail -- one level apart, on
                // two different surfaces. This was 16% over a 7% white wash
                // before, which read as a different component rather than as the
                // same one lit up.
                background: if active {
                    Some(alpha(accent(), 0.25).into())
                } else if hovered {
                    Some(surface_hover().into())
                } else {
                    None
                },
                text_color: if active { accent() } else if hovered { text() } else { text_dim() },
                border: Border { radius: R_RAIL.into(), ..Default::default() },
                ..Default::default()
            },
            Role::Tab { active } => button::Appearance {
                // The strip's own background is the page, but the fill is not a
                // composite of the page and the accent -- see
                // [`Palette::brand_highlight`] -- so it is the solid token.
                background: if active {
                    Some(brand_highlight().into())
                } else if hovered {
                    Some(alpha(text(), 0.08).into())
                } else {
                    None
                },
                text_color: text(),
                border: Border { radius: R_BUTTON.into(), ..Default::default() },
                ..Default::default()
            },
            Role::Chip { active } => button::Appearance {
                background: if active {
                    Some(alpha(accent(), 0.16).into())
                } else if hovered {
                    Some(alpha(text(), 0.08).into())
                } else {
                    None
                },
                text_color: if active { accent() } else { text_muted() },
                border: Border {
                    radius: R_CHIP.into(),
                    width: 1.0,
                    color: if active { accent() } else { border() },
                },
                ..Default::default()
            },
            Role::CardArea { selected } => button::Appearance {
                background: if selected {
                    Some(alpha(accent(), 0.06).into())
                } else if hovered {
                    Some(alpha(text(), 0.05).into())
                } else {
                    None
                },
                text_color: text(),
                border: Border { radius: R_CARD.into(), ..Default::default() },
                ..Default::default()
            },
            Role::Danger => button::Appearance {
                background: Some(alpha(danger(), if hovered { 0.24 } else { 0.14 }).into()),
                text_color: if hovered { danger_hover() } else { danger() },
                border: Border { radius: R_BUTTON.into(), width: 1.0, color: alpha(danger(), 0.45) },
                ..Default::default()
            },
            Role::ThemeCard { selected } => button::Appearance {
                // The card's own surface, so the label under a light preview sits
                // on light and the one under a dark preview sits on dark — the
                // card looks like the theme it offers.
                background: Some(if hovered { surface_hover() } else { surface() }.into()),
                text_color: if selected { accent() } else { text() },
                border: Border {
                    radius: R_CARD.into(),
                    width: if selected { 2.0 } else { 1.0 },
                    color: if selected { accent() } else { border() },
                },
                ..Default::default()
            },
            Role::Window => button::Appearance {
                background: if hovered { Some(alpha(text(), 0.10).into()) } else { None },
                text_color: if hovered { text() } else { text_dim() },
                border: Border { radius: 6.0.into(), ..Default::default() },
                ..Default::default()
            },
            // Deliberately the same look as `Role::Window`: which of the two
            // decides the hover is the only difference, and the button must
            // not change appearance depending on who noticed the pointer.
            Role::WindowExternallyHovered { hovered } => button::Appearance {
                background: if hovered { Some(alpha(text(), 0.10).into()) } else { None },
                text_color: if hovered { text() } else { text_dim() },
                border: Border { radius: 6.0.into(), ..Default::default() },
                ..Default::default()
            },
            Role::WindowClose => button::Appearance {
                background: if hovered { Some(danger().into()) } else { None },
                text_color: if hovered { Color::WHITE } else { text_dim() },
                border: Border { radius: 6.0.into(), ..Default::default() },
                ..Default::default()
            },
            Role::NavItem { active } => button::Appearance {
                // Dark `--color-button-bg-selected` is `--color-brand-highlight`
                // (the accent at 25%) carrying `--color-brand` as its text, and
                // `--color-button-bg` is `surface-4` carrying `--color-contrast`.
                // The row is `rounded-xl`, which is Omorphia's `--radius-xl`.
                background: if active {
                    Some(alpha(accent(), 0.25).into())
                } else if hovered {
                    Some(surface_hover().into())
                } else {
                    None
                },
                text_color: if active { accent() } else if hovered { text() } else { text_muted() },
                border: Border { radius: R_PANE.into(), ..Default::default() },
                ..Default::default()
            },
        };
        base
    }
}

impl button::StyleSheet for Btn {
    type Style = Theme;

    fn active(&self, _theme: &Theme) -> button::Appearance {
        self.appearance(false, false)
    }

    fn hovered(&self, _theme: &Theme) -> button::Appearance {
        self.appearance(true, false)
    }

    fn pressed(&self, _theme: &Theme) -> button::Appearance {
        self.appearance(true, true)
    }

    fn disabled(&self, _theme: &Theme) -> button::Appearance {
        let mut base = self.appearance(false, false);
        base.background = base.background.map(|background| match background {
            iced::Background::Color(color) => alpha(color, 0.35).into(),
            gradient => gradient,
        });
        if base.background.is_none() {
            // Ghost buttons need *some* surface so the disabled state reads.
            base.background = Some(alpha(text(), 0.04).into());
        }
        base.text_color = alpha(base.text_color, 0.4);
        base
    }
}

impl From<Btn> for iced::theme::Button {
    fn from(btn: Btn) -> Self {
        Self::custom(btn)
    }
}

// ---- Field (text input) ------------------------------------------------

/// Borderless dark text field matching the search boxes and form inputs.
#[derive(Debug, Clone, Copy, Default)]
pub struct Field;

impl text_input::StyleSheet for Field {
    type Style = Theme;

    fn active(&self, _theme: &Theme) -> text_input::Appearance {
        text_input::Appearance {
            background: surface_input().into(),
            border: Border { radius: R_BUTTON.into(), width: 1.0, color: border() },
            icon_color: text_dim(),
        }
    }

    fn focused(&self, _theme: &Theme) -> text_input::Appearance {
        text_input::Appearance {
            background: surface_input().into(),
            border: Border { radius: R_BUTTON.into(), width: 1.0, color: accent() },
            icon_color: accent(),
        }
    }

    fn placeholder_color(&self, _theme: &Theme) -> Color {
        text_dim()
    }

    fn value_color(&self, _theme: &Theme) -> Color {
        text()
    }

    fn disabled_color(&self, _theme: &Theme) -> Color {
        alpha(text(), 0.35)
    }

    fn selection_color(&self, _theme: &Theme) -> Color {
        alpha(accent(), 0.35)
    }

    fn disabled(&self, _theme: &Theme) -> text_input::Appearance {
        text_input::Appearance {
            background: alpha(surface_input(), 0.6).into(),
            border: Border { radius: R_BUTTON.into(), width: 1.0, color: alpha(border(), 0.6) },
            icon_color: alpha(text_dim(), 0.5),
        }
    }
}

impl From<Field> for iced::theme::TextInput {
    fn from(_: Field) -> Self {
        Self::Custom(Box::new(Field))
    }
}

// ---- Bar (progress) ----------------------------------------------------

/// The install progress bar.
///
/// A plain function rather than a unit struct, because `progress_bar` is one of
/// the widgets that take `impl Fn(&Theme) -> Appearance` — the same spelling
/// [`card`] and the rule styles use — so the colours are read at paint time and
/// the bar follows the look Settings switches to without a second stylesheet.
///
/// The track is the inset colour the fields use, so an empty bar reads as part
/// of the surface it sits on; the fill is the accent, the palette's one
/// saturated colour and therefore the one thing on screen that means moving.
pub fn bar(_theme: &Theme) -> progress_bar::Appearance {
    progress_bar::Appearance {
        background: surface_input().into(),
        bar: accent().into(),
        border_radius: R_CHIP.into(),
    }
}

// ---- Thin (scrollable) -------------------------------------------------

/// Slim scrollbar: small, low-contrast and cheap to composite on both wgpu
/// and tiny-skia. This paints the bar only — where the page sits, and how it
/// gets there, belongs to `crate::scroll`, which takes the wheel itself and
/// eases the offset instead of letting iced apply each notch as an instant
/// 60-pixel jump.
#[derive(Debug, Clone, Copy, Default)]
pub struct Thin;

impl Thin {
    fn appearance(&self, mouse_over: bool) -> scrollable::Appearance {
        scrollable::Appearance {
            container: container::Appearance::default(),
            scrollbar: scrollable::Scrollbar {
                background: None,
                border: Border { radius: 6.0.into(), ..Default::default() },
                scroller: scrollable::Scroller {
                    color: if mouse_over { alpha(accent(), 0.62) } else { alpha(text(), 0.24) },
                    border: Border { radius: 6.0.into(), ..Default::default() },
                },
            },
            gap: None,
        }
    }
}

impl scrollable::StyleSheet for Thin {
    type Style = Theme;

    fn active(&self, _theme: &Theme) -> scrollable::Appearance {
        self.appearance(false)
    }

    fn hovered(&self, _theme: &Theme, is_mouse_over_scrollbar: bool) -> scrollable::Appearance {
        self.appearance(is_mouse_over_scrollbar)
    }
}

// ---- Dropdown (pick list) ----------------------------------------------

/// Dark dropdown for the version, build and group pickers.
///
/// The stock `PickList::default()` is the *light* iced theme, so every version
/// selector in the shell used to open a white panel with near-black text in the
/// middle of a dark dialog. This pairs a surface-coloured field with a matching
/// menu so the closed control and its open list agree with the card around
/// them.
#[derive(Debug, Clone, Copy, Default)]
pub struct Dropdown;

impl Dropdown {
    fn field(&self, hovered: bool) -> pick_list::Appearance {
        pick_list::Appearance {
            text_color: text(),
            placeholder_color: text_dim(),
            handle_color: if hovered { text() } else { text_muted() },
            background: if hovered { surface_hover() } else { surface_input() }.into(),
            border: Border {
                radius: R_BUTTON.into(),
                width: 1.0,
                color: if hovered { border_strong() } else { border() },
            },
        }
    }
}

impl pick_list::StyleSheet for Dropdown {
    type Style = Theme;

    fn active(&self, _theme: &Theme) -> pick_list::Appearance {
        self.field(false)
    }

    fn hovered(&self, _theme: &Theme) -> pick_list::Appearance {
        self.field(true)
    }
}

impl menu::StyleSheet for Dropdown {
    type Style = Theme;

    fn appearance(&self, _theme: &Theme) -> menu::Appearance {
        menu::Appearance {
            text_color: text(),
            background: surface_hover().into(),
            border: Border { radius: R_BUTTON.into(), width: 1.0, color: border_strong() },
            selected_text_color: accent(),
            selected_background: alpha(accent(), 0.16).into(),
        }
    }
}

impl From<Dropdown> for iced::theme::PickList {
    fn from(dropdown: Dropdown) -> Self {
        use std::rc::Rc;
        Self::Custom(Rc::new(dropdown), Rc::new(dropdown))
    }
}

// ---- Tick (checkbox) ---------------------------------------------------

/// Square accent checkbox; the stock one is a light-theme rounded box.
#[derive(Debug, Clone, Copy, Default)]
pub struct Tick;

impl Tick {
    fn appearance(&self, checked: bool) -> checkbox::Appearance {
        checkbox::Appearance {
            background: if checked { accent().into() } else { surface_input().into() },
            icon_color: on_accent(),
            border: Border {
                radius: 4.0.into(),
                width: 1.0,
                color: if checked { accent() } else { border_strong() },
            },
            text_color: Some(if checked { text() } else { text_muted() }),
        }
    }
}

impl checkbox::StyleSheet for Tick {
    type Style = Theme;

    fn active(&self, _theme: &Theme, is_checked: bool) -> checkbox::Appearance {
        self.appearance(is_checked)
    }

    fn hovered(&self, _theme: &Theme, is_checked: bool) -> checkbox::Appearance {
        let mut appearance = self.appearance(is_checked);
        appearance.text_color = Some(text());
        if !is_checked {
            appearance.border.color = text_muted();
        }
        appearance
    }
}

impl From<Tick> for iced::theme::Checkbox {
    fn from(tick: Tick) -> Self {
        Self::Custom(Box::new(tick))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Takes [`THEME_TEST_LOCK`], surviving a panic in another test.
    fn theme_lock() -> std::sync::MutexGuard<'static, ()> {
        THEME_TEST_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// sRGB relative luminance, for the contrast checks below.
    fn luminance(color: Color) -> f32 {
        fn channel(value: f32) -> f32 {
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
    }

    /// WCAG contrast ratio between two opaque colors.
    fn contrast(a: Color, b: Color) -> f32 {
        let (high, low) = {
            let (a, b) = (luminance(a), luminance(b));
            if a > b {
                (a, b)
            } else {
                (b, a)
            }
        };
        (high + 0.05) / (low + 0.05)
    }

    #[test]
    fn palette_matches_the_reference_theme() {
        // Asserted against the dark palette directly rather than through the
        // global, so this cannot race a test that switches themes.
        let dark = Palette::dark();
        assert_eq!(
            (dark.bg.r, dark.bg.g, dark.bg.b),
            (0x16 as f32 / 255.0, 0x18 as f32 / 255.0, 0x1C as f32 / 255.0)
        );
        let accent = rgb(0x1B, 0xD9, 0x6A);
        assert!((accent.g - 0.851).abs() < 0.002, "accent green channel: {}", accent.g);
        assert_eq!(alpha(text(), 0.5).a, 0.5);
        assert_eq!(alpha(text(), 0.5).r, text().r);
    }

    #[test]
    fn each_color_theme_paints_its_own_surfaces() {
        let dark = Palette::dark();
        let light = Palette::light();
        let oled = Palette::oled();

        // The chrome of the light look is bright, the dark one is not, and the
        // OLED one is black to the reference's own numbers so those pixels can
        // switch off: `surface-1` is `#000000` and the rail beside it is
        // `#050506`, straight from Omorphia's `.oled-mode`. The rail is not
        // black because it still has to read as a plane beside the page, and
        // that is exactly what the design system does with it.
        assert!(luminance(light.bg) > 0.8, "light chrome should be bright");
        assert!(luminance(dark.bg) < 0.05, "dark chrome should be near-black");
        assert_eq!((oled.bg.r, oled.bg.g, oled.bg.b), (0.0, 0.0, 0.0));
        assert_eq!(
            (oled.bg_rail.r, oled.bg_rail.g, oled.bg_rail.b),
            (0x05 as f32 / 255.0, 0x05 as f32 / 255.0, 0x06 as f32 / 255.0)
        );
        assert!(
            luminance(oled.bg_rail) < luminance(dark.bg_rail),
            "even the OLED rail has to be darker than the ordinary dark one"
        );
        assert!(
            luminance(oled.surface) < luminance(dark.surface),
            "OLED cards should be darker than the ordinary dark ones"
        );

        // Text has to follow its own background: dark-on-light, light-on-dark.
        assert!(luminance(light.text) < luminance(light.bg), "light theme needs dark text");
        assert!(luminance(dark.text) > luminance(dark.bg), "dark theme needs light text");
        for palette in [dark, light, oled] {
            assert!(
                contrast(palette.text, palette.bg) >= 7.0,
                "body text should clear AAA on every theme"
            );
            assert!(
                contrast(palette.accent, palette.surface) >= 3.0,
                "the accent is used for text and must stay readable on a card"
            );
        }
    }

    #[test]
    fn system_theme_follows_the_operating_system() {
        // Only `System` consults the OS; an explicit choice is never overridden.
        assert_eq!(ColorTheme::System.resolve(true), ColorTheme::Light);
        assert_eq!(ColorTheme::System.resolve(false), ColorTheme::Dark);
        assert_eq!(ColorTheme::Dark.resolve(true), ColorTheme::Dark);
        assert_eq!(ColorTheme::Light.resolve(false), ColorTheme::Light);
        assert_eq!(ColorTheme::Oled.resolve(true), ColorTheme::Oled);
    }

    #[test]
    fn theme_ids_round_trip_through_the_settings_file() {
        for theme in ColorTheme::ALL {
            assert_eq!(ColorTheme::from_id(theme.id()), theme, "{} did not round-trip", theme.id());
        }
        // A hand-edited or future file must open, not fail.
        assert_eq!(ColorTheme::from_id("OLED"), ColorTheme::Oled);
        assert_eq!(ColorTheme::from_id("  light "), ColorTheme::Light);
        assert_eq!(ColorTheme::from_id("neon"), ColorTheme::Dark);
        assert_eq!(ColorTheme::from_id(""), ColorTheme::Dark);
        // Every id is distinct, or choosing one would paint another.
        let mut ids: Vec<&str> = ColorTheme::ALL.iter().map(|theme| theme.id()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), ColorTheme::ALL.len());
    }

    #[test]
    fn choosing_a_theme_changes_what_the_styles_paint() {
        let _guard = theme_lock();
        let original = color_theme();

        set_color_theme(ColorTheme::Light);
        assert_eq!(color_theme(), ColorTheme::Light);
        assert_eq!(bg(), Palette::light().bg);
        assert_eq!(app_theme().palette().background, Palette::light().bg);
        assert_eq!(card(&app_theme()).background, Some(Palette::light().surface.into()));

        set_color_theme(ColorTheme::Oled);
        assert_eq!(bg(), Palette::oled().bg);

        set_color_theme(ColorTheme::System);
        set_os_prefers_light(true);
        assert_eq!(bg(), Palette::light().bg, "System should follow a light OS");
        set_os_prefers_light(false);
        assert_eq!(bg(), Palette::dark().bg, "System should follow a dark OS");

        // Leave the process as it was found: every other test paints with it.
        set_os_prefers_light(false);
        set_color_theme(original);
    }

    #[test]
    fn theme_is_custom_and_named() {
        let theme = app_theme();
        assert!(format!("{theme:?}").contains("PalantirMC"));
        let palette = theme.palette();
        assert_eq!(palette.primary, accent());
        assert_eq!(palette.background, bg());
    }

    #[test]
    fn button_roles_paint_distinct_primary_and_danger() {
        let theme = app_theme();
        let resting = button::StyleSheet::active(&primary(), &theme);
        let destructive = button::StyleSheet::active(&destructive(), &theme);
        let plain = button::StyleSheet::active(&ghost(), &theme);
        assert_eq!(resting.text_color, on_accent());
        assert_ne!(resting.background, destructive.background);
        assert!(plain.background.is_none());
        // Hovered primary is brighter than the resting one.
        let hovered = button::StyleSheet::hovered(&primary(), &theme);
        match (hovered.background, resting.background) {
            (Some(iced::Background::Color(h)), Some(iced::Background::Color(r))) => {
                assert!(h.g > r.g);
            }
            other => panic!("expected color backgrounds, got {other:?}"),
        }
    }

    #[test]
    fn rail_and_chip_roles_track_active_state() {
        let theme = app_theme();
        let active = button::StyleSheet::active(&rail_button(true), &theme);
        let idle = button::StyleSheet::active(&rail_button(false), &theme);
        assert_eq!(active.text_color, accent());
        assert_eq!(idle.text_color, text_dim());
        assert!(idle.background.is_none());
        assert!(active.background.is_some());

        let chip_on = button::StyleSheet::active(&chip_button(true), &theme);
        let chip_off = button::StyleSheet::active(&chip_button(false), &theme);
        assert_ne!(chip_on.border.color, chip_off.border.color);

        // A tab is not a chip: the selected one is a solid fill with no border,
        // and its label stays the page's own text colour in both states.
        let tab_on = button::StyleSheet::active(&tab_button(true), &theme);
        let tab_off = button::StyleSheet::active(&tab_button(false), &theme);
        assert_ne!(tab_on.background, tab_off.background);
        assert_eq!(tab_on.border.width, 0.0);
        assert_eq!(tab_on.text_color, tab_off.text_color);
    }

    /// The welcome screen's two buttons and its key cap.
    ///
    /// The values are the point, not the shapes: a 40px-tall brand button on a
    /// 14px radius with black ink on it, and its neighbour on `surface-4` inside a
    /// `surface-5` ring. Each of those was measurable only against a capture
    /// (`REFERENCE.md`, `.scratch/ref-01-home.png`), and each was different before
    /// this port -- a 12px radius, and no ring at all -- in a way that a test on
    /// *structure* cannot see.
    #[test]
    fn the_welcome_buttons_and_key_cap_are_the_measured_ones() {
        let theme = app_theme();

        let brand = button::StyleSheet::active(&brand_large(), &theme);
        assert_eq!(brand.background, Some(accent().into()));
        assert_eq!(brand.text_color, on_accent());
        // Black, from `--color-accent-contrast` in dark -- and the reason the
        // plus in the button is black too: `[&>svg]:text-inherit`.
        assert_eq!(on_accent(), rgb(0, 0, 0));
        assert_eq!(R_BUTTON_LG, 14.0);
        assert_eq!(brand.border.radius, R_BUTTON_LG.into());
        // The stylesheet's 1px `::before` ramp, flattened to the 15% the capture
        // measures over most of the button's edge.
        assert_eq!(brand.border.color, alpha(rgb(0xFF, 0xFF, 0xFF), 0.15));

        let base = button::StyleSheet::active(&base_large(), &theme);
        assert_eq!(base.text_color, text());
        assert_eq!(base.border.radius, R_BUTTON_LG.into());
        // The reference's own Import button measures `#34363c` inside a `#42444a`
        // ring, and both are this palette's tokens rather than near-misses.
        assert_eq!(surface_input(), rgb(0x34, 0x36, 0x3C));
        assert_eq!(border_strong(), rgb(0x42, 0x44, 0x4A));
        assert_eq!(base.background, Some(surface_input().into()));
        assert_eq!(base.border.color, border_strong());

        // The key cap: `rounded-md` is Tailwind's 6, not Omorphia's `--radius-md`
        // 12, and reading the wrong scale is how it becomes as round as a button.
        let cap = keycap(&theme);
        assert_eq!(R_KEYCAP, 6.0);
        assert_eq!(cap.border.radius, R_KEYCAP.into());
        assert_eq!(cap.background, Some(surface_input().into()));
        assert_eq!(cap.border.color, border_strong());
    }

    /// The tokens whose value is a *measurement* rather than a transcription.
    ///
    /// Each of these disagreed with the project's stylesheet ladder and was
    /// settled by sampling the running app (`REFERENCE.md`). They are pinned
    /// here because a value that is quietly re-derived from the sheet is wrong
    /// in a way no other test can see: the shell still paints, the numbers still
    /// look plausible, and the window is a different colour than the one it was
    /// measured against.
    #[test]
    fn measured_tokens_keep_their_measured_values() {
        let palette = Palette::dark();
        assert_eq!(palette.accent, rgb(0x00, 0xDA, 0x75), "the accent is the app's own paint");
        assert_eq!(
            palette.brand_highlight,
            rgb(0x1D, 0x55, 0x40),
            "the active plate's fill"
        );
        assert_eq!(R_RAIL, 12.0, "the plate is a rounded square, not a circle");
        // The composite the highlight is a solid of: the accent at 25% over the
        // chrome. If the accent moves, this has to move with it.
        let a = palette.accent;
        let chrome = palette.surface;
        let mixed = Color {
            r: a.r * 0.25 + chrome.r * 0.75,
            g: a.g * 0.25 + chrome.g * 0.75,
            b: a.b * 0.25 + chrome.b * 0.75,
            a: 1.0,
        };
        let expect = palette.brand_highlight;
        for (got, want) in [(mixed.r, expect.r), (mixed.g, expect.g), (mixed.b, expect.b)] {
            assert!((got - want).abs() <= 0.01, "accent@25% over chrome should be the highlight");
        }
    }

    #[test]
    fn disabled_buttons_are_dimmed() {
        let theme = app_theme();
        let disabled = button::StyleSheet::disabled(&primary(), &theme);
        assert!(disabled.text_color.a < 1.0);
        let ghost_disabled = button::StyleSheet::disabled(&ghost(), &theme);
        assert!(ghost_disabled.background.is_some(), "ghost buttons need a disabled surface");
    }

    #[test]
    fn card_and_pill_styles_track_selection() {
        let theme = app_theme();
        let idle = button::StyleSheet::active(&card_area(false), &theme);
        let chosen = button::StyleSheet::active(&card_area(true), &theme);
        assert!(idle.background.is_none());
        assert!(chosen.background.is_some());
        assert_ne!(card_selected(&theme).border.color, card(&theme).border.color);
        assert_eq!(pill(accent())(&theme).background, Some(accent().into()));
        assert!(token_pill(&theme).background.is_some());
    }

    #[test]
    fn field_converts_into_the_input_style() {
        let style: iced::theme::TextInput = Field.into();
        assert!(matches!(style, iced::theme::TextInput::Custom(_)));
    }

    /// The progress bar is painted from the palette rather than being a second
    /// stylesheet: an inset track so an empty bar reads as part of the surface,
    /// and an accent fill, which is the one colour in this look that means
    /// moving.
    #[test]
    fn the_progress_bar_is_painted_from_the_palette() {
        let _guard = theme_lock();
        let theme = app_theme();
        let appearance = bar(&theme);
        assert_eq!(appearance.background, surface_input().into());
        assert_eq!(appearance.bar, accent().into());
        assert_ne!(
            appearance.background, appearance.bar,
            "a track the fill is invisible against is a bar that never moves"
        );
    }

    #[test]
    fn containers_and_fields_are_themed() {
        let theme = app_theme();
        assert_eq!(card(&theme).border.radius, R_CARD.into());
        assert_eq!(inset(&theme).background, Some(surface_input().into()));
        assert_eq!(chip(&theme).text_color, Some(accent()));
        assert_eq!(text_input::StyleSheet::placeholder_color(&Field, &theme), text_dim());
        assert_eq!(text_input::StyleSheet::focused(&Field, &theme).border.color, accent());
        assert_eq!(text_input::StyleSheet::active(&Field, &theme).border.color, border());
        let idle = scrollable::StyleSheet::active(&Thin, &theme);
        let hovering = scrollable::StyleSheet::hovered(&Thin, &theme, true);
        assert!(hovering.scrollbar.scroller.color.a > idle.scrollbar.scroller.color.a);
        assert!(backdrop(&theme).background.is_some());
        // The version and group dropdowns must not fall back to the stock
        // light theme: closed field dark, open menu dark, accent on the
        // selected row.
        let field = pick_list::StyleSheet::active(&Dropdown, &theme);
        assert_eq!(field.text_color, text());
        assert_eq!(field.background, surface_input().into());
        assert_eq!(pick_list::StyleSheet::hovered(&Dropdown, &theme).background, surface_hover().into());
        let open = menu::StyleSheet::appearance(&Dropdown, &theme);
        assert_eq!(open.background, surface_hover().into());
        assert_eq!(open.selected_text_color, accent());
        assert_eq!(open.text_color, text());
        // The snapshots toggle and the mod switches share the accent square.
        let off = checkbox::StyleSheet::active(&Tick, &theme, false);
        let on = checkbox::StyleSheet::active(&Tick, &theme, true);
        assert_eq!(on.background, accent().into());
        assert_ne!(off.background, on.background);
        assert!(on.text_color.expect("checked boxes need a label colour").r > off.text_color.expect("unchecked boxes need a label colour").r);
        // The dialog is a flat bordered surface: a blurred shadow of this size
        // is both expensive and, on tiny-skia, destructive to the card's own
        // contents (see `modal`).
        let dialog = modal(&theme);
        assert_eq!(dialog.shadow.blur_radius, 0.0);
        assert!(dialog.background.is_some());
        assert_eq!(dialog.border.width, 1.0);
    }
}
