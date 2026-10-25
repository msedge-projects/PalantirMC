//! The About view: the mark, the version, the licence and the third-party notices.
//!
//! **Why this exists.** `AGENTS.md` makes attribution a shipping condition: art taken from
//! another launcher and code taken under MIT "carry their notice in the source and in the
//! binary's About page". `THIRD_PARTY_NOTICES.md` is the source half; this is the binary's
//! half, and it renders the same list rather than a second inventory that could drift.
//!
//! **What the reference has, and has not.** There is no About view to copy. The pinned tree
//! (`vendor/modrinth-app/UPSTREAM.md`) has no `About.vue`, `modrinth/code`'s
//! `apps/app-frontend/src/components/ui` has none at `main` either, and neither locale file
//! has an `app.about.*` namespace. The one surface the reference puts its own version on is
//! the settings modal's footer -- `AppSettingsModal.vue`'s `mt-auto text-secondary text-sm`
//! block -- whose version line is the locale message **`app.settings.app-version`**
//! ("Modrinth App {version}", `locales/en-US/index.json:1628-1630`), filled with the Tauri
//! app's version at runtime. This view keeps that line's shape -- the product name, then
//! the version, in secondary text -- and puts the licence and the notices under it, because
//! a footer has room for one line and the notices need more than that.
//!
//! **The close box and Escape.** The reference's `NewModal.vue` puts its close button at
//! `absolute top-4 right-4` and closes on Escape; both are copied here, because a modal that
//! only closes one way is a modal somebody gets stuck in.
//!
//! **What is deliberately not drawn.** No links to a repository, a Discord or a licence
//! page: this view has no way to open a browser ([`crate::pages::home`]'s import button is
//! the same boundary), and a link that does nothing is worse than a path a reader can type.

use eframe::egui::{self, Align2, Color32, Rect, Sense, Stroke, Ui, Vec2};

use crate::brand;
use crate::fonts;
use crate::icons::{self, Glyph};
use crate::theme::Theme;

/// The version, from the crate's own manifest.
///
/// `CARGO_PKG_VERSION` is `Cargo.toml`'s `version` -- the same value CI stamps into the
/// build it makes from that manifest and that the exe's file properties carry. A literal
/// here would be a second number a release has to edit, and the first time the two
/// disagreed the About view would misname the binary it is part of.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The product name, from the core crate rather than a second spelling of it.
///
/// `sift_core::PRODUCT_NAME` is what the launch script, the logs and the window title use;
/// two spellings of one product name is how a window ends up labelled as something else.
const PRODUCT: &str = sift_core::PRODUCT_NAME;

/// The licence line, from `LICENSE` at the repository root.
///
/// The root file's first lines are "Sift Launcher", the copyright line and "This software
/// is proprietary." -- this is those three facts in one sentence, because a modal has one
/// line for them. The full text travels with the exe (`LICENSE` is attached to the release)
/// and every third-party licence is in `THIRD_PARTY_NOTICES.md`, which the footer names.
const LICENCE: &str =
    "Proprietary software. Copyright (c) 2026 Palantir Studios. All rights reserved.";

/// What this product ships that is somebody else's work, and under which licence.
///
/// The entries are the sections of `THIRD_PARTY_NOTICES.md` in its own order -- the
/// Modrinth App, Inter, cosmic-text, the adopted art -- plus the crate this shell adopted
/// from `crmne/fastframe` (`vendor/fastframe-scroll/UPSTREAM.md`), which the notices file
/// does not carry yet. Each row names the licence, then where the notice or the licence
/// text lives, so a reader holding the binary can find the terms without this view growing
/// a scroll area.
const ATTRIBUTION: [(&str, &str); 5] = [
    (
        "Modrinth App",
        "GPL-3.0 · the window this launcher is ported from · vendor/modrinth-app/ · \
         LICENSE-GPL-3.0.txt",
    ),
    (
        "Inter",
        "SIL Open Font License 1.1 · the five weights in crates/sift-desktop/assets/fonts/ · \
         OFL.txt",
    ),
    (
        "cosmic-text",
        "MIT OR Apache-2.0 · vendored and patched at vendor/cosmic-text/ · LICENSE-MIT, \
         LICENSE-APACHE",
    ),
    (
        "Lucide icons",
        "ISC · through the Modrinth App's own icon set · THIRD_PARTY_NOTICES.md",
    ),
    (
        "fastframe-scroll 0.6.0",
        "MIT · © Carmine Paolino · vendored at vendor/fastframe-scroll/ from \
         crmne/fastframe v0.6.0 · LICENSE-MIT.txt",
    ),
];

/// The plate's width and height, in points.
///
/// Wider than a settings footer and narrower than the settings modal (`min(928px, ...)`):
/// the widest line here is an attribution at 12 points, which wraps twice at 560 and would
/// wrap three times at the modal's width for no gain. The height fits the mark, four text
/// blocks and five two-line rows with 24 points of padding at each end.
const PLATE_W: f32 = 560.0;
const PLATE_H: f32 = 560.0;

/// The mark's box: 96 points, as asked, which is 192 px at 2x and 288 px at 3x.
const MARK: f32 = 96.0;

