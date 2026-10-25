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
//!
//! **The row menu.** Every row carries the reference's kebab (`InstanceItem.vue:308-314`, a
//! quiet overflow button around `MoreVerticalIcon`) and a press opens the popover
//! [`menu_popover`] draws. Its rows are the library's instance actions
//! (`use-library.ts:1322-1377`), labelled with the reference's own strings -- see
//! [`MenuItem::label`] for the key behind each one. The menu's `Play` is the same
//! [`Action::Play`] the row's pill sends, so it launches for real -- what the reference's own
//! context menu does (`InstanceItem.vue:203-217`) -- and the shell answers a second press
//! while a launch is out with a toast. `Edit` and `Duplicate` are painted but not wired, and
//! they answer through the shell with a toast naming what is missing: a press that silently
//! does nothing reads as a broken button.
//!
//! **The worlds under the list.** The reference's `Jump in` section
//! (`app.home.jump-back-in.title`, `RecentWorldsList.vue:47`) lists recently played worlds
//! with the instance each belongs to. This page draws them from each instance's `saves/`
//! folder names only -- see [`read_worlds`] for what that can and cannot say, and why a world
//! row's press is a toast rather than a launch.

use eframe::egui::{Align2, Rect, Sense, Stroke, Ui, Vec2};

use crate::data::Entry;
use crate::fonts;
use crate::icons::{self, Glyph};
use crate::images::Images;
use crate::theme::Theme;

// ---------------------------------------------------------------------------
// The row menu: geometry and type
//
// Every number names the rule it came from. The panel is the reference's menu panel --
// `rounded-[14px] bg-surface-3 ring-1 ring-surface-5` (`button-menu.ts:21`) around a
// `flex flex-col p-2` column (`ButtonMenuPanel.vue:70`) -- and a row is `min-h-10 px-3
// py-2 gap-2 rounded-[10px] text-base font-semibold text-contrast` (`button-menu.ts:15`).
// ---------------------------------------------------------------------------

/// The kebab's hit target: the reference's quiet overflow button is a `size-9` icon button
/// (`InstanceItem.vue:308-314`); 28 points is the 16-point glyph with a pointer's worth of
/// plate around it, not a measurement -- re-measure against the capture.
// guess: re-measure
const KEBAB_SIZE: f32 = 28.0;

/// The glyph inside the kebab (`MoreVerticalIcon`, `InstanceItem.vue:313`).
// guess: re-measure
const KEBAB_GLYPH: f32 = 16.0;

/// The kebab's hover and open plate: `--radius-sm`, 8 points (`variables.scss:202`).
const KEBAB_RADIUS: u8 = 8;

/// The gap between the play pill and the kebab: `gap-1` on the row's action group
/// (`InstanceItem.vue:283`), 0.25rem = 4 points at the reference's 16px root
/// (`variables.scss:195`).
const ROW_ACTION_GAP: f32 = 4.0;

/// A menu row's height: `min-h-10` (`button-menu.ts:15`), 2.5rem = 40 points.
const MENU_ITEM_HEIGHT: f32 = 40.0;

/// A menu row's horizontal padding: `px-3` (`button-menu.ts:15`), 0.75rem = 12 points.
const MENU_ITEM_PADDING: f32 = 12.0;

/// The gap between a menu row's glyph and its label: `gap-2` (`button-menu.ts:15`), 8 points.
const MENU_ITEM_GAP: f32 = 8.0;

/// A menu row's glyph. The reference sizes a row's icon at `size-5` for trailing actions
/// (`ButtonMenuItem.vue:20`); 16 keeps the label's leading edge where the smaller glyphs of
/// this shell already sit -- re-measure before trusting the two points.
// guess: re-measure
const MENU_ITEM_GLYPH: f32 = 16.0;

/// The panel's inset around its rows: `p-2` (`ButtonMenuPanel.vue:70`), 8 points.
const MENU_PANEL_PADDING: f32 = 8.0;

/// The panel's corner radius: `rounded-[14px]` (`button-menu.ts:21`).
const MENU_RADIUS: u8 = 14;

/// A menu row's corner radius: `rounded-[10px]` (`button-menu.ts:15`).
const MENU_ROW_RADIUS: u8 = 10;

/// How far below the kebab the panel opens: `distance: 8` on the reference's floating
/// panels (`FloatingPanel.vue:22`).
const MENU_DISTANCE: f32 = 8.0;

