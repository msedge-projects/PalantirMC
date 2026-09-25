//! The vocabulary the interface paints with: which token an ink is, and which
//! face a piece of text is set in.
//!
//! These lived in `shell.rs` while the shell was the only thing drawing. They are
//! here now because the pages need exactly the same names, and two copies of "the
//! token behind `text-primary`" is how a port ends up with two different greys.
//!
//! **The names are Tailwind classes, not tokens**, and that distinction has bitten
//! this port once already. `@modrinth/ui`'s preset remaps Tailwind's names onto
//! different tokens:
//!
//! ```text
//! primary:   var(--color-text-default)      -- not --color-text-primary
//! contrast:  var(--color-text-primary)
//! secondary: var(--color-text-tertiary)
//! ```
//!
//! so a port that reads the class name and reaches for the token with the same
//! word draws the wrong ink. Every constant below is named after the class the
//! reference writes, and says which token it resolves to.

use crate::theme_gen::{self, Ink, Theme as Gen};
use iced::{Color, Font};

/// The token behind the reference's `text-primary` class.
///
/// Not `--color-text-primary`: the preset maps `primary` to
/// `--color-text-default`. See the module docs.
pub const INK_DEFAULT: Ink = Ink::TextDefault;
/// The token behind `text-contrast`, which *is* `--color-text-primary`.
pub const INK_CONTRAST: Ink = Ink::TextPrimary;
/// The token behind `text-secondary`, which is `--color-text-tertiary`.
pub const INK_SECONDARY: Ink = Ink::TextTertiary;
/// The token behind the rail button's hover background, `hover:bg-button-bg`.
pub const INK_HOVER_BG: Ink = Ink::ButtonBg;
/// The rail's selection plate, `--color-button-bg-selected`.
pub const INK_PLATE: Ink = Ink::ButtonBgSelected;
/// The rail's selected icon, `--color-button-text-selected`.
pub const INK_PLATE_TEXT: Ink = Ink::ButtonTextSelected;

/// The family the entry point loads: five weights of Inter, from
/// `crates/palantir-desktop/assets/fonts`, which is the family the reference's
/// stylesheet pins.
pub const FAMILY: &str = "Inter";

/// A face of the interface's one family.
pub const fn inter(weight: iced::font::Weight) -> Font {
    Font { family: iced::font::Family::Name(FAMILY), weight, ..Font::DEFAULT }
}

/// `--font-weight-text: 500`, which is what every label in the reference
/// inherits: body text is medium, not regular.
pub const fn medium() -> Font {
    inter(iced::font::Weight::Medium)
}

/// `font-semibold`, which the reference uses for a label inside a control.
pub const fn semibold() -> Font {
    // iced spells the CSS `600` weight `Semibold`; `font-semibold` is what the
    // reference puts on a label inside a control.
    inter(iced::font::Weight::Semibold)
}

/// A heading. The reference draws its headings at 800 (`font-extrabold` on a
/// button's label, `--font-weight-heading` on a title), which is heavier than the
/// 600 the old shell used.
pub const fn heading() -> Font {
    inter(iced::font::Weight::ExtraBold)
}

/// How much darker the ink of a disabled control is, from the reference's
/// `opacity-20` on a disabled history chevron.
pub const DISABLED_OPACITY: f32 = 0.2;

/// The ink for a control that cannot be used.
///
/// A colour rather than a flag: a disabled control is the same paint with a
/// fainter ink, which is what `opacity-20` on a *child* icon means in the
/// reference.
pub fn disabled(theme: Gen, ink: Ink) -> Color {
    let color = theme_gen::ink(theme, ink);
    Color { a: color.a * DISABLED_OPACITY, ..color }
}
