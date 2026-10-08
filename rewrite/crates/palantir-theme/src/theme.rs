//! The theme: a matte black window with one orange accent.
//!
//! One file holds the whole visual language. The **roles are fixed** — a token
//! means one thing everywhere — and the **values are editable** — a palette is
//! for tuning. Every value below carries the reason it is that value, and no
//! reason is "because another application does it": these are our choices,
//! designed dark-first for v2.0 (light mode is out of scope).
//!
//! Three rules hold the set together:
//!
//! - **One accent.** Orange appears on the primary action of a surface, the
//!   selected navigation state, focus rings, and a progress fill. Nowhere else.
//! - **Matte means no pure black and no pure white**, so text has headroom and
//!   the window does not glare on OLED.
//! - **Elevation is luminance, not shadow.** The window is the floor of the
//!   ramp and every step up is a lighter fill — raised surfaces, then wells and
//!   control plates, then the hairline that separates them. The only shadow in
//!   the product is the one under a modal overlay.

use core::fmt;

/// A colour as four 8-bit sRGB channels.
///
/// Bytes rather than floats: a token is a fact, exact and comparable, while a
/// float invites rounding drift between the palette and what paints.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// An opaque colour from its RGB channels — the form every token takes.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 0xFF }
    }

    /// The `0xRRGGBB` form, for comparing a token against the palette table.
    pub const fn as_u32(self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

    /// sRGB channels scaled to `0.0..=1.0`, the form the GPU APIs want.
    pub fn to_rgba_f32(self) -> [f32; 4] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            self.a as f32 / 255.0,
        ]
    }
}

impl fmt::Display for Color {
    /// `#rrggbb`, the form the palette table is written in.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:06X}", self.as_u32())
    }
}

// ---------------------------------------------------------------------------
// Palette
// ---------------------------------------------------------------------------

/// The window and the page pane. Matte, deliberately not `#000000`: a true
/// black leaves raised surfaces nowhere to go and makes edges vanish on OLED.
pub const BG: Color = Color::rgb(0x0B, 0x0B, 0x0C);

/// The rail, the title bar, cards — anything raised one step above the window.
pub const BG_RAISED: Color = Color::rgb(0x13, 0x13, 0x15);

/// Text fields, search boxes, a hovered tab's plate: wells, one step above the
/// raised surfaces so an input reads as a place to type before it is focused.
pub const BG_INSET: Color = Color::rgb(0x1A, 0x1A, 0x1D);

/// Hairlines, field outlines, the rule under the title bar. Lighter than the
/// raised surfaces it separates so a 1 px edge survives on OLED.
pub const BORDER: Color = Color::rgb(0x26, 0x26, 0x2A);

/// Primary text and headings. Off-white, never `#FFFFFF`: full white on matte
/// black glares and clips the weight ladder's finest stems.
pub const TEXT: Color = Color::rgb(0xF3, 0xF3, 0xF5);

/// Secondary text, idle icons, placeholders — one clear step down from
/// [`TEXT`] and still comfortably above the 4.5:1 body minimum on [`BG`].
pub const TEXT_MUTED: Color = Color::rgb(0x9A, 0x9A, 0xA0);

/// The brand. On: the primary action of a surface, the selected rail entry or
/// tab, focus rings, progress fills. Nowhere else — see the crate docs.
pub const ACCENT: Color = Color::rgb(0xFF, 0x7A, 0x1A);

/// Hover on anything accent-coloured. A single step up the same hue.
pub const ACCENT_HOVER: Color = Color::rgb(0xFF, 0x8F, 0x3D);

/// The plate behind a selected rail entry or tab: the accent's hue at a low
/// luminance, so selection reads without a second accent appearing.
pub const ACCENT_PLATE: Color = Color::rgb(0x3A, 0x1F, 0x0C);

/// The label *on* an accent fill — the window's matte black, not white. Dark
/// text on `#FF7A1A` is the higher-contrast pairing (≈9:1 vs ≈2.2:1).
pub const ON_ACCENT: Color = Color::rgb(0x0B, 0x0B, 0x0C);

/// Success states and toasts. A green at mid luminance so a confirmation reads
/// without shouting, a third of the wheel from [`ACCENT`].
pub const SUCCESS: Color = Color::rgb(0x4F, 0xA6, 0x5E);

