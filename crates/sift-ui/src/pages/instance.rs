//! One instance's page: its header, its own tab strip, and the overview its landing tab opens
//! on.
//!
//! Follows `vendor/modrinth-app/app-frontend/src/pages/instance/layout.vue`: the header block
//! (`p-6 pr-2 pb-4`, lines 3-6) carries `InstancePageHeader` (lines 32-55), the tab strip is
//! `NavTabs` (line 58), and the tab body is a `RouterView` under it (lines 72-84). The facts the
//! header states -- loader and Minecraft version, last played -- come from
//! `pages/instance/components/page-header/index.vue:44-68`, which joins the loader and the game
//! version with one space (`:325-327`).
//!
//! **There is no `Overview` tab in this reference, and no `Settings` tab either.** The tabs are
//! Content, Files, Screenshots, Worlds, Logs, in that order (`layout.vue:476-524`), and Content
//! is the landing route (`routes.js:121-125`); this page draws the overview on it. The
//! instance's own facts live in the header, and settings is the modal the header and the menu's
//! `Edit` open (`layout.vue:50`, `:807-812`), not a tab. Files, Screenshots and Worlds are
//! app-settings gates (`layout.vue:485`, `:493`, `:501`; defaults in
//! `composables/use-app-settings.ts:53-55`, where Screenshots is off), and `Share`
//! (`app.instance.tab.share`) is only drawn when shared instances are available
//! (`layout.vue:464-474`, `:515-521`) -- a launcher without sharing never shows it, so this
//! page does not draw it.
//!
//! The tabs below Content are scaffolds: a heading, and one sentence naming what the reference
//! draws there. Those sentences are this file's own; the catalogue's nearest empty states
//! (`app.instance.worlds.no-worlds-heading`, `app.screenshots.empty-heading`) claim that
//! nothing is installed, which this page has not read, so they wait until the tabs do.

use eframe::egui::{self, Align2, Rect, Sense, Ui};

use crate::fonts;
use crate::icons::{self, Glyph};
use crate::images::Images;
use crate::theme::{Theme, RADIUS_MD, RADIUS_SM};

/// The header block's height, above the tab strip. Measured on the reference at 1440x900 for
/// the project page (`pages/project.rs:194-197`); the instance header is the same `p-6 pb-4`
/// block (`layout.vue:3-6`).
const HEADER_HEIGHT: f32 = 84.0;

/// The tab strip's height, as the project page measures it (`pages/project.rs:194-197`).
const TAB_STRIP_HEIGHT: f32 = 36.0;

/// A tab pill's height (`pages/project.rs:199-204`).
const TAB_HEIGHT: f32 = 30.0;

/// A tab pill's horizontal padding (`pages/project.rs:199-204`).
const TAB_PAD_X: f32 = 28.0;

/// A tab pill's corner radius: half its height, so the ends are round
/// (`pages/project.rs:207`).
const TAB_RADIUS: u8 = 15;

/// The gap between tab pills (`pages/project.rs:218`).
const TAB_GAP: f32 = 4.0;

/// The gap between the tab strip and the card under it (`pages/project.rs:223-226`).
const CARD_GAP: f32 = 14.0;

/// The card's inner padding: the reference's `p-6` on the tab body (`layout.vue:72`).
const PAD: f32 = 24.0;

/// One row of the Files and Logs lists: a 20-point line with room to breathe.
const ROW_H: f32 = 20.0;

/// The instance name's size: the reference's page-header title is `text-2xl`
/// (`ui/src/components/base/page-header/index.vue:13`).
const NAME_SIZE: f32 = 24.0;

/// The name's top inset.
// guess: re-measure
const NAME_TOP: f32 = 4.0;

/// The 13-point body size the pages share: the project header's summary
/// (`pages/project.rs:117`), its tab labels (`:200`) and its empty-tab text (`:297`).
const BODY_SIZE: f32 = 13.0;

/// The loader line's top inset under the name.
// guess: re-measure
const SUBTITLE_TOP: f32 = 36.0;

