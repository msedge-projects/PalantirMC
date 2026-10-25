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

use egui::{Align2, Color32, Rect, Sense, Stroke, Ui, Vec2};

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

    /// The name the reference prints in the page header.
    pub fn title(self) -> &'static str {
        match self {
            Rail::Discover => "Discover modpacks",
            Rail::Search => "Search",
            Rail::Library => "Instances",
            Rail::Files => "Files",
            Rail::Servers => "Servers",
        }
    }

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
            theme: crate::theme::dark(),
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
        // The reference's mark is its brand logo, which the icon set does not carry: it
        // is `logo512.png`, uploaded as a texture in the slice that does image loading.
        // Until then the plate is drawn in the accent, so the bar keeps its anchor.
        painter.rect_filled(mark, 5, self.theme.ACCENT_CONTRAST);
        painter.text(
            egui::pos2(mark.right() + 8.0, bar.center().y),
            Align2::LEFT_CENTER,
            "Sift Launcher",
            crate::fonts::semibold(14.0),
            self.theme.CONTRAST,
        );

        // The breadcrumb row, on the same line as the brand: back and forward in their
        // circles, the route's badge, then the page's name. Measured from the reference at
        // 2x (`top.jpg`): the arrows' circles start at 167, the badge at 240, the title 265.
        for (index, glyph) in [Glyph::ChevronLeft, Glyph::ChevronRight]
            .into_iter()
            .enumerate()
        {
            let centre = egui::pos2(
                g::BREADCRUMB_ARROWS + index as f32 * 34.0,
                bar.center().y,
            );
            let circle = Rect::from_center_size(centre, Vec2::splat(24.0));
            painter.circle_stroke(circle.center(), 12.0, Stroke::new(1.0, self.theme.DIVIDER));
            icons::draw(painter, glyph, Rect::from_center_size(centre, Vec2::splat(13.0)), self.theme.SECONDARY);
        }
        let badge = egui::pos2(g::BREADCRUMB_BADGE, bar.center().y);
        painter.circle_stroke(badge, 10.0, Stroke::new(1.0, self.theme.SECONDARY));
        icons::draw(
            painter,
            self.rail.glyph(),
            Rect::from_center_size(badge, Vec2::splat(12.0)),
            self.theme.SECONDARY,
        );
        painter.text(
            egui::pos2(badge.x + 18.0, bar.center().y),
            Align2::LEFT_CENTER,
            self.rail.title(),
            crate::fonts::label(15.0),
            self.theme.CONTRAST,
        );

        // The running-instance chip, right-aligned before the window controls: the
        // reference prints `No instances running` in secondary text with a small dot.
        let controls_left = bar.right() - 3.0 * 44.0;
        let chip = egui::pos2(controls_left - 16.0, bar.center().y);
        painter.circle_filled(egui::pos2(chip.x - 8.0, chip.y), 3.0, self.theme.DIVIDER);
        painter.text(
            chip,
            Align2::RIGHT_CENTER,
            "No instances running",
            crate::fonts::body(12.0),
            self.theme.SECONDARY,
        );

        // The three window controls, in the reference's order and pitch.
        for (index, glyph) in [Glyph::Minus, Glyph::Square, Glyph::X]
            .into_iter()
            .enumerate()
        {
            let centre = egui::pos2(
                controls_left + 22.0 + index as f32 * 44.0,
                bar.center().y,
            );
            let target = Rect::from_center_size(centre, Vec2::splat(28.0));
            let response = ui.interact(
                target,
                egui::Id::new(("window-control", index)),
                Sense::click(),
            );
            let ink = if response.hovered() {
                self.theme.CONTRAST
            } else {
                self.theme.SECONDARY
            };
            icons::draw(painter, glyph, Rect::from_center_size(centre, Vec2::splat(14.0)), ink);
        }
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
        // The utility group below a separator: settings, reload, account. The reference
        // pins them to the bottom of the rail, which is why they are positioned from the
        // rail's own bottom edge rather than continuing the pitch.
        painter.hline(
            (rail.left() + 16.0)..=(rail.right() - 16.0),
            rail.bottom() - 3.0 * g::RAIL_PITCH - 24.0,
            Stroke::new(1.0, self.theme.DIVIDER),
        );
        for (index, glyph) in [Glyph::Settings, Glyph::RefreshCw, Glyph::CircleUser]
            .into_iter()
            .enumerate()
        {
            let centre = egui::pos2(
                g::RAIL / 2.0,
                rail.bottom() - 24.0 - (2 - index) as f32 * g::RAIL_PITCH,
            );
            icons::draw(
                painter,
                glyph,
                Rect::from_center_size(centre, Vec2::splat(20.0)),
                self.theme.SECONDARY,
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
            crate::fonts::semibold(16.0),
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
                crate::fonts::body(13.0),
                self.theme.BASE,
            );
        }

        // `Hide already installed`, with the switch the reference draws beside it.
        let toggle = egui::pos2(panel.left() + 24.0, panel.top() + 190.0);
        painter.text(
            toggle,
            Align2::LEFT_CENTER,
            "Hide already installed",
            crate::fonts::body(13.0),
            self.theme.BASE,
        );
        let switch = Rect::from_center_size(
            egui::pos2(panel.right() - 32.0, toggle.y),
            egui::vec2(28.0, 16.0),
        );
        painter.rect_filled(switch, 8, self.theme.BUTTON_BG);
        painter.circle_filled(
            egui::pos2(switch.left() + 8.0, switch.center().y),
            6.0,
            self.theme.SECONDARY,
        );

        // The news card: a raised block with a heading row and the article title.
        let news = Rect::from_min_size(
            egui::pos2(panel.left() + 24.0, panel.top() + 240.0),
            egui::vec2(g::PANEL - 48.0, 190.0),
        );
        painter.rect_filled(news, 12, self.theme.RAISED_BG);
        icons::draw(
            painter,
            Glyph::Newspaper,
            Rect::from_min_size(news.min + egui::vec2(12.0, 12.0), Vec2::splat(14.0)),
            self.theme.SECONDARY,
        );
        painter.text(
            egui::pos2(news.left() + 34.0, news.top() + 19.0),
            Align2::LEFT_CENTER,
            "Modrinth news",
            crate::fonts::semibold(12.0),
            self.theme.SECONDARY,
        );
        painter.text(
            egui::pos2(news.left() + 12.0, news.top() + 44.0),
            Align2::LEFT_TOP,
            "Get your next server on Modrinth.",
            crate::fonts::semibold(16.0),
            self.theme.CONTRAST,
        );
    }

    /// The tabs, the search row and the scrolling list. Returns whether a glide is running.
    fn content_column(&mut self, ui: &mut Ui) -> bool {
        let content = Rect::from_min_size(
            egui::pos2(g::CONTENT_LEFT, g::TITLE_BAR),
            egui::vec2(g::PANEL_LEFT - g::CONTENT_LEFT - 16.0, g::WINDOW.1 - g::TITLE_BAR),
        );
        let painter = ui.painter();
        painter.rect_filled(content, 0, self.theme.BG);

        // The tab strip: a rounded container holding six pills, the first of which is the
        // page's own type and is active. Measured at 2x from `top.jpg`: the container starts
        // 8 px left of the content column at 48 down, its pills are 36 tall and start 6 in.
        let strip = Rect::from_min_size(
            egui::pos2(content.left() - 8.0, g::TAB_CONTAINER_TOP),
            egui::vec2(content.width() + 8.0, g::TAB_CONTAINER),
        );
        painter.rect_filled(strip, 12, self.theme.RAISED_BG);
        let mut x = strip.left() + 6.0;
        for (index, label) in ["Modpacks", "Mods", "Resource Packs", "Data Packs", "Shaders", "Servers"]
            .into_iter()
            .enumerate()
        {
            let galley = painter.layout_no_wrap(
                label.to_owned(),
                crate::fonts::semibold(14.0),
                if index == 0 { self.theme.BUTTON_TEXT_SELECTED } else { self.theme.BASE },
            );
            let pill = Rect::from_min_size(
                egui::pos2(x, strip.top() + 8.0),
                egui::vec2(galley.size().x + 32.0, g::TAB_PILL),
            );
            if index == 0 {
                painter.rect_filled(pill, 18, self.theme.BUTTON_BG_SELECTED);
            }
            painter.galley(
                egui::pos2(pill.center().x - galley.size().x / 2.0, pill.center().y - galley.size().y / 2.0),
                galley.clone(),
                self.theme.BASE,
            );
            x = pill.right() + 8.0;
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
        crate::fonts::title(16.0),
        theme.CONTRAST,
    );
    painter.text(
        egui::pos2(rect.left() + 72.0, rect.top() + 46.0),
        Align2::LEFT_TOP,
        "A summary line the length of a result card's description.",
        crate::fonts::body(13.0),
        theme.BASE,
    );
    ui.add_space(g::CARD_GAP);
}
