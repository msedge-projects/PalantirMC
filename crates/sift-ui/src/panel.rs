//! The right panel: `Getting started`, the `News` card and the hosting banner.
//!
//! **One file, because one panel draws on every page.** The reference's shell owns this
//! column and every page it swaps underneath keeps it (`App.vue:2486` draws the sidebar
//! beside the router view), so a copy per page would be the same arithmetic five times
//! over, and the first capture that disagreed with it would be fixed in one page and left
//! wrong in the other four. The shell draws its furniture here and the pages draw only the
//! content column, which is the split `pages/mod.rs` states from the other side.
//!
//! **The layout is arithmetic over constants, not a column that measures itself.** Every
//! number below carries where it came from: the reference's own measurement (the panel is
//! 340 wide with a 24-point inset, a row is 36 tall), a value read out of the vendored
//! stylesheet at the `file:line` beside it, or a guess, marked `// guess: re-measure` so
//! the parity loop knows which numbers a capture is meant to correct. Nothing waits for a
//! font to decide where the next row goes: the reference's card is a fixed stack, and a
//! card that grew with its copy would move the date somewhere the capture does not.
//!
//! **Nothing here is clickable yet, and the hover says so.** The steps and the upgrade row
//! have actions that belong to the shell's state -- creating an instance, signing in,
//! opening a URL -- and [`draw`] reports nothing back to spend them on. Each takes a hover
//! and no press: a control that brightens under the pointer and does nothing when clicked
//! is worse than one that plainly has no press state yet.

use eframe::egui::{Align2, Rect, Sense, Stroke, Ui, Vec2};

use crate::fonts;
use crate::icons::{self, Glyph};
use crate::theme::Theme;

// ---------------------------------------------------------------------------
// The panel's box
// ---------------------------------------------------------------------------

/// The inset between the panel's edges and its content column, on both sides.
///
/// 24 points from the reference's measurement: the panel is 340 wide
/// (`geometry::PANEL`) and its content is 292, which is why every row and card below
/// spans [`column`] and none of them spells out a width of its own.
const PAD: f32 = 24.0;

/// The first heading's inset from the panel's top edge.
const TOP_PAD: f32 = 20.0; // guess: re-measure

/// The gap between one section's last row and the next section's heading.
///
/// 20 points, from the reference's measurement. It is measured from the *bottom* of the
/// last row, not from its top: the reference rules between sections and not between rows,
/// so the space belongs to the heading above it rather than to the row.
const SECTION_GAP: f32 = 20.0;

/// The rule down the panel's left edge, and every other hairline here: the sheet states
/// it literally as `1px` (`assets/styles/classes.scss:753`), which [`Theme`]'s generated
/// ladder keeps as [`crate::theme::STROKE_WIDTH`].
const RULE: f32 = crate::theme::STROKE_WIDTH;

// ---------------------------------------------------------------------------
// Section headings
// ---------------------------------------------------------------------------

/// A section heading's size: `Getting started` and `News` are both semibold 16.
const HEADING_SIZE: f32 = 16.0;

/// A heading's line box. The checklist's title is `font-semibold leading-6`
/// (`ui/onboarding-checklist/index.vue:99`), and `leading-6` is 24 points.
const HEADING_LINE: f32 = 24.0;

/// The gap between a heading and the rows under it. The checklist's rows sit inside the
/// accordion's content, which is padded `p-3` (`ui/onboarding-checklist/index.vue:94`).
const HEADING_TO_BODY: f32 = 12.0;

/// The disclosure chevron on the `Getting started` heading: `DropdownIcon` at `size-5`
/// (`ui/components/base/Accordion.vue:36`), 20 points.
const CHEVRON: f32 = 20.0;

// ---------------------------------------------------------------------------
// `Getting started` rows
// ---------------------------------------------------------------------------

/// A checklist row's height: 36 points, from the reference's measurement. The sheet's step
/// button is `h-10` (40); the capture is what this follows, and `docs/PARITY-LOG.md`'s
/// OBS-4 is the entry that left the reference's row rhythm for the next capture to settle.
const ROW_H: f32 = 36.0;

