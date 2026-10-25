//! The screenshots page: the `Screenshots` heading, the toolbar's search plate and two
//! selects, and the date-grouped grid of screenshot cards.
//!
//! Follows `vendor/modrinth-app/app-frontend/src/pages/Screenshots.vue` and the component it
//! renders, `.../components/ui/screenshots-page/`:
//!
//! * `pages/Screenshots.vue` — the route's container is `box-border h-full p-6` (line 26)
//!   and it passes `show-heading`, so the heading is on.
//! * `.../screenshots-page/index.vue` — the page root is `flex h-full w-full flex-col gap-3`
//!   (line 1482) and carries `justify-center` while there is nothing to list (line 1483).
//!   The heading is `m-0 text-2xl font-bold text-contrast` (line 1486) — `font-bold`, where
//!   the library's heading is `font-semibold` — and heading *and* toolbar exist only when
//!   the list is non-empty (line 1485). The stored defaults are `sort = 'newest'` and
//!   `groupBy = 'date'` (lines 149-150).
//! * `.../screenshots-page/toolbar.vue` — two rows: a search `Input` with a `New group`
//!   button (lines 36-49), then the sort and group `Combobox`es (lines 50-85). **There is no
//!   view toggle**: the grid/list pair lives in the library's own toolbar
//!   (`.../library/library-toolbar`), and this toolbar does not import it, so this page
//!   draws none.
//! * `.../screenshots-page/group.vue` — the grid is `grid-cols-1 gap-3 sm:grid-cols-2
//!   2xl:grid-cols-4` with `min-h-[45px]` (lines 208-213).
//! * `.../screenshots-page/section.vue` — the group header is an `h-10` row with
//!   `border-b border-surface-5`, a 20-point `DropdownIcon`, the title at `text-base
//!   font-semibold text-primary` and a `TagItem` count (lines 112-154); the grid sits
//!   `mt-2.5` under it (line 164).
//! * `.../screenshots-page/card.vue` — the card is `aspect-video overflow-hidden rounded-xl
//!   border border-surface-5 bg-surface-2` (line 172) with the thumbnail filling it
//!   (line 222), and the caption is the *hover overlay* at the card's foot: `inset-x-0
//!   bottom-0 ... bg-gradient-to-t from-surface-1 to-transparent p-3 pt-[120px]`, the file
//!   name at `text-sm font-semibold`, the second line at `text-xs text-secondary`, and
//!   edit/copy/more `IconButton`s on the right (lines 232-277). It is the card's press hint
//!   — the card is `role="button"` and activates the viewer (lines 168-193) — and it fades
//!   in with `opacity-0 group-hover:opacity-100`.
//!
//! **The layout is arithmetic over the source's own constants**, not a guess.
//! `screenshots-page/index.vue` names `SCREENSHOT_GRID_GAP = 12` (line 127),
//! `SCREENSHOT_GROUP_SPACING = 12` (128), `SCREENSHOT_GROUP_HEADER_HEIGHT = 40` (129),
//! `SCREENSHOT_GROUP_CONTENT_SPACING = 10` (130), `SCREENSHOT_GRID_MIN_HEIGHT = 45` (132) and
//! `FALLBACK_SCREENSHOT_CARD_WIDTH = 320` (133). The column count is
//! `windowWidth >= 1536 ? 4 : windowWidth >= 640 ? 2 : 1` (lines 522-526), a card's width is
//! the list's width less the gaps over that count (528-532), its height is `width * 9 / 16`
//! (534), and a group's block is `header + 10 + grid + 12` (544-554). Every one of those is
//! a constant below with its line, and [`groups`], [`grid_layout`] and [`group_height`] are
//! the pure functions the tests pin.
//!
//! **The grid at 1440.** The shell's content column is 996 points (`PANEL_LEFT -
//! CONTENT_LEFT - 16`, `pages/mod.rs:46-59`), so the page's 24-point insets leave 948 for the
//! list. 1440 is below the 1536 breakpoint, so the source shows **two** columns: two
//! 468-point cards and one 12-point gap close the 948 exactly, and a card is 263.25 tall
//! (`468 * 9 / 16`). The 1536 window takes the four-column branch: `(948 - 36) / 4 = 228`
//! wide. Both branches are tests, including the identity
//! `columns * cardWidth + (columns - 1) * gap == listWidth`.
//!
//! **The grouping is the source's date rule, month by month.** `getDateGroup`
//! (`index.vue:980-1005`) buckets a screenshot by the day it was taken: `Today`,
//! `Yesterday`, `This week`, `This month`, and then the fallback `formatMonth(...)`, i.e.
//! "October 2026". The first four rungs are a function of the *reader's clock*; [`groups`]
//! takes no clock (the signature the page's seam fixes is `fn groups(&[Shot])`) and a clock
//! read inside it would make both the function and its tests time-dependent, so this page
//! implements the month rung — the one rung whose title is a property of the data — with the
//! source's own bucket id (`date:month:YYYY-MM`) and its own title formatter
//! (`useFormatDateTime({ month: 'long', year: 'numeric' })`, `index.vue:227`). The four
//! relative headings are listed under "what this page cannot do yet" below.
//!
//! The source offers a **group-by** select whose other modes are `custom`, `instance` and
//! `none` (`index.vue:395-400`); the instance mode (`:431-466`) is the one a global list
//! gets by default in the *library*, not here. This page is pinned to the `date` default
//! because [`groups`] carries no group-by argument, and the instance is still visible where
//! the source puts it in date mode: on every card, through
//! `app.screenshots.preview.instance-and-date`.
//!
//! **What this page cannot do yet.**
//!
//! * Load a screenshot. `Shot` carries no path, so [`draw_card`] paints the plate in
//!   `theme.SUPER_RAISED_BG`; the exact `Images` call site is marked in that function.
//! * Type in the search field. The plate is painted from [`State::search`]; the interactive
//!   field is a sibling's (`crate::widgets::search::Search`, whose `ui` takes exactly the
//!   rect this plate occupies), and its plate is currently a different shape
//!   (`RAISED_BG` at `RADIUS_SM`, text inset 34) than the source's input — the two must be
//!   reconciled when it is laid in.
//! * Draw the relative date headings (`Today`, `Yesterday`, `This week`, `This month`):
//!   they need the clock, which [`groups`] does not take.
//! * Select a sort or a group-by: the two `Combobox`es are painted at their stored values,
//!   and there is no dropdown, no hover and no press anywhere on this page.
//! * Read real screenshots. [`SHOTS`] is the capture's sample data; a reader would list each
//!   instance's `screenshots/` folder and hand the page a `&[Shot]`.
//! * Show one screenshot. The viewer (`ImageViewerEditor`, `index.vue:1439-1478`), the
//!   selection bar (`:1640-1702`), drag-and-drop grouping and the ZIP export are untouched.
//! * Load the empty state's illustration: `NoImagesIllustration` is a 200-point SVG asset
//!   this build does not carry, so the icon set's `Images` glyph stands in on its box.

use eframe::egui::{self, Align2, Color32, Rect, Sense, Stroke, Ui, Vec2};

use crate::fonts;
use crate::icons::{self, Glyph};
use crate::images::Images;
use crate::theme::{Theme, STROKE_WIDTH};

// ---------------------------------------------------------------------------
// Page and toolbar geometry
// ---------------------------------------------------------------------------

/// The route container's inset: `p-6` on `pages/Screenshots.vue:26`, i.e. 1.5rem = 24 points
/// at the reference's 16px root (`assets/styles/variables.scss:199`).
const PAGE_PADDING: f32 = 24.0;

/// The gap between the heading, the toolbar and the list: `gap-3` on the page root
/// (`screenshots-page/index.vue:1482`), i.e. `--gap-md` 0.75rem (`variables.scss:197`).
const SECTION_GAP: f32 = 12.0;

/// The heading's size: `text-2xl` (`index.vue:1486`) = 1.5rem = `--font-size-xl`
/// (`assets/styles/defaults.scss:29`).
const HEADING_SIZE: f32 = 24.0;

/// The heading's line box: `text-2xl`'s 2rem line height, which the reference states as
/// `leading-8` where it spells it out (`WelcomeScreen.vue:95`).
const HEADING_LINE: f32 = 32.0;

/// The height of every toolbar control: `InputFrame`'s `standard` size is `h-9`
/// (`inputs/InputFrame.vue:47`) and `ButtonFrame`'s `md` is `h-9`
/// (`buttons/ButtonFrame.vue:31`), which is what the `Combobox` trigger uses
/// (`Combobox.vue:234,338`).
const CONTROL_HEIGHT: f32 = 36.0;

/// The gap inside a toolbar row: `gap-2` on both rows (`toolbar.vue:36,50`), i.e.
/// `--gap-sm` 0.5rem (`variables.scss:196`).
const CONTROL_GAP: f32 = 8.0;

/// The gap between the toolbar's two rows: `flex flex-col gap-2` (`toolbar.vue:35`).
const TOOLBAR_ROW_GAP: f32 = 8.0;

/// The controls' radius: `rounded-xl` (`InputFrame.vue:47`, `ButtonFrame.vue:31`), i.e.
/// `--radius-md` 0.75rem (`variables.scss:203`).
const CONTROL_RADIUS: u8 = 12;

/// The search field's minimum width: `min-w-[16rem]` on the `Input` (`toolbar.vue:43`),
/// 16 * 16 = 256 points. The source's row is `flex-wrap`, so a narrower column moves the
/// `New group` button onto its own line; this page keeps one row and lets the field stop at
/// this floor, which is the same layout at any width the shell's content column takes.
const SEARCH_MIN_WIDTH: f32 = 256.0;

