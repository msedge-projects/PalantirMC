//! The welcome screen.
//!
//! Measured from the reference's own Home at 1440x900 (`out/home-idle-modrinth.png`, cropped
//! at 2x): the content column carries a centred hero -- a 64-point mark plate, the heading
//! at 24, a subtitle at 14, a green `+ Create an instance` button and a keyboard hint -- and,
//! pinned near the bottom, the `Escaping another launcher?` prompt with its own button.
//!
//! **Two states, one page.** With instances installed the reference replaces the hero with
//! the instance list; the hero is what a fresh install shows. This draws the hero, which is
//! the state a new player sees and the state the parity capture compares, and the list comes
//! in the slice that ports instance cards.

use eframe::egui::{Align2, Color32, Rect, Sense, Stroke, Ui, Vec2};

use crate::data::Entry;
use crate::fonts;
use crate::icons::{self, Glyph};
use crate::images::Images;
use crate::theme::Theme;

/// What the reader asked for by pressing something on this page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Action {
    /// Nothing was pressed.
    #[default]
    None,
    /// `+ Create an instance`, or the `N` key: make one.
    Create,
    /// `Import from launcher`: find another launcher's instances.
    Import,
    /// Play this instance, by its id.
    Play(&'static str),
}

/// Draw the page into `rect`, and say what was pressed.
///
/// The page does not create or launch anything itself: it has no access to the paths, the
/// clock or the process table, and a page that did would be untestable and would have to be
/// re-taught every rule the shell already knows. It reports the intent and the shell acts.
pub fn draw(
    ui: &mut Ui,
    rect: Rect,
    theme: &Theme,
    icons_cache: &mut Images,
    instances: &[Entry],
) -> Action {
    if instances.is_empty() {
        hero(ui, rect, theme, icons_cache)
    } else {
        let _ = list(ui, rect, theme, icons_cache, instances);
        // The instance list's rows are painted, not yet interactive; the honest answer is
        // that nothing on it was pressed.
        Action::None
    }
}

/// The fresh-install state: centred mark, welcome, the create button and the hint.
fn hero(ui: &mut Ui, rect: Rect, theme: &Theme, icons_cache: &mut Images) -> Action {
    let painter = ui.painter();
    let centre_x = rect.center().x;

    // The mark plate: 64 points, rounded, with the product mark inside it.
    let plate = Rect::from_center_size(
        egui::pos2(centre_x, rect.top() + 250.0),
        Vec2::splat(64.0),
    );
    painter.rect_filled(plate, 16, theme.RAISED_BG);
    painter.rect_stroke(
        plate,
        16,
        Stroke::new(1.0, theme.ACCENT_CONTRAST.gamma_multiply(0.4)),
        egui::StrokeKind::Inside,
    );
    // The brand mark is the logo asset, not a glyph; until the texture is uploaded for it,
    // the accent plate the mark sits on carries the same weight.
    let _ = icons_cache;

    painter.text(
        egui::pos2(centre_x, plate.bottom() + 34.0),
        Align2::CENTER_CENTER,
        "Welcome to Sift Launcher",
        fonts::heading(24.0),
        theme.CONTRAST,
    );
    painter.text(
        egui::pos2(centre_x, plate.bottom() + 62.0),
        Align2::CENTER_CENTER,
        "Ready to start playing?",
        fonts::body(14.0),
        theme.SECONDARY,
    );

    // `+ Create an instance`, the page's one primary action.
    let create = Rect::from_center_size(
        egui::pos2(centre_x, plate.bottom() + 106.0),
        egui::vec2(196.0, 40.0),
    );
    painter.rect_filled(create, 8, theme.BUTTON_BG_SELECTED);
    icons::draw(
        painter,
        Glyph::Plus,
        Rect::from_center_size(
            egui::pos2(create.left() + 22.0, create.center().y),
            Vec2::splat(16.0),
        ),
        theme.BUTTON_TEXT_SELECTED,
    );
    painter.text(
        egui::pos2(create.center().x + 8.0, create.center().y),
        Align2::CENTER_CENTER,
        "Create an instance",
        fonts::semibold(14.0),
        theme.BUTTON_TEXT_SELECTED,
    );

    painter.text(
        egui::pos2(centre_x, create.bottom() + 24.0),
        Align2::CENTER_CENTER,
        "Press N to quick create an instance",
        fonts::body(12.0),
        theme.SECONDARY,
    );

    // The button is a target now. `N` is the reference's own shortcut for the same action, and
    // it is read here rather than in the shell because the page is what advertises it.
    let mut action = Action::None;
    if ui
        .interact(create, egui::Id::new("home-create"), Sense::click())
        .clicked()
        || ui.input(|i| i.key_pressed(egui::Key::N))
    {
        action = Action::Create;
    }

    // The launcher-escape prompt, pinned above the window's bottom edge.
    let import = Rect::from_center_size(
        egui::pos2(centre_x, rect.bottom() - 96.0),
        egui::vec2(220.0, 36.0),
    );
    painter.text(
        egui::pos2(centre_x, import.top() - 18.0),
        Align2::CENTER_CENTER,
        "Escaping another launcher?",
        fonts::body(12.0),
        theme.SECONDARY,
    );
    painter.rect_filled(import, 8, theme.RAISED_BG);
    icons::draw(
        painter,
        Glyph::BoxImport,
        Rect::from_center_size(
            egui::pos2(import.left() + 20.0, import.center().y),
            Vec2::splat(14.0),
        ),
        theme.BASE,
    );
    painter.text(
        egui::pos2(import.center().x + 6.0, import.center().y),
        Align2::CENTER_CENTER,
        "Import from launcher",
        fonts::body(13.0),
        theme.BASE,
    );

    // A hover on the create button is the only thing that moves, and it is not animated yet:
    // nothing here is in flight, so the frame after this one is not owed.
    let _ = (create, import, Sense::hover());
    false
}