/// The path line's size, the project header's stat size (`pages/project.rs:137`).
const PATH_SIZE: f32 = 12.0;

/// The path line's top inset.
// guess: re-measure
const PATH_TOP: f32 = 56.0;

/// The play button's label size (`pages/project.rs:184`).
const BUTTON_TEXT_SIZE: f32 = 14.0;

/// A section heading inside the card, the project sidebar's heading size
/// (`pages/project.rs:312`).
const SECTION_SIZE: f32 = 15.0;

/// An information row's label size (`pages/project.rs:321`).
const ROW_LABEL_SIZE: f32 = 12.0;

/// The play button's size, the project page's install button (`pages/project.rs:166-169`).
const PLAY_WIDTH: f32 = 96.0;
/// The play button's height (`pages/project.rs:166-169`).
const PLAY_HEIGHT: f32 = 40.0;

/// The icon box inside a button (`pages/project.rs:174-177`).
const BUTTON_ICON: f32 = 16.0;

/// The button icon's centre inset from the button's left edge (`pages/project.rs:174-177`).
const BUTTON_ICON_INSET: f32 = 22.0;

/// The label's centre offset inside a button, so the icon and text balance
/// (`pages/project.rs:181`).
const BUTTON_TEXT_SHIFT: f32 = 8.0;

/// A label's line advance in a stacked row (`pages/project.rs:324`).
const LABEL_ADVANCE: f32 = 20.0;

/// The gap under an information row.
// guess: re-measure
const ROW_GAP: f32 = 14.0;

/// The description column's share of the card's width.
// guess: re-measure
const LEFT_SHARE: f32 = 0.6;

/// The gap between the overview's two columns.
// guess: re-measure
const COLUMN_GAP: f32 = 24.0;

/// The heading-to-body offset inside a stub tab, as the project header places its summary
/// under the title (`pages/project.rs:122`).
const HEADING_ADVANCE: f32 = 26.0;

/// What the page shows: one installed instance, as the shell lists it.
#[derive(Debug, Clone)]
pub struct Instance {
    /// The instance's id, as the shell keys it.
    pub id: String,
    /// The name the header prints.
    pub name: String,
    /// The mod loader's display name, such as `Fabric`.
    pub loader: String,
    /// The Minecraft version the instance launches.
    pub version: String,
    /// The instance's folder on disk.
    pub path: std::path::PathBuf,
}

/// The page's own tabs, in the reference's order. `Content` is the landing tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    /// The instance's installed content.
    #[default]
    Content,
    /// Its folder, file by file.
    Files,
    /// The screenshots taken in it.
    // shortcut: the reference leaves this tab off by default (`use-app-settings.ts:55`); it is
    // drawn until the shell has a display-settings page that can hide it.
    Screenshots,
    /// Its worlds and servers.
    Worlds,
    /// Its log files.
    Logs,
}

impl Tab {
    /// The five tabs the reference's strip draws, in its order (`layout.vue:476-524`).
    ///
    /// `Share` (`app.instance.tab.share`, `index.json:860-862`) is a sixth the reference only
    /// adds when shared instances are available (`layout.vue:464-474`), so it is not here.
    pub const ALL: [Tab; 5] = [
        Tab::Content,
        Tab::Files,
        Tab::Screenshots,
        Tab::Worlds,
        Tab::Logs,
    ];

    /// The tab's label, verbatim from
    /// `vendor/modrinth-app/app-frontend/src/locales/en-US/index.json`.
    pub fn label(self) -> &'static str {
        match self {
            // `app.instance.tab.content` (:848-850).
            Tab::Content => "Content",
            // `app.instance.tab.files` (:851-853).
            Tab::Files => "Files",
            // `app.instance.tab.screenshots` (:857-859).
            Tab::Screenshots => "Screenshots",
            // `app.instance.tab.worlds` (:863-865).
            Tab::Worlds => "Worlds",
            // `app.instance.tab.logs` (:854-856).
            Tab::Logs => "Logs",
        }
    }
}