/// The gap between two checklist rows: 8 points, from the reference's measurement. With
/// [`ROW_H`] this is the whole of OBS-4's open question.
const ROW_GAP: f32 = 8.0;

/// The distance between two rows' top edges. Derived, so a correction to either half of
/// the rhythm moves every row below it and nothing above it.
const ROW_PITCH: f32 = ROW_H + ROW_GAP;

/// A row's inset before its ring: the sheet's step button is `px-4`
/// (`ui/onboarding-checklist/index.vue:105`), 16 points.
const ROW_PAD: f32 = 16.0;

/// The ring glyph at a row's left: 16 points, from the reference's measurement.
const RING: f32 = 16.0;

/// The gap between the ring and its label: the sheet's `gap-2`
/// (`ui/onboarding-checklist/index.vue:105`), 8 points.
const RING_GAP: f32 = 8.0;

/// A row's label: 13-point body text, from the reference's measurement.
const STEP_SIZE: f32 = 13.0;

// ---------------------------------------------------------------------------
// The news card
// ---------------------------------------------------------------------------

/// The art plate's height: 128 points, from the reference's measurement.
const ART_H: f32 = 128.0; // guess: re-measure

/// The gap between the `News` heading and the card: the section's heading is `mb-4`
/// (`App.vue:2531`), 16 points.
const NEWS_HEADING_TO_CARD: f32 = 16.0;

/// The gap between the card's art and its title: the card's own stack is `gap-4`
/// (`ui/components/content/NewsArticleCard.vue:25`), 16 points.
const ART_TO_TITLE: f32 = 16.0;

/// The title's size: 14 semibold, from the reference's measurement.
const TITLE_SIZE: f32 = 14.0;

/// The title's line box. The card's `h3` is `leading-tight`
/// (`ui/components/content/NewsArticleCard.vue:31`), which is 1.25 x 14 = 17.5, rounded up
/// to the whole point a line box takes.
const TITLE_LINE: f32 = 18.0;

/// The gap between the title and the copy: the card's inner stack is `gap-2`
/// (`ui/components/content/NewsArticleCard.vue:30`), 8 points.
const TITLE_TO_COPY: f32 = 8.0;

/// The copy's size: 12 points, from the reference's measurement.
const COPY_SIZE: f32 = 12.0;

/// How many lines the copy takes in the reference: the article's summary wraps twice at
/// the panel's inner width, which is why the card reserves room for two.
const COPY_LINES: usize = 2;

/// One line of the copy's box.
const COPY_LINE: f32 = 16.0; // guess: re-measure

/// The room two lines of copy are given, and the least the card ever reserves for them.
const COPY_H: f32 = COPY_LINE * COPY_LINES as f32;

/// The gap between the copy and the date: the date is the last child of the same `gap-2`
/// stack (`ui/components/content/NewsArticleCard.vue:30`), so 8 points.
const COPY_TO_DATE: f32 = 8.0;

/// The date's size: 12 points.
const DATE_SIZE: f32 = 12.0; // guess: re-measure

/// The date's line box.
const DATE_LINE: f32 = 16.0; // guess: re-measure

/// The gap between the date and the upgrade row.
const DATE_TO_LINK: f32 = 12.0; // guess: re-measure

/// The upgrade row's height.
const LINK_H: f32 = 32.0; // guess: re-measure

/// The upgrade row's label size.
const LINK_SIZE: f32 = 13.0; // guess: re-measure

/// The sparkle beside the upgrade label: 12 points, "small" being all the reference's
/// measurement says about it.
const SPARKLE: f32 = 12.0;

/// The gap between the sparkle and the label.
const SPARKLE_GAP: f32 = 6.0; // guess: re-measure

// ---------------------------------------------------------------------------
// The hosting banner
// ---------------------------------------------------------------------------

/// The hosting banner's padding: the promo card's `p-4`
/// (`ui/components/servers/ServersPromo.vue:22`), 16 points.
const BANNER_PAD: f32 = 16.0;

