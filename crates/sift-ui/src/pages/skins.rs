//! The skins page: the `Skin selector` heading, its toolbar, and the grid of skin tiles.
//!
//! Follows the vendored app's skins route and the components it renders:
//!
//! * `app-frontend/src/pages/Skins.vue` — the container is
//!   `skin-layout box-border grow p-4` (line 1085), the heading is
//!   `h1.m-0.text-2xl.font-bold.flex.items-center.gap-2` carrying
//!   `appMessages.skinSelectorLabel` (lines 1087-1089), the left column is the
//!   `SkinPreviewRenderer` under a sticky wrapper (lines 1086-1239), and the right column is
//!   the list with `pt-2` (lines 1241-1261). The route's own stylesheet splits the page
//!   `minmax(0, 1fr) minmax(0, 2.5fr)` with `gap: 2.5rem` (lines 1340-1348), collapsing to one
//!   column under 700 points (`@media (max-width: 700px)`, line 1345).
//! * `.../components/ui/skin/VirtualSkinSectionList.vue` — the section list. Its geometry is
//!   a block of constants: the tile aspect `31 / 40` (lines 47-48), the grid gap `12`
//!   (line 49), the first section's `pt-1` and the rest `pt-6` (line 366),
//!   `SKIN_SECTION_SPACING` 24 (line 51), the header's 28 points (line 52), the content's
//!   `pt-2` (line 375), and the card width as `(listWidth - gaps) / columnCount`
//!   (lines 130-137) with `cardHeight = cardWidth * 40 / 31` (lines 139-141). The section
//!   header is a `DropdownIcon.size-6` plus `text-xl font-semibold leading-7 text-primary`
//!   in a `gap-[6px]` row (lines 373-388). The add-skin control is the Saved-skins grid's
//!   **first tile**, a `SkinLikeTextButton` at `aspect-[31/40] rounded-[20px]`
//!   (lines 426-443).
//! * `ui/src/components/skin/SkinButton.vue` — the card itself: `aspect-ratio: 31 / 40`,
//!   `border: 1px solid var(--surface-4)`, `border-radius: 20px`,
//!   `background: var(--surface-3)` (lines 153-162), with its action row and any text laid
//!   over the bottom at `bottom-3 px-3` (lines 118-123).
//! * `ui/src/components/skin/SkinLikeTextButton.vue` — the dropzone tile: a dashed
//!   `border-surface-5` edge at `rounded-[20px]` on `bg-surface-2` (lines 58-66), a
//!   `size-8` glyph, `gap-4` to the text block, `text-base font-semibold leading-6` over
//!   `text-sm font-medium leading-5 text-primary`, and `gap-0.5` between the two lines
//!   (lines 81-95).
//! * `.../components/ui/skin/EditSkinModal.vue` — where the model lives. The variants are
//!   the literals `'CLASSIC'` and `'SLIM'` (line 41), their labels are
//!   `app.skins.modal.arm-style-wide` and `app.skins.modal.arm-style-slim`
//!   (lines 44-45, 185-192), and the modal opens on `variant = ref<SkinModel>('CLASSIC')`
//!   (line 249), which is also what it resets to (lines 351, 367). So the page's default
//!   model is the classic one, not slim.
//! * `app-frontend/src/locales/en-US/index.json` — every string below, key by key.
//!
//! **What this page does not have, because the reference does not either.** The skins page
//! has no search field, no model tabs and no `Upload` button in a toolbar: the only upload
//! affordance in the source is the add-skin tile inside the Saved-skins grid
//! (`VirtualSkinSectionList.vue:426-443`), which opens a file input (`Skins.vue:826-847`).
//! It also has no empty state at all — with nothing saved, the section simply renders the
//! add-skin tile and no prose. And its `Skin` type carries no author (`helpers/skins.ts:6`
//! names a skin, `:18` its variant; the author line this page draws is this page's own).
//! The toolbar, the two empty states and the author line are therefore this launcher's
//! additions on top of the source's structure, and every string they print is copied from
//! the locale rather than invented — see the `Strings` block for where each one comes from.
//!
//! **The grid arithmetic is the library page's, by instruction.** The skins source measures
//! its own list column and switches *fixed* column counts by window width
//! (`VirtualSkinSectionList.vue:114-128`: three by default, four at 1300, five at 1750 and
//! six at 2050). This page instead mirrors
//! `pages/library.rs`'s `grid_layout` and the expressions behind it —
//! `gap = remSize * 0.75`, `columnCount = max(1, floor((gridWidth + gap) / (minWidth + gap)))`,
//! `cardWidth = (gridWidth - gap * (n - 1)) / n`, and the
//! `windowWidth < 1280 ? 8rem : 10rem` minimum — from
//! `instance-group/index.vue:121-134`, citing that same file for the breakpoint rule as the
//! brief asks. The card's *height* comes from the skin tile's own aspect instead of the
//! instance card's `+ 3.375rem`: `cardHeight = cardWidth * 40 / 31`
//! (`VirtualSkinSectionList.vue:139-141`), because a 31/40 skin tile is the one proportion
//! the source is unambiguous about.
//!
//! The two rules disagree about how many columns appear, and that is the honest cost of
//! taking the library's rule: the reference's 2.5fr list column at 1440 is about 660 points
//! wide and lays out four ~156-point tiles, where this page lays out five 180-point ones
//! across the full content column. Two grids that line up on two pages was judged worth
//! more than one grid that matches a column the page does not draw yet — the preview column
//! the source puts in that other 1fr is not drawn at all, because there is no skin renderer
//! (see the module's closing note).
//!
//! **The grid at 1440.** The shell's content column is 996 points
//! (`PANEL_LEFT - CONTENT_LEFT - 16`, `pages/mod.rs:39-58`), the page's 24-point insets
//! leave 948, the window is not below 1280 so the 160-point minimum applies, and
//! `floor((948 + 12) / (160 + 12)) = 5` columns of 180 points; the identity
//! `2 * 24 + 5 * 180 + 4 * 12 = 996` closes exactly, and each tile is
//! `180 * 40 / 31 = 232.26` points tall. The 8rem branch is the 960-point window's, whose
//! 516-point column gives three 148-point cards.
//!
//! **The page inset is 24, not the skins page's own 16.** `Skins.vue:1085` pads the route
//! with `p-4` (1rem = 16), where the library's route container pads with `p-6` (24,
//! `pages/Index.vue:96`). This page takes the 24 for two reasons: `geometry::CONTENT_INSET`
//! is 24 — the column's measured inset — and the grid arithmetic has to resolve to the same
//! numbers as the library's to keep the two pages' tiles aligned. At 16 the same 996-point
//! page would give five 183.2-point columns, which is neither the source's grid nor the
//! library's.
//!
//! **No renderer, so no preview column and no skin faces.** The source's whole left column
//! is a 3D `SkinPreviewRenderer` (`Skins.vue:1093-1237`), and each card's face is a baked
//! skin image (`BakedSkinButton.vue:27-45`). Neither exists here: the page is two
//! columns' worth of content flattened into one, and a card is a `SUPER_RAISED_BG` plate
//! with the shirt glyph, the skin's name, its author and its model tag.
//!
//! **This file is not wired into the shell yet.** `pages/mod.rs` declares no `skins` module
//! and `Page` has no `Skins` variant, so nothing calls [`draw`]. Declaring the module and
//! adding the route are outside this file's brief.
//!
//! [`draw`] returns `false`: nothing on this page animates, and the search plate is a
//! painted stand-in for the widget another agent owns (see [`search_plate`]).