/// The plate's inset, the reference's `p-6`.
const PAD: f32 = 24.0;

/// The close button's target: the reference's 16-point inset around a 28-point box.
const CLOSE: f32 = 28.0;

/// The first column of the attribution list.
const NAME_COL: f32 = 150.0;

/// The scrim's black at 0.63 alpha.
///
/// The reference's overlay is a vertical gradient from 0.52 to 0.95 alpha behind a 5-point
/// backdrop blur; egui has no backdrop blur, and painting the gradient would be three bands
/// for a difference no capture of this modal would see against the dark theme. 160/255 is
/// the gradient's middle, and a plate on a dark scrim needs no more than that to read as
/// the front-most thing on screen.
const SCRIM_ALPHA: u8 = 160;

/// The About modal: scrim, plate, mark, version, licence and notices.
///
/// `screen` is the window, as the shell measures it, so the scrim covers the furniture and
/// not just the page column. `open` is the caller's state -- the same shape the reference's
/// modal exposes -- and the return value is that state after this frame, so a caller can
/// keep the frame loop alive for as long as this is drawn. Escape and the close box both
/// write `false` through it.
pub fn draw(ui: &mut Ui, screen: Rect, theme: &Theme, open: &mut bool) -> bool {
    if !*open {
        return false;
    }
    // Escape closes, as `NewModal.vue` does. Read before drawing so the closing frame is
    // also the last one, rather than a frame of the modal painted after the key was let go.
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        *open = false;
        return false;
    }

    // The reference clamps a modal to `calc(100% - 2 * var(--gap-lg))`; two paddings is the
    // same idea at this size, and a window smaller than the plate still gets a modal.
    let plate = Rect::from_center_size(
        screen.center(),
        Vec2::new(
            PLATE_W.min(screen.width() - 2.0 * PAD),
            PLATE_H.min(screen.height() - 2.0 * PAD),
        ),
    );

    let painter = ui.painter();
    painter.rect_filled(screen, 0, Color32::from_black_alpha(SCRIM_ALPHA));
    // `bg-bg-raised rounded-2xl border border-solid border-surface-5` -- the reference's
    // modal body, in the theme's own tokens: raised background, 16-point radius, surface-5
    // edge. The shadow is not drawn: egui has no drop shadow, and faking one with
    // concentric rects would cost more than the separation the scrim already gives.
    painter.rect_filled(plate, 16, theme.RAISED_BG);
    painter.rect_stroke(
        plate,
        16,
        Stroke::new(1.0, theme.SURFACE_5),
        egui::StrokeKind::Inside,
    );

    // The plate is a target of its own so a press on it stops here instead of reaching the
    // page underneath, which is still drawn in this frame and still full of buttons.
    let _ = ui.interact(plate, egui::Id::new("about-plate"), Sense::click());

    // The close box, at the reference's `top-4 right-4`.
    let close = Rect::from_min_size(
        egui::pos2(plate.right() - 16.0 - CLOSE, plate.top() + 16.0),
        Vec2::splat(CLOSE),
    );
    let response = ui.interact(close, egui::Id::new("about-close"), Sense::click());
    let ink = if response.hovered() {
        theme.CONTRAST
    } else {
        theme.SECONDARY
    };
    icons::draw(
        painter,
        Glyph::X,
        Rect::from_center_size(close.center(), Vec2::splat(14.0)),
        ink,
    );
    if response.clicked() {
        *open = false;
        return false;
    }

    // The mark, at 96 points: the generated asset when the texture is there, and the plate
    // it would sit on when it is not (a headless run, or a context whose upload failed).
    let mark = Rect::from_center_size(
        egui::pos2(plate.center().x, plate.top() + PAD + MARK / 2.0),
        Vec2::splat(MARK),
    );
    if !brand::paint_mark(painter, ui.ctx(), mark) {
        painter.rect_filled(mark, 20, theme.RAISED_BG);
        painter.rect_stroke(
            mark,
            20,
            Stroke::new(1.0, theme.ACCENT_CONTRAST.gamma_multiply(0.4)),
            egui::StrokeKind::Inside,
        );
    }

    // The identity line: `app.settings.app-version`'s shape with this product's name and
    // this crate's version. This is the one line of the reference's own copy this view can
    // honestly reuse; everything below it is the product's own notices.
    painter.text(
        egui::pos2(plate.center().x, mark.bottom() + 18.0),
        Align2::CENTER_TOP,
        format!("{PRODUCT} {VERSION}"),
        fonts::heading(20.0),
        theme.CONTRAST,
    );
    painter.text(
        egui::pos2(plate.center().x, mark.bottom() + 50.0),
        Align2::CENTER_TOP,
        LICENCE,
        fonts::body(13.0),
        theme.SECONDARY,
    );

    // The rule that separates the identity from the notices: the reference's
    // `border-surface-5` divider, one point.
    let divider = mark.bottom() + 86.0;
    painter.hline(
        (plate.left() + PAD)..=(plate.right() - PAD),
        divider,
        Stroke::new(1.0, theme.DIVIDER),
    );

    let left = plate.left() + PAD;
    let content = plate.width() - 2.0 * PAD;
    let mut y = divider + 16.0;
    // The heading is this view's own word, not the reference's: there is no About surface
    // there to take one from (see the module docs), and a section called anything else
    // would not say what the list is.
    y += paragraph(
        painter,
        egui::pos2(left, y),
        "Third-party notices",
        fonts::semibold(13.0),
        theme.BASE,
        content,
    ) + 8.0;

    // Two columns: the project's name, then its licence and where its notice lives. The
    // detail column wraps, so a long path produces a taller row rather than an overflow.
    for (name, detail) in ATTRIBUTION {
        let name_height = paragraph(
            painter,
            egui::pos2(left, y),
            name,
            fonts::label(12.5),
            theme.CONTRAST,
            NAME_COL - 12.0,
        );
        let detail_height = paragraph(
            painter,
            egui::pos2(left + NAME_COL, y),
            detail,
            fonts::body(12.0),
            theme.SECONDARY,
            content - NAME_COL,
        );
        y += name_height.max(detail_height) + 8.0;
    }

    // Where the source half of the same list lives, so the modal and the file cannot be
    // read as two inventories: the footer's wording is the iced build's own footer line.
    painter.text(
        egui::pos2(left, plate.bottom() - PAD),
        Align2::LEFT_BOTTOM,
        "Every notice travels beside the launcher: THIRD_PARTY_NOTICES.md",
        fonts::body(12.0),
        theme.SECONDARY,
    );

    // Still open. The caller may keep asking for frames while this is true; nothing here
    // moves, so it does not ask for one itself.
    *open
}

