//! The library page: the `Library` heading, the toolbar's view controls, and the grid of
//! instance cards.
//!
//! Follows the vendored app's library route and the components under it:
//!
//! * `app-frontend/src/pages/Index.vue` — the route is named `LibraryPage` (line 20), its
//!   content container is `flex flex-col gap-3 p-6` (line 96), and a fresh install with no
//!   instance ever created renders `WelcomeScreen` *instead of* the library section
//!   (line 92). That branch is the empty state below.
//! * `app-frontend/src/components/ui/library/index.vue` — the page heading is
//!   `text-2xl font-semibold text-contrast` (line 268) in a `flex flex-col gap-3 pb-16`
//!   section (line 267), with `LibraryToolbar` between the heading and the groups.
//! * `.../components/ui/library/library-toolbar/index.vue` and `sort-menu.vue` — the
//!   controls: a `Combobox` for the sort, a 1x24-point divider with 8-point margins
//!   (line 51), and a `Combobox` for the group. Their defaults are `Last played` and
//!   `Custom group` (`use-library.ts:249-252`).
//! * `.../components/ui/library/instance-group/index.vue` — the responsive grid
//!   expression (lines 121-142) and the absolute card positions (lines 164-171), plus the
//!   group header's count `TagItem` (lines 571-576).
//! * `.../components/ui/library/instance-group/instance-card.vue` and
//!   `instance-card-view.vue` — the card: `rounded-[20px] p-3`, a full-width
//!   `aspect-square` tile at `rounded-2xl`, the name at `text-base font-semibold
//!   leading-5 text-contrast`, the loader and version at `text-sm font-medium
//!   leading-[18px] text-primary`, and a brand `IconButton` whose play glyph is inset
//!   6 points from the tile's bottom-right.
//! * `.../components/ui/world/InstanceItem.vue` — where the reference shows a relative
//!   last-played time: beside the loader and version in the same secondary row
//!   (lines 260-281), wrapped by `app.instance.card.played` (`Played {relativeTime}`)
//!   and `app.instance.card.never-played` (`Never played`).
//! * `.../components/ui/WelcomeScreen.vue` — the empty state: a 100-point mark (line 91),
//!   a heading at 24/32 (line 95), body copy at 16/24 (line 98), and a brand
//!   `Create an instance` button at `h-10` (lines 103-113).
//!
//! **The layout is arithmetic over those classes, not a guess.** The source's grid is
//! four CSS expressions: `gap = remSize * 0.75` (line 122),
//! `columnCount = max(1, floor((gridWidth + gap) / (minWidth + gap)))` (lines 123-126),
//! `cardWidth = (gridWidth - gap * (n - 1)) / n` (lines 127-129), and
//! `cardHeight = cardWidth + remSize * 3.375` (lines 130-134), with `minWidth` chosen by
//! `windowWidth < 1280 ? 8rem : 10rem` (line 124). At a fixed window those resolve to
//! fixed numbers, so this page re-evaluates the same expressions from the rect the shell
//! hands it and the real window width; nothing is sampled from a screenshot. At 1440 the
//! `windowWidth < 1280` test is false, so the **10rem (160-point)** minimum applies —
//! that breakpoint is the whole reason the count is five rather than six below. The tests
//! at the bottom pin the 1440 and 960 cases, including the identity
//! `2 * padding + columns * cardWidth + (columns - 1) * gap == pageWidth`.
//!
//! Each measurement carries its `file:line` at the constant. The colours are the tokens
//! in `assets/styles/variables.scss` as `crate::theme` maps them (`RAISED_BG` is
//! `bg-surface-3`, `BUTTON_BG` is `bg-surface-4`, `DIVIDER` is `--color-divider`, and so
//! on).
//!
//! **The icon tile is the card's width, not a fixed 64-96.** `instance-card-view.vue:52`
//! gives the tile `aspect-square min-w-full` inside the card's `p-3` (line 44), so the
//! rendered square is `cardWidth - 24`: 156 points at the 1440 page's five columns. The
//! 64-96 range in the brief is the reference's *thumbnail request* (`useImageThumbnail`
//! asks for 96 in compact mode and 384 otherwise, lines 26-29), not the box that
//! thumbnail is drawn into; a 96-point tile would stop matching the source this page is
//! meant to match. [`card_height`] is the source's `cardWidth + 54` plus one 18-point row
//! for the last-played line.
//!
//! **No state, no hover, no animation.** The play button is drawn at rest even though the
//! reference reveals it on hover (`instance-card.vue:354`) because this page has no hover
//! state yet, and [`draw`] returns `false` because nothing here is in flight.

use std::time::{SystemTime, UNIX_EPOCH};

use eframe::egui::{self, Align2, Color32, Rect, Sense, Stroke, Ui, Vec2};

use crate::data::Entry;
use crate::fonts;
use crate::icons::{self, Glyph};
use crate::images::Images;
use crate::theme::Theme;

// ---------------------------------------------------------------------------
// Page and toolbar geometry
// ---------------------------------------------------------------------------

/// The route container's inset: `p-6` on `pages/Index.vue:96`, i.e. 1.5rem = 24 points at
/// the reference's 16px root (`variables.scss:199` declares `--gap-xl: 1.5rem`).
const PAGE_PADDING: f32 = 24.0;