/// The search field's horizontal padding: `px-3` on the `standard` frame
/// (`InputFrame.vue:47`).
const SEARCH_PADDING: f32 = 12.0;

/// The search field's leading magnifier and its trailing clear `X` are both `size-5`
/// (`inputs/Input.vue:12`, `inputs/InputClearButton.vue:4`).
const SEARCH_GLYPH: f32 = 20.0;

/// The gap between the magnifier and the text, and between the text and the clear `X`:
/// `gap-2` on the frame's row (`InputFrame.vue:47`).
const SEARCH_INNER_GAP: f32 = 8.0;

/// The search field's text: `text-base` on a `standard` input (`Input.vue:55`), in the
/// frame's `font-medium` (`InputFrame.vue:4`).
const SEARCH_TEXT_SIZE: f32 = 16.0;

/// A text button's horizontal padding: `px-2.5` on the `md` frame (`ButtonFrame.vue:31`).
const BUTTON_PADDING: f32 = 10.0;

/// The gap between a text button's glyph and its label: `gap-1.5`
/// (`ButtonFrame.vue:31`).
const BUTTON_GAP: f32 = 6.0;

/// A button's glyph: `[&>svg]:size-5` (`ButtonFrame.vue:31`).
const BUTTON_GLYPH: f32 = 20.0;

/// A button's label: `text-base` with `font-semibold` (`ButtonFrame.vue:31`).
const BUTTON_TEXT_SIZE: f32 = 16.0;

/// A select's horizontal padding: `px-2.5` on the `md` frame (`ButtonFrame.vue:31`).
const SELECT_PADDING: f32 = 10.0;

/// A select's prefix glyph and its trailing chevron: `size-5` on the prefix
/// (`toolbar.vue:60,77`) and `size-5` on `ChevronLeftIcon` (`Combobox.vue:92`).
const SELECT_GLYPH: f32 = 20.0;

/// The gap between a select's prefix glyph and its label: `gap-2` on the trigger's inner row
/// (`Combobox.vue:71`).
const SELECT_INNER_GAP: f32 = 8.0;

/// The gap before a select's trailing chevron: the inner row's own `gap-1.5`
/// (`ButtonFrame.vue:31`) plus the suffix row's `gap-1` (`Combobox.vue:88`), with nothing
/// else in the suffix.
const SELECT_SUFFIX_GAP: f32 = 4.0;

/// A select's label: `font-semibold leading-tight` inside the `md` frame, so `text-base`
/// (`Combobox.vue:80`, `ButtonFrame.vue:31`).
const SELECT_LABEL_SIZE: f32 = 16.0;

/// The count pill's text: `text-sm` on `TagItem` (`base/TagItem.vue:19`).
const BADGE_TEXT_SIZE: f32 = 14.0;

/// The count pill's horizontal padding: `px-2` on `TagItem` (`TagItem.vue:19`).
const BADGE_PADDING: f32 = 8.0;

/// The count pill's height: `px-2 py-1 text-sm leading-none` plus a 1-point border
/// (`TagItem.vue:19`) is 4 + 14 + 4 + 2 = 24.
const BADGE_HEIGHT: f32 = 24.0;

/// The count pill's radius: half its 24-point height, which is what `rounded-full`
/// (`TagItem.vue:19`) resolves to.
const BADGE_RADIUS: u8 = 12;

// ---------------------------------------------------------------------------
// Grid, group and card geometry — the source's own constants
// ---------------------------------------------------------------------------

/// The grid gap: `SCREENSHOT_GRID_GAP = 12` (`screenshots-page/index.vue:127`), drawn as
/// `gap-3` (`group.vue:210`).
const GRID_GAP: f32 = 12.0;

/// The space between two group blocks: `SCREENSHOT_GROUP_SPACING = 12`
/// (`index.vue:128`), drawn as `pb-3` (`group.vue:178`).
const GROUP_SPACING: f32 = 12.0;

/// The group header's height: `SCREENSHOT_GROUP_HEADER_HEIGHT = 40` (`index.vue:129`),
/// drawn as `h-10` (`section.vue:114`).
const GROUP_HEADER_HEIGHT: f32 = 40.0;

/// The space between a header and its grid: `SCREENSHOT_GROUP_CONTENT_SPACING = 10`
/// (`index.vue:130`), drawn as `mt-2.5` (`section.vue:164`).
const GROUP_CONTENT_SPACING: f32 = 10.0;

/// The empty grid's height: `SCREENSHOT_GRID_MIN_HEIGHT = 45` (`index.vue:132`), drawn as
/// `min-h-[45px]` (`group.vue:208`).
const GRID_MIN_HEIGHT: f32 = 45.0;

/// The card width used before the list has been measured:
/// `FALLBACK_SCREENSHOT_CARD_WIDTH = 320` (`index.vue:133`).
const FALLBACK_CARD_WIDTH: f32 = 320.0;

/// The 2xl breakpoint, above which the grid is four columns: `windowWidth >= 1536`
/// (`index.vue:523`) and Tailwind's `2xl` (`group.vue:210`).
const WIDE_BREAKPOINT: f32 = 1536.0;

/// The sm breakpoint, above which the grid is two columns: `windowWidth >= 640`
/// (`index.vue:524`) and Tailwind's `sm` (`group.vue:210`).
const NARROW_BREAKPOINT: f32 = 640.0;

/// The column count above [`WIDE_BREAKPOINT`] (`index.vue:523`, `2xl:grid-cols-4`).
const WIDE_COLUMNS: usize = 4;

/// The column count between the two breakpoints (`index.vue:524`, `sm:grid-cols-2`).
const NARROW_COLUMNS: usize = 2;

/// The column count below [`NARROW_BREAKPOINT`] (`index.vue:525`, `grid-cols-1`).
const SLIM_COLUMNS: usize = 1;

/// A card's aspect: `aspect-video` (`card.vue:172`) and `screenshotCardHeight = width * 9 /
/// 16` (`index.vue:534`).
const CARD_ASPECT: f32 = 9.0 / 16.0;

/// The card's radius: `rounded-xl` (`card.vue:172`), i.e. `--radius-md`
/// (`variables.scss:203`).
const CARD_RADIUS: u8 = 12;

/// The overlay's padding: `p-3` (`card.vue:233`).
const OVERLAY_PADDING: f32 = 12.0;

/// The gap between the overlay's text column and its action cluster: `gap-2`
/// (`card.vue:233`).
const OVERLAY_GAP: f32 = 8.0;

/// The overlay's fade: `pt-[120px]` (`card.vue:233`) — the first 120 points above the
/// overlay's content are the gradient from `--surface-1` to transparent.
const OVERLAY_FADE: f32 = 120.0;

/// The fade sampled at its foot: `from-surface-1` is opaque (`card.vue:233`).
const SCRIM_ALPHA_BOTTOM: u8 = 255;

/// The fade sampled two thirds of the way down: `255 * 2 / 3`.
const SCRIM_ALPHA_MIDDLE: u8 = 170;

/// The fade sampled one third of the way down: `255 / 3`. Three stacked bands read as the
/// CSS ramp without a per-pixel mesh, and they are the only stops this page needs.
const SCRIM_ALPHA_TOP: u8 = 85;

/// The caption's first line: `text-sm font-semibold` (`card.vue:236`) — 14 points on a
/// 1.25rem line box.
const CAPTION_SIZE: f32 = 14.0;

/// The caption's first line box: `text-sm`'s `line-height: 1.25rem` = 20 points.
const CAPTION_LINE: f32 = 20.0;

/// The caption's second line: `text-xs` (`card.vue:239`) — 12 points on a 1rem line box.
const DATE_SIZE: f32 = 12.0;

/// The caption's second line box: `text-xs`'s `line-height: 1rem` = 16 points.
const DATE_LINE: f32 = 16.0;

/// An overlay action's size: `IconButton` at its default `md` is `h-9 w-9`
/// (`ButtonFrame.vue:31,36-40`) and `circular` by default (`IconButton.vue:29`,
/// `ButtonFrame.vue:101`), so the three actions are 36-point circles.
const ACTION_SIZE: f32 = 36.0;

/// The gap between two overlay actions: `gap-1` (`card.vue:245`).
const ACTION_GAP: f32 = 4.0;

/// An overlay action's glyph: `[&>svg]:size-5` (`ButtonFrame.vue:31`).
const ACTION_GLYPH: f32 = 20.0;

/// The group header's chevron: `size-5` on `DropdownIcon` (`section.vue:125`).
const HEADER_GLYPH: f32 = 20.0;

/// The gap between the header's chevron, its title and its count: `gap-2` on the header row
/// (`section.vue:114`) and on the open-target inside it (`section.vue:116`).
const HEADER_GAP: f32 = 8.0;

/// The group title: `text-base font-semibold text-primary` (`section.vue:144`).
const GROUP_TITLE_SIZE: f32 = 16.0;

/// The empty state's illustration box: `h-[200px]` on the `EmptyState` illustration slot
/// (`base/EmptyState.vue:4`).
const ILLUSTRATION_HEIGHT: f32 = 200.0;

/// The glyph standing in for that illustration. The source's SVG is a 200-point-tall drawing
/// this build has no asset for, so the size of the icon that replaces it inside the box is a
/// choice rather than a measurement.
// guess: re-measure
const ILLUSTRATION_GLYPH: f32 = 64.0;

/// The gap between the empty state's heading and its description: `gap-1.5`
/// (`EmptyState.vue:6`). The illustration and the text block sit flush: the container has no
/// `gap` of its own (`EmptyState.vue:2`).
const EMPTY_TEXT_GAP: f32 = 6.0;