use eframe::egui::{self, Align2, Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::fonts;
use crate::icons::{self, Glyph};
use crate::images::Images;
use crate::theme::Theme;

// ---------------------------------------------------------------------------
// Page and toolbar geometry
// ---------------------------------------------------------------------------

/// The page's inset: `p-6` on the library's route container
/// (`app-frontend/src/pages/Index.vue:96`) = 1.5rem, which is
/// `geometry::CONTENT_INSET` and every other page's inset. The skins route itself pads
/// `p-4` (`Skins.vue:1085`); the module docs say why this page takes the 24 anyway.
const PAGE_PADDING: f32 = 24.0;

/// The gap between the heading, the toolbar and the section header: `gap-3`
/// (`ui/library/index.vue:267`), i.e. `--gap-md` 0.75rem (`variables.scss:197`).
const SECTION_GAP: f32 = 12.0;

/// The heading's size: `text-2xl` (`Skins.vue:1087`) = 1.5rem at the 16px root.
const HEADING_SIZE: f32 = 24.0;

/// The heading's line box: `text-2xl` resolves to a 2rem line, the same size the library's
/// heading uses (`leading-8` in `WelcomeScreen.vue:95`).
const HEADING_LINE: f32 = 32.0;

/// The height of every toolbar control. `Combobox`'s trigger and the search field are both
/// `h-9` at `md`/`standard` (`ui/src/components/base/ButtonFrame.vue:31`,
/// `InputFrame.vue:47`), and the library's toolbar uses the same 36.
const CONTROL_HEIGHT: f32 = 36.0;

/// The gap between two controls in the row: `gap-2` (`ui/library/library-toolbar/index.vue:30`),
/// i.e. `--gap-sm` 0.5rem (`variables.scss:196`).
const CONTROL_GAP: f32 = 8.0;

/// The search plate's width when the row has room for it. The reference has no search field
/// on this page, so there is no source width to copy.
// guess: re-measure once the interactive field lands and has a designed width.
const SEARCH_WIDTH: f32 = 320.0;

/// The narrowest the search plate may be squeezed to, so the tabs and the Upload button
/// win the row rather than the plate eating them.
// guess: re-measure -- chosen to keep the magnifier and a few characters legible.
const SEARCH_MIN_WIDTH: f32 = 120.0;

/// The plate's corner: `--radius-sm` (`variables.scss:202`), which is the radius the crate's
/// own search field paints (`widgets/search.rs:181`).
const SEARCH_PLATE_RADIUS: u8 = 8;

/// The magnifier's box: 12 points, centred 18 in from the plate's left edge
/// (`widgets/search.rs:185-193`), which is the same glyph the shell's search field draws.
const SEARCH_GLYPH: f32 = 12.0;

/// How far in from the plate's left edge the magnifier's centre sits
/// (`widgets/search.rs:188-192`): 18 for the glyph's centre, 6 for its radius, 10 of gap.
const SEARCH_GLYPH_INSET: f32 = 18.0;

/// The placeholder's left inset (`widgets/search.rs:195-197`).
const SEARCH_TEXT_INSET: f32 = 34.0;

/// The placeholder's size: the search field's own `fonts::body(13.0)`
/// (`widgets/search.rs:218`).
const SEARCH_TEXT_SIZE: f32 = 13.0;

/// A toolbar control's corner: `rounded-xl` on a `md` `ButtonFrame` (`ButtonFrame.vue:31`),
/// i.e. `--radius-md` 12 (`variables.scss:203`) -- the library's selects use the same.
const CONTROL_RADIUS: u8 = 12;

/// A model tab's inset inside the segmented control: 2 points, matching the library's
/// grid/list toggle (`pages/library.rs:467-468`).
const SEGMENT_INSET: f32 = 2.0;

/// A model tab's height inside the 36-point control: 36 - 2 * 2, the library toggle's own
/// 32-point segment (`pages/library.rs:467-468`).
const SEGMENT_HEIGHT: f32 = 32.0;

/// A model tab's corner: the 12-point control radius less the 2-point inset
/// (`pages/library.rs:469`).
const SEGMENT_RADIUS: u8 = 10;

/// A model tab's horizontal padding.
// guess: re-measure -- the reference has no model tabs on this page to measure.
const SEGMENT_PADDING: f32 = 12.0;

/// A model tab's label size: `text-sm` at semibold, the weight the library's selects use
/// (`pages/library.rs:413`).
const SEGMENT_TEXT_SIZE: f32 = 14.0;

/// The Upload button's horizontal padding, `px-3` on a `md` `ButtonFrame`
/// (`ButtonFrame.vue:31`).
const BUTTON_PADDING: f32 = 12.0;

/// The Upload button's glyph: `[&>svg]:size-5` on a `md` `ButtonFrame`
/// (`ButtonFrame.vue:31`), read as 18 here so the 36-point control keeps 9 points above and
/// below it.
const BUTTON_GLYPH: f32 = 18.0;

/// The gap between the Upload button's glyph and its label: `gap-1.5` (`ButtonFrame.vue:31`).
const BUTTON_GAP: f32 = 6.0;

/// The Upload button's label size, `text-sm` semibold like the reference's own buttons
/// (`pages/library.rs:413`).
const BUTTON_TEXT_SIZE: f32 = 14.0;

// ---------------------------------------------------------------------------
// Section header
// ---------------------------------------------------------------------------

/// The section header's line box: `leading-7` (`VirtualSkinSectionList.vue:388`) = 1.75rem.
const SECTION_HEADER_LINE: f32 = 28.0;

/// The section title's size: `text-xl` (`VirtualSkinSectionList.vue:388`).
const SECTION_HEADING_SIZE: f32 = 20.0;

/// The section title's leading glyph: `size-6` on the `DropdownIcon`
/// (`VirtualSkinSectionList.vue:385`).
const SECTION_HEADER_GLYPH: f32 = 24.0;

/// The gap between that glyph and the title: `gap-[6px]`
/// (`VirtualSkinSectionList.vue:374`).
const SECTION_HEADER_GAP: f32 = 6.0;

/// The space between the header and the grid: the accordion's `content-class="pt-2"`
/// (`VirtualSkinSectionList.vue:375`).
const SECTION_CONTENT_SPACING: f32 = 8.0;

// ---------------------------------------------------------------------------
// Grid geometry -- the library's expressions, the skin tile's aspect
// ---------------------------------------------------------------------------

/// The grid gap: `gap = remSize * 0.75` with `remSize` 16
/// (`instance-group/index.vue:121-122`), which is also this page's `SKIN_GRID_GAP` of 12
/// (`VirtualSkinSectionList.vue:49`). Both sources state 12.
const GRID_GAP: f32 = 12.0;

/// The minimum card width at or above the 1280 breakpoint: `remSize * 10`
/// (`instance-group/index.vue:124`, the `windowWidth < 1280 ? 8 : 10` ternary).
const MIN_CARD_AT_1440: f32 = 160.0;

/// The minimum card width below 1280: `remSize * 8` (`instance-group/index.vue:124`).
const MIN_CARD_BELOW_1280: f32 = 128.0;

/// The breakpoint in that ternary: the grid narrows its minimum at this window width
/// (`instance-group/index.vue:124`).
const NARROW_BREAKPOINT: f32 = 1280.0;

/// The skin tile's width in its own aspect ratio (`VirtualSkinSectionList.vue:47`).
const SKIN_ASPECT_WIDTH: f32 = 31.0;

/// The skin tile's height in its own aspect ratio (`VirtualSkinSectionList.vue:48`).
const SKIN_ASPECT_HEIGHT: f32 = 40.0;

/// The grid's minimum height: `min-h-[45px]` (`instance-group/index.vue:611`), repeated in
/// that file's `Math.max(45, ...)` height calculation (`:135-142`).
const GRID_MIN_HEIGHT: f32 = 45.0;

/// The card's radius: `rounded-[20px]` (`Skins.vue:428`, `SkinButton.vue:156`), which is
/// `--radius-xl` (`variables.scss:205`).
const CARD_RADIUS: u8 = 20;

/// The card's padding: the reference insets its card contents by `p-3`/`px-3`/`bottom-3`
/// (`SkinButton.vue:120`), i.e. `--gap-md` (`variables.scss:197`).
const CARD_PADDING: f32 = 12.0;

/// The gap between the card's name, author and tag rows: `gap-1` on the library card's text
/// column (`instance-card-view.vue:74`).
const CARD_TEXT_GAP: f32 = 4.0;

/// The name's line box: `leading-5` (`instance-card-view.vue:80`) = 20 points.
const NAME_LINE: f32 = 20.0;

/// The name's size: the library card's 17-point title (`pages/library.rs:612`), which is
/// one point above the source's `text-base`.
const NAME_SIZE: f32 = 17.0;

/// The author's line box: `leading-[18px]` on the library card's meta row
/// (`instance-card-view.vue:87`).
const AUTHOR_LINE: f32 = 18.0;

/// The author's size: the library card's 13-point meta text (`pages/library.rs:629`).
const AUTHOR_SIZE: f32 = 13.0;

/// The model tag's height. The reference's `TagItem` is `px-2 py-1 text-sm`
/// (`instance-group/index.vue:571-576`), which is 28 points; this tag is deliberately
/// smaller so it stays a tag on a 180-point card.
// guess: re-measure against a capture once the tiles carry artwork.
const TAG_HEIGHT: f32 = 20.0;

/// The model tag's horizontal padding (`px-2` on the same `TagItem`).
const TAG_PADDING: f32 = 8.0;

/// The model tag's text size: `text-xs` (`--font-size-xs`, `variables.scss` / `defaults.scss:24`).
const TAG_TEXT_SIZE: f32 = 12.0;

/// The model tag's corner: half of its 20-point height, i.e. the reference's pill.
const TAG_RADIUS: u8 = 10;

// ---------------------------------------------------------------------------
// The add-skin dropzone tile
// ---------------------------------------------------------------------------

/// The dropzone's glyph: `size-8` on the `PlusIcon` (`VirtualSkinSectionList.vue:439`,
/// `SkinLikeTextButton.vue:85`).
const DROPZONE_GLYPH: f32 = 32.0;

/// The space between that glyph and the text block: `gap-4`
/// (`SkinLikeTextButton.vue:82`).
const DROPZONE_GAP: f32 = 16.0;

/// `Add skin`'s line box: `leading-6` (`SkinLikeTextButton.vue:89`).
const DROPZONE_TITLE_LINE: f32 = 24.0;

/// `Add skin`'s size: `text-base` (`SkinLikeTextButton.vue:89`).
const DROPZONE_TITLE_SIZE: f32 = 16.0;

/// The space between the tile's two lines: `gap-0.5` (`SkinLikeTextButton.vue:88`).
const DROPZONE_TEXT_GAP: f32 = 2.0;

/// `Drag and drop`'s line box: `leading-5` (`SkinLikeTextButton.vue:92`).
const DROPZONE_SUBTITLE_LINE: f32 = 20.0;

/// `Drag and drop`'s size: `text-sm` (`SkinLikeTextButton.vue:92`).
const DROPZONE_SUBTITLE_SIZE: f32 = 14.0;

/// The dropzone's dash length, for the `border-dashed` edge
/// (`SkinLikeTextButton.vue:58`). CSS's default dash is proportional to the border width;
/// this is a visible dash on a 180-point tile.
// guess: re-measure against a capture of the reference's dashed tile.
const DASH_LENGTH: f32 = 6.0;

/// The gap between two dashes.
// guess: re-measure -- CSS gives no number for the default dash gap.
const DASH_GAP: f32 = 5.0;

/// How far past the corner radius the dashes start, so they do not cut the rounded corner.
// guess: re-measure.
const DASH_CORNER_INSET: f32 = 3.0;

// ---------------------------------------------------------------------------
// The no-matches block
// ---------------------------------------------------------------------------

/// The magnifier above the no-matches line: `size-6`, the section header's own glyph size.
const NO_MATCHES_GLYPH: f32 = 24.0;

/// The space between that glyph and the line: `gap-3` (`--gap-md`, `variables.scss:197`).
const NO_MATCHES_GAP: f32 = 12.0;

/// The no-matches line's box: `text-sm` with `leading-5`.
const NO_MATCHES_LINE: f32 = 20.0;

/// The no-matches line's size: `text-sm`.
const NO_MATCHES_SIZE: f32 = 14.0;

// ---------------------------------------------------------------------------
// Strings -- every one copied from the vendored locale
// ---------------------------------------------------------------------------

/// `app.skins.title` -- "Skin selector" (`locales/en-US/index.json:3284-3286`), read through
/// `appMessages.skinSelectorLabel` (`app-frontend/src/utils/app-messages.ts:4-7`) and printed
/// by the page's own `h1` (`Skins.vue:1087-1089`). The rail calls the destination "Skins"
/// (`shell.rs:51`); the page prints the source's word.
const HEADING: &str = "Skin selector";

/// `app.skins.section.saved-skins` -- "Saved skins" (`locales/en-US/index.json:3266-3268`),
/// the title of the first section in the list (`VirtualSkinSectionList.vue:56-60`).
const SECTION_TITLE: &str = "Saved skins";

/// `app.skins.section.default-skins` — "Default skins" (`locales/en-US/index.json:3248-3250`),
/// the second section of the reference's right-hand column.
const DEFAULT_SECTION_TITLE: &str = "Default skins";

/// The reference's own line for a section whose contents come from an account this launcher
/// does not have (`app.skins.default-skins.empty`). Said plainly rather than drawing eight
/// invented names under a heading that promises Mojang's.
const DEFAULT_SECTION_BODY: &str =
    "Sign in to your Minecraft account to use its default skins.";

/// `app.skins.add-button` -- "Add skin" (`locales/en-US/index.json:3131-3133`). The reference
/// prints it on the grid's add tile (`VirtualSkinSectionList.vue:61-64,441`); this page
/// prints it on that tile and, as the brief's `Upload` control, on the toolbar button.
const ADD_SKIN: &str = "Add skin";

/// `app.skins.add-button.drag-and-drop` -- "Drag and drop"
/// (`locales/en-US/index.json:3134-3136`), the add tile's subtitle
/// (`VirtualSkinSectionList.vue:65-68,442`).
const DRAG_AND_DROP: &str = "Drag and drop";

/// `app.skins.modal.arm-style-wide` -- "Wide" (`locales/en-US/index.json:3182-3184`). It is
/// the label the reference puts on its `CLASSIC` variant (`EditSkinModal.vue:41-45`), which
/// is the model this page calls [`Model::Classic`].
const MODEL_CLASSIC_LABEL: &str = "Wide";

/// `app.skins.modal.arm-style-slim` -- "Slim" (`locales/en-US/index.json:3179-3181`).
const MODEL_SLIM_LABEL: &str = "Slim";

/// `app.screenshots.no-results-description` -- "Try a different search or filter."
/// (`locales/en-US/index.json:1568-1570`). The `app.skins.*` block has no empty-state key at
/// all (the page never renders one), so the no-matches state borrows the app's own
/// noun-neutral wording for a filtered grid -- the screenshots page is its other searchable
/// grid and pairs this line with a heading whose noun ("screenshots") does not fit here,
/// which is why only the description is used.
const NO_MATCHES: &str = "Try a different search or filter.";

/// "Search skins..." -- the brief's placeholder for the toolbar's plate. It is **not** a
/// locale string: the reference has no skins search field, and its nearest placeholder keys
/// are `app.library.search.placeholder` / `app.screenshots.search`, both of which read
/// "Search" (`locales/en-US/index.json:1193-1195`, `:1577-1579`). This plate is a painted
/// stand-in for the interactive field a sibling agent owns, so the copy lives here until
/// that field brings its own.
const SEARCH_PLACEHOLDER: &str = "Search skins...";

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Which arm style a skin is drawn for. The variants are the reference's own
/// `SkinModel` literals, `'CLASSIC'` and `'SLIM'` (`EditSkinModal.vue:41`), and their labels
/// are the locale's "Wide" and "Slim" -- Minecraft's classic model is the wide-armed one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    /// `'CLASSIC'` -- the wide-armed model, and the reference's default.
    Classic,
    /// `'SLIM'` -- the narrow-armed model.
    Slim,
}