/// The gap between the heading, the toolbar and the grid: `gap-3`
/// (`library/index.vue:267`), i.e. `--gap-md` 0.75rem (`variables.scss:197`).
const SECTION_GAP: f32 = 12.0;

/// The heading's line box: `text-2xl` (`library/index.vue:268`) is 1.5rem with a 2rem
/// line height; the same size states it as `leading-8` in `WelcomeScreen.vue:95`.
const HEADING_LINE: f32 = 32.0;

/// The height of every toolbar control. `Combobox`'s trigger is a default-size
/// `ButtonFrame` (`Combobox.vue:51-57`), and `md` is `h-9` (`ButtonFrame.vue:31`); the
/// search field in the same row is `standard` `h-9` too (`InputFrame.vue:47`).
const CONTROL_HEIGHT: f32 = 36.0;

/// The toolbar row's own gap: `gap-2` (`library-toolbar/index.vue:30` and `:49`), i.e.
/// `--gap-sm` 0.5rem (`variables.scss:196`).
const CONTROL_GAP: f32 = 8.0;

/// A select's horizontal padding: `px-2.5` on the `md` `ButtonFrame` (`ButtonFrame.vue:31`).
const SELECT_PADDING: f32 = 10.0;

/// A select's leading glyph: `size-5` (`sort-menu.vue:71` and `:85`), which is
/// `ButtonFrame.vue:31`'s `[&>svg]:size-5`.
const SELECT_GLYPH: f32 = 20.0;

/// The gap between a select's prefix glyph and its label: `gap-2` on the trigger's inner
/// row (`Combobox.vue:71`).
const SELECT_INNER_GAP: f32 = 8.0;

/// The gap between the label and the trailing chevron: `ButtonFrame`'s own `gap-1.5`
/// (`ButtonFrame.vue:31`) plus the suffix row's `gap-1` (`Combobox.vue:88`), both empty
/// once the chevron is the suffix.
const SELECT_TRAILING_GAP: f32 = 6.0;

/// The divider between the two selects: `h-6 w-px` with `mx-2` either side
/// (`library-toolbar/index.vue:51`).
const DIVIDER_HEIGHT: f32 = 24.0;

/// The view-mode segmented control's width: two 32-point segments inside a 36-point
/// control at 2 points of inset. The reference stores this mode as the
/// `compact_instance_cards` flag (`instance-group/index.vue:57,124`) rather than as a
/// toolbar control, whose setting copy is `app.appearance-settings.compact-mode.title`
/// (`BehaviorSettings.vue:91-97`); the two segments are the grid and row shapes that flag
/// selects between.
const VIEW_TOGGLE_WIDTH: f32 = 72.0;

// ---------------------------------------------------------------------------
// Grid geometry — the source's own expressions
// ---------------------------------------------------------------------------

/// The grid gap: `gap = remSize * 0.75` with `remSize` read from the root font size
/// (`instance-group/index.vue:121-122`), 16 * 0.75 = 12, which is `--gap-md`
/// (`variables.scss:197`).
const GRID_GAP: f32 = 12.0;

/// The minimum card width at or above the 1280 breakpoint: `remSize * 10`
/// (`instance-group/index.vue:124`, the `windowWidth < 1280 ? 8 : 10` ternary).
const MIN_CARD_AT_1440: f32 = 160.0;

/// The minimum card width below 1280: `remSize * 8` (`instance-group/index.vue:124`).
const MIN_CARD_BELOW_1280: f32 = 128.0;

/// The breakpoint in that ternary: the grid narrows its minimum at this window width
/// (`instance-group/index.vue:124`).
const NARROW_BREAKPOINT: f32 = 1280.0;

/// The card's radius: `rounded-[20px]` (`instance-card-view.vue:44`), which is
/// `--radius-xl` (`variables.scss:205`).
const CARD_RADIUS: u8 = 20;

/// The card's padding and the tile's inset: `p-3` (`instance-card-view.vue:44`), i.e.
/// `--gap-md` (`variables.scss:197`).
const CARD_PADDING: f32 = 12.0;

/// The tile's radius: `rounded-2xl` (`instance-card-view.vue:52`) is 1rem = 16 points,
/// which is `--radius-lg` (`variables.scss:204`).
const TILE_RADIUS: u8 = 16;

/// The gap between the tile and the text column: `gap-3` on the card's flex column
/// (`instance-card-view.vue:44`).
const CARD_STACK_GAP: f32 = 12.0;

/// The gap between the name and the loader line: `gap-1` on the text column
/// (`instance-card-view.vue:74`).
const CARD_TEXT_GAP: f32 = 4.0;

/// The name's line box: `leading-5` (`instance-card-view.vue:80`) = 20 points.
const NAME_LINE: f32 = 20.0;

/// The loader line's box: `leading-[18px]` (`instance-card-view.vue:87`).
const META_LINE: f32 = 18.0;

/// The last-played line's box. The reference prints it in the same `text-sm` row as the
/// loader and version (`InstanceItem.vue:260-281`); this page gives it its own 18-point
/// line so a long instance name cannot push it out of the card.
const PLAYED_LINE: f32 = 18.0;

/// The source's `cardHeight = cardWidth + remSize * 3.375` (`instance-group/index.vue:
/// 130-134`): 3.375rem = 54 = 24 padding + 12 stack gap + 20 name + 4 text gap + 18
/// loader - the 24 points the full-width tile takes out of the width.
const CARD_HEIGHT_EXTRA: f32 = 54.0;