/// The empty state's body line: the interface's default `body` is 16/24
/// (`defaults.scss:24-31`), and `EmptyState`'s description takes `text-secondary` with no
/// size of its own (`EmptyState.vue:10`).
const EMPTY_BODY_LINE: f32 = 24.0;

/// The empty state's body size: 16 points, `--font-size-nm` (`defaults.scss:26`).
const EMPTY_BODY_SIZE: f32 = 16.0;

// ---------------------------------------------------------------------------
// Strings — `app-frontend/src/locales/en-US/index.json`, verbatim
// ---------------------------------------------------------------------------

/// `app.screenshots.heading` — "Screenshots" (`locales/en-US/index.json:1562`).
const HEADING: &str = "Screenshots";

/// `app.screenshots.sort.newest` — "Newest" (`index.json:1622`), the reference's stored
/// default (`screenshots-page/index.vue:149`).
const SORT_NEWEST: &str = "Newest";

/// `app.screenshots.sort.oldest` — "Oldest" (`index.json:1625`).
const SORT_OLDEST: &str = "Oldest";

/// `app.screenshots.sort.name` — "Name" (`index.json:1619`).
const SORT_NAME: &str = "Name";

/// `app.screenshots.group.date` — "Date" (`index.json:1520`), the reference's stored default
/// group-by (`screenshots-page/index.vue:150`), so it is what the second select shows.
const GROUP_LABEL: &str = "Date";

/// `app.screenshots.search` — "Search" (`index.json:1577`), the input's placeholder
/// (`toolbar.vue:41`).
const SEARCH_PLACEHOLDER: &str = "Search";

/// `app.screenshots.group.new` — "New group" (`index.json:1541`), the toolbar's button
/// (`toolbar.vue:45-48`).
const NEW_GROUP: &str = "New group";

/// `app.screenshots.empty-heading` — "No screenshots yet" (`index.json:1499`), from the
/// `EmptyState` the page shows when it has nothing at all (`index.vue:1515-1519`).
const EMPTY_HEADING: &str = "No screenshots yet";

/// `app.screenshots.empty-description` — "Screenshots you take in-game will appear here."
/// (`index.json:1496`), the same state's description.
const EMPTY_BODY: &str = "Screenshots you take in-game will appear here.";

/// `app.screenshots.no-results-heading` — "No matching screenshots" (`index.json:1571`), the
/// state for a search that matches nothing (`index.vue:1520-1524`).
const NO_RESULTS_HEADING: &str = "No matching screenshots";

/// `app.screenshots.no-results-description` — "Try a different search or filter."
/// (`index.json:1568`), the same state's description.
const NO_RESULTS_BODY: &str = "Try a different search or filter.";

/// Intl's `long` month names for en-US, which is what `formatMonth` asks for
/// (`useFormatDateTime({ month: 'long', year: 'numeric' })`, `screenshots-page/index.vue:227`):
/// a group heading is `October 2026`.
const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Intl's `shortMonth` values for en-US, which is what `dateStyle: 'medium'` prints
/// (`useFormatDateTime({ dateStyle: 'medium', timeStyle: 'short' })`, `card.vue:46`):
/// a card's date line is `Oct 21, 2026, 2:32 PM`.
const MONTHS_SHORT: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

// ---------------------------------------------------------------------------
// The model
// ---------------------------------------------------------------------------

/// The list's order: the `sort` the page stores (`index.vue:149`), whose options are the
/// three `app.screenshots.sort.*` strings (`index.vue:390-394`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    /// Newest first, the stored default.
    Newest,
    /// Oldest first.
    Oldest,
    /// By file name, ascending — `localeCompare` on the ASCII names the game writes is byte
    /// order (`index.vue:414`).
    Name,
}

impl Default for Sort {
    fn default() -> Sort {
        Sort::Newest
    }
}

impl Sort {
    /// The label the sort select shows for this value.
    pub fn label(self) -> &'static str {
        match self {
            Sort::Newest => SORT_NEWEST,
            Sort::Oldest => SORT_OLDEST,
            Sort::Name => SORT_NAME,
        }
    }
}

/// The page's own state: what the toolbar has selected and what the search plate holds.
///
/// The search string is **read, not typed**. The interactive field is a sibling's
/// (`crate::widgets::search::Search`), and this page paints only the plate it sits in; a
/// string set there filters this list exactly as the source's `search` ref does
/// (`index.vue:402-411`).
#[derive(Debug, Clone)]
pub struct State {
    /// The selected order. Defaults to the source's stored `newest`.
    pub sort: Sort,
    /// The query the search field holds, as typed.
    pub search: String,
}

impl Default for State {
    fn default() -> State {
        State {
            sort: Sort::default(),
            search: String::new(),
        }
    }
}

impl State {
    /// The query to filter with, or `None` when the field asks for nothing.
    ///
    /// Whitespace is not a query, for the reason `widgets::search::Search::query` gives: a
    /// single space is what a reader leaves behind after deleting a word, and treating it as
    /// a query would empty the list instead of restoring it.
    pub fn query(&self) -> Option<&str> {
        let trimmed = self.search.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }
}

/// A wall-clock date and time, in the form the game writes into a screenshot's name
/// (`2026-10-21_14.32.07.png`).
///
/// A civil stamp rather than an epoch count: the source reads `created_at` through dayjs
/// (`index.vue:981`) and prints it through `Intl.DateTimeFormat` (`card.vue:46`), and both
/// are local-calendar operations. A stamp needs no timezone, no dependency and no clock to
/// sort or to print, and `Ord` over the field order below **is** chronological order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Stamp {
    year: i32,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
}

/// One screenshot, as far as this page reads it: the instance it belongs to, its file name
/// (which is what the source's search matches on, `index.vue:408`) and when it was taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Shot {
    instance: &'static str,
    file_name: &'static str,
    taken: Stamp,
}

/// One heading's worth of screenshots: every shot taken in the same calendar month.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Group {
    year: i32,
    month: u8,
    shots: Vec<Shot>,
}

// ---------------------------------------------------------------------------
// The rules, as pure functions
// ---------------------------------------------------------------------------

/// The source's search (`index.vue:402-411`): the query is trimmed, lowercased, and a shot
/// matches when its **file name or its instance name** contains it. No query is every shot,
/// which is what makes an untouched field show the whole list.
fn filtered(shots: &[Shot], query: Option<&str>) -> Vec<Shot> {
    let needle = match query {
        Some(text) => text.to_lowercase(),
        None => return shots.to_vec(),
    };
    shots
        .iter()
        .copied()
        .filter(|shot| {
            shot.file_name.to_lowercase().contains(&needle)
                || shot.instance.to_lowercase().contains(&needle)
        })
        .collect()
}

/// The source's order for the shots themselves (`index.vue:413-418`): by file name for
/// `Name`, and by creation time for the two date orders, either end being a reversal of the
/// other. The sort is stable, as JavaScript's `Array.prototype.sort` is.
fn in_sort_order(shots: &[Shot], sort: Sort) -> Vec<Shot> {
    let mut ordered = shots.to_vec();
    match sort {
        Sort::Name => ordered.sort_by(|a, b| a.file_name.cmp(b.file_name)),
        Sort::Newest => ordered.sort_by(|a, b| b.taken.cmp(&a.taken)),
        Sort::Oldest => ordered.sort_by(|a, b| a.taken.cmp(&b.taken)),
    }
    ordered
}

/// The source's date grouping, as a pure function of the shots: **one group per calendar
/// month, the groups newest first, no group for a month with nothing in it, and a month with
/// a single screenshot is still a group.**
///
/// This is `getDateGroup`'s month rung (`index.vue:980-1005`) with the source's own bucket id
/// (`date:month:${created.format('YYYY-MM')}`) and its own title (`formatMonth`). The rungs
/// above it — `Today`, `Yesterday`, `This week`, `This month` (`:983-999`) — are the reader's
/// clock, and this function takes none; see the module documentation.
///
/// The order is the source's too: it sorts the groups by their **newest** screenshot,
/// descending unless the sort is `Oldest` (`index.vue:511-519`). Each group here is one
/// month, so comparing the month keys is exactly that comparison; the two shots inside a
/// group keep the order they arrived in, because the source groups an array it has already
/// sorted by `sort` (`:413-418`).
fn groups(shots: &[Shot]) -> Vec<Group> {
    let mut out: Vec<Group> = Vec::new();
    for shot in shots {
        // The index is found first and the borrow taken after it, because a `find` over
        // `iter_mut` would still be live in the arm that pushes a new group.
        let index = out
            .iter()
            .position(|group| group.year == shot.taken.year && group.month == shot.taken.month);
        if let Some(index) = index {
            // `get_mut` rather than `out[index]`: an index that somehow missed is a group
            // this shot starts, not a panic.
            if let Some(group) = out.get_mut(index) {
                group.shots.push(*shot);
                continue;
            }
        }
        out.push(Group {
            year: shot.taken.year,
            month: shot.taken.month,
            shots: vec![*shot],
        });
    }
    // Newest month first; `sort_by` is stable, so two groups can never swap for a tie.
    out.sort_by(|a, b| (b.year, b.month).cmp(&(a.year, a.month)));
    out
}

/// The groups in the order the selected sort asks for: the source reverses its date
/// comparison when the sort is `Oldest` and uses the descending one otherwise — including
/// for `Name`, where the groups still run newest first (`index.vue:518`).
fn in_group_order(groups: Vec<Group>, sort: Sort) -> Vec<Group> {
    match sort {
        Sort::Oldest => groups.into_iter().rev().collect(),
        Sort::Newest | Sort::Name => groups,
    }
}