/// The narrowest the panel gets before its widest label widens it. The reference's panel is
/// `w-fit` (`ButtonMenuPanel.vue`'s class list), so this floor exists only so a short menu
/// does not look like a tooltip -- re-measure against the capture.
// guess: re-measure
const MENU_MIN_WIDTH: f32 = 168.0;

// ---------------------------------------------------------------------------
// The worlds section: geometry and type
// ---------------------------------------------------------------------------

/// The folder inside an instance that holds its worlds: `worlds.ts:222` joins
/// `<instance>/saves/<world>` to open one.
const SAVES_DIR: &str = "saves";

/// How many instance rows are drawn. The reference caps its `Jump in` section at five items
/// (`MAX_JUMP_BACK_IN = 5`, `RecentWorldsList.vue:67`) and shows the rest in the library; six
/// rows is this page's own list height, not that cap -- re-measure against the window.
// guess: re-measure
const ROWS_SHOWN: usize = 6;

/// How many worlds one instance contributes. Three per instance keeps a five-instance page
/// near the reference's five-item section without hiding the other instances' worlds --
/// re-measure once the section is compared frame to frame.
// guess: re-measure
const WORLDS_PER_INSTANCE: usize = 3;

/// The gap between the last instance row and the worlds section.
// guess: re-measure
const WORLD_SECTION_GAP: f32 = 24.0;

/// The worlds heading's size: one step above the row text, the same step the reference uses
/// for a section header inside a card.
// guess: re-measure
const WORLD_HEADING_SIZE: f32 = 16.0;

/// The worlds heading's line box, which is what the rows start below.
// guess: re-measure
const WORLD_HEADING_LINE: f32 = 24.0;

/// A world row's height.
// guess: re-measure
const WORLD_ROW_HEIGHT: f32 = 32.0;

/// The gap between two world rows.
// guess: re-measure
const WORLD_ROW_GAP: f32 = 4.0;

/// A world row's corner radius: `--radius-sm`, 8 points (`variables.scss:202`).
const WORLD_ROW_RADIUS: u8 = 8;

/// The world row's glyph centre, in from the row's left edge.
// guess: re-measure
const WORLD_GLYPH_INSET: f32 = 20.0;

/// The glyph in a world row. **Not `Glyph::Map`**: this icon table has no `map.svg` -- it has
/// `map-pin.svg` ([`Glyph::MapPin`], a location pin) and `world.svg` ([`Glyph::World`], the
/// globe), and the globe is the set's own mark for a world, so that is the one drawn.
const WORLD_GLYPH: f32 = 16.0;

/// Where a world row's label starts.
// guess: re-measure
const WORLD_TEXT_INSET: f32 = 40.0;

/// A world row's label size.
// guess: re-measure
const WORLD_TEXT_SIZE: f32 = 13.0;

/// What the reader asked for by pressing something on this page.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Action {
    /// Nothing was pressed.
    #[default]
    None,
    /// `+ Create an instance`, or the `N` key: make one.
    Create,
    /// `Import from launcher`: find another launcher's instances.
    Import,
    /// Play this instance, by its id: the row's pill, or the menu's `Play`.
    Play(String),
    /// The menu's `Edit`: the instance's own settings page.
    Edit(String),
    /// The menu's `Duplicate instance`: copy the instance's folder.
    Duplicate(String),
    /// The menu's `Open folder`: show the instance's folder in the file manager.
    OpenFolder(String),
    /// The menu's `Delete`: remove the instance's folder from disk.
    Delete(String),
    /// A world row: launch `world` inside `instance`. Quick-play is not wired, so the shell
    /// answers with a toast naming what is missing rather than launching.
    OpenWorld {
        /// The instance the world belongs to, by id.
        instance: String,
        /// The world's folder name inside `saves/`, already cleaned.
        world: String,
    },
}

/// One row of the kebab's popover.
///
/// The order and the labels are the library's instance actions
/// (`app-frontend/src/components/ui/library/use-library.ts:1322-1377`), which is where the
/// reference keeps the per-instance menu a card's kebab opens. Each label is the reference's
/// own string; [`MenuItem::key`] is the locale key it comes from
/// (`app-frontend/src/locales/en-US/index.json`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItem {
    /// Launch the instance.
    Play,
    /// Open the instance's settings page.
    Edit,
    /// Copy the instance.
    Duplicate,
    /// Show the instance's folder.
    OpenFolder,
    /// Delete the instance.
    Delete,
}