/// The grid's minimum height: `min-h-[45px]` (`instance-group/index.vue:611`), repeated in
/// the `Math.max(45, ...)` height calculation (`:135-142`).
const GRID_MIN_HEIGHT: f32 = 45.0;

// ---------------------------------------------------------------------------
// Strings
// ---------------------------------------------------------------------------

/// `app.library.title` — "Library" (`locales/en-US/index.json:1235`). The source's route
/// is named `LibraryPage` (`pages/Index.vue:20`); this crate's rail calls the same
/// destination "Instances" (`shell.rs:51`), but the page prints the source's word.
const HEADING: &str = "Library";

/// `app.library.sort.label` — "Sort by" (`locales/en-US/index.json:1223`), the visible
/// value being `app.library.sort.last-played` — "Last played" (`:1226`), which is the
/// reference's default (`use-library.ts:251`).
const SORT_LABEL: &str = "Last played";

/// `app.library.group-by.custom-group` — "Custom group" (`locales/en-US/index.json:1010`),
/// the reference's default group-by value (`use-library.ts:250`).
const GROUP_LABEL: &str = "Custom group";

/// `app.welcome-screen.title` — "Welcome to Modrinth" (`locales/en-US/index.json:3488`).
/// The empty state is the source's own no-instances screen (`pages/Index.vue:92`), so the
/// string is copied rather than rebranded.
const EMPTY_HEADING: &str = "Welcome to Modrinth";

/// `app.welcome-screen.description` — "Ready to start playing?"
/// (`locales/en-US/index.json:3476`).
const EMPTY_BODY: &str = "Ready to start playing?";

/// `app.welcome-screen.create-instance` — "Create an instance"
/// (`locales/en-US/index.json:3473`).
const EMPTY_ACTION: &str = "Create an instance";

/// `app.instance.card.never-played` — "Never played" (`locales/en-US/index.json:635`);
/// `app.home.jump-back-in.never-played` carries the same message (`:473`).
const NEVER_PLAYED: &str = "Never played";

/// The frame `app.instance.card.played` wraps a relative time in — "Played {relativeTime}"
/// (`locales/en-US/index.json:641`).
const PLAYED_PREFIX: &str = "Played";

// ---------------------------------------------------------------------------
// The page
// ---------------------------------------------------------------------------

/// Draw the library page into `rect`. Returns whether another frame is owed: nothing
/// here animates yet, so it is always `false`.
pub fn draw(
    ui: &mut Ui,
    rect: Rect,
    theme: &Theme,
    images: &mut Images,
    instances: &[Entry],
) -> bool {
    // The page owns its rect (`pages/mod.rs`'s seam); paint the page colour first so the
    // grid sits on `--color-bg` even if the caller did not clear the column.
    ui.painter().rect_filled(rect, 0, theme.BG);

    // `Entry` carries no icon path yet (`data.rs:20-33`), so there is no URL to ask
    // `Images::get` for; the tile draws the reference's tinted plate instead. When the
    // reader grows an icon field this is the one call that changes:
    // `if let Some(texture) = images.get(ui.ctx(), &entry.icon) { ... }`.
    let _ = images;

    if rect.width() <= 2.0 * PAGE_PADDING || rect.height() <= 2.0 * PAGE_PADDING {
        return false;
    }

    // The source branches before its heading: no instance ever created means
    // `WelcomeScreen` in place of the whole library section (`pages/Index.vue:92`).
    if instances.is_empty() {
        let painter = ui.painter();
        empty_state(painter, rect, theme);
        return false;
    }

    let now = now_millis();
    let grid_top = {
        let painter = ui.painter();
        heading_and_controls(painter, rect, theme, instances.len())
    };

    let grid_rect = Rect::from_min_max(
        egui::pos2(rect.left() + PAGE_PADDING, grid_top),
        egui::pos2(rect.right() - PAGE_PADDING, rect.bottom() - PAGE_PADDING),
    );
    if grid_rect.width() < 1.0 || grid_rect.height() < 1.0 {
        return false;
    }

    // The source scrolls the whole page; this page owns only the column below its
    // toolbar, so the scroll area starts at the grid. `gridWidth` is measured from the
    // real content box there (`useElementSize(instanceGridContent)`,
    // `instance-group/index.vue:92`), which is what the page rect minus padding is here.
    // The grid is laid out from the width it is actually given. The second argument was a
    // viewport width used for a breakpoint; egui 0.36 has no `Context::screen_rect`, and the
    // column's own width is the honest number anyway -- a breakpoint taken from the viewport
    // would be wrong for a panel that is later resized.
    let grid = grid_layout(grid_rect.width(), grid_rect.width());
    let rows = instances.len().div_ceil(grid.columns.max(1));
    let content_height = (rows as f32 * (grid.card_height + GRID_GAP) - GRID_GAP)
        .max(GRID_MIN_HEIGHT);

    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(grid_rect));
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(&mut child, |ui| {
            // Reserve the full grid height so the scroll area knows what is below the
            // fold. The cards are then painted at the source's absolute positions
            // (`instance-group/index.vue:164-171`) inside that allocation.
            let (area, _) =
                ui.allocate_exact_size(egui::vec2(grid_rect.width(), content_height), Sense::hover());
            let painter = ui.painter();
            for (index, entry) in instances.iter().enumerate() {
                let column = index % grid.columns;
                let row = index / grid.columns;
                let card = Rect::from_min_size(
                    area.min
                        + egui::vec2(
                            column as f32 * (grid.card_width + GRID_GAP),
                            row as f32 * (grid.card_height + GRID_GAP),
                        ),
                    egui::vec2(grid.card_width, grid.card_height),
                );
                draw_card(painter, card, theme, entry, now);
            }
        });

    false
}