/// The heading of a month's group: `formatMonth`, i.e. `useFormatDateTime({ month: 'long',
/// year: 'numeric' })` (`index.vue:227,1003`), which prints `October 2026` in en-US.
///
/// A month outside 1-12 cannot come from a [`Stamp`] the sample or a reader writes; it still
/// answers a string rather than panicking, because a heading that reads oddly is a look that
/// is wrong and a panic is a window that does not open.
fn month_title(year: i32, month: u8) -> String {
    let name = MONTHS
        .get(usize::from(month.saturating_sub(1)))
        .copied()
        .map_or("", |name| name);
    format!("{name} {year}")
}

/// The date on a card, as the source prints it in the overlay: `formatTime` is
/// `useFormatDateTime({ dateStyle: 'medium', timeStyle: 'short' })` (`card.vue:46`), which
/// is `Oct 21, 2026, 2:32 PM` in en-US. The hour is the 12-hour clock `Intl` prints, with
/// midnight and noon as `12`.
fn stamp_label(stamp: Stamp) -> String {
    let month = MONTHS_SHORT
        .get(usize::from(stamp.month.saturating_sub(1)))
        .copied()
        .map_or("", |name| name);
    let hour = match stamp.hour % 12 {
        0 => 12,
        other => other,
    };
    let meridiem = if stamp.hour < 12 { "AM" } else { "PM" };
    format!(
        "{month} {}, {}, {hour}:{:02} {meridiem}",
        stamp.day, stamp.year, stamp.minute
    )
}

/// The instance and the date on one line, which is `app.screenshots.preview.instance-and-date`
/// — `{instance} · {date}` (`locales/en-US/index.json:1574`). In the source that line is the
/// preview's description for a global screenshot (`index.vue:668-673`); this page uses it for
/// the card's caption, because a date heading has already said the month.
fn instance_and_date(shot: Shot) -> String {
    format!("{} · {}", shot.instance, stamp_label(shot.taken))
}

/// The resolved grid: the column count and one card's size, in the page's points.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Grid {
    columns: usize,
    card_width: f32,
    card_height: f32,
}

/// The source's column count (`index.vue:522-526`), which is the ladder its grid classes
/// spell out: `grid-cols-1 sm:grid-cols-2 2xl:grid-cols-4` (`group.vue:210`).
fn column_count(window_width: f32) -> usize {
    if window_width >= WIDE_BREAKPOINT {
        WIDE_COLUMNS
    } else if window_width >= NARROW_BREAKPOINT {
        NARROW_COLUMNS
    } else {
        SLIM_COLUMNS
    }
}

/// The source's `screenshotCardWidth` (`index.vue:528-532`) and `screenshotCardHeight`
/// (`:534`): the list's width less the gaps, over the column count — or the 320-point
/// fallback while the list has not been measured.
fn grid_layout(list_width: f32, window_width: f32) -> Grid {
    let columns = column_count(window_width);
    let gaps = GRID_GAP * (columns.saturating_sub(1)) as f32;
    let card_width = if list_width <= 0.0 {
        FALLBACK_CARD_WIDTH
    } else {
        ((list_width - gaps) / columns as f32).max(0.0)
    };
    Grid {
        columns,
        card_width,
        card_height: card_width * CARD_ASPECT,
    }
}

/// One group's block height, from the source's own sum (`index.vue:544-554`): the header,
/// the 10-point gap under it, the grid's height, and the group's 12-point tail.
///
/// The grid's height is `rows * cardHeight + (rows - 1) * gap` for a group with screenshots,
/// and the 45-point minimum for one without (`:545-548`).
fn group_height(columns: usize, card_height: f32, count: usize) -> f32 {
    let rows = count.div_ceil(columns.max(1));
    let grid_height = if rows == 0 {
        GRID_MIN_HEIGHT
    } else {
        rows as f32 * card_height + rows.saturating_sub(1) as f32 * GRID_GAP
    };
    GROUP_HEADER_HEIGHT + GROUP_CONTENT_SPACING + grid_height + GROUP_SPACING
}

// ---------------------------------------------------------------------------
// The page
// ---------------------------------------------------------------------------

/// Draw the screenshots page into `rect`. Returns whether another frame is owed: nothing
/// here animates yet, so it is always `false`.
pub fn draw(
    ui: &mut Ui,
    rect: Rect,
    theme: &Theme,
    images: &mut Images,
    state: &mut State,
) -> bool {
    // The page owns its rect (`pages/mod.rs`'s seam); paint the page colour first so the
    // cards sit on `--color-bg` even if the caller did not clear the column.
    ui.painter().rect_filled(rect, 0, theme.BG);

    if rect.width() <= 2.0 * PAGE_PADDING || rect.height() <= 2.0 * PAGE_PADDING {
        return false;
    }

    // The whole set and the set the search leaves, kept apart: the source branches on
    // `screenshots.length` for "nothing yet" and on `filteredScreenshots.length` for
    // "nothing matched" (`index.vue:1515-1525`), and the two say different things.
    let visible = in_sort_order(&filtered(&SHOTS, state.query()), state.sort);

    if SHOTS.is_empty() {
        empty_state(
            ui.painter(),
            rect,
            theme,
            Glyph::Images,
            EMPTY_HEADING,
            EMPTY_BODY,
        );
        return false;
    }

    // Heading and toolbar, and the y the list starts at. The painter borrows `ui` only for
    // the length of the block, because the scroll area below needs `&mut ui`.
    let list_top = {
        let painter = ui.painter();
        heading_and_toolbar(painter, rect, theme, state, SHOTS.len())
    };

    let list_rect = Rect::from_min_max(
        egui::pos2(rect.left() + PAGE_PADDING, list_top),
        egui::pos2(rect.right() - PAGE_PADDING, rect.bottom() - PAGE_PADDING),
    );
    if list_rect.width() < 1.0 || list_rect.height() < 1.0 {
        return false;
    }

    // `gridWidth` is the measured content box (`useElementSize`, `index.vue:224`), which is
    // this page's list rect; `windowWidth` is the *window's* (`useWindowSize`, `:225`), and
    // it picks the column count before any width is divided, so it cannot be the list's own
    // width. egui 0.36 calls the window's content rectangle `Context::content_rect`.
    let grid = grid_layout(list_rect.width(), ui.ctx().content_rect().width());

    // The state for a search that matches nothing: the toolbar stays, so the query can be
    // cleared, and the list area says why it is empty (`index.vue:1520-1524`).
    if visible.is_empty() {
        empty_state(
            ui.painter(),
            list_rect,
            theme,
            Glyph::Search,
            NO_RESULTS_HEADING,
            NO_RESULTS_BODY,
        );
        return false;
    }

    let blocks: Vec<(Group, f32)> = in_group_order(groups(&visible), state.sort)
        .into_iter()
        .map(|group| {
            let height = group_height(grid.columns, grid.card_height, group.shots.len());
            (group, height)
        })
        .collect();
    let content_height: f32 = blocks.iter().map(|(_, height)| *height).sum();

    // The source scrolls the whole page; this page owns the column below its toolbar, so the
    // scroll area starts at the list and the heading stays put.
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(list_rect));
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(&mut child, |ui| {
            // Reserve the whole list height so the scroll area knows what is below the fold;
            // the blocks are then painted at the source's absolute positions.
            let (area, _) =
                ui.allocate_exact_size(egui::vec2(list_rect.width(), content_height), Sense::hover());
            let painter = ui.painter();
            let mut top = area.top();
            for (group, height) in &blocks {
                draw_group(
                    painter,
                    egui::pos2(area.left(), top),
                    list_rect.width(),
                    grid,
                    group,
                    theme,
                    images,
                );
                top += *height;
            }
        });

    false
}

// ---------------------------------------------------------------------------
// Heading and toolbar
// ---------------------------------------------------------------------------

/// Draw the heading, its count and the toolbar's two rows. Returns the y where the list
/// begins.
fn heading_and_toolbar(
    painter: &egui::Painter,
    rect: Rect,
    theme: &Theme,
    state: &State,
    count: usize,
) -> f32 {
    let heading_at = egui::pos2(rect.left() + PAGE_PADDING, rect.top() + PAGE_PADDING);
    // `h1.m-0.text-2xl.font-bold.text-contrast` (`index.vue:1486`). `font-bold` is Inter
    // 700, one weight above the library's `text-2xl font-semibold` heading.
    let heading = painter.text(
        heading_at,
        Align2::LEFT_TOP,
        HEADING,
        fonts::title(HEADING_SIZE),
        theme.CONTRAST,
    );

    // The source has no page-level count: its count belongs to each group's header
    // (`section.vue:148-150`). This page puts the total beside the heading, drawn with the
    // same pill, so a capture says how many screenshots the list holds.
    count_pill(
        painter,
        heading.right() + HEADER_GAP,
        heading_at.y + HEADING_LINE / 2.0,
        count,
        theme,
    );

    // Row 1: the search plate and the `New group` button (`toolbar.vue:36-49`), the field
    // `flex-1` at a 256-point floor and the button content-sized after it.
    let row_top = heading_at.y + HEADING_LINE + SECTION_GAP;
    let button_width = text_button_width(painter, NEW_GROUP, theme);
    let search_width = (rect.width() - 2.0 * PAGE_PADDING - CONTROL_GAP - button_width)
        .max(SEARCH_MIN_WIDTH);
    let search = Rect::from_min_size(
        egui::pos2(rect.left() + PAGE_PADDING, row_top),
        egui::vec2(search_width, CONTROL_HEIGHT),
    );
    search_plate(painter, search, theme, state);
    text_button(
        painter,
        egui::pos2(search.right() + CONTROL_GAP, row_top),
        button_width,
        NEW_GROUP,
        Glyph::SquarePlus,
        theme,
    );

    // Row 2: the sort and group `Combobox`es, side by side (`toolbar.vue:50-85`). There is
    // no third control — this toolbar has no view toggle.
    let selects_top = row_top + CONTROL_HEIGHT + TOOLBAR_ROW_GAP;
    let sort = select(
        painter,
        egui::pos2(rect.left() + PAGE_PADDING, selects_top),
        theme,
        Glyph::ArrowUpDown,
        state.sort.label(),
    );
    select(
        painter,
        egui::pos2(sort.right() + CONTROL_GAP, selects_top),
        theme,
        Glyph::LayoutGrid,
        GROUP_LABEL,
    );

    selects_top + CONTROL_HEIGHT + SECTION_GAP
}

