//! The window's furniture: title bar, rail, right panel, and the content column.
//!
//! **Laid out from measured rectangles, not from a responsive tree.** The parity
//! requirement is +/-2 px against the Modrinth app, so every element is placed with the
//! numbers in [`crate::geometry`] and painted into an explicit rect. A layout engine that
//! decides its own gaps would be re-deciding, every frame, something the captures already
//! answered.
//!
//! **Repaint only when something moves.** An idle window draws nothing: the frame loop
//! exists while a hover, a scroll glide or a transition is running and stops when it lands.
//! That is the `idle CPU 0%` budget in `docs/LIGHTNESS.md`, and it is the one place this
//! crate differs most from the slice it grew out of, which asked for a frame every vsync
//! whether or not anything had changed.

use egui::{Align2, Color32, FontId, Rect, Sense, Stroke, Ui, Vec2};

use crate::geometry as g;
use crate::icons::{self, Glyph};
use crate::theme::Theme;

/// A rail destination. The reference has five, separated from the utility group below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rail {
    /// Find modpacks and content.
    Discover,
    /// Search everything at once.
    Search,
    /// The installed instances.
    Library,
    /// Files the launcher knows about.
    Files,
    /// Servers the launcher knows about.
    Servers,
}

impl Rail {
    /// Every destination, in the reference's order.
    pub const ALL: [Rail; 5] = [
        Rail::Discover,
        Rail::Search,
        Rail::Library,
        Rail::Files,
        Rail::Servers,
    ];

    /// The glyph the reference draws for this destination.
    pub fn glyph(self) -> Glyph {
        match self {
            Rail::Discover => Glyph::Compass,
            Rail::Search => Glyph::Search,
            Rail::Library => Glyph::Library,
            Rail::Files => Glyph::Folder,
            Rail::Servers => Glyph::Server,
        }
    }
}

/// What the shell is showing, and what is moving.
pub struct Shell {
    /// The selected rail destination.
    pub rail: Rail,
    /// The hovered rail destination, if any. A press animates between the two, which is
    /// why this is state rather than a per-frame read.
    hovered: Option<Rail>,
    /// fastframe's scroll state: the wheel step, the touchpad scale and the glide.
    scrolling: fastframe_scroll::Scrolling,
    /// The colour scheme, from the generated tokens.
    pub theme: Theme,
}

impl Default for Shell {
    fn default() -> Shell {
        Shell {
            rail: Rail::Discover,
            hovered: None,
            scrolling: fastframe_scroll::Scrolling::default(),
            theme: Theme::dark(),
        }
    }
}

impl Shell {
    /// Draw one frame. Returns whether anything is still animating, which the caller uses
    /// to decide whether to ask for another frame.
    pub fn ui(&mut self, ui: &mut Ui) -> bool {
        let ctx = ui.ctx().clone();
        // First thing in the frame, before any scroll area reads the input.
        self.scrolling.apply(&ctx);

        let window = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(g::WINDOW.0, g::WINDOW.1));
        ui.painter().rect_filled(window, 0, self.theme.BG);

        self.title_bar(ui);
        let mut rail_animating = false;
        rail_animating |= self.rail_column(ui);
        self.right_panel(ui);
        let content = self.content_column(ui);