impl Model {
    /// Both models, in the order the reference's radio buttons show them
    /// (`EditSkinModal.vue:41`).
    pub const ALL: [Model; 2] = [Model::Classic, Model::Slim];

    /// The model's tab label, from the locale (see the two label strings above).
    pub fn label(self) -> &'static str {
        match self {
            Model::Classic => MODEL_CLASSIC_LABEL,
            Model::Slim => MODEL_SLIM_LABEL,
        }
    }
}

/// The reference opens the editor on `CLASSIC` (`EditSkinModal.vue:249`) and resets to it
/// (`:351`, `:367`), so the page opens on the classic tab. Slim is deliberately *not* the
/// default: the source does not say it is.
impl Default for Model {
    fn default() -> Model {
        Model::Classic
    }
}

/// The page's own state: which model tab is selected, and what has been typed into the
/// search plate.
#[derive(Debug, Default)]
pub struct State {
    /// The selected model tab.
    pub model: Model,
    /// The search query as typed, untrimmed. The trimming belongs to [`State::query`], once,
    /// at the point of use -- the same rule `widgets/search.rs:254-267` states for the
    /// shell's field.
    pub query: String,
}

impl State {
    /// The query to filter with, or `None` when the field asks for nothing. Whitespace is
    /// not a query: a space left behind after deleting a word should restore the grid, not
    /// empty it.
    pub fn query(&self) -> Option<&str> {
        let trimmed = self.query.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }
}