/// The search field's plate: what the source's `Input` looks like, painted and not
/// interactive.
///
/// `InputFrame` at its `standard` size is `h-9 rounded-xl px-3 gap-2 border
/// border-surface-5 bg-surface-4` (`InputFrame.vue:4,47`), with a 20-point leading glyph in
/// `text-secondary` (`Input.vue:12`), the text at `text-base font-medium text-primary` and
/// the placeholder in `text-secondary` (`Input.vue:51,55`). The field is `clearable`
/// (`toolbar.vue:42`), so a non-empty query has the 20-point `X` at its right end
/// (`InputClearButton.vue:4`).
///
/// The keyboard, the caret and the edit are a sibling's: `crate::widgets::search::Search::ui`
/// takes exactly this rect. This function is only the plate it will sit in.
fn search_plate(painter: &egui::Painter, rect: Rect, theme: &Theme, state: &State) {
    painter.rect_filled(rect, CONTROL_RADIUS, theme.SURFACE_4);
    painter.rect_stroke(
        rect,
        CONTROL_RADIUS,
        Stroke::new(STROKE_WIDTH, theme.SURFACE_5),
        egui::StrokeKind::Inside,
    );

    icons::draw(
        painter,
        Glyph::Search,
        Rect::from_center_size(
            egui::pos2(
                rect.left() + SEARCH_PADDING + SEARCH_GLYPH / 2.0,
                rect.center().y,
            ),
            Vec2::splat(SEARCH_GLYPH),
        ),
        // The source draws this glyph at 60% opacity (`Input.vue:12`); the colour is the same
        // `--color-secondary` at full strength, because a washed glyph is two points of
        // difference and a second colour token would be a second source of truth.
        theme.SECONDARY,
    );

    let clear = Rect::from_center_size(
        egui::pos2(
            rect.right() - SEARCH_PADDING - SEARCH_GLYPH / 2.0,
            rect.center().y,
        ),
        Vec2::splat(SEARCH_GLYPH),
    );
    let text_left = rect.left() + SEARCH_PADDING + SEARCH_GLYPH + SEARCH_INNER_GAP;
    let empty = state.search.is_empty();
    let text_right = if empty {
        rect.right() - SEARCH_PADDING
    } else {
        clear.left() - SEARCH_INNER_GAP
    };

    let (contents, ink) = if empty {
        (SEARCH_PLACEHOLDER, theme.SECONDARY)
    } else {
        (state.search.as_str(), theme.TEXT_PRIMARY)
    };
    let galley = painter.layout_no_wrap(contents.to_owned(), fonts::label(SEARCH_TEXT_SIZE), ink);
    painter
        .with_clip_rect(Rect::from_min_max(
            egui::pos2(text_left, rect.top()),
            egui::pos2(text_right, rect.bottom()),
        ))
        .galley(
            egui::pos2(text_left, rect.center().y - galley.size().y / 2.0),
            galley,
            ink,
        );

    if !empty {
        icons::draw(painter, Glyph::X, clear, theme.SECONDARY);
    }
}

/// The width of a text button with a leading glyph: the frame's two paddings, the glyph, the
/// gap and the label — `px-2.5 ... gap-1.5 ... [&>svg]:size-5` (`ButtonFrame.vue:31`).
fn text_button_width(painter: &egui::Painter, label: &str, theme: &Theme) -> f32 {
    let galley = painter.layout_no_wrap(
        label.to_owned(),
        fonts::semibold(BUTTON_TEXT_SIZE),
        theme.CONTRAST,
    );
    2.0 * BUTTON_PADDING + BUTTON_GLYPH + BUTTON_GAP + galley.size().x
}

/// A `Button` at its default type and `md` size: `bg-surface-4 text-contrast` with a
/// 1-point inset `--surface-5` ring (`ButtonFrame.vue:45,134-140`), `h-9 rounded-xl px-2.5
/// gap-1.5`, the glyph in `text-primary` and the label at `text-base font-semibold`
/// (`ButtonFrame.vue:31`).
fn text_button(
    painter: &egui::Painter,
    min: egui::Pos2,
    width: f32,
    label: &str,
    glyph: Glyph,
    theme: &Theme,
) {
    let rect = Rect::from_min_size(min, egui::vec2(width, CONTROL_HEIGHT));
    painter.rect_filled(rect, CONTROL_RADIUS, theme.SURFACE_4);
    painter.rect_stroke(
        rect,
        CONTROL_RADIUS,
        Stroke::new(STROKE_WIDTH, theme.SURFACE_5),
        egui::StrokeKind::Inside,
    );
    icons::draw(
        painter,
        glyph,
        Rect::from_center_size(
            egui::pos2(
                rect.left() + BUTTON_PADDING + BUTTON_GLYPH / 2.0,
                rect.center().y,
            ),
            Vec2::splat(BUTTON_GLYPH),
        ),
        theme.TEXT_PRIMARY,
    );
    painter.text(
        egui::pos2(
            rect.left() + BUTTON_PADDING + BUTTON_GLYPH + BUTTON_GAP,
            rect.center().y,
        ),
        Align2::LEFT_CENTER,
        label,
        fonts::semibold(BUTTON_TEXT_SIZE),
        theme.CONTRAST,
    );
}

/// One `Combobox` trigger, drawn the way its `ButtonFrame` resolves at `md`: `h-9 rounded-xl
/// px-2.5 ... gap-1.5 [&>svg]:size-5` (`ButtonFrame.vue:31`), a 20-point prefix in
/// `text-primary` (`toolbar.vue:60`), the selected label at `font-semibold` inheriting
/// `text-contrast` (`Combobox.vue:80`), and a 20-point down chevron in the suffix row
/// (`Combobox.vue:88-93`). Returns the rect it used.
fn select(
    painter: &egui::Painter,
    min: egui::Pos2,
    theme: &Theme,
    glyph: Glyph,
    label: &str,
) -> Rect {
    let label_galley =
        painter.layout_no_wrap(label.to_owned(), fonts::semibold(SELECT_LABEL_SIZE), theme.CONTRAST);
    let width = 2.0 * SELECT_PADDING
        + SELECT_GLYPH
        + SELECT_INNER_GAP
        + label_galley.size().x
        + SELECT_SUFFIX_GAP
        + SELECT_GLYPH;
    let rect = Rect::from_min_size(min, egui::vec2(width, CONTROL_HEIGHT));
    painter.rect_filled(rect, CONTROL_RADIUS, theme.BUTTON_BG);
    painter.rect_stroke(
        rect,
        CONTROL_RADIUS,
        Stroke::new(STROKE_WIDTH, theme.SURFACE_5),
        egui::StrokeKind::Inside,
    );

    icons::draw(
        painter,
        glyph,
        Rect::from_center_size(
            egui::pos2(
                rect.left() + SELECT_PADDING + SELECT_GLYPH / 2.0,
                rect.center().y,
            ),
            Vec2::splat(SELECT_GLYPH),
        ),
        theme.TEXT_PRIMARY,
    );
    painter.text(
        egui::pos2(
            rect.left() + SELECT_PADDING + SELECT_GLYPH + SELECT_INNER_GAP,
            rect.center().y,
        ),
        Align2::LEFT_CENTER,
        label,
        fonts::semibold(SELECT_LABEL_SIZE),
        theme.CONTRAST,
    );
    icons::draw(
        painter,
        Glyph::ChevronDown,
        Rect::from_center_size(
            egui::pos2(
                rect.right() - SELECT_PADDING - SELECT_GLYPH / 2.0,
                rect.center().y,
            ),
            Vec2::splat(SELECT_GLYPH),
        ),
        theme.CONTRAST,
    );
    rect
}

/// The count pill, which is `TagItem`'s shape: `px-2 py-1 text-sm leading-none
/// rounded-full` with a 1-point border (`base/TagItem.vue:19`), in the colours the group
/// header gives it — `border-surface-3 bg-surface-2` (`section.vue:148`).
fn count_pill(painter: &egui::Painter, left: f32, centre_y: f32, count: usize, theme: &Theme) {
    let text = count.to_string();
    let galley = painter.layout_no_wrap(
        text.clone(),
        fonts::body(BADGE_TEXT_SIZE),
        theme.SECONDARY,
    );
    let pill = Rect::from_min_size(
        egui::pos2(left, centre_y - BADGE_HEIGHT / 2.0),
        egui::vec2(galley.size().x + 2.0 * BADGE_PADDING, BADGE_HEIGHT),
    );
    painter.rect_filled(pill, BADGE_RADIUS, theme.SURFACE_2);
    painter.rect_stroke(
        pill,
        BADGE_RADIUS,
        Stroke::new(STROKE_WIDTH, theme.SURFACE_3),
        egui::StrokeKind::Inside,
    );
    painter.text(
        pill.center(),
        Align2::CENTER_CENTER,
        text,
        fonts::body(BADGE_TEXT_SIZE),
        theme.SECONDARY,
    );
}