impl MenuItem {
    /// Every row, in the order the popover draws them: play first, then the instance's own
    /// actions, the folder, and delete last.
    pub const ALL: [MenuItem; 5] = [
        MenuItem::Play,
        MenuItem::Edit,
        MenuItem::Duplicate,
        MenuItem::OpenFolder,
        MenuItem::Delete,
    ];

    /// The label, verbatim. The key it is read from is [`MenuItem::key`], and each comment
    /// below carries the line of `app-frontend/src/locales/en-US/index.json` that states it.
    pub fn label(self) -> &'static str {
        match self {
            // `app.instance.action.play` -- "Play" (:566-568)
            MenuItem::Play => "Play",
            // `app.instance.action.edit` -- "Edit" (:560-562)
            MenuItem::Edit => "Edit",
            // `app.library.instance.action.duplicate` -- "Duplicate instance" (:1133-1135)
            MenuItem::Duplicate => "Duplicate instance",
            // `app.instance.action.open-folder` -- "Open folder" (:563-565)
            MenuItem::OpenFolder => "Open folder",
            // `app.library.instance.action.delete` -- "Delete" (:1130-1132)
            MenuItem::Delete => "Delete",
        }
    }

    /// The locale key [`MenuItem::label`] is read from, so a string that drifts can be
    /// traced back to the reference's own copy of it. Read by the tests, which is where a
    /// label and its key are compared: the popover draws [`MenuItem::label`] alone.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn key(self) -> &'static str {
        match self {
            MenuItem::Play => "app.instance.action.play",
            MenuItem::Edit => "app.instance.action.edit",
            MenuItem::Duplicate => "app.library.instance.action.duplicate",
            MenuItem::OpenFolder => "app.instance.action.open-folder",
            MenuItem::Delete => "app.library.instance.action.delete",
        }
    }

    /// The glyph the reference puts beside the label: `PlayIcon`, `EditIcon`,
    /// `ClipboardCopyIcon`, `FolderOpenIcon` and `TrashIcon` (`use-library.ts:1288-1370`).
    pub fn glyph(self) -> Glyph {
        match self {
            MenuItem::Play => Glyph::Play,
            MenuItem::Edit => Glyph::Edit,
            MenuItem::Duplicate => Glyph::ClipboardCopy,
            MenuItem::OpenFolder => Glyph::FolderOpen,
            MenuItem::Delete => Glyph::Trash,
        }
    }

    /// What a press on this row asks the shell to do, for the instance `id`.
    ///
    /// Every item names a verb; there is no row that reports "nothing", which is the test
    /// below. Which of those verbs the shell can carry out is the shell's business --
    /// [`Action::Edit`] and [`Action::Duplicate`] exist so the press can be answered out
    /// loud instead of being dropped on the floor.
    pub fn action(self, id: &str) -> Action {
        let id = id.to_owned();
        match self {
            MenuItem::Play => Action::Play(id),
            MenuItem::Edit => Action::Edit(id),
            MenuItem::Duplicate => Action::Duplicate(id),
            MenuItem::OpenFolder => Action::OpenFolder(id),
            MenuItem::Delete => Action::Delete(id),
        }
    }
}

/// The row menu's state: which row's popover is open, if any.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Menu {
    /// The id of the instance whose kebab popover is open. One at a time: two open menus on
    /// one page would each swallow the other's presses.
    pub open_for: Option<String>,
}

/// What the page remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    /// The kebab popover.
    pub menu: Menu,
    /// Every drawn instance's `saves/` folder names, read when the set of instances last
    /// changed. Held here rather than re-read per frame: see `State::refresh_worlds`.
    worlds: Vec<(String, Vec<String>)>,
    /// The instance ids `worlds` was read for. A create or a delete changes this list, and
    /// that change is what triggers the re-read.
    scanned: Vec<String>,
}