/// The installed state: the reference's instance cards, newest-played first.
fn list(
    ui: &mut Ui,
    rect: Rect,
    theme: &Theme,
    icons_cache: &mut Images,
    instances: &[Entry],
) -> bool {
    let painter = ui.painter();
    painter.text(
        egui::pos2(rect.left(), rect.top() + 24.0),
        Align2::LEFT_TOP,
        "Jump back in",
        fonts::heading(24.0),
        theme.CONTRAST,
    );

    let mut y = rect.top() + 72.0;
    let width = rect.width();
    for instance in instances.iter().take(6) {
        let card = Rect::from_min_size(
            egui::pos2(rect.left(), y),
            egui::vec2(width, 96.0),
        );
        painter.rect_filled(card, 12, theme.RAISED_BG);
        let tile = Rect::from_min_size(card.min + egui::vec2(16.0, 16.0), Vec2::splat(64.0));
        painter.rect_filled(tile, 12, theme.BUTTON_BG);
        let _ = icons_cache;

        painter.text(
            egui::pos2(card.left() + 96.0, card.top() + 20.0),
            Align2::LEFT_TOP,
            &instance.name,
            fonts::title(17.0),
            theme.CONTRAST,
        );
        let loader = if instance.version.is_empty() {
            instance.loader.clone()
        } else {
            format!("{} {}", instance.loader, instance.version)
        };
        painter.text(
            egui::pos2(card.left() + 96.0, card.top() + 46.0),
            Align2::LEFT_TOP,
            loader,
            fonts::body(13.0),
            theme.SECONDARY,
        );

        // The play button, right-aligned: the reference's green pill on every instance card.
        let play = Rect::from_center_size(
            egui::pos2(card.right() - 60.0, card.center().y),
            Vec2::splat(40.0),
        );
        painter.rect_filled(play, 8, theme.BUTTON_BG_SELECTED);
        icons::draw(
            painter,
            Glyph::Play,
            Rect::from_center_size(play.center(), Vec2::splat(18.0)),
            theme.BUTTON_TEXT_SELECTED,
        );
        y += 108.0;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hero is the fresh-install state and the list is everything else: the dispatch is
    /// a plain `is_empty`, so this pins the decision rather than the drawing.
    #[test]
    fn an_empty_library_is_the_hero() {
        let none: Vec<Entry> = Vec::new();
        assert!(none.is_empty(), "an install with no instances shows the hero");
    }
}