// ---------------------------------------------------------------------------
// Sample data
// ---------------------------------------------------------------------------

/// One skin tile's worth of data.
///
/// The reference's `Skin` has a name, a source, a texture, a variant, an optional cape and an
/// equipped flag (`helpers/skins.ts:6-20`) and **no author**; the author line this page draws
/// is this launcher's addition, so the field is filled with plausible filler rather than
/// attributed to anyone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Skin {
    /// The skin's name.
    pub name: &'static str,
    /// Who made it. Not a real attribution -- see the type's note.
    pub author: &'static str,
    /// The arm style it is drawn for.
    pub model: Model,
}

/// Eight sample skins, so a capture shows a grid rather than an empty one.
///
/// Five are wide-armed and three slim, which is what makes the default tab a full row of
/// five at 1440 and the Slim tab a shorter one. No licence, file, download or texture URL is
/// recorded here: the reference's skins are either Mojang's or the player's own upload, and
/// neither is this page's to invent.
pub const SKINS: [Skin; 8] = [
    Skin {
        name: "Overgrown",
        author: "fernlight",
        model: Model::Classic,
    },
    Skin {
        name: "Copper Patina",
        author: "ingot",
        model: Model::Classic,
    },
    Skin {
        name: "Lantern Keeper",
        author: "wickburn",
        model: Model::Classic,
    },
    Skin {
        name: "Snowbound",
        author: "flurry",
        model: Model::Classic,
    },
    Skin {
        name: "Mossy Cobble",
        author: "graveljaw",
        model: Model::Classic,
    },
    Skin {
        name: "Night Shift",
        author: "moonpetal",
        model: Model::Slim,
    },
    Skin {
        name: "Ender Drift",
        author: "voidsalt",
        model: Model::Slim,
    },
    Skin {
        name: "Aurora",
        author: "northlight",
        model: Model::Slim,
    },
];

/// Which empty state the grid shows, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EmptyState {
    /// Nothing uploaded yet and nothing searched for: the source's own state, which is the
    /// add-skin dropzone tile (`VirtualSkinSectionList.vue:426-443`) and no prose.
    NoSkins,
    /// A query that matches none of the tiles on the selected tab.
    NoMatches,
}

/// The tiles to show, in the reference's order: the selected model, then the query matched
/// case-insensitively against the name and the author.
///
/// **This is not a search engine.** It is a substring test over the eight constants above --
/// no index, no other fields, no catalogue, no network -- and it exists so the two empty
/// states are reachable from the UI instead of being dead code. A real one replaces it.
fn visible_skins(model: Model, query: Option<&str>) -> Vec<Skin> {
    SKINS
        .iter()
        .filter(|skin| skin.model == model)
        .filter(|skin| match query {
            None => true,
            Some(query) => {
                contains_ignore_case(skin.name, query) || contains_ignore_case(skin.author, query)
            }
        })
        .copied()
        .collect()
}

/// Whether `haystack` contains `needle`, ignoring ASCII case. `to_ascii_lowercase` rather
/// than `to_lowercase`: the sample data is ASCII, and the ASCII fold is the one that cannot
/// change a string's length.
fn contains_ignore_case(haystack: &str, needle: &str) -> bool {
    haystack
        .to_ascii_lowercase()
        .contains(needle.to_ascii_lowercase().as_str())
}

/// The empty state the grid is in, given what the field says and how many tiles survived
/// both filters. `None` means the grid has tiles to draw.
///
/// The source has only the first state, and it is not prose: with nothing saved, the
/// Saved-skins section still renders its add tile. The second state does not exist in the
/// reference at all -- see [`NO_MATCHES`] for why the line under it is borrowed rather than
/// invented.
fn empty_state(query: Option<&str>, matches: usize) -> Option<EmptyState> {
    if matches > 0 {
        return None;
    }
    match query {
        None => Some(EmptyState::NoSkins),
        Some(_) => Some(EmptyState::NoMatches),
    }
}

// ---------------------------------------------------------------------------
// The page
// ---------------------------------------------------------------------------