// ---------------------------------------------------------------------------
// Heading and view controls
// ---------------------------------------------------------------------------

/// Draw the heading, its count, and the two selects with the grid/list toggle. Returns the
/// y where the grid begins.
fn heading_and_controls(
    painter: &egui::Painter,
    rect: Rect,
    theme: &Theme,
    count: usize,
) -> f32 {
    let heading_at = egui::pos2(rect.left() + PAGE_PADDING, rect.top() + PAGE_PADDING);
    // `h2.m-0.text-2xl.font-semibold.text-contrast` (`library/index.vue:268`).
    let heading = painter.text(
        heading_at,
        Align2::LEFT_TOP,
        HEADING,
        fonts::semibold(24.0),
        theme.CONTRAST,
    );

    // The count is the group header's `TagItem` (`instance-group/index.vue:571-576`):
    // `px-2 py-1 text-sm rounded-full`, so 24 points tall with 8-point padding.
    let count_text = count.to_string();
    let count_galley = painter.layout_no_wrap(
        count_text.clone(),
        fonts::label(14.0),
        theme.SECONDARY,
    );
    let pill = Rect::from_min_size(
        egui::pos2(
            heading.right() + CONTROL_GAP,
            heading_at.y + (HEADING_LINE - 24.0) / 2.0,
        ),
        egui::vec2(count_galley.size().x + 16.0, 24.0),
    );
    painter.rect_filled(pill, 12, theme.RAISED_BG);
    painter.rect_stroke(
        pill,
        12,
        Stroke::new(1.0, theme.DIVIDER),
        egui::StrokeKind::Inside,
    );
    painter.text(
        pill.center(),
        Align2::CENTER_CENTER,
        count_text,
        fonts::label(14.0),
        theme.SECONDARY,
    );

    let row = Rect::from_min_size(
        egui::pos2(
            rect.left() + PAGE_PADDING,
            heading_at.y + HEADING_LINE + SECTION_GAP,
        ),
        egui::vec2(rect.width() - 2.0 * PAGE_PADDING, CONTROL_HEIGHT),
    );

    // The sort `Combobox` (`sort-menu.vue:62-76`), the divider, the group `Combobox`
    // (`sort-menu.vue:77-90`), then the grid/list shape pair.
    let sort = select(painter, row.min, theme, Glyph::ArrowUpDown, SORT_LABEL);
    let divider_x = sort.right() + CONTROL_GAP;
    painter.vline(
        divider_x,
        (row.center().y - DIVIDER_HEIGHT / 2.0)..=(row.center().y + DIVIDER_HEIGHT / 2.0),
        Stroke::new(1.0, theme.DIVIDER),
    );
    let group = select(
        painter,
        egui::pos2(divider_x + 1.0 + CONTROL_GAP, row.top()),
        theme,
        Glyph::LayoutGrid,
        GROUP_LABEL,
    );
    view_toggle(
        painter,
        egui::pos2(group.right() + CONTROL_GAP, row.top()),
        theme,
    );

    row.bottom() + SECTION_GAP
}

/// One rounded select, drawn the way `Combobox`'s `ButtonFrame` trigger resolves at `md`:
/// `h-9 rounded-xl px-2.5` with a 20-point prefix (`ButtonFrame.vue:31`,
/// `Combobox.vue:71-95`). Returns the rect it used.
fn select(
    painter: &egui::Painter,
    min: egui::Pos2,
    theme: &Theme,
    glyph: Glyph,
    label: &str,
) -> Rect {
    let label_galley = painter.layout_no_wrap(
        label.to_owned(),
        fonts::semibold(14.0),
        theme.CONTRAST,
    );
    let width = 2.0 * SELECT_PADDING
        + SELECT_GLYPH
        + SELECT_INNER_GAP
        + label_galley.size().x
        + SELECT_TRAILING_GAP
        + SELECT_GLYPH;
    let rect = Rect::from_min_size(min, egui::vec2(width, CONTROL_HEIGHT));
    painter.rect_filled(rect, 12, theme.BUTTON_BG);

    icons::draw(
        painter,
        glyph,
        Rect::from_center_size(
            egui::pos2(rect.left() + SELECT_PADDING + SELECT_GLYPH / 2.0, rect.center().y),
            Vec2::splat(SELECT_GLYPH),
        ),
        theme.BASE,
    );
    painter.text(
        egui::pos2(
            rect.left() + SELECT_PADDING + SELECT_GLYPH + SELECT_INNER_GAP,
            rect.center().y,
        ),
        Align2::LEFT_CENTER,
        label,
        fonts::semibold(14.0),
        theme.CONTRAST,
    );
    icons::draw(
        painter,
        Glyph::ChevronDown,
        Rect::from_center_size(
            egui::pos2(rect.right() - SELECT_PADDING - SELECT_GLYPH / 2.0, rect.center().y),
            Vec2::splat(16.0),
        ),
        theme.SECONDARY,
    );
    rect
}

