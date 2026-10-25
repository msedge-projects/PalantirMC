//! A project's page: header, tabs, description, and the compatibility block its sidebar
//! carries.
//!
//! Follows `vendor/modrinth-app/app-frontend/src/pages/project/{Index,Description,Versions,
//! Gallery}.vue` and the layout classes in its template. Measured from the reference's own
//! capture at 1440x900 (`out/project-idle-modrinth.png`): a 64-point icon tile at the content
//! column's left inset, the title at 20 semibold beside it, a 13-point summary under the
//! title, the stats row (downloads, followers) and tag pills under that, and the install
//! button and kebab menu right-aligned on the header row.
//!
//! **The sidebar is this page's, not the shell's.** The reference *teleports* its project
//! sidebar into the right panel (`<Teleport to="#sidebar-teleport-target">` in `Index.vue`),
//! which is why `panel` exists here: on a project page the panel's first sections describe
//! the project, and the shared promo stays below them.

use eframe::egui::{Align2, Color32, Rect, Sense, Stroke, Ui, Vec2};

use crate::fonts;
use crate::icons::{self, Glyph};
use crate::images::Images;
use crate::theme::Theme;

/// What the page shows. The Discover card carries the same fields, plus what a full project
/// page needs.
#[derive(Clone)]
pub struct Project {
    /// The project's name.
    pub title: &'static str,
    /// The team that publishes it.
    pub author: &'static str,
    /// The one-line summary the header shows.
    pub summary: &'static str,
    /// The longer description, already broken into paragraphs.
    pub body: &'static [&'static str],
    /// The loader and environment tags the header shows.
    pub tags: &'static [(&'static str, Glyph)],
    /// Game versions the project supports, newest first.
    pub game_versions: &'static [&'static str],
    /// The download count, abbreviated.
    pub downloads: &'static str,
    /// The follower count, abbreviated.
    pub followers: &'static str,
    /// The icon's URL.
    pub icon: &'static str,
}

/// The three tabs the page's own strip has. `Description` is the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// The project's long description.
    Description,
    /// Its released files.
    Versions,
    /// Its images.
    Gallery,
}

impl Tab {
    /// The three tabs, in the reference's order.
    pub const ALL: [Tab; 3] = [Tab::Description, Tab::Versions, Tab::Gallery];

    /// The tab's label.
    pub fn label(self) -> &'static str {
        match self {
            Tab::Description => "Description",
            Tab::Versions => "Versions",
            Tab::Gallery => "Gallery",
        }
    }
}

/// The page's own state: which tab is showing.
#[derive(Debug, Default)]
pub struct State {
    /// The selected tab.
    pub tab: Option<Tab>,
}

impl State {
    /// The tab that is showing.
    pub fn current(&self) -> Tab {
        self.tab.unwrap_or(Tab::Description)
    }
}

