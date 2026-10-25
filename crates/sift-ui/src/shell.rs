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
    /// Project icons, fetched off the frame thread and cached as textures.
    icons: crate::images::Images,
}

impl Default for Shell {
    fn default() -> Shell {
        Shell {
            rail: Rail::Discover,
            hovered: None,
            scrolling: fastframe_scroll::Scrolling::default(),
            theme: crate::theme::dark(),
            icons: crate::images::Images::default(),
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

        // An icon in flight is a reason to draw the next frame, exactly like a hover.
        rail_animating || content || self.icons.loading()
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

        // The search row: the field, the sort and view selects, the result count and the
        // pager, in the reference's order. Heights are the reference's 40-point controls.
        let row = Rect::from_min_size(
            egui::pos2(content.left(), strip.bottom() + 12.0),
            egui::vec2(content.width(), g::SEARCH_ROW),
        );
        let field = Rect::from_min_size(
            row.min,
            egui::vec2(row.width() * 0.62, g::SEARCH_ROW),
        );
        painter.rect_filled(field, 8, self.theme.RAISED_BG);
        painter.circle_stroke(
            egui::pos2(field.left() + 18.0, field.center().y),
            6.0,
            Stroke::new(1.5, self.theme.SECONDARY),
        );
        painter.text(
            egui::pos2(field.left() + 34.0, field.center().y),
            Align2::LEFT_CENTER,
            "Search modpacks...",
            crate::fonts::body(13.0),
            self.theme.SECONDARY,
        );
        // The two selects: `Sort by: Relevance` and `View: 20`, each its own rounded box.
        let select_w = 150.0;
        for (index, label) in ["Sort by:  Relevance", "View: 20"]
            .into_iter()
            .enumerate()
        {
            let select = Rect::from_min_size(
                egui::pos2(field.right() + 12.0 + index as f32 * (select_w + 8.0), row.top()),
                egui::vec2(select_w, g::SEARCH_ROW),
            );
            painter.rect_filled(select, 8, self.theme.RAISED_BG);
            painter.text(
                egui::pos2(select.left() + 12.0, select.center().y),
                Align2::LEFT_CENTER,
                label,
                crate::fonts::body(13.0),
                self.theme.BASE,
            );
            icons::draw(
                painter,
                Glyph::ChevronDown,
                Rect::from_center_size(
                    egui::pos2(select.right() - 16.0, select.center().y),
                    Vec2::splat(12.0),
                ),
                self.theme.SECONDARY,
            );
        }
        // The result count and the pager, right-aligned on the same row.
        painter.text(
            egui::pos2(row.right() - 110.0, row.center().y),
            Align2::LEFT_CENTER,
            "1  -  946",
            crate::fonts::body(13.0),
            self.theme.SECONDARY,
        );

        // The list, via fastframe's scrolling: 120 points per notch rather than egui's 40.
        let list = Rect::from_min_max(
            egui::pos2(content.left(), row.bottom() + 12.0),
            content.max,
        );
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(list));
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(&mut child, |ui| {
                // The reference's own six results first, then repeats so the list is long
                // enough to scroll: a capture compares anatomy, not the catalogue.
                let results = sample_results();
                for index in 0..12 {
                    card(ui, &self.theme, &results[index % results.len()], &mut self.icons);
                }
            });
        false
    }
}