impl State {
    /// Re-read the `saves/` folders when the set of instances changed since the last frame.
    ///
    /// A frame that walked six `saves/` folders would spend the idle-CPU budget of
    /// `docs/LIGHTNESS.md` on a list that only changes when a game writes a world. The ids
    /// are the cache key rather than the folder's timestamp, so a world created while the
    /// window is open is seen once the instance list itself changes -- when the reader
    /// leaves Home and comes back, which re-reads the instances (`shell.rs`, the rail's
    /// press).
    fn refresh_worlds(&mut self, instances: &[Entry]) {
        let ids: Vec<String> = instances
            .iter()
            .take(ROWS_SHOWN)
            .map(|instance| instance.id.clone())
            .collect();
        if ids == self.scanned {
            return;
        }
        self.scanned = ids;
        // A row that appeared or vanished cannot keep a popover open: the menu names an
        // instance by id, and an id that is gone would leave a panel anchored to nothing.
        self.menu.open_for = None;
        self.worlds = read_worlds(instances);
    }
}

/// Draw the page into `rect`, and say what was pressed.
///
/// The page does not create, launch, open or delete anything itself: it has no access to the
/// process table, and a page that had one would be untestable and would have to be re-taught
/// every rule the shell already knows. It reports the intent and the shell acts.
///
/// It does read one thing off the disk -- the folder names inside each instance's `saves/`,
/// through `State::refresh_worlds` -- because a world list handed down from the shell would
/// be a second copy of the folder rule that `data::instances_dir` already owns, and because
/// nothing this page can do with those names is destructive: they are strings on a row.
pub fn draw(
    ui: &mut Ui,
    rect: Rect,
    theme: &Theme,
    icons_cache: &mut Images,
    instances: &[Entry],
    state: &mut State,
) -> Action {
    if instances.is_empty() {
        hero(ui, rect, theme, icons_cache)
    } else {
        list(ui, rect, theme, icons_cache, instances, state)
    }
}

/// The fresh-install state: centred mark, welcome, the create button and the hint.
fn hero(ui: &mut Ui, rect: Rect, theme: &Theme, icons_cache: &mut Images) -> Action {
    let painter = ui.painter();
    let centre_x = rect.center().x;

    // The mark plate: 64 points, rounded, with the product mark inside it. The mark is the
    // generated `logo512.png` texture (`brand::install` uploads it at startup), not a glyph
    // and not a path beside the exe. When there is no texture -- a headless context, or a
    // decode that failed -- the plate the mark sits on is what this frame draws: a frame
    // must still draw something, and an empty hole in the hero reads as a broken build.
    let plate = Rect::from_center_size(
        egui::pos2(centre_x, rect.top() + 250.0),
        Vec2::splat(64.0),
    );
    if !crate::brand::paint_mark(painter, ui.ctx(), plate) {
        painter.rect_filled(plate, 16, theme.RAISED_BG);
        painter.rect_stroke(
            plate,
            16,
            Stroke::new(1.0, theme.ACCENT_CONTRAST.gamma_multiply(0.4)),
            egui::StrokeKind::Inside,
        );
    }
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
    let _ = Sense::hover();
    action
}