/// Warnings. A gold, not an amber: the accent already owns the orange corner
/// of the wheel, and a warning must never read as the brand.
pub const WARN: Color = Color::rgb(0xD6, 0xBC, 0x3F);

/// Destructive actions and failures. A red held darker than [`ACCENT`] so a
/// delete button can never be mistaken for the primary action at a glance.
pub const DANGER: Color = Color::rgb(0xD6, 0x45, 0x45);

// ---------------------------------------------------------------------------
// Type
// ---------------------------------------------------------------------------

/// The one family shipped: Inter, SIL Open Font Licence 1.1, five weights.
pub const FONT_FAMILY: &str = "Inter";

/// Body copy — the default size of the product.
pub const FONT_BODY: u16 = 14;
/// Secondary text and rail labels: one step off body, so a label reads as
/// supporting without being dimmed to do it.
pub const FONT_SMALL: u16 = 13;
/// Timestamps, versions, hashes — the smallest size drawn; smaller stops being
/// readable at 100% scale.
pub const FONT_CAPTION: u16 = 12;
/// Card titles and section labels.
pub const FONT_SUBTITLE: u16 = 16;
/// Page titles.
pub const FONT_TITLE: u16 = 20;
/// Empty-state headlines and About: the loudest the type gets.
pub const FONT_DISPLAY: u16 = 24;

/// Body text.
pub const WEIGHT_REGULAR: u16 = 400;
/// Buttons, tabs, toggles — controls read as actionable.
pub const WEIGHT_MEDIUM: u16 = 500;
/// Headings and titles.
pub const WEIGHT_SEMIBOLD: u16 = 600;

// ---------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------

/// Hairline width: one device-independent pixel, everywhere.
pub const BORDER_WIDTH_PX: u16 = 1;

/// Fields and compact controls.
pub const RADIUS_SM: u16 = 4;
/// Cards, dialogs, page-level buttons.
pub const RADIUS_MD: u16 = 8;
/// Chips and badges (effectively a capsule).
pub const RADIUS_PILL: u16 = 999;

/// Chips, tab strips, toolbars.
pub const HEIGHT_COMPACT: u16 = 28;
/// Buttons and fields — the default control height.
pub const HEIGHT_DEFAULT: u16 = 32;
/// The one primary button of a page.
pub const HEIGHT_PRIMARY: u16 = 40;

/// Inside a control (chip padding).
pub const SPACE_XS: u16 = 4;
/// Field and button padding.
pub const SPACE_SM: u16 = 8;
/// Between siblings (label to field, icon to text).
pub const SPACE_MD: u16 = 12;
/// Card padding.
pub const SPACE_LG: u16 = 16;
/// Between groups in a pane.
pub const SPACE_XL: u16 = 24;
/// Page margins.
pub const SPACE_2XL: u16 = 32;
/// Between a page head and its content; the largest step.
pub const SPACE_3XL: u16 = 40;

// ---------------------------------------------------------------------------
// Motion
// ---------------------------------------------------------------------------

/// Hover fill, focus ring, switch knob, modal arrival: 150 ms, ease-out.
pub const MOTION_MS: u32 = 150;
/// The ceiling. Nothing in the product animates longer than this.
pub const MOTION_MAX_MS: u32 = 250;
/// Hover brightens the whole control (never per-part).
pub const HOVER_BRIGHTEN: f32 = 1.1;
/// Press dims the whole control.
pub const PRESS_DIM: f32 = 0.9;

#[cfg(test)]
mod tests {
    use super::*;

    /// The palette table, restated as numbers so an edit that drifts from the
    /// written design fails here rather than in a screenshot review.
    #[test]
    fn palette_matches_the_written_table() {
        assert_eq!(BG.as_u32(), 0x0B_0B_0C);
        assert_eq!(BG_RAISED.as_u32(), 0x13_13_15);
        assert_eq!(BG_INSET.as_u32(), 0x1A_1A_1D);
        assert_eq!(BORDER.as_u32(), 0x26_26_2A);
        assert_eq!(TEXT.as_u32(), 0xF3_F3_F5);
        assert_eq!(TEXT_MUTED.as_u32(), 0x9A_9A_A0);
        assert_eq!(ACCENT.as_u32(), 0xFF_7A_1A);
        assert_eq!(ACCENT_HOVER.as_u32(), 0xFF_8F_3D);
        assert_eq!(ACCENT_PLATE.as_u32(), 0x3A_1F_0C);
        assert_eq!(ON_ACCENT.as_u32(), 0x0B_0B_0C);
    }