/// The page's own state: which tab is showing.
#[derive(Debug, Clone, Copy, Default)]
pub struct State {
    /// The selected tab.
    pub tab: Tab,
}

/// Draw the page's content column.
pub fn draw(
    ui: &mut Ui,
    rect: Rect,
    theme: &Theme,
    images: &mut Images,
    state: &mut State,
    instance: &Instance,
) -> bool {
    // shortcut: the header has no icon tile, because `Instance` carries no icon path; add one
    // when the shell's instance list has an icon for it to pass in.
    let _ = images;

    let painter = ui.painter();

    // ---- Header: the name, the loader and version, the path, and Play ----
    painter.text(
        egui::pos2(rect.left(), rect.top() + NAME_TOP),
        Align2::LEFT_TOP,
        instance.name.as_str(),
        fonts::semibold(NAME_SIZE),
        theme.CONTRAST,
    );
    painter.text(
        egui::pos2(rect.left(), rect.top() + SUBTITLE_TOP),
        Align2::LEFT_TOP,
        loader_line(instance),
        fonts::body(BODY_SIZE),
        theme.SECONDARY,
    );
    painter.text(
        egui::pos2(rect.left(), rect.top() + PATH_TOP),
        Align2::LEFT_TOP,
        instance.path.display().to_string(),
        fonts::body(PATH_SIZE),
        theme.SECONDARY,
    );

    // The play button is painted here and wired by the shell, as the project page paints its
    // install button. The reference's is the brand `size="xl"` button in the header's actions
    // (`page-header/index.vue:132-142`); its label is `app.instance.action.play` (:566-568).
    let play = Rect::from_min_size(
        egui::pos2(rect.right() - PLAY_WIDTH, rect.top() + NAME_TOP),
        egui::vec2(PLAY_WIDTH, PLAY_HEIGHT),
    );
    painter.rect_filled(play, RADIUS_SM, theme.BUTTON_BG_SELECTED);
    icons::draw(
        painter,
        Glyph::Play,
        Rect::from_center_size(
            egui::pos2(play.left() + BUTTON_ICON_INSET, play.center().y),
            egui::vec2(BUTTON_ICON, BUTTON_ICON),
        ),
        theme.BUTTON_TEXT_SELECTED,
    );
    painter.text(
        egui::pos2(play.center().x + BUTTON_TEXT_SHIFT, play.center().y),
        Align2::CENTER_CENTER,
        "Play",
        fonts::semibold(BUTTON_TEXT_SIZE),
        theme.BUTTON_TEXT_SELECTED,
    );

    // ---- The page's own tab strip ----
    let tabs = Rect::from_min_size(
        egui::pos2(rect.left(), rect.top() + HEADER_HEIGHT),
        egui::vec2(rect.width(), TAB_STRIP_HEIGHT),
    );
    let mut tab_x = tabs.left();
    for tab in Tab::ALL {
        let label = painter.layout_no_wrap(tab.label().to_owned(), fonts::semibold(BODY_SIZE), theme.BASE);
        let pill = Rect::from_min_size(
            egui::pos2(tab_x, tabs.top()),
            egui::vec2(label.size().x + TAB_PAD_X, TAB_HEIGHT),
        );
        let active = tab == state.tab;
        if active {
            painter.rect_filled(pill, TAB_RADIUS, theme.BUTTON_BG_SELECTED);
        }
        painter.galley(
            egui::pos2(
                pill.center().x - label.size().x / 2.0,
                pill.center().y - label.size().y / 2.0,
            ),
            label.clone(),
            if active {
                theme.BUTTON_TEXT_SELECTED
            } else {
                theme.BASE
            },
        );
        // The whole pill is the hit target, so a press anywhere on it switches tabs.
        let response = ui.interact(
            pill,
            egui::Id::new(("instance-tab", tab.label())),
            Sense::click(),
        );
        if response.clicked() {
            state.tab = tab;
        }
        tab_x = pill.right() + TAB_GAP;
    }

    // ---- The tab's own card ----
    let card = Rect::from_min_size(
        egui::pos2(rect.left(), tabs.bottom() + CARD_GAP),
        egui::vec2(rect.width(), rect.bottom() - tabs.bottom() - CARD_GAP),
    );
    painter.rect_filled(card, RADIUS_MD, theme.RAISED_BG);
    match state.tab {
        Tab::Content => overview(ui, card, theme, instance),
        Tab::Files => files_tab(ui, card, theme, instance),
        Tab::Screenshots => screenshots_tab(ui, card, theme, images, instance),
        Tab::Worlds => worlds_tab(ui, card, theme, instance),
        Tab::Logs => logs_tab(ui, card, theme, instance),
    }
    // Nothing on this page animates, so the shell has no frame to schedule.
    false
}