/// Draw the skins page into `rect`. Returns whether another frame is owed: nothing here
/// animates yet, so it is always `false`.
pub fn draw(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    theme: &crate::theme::Theme,
    images: &mut crate::images::Images,
    state: &mut State,
) -> bool {
    // The page owns its rect (`pages/mod.rs`'s seam); paint the page colour first so the
    // grid sits on `--color-bg` even if the caller did not clear the column.
    ui.painter().rect_filled(rect, 0, theme.BG);

    // `Images` is the project-icon cache and there is nothing to ask it for: a skin's face
    // would come from a texture this page does not have (no file, no download link), and
    // the reference bakes those previews through its own WebGL renderer
    // (`SkinPreviewRendererImpl.vue`), not through an icon URL. When a renderer lands, this
    // is the call that changes: `if let Some(texture) = images.get(ui.ctx(), &skin.texture)`.
    let _ = images;

    if rect.width() <= 2.0 * PAGE_PADDING || rect.height() <= 2.0 * PAGE_PADDING {
        return false;
    }

    let toolbar_bottom = heading_and_toolbar(ui, rect, theme, state);
    if toolbar_bottom >= rect.bottom() - PAGE_PADDING {
        return false;
    }

    let grid_top = section_header(ui, rect, toolbar_bottom, theme, SECTION_TITLE);
    // The Saved grid stops short of the page's foot by the height of the Default skins
    // heading, so that second heading has somewhere to be drawn that a reader can see.
    let defaults_strip = SECTION_HEADER_LINE + SECTION_CONTENT_SPACING + HEADING_LINE;
    let grid_rect = Rect::from_min_max(
        egui::pos2(rect.left() + PAGE_PADDING, grid_top),
        egui::pos2(
            rect.right() - PAGE_PADDING,
            (rect.bottom() - PAGE_PADDING - defaults_strip).max(grid_top + GRID_MIN_HEIGHT),
        ),
    );
    if grid_rect.width() < 1.0 || grid_rect.height() < 1.0 {
        return false;
    }

    // `gridWidth` in the source is the measured content box of the grid
    // (`useElementSize`, `instance-group/index.vue:92`), which is the page rect minus its
    // insets here. The second argument is the source's `windowWidth` (`useWindowSize()`,
    // `:93`): the `windowWidth < 1280 ? 8 : 10` ternary picks the minimum before the
    // columns are counted, so passing 948 at 1440 would take the 8rem branch the source
    // never takes there. egui 0.36 calls the window's content rectangle
    // `Context::content_rect`.
    let grid = grid_layout(rect.width(), ui.ctx().content_rect().width());
    let query = state.query();
    let visible = visible_skins(state.model, query);

    // Where the next section starts: the saved grid, its rows, and the gap below it.
    let mut below_grid = grid_rect.top();

    match empty_state(query, visible.len()) {
        // The source's own state for "nothing saved": the Saved-skins grid's first tile is
        // the add dropzone (`VirtualSkinSectionList.vue:426-443`), drawn where the first
        // card would be. There is no prose because the reference has none.
        Some(EmptyState::NoSkins) => {
            let painter = ui.painter().clone();
            let tile =
                Rect::from_min_size(grid_rect.min, egui::vec2(grid.card_width, grid.card_height));
            dropzone_tile(&painter, tile, theme);
            below_grid = tile.bottom() + GRID_GAP;
        }
        // A filtered grid with no survivors: this state is this launcher's own, since the
        // reference has no search on this page.
        Some(EmptyState::NoMatches) => {
            let painter = ui.painter().clone();
            no_matches(&painter, grid_rect, theme);
            below_grid = grid_rect.top() + GRID_MIN_HEIGHT;
        }
        None => {
            let columns = grid.columns.max(1);
            // The reference's Saved-skins grid holds the add dropzone as its first tile
            // (`VirtualSkinSectionList.vue:426-443`), so the reader's own skins start at the
            // second slot whether the list is empty or not.
            let slots = visible.len() + 1;
            let rows = slots.div_ceil(columns);
            let content_height =
                (rows as f32 * (grid.card_height + GRID_GAP) - GRID_GAP).max(GRID_MIN_HEIGHT);
            // The grid scrolls inside `grid_rect`; the section below starts at the strip
            // reserved for it, not at whatever the contents measured.
            below_grid = grid_rect.bottom() + SECTION_GAP;

            // The source scrolls the whole page; this page owns only the column below its
            // section header, so the scroll area starts at the grid.
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(grid_rect));
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(&mut child, |ui| {
                    // Reserve the full grid height so the scroll area knows what is below
                    // the fold, then paint the cards at the source's absolute positions
                    // (`instance-group/index.vue:164-171`).
                    let (area, _) = ui.allocate_exact_size(
                        egui::vec2(grid_rect.width(), content_height),
                        Sense::hover(),
                    );
                    let painter = ui.painter();
                    for slot in 0..slots {
                        let column = slot % columns;
                        let row = slot / columns;
                        let card = Rect::from_min_size(
                            area.min
                                + egui::vec2(
                                    column as f32 * (grid.card_width + GRID_GAP),
                                    row as f32 * (grid.card_height + GRID_GAP),
                                ),
                            egui::vec2(grid.card_width, grid.card_height),
                        );
                        // Slot zero is the dropzone; the reader's own skins start at the
                        // second slot (`VirtualSkinSectionList.vue:426-443`).
                        match slot.checked_sub(1).and_then(|index| visible.get(index)) {
                            Some(skin) => draw_card(painter, card, theme, skin),
                            None => dropzone_tile(painter, card, theme),
                        }
                    }
                });
        }
    }

    let defaults_top = section_header(ui, rect, below_grid, theme, DEFAULT_SECTION_TITLE);
    if defaults_top + HEADING_LINE <= rect.bottom() - PAGE_PADDING {
        ui.painter().text(
            egui::pos2(rect.left() + PAGE_PADDING, defaults_top),
            Align2::LEFT_TOP,
            DEFAULT_SECTION_BODY,
            fonts::body(13.0),
            theme.SECONDARY,
        );
    }

    false
}

// ---------------------------------------------------------------------------
// Heading and toolbar
// ---------------------------------------------------------------------------

/// Draw the heading and the toolbar row. Returns the y where the section header begins.
///
/// The heading is `h1.m-0.text-2xl.font-bold` (`Skins.vue:1087`) -- note `font-bold`, 700,
/// where the library's heading is `font-semibold` (`library/index.vue:268`), which is why
/// this one is [`fonts::title`] and that one is [`fonts::semibold`].
fn heading_and_toolbar(ui: &mut Ui, rect: Rect, theme: &Theme, state: &mut State) -> f32 {
    // An owned clone, not a borrow: `model_tabs` needs `&mut Ui` to take the clicks, and a
    // `&Painter` borrowed from `ui` would still be alive while it does.
    let painter = ui.painter().clone();

    let heading_at = egui::pos2(rect.left() + PAGE_PADDING, rect.top() + PAGE_PADDING);
    painter.text(
        heading_at,
        Align2::LEFT_TOP,
        HEADING,
        fonts::title(HEADING_SIZE),
        theme.CONTRAST,
    );

    let row = Rect::from_min_size(
        egui::pos2(
            rect.left() + PAGE_PADDING,
            heading_at.y + HEADING_LINE + SECTION_GAP,
        ),
        egui::vec2(rect.width() - 2.0 * PAGE_PADDING, CONTROL_HEIGHT),
    );

    // The two measured controls first, then whatever is left over for the search plate. The
    // reference has no search field here, so there is no width to copy; a flat 320 would
    // overlap the tabs on a 960-point window.
    let tabs_width = model_tabs_width(&painter);
    let upload_width = upload_button_width(&painter);
    let search_width = SEARCH_WIDTH
        .min((row.width() - tabs_width - upload_width - 2.0 * CONTROL_GAP).max(SEARCH_MIN_WIDTH));

    let search = Rect::from_min_size(row.min, egui::vec2(search_width, CONTROL_HEIGHT));
    search_plate(&painter, search, theme);
    model_tabs(
        ui,
        &painter,
        egui::pos2(search.right() + CONTROL_GAP, row.top()),
        theme,
        state,
    );
    upload_button(&painter, row.right(), row.top(), theme);

    row.bottom() + SECTION_GAP
}

/// The search plate: the raised fill, the magnifier, and the placeholder in
/// `--color-secondary`.
///
/// **Only a plate.** The interactive field belongs to `crate::widgets::search::Search`,
/// which draws the same plate, glyph, placeholder and caret and takes the keyboard
/// (`widgets/search.rs:74-252`). That widget is a sibling's work; this function paints the
/// control's resting look so the row reads right in a capture, and the day the field lands
/// its `ui` call replaces this one at the same rect.
fn search_plate(painter: &egui::Painter, rect: Rect, theme: &Theme) {
    painter.rect_filled(rect, SEARCH_PLATE_RADIUS, theme.RAISED_BG);
    icons::draw(
        painter,
        Glyph::Search,
        Rect::from_center_size(
            egui::pos2(rect.left() + SEARCH_GLYPH_INSET, rect.center().y),
            Vec2::splat(SEARCH_GLYPH),
        ),
        theme.SECONDARY,
    );

    // Clipped rather than wrapped: this is a one-line control, and a hint that outgrows it
    // should disappear at the plate's edge rather than spill over the tabs beside it.
    let text_rect = Rect::from_min_max(
        egui::pos2(rect.left() + SEARCH_TEXT_INSET, rect.top()),
        egui::pos2(rect.right() - SEARCH_GLYPH_INSET, rect.bottom()),
    );
    let clipped = painter.with_clip_rect(text_rect);
    let galley = clipped.layout_no_wrap(
        SEARCH_PLACEHOLDER.to_owned(),
        fonts::body(SEARCH_TEXT_SIZE),
        theme.SECONDARY,
    );
    clipped.galley(
        egui::pos2(text_rect.left(), rect.center().y - galley.size().y / 2.0),
        galley,
        theme.SECONDARY,
    );
}