// ---------------------------------------------------------------------------
// The groups
// ---------------------------------------------------------------------------

/// One group: the source's header row (`section.vue:112-154`) over its grid (`group.vue:208`).
fn draw_group(
    painter: &egui::Painter,
    min: egui::Pos2,
    width: f32,
    grid: Grid,
    group: &Group,
    theme: &Theme,
    images: &mut Images,
) {
    let header = Rect::from_min_size(min, egui::vec2(width, GROUP_HEADER_HEIGHT));

    // The rule under the header: `border-b border-surface-5` (`section.vue:114`), inside the
    // 40-point row because the sheet's `box-sizing` is `border-box`.
    painter.rect_filled(
        Rect::from_min_size(
            egui::pos2(min.x, header.bottom() - STROKE_WIDTH),
            egui::vec2(width, STROKE_WIDTH),
        ),
        0,
        theme.SURFACE_5,
    );

    // `DropdownIcon` at `size-5 text-secondary`, rotated `rotate-180` while the group is open
    // (`section.vue:124-127`) — the sheet's chevron points down, so an open group's points up.
    // Every group on this page is drawn open.
    icons::draw(
        painter,
        Glyph::ChevronUp,
        Rect::from_center_size(
            egui::pos2(min.x + HEADER_GLYPH / 2.0, header.center().y),
            Vec2::splat(HEADER_GLYPH),
        ),
        theme.SECONDARY,
    );

    // The title: `text-base font-semibold text-primary` (`section.vue:144`).
    let title = painter.text(
        egui::pos2(
            min.x + HEADER_GLYPH + HEADER_GAP,
            header.center().y,
        ),
        Align2::LEFT_CENTER,
        month_title(group.year, group.month),
        fonts::semibold(GROUP_TITLE_SIZE),
        theme.TEXT_PRIMARY,
    );

    count_pill(
        painter,
        title.right() + HEADER_GAP,
        header.center().y,
        group.shots.len(),
        theme,
    );

    // The grid, `mt-2.5` under the header (`section.vue:164`) at `gap-3` (`group.vue:210`),
    // in the source's absolute positions: column-major within a row, rows one card apart.
    let grid_top = header.bottom() + GROUP_CONTENT_SPACING;
    for (index, shot) in group.shots.iter().enumerate() {
        let column = index % grid.columns.max(1);
        let row = index / grid.columns.max(1);
        let card = Rect::from_min_size(
            egui::pos2(
                min.x + column as f32 * (grid.card_width + GRID_GAP),
                grid_top + row as f32 * (grid.card_height + GRID_GAP),
            ),
            egui::vec2(grid.card_width, grid.card_height),
        );
        draw_card(painter, card, theme, shot, images);
    }
}

/// One screenshot card.
///
/// `article` is `aspect-video min-w-0 overflow-hidden rounded-xl border border-solid
/// border-surface-5 bg-surface-2` (`card.vue:172`), and its caption is the hover overlay at
/// the foot (`:232-277`).
fn draw_card(painter: &egui::Painter, rect: Rect, theme: &Theme, shot: &Shot, images: &mut Images) {
    if rect.width() < 1.0 || rect.height() < 1.0 {
        return;
    }

    // The plate, in the image's aspect (`aspect-video`, `card.vue:172`). The reference fills
    // the card with `bg-surface-2` and covers it with the thumbnail (`card.vue:172,222`),
    // showing `bg-button-bg` while the image decodes (`:227-231`); with no image to load the
    // plate is `theme.SUPER_RAISED_BG`, one step above the page, so the card reads as a card
    // rather than as a hole.
    painter.rect_filled(rect, CARD_RADIUS, theme.SUPER_RAISED_BG);

    // **The image's call site is the line below.** The reference asks
    // `useImageThumbnail(path, 512, modified_at)` for a 512-point thumbnail (`card.vue:37-41`),
    // decoded off the frame thread and faded in over 350 ms (`:281-285`). This is where that
    // becomes a texture:
    //
    //     if let Some(texture) = images.get(painter.ctx(), shot.path) {
    //         painter.image(texture.id(), rect, UV, Color32::WHITE);
    //     }
    //
    // `Shot` carries no path yet, so nothing can be asked for and the cache stays idle.
    let _ = images;

    draw_overlay(painter, rect, theme, shot);

    // The border last, over the scrim: `border border-surface-5` (`card.vue:172`).
    painter.rect_stroke(
        rect,
        CARD_RADIUS,
        Stroke::new(STROKE_WIDTH, theme.SURFACE_5),
        egui::StrokeKind::Inside,
    );
}

/// The card's foot, drawn at rest: the gradient, the file name, the instance-and-date line
/// and the three actions (`card.vue:232-277`).
///
/// The reference fades this in on hover (`opacity-0 ... group-hover:opacity-100`) and slides
/// the actions up 4 points as it arrives; this page has no hover state, so what is drawn is
/// the overlay's resolved state — the card's press hint, since the card itself is the button
/// that opens the viewer (`card.vue:168-193`).
///
/// The gradient is `bg-gradient-to-t from-surface-1 to-transparent` with `pt-[120px]`
/// (`card.vue:233`). Three stacked bands sample that ramp instead of a per-pixel mesh, and
/// they are clipped to the plate: `--surface-1` is `--color-bg` in both schemes
/// (`theme.rs` — `SURFACE_1` and `BG` hold the same value), so the corner triangles the
/// rectangular clip cannot round out are the page's own colour and do not show.
fn draw_overlay(painter: &egui::Painter, plate: Rect, theme: &Theme, shot: &Shot) {
    let painter = painter.with_clip_rect(plate);

    let fade = OVERLAY_FADE.min(plate.height());
    for (share, alpha) in [
        (1.0, SCRIM_ALPHA_TOP),
        (2.0 / 3.0, SCRIM_ALPHA_MIDDLE),
        (1.0 / 3.0, SCRIM_ALPHA_BOTTOM),
    ] {
        let band = Rect::from_min_max(
            egui::pos2(plate.left(), plate.bottom() - fade * share),
            plate.max,
        );
        painter.rect_filled(band, 0, scrim(theme, alpha));
    }

    let inner = Rect::from_min_max(
        egui::pos2(plate.left() + OVERLAY_PADDING, plate.top()),
        egui::pos2(
            plate.right() - OVERLAY_PADDING,
            plate.bottom() - OVERLAY_PADDING,
        ),
    );
    let actions_width = 3.0 * ACTION_SIZE + 2.0 * ACTION_GAP;
    // The actions are dropped rather than overlapped on a card too narrow to hold them and
    // its text: the text is what the caption is for.
    let actions_left = if inner.width() >= actions_width + OVERLAY_GAP {
        Some(inner.right() - actions_width)
    } else {
        None
    };
    let text_width = match actions_left {
        Some(left) => left - OVERLAY_GAP - inner.left(),
        None => inner.width(),
    };

    // Two lines, bottom-aligned: the file name and the instance-and-date line
    // (`card.vue:236-241`).
    let date_top = inner.bottom() - DATE_LINE;
    clipped_line(
        &painter,
        Rect::from_min_size(
            egui::pos2(inner.left(), date_top - CAPTION_LINE),
            egui::vec2(text_width.max(1.0), CAPTION_LINE),
        ),
        shot.file_name,
        fonts::semibold(CAPTION_SIZE),
        theme.CONTRAST,
    );
    // `app.screenshots.preview.instance-and-date` — "{instance} · {date}"
    // (`locales/en-US/index.json:1574`).
    clipped_line(
        &painter,
        Rect::from_min_size(
            egui::pos2(inner.left(), date_top),
            egui::vec2(text_width.max(1.0), DATE_LINE),
        ),
        &instance_and_date(*shot),
        fonts::body(DATE_SIZE),
        theme.SECONDARY,
    );

    if let Some(left) = actions_left {
        for (index, glyph) in [
            Glyph::Edit,
            Glyph::ClipboardCopy,
            Glyph::MoreHorizontal,
        ]
        .into_iter()
        .enumerate()
        {
            let centre = egui::pos2(
                left + index as f32 * (ACTION_SIZE + ACTION_GAP) + ACTION_SIZE / 2.0,
                inner.bottom() - ACTION_SIZE / 2.0,
            );
            // A quiet `IconButton` carrying `bg-surface-2 text-contrast` (`card.vue:248-275`),
            // circular by default (`IconButton.vue:29`) — 36 points across.
            painter.circle_filled(centre, ACTION_SIZE / 2.0, theme.SURFACE_2);
            icons::draw(
                &painter,
                glyph,
                Rect::from_center_size(centre, Vec2::splat(ACTION_GLYPH)),
                theme.CONTRAST,
            );
        }
    }
}

/// One band of the overlay's scrim: `--surface-1` at `alpha`, which is the `from-surface-1`
/// end of the source's gradient (`card.vue:233`).
fn scrim(theme: &Theme, alpha: u8) -> Color32 {
    let base = theme.SURFACE_1;
    Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), alpha)
}

/// Draw one line of text, clipped to its own box.
///
/// The overlay truncates both of its lines (`truncate`, `card.vue:236,239`): a long file name
/// or instance ends at the caption's edge instead of wrapping into the line below it.
fn clipped_line(painter: &egui::Painter, rect: Rect, text: &str, font: egui::FontId, ink: Color32) {
    let galley = painter.layout(text.to_owned(), font, ink, rect.width());
    painter.with_clip_rect(rect).galley(rect.min, galley, ink);
}