/// One result card, drawn to the reference's anatomy.
///
/// Measured from a 2.2x crop of the reference (`card.jpg`), in window pixels from the
/// card's own top-left: a 64-point icon tile at 16,16; the title at 96,16 with the author
/// beside it and a link glyph after; a two-line summary at 96,44; a row of tag pills at
/// 96,104; the stats right-aligned on that same row; and the install button at the card's
/// top-right corner. The card itself is 148 tall with a 12-point gap, which is what the
/// reference's pitch between two card tops measures.
fn card(ui: &mut Ui, theme: &Theme, result: &Card<'_>, icons: &mut crate::images::Images) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, g::CARD), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 12, theme.RAISED_BG);

    // The icon tile. The reference draws the project's icon here; until images load, a
    // plate in the accent carries the same weight and the same 12-point radius.
    let tile = Rect::from_min_size(rect.min + egui::vec2(16.0, 16.0), Vec2::splat(64.0));
    painter.rect_filled(tile, 12, theme.BUTTON_BG);
    // The project's own icon once it has arrived; the plate until then, and for ever if the
    // fetch failed. Rounded corners are the tile's: a texture drawn into a square rect inside
    // a rounded plate keeps the plate's corner showing, which is what the reference does.
    if let Some(texture) = icons.get(ui.ctx(), result.icon) {
        let inner = tile.shrink(2.0);
        painter.image(
            texture.id(),
            inner,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }

    // The title, the author and the link glyph, on one baseline.
    let title = painter.layout_no_wrap(
        result.title.to_owned(),
        crate::fonts::title(17.0),
        theme.CONTRAST,
    );
    let title_at = egui::pos2(rect.left() + 96.0, rect.top() + 18.0);
    painter.galley(title_at, title.clone(), theme.CONTRAST);
    let by = painter.layout_no_wrap(
        format!("by {}", result.author),
        crate::fonts::body(13.0),
        theme.SECONDARY,
    );
    let by_at = egui::pos2(title_at.x + title.size().x + 10.0, title_at.y + 3.0);
    painter.galley(by_at, by.clone(), theme.SECONDARY);
    icons::draw(
        painter,
        Glyph::External,
        Rect::from_min_size(
            egui::pos2(by_at.x + by.size().x + 8.0, by_at.y + 2.0),
            Vec2::splat(12.0),
        ),
        theme.SECONDARY,
    );

    // The summary's measure is the reference's, not the space the button leaves: at 2.2x
    // its first card breaks after "in a", which is a ~440-point line, so the second line
    // exists for the same reason it does there.
    let summary_width = (rect.width() - 96.0 - 150.0).min(440.0);
    let summary = painter.layout(
        result.summary.to_owned(),
        crate::fonts::body(13.5),
        theme.SECONDARY,
        summary_width,
    );
    painter.galley(egui::pos2(rect.left() + 96.0, rect.top() + 44.0), summary, theme.SECONDARY);

    // The tag pills, each with its own glyph, then the stats on the same row.
    let mut x = rect.left() + 96.0;
    for tag in result.tags {
        let text = painter.layout_no_wrap(tag.0.to_owned(), crate::fonts::body(11.0), theme.SECONDARY);
        let pill = Rect::from_min_size(
            egui::pos2(x, rect.top() + 104.0),
            egui::vec2(text.size().x + 30.0, 24.0),
        );
        painter.rect_filled(pill, 12, theme.BUTTON_BG);
        icons::draw(
            painter,
            tag.1,
            Rect::from_center_size(
                egui::pos2(pill.left() + 14.0, pill.center().y),
                Vec2::splat(11.0),
            ),
            theme.SECONDARY,
        );
        painter.galley(
            egui::pos2(pill.left() + 24.0, pill.center().y - text.size().y / 2.0),
            text.clone(),
            theme.SECONDARY,
        );
        x = pill.right() + 6.0;
    }

    // The stats and the install button, right-aligned: downloads, followers, updated.
    let install = Rect::from_min_size(
        egui::pos2(rect.right() - 16.0 - 96.0, rect.top() + 16.0),
        egui::vec2(96.0, 36.0),
    );
    painter.rect_filled(install, 8, theme.BUTTON_BG_SELECTED);
    painter.text(
        install.center(),
        Align2::CENTER_CENTER,
        "Install",
        crate::fonts::semibold(13.0),
        theme.BUTTON_TEXT_SELECTED,
    );
    // The three stats, right-aligned on the tag row in the reference's order.
    let mut right = rect.right() - 16.0;
    for (glyph, value) in [
        (Glyph::Download, result.downloads),
        (Glyph::Heart, result.followers),
        (Glyph::Calendar, result.updated),
    ] {
        let label = painter.layout_no_wrap(
            value.to_owned(),
            crate::fonts::body(11.0),
            theme.SECONDARY,
        );
        painter.text(
            egui::pos2(right, rect.top() + 116.0),
            Align2::RIGHT_CENTER,
            value,
            crate::fonts::body(11.0),
            theme.SECONDARY,
        );
        icons::draw(
            painter,
            glyph,
            Rect::from_min_size(
                egui::pos2(right - label.size().x - 16.0, rect.top() + 116.0 - 6.0),
                Vec2::splat(12.0),
            ),
            theme.SECONDARY,
        );
        right -= label.size().x + 28.0;
    }

    ui.add_space(g::CARD_GAP);
}

/// What a result card shows. The reference's list carries these fields and no others.
pub struct Card<'a> {
    /// The project's name.
    pub title: &'a str,
    /// The team that published it.
    pub author: &'a str,
    /// One or two sentences of description.
    pub summary: &'a str,
    /// The categories and loader, as (label, glyph).
    pub tags: &'a [(&'a str, Glyph)],
    /// The download count, already abbreviated the way the reference shows it.
    pub downloads: &'a str,
    /// The follower count, abbreviated the same way.
    pub followers: &'a str,
    /// When it last changed, in the reference's phrasing.
    pub updated: &'a str,
    /// The project icon's URL.
    pub icon: &'a str,
}