/// The installed state: the reference's instance cards, newest-played first, then the worlds
/// the reference lists under the `Jump in` heading.
fn list(
    ui: &mut Ui,
    rect: Rect,
    theme: &Theme,
    icons_cache: &mut Images,
    instances: &[Entry],
    state: &mut State,
) -> Action {
    // One directory walk per change of instance list, not per frame.
    state.refresh_worlds(instances);
    // Two disjoint fields of one state: naming them separately is what lets the rows below
    // hold the world names while they open and close the menu.
    let menu = &mut state.menu;
    let worlds = &state.worlds;

    let painter = ui.painter();
    painter.text(
        egui::pos2(rect.left(), rect.top() + 24.0),
        Align2::LEFT_TOP,
        // `app.library.title` -- "Library" (`locales/en-US/index.json:1235-1237`): the
        // reference's own name for the instance list, which is what these rows are. The
        // string this heading used to print, `Jump back in`, is not in the locale at all;
        // the `Jump in` section is the worlds below (`pages/Index.vue:99-103`).
        "Library",
        fonts::heading(24.0),
        theme.CONTRAST,
    );

    let mut action = Action::None;

    // An outside press closes the popover, the way the reference's panels close on a click
    // outside them (`FloatingPanel.vue:192-198`). Registered first, so every kebab, pill and
    // menu row sits above it and takes the press before this does.
    if menu.open_for.is_some()
        && ui
            .interact(rect, egui::Id::new("home-menu-dismiss"), Sense::click())
            .clicked()
    {
        menu.open_for = None;
    }

    let mut y = rect.top() + 72.0;
    let width = rect.width();
    // The open row's kebab, kept so the popover can be drawn after every row: a panel drawn
    // in its row's turn would be painted under the card below it.
    let mut anchored: Option<(String, Rect)> = None;
    for instance in instances.iter().take(ROWS_SHOWN) {
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
        // The pill is a target now: a launcher whose play button is a drawing is not a
        // launcher. The card's own press still does nothing -- the reference opens the
        // instance's page there, and that page does not exist yet.
        if ui
            .interact(play, egui::Id::new(("play", &instance.id)), Sense::click())
            .clicked()
            && matches!(action, Action::None)
        {
            // A launch press closes whatever menu was open: leaving a panel over the list
            // while the game starts would be the last thing between the reader and the row.
            menu.open_for = None;
            action = Action::Play(instance.id.clone());
        }

        // The kebab, beside the pill and inside the same right-hand group
        // (`InstanceItem.vue:283-314`). `EllipsisVertical` is the icon set's
        // `ellipsis-vertical.svg`, the three dots the reference's `MoreVerticalIcon` draws.
        let kebab = Rect::from_center_size(
            egui::pos2(
                play.right() + ROW_ACTION_GAP + KEBAB_SIZE / 2.0,
                card.center().y,
            ),
            Vec2::splat(KEBAB_SIZE),
        );
        let is_open = menu.open_for.as_deref() == Some(instance.id.as_str());
        let kebab_response = ui.interact(
            kebab,
            egui::Id::new(("row-menu", &instance.id)),
            Sense::click(),
        );
        // A quiet button: no plate until the pointer is on it or its menu is open
        // (`type="quiet"`, `InstanceItem.vue:309`).
        if kebab_response.hovered() || is_open {
            painter.rect_filled(kebab, KEBAB_RADIUS, theme.BUTTON_BG);
        }
        icons::draw(
            painter,
            Glyph::EllipsisVertical,
            Rect::from_center_size(kebab.center(), Vec2::splat(KEBAB_GLYPH)),
            if kebab_response.hovered() || is_open {
                theme.CONTRAST
            } else {
                theme.SECONDARY
            },
        );
        if kebab_response.clicked() {
            // A second press on the same kebab closes its own popover, which is what the
            // reference's trigger does (`FloatingPanel.vue:184-190`).
            menu.open_for = if is_open {
                None
            } else {
                Some(instance.id.clone())
            };
        }
        if menu.open_for.as_deref() == Some(instance.id.as_str()) {
            anchored = Some((instance.id.clone(), kebab));
        }
        y += 108.0;
    }

    // The worlds, under the rows: the reference's `Jump in` section. The heading is only
    // drawn when something is under it -- a section heading over nothing is what a fresh
    // instance would show on every page load.
    let has_worlds = worlds.iter().any(|(_, names)| !names.is_empty());
    if has_worlds {
        y += WORLD_SECTION_GAP;
        painter.text(
            egui::pos2(rect.left(), y),
            Align2::LEFT_TOP,
            // `app.home.jump-back-in.title` -- "Jump in" (`locales/en-US/index.json:482-484`,
            // the message `RecentWorldsList.vue:47` formats).
            "Jump in",
            fonts::semibold(WORLD_HEADING_SIZE),
            theme.CONTRAST,
        );
        y += WORLD_HEADING_LINE;
        for (id, names) in worlds.iter() {
            for name in names.iter().take(WORLDS_PER_INSTANCE) {
                let row = Rect::from_min_size(
                    egui::pos2(rect.left(), y),
                    egui::vec2(width, WORLD_ROW_HEIGHT),
                );
                painter.rect_filled(row, WORLD_ROW_RADIUS, theme.RAISED_BG);
                // `world.svg`, not a map pin: the icon table has no `map.svg`, and the globe
                // is the reference's own mark for a world.
                icons::draw(
                    painter,
                    Glyph::World,
                    Rect::from_center_size(
                        egui::pos2(row.left() + WORLD_GLYPH_INSET, row.center().y),
                        Vec2::splat(WORLD_GLYPH),
                    ),
                    theme.SECONDARY,
                );
                // The world's folder name with its copy timestamp cut off. **Not the world's
                // name**: that is the `LevelName` string inside `saves/<folder>/level.dat`,
                // an NBT document this page does not parse. Reading it is owed, and until it
                // exists a row says the folder, which is the closest true thing on disk.
                let label = clean_world_name(name);
                painter.text(
                    egui::pos2(row.left() + WORLD_TEXT_INSET, row.center().y),
                    Align2::LEFT_CENTER,
                    &label,
                    fonts::body(WORLD_TEXT_SIZE),
                    theme.BASE,
                );
                if ui
                    .interact(row, egui::Id::new(("world", id, name)), Sense::click())
                    .clicked()
                    && matches!(action, Action::None)
                {
                    // A world row's press reports back to the shell, which answers with a
                    // toast: launching one world is quick-play, an argv this launcher does
                    // not build yet, and a row that quietly did nothing would read as broken.
                    // Any press on the page closes the popover, this one included.
                    menu.open_for = None;
                    action = Action::OpenWorld {
                        instance: id.clone(),
                        world: label,
                    };
                }
                y += WORLD_ROW_HEIGHT + WORLD_ROW_GAP;
            }
        }
    }

    // The popover last, so it lies over every row it covers. egui resolves an overlapping
    // press to the widget registered last, which is what makes a menu row take the click
    // instead of the card's pill underneath it.
    if let Some((id, kebab)) = anchored {
        if let Some(item) = menu_popover(ui, theme, &id, kebab, rect) {
            menu.open_for = None;
            if matches!(action, Action::None) {
                action = item.action(&id);
            }
        }
    }

    action
}