/// The banner's two lines: 16 semibold, from the reference's measurement.
const BANNER_SIZE: f32 = 16.0;

/// One banner line's box: the promo's copy is `leading-tight`
/// (`ui/components/servers/ServersPromo.vue:31`) at 16, which is 20 points.
const BANNER_LINE: f32 = 20.0;

/// The room the banner keeps at its bottom for the promo art.
///
/// The reference draws the big Modrinth mark bleeding off the plate's corner
/// (`ui/components/servers/ServersPromo.vue:24-29`); this launcher paints nothing into the
/// space yet, and the reservation is what keeps the two lines where the capture puts them.
const BANNER_ART: f32 = 64.0; // guess: re-measure

/// The banner's height: its padding, its two lines, and the art's room.
const BANNER_H: f32 = BANNER_PAD * 2.0 + BANNER_LINE * 2.0 + BANNER_ART;

// ---------------------------------------------------------------------------
// Radii
// ---------------------------------------------------------------------------

/// A row's radius: 8, from the reference's measurement. The sheet's step button is
/// `rounded-xl` (`ui/onboarding-checklist/index.vue:105`), which this preset's Tailwind
/// defaults resolve to 12, so the two disagree by four points; the measurement is what is
/// drawn, and the generated ladder is where the 8 comes from rather than a second literal.
const ROW_RADIUS: u8 = crate::theme::RADIUS_SM;

/// A card's radius: 12, from the reference's measurement, which is also what the article's
/// `rounded-xl` image resolves to. The hosting banner is `rounded-2xl` (16) in the sheet;
/// the measurement says both plates are 12, and the parity loop settles the difference.
const CARD_RADIUS: u8 = crate::theme::RADIUS_MD;

// ---------------------------------------------------------------------------
// The words
//
// The reference's own strings, not written here: a capture compares this section against
// the reference's text, so the text has to be the reference's. The article's four fields
// are the feed's own answers for the entry it leads with
// (`https://modrinth.com/news/feed/articles.json`, read live in `GATES.md`'s G101).
// ---------------------------------------------------------------------------

/// One step of the checklist, as a thing the shell can act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// `Create first instance`.
    CreateInstance,
    /// `Sign in to Minecraft`.
    Minecraft,
    /// `Sign in to Modrinth`.
    Modrinth,
}

impl Step {
    /// The three, in the order they are drawn.
    pub const ALL: [Step; 3] = [Step::CreateInstance, Step::Minecraft, Step::Modrinth];

    /// The row's own label.
    pub fn label(self) -> &'static str {
        match self {
            Step::CreateInstance => STEPS[0],
            Step::Minecraft => STEPS[1],
            Step::Modrinth => STEPS[2],
        }
    }
}

/// What a checklist row says, given who is signed in.
///
/// The Minecraft row is the one that changes: the reference's checklist strikes a finished step
/// through, and the launcher's own wording for that state is the account's name. A row that
/// insisted `Sign in to Minecraft` after a successful sign-in would be asking again.
pub fn row_label(step: Step, account: Option<&str>) -> String {
    match (step, account) {
        (Step::Minecraft, Some(name)) => format!("Signed in as {name}"),
        (step, _) => step.label().to_owned(),
    }
}

/// The heading the reference prints above the checklist.
const GETTING_STARTED: &str = "Getting started";

/// The checklist's steps, in the reference's order: an instance, a Minecraft session, then
/// a Modrinth one, because the first is what the other two are useful with.
const STEPS: [&str; 3] = [
    "Create first instance",
    "Sign in to Minecraft",
    "Sign in to Modrinth",
];

/// The second section's heading: the reference's `app.news.title`.
const NEWS: &str = "News";

/// The lead article's title.
const NEWS_TITLE: &str = "Sync settings across instances";

/// The lead article's summary, which is the two lines of copy the card is measured for.
const NEWS_COPY: &str =
    "Keep game options, servers, resource packs, and more the same across all your instances.";