/// The landing tab: the description and notes on the left, the information rows on the right.
fn overview(ui: &mut Ui, card: Rect, theme: &Theme, instance: &Instance) {
    let painter = ui.painter();
    let inner = card.shrink(PAD);
    let left_width = ((inner.width() - COLUMN_GAP) * LEFT_SHARE).max(0.0);
    let left = Rect::from_min_size(inner.min, egui::vec2(left_width, inner.height()));
    let right = Rect::from_min_size(
        egui::pos2(left.right() + COLUMN_GAP, inner.top()),
        egui::vec2(
            (inner.width() - left_width - COLUMN_GAP).max(0.0),
            inner.height(),
        ),
    );

    // The left column's heading is the catalogue's only "Description" string, the project
    // page's tab label (`app.project.tab.description`, `index.json:1433-1435`).
    painter.text(
        left.min,
        Align2::LEFT_TOP,
        "Description",
        fonts::semibold(SECTION_SIZE),
        theme.CONTRAST,
    );
    // owed: the instance's description and notes. The reference has no field for either on this
    // page -- its header carries the name, loader, playtime and last played, nothing else -- so
    // the column is reserved until the shell can supply text, starting with a modpack
    // instance's linked project description.

    // The right column: each row is its label, then its value wrapped to the column.
    let mut y = right.top();
    for (label, value) in info_rows(instance) {
        if y > right.bottom() {
            break;
        }
        painter.text(
            egui::pos2(right.left(), y),
            Align2::LEFT_TOP,
            label,
            fonts::body(ROW_LABEL_SIZE),
            theme.SECONDARY,
        );
        y += LABEL_ADVANCE;
        let value = painter.layout(value, fonts::label(BODY_SIZE), theme.BASE, right.width());
        let height = value.size().y;
        painter.galley(egui::pos2(right.left(), y), value, theme.BASE);
        y += height + ROW_GAP;
    }
}