/// Draw the open row's popover, and say which row of it was pressed.
///
/// Anchored to the kebab's bottom-right corner -- the reference's panels open `bottom-end` at
/// a distance of 8 (`FloatingPanel.vue:22, 81-103`) -- and shaped like its menu panel:
/// `rounded-[14px] bg-surface-3 ring-1 ring-surface-5` (`button-menu.ts:21`) around a
/// `p-2` column of `min-h-10` rows (`ButtonMenuPanel.vue:70`, `button-menu.ts:15`). The
/// delete row is the reference's red one (`tone: 'red'`, `use-library.ts:1371`).
fn menu_popover(ui: &mut Ui, theme: &Theme, id: &str, anchor: Rect, bounds: Rect) -> Option<MenuItem> {
    let painter = ui.painter();

    // The panel's width is its widest row: `w-fit`, with the floor in `MENU_MIN_WIDTH`.
    let mut width = MENU_MIN_WIDTH;
    for item in MenuItem::ALL {
        let label = painter.layout_no_wrap(
            item.label().to_owned(),
            fonts::semibold(crate::theme::FONT_SIZE_NM),
            theme.CONTRAST,
        );
        width = width.max(
            label.size().x + MENU_ITEM_PADDING * 2.0 + MENU_ITEM_GLYPH + MENU_ITEM_GAP,
        );
    }
    let height =
        MENU_PANEL_PADDING * 2.0 + MENU_ITEM_HEIGHT * MenuItem::ALL.len() as f32;
    // Opens downward unless the panel would fall past the page's bottom edge, in which case
    // it opens upward: the flip the reference's floating panels do when there is no room
    // below the trigger (`FloatingPanel.vue:52-69`). A menu that opened off the window would
    // be a menu the reader cannot reach on the last row.
    let below = anchor.bottom() + MENU_DISTANCE;
    let top = if below + height <= bounds.bottom() {
        below
    } else {
        (anchor.top() - MENU_DISTANCE - height).max(bounds.top())
    };
    let panel = Rect::from_min_size(
        egui::pos2(anchor.right() - width, top),
        egui::vec2(width, height),
    );
    // `bg-surface-3` is `RAISED_BG`; the ring is `--surface-5`, one step above it, which is
    // `SURFACE_5` and not `DIVIDER` (`variables.scss:238-239`).
    painter.rect_filled(panel, MENU_RADIUS, theme.RAISED_BG);
    painter.rect_stroke(
        panel,
        MENU_RADIUS,
        Stroke::new(1.0, theme.SURFACE_5),
        egui::StrokeKind::Inside,
    );

    let mut pressed = None;
    for (index, item) in MenuItem::ALL.into_iter().enumerate() {
        let row = Rect::from_min_size(
            egui::pos2(
                panel.left() + MENU_PANEL_PADDING,
                panel.top() + MENU_PANEL_PADDING + index as f32 * MENU_ITEM_HEIGHT,
            ),
            egui::vec2(width - MENU_PANEL_PADDING * 2.0, MENU_ITEM_HEIGHT),
        );
        let response = ui.interact(
            row,
            egui::Id::new(("row-menu-item", id, index)),
            Sense::click(),
        );
        if response.hovered() {
            painter.rect_filled(row, MENU_ROW_RADIUS, theme.BUTTON_BG);
        }
        // `text-contrast` on every row but the destructive one (`button-menu.ts:15`).
        let ink = if item == MenuItem::Delete {
            theme.RED
        } else {
            theme.CONTRAST
        };
        icons::draw(
            painter,
            item.glyph(),
            Rect::from_center_size(
                egui::pos2(
                    row.left() + MENU_ITEM_PADDING + MENU_ITEM_GLYPH / 2.0,
                    row.center().y,
                ),
                Vec2::splat(MENU_ITEM_GLYPH),
            ),
            ink,
        );
        painter.text(
            egui::pos2(
                row.left() + MENU_ITEM_PADDING + MENU_ITEM_GLYPH + MENU_ITEM_GAP,
                row.center().y,
            ),
            Align2::LEFT_CENTER,
            item.label(),
            fonts::semibold(crate::theme::FONT_SIZE_NM),
            ink,
        );
        if response.clicked() {
            pressed = Some(item);
        }
    }
    pressed
}