/// The lead article's date, in the reference's own `dateStyle: 'long'` spelling.
const NEWS_DATE: &str = "September 7, 2026";

/// The upgrade row's label: the reference's `app.nav.upgrade-to-modrinth-plus`.
const UPGRADE: &str = "Upgrade to Modrinth+";

/// The hosting banner's first line.
const HOSTING: &str = "Get your next server";

/// The hosting banner's second line, which the reference highlights in the accent.
const HOSTING_ACCENT: &str = "on Modrinth.";

/// Draw the panel into `rect`: its ground and left rule, `Getting started`, the `News`
/// card, then the hosting banner.
///
/// Returns whether anything is still animating. Nothing is: the hovers are steps between
/// two fills rather than tweens, and egui asks for a frame by itself when the pointer
/// moves, so no frame is owed after this one.
pub fn draw(ui: &mut Ui, rect: Rect, theme: &Theme, account: Option<&str>) -> Option<Step> {
    let painter = ui.painter();

    // The panel's own ground, then its one-pixel left rule.
    //
    // The rule is stroked half a pixel in from the edge rather than on it: a stroke is
    // centred on its coordinate, so `left + 0.5` is what puts the line's whole width
    // inside the panel instead of half of it hanging over the seam to the content column.
    painter.rect_filled(rect, 0, theme.BG);
    painter.vline(
        rect.left() + 0.5,
        rect.y_range(),
        Stroke::new(RULE, theme.DIVIDER),
    );

    // The copy is laid out before the plan, because the card's rows start where the copy
    // ends. `COPY_H` is the floor: if a future summary wraps to a third line, the date
    // moves down with it rather than being written over.
    let copy = painter.layout(
        NEWS_COPY.to_owned(),
        fonts::body(COPY_SIZE),
        theme.BASE,
        column(rect).width(),
    );
    let plan = plan_with_copy(rect, copy.size().y.max(COPY_H));
    let column = plan.column;

    // ---- `Getting started` ---------------------------------------------------
    painter.text(
        egui::pos2(column.left(), plan.getting_started),
        Align2::LEFT_TOP,
        GETTING_STARTED,
        fonts::semibold(HEADING_SIZE),
        theme.CONTRAST,
    );
    // The disclosure chevron, on the heading's own line and in its own ink. The checklist's
    // accordion is open by default (`open-by-default`,
    // `ui/onboarding-checklist/index.vue:91`), and `Accordion.vue:19` rotates its
    // `dropdown` glyph 180 degrees while open -- which is `chevron-up`. `icons::draw` fits
    // and centres a glyph and cannot rotate one, so the open state is named by choosing
    // the glyph the rotation would have produced.
    icons::draw(
        painter,
        Glyph::ChevronUp,
        Rect::from_min_size(
            egui::pos2(
                column.right() - CHEVRON,
                plan.getting_started + (HEADING_LINE - CHEVRON) / 2.0,
            ),
            Vec2::splat(CHEVRON),
        ),
        theme.CONTRAST,
    );

    // The pointer, once, for every row: where it is and whether it just pressed. Nothing is
    // registered with egui, so nothing drawn later in the frame can take the press away.
    let (pointer, clicked) = ui.input(|input| {
        (
            input.pointer.interact_pos(),
            input.pointer.primary_clicked(),
        )
    });
    let mut pressed = None;
    for (index, step) in STEPS.into_iter().enumerate() {
        let row = Rect::from_min_size(
            egui::pos2(column.left(), row_top(plan.steps_first, index)),
            egui::vec2(column.width(), ROW_H),
        );
        // A step brightens under the pointer the way the sheet's does
        // (`hover:brightness-110`, `ui/onboarding-checklist/index.vue:109`), by the
        // theme's own `--hover-brightness`, and a press is reported to the shell.
        let over = pointer.is_some_and(|pointer| row.contains(pointer));
        if over && clicked {
            pressed = Step::ALL.get(index).copied();
        }
        let hovered = over;
        let fill = if hovered {
            theme.hovered(theme.RAISED_BG)
        } else {
            theme.RAISED_BG
        };
        painter.rect_filled(row, ROW_RADIUS, fill);

        // The ring, in the row's own `text-primary`: the sheet's unchecked
        // `RadioButtonIcon` inherits the button's colour, and a completed step replaces it
        // with a check. The checklist's state is the shell's, so this draws the state a
        // fresh install is in -- nothing done yet.
        icons::draw(
            painter,
            Glyph::RadioButton,
            Rect::from_min_size(
                egui::pos2(row.left() + ROW_PAD, row.center().y - RING / 2.0),
                Vec2::splat(RING),
            ),
            theme.BASE,
        );
        painter.text(
            egui::pos2(row.left() + ROW_PAD + RING + RING_GAP, row.center().y),
            Align2::LEFT_CENTER,
            row_label(Step::ALL[index], account),
            fonts::body(STEP_SIZE),
            theme.BASE,
        );
    }

    // ---- `News` --------------------------------------------------------------
    painter.text(
        egui::pos2(column.left(), plan.news),
        Align2::LEFT_TOP,
        NEWS,
        fonts::semibold(HEADING_SIZE),
        theme.CONTRAST,
    );

    let rows = plan.news_rows;
    let card = plan.card;

    // The art plate. The reference loads the article's `thumbnail` into the top of this
    // card; the launcher does not load news images yet -- the feed answers with a URL and
    // the only image pipeline here fetches project icons -- so the plate stands in, in the
    // super-raised fill, inside the sheet's own `border-button-border` edge
    // (`ui/components/content/NewsArticleCard.vue:28`).
    let art = Rect::from_min_size(
        egui::pos2(column.left(), card + rows.art),
        egui::vec2(column.width(), ART_H),
    );
    painter.rect_filled(art, CARD_RADIUS, theme.SUPER_RAISED_BG);
    painter.rect_stroke(
        art,
        CARD_RADIUS,
        Stroke::new(RULE, theme.BUTTON_BORDER),
        egui::StrokeKind::Inside,
    );

    painter.text(
        egui::pos2(column.left(), card + rows.title),
        Align2::LEFT_TOP,
        NEWS_TITLE,
        fonts::semibold(TITLE_SIZE),
        theme.CONTRAST,
    );
    // The copy is drawn into the slot the plan reserved for it, not into whatever height
    // it happened to take: the rows below are on the reference's rhythm, and text that
    // pushed them around would move the date on every article that wrapped differently.
    painter.galley(
        egui::pos2(column.left(), card + rows.copy),
        copy,
        theme.BASE,
    );
    painter.text(
        egui::pos2(column.left(), card + rows.date),
        Align2::LEFT_TOP,
        NEWS_DATE,
        fonts::body(DATE_SIZE),
        theme.SECONDARY,
    );

    // The upgrade row. Its words are measured in `ACCENT_CONTRAST`, which is the
    // stylesheet's name for text *on* a brand fill and resolves to black in the dark
    // scheme -- so the row is the brand-filled control the reference's news section carries
    // for its other link (`ButtonLink type="colored" color="brand"`, `App.vue:2540`), and
    // the ink is legible in both schemes. If a capture shows the reference's plain purple
    // link there (`text-purple`, `App.vue:2558`), this is the one fill to change.
    let link = Rect::from_min_size(
        egui::pos2(column.left(), card + rows.link),
        egui::vec2(column.width(), LINK_H),
    );
    let link_hovered = ui
        .interact(link, egui::Id::new("panel-upgrade"), Sense::hover())
        .hovered();
    let link_fill = if link_hovered {
        theme.hovered(theme.BRAND)
    } else {
        theme.BRAND
    };
    painter.rect_filled(link, ROW_RADIUS, link_fill);
    let label = painter.layout_no_wrap(
        UPGRADE.to_owned(),
        fonts::semibold(LINK_SIZE),
        theme.ACCENT_CONTRAST,
    );
    // Sparkle and label together, centred: the sheet centres this row's contents, and a
    // pair laid out from the left would leave the icon hanging outside the text's measure.
    let content = SPARKLE + SPARKLE_GAP + label.size().x;
    let x = link.center().x - content / 2.0;
    icons::draw(
        painter,
        Glyph::Sparkles,
        Rect::from_center_size(
            egui::pos2(x + SPARKLE / 2.0, link.center().y),
            Vec2::splat(SPARKLE),
        ),
        theme.ACCENT_CONTRAST,
    );
    painter.galley(
        egui::pos2(
            x + SPARKLE + SPARKLE_GAP,
            link.center().y - label.size().y / 2.0,
        ),
        label,
        theme.ACCENT_CONTRAST,
    );

    // ---- The hosting banner --------------------------------------------------
    let banner = Rect::from_min_size(
        egui::pos2(column.left(), plan.banner),
        egui::vec2(column.width(), BANNER_H),
    );
    painter.rect_filled(banner, CARD_RADIUS, theme.RAISED_BG);
    painter.text(
        egui::pos2(banner.left() + BANNER_PAD, banner.top() + BANNER_PAD),
        Align2::LEFT_TOP,
        HOSTING,
        fonts::semibold(BANNER_SIZE),
        theme.CONTRAST,
    );
    // The accent is `--color-brand`, which is how the promo highlights its second line
    // (`text-brand` on "your friends?", `ui/components/servers/ServersPromo.vue:33`).
    // `ACCENT_CONTRAST` is the foreground of that colour -- black in the dark scheme -- so
    // it is the wrong token for text on the panel's own ground.
    painter.text(
        egui::pos2(
            banner.left() + BANNER_PAD,
            banner.top() + BANNER_PAD + BANNER_LINE,
        ),
        Align2::LEFT_TOP,
        HOSTING_ACCENT,
        fonts::semibold(BANNER_SIZE),
        theme.BRAND,
    );
    // Everything below the two lines is the art's room, and stays empty until the promo's
    // mark is loaded as a texture: `BANNER_H` already counts it.

    pressed
}