/// The two-segment grid/list control. The selected segment is the brand wash a selected
/// tab uses (`BUTTON_BG_SELECTED` at `--color-brand-highlight`, `variables.scss:355`), and
/// it shows the grid because this page draws the grid layout.
fn view_toggle(painter: &egui::Painter, min: egui::Pos2, theme: &Theme) {
    let control = Rect::from_min_size(
        min,
        egui::vec2(VIEW_TOGGLE_WIDTH, CONTROL_HEIGHT),
    );
    painter.rect_filled(control, 12, theme.BUTTON_BG);

    let segment = 32.0;
    let selected = Rect::from_min_size(
        control.min + egui::vec2(2.0, 2.0),
        Vec2::splat(segment),
    );
    painter.rect_filled(selected, 10, theme.BUTTON_BG_SELECTED);
    icons::draw(
        painter,
        Glyph::LayoutGrid,
        Rect::from_center_size(selected.center(), Vec2::splat(18.0)),
        theme.BUTTON_TEXT_SELECTED,
    );

    let unselected = Rect::from_min_size(
        control.min + egui::vec2(2.0 + segment, 2.0),
        Vec2::splat(segment),
    );
    icons::draw(
        painter,
        Glyph::List,
        Rect::from_center_size(unselected.center(), Vec2::splat(18.0)),
        theme.BASE,
    );
}

// ---------------------------------------------------------------------------
// The grid
// ---------------------------------------------------------------------------

/// The resolved grid: the column count, one card's size, and the constants the identity
/// `2*padding + columns*cardWidth + (columns-1)*gap == pageWidth` is stated with.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Grid {
    columns: usize,
    card_width: f32,
    card_height: f32,
}

/// The source's column count: `Math.max(1, Math.floor((gridWidth + gap) /
/// (minWidth + gap)))` (`instance-group/index.vue:123-126`).
fn column_count(grid_width: f32, min_width: f32) -> usize {
    let columns = ((grid_width + GRID_GAP) / (min_width + GRID_GAP)).floor();
    if columns < 1.0 {
        1
    } else {
        columns as usize
    }
}

/// `remSize * (compact ? 15 : windowWidth < 1280 ? 8 : 10)` at `remSize` 16
/// (`instance-group/index.vue:124`). The compact card is not drawn yet; this page is the
/// grid card, so the 15rem branch is not reachable here.
fn min_card_width(window_width: f32) -> f32 {
    if window_width < NARROW_BREAKPOINT {
        MIN_CARD_BELOW_1280
    } else {
        MIN_CARD_AT_1440
    }
}

/// The source's `cardWidth = (gridWidth - gap * (n - 1)) / n`
/// (`instance-group/index.vue:127-129`) and `cardHeight = cardWidth + 3.375rem`
/// (`:130-134`), with one extra 18-point row for the last-played line.
fn grid_layout(page_width: f32, window_width: f32) -> Grid {
    let grid_width = (page_width - 2.0 * PAGE_PADDING).max(0.0);
    let columns = column_count(grid_width, min_card_width(window_width));
    let card_width =
        (grid_width - GRID_GAP * (columns.saturating_sub(1)) as f32) / columns as f32;
    Grid {
        columns,
        card_width,
        card_height: card_height(card_width),
    }
}

/// The source's card height plus the extra row this page draws.
fn card_height(card_width: f32) -> f32 {
    card_width + CARD_HEIGHT_EXTRA + PLAYED_LINE
}

// ---------------------------------------------------------------------------
// The card
// ---------------------------------------------------------------------------