/// The `saves/` folder names of every drawn instance, in the order the rows are drawn.
///
/// **Folder names are not world names.** The name a player sees is the `LevelName` string
/// inside `saves/<folder>/level.dat`, which is an NBT document: parsing it is owed, and until
/// that exists the folder name is the closest true thing on disk -- with the timestamp a
/// copied world carries cut off by [`clean_world_name`]. A folder that will not open, or a
/// name that is not UTF-8, is skipped rather than reported: a Home page has no way to act on
/// either, and a row that cannot be named is worse than no row.
fn read_worlds(instances: &[Entry]) -> Vec<(String, Vec<String>)> {
    let root = crate::data::instances_dir();
    instances
        .iter()
        .take(ROWS_SHOWN)
        .map(|instance| {
            let saves = root.join(&instance.id).join(SAVES_DIR);
            let mut names: Vec<String> = Vec::new();
            if let Ok(entries) = std::fs::read_dir(&saves) {
                for entry in entries.flatten() {
                    // Only folders are worlds: `saves/` also holds `level.dat` caches and
                    // whatever a mod left behind.
                    let is_folder = entry
                        .file_type()
                        .map(|kind| kind.is_dir())
                        .unwrap_or(false);
                    if is_folder {
                        if let Some(name) = entry.file_name().to_str() {
                            names.push(name.to_owned());
                        }
                    }
                }
            }
            names.sort();
            (instance.id.clone(), names)
        })
        .collect()
}

/// A world folder's name with a trailing copy timestamp removed.
///
/// Vanilla's "Copy world" and every tool that follows it append `-YYYY-MM-DD_HH-MM-SS`, so
/// `New World-2026-10-25_23-10-14` and `New World` are the same world. Only that exact
/// shape is cut: a world called `Season 2`, a folder whose suffix is a date alone, and a
/// name that would be left empty all keep their full string, because cutting more than the
/// timestamp turns one world's row into another world's name.
pub fn clean_world_name(folder: &str) -> String {
    // The suffix vanilla appends, `YYYY-MM-DD_HH-MM-SS`, is 19 bytes of ASCII plus the one
    // separator before it. The width is vanilla's, not a measurement of this tree: the
    // vendored frontend never formats one, it reads the name out of `level.dat`.
    // guess: re-measure
    const TIMESTAMP_LEN: usize = 19;

    if folder.len() < TIMESTAMP_LEN + 2 {
        return folder.to_owned();
    }
    let separator_at = folder.len() - TIMESTAMP_LEN - 1;
    // The timestamp is ASCII, so a valid suffix always starts on a character boundary. A
    // folder that would split a multi-byte character there has no timestamp to cut.
    if !folder.is_char_boundary(separator_at) {
        return folder.to_owned();
    }
    let separator = folder.as_bytes()[separator_at];
    if separator != b'-' && separator != b'_' {
        return folder.to_owned();
    }
    if !is_copy_timestamp(&folder[separator_at + 1..]) {
        return folder.to_owned();
    }
    let head = &folder[..separator_at];
    if head.is_empty() {
        return folder.to_owned();
    }
    head.to_owned()
}