/// The panel's content column: the panel less [`PAD`] on each side.
///
/// Every row and card spans this and nothing else, so the padding is stated once and a
/// capture that moves it moves the whole panel rather than half of it.
fn column(panel: Rect) -> Rect {
    Rect::from_min_max(
        egui::pos2(panel.left() + PAD, panel.top()),
        egui::pos2(panel.right() - PAD, panel.bottom()),
    )
}

/// The top edge of the row at `index` in a stack that begins at `first_top`.
///
/// A row is [`ROW_H`] tall and the gap to the next is [`ROW_GAP`], so a row's top sits one
/// [`ROW_PITCH`] below the one above it. The drawing and the tests both read this rather
/// than multiplying the pitch out by hand in two places.
fn row_top(first_top: f32, index: usize) -> f32 {
    first_top + index as f32 * ROW_PITCH
}

/// How tall a stack of `count` rows is: the rows plus the gaps between them, and no gap
/// after the last one, which is what makes a section's bottom edge a row's bottom edge.
fn stack_height(count: usize) -> f32 {
    if count == 0 {
        return 0.0;
    }
    count as f32 * ROW_H + count.saturating_sub(1) as f32 * ROW_GAP
}

/// The news card's rows, as offsets from the card's own top edge.
///
/// Each offset is derived from the one above it, so a re-measured gap moves everything
/// below it and nothing above it -- which is what makes a single corrected constant enough
/// to fix the card rather than a rewrite of five positions.
#[derive(Debug, Clone, Copy)]
struct NewsRows {
    /// The top of the art plate. Zero: the plate is the card's first row.
    art: f32,
    /// The top of the title's line box.
    title: f32,
    /// The top of the copy's first line.
    copy: f32,
    /// The top of the date's line box.
    date: f32,
    /// The top of the upgrade row.
    link: f32,
    /// The card's height: the bottom edge of the upgrade row.
    height: f32,
}