/// A tab whose reference screen is not built yet: its own name as the heading, and one sentence
/// naming what will be there.
/// The Screenshots tab: the instance's screenshots, three to a row, newest first.
fn screenshots_tab(
    ui: &mut Ui,
    card: Rect,
    theme: &Theme,
    images: &mut Images,
    instance: &Instance,
) {
    let shots = screenshot_files(&instance.path);
    if shots.is_empty() {
        stub(
            ui,
            card,
            theme,
            Tab::Screenshots,
            "Screenshots taken in this instance will appear here.",
        );
        return;
    }
    let painter = ui.painter();
    let inner = card.shrink(PAD);
    let gap = 8.0;
    let tile_width = (inner.width() - 2.0 * gap) / 3.0;
    let tile_height = tile_width * 9.0 / 16.0;
    for (index, shot) in shots.iter().take(9).enumerate() {
        let column = (index % 3) as f32;
        let row = (index / 3) as f32;
        let tile = Rect::from_min_size(
            egui::pos2(
                inner.left() + column * (tile_width + gap),
                inner.top() + row * (tile_height + gap),
            ),
            egui::vec2(tile_width, tile_height),
        );
        if tile.bottom() > inner.bottom() {
            break;
        }
        painter.rect_filled(tile, 6, theme.SURFACE_2);
        if let Some(texture) = images.get_file(ui.ctx(), shot) {
            painter.image(
                texture.id(),
                tile,
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
    }
}

/// The Worlds tab: one row per save, the world's own name with its folder dim on the right.
fn worlds_tab(ui: &mut Ui, card: Rect, theme: &Theme, instance: &Instance) {
    let worlds = saves(&instance.path);
    if worlds.is_empty() {
        stub(
            ui,
            card,
            theme,
            Tab::Worlds,
            "This instance has no worlds yet. Create one in the game and it appears here.",
        );
        return;
    }
    let painter = ui.painter();
    let inner = card.shrink(PAD);
    let mut y = inner.top();
    for (folder, name) in worlds.iter().take(24) {
        if y + ROW_H > inner.bottom() {
            break;
        }
        icons::draw(
            painter,
            Glyph::World,
            Rect::from_center_size(
                egui::pos2(inner.left() + 8.0, y + ROW_H / 2.0),
                egui::vec2(14.0, 14.0),
            ),
            theme.SECONDARY,
        );
        painter.text(
            egui::pos2(inner.left() + 26.0, y + ROW_H / 2.0),
            Align2::LEFT_CENTER,
            name,
            fonts::body(12.0),
            theme.BASE,
        );
        painter.text(
            egui::pos2(inner.right(), y + ROW_H / 2.0),
            Align2::RIGHT_CENTER,
            folder,
            fonts::body(11.0),
            theme.SECONDARY,
        );
        y += ROW_H;
    }
}

/// The Files tab: the instance's folder, one row per entry.
fn files_tab(ui: &mut Ui, card: Rect, theme: &Theme, instance: &Instance) {
    let entries = folder_entries(&instance.path);
    if entries.is_empty() {
        stub(ui, card, theme, Tab::Files, "This instance's folder is empty.");
        return;
    }
    let painter = ui.painter();
    let inner = card.shrink(PAD);
    let mut y = inner.top();
    for (name, is_dir, size) in entries.iter().take(24) {
        if y + ROW_H > inner.bottom() {
            break;
        }
        icons::draw(
            painter,
            if *is_dir { Glyph::Folder } else { Glyph::File },
            Rect::from_center_size(
                egui::pos2(inner.left() + 8.0, y + ROW_H / 2.0),
                egui::vec2(14.0, 14.0),
            ),
            theme.SECONDARY,
        );
        painter.text(
            egui::pos2(inner.left() + 26.0, y + ROW_H / 2.0),
            Align2::LEFT_CENTER,
            name,
            fonts::body(12.0),
            theme.BASE,
        );
        if !*is_dir {
            painter.text(
                egui::pos2(inner.right(), y + ROW_H / 2.0),
                Align2::RIGHT_CENTER,
                format_size(*size),
                fonts::body(11.0),
                theme.SECONDARY,
            );
        }
        y += ROW_H;
    }
}

/// The Logs tab: the last screenful of the game's own log, newest at the bottom.
fn logs_tab(ui: &mut Ui, card: Rect, theme: &Theme, instance: &Instance) {
    let Some(text) = log_text(&instance.path) else {
        stub(
            ui,
            card,
            theme,
            Tab::Logs,
            "This instance has no log yet. Start it once and the game writes one.",
        );
        return;
    };
    let painter = ui.painter();
    let inner = card.shrink(PAD);
    let fits = ((inner.height() / ROW_H).floor() as usize).max(1);
    let mut y = inner.top();
    for line in tail_lines(&text, fits) {
        painter.text(
            egui::pos2(inner.left(), y),
            Align2::LEFT_TOP,
            line,
            fonts::body(11.0),
            theme.SECONDARY,
        );
        y += ROW_H;
    }
}

/// The instance's screenshots, newest first: the game names them by date.
fn screenshot_files(path: &std::path::Path) -> Vec<std::path::PathBuf> {
    for candidate in ["minecraft/screenshots", "screenshots"] {
        let folder = path.join(candidate);
        if !folder.is_dir() {
            continue;
        }
        let Ok(read) = std::fs::read_dir(&folder) else {
            continue;
        };
        let mut shots: Vec<std::path::PathBuf> = read
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                matches!(
                    path.extension().and_then(|extension| extension.to_str()),
                    Some("png" | "jpg" | "jpeg")
                )
            })
            .collect();
        shots.sort();
        shots.reverse();
        return shots;
    }
    Vec::new()
}