/// The two-segment model control. The selected segment is the brand wash a selected tab
/// uses (`BUTTON_BG_SELECTED`, `--color-brand-highlight`), and a press anywhere on a segment
/// selects that model.
fn model_tabs(
    ui: &mut Ui,
    painter: &egui::Painter,
    min: Pos2,
    theme: &Theme,
    state: &mut State,
) -> Rect {
    let control = Rect::from_min_size(min, egui::vec2(model_tabs_width(painter), CONTROL_HEIGHT));
    painter.rect_filled(control, CONTROL_RADIUS, theme.BUTTON_BG);

    let mut x = control.left() + SEGMENT_INSET;
    for model in Model::ALL {
        let width = tab_width(painter, model);
        let segment = Rect::from_min_size(
            egui::pos2(x, control.top() + SEGMENT_INSET),
            egui::vec2(width, SEGMENT_HEIGHT),
        );
        let selected = state.model == model;
        if selected {
            painter.rect_filled(segment, SEGMENT_RADIUS, theme.BUTTON_BG_SELECTED);
        }
        painter.text(
            segment.center(),
            Align2::CENTER_CENTER,
            model.label(),
            fonts::semibold(SEGMENT_TEXT_SIZE),
            if selected {
                theme.BUTTON_TEXT_SELECTED
            } else {
                theme.BASE
            },
        );

        // The whole segment is the hit target, so a press anywhere on it switches models.
        let response = ui.interact(
            segment,
            egui::Id::new(("skins-model-tab", model.label())),
            Sense::click(),
        );
        if response.clicked() {
            state.model = model;
        }

        x = segment.right();
    }

    control
}

/// The width the model control needs: two segments plus the outer inset either side.
fn model_tabs_width(painter: &egui::Painter) -> f32 {
    let mut width = 2.0 * SEGMENT_INSET;
    for model in Model::ALL {
        width += tab_width(painter, model);
    }
    width
}

/// One model tab's width: its label plus the segment's padding either side.
fn tab_width(painter: &egui::Painter, model: Model) -> f32 {
    text_width(painter, model.label(), fonts::semibold(SEGMENT_TEXT_SIZE)) + 2.0 * SEGMENT_PADDING
}

/// The Upload button, drawn at rest and right-aligned on the row: the reference's brand
/// button (`type="colored" color="brand"`, the same fill the welcome screen's action uses,
/// `WelcomeScreen.vue:106`).
///
/// **Painted, not wired.** Opening a file picker, decoding a PNG and normalising its texture
/// is the reference's `processSkinFileBuffer` (`Skins.vue:973-988`) and is not this file's;
/// the button exists so the toolbar is complete and the seam is visible.
fn upload_button(painter: &egui::Painter, right: f32, top: f32, theme: &Theme) -> Rect {
    let rect = Rect::from_min_size(
        egui::pos2(right - upload_button_width(painter), top),
        egui::vec2(upload_button_width(painter), CONTROL_HEIGHT),
    );
    painter.rect_filled(rect, CONTROL_RADIUS, theme.BRAND);
    icons::draw(
        painter,
        Glyph::Upload,
        Rect::from_center_size(
            egui::pos2(
                rect.left() + BUTTON_PADDING + BUTTON_GLYPH / 2.0,
                rect.center().y,
            ),
            Vec2::splat(BUTTON_GLYPH),
        ),
        theme.ACCENT_CONTRAST,
    );
    painter.text(
        egui::pos2(
            rect.left() + BUTTON_PADDING + BUTTON_GLYPH + BUTTON_GAP,
            rect.center().y,
        ),
        Align2::LEFT_CENTER,
        ADD_SKIN,
        fonts::semibold(BUTTON_TEXT_SIZE),
        theme.ACCENT_CONTRAST,
    );
    rect
}

/// The width the Upload button needs: its padding, glyph, gap and label.
fn upload_button_width(painter: &egui::Painter) -> f32 {
    2.0 * BUTTON_PADDING
        + BUTTON_GLYPH
        + BUTTON_GAP
        + text_width(painter, ADD_SKIN, fonts::semibold(BUTTON_TEXT_SIZE))
}

/// One string's advance, so a control can be sized around its own label. The ink does not
/// affect a galley's width, which is why a placeholder colour is enough.
fn text_width(painter: &egui::Painter, text: &str, font: egui::FontId) -> f32 {
    painter
        .layout_no_wrap(text.to_owned(), font, Color32::WHITE)
        .size()
        .x
}

// ---------------------------------------------------------------------------
// The section header
// ---------------------------------------------------------------------------