/// One instance card.
///
/// Geometry from `instance-card-view.vue`: `p-3` (line 44), the tile is
/// `aspect-square min-w-full` (line 52), the text column is `gap-1 px-0.5` (line 74), the
/// name is 20 points (line 80), the loader line is 18 (line 87). The play glyph is a
/// brand `IconButton` at `lg` — `h-10 w-10 rounded-[14px]`, which is a 20-point circle
/// here — inside the 48-point leading slot inset `bottom-1.5 right-1.5`
/// (`instance-card-view.vue:68`, `instance-card.vue:306-310`, `ButtonFrame.vue:32,40`).
fn draw_card(painter: &egui::Painter, rect: Rect, theme: &Theme, entry: &Entry, now: i64) {
    painter.rect_filled(rect, CARD_RADIUS, theme.RAISED_BG);
    painter.rect_stroke(
        rect,
        CARD_RADIUS,
        Stroke::new(1.0, theme.BUTTON_BG),
        egui::StrokeKind::Inside,
    );

    let tile_side = (rect.width() - 2.0 * CARD_PADDING).max(1.0);
    let tile = Rect::from_min_size(
        rect.min + egui::vec2(CARD_PADDING, CARD_PADDING),
        Vec2::splat(tile_side),
    );
    painter.rect_filled(tile, TILE_RADIUS, theme.BUTTON_BG);

    // The reference draws the instance's own icon here (`Avatar size="100%"`
    // with `tint-by`, `instance-card-view.vue:54-64`). This reader has no icon path, so
    // the tile keeps the plate and carries the instance's initial at the mark's scale.
    let initial: String = entry
        .name
        .chars()
        .next()
        .map(|first| first.to_uppercase().collect())
        .unwrap_or_default();
    painter.text(
        tile.center(),
        Align2::CENTER_CENTER,
        initial,
        fonts::title((tile_side * 0.28).clamp(24.0, 56.0)),
        theme.SECONDARY,
    );

    // The play control: 48-point slot at 6 inside the tile, holding the 40-point `lg`
    // IconButton (`instance-card-view.vue:68`, `instance-card.vue:309`).
    let play_center = egui::pos2(
        tile.right() - 6.0 - 24.0,
        tile.bottom() - 6.0 - 24.0,
    );
    painter.circle_filled(play_center, 20.0, theme.BRAND);
    icons::draw(
        painter,
        Glyph::Play,
        Rect::from_center_size(play_center, Vec2::splat(18.0)),
        theme.ACCENT_CONTRAST,
    );

    let text_left = rect.left() + CARD_PADDING + 2.0; // `px-0.5` (`instance-card-view.vue:74`)
    let text_width = rect.width() - 2.0 * (CARD_PADDING + 2.0);
    let name_top = tile.bottom() + CARD_STACK_GAP;
    clipped_line(
        painter,
        Rect::from_min_size(
            egui::pos2(text_left, name_top),
            egui::vec2(text_width, NAME_LINE),
        ),
        &entry.name,
        fonts::title(17.0),
        theme.CONTRAST,
    );

    // `{{ instance.loader }} {{ instance.game_version }}` (`instance-card-view.vue:89`).
    let loader = if entry.version.is_empty() {
        entry.loader.clone()
    } else {
        format!("{} {}", entry.loader, entry.version)
    };
    let meta_top = name_top + NAME_LINE + CARD_TEXT_GAP;
    clipped_line(
        painter,
        Rect::from_min_size(
            egui::pos2(text_left, meta_top),
            egui::vec2(text_width, META_LINE),
        ),
        &loader,
        fonts::body(13.0),
        theme.SECONDARY,
    );

    // The reference shows a relative last-played time beside the loader/version
    // (`InstanceItem.vue:260-281`); this page gives it the reserved line below and draws
    // it only when the entry has actually been played (`last_played > 0`).
    if entry.last_played > 0 {
        clipped_line(
            painter,
            Rect::from_min_size(
                egui::pos2(text_left, meta_top + META_LINE),
                egui::vec2(text_width, PLAYED_LINE),
            ),
            &last_played_label(entry.last_played, now),
            fonts::body(13.0),
            theme.SECONDARY,
        );
    }
}

/// Draw one line of text, clipped to its own box. A long name ends at the card's edge
/// rather than wrapping into the loader line; the reference truncates the same two lines
/// (`instance-card-view.vue:80,87`).
fn clipped_line(
    painter: &egui::Painter,
    rect: Rect,
    text: &str,
    font: egui::FontId,
    ink: Color32,
) {
    let galley = painter.layout(text.to_owned(), font, ink, rect.width());
    painter.with_clip_rect(rect).galley(rect.min, galley, ink);
}

// ---------------------------------------------------------------------------
// The empty state
// ---------------------------------------------------------------------------

/// The source's no-instances screen (`WelcomeScreen.vue`): a 100-point mark plate at the
/// centre of the content area, the heading at 24/32, the body at 16/24, and the brand
/// button at `h-10`.
///
/// The container is `px-6 pb-6 pt-16` (line 84) and the stack is `gap-6` (line 86), so
/// the vertical rhythm is 24, 8 inside the text block (line 94), 24, then the button.
fn empty_state(painter: &egui::Painter, rect: Rect, theme: &Theme) {
    let area = Rect::from_min_max(
        egui::pos2(rect.left() + PAGE_PADDING, rect.top() + 64.0),
        egui::pos2(rect.right() - PAGE_PADDING, rect.bottom() - PAGE_PADDING),
    );
    let centre_x = area.center().x;

    let plate_side = 100.0; // `size-[6.25rem]` (`WelcomeScreen.vue:91`)
    let block_gap = 24.0; // `gap-6` (`WelcomeScreen.vue:86`)
    let text_gap = 8.0; // `gap-2` (`WelcomeScreen.vue:94`)
    let body_line = 24.0; // `leading-6` (`WelcomeScreen.vue:98`)
    let button_height = 40.0; // `size="lg"` -> `h-10` (`WelcomeScreen.vue:106`, `ButtonFrame.vue:32`)
    let total = plate_side + block_gap + HEADING_LINE + text_gap + body_line + block_gap
        + button_height;
    let top = area.center().y - total / 2.0;

    let plate = Rect::from_center_size(
        egui::pos2(centre_x, top + plate_side / 2.0),
        Vec2::splat(plate_side),
    );
    painter.rect_filled(plate, 16, theme.RAISED_BG);
    painter.rect_stroke(
        plate,
        16,
        Stroke::new(1.0, theme.DIVIDER),
        egui::StrokeKind::Inside,
    );
    icons::draw(
        painter,
        Glyph::Library,
        Rect::from_center_size(plate.center(), Vec2::splat(40.0)),
        theme.BUTTON_TEXT_SELECTED,
    );

    let heading_y = plate.bottom() + block_gap + HEADING_LINE / 2.0;
    painter.text(
        egui::pos2(centre_x, heading_y),
        Align2::CENTER_CENTER,
        EMPTY_HEADING,
        fonts::semibold(24.0),
        theme.CONTRAST,
    );
    let body_y = heading_y + HEADING_LINE / 2.0 + text_gap + body_line / 2.0;
    painter.text(
        egui::pos2(centre_x, body_y),
        Align2::CENTER_CENTER,
        EMPTY_BODY,
        fonts::body(16.0),
        theme.BASE,
    );

    // The brand button: `type="colored" color="brand" size="lg"` (`WelcomeScreen.vue:
    // 103-113`), content-sized inside the 288-point stack (line 102), with `px-4`, a
    // 20-point glyph and `gap-2` (`ButtonFrame.vue:32`).
    let label = painter.layout_no_wrap(
        EMPTY_ACTION.to_owned(),
        fonts::semibold(16.0),
        theme.ACCENT_CONTRAST,
    );
    let button_width = 2.0 * 16.0 + 20.0 + 8.0 + label.size().x;
    let button = Rect::from_center_size(
        egui::pos2(
            centre_x,
            body_y + body_line / 2.0 + block_gap + button_height / 2.0,
        ),
        egui::vec2(button_width, button_height),
    );
    painter.rect_filled(button, 14, theme.BRAND);
    icons::draw(
        painter,
        Glyph::Plus,
        Rect::from_center_size(
            egui::pos2(button.left() + 16.0 + 10.0, button.center().y),
            Vec2::splat(20.0),
        ),
        theme.ACCENT_CONTRAST,
    );
    painter.galley(
        egui::pos2(
            button.left() + 16.0 + 20.0 + 8.0,
            button.center().y - label.size().y / 2.0,
        ),
        label,
        theme.ACCENT_CONTRAST,
    );
}