/// The instance's worlds: `(folder, name)`, the name from `level.dat` and the folder when it has
/// none -- the same fallback the game uses.
fn saves(path: &std::path::Path) -> Vec<(String, String)> {
    for candidate in ["minecraft/saves", "saves"] {
        let saves = path.join(candidate);
        if !saves.is_dir() {
            continue;
        }
        let Ok(read) = std::fs::read_dir(&saves) else {
            continue;
        };
        let mut worlds: Vec<(String, String)> = read
            .flatten()
            .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
            .filter_map(|entry| {
                let folder = entry.file_name().to_str()?.to_owned();
                let name = crate::nbt::world_name(&entry.path()).unwrap_or_else(|| folder.clone());
                Some((folder, name))
            })
            .collect();
        worlds.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
        return worlds;
    }
    Vec::new()
}

/// The last `count` lines of a log, which is what a log pane shows.
///
/// A real `latest.log` reaches megabytes and the pane can show a screenful, so the read is bounded
/// by the answer rather than by the file.
fn tail_lines(text: &str, count: usize) -> Vec<&str> {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(count);
    lines[start..].to_vec()
}

/// A byte count as the words a file browser uses: `842 B`, `1.4 KB`, `12.1 MB`, `1.2 GB`.
fn format_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let value = bytes as f64;
    if value < KB {
        format!("{bytes} B")
    } else if value < KB * KB {
        format!("{:.1} KB", value / KB)
    } else if value < KB * KB * KB {
        format!("{:.1} MB", value / (KB * KB))
    } else {
        format!("{:.1} GB", value / (KB * KB * KB))
    }
}

/// The instance folder's entries, folders first then files, each with its size.
fn folder_entries(path: &std::path::Path) -> Vec<(String, bool, u64)> {
    let Ok(read) = std::fs::read_dir(path) else {
        return Vec::new();
    };
    let mut entries: Vec<(String, bool, u64)> = read
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.to_owned();
            let metadata = entry.metadata().ok()?;
            let is_dir = metadata.is_dir();
            Some((name, is_dir, if is_dir { 0 } else { metadata.len() }))
        })
        .collect();
    // Folders first, then names, which is how every file browser a reader has used orders them.
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase())));
    entries
}

/// The text of the instance's newest log, or nothing when the game has not run.
fn log_text(path: &std::path::Path) -> Option<String> {
    for candidate in ["minecraft/logs/latest.log", "logs/latest.log"] {
        if let Ok(text) = std::fs::read_to_string(path.join(candidate)) {
            return Some(text);
        }
    }
    None
}

fn stub(ui: &mut Ui, card: Rect, theme: &Theme, tab: Tab, line: &str) {
    let painter = ui.painter();
    let inner = card.shrink(PAD);
    painter.text(
        inner.min,
        Align2::LEFT_TOP,
        tab.label(),
        fonts::semibold(SECTION_SIZE),
        theme.CONTRAST,
    );
    let line = painter.layout(
        line.to_owned(),
        fonts::body(BODY_SIZE),
        theme.SECONDARY,
        inner.width(),
    );
    painter.galley(
        egui::pos2(inner.left(), inner.top() + HEADING_ADVANCE),
        line,
        theme.SECONDARY,
    );
}

/// The loader and the game version on one line, joined exactly as the reference's `loaderLabel`
/// joins them: `[loader, game_version].filter(Boolean).join(' ')`
/// (`page-header/index.vue:325-327`).
fn loader_line(instance: &Instance) -> String {
    match (instance.loader.is_empty(), instance.version.is_empty()) {
        (true, _) => instance.version.clone(),
        (_, true) => instance.loader.clone(),
        _ => format!("{} {}", instance.loader, instance.version),
    }
}