/// Draw the page's content column.
pub fn draw(
    ui: &mut Ui,
    rect: Rect,
    theme: &Theme,
    images: &mut Images,
    state: &mut State,
    project: &Project,
) -> bool {
    let painter = ui.painter();

    // ---- Header: the tile, the name and summary, the stats, and the actions ----
    let tile = Rect::from_min_size(rect.min + egui::vec2(0.0, 8.0), Vec2::splat(64.0));
    painter.rect_filled(tile, 12, theme.BUTTON_BG);
    if let Some(texture) = images.get(ui.ctx(), project.icon) {
        painter.image(
            texture.id(),
            tile,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }
    let text_x = tile.right() + 16.0;
    painter.text(
        egui::pos2(text_x, tile.top()),
        Align2::LEFT_TOP,
        project.title,
        fonts::title(20.0),
        theme.CONTRAST,
    );
    let summary = painter.layout(
        project.summary.to_owned(),
        fonts::body(13.0),
        theme.SECONDARY,
        rect.width() - 320.0,
    );
    painter.galley(egui::pos2(text_x, tile.top() + 26.0), summary, theme.SECONDARY);

    // The stats row: downloads then followers, each with its glyph, then the tag pills.
    let mut x = text_x;
    let stats_y = tile.top() + 52.0;
    for (glyph, value) in [
        (Glyph::Download, project.downloads),
        (Glyph::Heart, project.followers),
    ] {
        icons::draw(
            painter,
            glyph,
            Rect::from_min_size(egui::pos2(x, stats_y), Vec2::splat(12.0)),
            theme.SECONDARY,
        );
        let label = painter.layout_no_wrap(value.to_owned(), fonts::body(12.0), theme.SECONDARY);
        painter.galley(egui::pos2(x + 16.0, stats_y), label.clone(), theme.SECONDARY);
        x += 16.0 + label.size().x + 14.0;
    }
    for (tag, glyph) in project.tags {
        let label = painter.layout_no_wrap((*tag).to_owned(), fonts::body(11.0), theme.SECONDARY);
        let pill = Rect::from_min_size(
            egui::pos2(x, stats_y - 6.0),
            egui::vec2(label.size().x + 28.0, 22.0),
        );
        painter.rect_filled(pill, 11, theme.RAISED_BG);
        icons::draw(
            painter,
            *glyph,
            Rect::from_center_size(
                egui::pos2(pill.left() + 13.0, pill.center().y),
                Vec2::splat(10.0),
            ),
            theme.SECONDARY,
        );
        painter.galley(
            egui::pos2(pill.left() + 22.0, pill.center().y - label.size().y / 2.0),
            label.clone(),
            theme.SECONDARY,
        );
        x = pill.right() + 6.0;
    }

    // The install button and the kebab menu, right-aligned on the header row.
    let install = Rect::from_min_size(
        egui::pos2(rect.right() - 148.0, rect.top() + 8.0),
        egui::vec2(112.0, 40.0),
    );
    painter.rect_filled(install, 8, theme.BUTTON_BG_SELECTED);
    icons::draw(
        painter,
        Glyph::Download,
        Rect::from_center_size(
            egui::pos2(install.left() + 22.0, install.center().y),
            Vec2::splat(16.0),
        ),
        theme.BUTTON_TEXT_SELECTED,
    );
    painter.text(
        egui::pos2(install.center().x + 8.0, install.center().y),
        Align2::CENTER_CENTER,
        "Install",
        fonts::semibold(14.0),
        theme.BUTTON_TEXT_SELECTED,
    );
    let kebab = Rect::from_center_size(
        egui::pos2(rect.right() - 16.0, install.center().y),
        Vec2::splat(24.0),
    );
    icons::draw(painter, Glyph::EllipsisVertical, kebab.shrink(6.0), theme.SECONDARY);

    // ---- The page's own tab strip ----
    let tabs = Rect::from_min_size(
        egui::pos2(rect.left(), rect.top() + 84.0),
        egui::vec2(rect.width(), 36.0),
    );
    let mut tab_x = tabs.left();
    for tab in Tab::ALL {
        let label = painter.layout_no_wrap(tab.label().to_owned(), fonts::semibold(13.0), theme.BASE);
        let pill = Rect::from_min_size(
            egui::pos2(tab_x, tabs.top()),
            egui::vec2(label.size().x + 28.0, 30.0),
        );
        let active = tab == state.current();
        if active {
            painter.rect_filled(pill, 15, theme.BUTTON_BG_SELECTED);
        }
        painter.galley(
            egui::pos2(pill.center().x - label.size().x / 2.0, pill.center().y - label.size().y / 2.0),
            label.clone(),
            if active { theme.BUTTON_TEXT_SELECTED } else { theme.BASE },
        );
        // The whole pill is the hit target, so a press anywhere on it switches tabs.
        let response = ui.interact(pill, egui::Id::new(("project-tab", tab.label())), Sense::click());
        if response.clicked() {
            state.tab = Some(tab);
        }
        tab_x = pill.right() + 4.0;
    }

    // ---- The tab's own card ----
    let card = Rect::from_min_size(
        egui::pos2(rect.left(), tabs.bottom() + 14.0),
        egui::vec2(rect.width(), rect.bottom() - tabs.bottom() - 14.0),
    );
    painter.rect_filled(card, 12, theme.RAISED_BG);
    match state.current() {
        Tab::Description => description(ui, card, theme, project),
        Tab::Versions => empty_tab(ui, card, theme, "No versions yet."),
        Tab::Gallery => empty_tab(ui, card, theme, "No images yet."),
    }
    false
}

/// The description tab: the action row above the body, then the body itself.
fn description(ui: &mut Ui, card: Rect, theme: &Theme, project: &Project) {
    let painter = ui.painter();
    let row = Rect::from_min_size(card.min + egui::vec2(16.0, 16.0), egui::vec2(card.width() - 32.0, 36.0));
    let mut x = row.left();
    for (index, (label, glyph)) in [
        ("How to install", Glyph::Download),
        ("Discord", Glyph::Message),
        ("Rate", Glyph::Star),
    ]
    .into_iter()
    .enumerate()
    {
        let text = painter.layout_no_wrap(label.to_owned(), fonts::body(13.0), theme.BASE);
        let button = Rect::from_min_size(
            egui::pos2(x, row.top()),
            egui::vec2(text.size().x + 40.0, 32.0),
        );
        painter.rect_filled(button, 8, if index == 0 { theme.BUTTON_BG_SELECTED } else { theme.SUPER_RAISED_BG });
        icons::draw(
            painter,
            glyph,
            Rect::from_center_size(
                egui::pos2(button.left() + 18.0, button.center().y),
                Vec2::splat(14.0),
            ),
            if index == 0 { theme.BUTTON_TEXT_SELECTED } else { theme.BASE },
        );
        painter.galley(
            egui::pos2(button.left() + 32.0, button.center().y - text.size().y / 2.0),
            text.clone(),
            if index == 0 { theme.BUTTON_TEXT_SELECTED } else { theme.BASE },
        );
        x = button.right() + 8.0;
    }

    // The body: paragraphs stacked with the reference's 12-point leading.
    let mut y = row.bottom() + 20.0;
    for paragraph in project.body {
        let galley = painter.layout(
            (*paragraph).to_owned(),
            fonts::body(13.0),
            theme.BASE,
            card.width() - 64.0,
        );
        let height = galley.size().y;
        painter.galley(egui::pos2(card.left() + 32.0, y), galley, theme.BASE);
        y += height + 12.0;
        if y > card.bottom() - 24.0 {
            break;
        }
    }
    let _ = Sense::hover();
}

/// A tab with nothing in it yet, drawn so the card does not look broken.
fn empty_tab(ui: &mut Ui, card: Rect, theme: &Theme, message: &str) {
    ui.painter().text(
        card.center(),
        Align2::CENTER_CENTER,
        message,
        fonts::body(13.0),
        theme.SECONDARY,
    );
}

/// The sections this page contributes to the right panel.
///
/// The reference's `ProjectSidebarCompatibility`: the game versions as chips, then the
/// platform, then the supported environment. Drawn under the panel's own `Getting started`.
pub fn panel(ui: &mut Ui, rect: Rect, theme: &Theme, project: &Project) -> f32 {
    let painter = ui.painter();
    let mut y = rect.top();
    painter.text(
        egui::pos2(rect.left(), y),
        Align2::LEFT_TOP,
        "Compatibility",
        fonts::semibold(15.0),
        theme.CONTRAST,
    );
    y += 30.0;
    painter.text(
        egui::pos2(rect.left(), y),
        Align2::LEFT_TOP,
        "Minecraft: Java Edition",
        fonts::body(12.0),
        theme.SECONDARY,
    );
    y += 20.0;

    // Version chips, wrapping at the panel's width.
    let mut x = rect.left();
    for version in project.game_versions {
        let label = painter.layout_no_wrap((*version).to_owned(), fonts::body(11.0), theme.BASE);
        let chip = Rect::from_min_size(egui::pos2(x, y), egui::vec2(label.size().x + 16.0, 22.0));
        if chip.right() > rect.right() {
            x = rect.left();
            y += 26.0;
        }
        let chip = Rect::from_min_size(egui::pos2(x, y), egui::vec2(label.size().x + 16.0, 22.0));
        painter.rect_filled(chip, 6, theme.SUPER_RAISED_BG);
        painter.galley(egui::pos2(chip.left() + 8.0, chip.center().y - label.size().y / 2.0), label.clone(), theme.BASE);
        x = chip.right() + 6.0;
    }
    y += 34.0;

    for (heading, values) in [
        ("Platforms", project.tags.iter().filter(|(tag, _)| matches!(*tag, "Fabric" | "Forge" | "NeoForge" | "Quilt")).copied().collect::<Vec<_>>()),
        ("Supported environments", project.tags.iter().filter(|(tag, _)| *tag == "Client-side" || *tag == "Server-side").copied().collect::<Vec<_>>()),
    ] {
        painter.text(
            egui::pos2(rect.left(), y),
            Align2::LEFT_TOP,
            heading,
            fonts::body(12.0),
            theme.SECONDARY,
        );
        y += 20.0;
        let mut chip_x = rect.left();
        for (tag, glyph) in values {
            let label = painter.layout_no_wrap(tag.to_owned(), fonts::body(11.0), theme.BASE);
            let chip = Rect::from_min_size(egui::pos2(chip_x, y), egui::vec2(label.size().x + 26.0, 22.0));
            painter.rect_filled(chip, 6, theme.SUPER_RAISED_BG);
            icons::draw(
                painter,
                glyph,
                Rect::from_center_size(egui::pos2(chip.left() + 12.0, chip.center().y), Vec2::splat(11.0)),
                theme.BASE,
            );
            painter.galley(egui::pos2(chip.left() + 20.0, chip.center().y - label.size().y / 2.0), label.clone(), theme.BASE);
            chip_x = chip.right() + 6.0;
        }
        y += 32.0;
    }
    y - rect.top()
}

/// The project the Discover list's first card opens, so the page can be captured against the
/// reference's own first card.
pub fn sample() -> Project {
    Project {
        title: "Fabulously Optimized",
        author: "Fabulously Optimized",
        summary: "Beautiful graphics, speedy performance and familiar features in a simple package. Wilderness Bound beta!",
        body: &[
            "A simple Minecraft modpack focusing on performance and graphics enhancements.",
            "Fabulously Optimized is faster than competing game clients, thanks to modern performance mods like Sodium, Lithium, and others. The bundled mods have been carefully selected to be safe in your worlds and provide noticeable speed benefits on any device.",
        ],
        tags: &[
            ("Lightweight", Glyph::Sparkles),
            ("Multiplayer", Glyph::Users),
            ("Optimization", Glyph::Rocket),
            ("Fabric", Glyph::Box),
            ("Client-side", Glyph::Monitor),
        ],
        game_versions: &["26.3", "26.2", "26.1.x", "1.21.x", "1.20.x", "1.19.x", "1.18.1-1.18.2", "1.17.1", "1.16.5"],
        downloads: "19.4M downloads",
        followers: "5K followers",
        icon: "https://cdn.modrinth.com/data/1KVo5zza/d8152911f8fd5d7e9a8c499fe89045af81fe816e_96.webp",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The page defaults to Description, which is what the reference shows on arrival.
    #[test]
    fn a_project_opens_on_its_description() {
        let state = State::default();
        assert_eq!(state.current(), Tab::Description);
    }

    /// The tabs are in the reference's order and each has a label: a tab that rendered an
    /// empty string would be an invisible target.
    #[test]
    fn the_tabs_are_named_and_ordered() {
        assert_eq!(Tab::ALL.map(Tab::label), ["Description", "Versions", "Gallery"]);
    }
}