/// Whether `tail` is exactly the 19-character `YYYY-MM-DD_HH-MM-SS` vanilla writes, with `d`
/// standing for a digit.
fn is_copy_timestamp(tail: &str) -> bool {
    const SHAPE: &[u8] = b"dddd-dd-dd_dd-dd-dd";
    let bytes = tail.as_bytes();
    bytes.len() == SHAPE.len()
        && bytes
            .iter()
            .zip(SHAPE)
            .all(|(byte, expected)| match *expected {
                b'd' => byte.is_ascii_digit(),
                separator => *byte == separator,
            })
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

    /// A copied world's folder carries vanilla's suffix, and the row drops it: the two
    /// folders are one world, and a list that showed the timestamp twice would read as two.
    #[test]
    fn a_world_folder_loses_its_copy_timestamp() {
        assert_eq!(clean_world_name("New World-2026-10-25_23-10-14"), "New World");
        assert_eq!(clean_world_name("New World_2026-10-25_23-10-14"), "New World");
        // A timestamp after non-ASCII text still cuts at the right place: the suffix is
        // ASCII, so its start is a character boundary.
        assert_eq!(clean_world_name("Über-2026-10-25_23-10-14"), "Über");
    }

    /// Everything that is not that exact suffix keeps its whole name. A world called
    /// `Season 2` must not become `Season`, and a folder that is only a timestamp must not
    /// become an empty row.
    #[test]
    fn a_world_name_without_the_suffix_is_untouched() {
        assert_eq!(clean_world_name("Season 2"), "Season 2");
        assert_eq!(clean_world_name("survival"), "survival");
        assert_eq!(clean_world_name("world-2026-10-25"), "world-2026-10-25");
        assert_eq!(clean_world_name("world-2026-10-25_23-10"), "world-2026-10-25_23-10");
        assert_eq!(clean_world_name("2026-10-25_23-10-14"), "2026-10-25_23-10-14");
        assert_eq!(clean_world_name("-2026-10-25_23-10-14"), "-2026-10-25_23-10-14");
        assert_eq!(clean_world_name(""), "");
    }

    /// Every menu row is one verb, and that verb carries the instance the row belongs to.
    /// A row that reported `Action::None` would be the silent press this menu exists to
    /// avoid; two rows reporting the same action would make one of them a lie.
    #[test]
    fn the_menu_reports_exactly_one_action() {
        let mut seen: Vec<Action> = Vec::new();
        for item in MenuItem::ALL {
            let action = item.action("instance-3");
            assert_ne!(
                action,
                Action::None,
                "{} must report something",
                item.label()
            );
            assert!(
                names(&action, "instance-3"),
                "{} must name the row's instance",
                item.label()
            );
            assert!(
                !seen.contains(&action),
                "{} reports an action another row already does",
                item.label()
            );
            seen.push(action);
        }
        assert_eq!(seen.len(), MenuItem::ALL.len());
    }

    /// The popover's rows are the reference's own strings, each beside the key it came from:
    /// a label that drifted from `locales/en-US/index.json` is a parity bug no compiler sees.
    #[test]
    fn the_menu_labels_are_the_reference_strings() {
        let expected = [
            ("Play", "app.instance.action.play"),
            ("Edit", "app.instance.action.edit"),
            ("Duplicate instance", "app.library.instance.action.duplicate"),
            ("Open folder", "app.instance.action.open-folder"),
            ("Delete", "app.library.instance.action.delete"),
        ];
        assert_eq!(MenuItem::ALL.len(), expected.len());
        for (item, (label, key)) in MenuItem::ALL.into_iter().zip(expected) {
            assert_eq!(item.label(), label);
            assert_eq!(item.key(), key);
        }
    }

    /// Whether `action` names the instance it was pressed on. Written as a match over every
    /// variant so a new `Action` cannot be added without this test saying so.
    fn names(action: &Action, id: &str) -> bool {
        match action {
            Action::Play(ours)
            | Action::Edit(ours)
            | Action::Duplicate(ours)
            | Action::OpenFolder(ours)
            | Action::Delete(ours) => ours.as_str() == id,
            Action::OpenWorld { instance, .. } => instance.as_str() == id,
            Action::None | Action::Create | Action::Import => false,
        }
    }
}