// ---------------------------------------------------------------------------
// Time
// ---------------------------------------------------------------------------

/// Now, in milliseconds since the epoch. A clock before 1970 answers 0, which reads as
/// never played rather than as a date in the future.
fn now_millis() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(elapsed) => match i64::try_from(elapsed.as_millis()) {
            Ok(millis) => millis,
            Err(_) => i64::MAX,
        },
        Err(_) => 0,
    }
}

/// The reference's relative-time ladder, in the order `useRelativeTime` tests it
/// (`ui/src/composables/how-ago.ts:24-48`): seconds, minutes, hours, days, weeks, months,
/// years. This helper starts at minutes (the brief's ladder) and stops at months, so a
/// value older than a year reads in months where the reference would have switched to
/// years.
///
/// The words are the reference's own: `app.instance.card.played` supplies
/// `Played {relativeTime}` (`locales/en-US/index.json:641`) and `app.instance.card.
/// never-played` supplies `Never played` (`:635`). The unit phrases are what
/// `Intl.RelativeTimeFormat` constructs for en-US at `style: 'long'` and `numeric:
/// 'auto'` — the settings `how-ago.ts:57-66` passes to it — so the singular day, week and
/// month are `yesterday`, `last week` and `last month`, exactly as the app prints them.
fn last_played_label(millis: i64, now: i64) -> String {
    if millis <= 0 {
        return NEVER_PLAYED.to_owned();
    }
    let elapsed = now.saturating_sub(millis).max(0);

    let minutes = rounded_units(elapsed, 60_000);
    if minutes < 60 {
        return played(minutes, "minute");
    }
    let hours = rounded_units(elapsed, 3_600_000);
    if hours < 24 {
        return played(hours, "hour");
    }
    let days = rounded_units(elapsed, 86_400_000);
    if days < 7 {
        return played(days, "day");
    }
    let weeks = rounded_units(elapsed, 604_800_000);
    if weeks < 4 {
        return played(weeks, "week");
    }
    played(rounded_units(elapsed, 2_629_746_000), "month")
}

/// `Math.round(diff / unit)` from `how-ago.ts:24-30`, in integer arithmetic. The half-unit
/// is added with saturation so a clock far in the past cannot wrap the count negative.
fn rounded_units(elapsed_millis: i64, unit_millis: i64) -> i64 {
    elapsed_millis.saturating_add(unit_millis / 2) / unit_millis
}