/// The card's rows, given the height the copy was given.
fn news_rows(copy_height: f32) -> NewsRows {
    let art = 0.0;
    let title = art + ART_H + ART_TO_TITLE;
    let copy = title + TITLE_LINE + TITLE_TO_COPY;
    let date = copy + copy_height + COPY_TO_DATE;
    let link = date + DATE_LINE + DATE_TO_LINK;
    NewsRows {
        art,
        title,
        copy,
        date,
        link,
        height: link + LINK_H,
    }
}

/// Every edge the panel draws at, computed once from the panel's own rect.
///
/// The drawing and the tests read the same arithmetic. A test that re-derived the stacking
/// would keep passing while the panel painted something else, which is the failure this
/// struct exists to make impossible.
struct Plan {
    /// The content column, from [`column`].
    column: Rect,
    /// The top of the `Getting started` heading's line box.
    getting_started: f32,
    /// The top of the first checklist row; every row's top is [`row_top`] from here.
    steps_first: f32,
    /// The top of the `News` heading's line box.
    news: f32,
    /// The top of the news card.
    card: f32,
    /// The card's own rows, from the card's top edge.
    news_rows: NewsRows,
    /// The top of the hosting banner's plate.
    banner: f32,
}

/// The panel's plan, given the height the news copy took.
fn plan_with_copy(panel: Rect, copy_height: f32) -> Plan {
    let column = column(panel);
    let getting_started = panel.top() + TOP_PAD;
    let steps_first = getting_started + HEADING_LINE + HEADING_TO_BODY;
    // The next heading is measured from the last row's bottom, not its top: the space
    // belongs to the section boundary, and a row is 36 tall.
    let news = steps_first + stack_height(STEPS.len()) + SECTION_GAP;
    let card = news + HEADING_LINE + NEWS_HEADING_TO_CARD;
    let rows = news_rows(copy_height);
    let banner = card + rows.height + SECTION_GAP;
    Plan {
        column,
        getting_started,
        steps_first,
        news,
        card,
        news_rows: rows,
        banner,
    }
}