// ---------------------------------------------------------------------------
// The empty states
// ---------------------------------------------------------------------------

/// The source's `EmptyState` (`base/EmptyState.vue`): a 200-point illustration, the heading at
/// 24/32 semibold `text-contrast`, 6 points, and the description at 16/24 `text-secondary`,
/// centred in the area it is given — the page root is `justify-center` while nothing is
/// listed (`index.vue:1483`).
fn empty_state(
    painter: &egui::Painter,
    rect: Rect,
    theme: &Theme,
    glyph: Glyph,
    heading: &str,
    body: &str,
) {
    let total = ILLUSTRATION_HEIGHT + HEADING_LINE + EMPTY_TEXT_GAP + EMPTY_BODY_LINE;
    if rect.height() < total || rect.width() < 1.0 {
        return;
    }
    let top = rect.center().y - total / 2.0;
    let centre_x = rect.center().x;

    // The illustration's box (`h-[200px] w-auto`, `EmptyState.vue:4`) with the icon set's
    // stand-in at its centre: this build carries no `NoImagesIllustration` asset.
    icons::draw(
        painter,
        glyph,
        Rect::from_center_size(
            egui::pos2(centre_x, top + ILLUSTRATION_HEIGHT / 2.0),
            Vec2::splat(ILLUSTRATION_GLYPH),
        ),
        theme.SECONDARY,
    );

    let heading_y = top + ILLUSTRATION_HEIGHT + HEADING_LINE / 2.0;
    painter.text(
        egui::pos2(centre_x, heading_y),
        Align2::CENTER_CENTER,
        heading,
        fonts::semibold(HEADING_SIZE),
        theme.CONTRAST,
    );
    painter.text(
        egui::pos2(
            centre_x,
            heading_y + HEADING_LINE / 2.0 + EMPTY_TEXT_GAP + EMPTY_BODY_LINE / 2.0,
        ),
        Align2::CENTER_CENTER,
        body,
        fonts::body(EMPTY_BODY_SIZE),
        theme.SECONDARY,
    );
}

// ---------------------------------------------------------------------------
// Sample data
// ---------------------------------------------------------------------------