/// The rows the reference's page header states -- loader and game version, then last played
/// (`page-header/index.vue:44-68`) -- plus the folder the page only offers as `Copy path`
/// (`layout.vue:821-826`). Each label is a catalogue string.
fn info_rows(instance: &Instance) -> [(&'static str, String); 4] {
    [
        // `app.library.filter.loader` (`index.json:1007-1009`).
        ("Loader", instance.loader.clone()),
        // `app.library.filter.game-version` (`index.json:989-991`).
        ("Game version", instance.version.clone()),
        // `instance.last-played` (`index.json:3935-3937`). The value is owed: the shell holds
        // the import's `last_played` stamp, and this page has no field for it yet.
        ("Last played", "\u{2014}".to_owned()),
        // `app.java-detection.columns.path` (`index.json:938-940`) is the catalogue's only
        // "Path" string; the instance page states its folder without a label of its own.
        ("Path", instance.path.display().to_string()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An instance with every field filled, so a test only has to change the one it is about.
    fn instance() -> Instance {
        Instance {
            id: "test-instance".to_owned(),
            name: "Test Instance".to_owned(),
            loader: "Fabric".to_owned(),
            version: "1.21.1".to_owned(),
            path: std::path::PathBuf::from("instances").join("test-instance"),
        }
    }

    /// The tabs are the reference's five, in its order, and each has a label: a tab that
    /// rendered an empty string would be an invisible target.
    #[test]
    fn the_tabs_are_named_and_ordered() {
        assert_eq!(
            Tab::ALL.map(Tab::label),
            ["Content", "Files", "Screenshots", "Worlds", "Logs"]
        );
    }

    /// The page opens on Content, which is the reference's landing route.
    #[test]
    fn an_instance_opens_on_its_content_tab() {
        assert_eq!(State::default().tab, Tab::Content);
    }

    /// The header's second line is the loader and the version, one space apart.
    #[test]
    fn the_loader_line_joins_loader_and_version() {
        assert_eq!(loader_line(&instance()), "Fabric 1.21.1");
    }

    /// An empty half is dropped rather than leaving a stray space, as the reference's
    /// `filter(Boolean)` does.
    #[test]
    fn the_loader_line_drops_an_empty_part() {
        let mut no_version = instance();
        no_version.version.clear();
        assert_eq!(loader_line(&no_version), "Fabric");

        let mut no_loader = instance();
        no_loader.loader.clear();
        assert_eq!(loader_line(&no_loader), "1.21.1");
    }

    /// The information rows are labelled and ordered as the reference's header states them,
    /// the path is the instance's own, and last played is still the owed placeholder.
    /// A log pane shows the end of the file, never the start, and never more lines than asked.
    #[test]
    fn a_tail_is_the_end_of_a_log() {
        let text = "one\ntwo\nthree\nfour";
        assert_eq!(tail_lines(text, 2), vec!["three", "four"]);
        assert_eq!(tail_lines(text, 99), vec!["one", "two", "three", "four"]);
        assert_eq!(tail_lines("", 5), Vec::<&str>::new());
    }

    /// A file browser's sizes read as words at every scale it meets.
    #[test]
    fn sizes_read_as_words() {
        assert_eq!(format_size(842), "842 B");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(12_700_000), "12.1 MB");
        assert_eq!(format_size(2_147_483_648), "2.0 GB");
    }

    #[test]
    fn the_information_rows_are_labelled_and_ordered() {
        assert_eq!(
            info_rows(&instance()).map(|(label, _)| label),
            ["Loader", "Game version", "Last played", "Path"]
        );
        let rows = info_rows(&instance());
        assert_eq!(rows[2].1, "\u{2014}");
        assert_eq!(rows[3].1, instance().path.display().to_string());
    }
}