#[cfg(test)]
mod label_tests {
    use super::*;

    /// Only the Minecraft row changes, and only when there is a name to show.
    #[test]
    fn the_minecraft_row_names_the_account() {
        assert_eq!(
            row_label(Step::Minecraft, Some("Steve")),
            "Signed in as Steve"
        );
        assert_eq!(row_label(Step::Minecraft, None), "Sign in to Minecraft");
        assert_eq!(
            row_label(Step::CreateInstance, Some("Steve")),
            "Create first instance",
            "other rows are unaffected by the account"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry as g;

    /// The panel's rect, built the way `Shell` builds it, so a test that passes is a panel
    /// drawn into the window the captures are taken at rather than into a convenient box.
    fn panel_rect() -> Rect {
        Rect::from_min_size(
            egui::pos2(g::PANEL_LEFT, g::TITLE_BAR),
            egui::vec2(g::PANEL, g::WINDOW.1 - g::TITLE_BAR),
        )
    }

    /// The three checklist rows' top edges, as the drawing would place them.
    fn step_tops() -> Vec<f32> {
        let plan = plan_with_copy(panel_rect(), COPY_H);
        (0..STEPS.len())
            .map(|index| row_top(plan.steps_first, index))
            .collect()
    }

    /// Three rows of 36 with 8 between them is the reference's checklist, and it is the one
    /// thing `docs/PARITY-LOG.md` OBS-4 left for a capture to settle.
    #[test]
    fn three_rows_are_thirty_six_tall_and_eight_apart() {
        let tops = step_tops();
        assert_eq!(tops.len(), 3, "the checklist has three steps");
        assert_eq!(ROW_H, 36.0, "a row is 36 tall");
        assert_eq!(ROW_GAP, 8.0, "the gap between two rows is 8");
        for pair in tops.windows(2) {
            let pitch = pair[1] - pair[0];
            assert_eq!(pitch, ROW_PITCH, "two rows are one pitch apart");
            assert_eq!(pitch - ROW_H, ROW_GAP, "what is left of the pitch is the gap");
        }
        // Three rows and the two gaps between them: 108 + 16.
        assert_eq!(stack_height(3), 124.0);
        assert_eq!(stack_height(0), 0.0, "no rows end where they start");
    }

    /// A row is inside the panel's content column, and the column is the panel less its
    /// padding: a row that ran past the padding would touch the window's own edge.
    #[test]
    fn the_rows_stay_inside_the_panel() {
        let panel = panel_rect();
        let column = column(panel);
        assert_eq!(column.width(), g::PANEL - 2.0 * PAD, "24 points of padding a side");
        assert_eq!(column.width(), 292.0);
        assert_eq!(column.left() - panel.left(), PAD);
        assert_eq!(column.right(), panel.right() - PAD);
        for top in step_tops() {
            let row = Rect::from_min_size(
                egui::pos2(column.left(), top),
                egui::vec2(column.width(), ROW_H),
            );
            assert!(row.left() >= panel.left(), "the row starts inside the panel");
            assert!(row.right() <= panel.right(), "the row ends inside the panel");
            assert!(row.top() >= panel.top(), "the row starts below the panel's top");
            assert!(row.bottom() <= panel.bottom(), "the row ends above the panel's bottom");
        }
    }

    /// The section rhythm the reference states: a heading sits 20 below the last row of the
    /// section above it, and the hosting banner 20 below the news card.
    #[test]
    fn a_section_heading_sits_twenty_below_the_section_above_it() {
        let plan = plan_with_copy(panel_rect(), COPY_H);
        let steps_bottom = plan.steps_first + stack_height(STEPS.len());
        assert_eq!(plan.news - steps_bottom, SECTION_GAP);
        assert_eq!(plan.banner - (plan.card + plan.news_rows.height), SECTION_GAP);
        assert!(plan.card > plan.news, "the card sits below its own heading");
    }

    /// The card's rows stack downward and it stops at the upgrade row's bottom: this is the
    /// arithmetic a re-measured gap moves, so it is the one worth pinning.
    #[test]
    fn the_news_card_stacks_downward() {
        let rows = news_rows(COPY_H);
        assert_eq!(rows.art, 0.0, "the art is the card's first row");
        assert!(rows.art < rows.title, "the title is below the art");
        assert!(rows.title < rows.copy, "the copy is below the title");
        assert!(rows.copy < rows.date, "the date is below the copy");
        assert!(rows.date < rows.link, "the link is below the date");
        assert_eq!(rows.height, rows.link + LINK_H, "the card ends with the link row");
    }

    /// A copy that wrapped to a third line pushes the date and the row below it down by
    /// exactly one line and leaves the art and the title where they were -- which is why
    /// the card is laid out from the copy's height rather than from a fixed total.
    #[test]
    fn a_third_line_of_copy_moves_everything_below_it() {
        let two = news_rows(COPY_H);
        let three = news_rows(COPY_H + COPY_LINE);
        assert_eq!(three.height - two.height, COPY_LINE);
        assert_eq!(three.art, two.art);
        assert_eq!(three.title, two.title);
        assert_eq!(three.date - two.date, COPY_LINE);
        assert_eq!(three.link - two.link, COPY_LINE);
    }

    /// Everything the panel draws fits between the title bar and the window's bottom edge.
    /// The panel does not scroll, so a constant that grew would draw off the window, and one
    /// that collapsed would leave the column half empty.
    #[test]
    fn the_whole_panel_fits_the_window() {
        let panel = panel_rect();
        assert_eq!(panel.bottom(), g::WINDOW.1, "the panel ends at the window's edge");
        let plan = plan_with_copy(panel, COPY_H);
        let bottom = plan.banner + BANNER_H;
        assert!(bottom <= g::WINDOW.1, "the hosting banner ends inside the window");
        assert!(
            bottom > g::WINDOW.1 / 2.0,
            "the three sections fill more than half the column"
        );
        assert!(
            plan.card < plan.banner,
            "the card sits above the banner: card top {} must be less than banner top {}",
            plan.card,
            plan.banner
        );
    }
}