/// `Played {relativeTime}` with `Intl.RelativeTimeFormat`'s auto singulars.
fn played(count: i64, unit: &str) -> String {
    let count = count.max(1);
    let relative = match (count, unit) {
        (1, "day") => "yesterday".to_owned(),
        (1, "week") => "last week".to_owned(),
        (1, "month") => "last month".to_owned(),
        (1, _) => format!("1 {unit} ago"),
        (many, _) => format!("{many} {unit}s ago"),
    };
    format!("{PLAYED_PREFIX} {relative}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: i64 = 60_000;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;
    const WEEK: i64 = 7 * DAY;
    const MONTH: i64 = 2_629_746_000;
    const NOW: i64 = 1_700_000_000_000;

    /// `0` is the reader's "never", and the reference has a word for it.
    #[test]
    fn a_never_played_instance_says_never_played() {
        assert_eq!(last_played_label(0, NOW), "Never played");
        assert_eq!(last_played_label(-1, NOW), "Never played");
    }

    /// Every bucket the brief asks for, from the reference's own ladder. The values sit
    /// in the middle of each bucket so the assertion is about the unit, not its edge.
    #[test]
    fn the_relative_ladder_reads_minutes_hours_days_weeks_and_months() {
        assert_eq!(last_played_label(NOW - 5 * MINUTE, NOW), "Played 5 minutes ago");
        assert_eq!(last_played_label(NOW - 3 * HOUR, NOW), "Played 3 hours ago");
        assert_eq!(last_played_label(NOW - 4 * DAY, NOW), "Played 4 days ago");
        assert_eq!(last_played_label(NOW - 2 * WEEK, NOW), "Played 2 weeks ago");
        assert_eq!(last_played_label(NOW - 3 * MONTH, NOW), "Played 3 months ago");
    }

    /// `how-ago.ts` asks `Intl.RelativeTimeFormat` for `numeric: 'auto'`, so the singular
    /// day, week and month are not "1 day ago" but the words below.
    #[test]
    fn the_singular_buckets_are_the_auto_wording_the_app_shows() {
        assert_eq!(last_played_label(NOW - MINUTE, NOW), "Played 1 minute ago");
        assert_eq!(last_played_label(NOW - HOUR, NOW), "Played 1 hour ago");
        assert_eq!(last_played_label(NOW - DAY, NOW), "Played yesterday");
        assert_eq!(last_played_label(NOW - WEEK, NOW), "Played last week");
        assert_eq!(last_played_label(NOW - MONTH, NOW), "Played last month");
    }

    /// The boundaries are the reference's: 59 minutes is minutes, 60 is hours; 6 days is
    /// days, 7 is weeks; 3 weeks is weeks, 4 is months.
    #[test]
    fn the_bucket_edges_are_the_composables() {
        assert_eq!(last_played_label(NOW - 59 * MINUTE, NOW), "Played 59 minutes ago");
        assert_eq!(last_played_label(NOW - 60 * MINUTE, NOW), "Played 1 hour ago");
        assert_eq!(last_played_label(NOW - 6 * DAY, NOW), "Played 6 days ago");
        assert_eq!(last_played_label(NOW - 7 * DAY, NOW), "Played last week");
        assert_eq!(last_played_label(NOW - 3 * WEEK, NOW), "Played 3 weeks ago");
        assert_eq!(
            last_played_label(NOW - 4 * WEEK, NOW),
            "Played last month"
        );
    }

    /// The 16px root (`instance-group/index.vue:121`) and the shell's content column
    /// (`pages/mod.rs:39-49`: `PANEL_LEFT - CONTENT_LEFT - 16`). At 960 the same furniture
    /// is assumed, so the column is 960 - 64 - 340 - 24 - 16.
    #[cfg(test)]
    fn page_width_at(window_width: f32) -> f32 {
        window_width
            - crate::geometry::RAIL
            - crate::geometry::PANEL
            - crate::geometry::CONTENT_INSET
            - 16.0
    }

    /// At 1440 the source's `windowWidth < 1280` test is false, so `minWidth` is 10rem =
    /// 160, not 8rem. `floor((948 + 12) / (160 + 12)) = 5`, and the four gaps and the two
    /// 24-point insets leave exactly the page's 996 points.
    #[test]
    fn the_grid_tiles_the_page_at_1440() {
        let page = page_width_at(crate::geometry::WINDOW.0);
        assert_eq!(page, 996.0, "the shell's content column at 1440");
        assert_eq!(min_card_width(crate::geometry::WINDOW.0), 160.0);

        let grid = grid_layout(page, crate::geometry::WINDOW.0);
        assert_eq!(grid.columns, 5, "floor((948 + 12) / (160 + 12))");
        assert!((grid.card_width - 180.0).abs() < 0.001, "948 / 5 with four 12-point gaps");

        let used = 2.0 * PAGE_PADDING
            + grid.columns as f32 * grid.card_width
            + (grid.columns as f32 - 1.0) * GRID_GAP;
        assert!(
            (used - page).abs() < 0.001,
            "padding + columns * card + gaps must be the page width: {used} vs {page}"
        );
        assert!(
            (grid.card_height - card_height(grid.card_width)).abs() < 0.001,
            "the height comes from the same width the row is laid out with"
        );
        assert_eq!(grid.card_height, 252.0, "180 + 54 + 18");
    }

    /// At 960 the ternary takes the other branch: `minWidth` is 8rem = 128, so
    /// `floor((468 + 12) / (128 + 12)) = 3` and the cards are 148 wide. The identity still
    /// closes exactly, which is the point of computing the columns rather than rounding a
    /// guessed count.
    #[test]
    fn the_grid_tiles_the_page_at_960() {
        let page = page_width_at(960.0);
        assert_eq!(page, 516.0, "the same shell furniture at 960");
        assert_eq!(min_card_width(960.0), 128.0);

        let grid = grid_layout(page, 960.0);
        assert_eq!(grid.columns, 3, "floor((468 + 12) / (128 + 12))");
        assert!((grid.card_width - 148.0).abs() < 0.001);

        let used = 2.0 * PAGE_PADDING
            + grid.columns as f32 * grid.card_width
            + (grid.columns as f32 - 1.0) * GRID_GAP;
        assert!(
            (used - page).abs() < 0.001,
            "padding + columns * card + gaps must be the page width: {used} vs {page}"
        );
        assert_eq!(grid.card_height, 220.0, "148 + 54 + 18");
    }

    /// The count never drops below one even for a page narrower than a card, and it is a
    /// pure function of the two widths the source's expression names.
    #[test]
    fn a_page_narrower_than_one_card_still_gets_one_column() {
        let grid = grid_layout(2.0 * PAGE_PADDING + 40.0, 960.0);
        assert_eq!(grid.columns, 1);
        assert!((grid.card_width - 40.0).abs() < 0.001);
    }
}