/// The Saved-skins header: the accordion's chevron and its title. The list is always open on
/// this page, so the glyph is drawn in its open orientation (`rotate-180` on the
/// `DropdownIcon`, `VirtualSkinSectionList.vue:384-387`). Returns the y where the grid
/// begins.
fn section_header(ui: &Ui, rect: Rect, top: f32, theme: &Theme, title: &str) -> f32 {
    let painter = ui.painter();
    let glyph = Rect::from_min_size(
        egui::pos2(
            rect.left() + PAGE_PADDING,
            top + (SECTION_HEADER_LINE - SECTION_HEADER_GLYPH) / 2.0,
        ),
        Vec2::splat(SECTION_HEADER_GLYPH),
    );
    icons::draw(painter, Glyph::ChevronDown, glyph, theme.CONTRAST);
    painter.text(
        egui::pos2(
            glyph.right() + SECTION_HEADER_GAP,
            top + SECTION_HEADER_LINE / 2.0,
        ),
        Align2::LEFT_CENTER,
        title,
        fonts::semibold(SECTION_HEADING_SIZE),
        theme.CONTRAST,
    );
    top + SECTION_HEADER_LINE + SECTION_CONTENT_SPACING
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

/// `remSize * (windowWidth < 1280 ? 8 : 10)` at `remSize` 16
/// (`instance-group/index.vue:124`).
fn min_card_width(window_width: f32) -> f32 {
    if window_width < NARROW_BREAKPOINT {
        MIN_CARD_BELOW_1280
    } else {
        MIN_CARD_AT_1440
    }
}

/// The source's `cardWidth = (gridWidth - gap * (n - 1)) / n`
/// (`instance-group/index.vue:127-129`), with the height taken from the skin tile's own
/// aspect rather than the instance card's `+ 3.375rem`.
fn grid_layout(page_width: f32, window_width: f32) -> Grid {
    let grid_width = (page_width - 2.0 * PAGE_PADDING).max(0.0);
    let columns = column_count(grid_width, min_card_width(window_width));
    let card_width = (grid_width - GRID_GAP * (columns.saturating_sub(1)) as f32) / columns as f32;
    Grid {
        columns,
        card_width,
        card_height: card_height(card_width),
    }
}

/// The skin tile's height at a given width: `cardWidth * 40 / 31`
/// (`VirtualSkinSectionList.vue:139-141`).
fn card_height(card_width: f32) -> f32 {
    card_width * SKIN_ASPECT_HEIGHT / SKIN_ASPECT_WIDTH
}

// ---------------------------------------------------------------------------
// The card
// ---------------------------------------------------------------------------

/// One skin card: a plain plate with the skin's name, its author and its model.
///
/// Geometry from `SkinButton.vue:153-162` -- the 31/40 box, the 20-point radius and the
/// 1-point edge -- with the contents inset by `CARD_PADDING`. The name, author and tag stack
/// up from the bottom the way the reference's action row sits at `bottom-3`
/// (`SkinButton.vue:118-123`).
fn draw_card(painter: &egui::Painter, rect: Rect, theme: &Theme, skin: &Skin) {
    painter.rect_filled(rect, CARD_RADIUS, theme.SUPER_RAISED_BG);
    painter.rect_stroke(
        rect,
        CARD_RADIUS,
        Stroke::new(crate::theme::STROKE_WIDTH, theme.SURFACE_5),
        egui::StrokeKind::Inside,
    );

    let text_left = rect.left() + CARD_PADDING;
    let content_width = (rect.width() - 2.0 * CARD_PADDING).max(1.0);

    let tag = Rect::from_min_size(
        egui::pos2(text_left, rect.bottom() - CARD_PADDING - TAG_HEIGHT),
        egui::vec2(
            content_width.min(text_width_of(
                painter,
                skin.model.label(),
                fonts::label(TAG_TEXT_SIZE),
                TAG_PADDING,
            )),
            TAG_HEIGHT,
        ),
    );
    let author_top = tag.top() - CARD_TEXT_GAP - AUTHOR_LINE;
    let name_top = author_top - CARD_TEXT_GAP - NAME_LINE;

    // The skin's own face would be here (`BakedSkinButton.vue:27-45` bakes a preview and
    // hands it to `SkinButton`'s `<img>`). There is no renderer in this launcher, so the
    // shirt glyph stands in on the plate, sized to whatever room the text leaves.
    let face = Rect::from_min_max(
        egui::pos2(rect.left() + CARD_PADDING, rect.top() + CARD_PADDING),
        egui::pos2(rect.right() - CARD_PADDING, name_top - CARD_TEXT_GAP),
    );
    if face.width() > 1.0 && face.height() > 1.0 {
        let side = (face.height() * 0.5).clamp(16.0, 64.0);
        icons::draw(
            painter,
            Glyph::Shirt,
            Rect::from_center_size(face.center(), Vec2::splat(side)),
            theme.SECONDARY,
        );
    }

    clipped_line(
        painter,
        Rect::from_min_size(
            egui::pos2(text_left, name_top),
            egui::vec2(content_width, NAME_LINE),
        ),
        skin.name,
        fonts::semibold(NAME_SIZE),
        theme.CONTRAST,
    );
    clipped_line(
        painter,
        Rect::from_min_size(
            egui::pos2(text_left, author_top),
            egui::vec2(content_width, AUTHOR_LINE),
        ),
        skin.author,
        fonts::body(AUTHOR_SIZE),
        theme.SECONDARY,
    );

    // The model tag: the app's pill, filled with the page behind the card so it reads as a
    // cut-out rather than another plate.
    painter.rect_filled(tag, TAG_RADIUS, theme.BG);
    painter.rect_stroke(
        tag,
        TAG_RADIUS,
        Stroke::new(crate::theme::STROKE_WIDTH, theme.SURFACE_5),
        egui::StrokeKind::Inside,
    );
    painter.text(
        tag.center(),
        Align2::CENTER_CENTER,
        skin.model.label(),
        fonts::label(TAG_TEXT_SIZE),
        theme.SECONDARY,
    );
}

/// A pill's width around its label: the text plus the padding either side.
fn text_width_of(painter: &egui::Painter, text: &str, font: egui::FontId, padding: f32) -> f32 {
    text_width(painter, text, font) + 2.0 * padding
}

/// Draw one line of text, clipped to its own box. A long name ends at the card's edge rather
/// than wrapping into the author line.
fn clipped_line(painter: &egui::Painter, rect: Rect, text: &str, font: egui::FontId, ink: Color32) {
    let galley = painter.layout(text.to_owned(), font, ink, rect.width());
    painter.with_clip_rect(rect).galley(rect.min, galley, ink);
}

// ---------------------------------------------------------------------------
// The empty states
// ---------------------------------------------------------------------------

/// The source's add-skin tile: the `border-dashed border-surface-5 bg-surface-2` plate
/// (`SkinLikeTextButton.vue:58-66`) with the plus glyph, `Add skin` and `Drag and drop`
/// (`:81-95`), at the first cell of the grid.
fn dropzone_tile(painter: &egui::Painter, rect: Rect, theme: &Theme) {
    painter.rect_filled(rect, CARD_RADIUS, theme.SURFACE_2);
    dashed_border(
        painter,
        rect,
        Stroke::new(crate::theme::STROKE_WIDTH, theme.SURFACE_5),
    );

    let stack = DROPZONE_GLYPH
        + DROPZONE_GAP
        + DROPZONE_TITLE_LINE
        + DROPZONE_TEXT_GAP
        + DROPZONE_SUBTITLE_LINE;
    if rect.height() < stack {
        return;
    }
    let mut y = rect.center().y - stack / 2.0;
    icons::draw(
        painter,
        Glyph::Plus,
        Rect::from_center_size(
            egui::pos2(rect.center().x, y + DROPZONE_GLYPH / 2.0),
            Vec2::splat(DROPZONE_GLYPH),
        ),
        theme.CONTRAST,
    );
    y += DROPZONE_GLYPH + DROPZONE_GAP;
    painter.text(
        egui::pos2(rect.center().x, y + DROPZONE_TITLE_LINE / 2.0),
        Align2::CENTER_CENTER,
        ADD_SKIN,
        fonts::semibold(DROPZONE_TITLE_SIZE),
        theme.CONTRAST,
    );
    y += DROPZONE_TITLE_LINE + DROPZONE_TEXT_GAP;
    painter.text(
        egui::pos2(rect.center().x, y + DROPZONE_SUBTITLE_LINE / 2.0),
        Align2::CENTER_CENTER,
        DRAG_AND_DROP,
        fonts::body(DROPZONE_SUBTITLE_SIZE),
        theme.BASE,
    );
}

/// A `border-dashed` edge, four dashes at a time. egui has no dashed stroke on a rounded
/// rectangle, so the dashes are drawn as short segments inset past each corner; the rounded
/// corners themselves are left to the plate's fill.
fn dashed_border(painter: &egui::Painter, rect: Rect, stroke: Stroke) {
    let inset = CARD_RADIUS as f32 + DASH_CORNER_INSET;
    let top = rect.top();
    let bottom = rect.bottom();
    let left = rect.left();
    let right = rect.right();
    dash(
        painter,
        egui::pos2(left + inset, top),
        egui::pos2(right - inset, top),
        stroke,
    );
    dash(
        painter,
        egui::pos2(left + inset, bottom),
        egui::pos2(right - inset, bottom),
        stroke,
    );
    dash(
        painter,
        egui::pos2(left, top + inset),
        egui::pos2(left, bottom - inset),
        stroke,
    );
    dash(
        painter,
        egui::pos2(right, top + inset),
        egui::pos2(right, bottom - inset),
        stroke,
    );
}

/// Draw dashes along one straight edge.
fn dash(painter: &egui::Painter, from: Pos2, to: Pos2, stroke: Stroke) {
    let span = (to - from).length();
    let step = DASH_LENGTH + DASH_GAP;
    if span <= 0.0 || step <= 0.0 {
        return;
    }
    let direction = (to - from) / span;
    let mut along = 0.0;
    while along < span {
        let start = from + direction * along;
        let end = from + direction * (along + DASH_LENGTH).min(span);
        painter.line_segment([start, end], stroke);
        along += step;
    }
}

/// A filtered grid with nothing in it. The line is the app's own wording (see
/// [`NO_MATCHES`]); the magnifier above it is the glyph the search plate already uses, so
/// the state points back at the control that caused it.
fn no_matches(painter: &egui::Painter, rect: Rect, theme: &Theme) {
    let stack = NO_MATCHES_GLYPH + NO_MATCHES_GAP + NO_MATCHES_LINE;
    let mut y = rect.center().y - stack / 2.0;
    icons::draw(
        painter,
        Glyph::Search,
        Rect::from_center_size(
            egui::pos2(rect.center().x, y + NO_MATCHES_GLYPH / 2.0),
            Vec2::splat(NO_MATCHES_GLYPH),
        ),
        theme.SECONDARY,
    );
    y += NO_MATCHES_GLYPH + NO_MATCHES_GAP;
    painter.text(
        egui::pos2(rect.center().x, y + NO_MATCHES_LINE / 2.0),
        Align2::CENTER_CENTER,
        NO_MATCHES,
        fonts::body(NO_MATCHES_SIZE),
        theme.SECONDARY,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shell's content column at a window width: the rail, the panel and the 24-point
    /// inset, with the last 16 points the shell keeps for itself (`pages/mod.rs:39-58`).
    fn page_width_at(window_width: f32) -> f32 {
        window_width
            - crate::geometry::RAIL
            - crate::geometry::PANEL
            - crate::geometry::CONTENT_INSET
            - 16.0
    }

    /// At 1440 the source's `windowWidth < 1280` test is false, so `minWidth` is 10rem =
    /// 160, not 8rem. `floor((948 + 12) / (160 + 12)) = 5` columns of 180 points, and the
    /// four gaps and the two 24-point insets leave exactly the page's 996 points.
    #[test]
    fn the_grid_tiles_the_page_at_1440() {
        let page = page_width_at(crate::geometry::WINDOW.0);
        assert_eq!(page, 996.0, "the shell's content column at 1440");
        assert_eq!(min_card_width(crate::geometry::WINDOW.0), 160.0);

        let grid = grid_layout(page, crate::geometry::WINDOW.0);
        assert_eq!(grid.columns, 5, "floor((948 + 12) / (160 + 12))");
        assert!(
            (grid.card_width - 180.0).abs() < 0.001,
            "948 / 5 with four 12-point gaps"
        );

        let used = 2.0 * PAGE_PADDING
            + grid.columns as f32 * grid.card_width
            + (grid.columns as f32 - 1.0) * GRID_GAP;
        assert!(
            (used - page).abs() < 0.001,
            "padding + columns * card + gaps must be the page width: {used} vs {page}"
        );

        // 180 * 40 / 31: the skin tile's own aspect decides the height, not the instance
        // card's `+ 3.375rem`.
        assert!(
            (grid.card_height - 232.258).abs() < 0.01,
            "180 * 40 / 31, got {}",
            grid.card_height
        );
        assert_eq!(grid.card_height, card_height(grid.card_width));
    }

    /// At 960 the ternary takes the other branch: `minWidth` is 8rem = 128, so
    /// `floor((468 + 12) / (128 + 12)) = 3` and the cards are 148 wide. The identity still
    /// closes exactly.
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
        assert!(
            (grid.card_height - 190.968).abs() < 0.01,
            "148 * 40 / 31, got {}",
            grid.card_height
        );
    }

    /// The ternary is `<`, so the wider minimum starts exactly at 1280 and 1279 keeps the
    /// narrower one. A page that switched at the wrong side would show a column count the
    /// reference never shows.
    #[test]
    fn the_breakpoint_is_the_sources_less_than() {
        assert_eq!(min_card_width(1279.0), MIN_CARD_BELOW_1280);
        assert_eq!(min_card_width(1280.0), MIN_CARD_AT_1440);
    }

    /// The count never drops below one even for a page narrower than a card, and it is a
    /// pure function of the two widths the source's expression names.
    #[test]
    fn a_page_narrower_than_one_card_still_gets_one_column() {
        let grid = grid_layout(2.0 * PAGE_PADDING + 40.0, 960.0);
        assert_eq!(grid.columns, 1);
        assert!((grid.card_width - 40.0).abs() < 0.001);
    }

    /// The tile's proportion is the source's `31 / 40`, exactly: a 310-point tile is 400
    /// tall, and the height always comes from the width the row was laid out with.
    #[test]
    fn the_skin_tile_keeps_the_sources_aspect() {
        assert_eq!(card_height(310.0), 400.0, "310 * 40 / 31");
        assert!(card_height(1.0) > 0.0);
    }

    /// The reference opens its skin editor on `CLASSIC` (`EditSkinModal.vue:249`), so the
    /// page opens on the classic tab -- not slim.
    #[test]
    fn the_page_opens_on_the_classic_model() {
        let state = State::default();
        assert_eq!(state.model, Model::Classic);
        assert_eq!(Model::default(), Model::Classic);
        assert_eq!(state.query(), None, "an untouched field asks for nothing");
    }

    /// The tabs print the locale's own words, in the reference's own order, and each is a
    /// distinct non-empty label: a tab with no text would be an invisible target.
    #[test]
    fn the_model_tabs_are_labelled_and_ordered() {
        assert_eq!(Model::ALL.map(Model::label), ["Wide", "Slim"]);
        assert_eq!(Model::Classic.label(), MODEL_CLASSIC_LABEL);
        assert_eq!(Model::Slim.label(), MODEL_SLIM_LABEL);
    }

    /// The empty state is chosen by the query, not by the count alone: an empty grid with no
    /// query is the source's add tile, an empty grid with a query is the no-matches line, and
    /// a grid with tiles is neither.
    #[test]
    fn the_empty_state_follows_the_query() {
        assert_eq!(empty_state(None, 0), Some(EmptyState::NoSkins));
        assert_eq!(empty_state(Some("zzz"), 0), Some(EmptyState::NoMatches));
        assert_eq!(empty_state(None, 1), None);
        assert_eq!(empty_state(Some("steve"), 3), None);
    }

    /// The sample grid is eight named, attributed skins -- enough for a capture -- and every
    /// one of them belongs to a model the tabs can show.
    #[test]
    fn the_sample_grid_is_eight_named_skins() {
        assert_eq!(SKINS.len(), 8);
        for skin in SKINS {
            assert!(!skin.name.is_empty(), "every tile needs a name");
            assert!(!skin.author.is_empty(), "every tile needs an author");
            assert!(
                Model::ALL.contains(&skin.model),
                "{} has a model no tab shows",
                skin.name
            );
        }
    }

    /// The model filter splits the sample grid: five wide-armed and three slim, which is why
    /// the default tab fills one 1440 row and the other does not.
    #[test]
    fn the_model_tab_filters_the_sample_grid() {
        assert_eq!(visible_skins(Model::Classic, None).len(), 5);
        assert_eq!(visible_skins(Model::Slim, None).len(), 3);
        assert!(
            visible_skins(Model::Slim, None)
                .iter()
                .all(|skin| skin.model == Model::Slim),
            "the slim tab shows no wide-armed skin"
        );
    }

    /// The query matches the name and the author, ignoring case, and an unmatched query
    /// leaves the grid empty -- which is what puts the no-matches state on screen.
    #[test]
    fn the_query_matches_names_and_authors() {
        assert_eq!(visible_skins(Model::Classic, Some("over")).len(), 1);
        assert_eq!(visible_skins(Model::Classic, Some("OVER")).len(), 1);
        assert_eq!(
            visible_skins(Model::Classic, Some("GRAVEL")).len(),
            1,
            "the author is searchable too"
        );
        assert_eq!(visible_skins(Model::Classic, Some("zzz")).len(), 0);
        assert_eq!(visible_skins(Model::Slim, Some("aurora")).len(), 1);
        assert_eq!(
            visible_skins(Model::Slim, Some("aurora")),
            vec![SKINS[7]],
            "the match is the skin, not a copy of its fields"
        );
    }

    /// A whitespace-only query is no query, so the grid comes back rather than emptying --
    /// the same rule the shell's search field states (`widgets/search.rs:254-267`).
    #[test]
    fn a_whitespace_query_is_no_query() {
        let blank = State {
            query: "   ".to_owned(),
            ..State::default()
        };
        assert_eq!(blank.query(), None);

        let padded = State {
            query: "  mossy ".to_owned(),
            ..State::default()
        };
        assert_eq!(padded.query(), Some("mossy"));
        assert_eq!(
            padded.query, "  mossy ",
            "the stored text keeps what was typed"
        );
    }
}