    /// "Matte means no pure black and no pure white" — checked over the whole
    /// palette, not just the two tokens the rule names.
    #[test]
    fn matte_leaves_headroom() {
        let all = [
            BG,
            BG_RAISED,
            BG_INSET,
            BORDER,
            TEXT,
            TEXT_MUTED,
            ACCENT,
            ACCENT_HOVER,
            ACCENT_PLATE,
            ON_ACCENT,
            SUCCESS,
            WARN,
            DANGER,
        ];
        for c in all {
            assert_ne!(c.as_u32(), 0x00_00_00, "{c} is pure black");
            assert_ne!(c.as_u32(), 0xFF_FF_FF, "{c} is pure white");
        }
    }

    /// The label on an accent fill is the window's matte black by design, so
    /// the two tokens must never drift apart.
    #[test]
    fn on_accent_is_the_window_black() {
        assert_eq!(ON_ACCENT, BG);
    }

    /// Elevation is luminance: one ascending ramp from the window to the
    /// hairline — window, raised surfaces, wells, border. (The rewrite spec's
    /// prose once said "insets are darker"; its own value table and named uses
    /// say the opposite, and `NOTES.md` records why the table wins.)
    #[test]
    fn elevation_is_a_luminance_ramp() {
        fn luminance(c: Color) -> u32 {
            // A cheap relative luminance stand-in: channel sum. The rule only
            // orders tokens, so no colour-space conversion is needed.
            u32::from(c.r) + u32::from(c.g) + u32::from(c.b)
        }
        assert!(luminance(BG) < luminance(BG_RAISED));
        assert!(luminance(BG_RAISED) < luminance(BG_INSET));
        assert!(luminance(BG_INSET) < luminance(BORDER));
    }

    #[test]
    fn motion_respects_the_ceiling() {
        // The comparisons go through arrays on purpose: these tests exist to
        // catch a future *edit* to the constants, and indexing keeps the
        // comparison a runtime one instead of a folded constant that warns.
        let durations = [MOTION_MS, MOTION_MAX_MS];
        assert!(durations[0] <= durations[1], "motion exceeds its ceiling");
        let brightness = [PRESS_DIM, 1.0, HOVER_BRIGHTEN];
        assert!(
            brightness[0] < brightness[1] && brightness[1] < brightness[2],
            "press must dim and hover must brighten"
        );
    }

    /// A ladder is an ordered list, so the tests are written as one: each
    /// step must be a step, or the list is an unordered grab bag.
    #[test]
    fn ladders_ascend() {
        let type_scale = [
            FONT_CAPTION,
            FONT_SMALL,
            FONT_BODY,
            FONT_SUBTITLE,
            FONT_TITLE,
            FONT_DISPLAY,
        ];
        let radii = [RADIUS_SM, RADIUS_MD, RADIUS_PILL];
        let heights = [HEIGHT_COMPACT, HEIGHT_DEFAULT, HEIGHT_PRIMARY];
        let spacing = [
            SPACE_XS, SPACE_SM, SPACE_MD, SPACE_LG, SPACE_XL, SPACE_2XL, SPACE_3XL,
        ];
        for ladder in [
            type_scale.as_slice(),
            radii.as_slice(),
            heights.as_slice(),
            spacing.as_slice(),
        ] {
            for pair in ladder.windows(2) {
                assert!(pair[0] < pair[1], "ladder out of order: {ladder:?}");
            }
        }
    }

    #[test]
    fn colors_display_as_palette_hex() {
        assert_eq!(ACCENT.to_string(), "#FF7A1A");
        assert_eq!(BG.to_string(), "#0B0B0C");
    }

    #[test]
    fn gpu_channels_are_scaled() {
        let c = BG.to_rgba_f32();
        assert_eq!(c[3], 1.0);
        assert!((c[0] - 11.0 / 255.0).abs() < 1e-6);
    }
}