/// Six results with the reference's own content, so a capture shows the real anatomy
/// rather than lorem ipsum: the first four are the cards the reference's first capture
/// displayed, in its order.
///
/// The icon URLs are the API's own `icon_url` answers for those six projects, fetched with
/// `tools/parity`'s probe rather than constructed: `/data/<id>/icon.png` is not a URL the
/// CDN serves -- it answers 400 on every id -- and the hashed `_96.webp` is.
fn sample_results() -> [Card<'static>; 6] {
    [
        Card {
            title: "Fabulously Optimized",
            author: "Fabulously Optimized",
            summary: "Beautiful graphics, speedy performance and familiar features in a simple package. Wilderness Bound beta!",
            tags: &[
                ("Client", Glyph::Monitor),
                ("Lightweight", Glyph::Sparkles),
                ("Multiplayer", Glyph::Users),
                ("Optimization", Glyph::Rocket),
                ("Fabric", Glyph::Box),
            ],
            downloads: "19.38M",
            followers: "5,005",
            updated: "5 days ago",
            icon: "https://cdn.modrinth.com/data/1KVo5zza/d8152911f8fd5d7e9a8c499fe89045af81fe816e_96.webp",
        },
        Card {
            title: "Zombie Invade 100 Days",
            author: "FlameFire",
            summary: "Same as Forge Labs 100 Days Zombie Apocalypse in new Minecraft.",
            tags: &[
                ("Client and server", Glyph::Users),
                ("Challenging", Glyph::TagCategorySwords),
                ("Forge", Glyph::Box),
            ],
            downloads: "15.36M",
            followers: "9,659",
            updated: "10 months ago",
            icon: "https://cdn.modrinth.com/data/l9m9tuPN/fefe3f67c37744344d100638452c7bf059d586a1_96.webp",
        },
        Card {
            title: "Cobblemon Official Modpack [Fabric]",
            author: "Cobbled Studios",
            summary: "The official modpack of the Cobblemon mod, for Fabric!",
            tags: &[
                ("Client and server", Glyph::Users),
                ("Adventure", Glyph::Compass),
                ("Lightweight", Glyph::Sparkles),
                ("Fabric", Glyph::Box),
            ],
            downloads: "11.53M",
            followers: "2,732",
            updated: "12 hours ago",
            icon: "https://cdn.modrinth.com/data/5FFgwNNP/e7f9ee2e9d361623847853fe2ddce42f519ee64f.png",
        },
        Card {
            title: "COBBLEVERSE - Pokemon Adventure",
            author: "LUMYVERSE",
            summary: "Start a true Pokemon adventure in Minecraft: Cobblemon 1.7.3 [ALL 1025 Pokemon] • Mega Evolutions • Gyms & Badges • Starter Kit.",
            tags: &[("Adventure", Glyph::Compass), ("Fabric", Glyph::Box)],
            downloads: "7.31M",
            followers: "2,092",
            updated: "Updated today",
            icon: "https://cdn.modrinth.com/data/Jkb29YJU/581ecf54530972afb18a04660afd820f2f24f6c7.png",
        },
        Card {
            title: "Better MC [FORGE]",
            author: "Shivaxi",
            summary: "The kitchen-sink pack that keeps its focus: quests, dungeons and a world that rewards exploring.",
            tags: &[("Adventure", Glyph::Compass), ("Forge", Glyph::Box)],
            downloads: "5.02M",
            followers: "1,940",
            updated: "3 days ago",
            icon: "https://cdn.modrinth.com/data/4BV47HRn/7f5ef7cc61b2593609aabef3ed07a866a4f11b73_96.webp",
        },
        Card {
            title: "All the Mods 10",
            author: "ATMTeam",
            summary: "Every mod that matters, curated and tested, with a quest book to hold it together.",
            tags: &[("Kitchen sink", Glyph::Boxes), ("NeoForge", Glyph::Box)],
            downloads: "4.88M",
            followers: "1,512",
            updated: "6 days ago",
            icon: "https://cdn.modrinth.com/data/CfpXILxG/832f62314350e237f299f2ee39d90fb7d85b6419_96.webp",
        },
    ]
}