/// Nine screenshots across three instances and two dates, so both dimensions of the grouping
/// are visible in one capture: the month headings split the list in two (October 2026, then
/// September 2026), and every card's caption names its own instance, which is what the
/// source's date mode does with `showInstanceName` (`index.vue:1569`, `card.vue:240`).
///
/// The names are the game's own file-name format — `2026-10-21_14.32.07.png` — which is also
/// what the search matches on and what `Name` sorts by. One instance name is long enough to
/// be truncated in a card's caption.
const SHOTS: [Shot; 9] = [
    Shot {
        instance: "Fabulously Optimized",
        file_name: "2026-10-21_14.32.07.png",
        taken: Stamp {
            year: 2026,
            month: 10,
            day: 21,
            hour: 14,
            minute: 32,
        },
    },
    Shot {
        instance: "Vanilla Plus",
        file_name: "2026-10-21_09.05.41.png",
        taken: Stamp {
            year: 2026,
            month: 10,
            day: 21,
            hour: 9,
            minute: 5,
        },
    },
    Shot {
        instance: "Create: Above and Beyond",
        file_name: "2026-10-19_22.47.50.png",
        taken: Stamp {
            year: 2026,
            month: 10,
            day: 19,
            hour: 22,
            minute: 47,
        },
    },
    Shot {
        instance: "Fabulously Optimized",
        file_name: "2026-10-14_11.20.55.png",
        taken: Stamp {
            year: 2026,
            month: 10,
            day: 14,
            hour: 11,
            minute: 20,
        },
    },
    Shot {
        instance: "Vanilla Plus",
        file_name: "2026-10-02_18.03.12.png",
        taken: Stamp {
            year: 2026,
            month: 10,
            day: 2,
            hour: 18,
            minute: 3,
        },
    },
    Shot {
        instance: "Create: Above and Beyond",
        file_name: "2026-09-30_16.58.09.png",
        taken: Stamp {
            year: 2026,
            month: 9,
            day: 30,
            hour: 16,
            minute: 58,
        },
    },
    Shot {
        instance: "Fabulously Optimized",
        file_name: "2026-09-24_20.11.03.png",
        taken: Stamp {
            year: 2026,
            month: 9,
            day: 24,
            hour: 20,
            minute: 11,
        },
    },
    Shot {
        instance: "Vanilla Plus",
        file_name: "2026-09-18_08.39.27.png",
        taken: Stamp {
            year: 2026,
            month: 9,
            day: 18,
            hour: 8,
            minute: 39,
        },
    },
    Shot {
        instance: "Create: Above and Beyond",
        file_name: "2026-09-03_13.26.44.png",
        taken: Stamp {
            year: 2026,
            month: 9,
            day: 3,
            hour: 13,
            minute: 26,
        },
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    /// A shot with only the fields a grouping test reads.
    fn shot(instance: &'static str, name: &'static str, year: i32, month: u8, day: u8) -> Shot {
        Shot {
            instance,
            file_name: name,
            taken: Stamp {
                year,
                month,
                day,
                hour: 12,
                minute: 0,
            },
        }
    }

    /// The shell's content column at a 1440 window, the same identity `library.rs` pins:
    /// `PANEL_LEFT - CONTENT_LEFT - 16`.
    fn page_width_at(window_width: f32) -> f32 {
        window_width
            - crate::geometry::RAIL
            - crate::geometry::PANEL
            - crate::geometry::CONTENT_INSET
            - 16.0
    }

    /// **Ordering.** The groups run newest month first whatever order they arrive in, which is
    /// the source's comparison on each group's newest screenshot (`index.vue:511-519`).
    #[test]
    fn groups_are_ordered_by_month_with_the_newest_first() {
        let shots = [
            shot("a", "2026-05-02_10.00.00.png", 2026, 5, 2),
            shot("a", "2026-07-19_10.00.00.png", 2026, 7, 19),
            shot("a", "2026-12-24_10.00.00.png", 2026, 12, 24),
            shot("a", "2026-07-01_10.00.00.png", 2026, 7, 1),
        ];
        let out = groups(&shots);
        let months: Vec<(i32, u8)> = out.iter().map(|group| (group.year, group.month)).collect();
        assert_eq!(
            months,
            [(2026, 12), (2026, 7), (2026, 5)],
            "December, then July, then May — the input's order is not the output's"
        );
        assert_eq!(out[1].shots.len(), 2, "July holds the two shots taken in July");
    }

    /// **Empty groups dropped.** The month ladder has a hole in it — no August between July and
    /// May — and the hole is not a group. There is no `Group` with an empty `shots` in the
    /// output, so a heading can never be drawn over nothing.
    #[test]
    fn a_month_with_no_screenshots_is_not_a_group() {
        let shots = [
            shot("a", "2026-07-19_10.00.00.png", 2026, 7, 19),
            shot("a", "2026-05-02_10.00.00.png", 2026, 5, 2),
        ];
        let out = groups(&shots);
        assert_eq!(out.len(), 2, "July and May, and no June or August between them");
        assert!(
            out.iter().all(|group| !group.shots.is_empty()),
            "every group the rule emits has at least one screenshot"
        );
        assert_eq!(groups(&[]).len(), 0, "an empty list has no groups at all");
    }

    /// **A group with one item is still a group.** The source draws a heading and its count
    /// for a single shot exactly as it does for nine; a lone screenshot must not be folded
    /// into its neighbour or handed back ungrouped.
    #[test]
    fn a_group_holding_one_screenshot_is_still_a_group() {
        let shots = [
            shot("a", "2026-10-21_14.32.07.png", 2026, 10, 21),
            shot("a", "2026-09-30_16.58.09.png", 2026, 9, 30),
            shot("a", "2026-09-02_08.00.00.png", 2026, 9, 2),
        ];
        let out = groups(&shots);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].shots.len(), 1, "October's group holds its one shot");
        assert_eq!(out[1].shots.len(), 2);
        assert_eq!(month_title(out[0].year, out[0].month), "October 2026");
    }

    /// The shots inside a group keep the order they arrived in, because the source groups an
    /// array it has already sorted (`index.vue:413-418`): the caller's sort is the group's
    /// order, and `groups` must not impose a second one.
    #[test]
    fn a_groups_shots_keep_the_order_they_arrived_in() {
        let newest = shot("a", "2026-10-21_14.32.07.png", 2026, 10, 21);
        let oldest = shot("a", "2026-10-02_18.03.12.png", 2026, 10, 2);
        let out = groups(&[oldest, newest]);
        assert_eq!(out[0].shots, vec![oldest, newest], "input order, not re-sorted");

        let by_name = in_sort_order(&[newest, oldest], Sort::Name);
        let out = groups(&by_name);
        assert_eq!(
            out[0].shots,
            vec![oldest, newest],
            "a name sort reaches the group's contents"
        );
    }

    /// The source reverses the group order for `Oldest` and leaves it descending — even for a
    /// name sort — otherwise (`index.vue:518`).
    #[test]
    fn the_group_order_follows_the_sort() {
        let shots = [
            shot("a", "2026-05-02_10.00.00.png", 2026, 5, 2),
            shot("a", "2026-07-19_10.00.00.png", 2026, 7, 19),
        ];
        let keys = |groups: Vec<Group>| -> Vec<u8> {
            groups.iter().map(|group| group.month).collect()
        };
        assert_eq!(keys(in_group_order(groups(&shots), Sort::Newest)), vec![7, 5]);
        assert_eq!(keys(in_group_order(groups(&shots), Sort::Oldest)), vec![5, 7]);
        assert_eq!(
            keys(in_group_order(groups(&shots), Sort::Name)),
            vec![7, 5],
            "a name sort keeps the newest group first"
        );
    }

    /// The three sorts, from the source's comparator (`index.vue:413-418`).
    #[test]
    fn the_three_sorts_order_the_month_s_screenshots() {
        let early = shot("a", "2026-10-02_18.03.12.png", 2026, 10, 2);
        let late = shot("a", "2026-10-21_14.32.07.png", 2026, 10, 21);
        assert_eq!(
            in_sort_order(&[early, late], Sort::Newest),
            vec![late, early]
        );
        assert_eq!(
            in_sort_order(&[late, early], Sort::Oldest),
            vec![early, late]
        );
        assert_eq!(in_sort_order(&[late, early], Sort::Name), vec![early, late]);
    }

    /// The search matches a file name and an instance name, ignores case, and asks for
    /// nothing when it is empty or only spaces (`index.vue:402-411`, and `State::query`).
    #[test]
    fn the_search_matches_names_and_instances() {
        let shots = [
            shot("Fabulously Optimized", "2026-10-21_14.32.07.png", 2026, 10, 21),
            shot("Vanilla Plus", "2026-09-30_16.58.09.png", 2026, 9, 30),
        ];
        let all = filtered(&shots, None);
        assert_eq!(all.len(), 2, "an empty query is every screenshot");

        let by_instance = filtered(&shots, Some("vanilla"));
        assert_eq!(by_instance.len(), 1, "an instance name matches, case-insensitively");
        assert_eq!(by_instance[0].instance, "Vanilla Plus");

        let by_file = filtered(&shots, Some("2026-09-30"));
        assert_eq!(by_file.len(), 1);
        assert_eq!(by_file[0].instance, "Vanilla Plus");

        assert_eq!(filtered(&shots, Some("optifine")).len(), 0, "no match is no shot");

        let mut state = State::default();
        assert_eq!(state.query(), None, "a fresh page asks for nothing");
        state.search = "   ".to_owned();
        assert_eq!(state.query(), None, "spaces are not a query");
        state.search = "  vanity  ".to_owned();
        assert_eq!(state.query(), Some("vanity"), "the query is trimmed at its use");
        assert_eq!(filtered(&shots, state.query()).len(), 0);
    }

    /// The month heading and the card's date are the two `Intl` formats the source asks for:
    /// `month: 'long', year: 'numeric'` and `dateStyle: 'medium', timeStyle: 'short'`.
    #[test]
    fn the_dates_read_the_way_intl_prints_them() {
        assert_eq!(month_title(2026, 10), "October 2026");
        assert_eq!(month_title(2026, 1), "January 2026");
        assert_eq!(
            stamp_label(Stamp {
                year: 2026,
                month: 10,
                day: 21,
                hour: 14,
                minute: 32,
            }),
            "Oct 21, 2026, 2:32 PM"
        );
        assert_eq!(
            stamp_label(Stamp {
                year: 2026,
                month: 9,
                day: 3,
                hour: 0,
                minute: 5,
            }),
            "Sep 3, 2026, 12:05 AM",
            "midnight is 12 AM, and the minutes keep their leading zero"
        );
        assert_eq!(
            stamp_label(Stamp {
                year: 2026,
                month: 12,
                day: 31,
                hour: 12,
                minute: 0,
            }),
            "Dec 31, 2026, 12:00 PM",
            "noon is 12 PM"
        );
    }

    /// The caption is `app.screenshots.preview.instance-and-date`, `{instance} · {date}`
    /// (`locales/en-US/index.json:1574`).
    #[test]
    fn the_caption_puts_the_instance_and_the_date_on_one_line() {
        let shot = shot("Fabulously Optimized", "2026-10-21_14.32.07.png", 2026, 10, 21);
        assert_eq!(
            instance_and_date(shot),
            "Fabulously Optimized · Oct 21, 2026, 12:00 PM"
        );
    }

    /// At 1440 the list is 948 points wide and the 1536 breakpoint has not been reached, so
    /// the source shows two columns of 468. The identity closes exactly: two cards and one
    /// gap are the list's width, and the height comes from the same width.
    #[test]
    fn the_grid_tiles_the_list_at_1440() {
        let page = page_width_at(crate::geometry::WINDOW.0);
        assert_eq!(page, 996.0, "the shell's content column at 1440");
        let list = page - 2.0 * PAGE_PADDING;

        let grid = grid_layout(list, crate::geometry::WINDOW.0);
        assert_eq!(grid.columns, 2, "1440 is below the 1536 four-column breakpoint");
        assert!((grid.card_width - 468.0).abs() < 0.001, "(948 - 12) / 2");
        assert!(
            (grid.card_height - 263.25).abs() < 0.001,
            "468 * 9 / 16"
        );

        let used = (grid.columns as f32) * grid.card_width
            + (grid.columns as f32 - 1.0) * GRID_GAP;
        assert!(
            (used - list).abs() < 0.001,
            "columns * card + gaps must be the list width: {used} vs {list}"
        );
    }

    /// At the 1536 breakpoint the ternary takes the other branch: four 228-point cards and
    /// three gaps fill the same 948 points.
    #[test]
    fn the_grid_tiles_the_list_at_1536() {
        let page = page_width_at(crate::geometry::WINDOW.0);
        let list = page - 2.0 * PAGE_PADDING;
        let grid = grid_layout(list, WIDE_BREAKPOINT);
        assert_eq!(grid.columns, WIDE_COLUMNS);
        assert!((grid.card_width - 228.0).abs() < 0.001, "(948 - 36) / 4");
        let used = (grid.columns as f32) * grid.card_width
            + (grid.columns as f32 - 1.0) * GRID_GAP;
        assert!((used - list).abs() < 0.001, "{used} vs {list}");

        assert_eq!(column_count(640.0), NARROW_COLUMNS, "the sm edge is inclusive");
        assert_eq!(column_count(639.0), SLIM_COLUMNS);
    }

    /// A list nobody has measured takes the source's 320-point fallback rather than dividing
    /// by a zero column (`index.vue:529`).
    #[test]
    fn an_unmeasured_list_falls_back_to_the_source_s_card_width() {
        let grid = grid_layout(0.0, 1440.0);
        assert!((grid.card_width - FALLBACK_CARD_WIDTH).abs() < 0.001);
        assert!((grid.card_height - FALLBACK_CARD_WIDTH * CARD_ASPECT).abs() < 0.001);
    }

    /// The block height is the source's sum (`index.vue:544-554`): header, 10, grid, 12 — and
    /// the grid is `rows * card + (rows - 1) * gap`. Five shots over two columns is three
    /// rows: 3 * 263.25 + 2 * 12 = 813.75, and the block is 875.75.
    #[test]
    fn a_group_is_as_tall_as_its_rows_plus_its_furniture() {
        let page = page_width_at(crate::geometry::WINDOW.0);
        let grid = grid_layout(page - 2.0 * PAGE_PADDING, crate::geometry::WINDOW.0);
        let height = group_height(grid.columns, grid.card_height, 5);
        assert!((height - 875.75).abs() < 0.001, "40 + 10 + 813.75 + 12");

        let one_row = group_height(grid.columns, grid.card_height, 2);
        assert!((one_row - 325.25).abs() < 0.001, "40 + 10 + 263.25 + 12");

        // An empty group is the grid's 45-point minimum, not a zero-height grid.
        let empty = group_height(grid.columns, grid.card_height, 0);
        assert!((empty - (GROUP_HEADER_HEIGHT + GROUP_CONTENT_SPACING + GRID_MIN_HEIGHT + GROUP_SPACING)).abs() < 0.001);
    }

    /// The sample is the shape the brief asks for, and the page's own rules agree with it:
    /// nine shots, three instances, two months, two groups.
    #[test]
    fn the_sample_shows_three_instances_over_two_months() {
        assert_eq!(SHOTS.len(), 9);

        let mut instances: Vec<&str> = SHOTS.iter().map(|shot| shot.instance).collect();
        instances.sort_unstable();
        instances.dedup();
        assert_eq!(instances.len(), 3, "three instances");
        assert_eq!(instances[0], "Create: Above and Beyond");

        let mut months: Vec<(i32, u8)> = SHOTS
            .iter()
            .map(|shot| (shot.taken.year, shot.taken.month))
            .collect();
        months.sort_unstable();
        months.dedup();
        assert_eq!(months, [(2026, 9), (2026, 10)], "two dates, two months");

        let newest_first = in_sort_order(&SHOTS, Sort::Newest);
        let out = groups(&newest_first);
        assert_eq!(out.len(), 2, "the capture shows two group headings");
        assert_eq!(month_title(out[0].year, out[0].month), "October 2026");
        assert_eq!(month_title(out[1].year, out[1].month), "September 2026");
        assert_eq!(out[0].shots.len(), 5);
        assert_eq!(out[1].shots.len(), 4);
        assert_eq!(
            out[0].shots.len() + out[1].shots.len(),
            SHOTS.len(),
            "every screenshot lands in exactly one group"
        );

        // The two totals the page prints: the heading's count and the two group pills.
        let page_height: f32 = in_group_order(out, Sort::Newest)
            .iter()
            .map(|group| {
                let page = page_width_at(crate::geometry::WINDOW.0);
                let grid = grid_layout(page - 2.0 * PAGE_PADDING, crate::geometry::WINDOW.0);
                group_height(grid.columns, grid.card_height, group.shots.len())
            })
            .sum();
        assert!(page_height > 0.0, "the list has something to scroll");
    }
}