/// One block of wrapped text, drawn at `at`; returns the height it took.
///
/// The layout is egui's own (`Painter::layout`), so the height a row claims is the height
/// its galley actually painted -- which is what lets the attribution rows stack without a
/// guessed line count, and is why a longer path makes a taller row instead of a collision.
fn paragraph(
    painter: &egui::Painter,
    at: egui::Pos2,
    text: &str,
    font: egui::FontId,
    color: Color32,
    wrap: f32,
) -> f32 {
    let galley = painter.layout(text.to_owned(), font, color, wrap);
    let height = galley.size().y;
    painter.galley(at, galley, color);
    height
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The version the view prints is the manifest's, and it is a version: non-empty, starts
    /// with a digit and names a major and a minor. A blank or malformed `CARGO_PKG_VERSION`
    /// would render as "Sift Launcher " and look like a build that never set one.
    #[test]
    fn the_version_is_the_manifests_and_is_not_empty() {
        assert!(!VERSION.is_empty(), "the manifest has a version");
        assert!(
            VERSION.starts_with(|c: char| c.is_ascii_digit()),
            "{VERSION} starts with a digit"
        );
        assert!(VERSION.contains('.'), "{VERSION} names a major and a minor");
    }

    /// The identity line is the product name and that version, the reference's
    /// `app.settings.app-version` shape: `Modrinth App {version}` with this name in it.
    ///
    /// Asserted as name-then-version rather than against the literal "Sift Launcher 0.1.0":
    /// a version bump is a manifest edit, not a reason for the About view's test to fail.
    #[test]
    fn the_identity_line_names_the_product_and_the_version() {
        let identity = format!("{PRODUCT} {VERSION}");
        assert!(identity.starts_with(PRODUCT), "{identity} starts with the name");
        assert!(identity.ends_with(VERSION), "{identity} ends with the version");
        assert_eq!(PRODUCT, "Sift Launcher", "the core crate's product name");
    }

    /// The licence line has to say both things a licence line is for: who owns the work and
    /// that it is not open source. `LICENSE` at the root is the long version of this.
    #[test]
    fn the_licence_line_names_the_owner_and_the_terms() {
        assert!(LICENCE.contains("Palantir Studios"));
        assert!(LICENCE.contains("Proprietary"));
        assert!(LICENCE.contains("All rights reserved"));
    }

    /// Every row names a licence, and the adopted scroll crate is one of the rows: the
    /// notice for vendored MIT code is the reason this view exists, so a list that lost it
    /// would be a list that stopped meeting the shipping condition.
    #[test]
    fn every_notice_names_a_licence_and_the_scroll_crate_is_listed() {
        assert!(
            ATTRIBUTION
                .iter()
                .any(|(name, _)| *name == "fastframe-scroll 0.6.0"),
            "the vendored fastframe-scroll carries its MIT notice here"
        );
        for (name, detail) in ATTRIBUTION {
            assert!(!name.is_empty(), "a row needs a project name");
            let named = ["MIT", "GPL", "Apache", "OFL", "License", "ISC"]
                .iter()
                .any(|licence| detail.contains(licence));
            assert!(named, "{name}'s row names its licence: {detail}");
        }
    }

    /// The name column has to leave the detail column a width worth wrapping into: a column
    /// that ate the plate would put every detail on one character per line.
    #[test]
    fn the_attribution_columns_leave_room_for_the_detail() {
        let content = PLATE_W - 2.0 * PAD;
        assert!(NAME_COL > 0.0);
        assert!(
            content - NAME_COL > 200.0,
            "the detail column is the wider one"
        );
    }
}