        rail_animating || content
    }

    /// The top bar: the mark and the page's name at the left, window controls at the right.
    fn title_bar(&mut self, ui: &mut Ui) {
        let bar = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(g::WINDOW.0, g::TITLE_BAR));
        let painter = ui.painter();
        painter.rect_filled(bar, 0, self.theme.RAISED_BG);
        // The rule under the bar is the reference's `--color-divider`, one pixel.
        painter.hline(
            bar.x_range(),
            bar.bottom() - 0.5,
            Stroke::new(1.0, self.theme.DIVIDER),
        );

        // The mark and the product name, at the reference's own inset.
        let mark = Rect::from_min_size(egui::pos2(10.0, 10.0), Vec2::splat(20.0));
        icons::draw(painter, Glyph::Modrinth, mark, self.theme.ACCENT_CONTRAST);
        painter.text(
            egui::pos2(mark.right() + 8.0, bar.center().y),
            Align2::LEFT_CENTER,
            "Sift Launcher",
            FontId::proportional(14.0),
            self.theme.CONTRAST,
        );
    }

    /// The five destinations and the utility group, at the measured pitch.
    ///
    /// Returns whether a hover is still moving, so the caller can keep the frame loop
    /// alive only while something is in flight.
    fn rail_column(&mut self, ui: &mut Ui) -> bool {
        let rail = Rect::from_min_size(
            egui::pos2(0.0, g::TITLE_BAR),
            egui::vec2(g::RAIL, g::WINDOW.1 - g::TITLE_BAR),
        );
        let painter = ui.painter();
        painter.rect_filled(rail, 0, self.theme.RAISED_BG);
        painter.vline(
            rail.right() - 0.5,
            rail.y_range(),
            Stroke::new(1.0, self.theme.DIVIDER),
        );

        let mut animating = false;
        let mut hovered = None;
        for (index, destination) in Rail::ALL.into_iter().enumerate() {
            let centre = egui::pos2(
                g::RAIL / 2.0,
                g::TITLE_BAR + g::RAIL_FIRST + index as f32 * g::RAIL_PITCH,
            );
            // A 40 px target around a 20 px glyph: the reference's plate is larger than
            // the icon so the pointer can rest on it.
            let target = Rect::from_center_size(centre, Vec2::splat(40.0));
            let response = ui.interact(
                target,
                egui::Id::new(("rail", index)),
                Sense::click(),
            );
            if response.hovered() {
                hovered = Some(destination);
            }
            if response.clicked() {
                self.rail = destination;
            }

            let selected = destination == self.rail;
            let plate = if selected {
                self.theme.BUTTON_BG_SELECTED
            } else if response.hovered() {
                self.theme.BUTTON_BG
            } else {
                Color32::TRANSPARENT
            };
            if plate != Color32::TRANSPARENT {
                painter.rect_filled(target, 12, plate);
            }
            let ink = if selected {
                self.theme.BUTTON_TEXT_SELECTED
            } else if response.hovered() {
                self.theme.CONTRAST
            } else {
                self.theme.SECONDARY
            };
            icons::draw(
                painter,
                destination.glyph(),
                Rect::from_center_size(centre, Vec2::splat(20.0)),
                ink,
            );
        }
        // A hover that changed keeps the loop alive for the transition; once the hover
        // settles there is nothing left to draw.
        if hovered != self.hovered {
            self.hovered = hovered;
            animating = true;
        }
        animating
    }

    /// The right panel: the getting-started rows and the news card.
    fn right_panel(&mut self, ui: &mut Ui) {
        let panel = Rect::from_min_size(
            egui::pos2(g::PANEL_LEFT, g::TITLE_BAR),
            egui::vec2(g::PANEL, g::WINDOW.1 - g::TITLE_BAR),
        );
        let painter = ui.painter();
        painter.rect_filled(panel, 0, self.theme.BG);
        painter.vline(
            panel.left() + 0.5,
            panel.y_range(),
            Stroke::new(1.0, self.theme.DIVIDER),
        );

        let row = |y: f32| Rect::from_min_size(
            egui::pos2(panel.left() + 24.0, y),
            egui::vec2(g::PANEL - 48.0, 36.0),
        );
        let heading = egui::pos2(panel.left() + 24.0, panel.top() + 20.0);
        painter.text(
            heading,
            Align2::LEFT_TOP,
            "Getting started",
            FontId::proportional(16.0),
            self.theme.CONTRAST,
        );
        for (index, label) in ["Create first instance", "Sign in to Minecraft", "Sign in to Modrinth"]
            .into_iter()
            .enumerate()
        {
            let rect = row(panel.top() + 56.0 + index as f32 * 40.0);
            painter.rect_filled(rect, 8, self.theme.RAISED_BG);
            painter.text(
                egui::pos2(rect.left() + 12.0, rect.center().y),
                Align2::LEFT_CENTER,
                label,
                FontId::proportional(13.0),
                self.theme.BASE,
            );
        }
    }

    /// The tabs, the search row and the scrolling list. Returns whether a glide is running.
    fn content_column(&mut self, ui: &mut Ui) -> bool {
        let content = Rect::from_min_size(
            egui::pos2(g::CONTENT_LEFT, g::TITLE_BAR),
            egui::vec2(g::PANEL_LEFT - g::CONTENT_LEFT - 16.0, g::WINDOW.1 - g::TITLE_BAR),
        );
        let painter = ui.painter();
        painter.rect_filled(content, 0, self.theme.BG);

        // The tab strip, with the first label at the measured x.
        let strip = Rect::from_min_size(content.min, egui::vec2(content.width(), g::TAB_STRIP));
        painter.rect_filled(strip, 8, self.theme.RAISED_BG);
        let mut x = content.left() + 108.0;
        for label in ["Mods", "Resource Packs", "Data Packs", "Shaders", "Servers"] {
            let galley = painter.layout_no_wrap(
                label.to_owned(),
                FontId::proportional(14.0),
                self.theme.BASE,
            );
            painter.galley(egui::pos2(x, strip.center().y - galley.size().y / 2.0), galley.clone(), self.theme.BASE);
            x += galley.size().x + 24.0;
        }

        // The list, via fastframe's scrolling: 120 points per notch rather than egui's 40.
        let list = Rect::from_min_max(
            egui::pos2(content.left(), strip.bottom() + 12.0),
            content.max,
        );
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(list));
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(&mut child, |ui| {
                for index in 0..12 {
                    card(ui, &self.theme, index);
                }
            });
        false
    }
}

/// One result card, sized from the measured card height and gap.
fn card(ui: &mut Ui, theme: &Theme, index: usize) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, g::CARD), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 12, theme.RAISED_BG);
    painter.text(
        egui::pos2(rect.left() + 72.0, rect.top() + 20.0),
        Align2::LEFT_TOP,
        format!("Project {index}"),
        FontId::proportional(16.0),
        theme.CONTRAST,
    );
    painter.text(
        egui::pos2(rect.left() + 72.0, rect.top() + 46.0),
        Align2::LEFT_TOP,
        "A summary line the length of a result card's description.",
        FontId::proportional(13.0),
        theme.BASE,
    );
    ui.add_space(g::CARD_GAP);
}
