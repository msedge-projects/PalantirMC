//! A user's profile: `pages/User.vue` and the `UserProfilePageLayout` it renders,
//! at `/user/:user/:projectType?`.
//!
//! The reference's page is a header -- the avatar, the name and handle, a summary,
//! and three facts: how many projects, how many downloads between them, and when
//! the account joined -- over a strip of that user's project types and the list
//! under it. The header's facts and the whole list come from Modrinth's *published*
//! API: `GET /v2/user/{name}` and `GET /v2/user/{id}/projects`. That is what makes
//! this page real without a Modrinth session.
//!
//! Two of the things the page draws are *not* on the published API, and a comment
//! here once said they were unreachable without a Modrinth credential. They are not.
//! Measured, anonymously, against the live service:
//!
//! ```text
//! GET /v3/user/P3U9o13d/projects     -> 200   (project_types, the array v2 flattens)
//! GET /v3/user/FlameFire/collections -> 200, 1176 bytes
//! GET /v2/user/FlameFire/collections -> 404   (there is no v2 spelling of it)
//! ```
//!
//! So the strip is counted off v3's array through [`Profile::type_of`], and its
//! *Collections* tab and the cards under it are read from v3's collections route.
//! A comment that calls a route unreachable when it answers 200 is worse than no
//! comment: the next reader believes it and stops looking.
//!
//! The page *asks* rather than fetches, like Discover and the project page:
//! [`State::update`] and [`State::opening`] hand the shell an [`Asked`], the shell
//! runs it through the store off the frame thread, and the answer comes back as
//! [`Message::Found`]. The round travels with the request, so an answer to a user
//! the reader has already left is dropped instead of drawn under the one they are
//! looking at.
//!
//! Three of the things the page draws are the reference's own arithmetic over the
//! projects list -- the count, the sum of its downloads, and which filters the strip
//! has -- so they are computed here rather than asked for: the API publishes one
//! document per request, and a total is not one of them.
//!
//! One card's tags row is the fourth, and it is the one place where reading v2
//! costs the page something the reference draws. `ProjectList.vue` composes that
//! row out of a **v3** project: `getProjectCardTags` is its `categories`, its
//! `loaders` and its `mrpack_loaders`, and `catalogProjectTypes` is its
//! `project_types` *array* -- which is where the strip's *Data Packs* comes from,
//! against the v2 `project_type` string that says *Mods*.
//!
//! Both of those are fields `/v2/user/{id}/projects` does not publish, and the one
//! request that would carry both is `GET /v3/user/{id}/projects` -- public, and
//! answering 200 for an unauthenticated read of a public account, which is how
//! G105's "the v3 user service needs a session" was narrowed to the routes that
//! actually do. `ModrinthApi::user_projects_v3` asks for it and
//! [`Profile::projects_v3`] carries it, so the strip is counted off v3's array
//! through [`Profile::type_of`].
//!
//! Collections are the other v3 read, and the only one with no v2 route behind it at
//! all: `GET /v3/user/{id}/collections` answers 200 anonymously and
//! `GET /v2/user/{id}/collections` answers 404.
//! [`Profile::collections`] carries it, and it is what decides whether the strip ends
//! in a *Collections* tab (`layout.vue:765`), which selects a different branch of the
//! page entirely -- the collection cards rather than the project list.
//!
//! What that read still cannot fix is a card's row. `ProjectCard.vue` is composed
//! out of the v3 project document as a whole -- `name` where v2 says `title`,
//! `summary` where it says `description` -- and moving the page onto that document
//! is a change to every card rather than to one field, so [`card_tags`] still puts
//! back the one tag it can derive from what v2 *does* publish (a modpack's `mrpack`
//! loader, measured across a hundred modpacks). The array is read; the rest of the
//! document is not.

use iced::mouse::Interaction;
use iced::widget::container;
use iced::widget::{column, mouse_area, row, Space};
use iced::{Alignment, Border, Element, Length, Padding, Vector};
// The deep path rather than a `palantir_net` re-export: this is the one type the
// page names that `palantir_net`'s convenience list does not carry, and `lib.rs` is
// not this slice's to edit.
use palantir_net::engine::modrinth::{ModrinthCollection, ModrinthV3Project};
use palantir_net::modrinth::{ModrinthUser, ModrinthUserProject};

use super::overlay::Stack;
use crate::icon;
use crate::icons_gen::Glyph;
use crate::page::{self, Load, GAP};
use crate::route::ProjectType;
use crate::store::Store;
use crate::style::{medium, regular, semibold, INK_CONTRAST, INK_DEFAULT, INK_SECONDARY};
use crate::text_gen::{self, Key};
use crate::theme_gen::{self, Ink, Theme as Gen};
use crate::ui::{self, text, Hovered};

// ---- The page's geometry, quoted ------------------------------------------

/// The avatar's side on the page.
///
/// `UserPageHeader.vue` draws `:size="isModrinthUser ? '64px' : '96px'"`, and which
/// account is Modrinth's own is a lookup in `@modrinth/utils`, a package this tree
/// does not vendor -- so every profile is drawn at the 96 that a normal account
/// gets. The alternative would be a size that changed on an id nobody here can
/// recognize. Measured at the reference's own 1280x720: the picture occupies
/// y=72..167, which is the 96.
const AVATAR: f32 = 96.0;

/// `gap-4` on `PageHeader`'s row and on the row inside it, so the avatar and the
/// text column are 16 apart: measured, the avatar ends at x=181 and the title
/// starts at x=202.
const HEADER_GAP: f32 = 16.0;
/// `text-2xl` with `leading-none`: a 24-pixel line, measured as a 17-pixel cap
/// between y=85 and y=101.
const TITLE: f32 = 24.0;
/// `gap-1.5`, between the title and the summary.
const TITLE_GAP: f32 = 6.0;
/// `gap-2`, between the title-and-summary block and the metadata row.
const HEADER_GROUP_GAP: f32 = 8.0;
/// The summary and the metadata are both 16: measured, the `J` of *Just* and the
/// `J` of *Joined* are each twelve rows tall.
const HEADER_TEXT: f32 = 16.0;
/// The line the header's own summary is set on -- the CSS line, not the painted one.
///
/// `page-header/index.vue:20` gives the summary no size class, so it inherits the
/// body size (16) and the stylesheet's `line-height: 1.15`, which is 18.4 pixels of
/// CSS. The same reading [`crate::ui::NAV_LABEL_LINE`] records for a tab label.
///
/// 18.4 rather than the 18 those fifteen paint to, and the reason is that this line
/// height is load-bearing for the column around it:
/// `page-header/index.vue:9`'s text column is stretched to the avatar's 96 rows
/// and its children are centred by `justify-center`, so the column starts at
/// `(96 - H) / 2` with `H = 24 + 6 + this + 8 + 20` -- and half a pixel of `H` is
/// half a pixel of where the title and the metadata are drawn.
///
/// Measured at 1280x720 against `/tmp/ref/user-ref.png`, ink rows at half coverage,
/// across the three values this column can take:
///
/// | this line, and the metadata row's | title | summary | metadata | pixels off the reference, nine bands |
/// | --- | --- | --- | --- | --- |
/// | 18.0, row 20.8 (before [`METADATA_LINE`]) | 84..101 | 114..128 | 139..156 | 29522 |
/// | 18.0, row 20 | 85..102 | 115..129 | 139..156 | 29213 |
/// | **18.4, row 20** | **84..101** | 114..128 | 139..156 | **29204** |
/// | the reference | 84..101 | 115..129 | 139..156 | -- |
///
/// The title and the summary trade a single row between them, because a centred
/// first child puts the title's baseline at `31 - H_summary / 2` and the summary's
/// at `title + 12 + 6 + H_summary / 2` -- both the same number for either 18, and
/// the capture differs only in which side of a whole-pixel boundary each falls on.
/// 18.4 is the one that leaves the *title* on the reference's rows, and the title
/// is the landmark the profile page is known for, so the summary's row is what
/// pays.
const HEADER_SUMMARY_LINE: f32 = 18.4;
/// The line the metadata row's words are set on, which is *not* the summary's.
///
/// `page-header-metadata-item.vue:79`'s `baseClass` ends in `leading-none`, and
/// `leading-none` is `line-height: 1` -- neither the `text-*` scale's line nor
/// the inherited `1.15` the header's own summary keeps. It beats `text-sm`'s own
/// `1.25rem` because both are utilities of one class and the line-height ones are
/// written after the font-size ones: the same order that gives
/// `page-header/index.vue:13`'s `text-2xl leading-none` `h1` a 24-pixel line
/// rather than 32, and that makes a `TagItem`'s icon-less pill
/// `1 + py-1(4) + 14 + py-1(4) + 1 = 24` rows rather than the 30 a `text-sm`
/// line would ask for.
///
/// So the number is the label's own size, and setting it is not tidiness: left at
/// iced's default `Relative(1.3)` the words measured 20.8 rows against the 20 of
/// the `size-5` icon beside them, and the row -- the header's last block -- grew
/// by four fifths of a pixel.
const METADATA_LINE: f32 = 16.0;
/// `gap-x-[1.625rem]` on the metadata row, which is also the width of the span
/// each `BulletDivider` sits in -- so the gap and the divider are one number.
const METADATA_GAP: f32 = 26.0;
/// `min-w-1.5 min-h-1.5` on the divider, and `mx-0.5` around it.
const METADATA_DOT: f32 = 6.0;
/// `size-5` on a metadata item's icon, and `gap-2` between it and the words.
const METADATA_ICON: f32 = 20.0;
const METADATA_TEXT_GAP: f32 = 8.0;
/// `pb-4` under the header's own rule, which is the 16 pixels between the rule at
/// y=184 and the strip at y=201.
const HEADER_PAD_BOTTOM: f32 = 16.0;

/// The size and the line of the sentence an empty list draws.
///
/// `EmptyState.vue:7` is `<span class="text-2xl font-semibold text-contrast">`, and
/// both empty sentences on this page are that component's *heading*
/// (`layout.vue:243`'s `profile.label.no-projects` and `layout.vue:328`'s
/// `profile.label.no-collections`) rather than its description -- the descriptions
/// are the `isSelf`-only arm this launcher never reaches. `text-2xl` is
/// `1.5rem`/`2rem`, so the sentence is twenty-four pixels of text on a
/// thirty-two-pixel line; it is a `span` and not a heading, so there is no user
/// agent's line-height in it to read instead, whatever `preflight: false` does
/// elsewhere on this page.
///
/// The weight is the third half of the same class, and it is 600 and not the 800
/// [`crate::style::heading`] draws a real heading at: `font-semibold` in the one
/// span, `--font-weight-heading` in the other. Measured, the reference's own
/// sentence is set no heavier than the `text-base font-medium` words under it.
const EMPTY_HEADING: f32 = 24.0;
/// [`EMPTY_HEADING`]'s own line, which is `text-2xl`'s `2rem` and not its size.
const EMPTY_HEADING_LINE: f32 = 32.0;

/// `gap-x-3` and `gap-y-2` on a project card's own grid.
///
/// The horizontal one is also a *track* of its own, which is what the width a
/// card's summary has to stop in is made of: see [`CARD_ICON`], [`project_row`]
/// and css-grid-1 §11.2's note that "gutters are treated as fixed-size tracks --
/// tracks with their min and max sizing functions both set to the gutter's used
/// size -- for the purpose of the grid sizing algorithm".
const CARD_GAP_X: f32 = 12.0;
const CARD_GAP_Y: f32 = 8.0;
/// `gap-3` on `ProjectCardList`, between the cards of the list.
const LIST_GAP: f32 = 12.0;
/// `text-xl` on `ProjectCardTitle`, which is the list layout's own size.
const CARD_TITLE: f32 = 20.0;
/// `Avatar size="100px"` in the list layout: the icon every card opens with.
const CARD_ICON: f32 = crate::avatar::ICON_SIDE as f32;
/// `gap-3` between a card's stats and its date, and on the stats row.
const CARD_STATS_GAP: f32 = 12.0;
/// The room between a card's *Install* button and the stats below it.
///
/// `__stats` spans grid rows two and three and carries `:class="{ 'mt-3': ... }"`,
/// so its first row starts a `gap-y-2` below the button's row *and* an `mt-3` into
/// it: `8 + 12 = 20`. Measured: the button's ring is y=276..311 and the downloads
/// icon's 20-pixel box begins at y=332.
const CARD_STATS_LEAD: f32 = CARD_GAP_Y + 12.0;
/// `gap-2` inside `__info`, between a card's title and its summary.
const CARD_INFO_GAP: f32 = 8.0;
/// The body size, which is what a card's summary is: [`HEADER_TEXT`], sixteen.
///
/// `ProjectCard.vue:120`'s list layout gives `.project-card-summary` no size class
/// at all, and `ProjectCard.vue:402-404`'s `@apply text-sm` is inside the
/// `@container (width < 550px)` block beside it -- an 868-pixel card, measured on
/// the reference's own capture, is nowhere near it. So the summary inherits the
/// body size.
///
/// Three readings off `/tmp/ref/user-ref.png` at 1280x720 fix the size at sixteen
/// and leave no room for fourteen, and the first is a ruler rather than a
/// comparison: Inter's em dash has an advance of exactly `1.0000 em` and an ink of
/// exactly `1.0000 em` (`Inter-400.ttf`, upem 2816, `U+2014` advance 2816), and
/// card two's summary ends in a pair of them -- thirty-two solid pixels from x=620
/// to x=651 with nothing antialiased at either end, which is two ems and so two
/// sixteens. The second is that the summary's `6` and the `6` of
/// `ProjectCardStats`' downloads count -- which carries no size class either, and so
/// is known to be sixteen -- are both twelve rows of ink, `461..472` and `490..501`
/// on card two. The third is that the same two digits sit exactly eighteen rows
/// apart with identical profiles, which is [`CARD_SUMMARY_LINE`] and not the twenty
/// a `text-sm` line would put between them.
///
/// A fourteen-pixel reading is also refuted by the wrap, which is what fixed this
/// size before the column's width was known: the reference's card-two first line
/// carries one word more than ours did (it ends *A skyblock*, ours ended *A*), and
/// the two sides' word boundaries were identical to the pixel over the whole shared
/// prefix -- `——` at x=620..651 and `A` at x=657..666 on both -- so the two are set
/// at the same size and the difference is where the column stops, not how big the
/// text is. That column is [`project_row`]'s `right` away now; the residual on the
/// second and third cards is our *Install to instance* button measuring 183 where
/// the reference's measures 189, which is the advance question the summary above
/// the `1fr` column and not a number in this file.
const CARD_SUMMARY: f32 = HEADER_TEXT;
/// The line the summary is set on: the painted eighteen, not the CSS eighteen-and-
/// four-fifths [`HEADER_SUMMARY_LINE`] keeps.
///
/// The same `1.15` at sixteen, one rounding apart from the header's own summary,
/// and the rounding is the point: `line-clamp-2` is a *height*, and this line is
/// what the box below the title is two of (see the `line-clamp-2` note at the call
/// site and the `36.0` the gate test asserts), so it is stored as the eighteen the
/// reference's own two lines are measured apart rather than as the eighteen-and-
/// four-fifths CSS calls for. `text-sm` would make it 20.
const CARD_SUMMARY_LINE: f32 = 18.0;
/// `size-5` on `ProjectCardStats`' and `ProjectCardDate`'s icons.
const CARD_STAT_ICON: f32 = 20.0;
/// The card's own content box.
///
/// `ProjectCard.vue`'s list card measures y=259..400 at the reference's own window
/// -- 142 including its two 1-pixel borders -- so the box inside the padding is
/// `142 - 2 - 32 = 108`... except that the box runs from y=276 to y=384 *inclusive*,
/// which is 109. The extra pixel is the border: CSS puts a `border-1px` outside the
/// padding edge and iced paints one inside its bounds, so the card asks for one
/// more of content (see [`CARD_PAD_TOP`]) to land its children where the
/// reference's do. The row heights then fall out of the reference's own grid:
/// 36 for the button's row, 8 of `gap-y-2`, 12 of `mt-3`, the 20-pixel stats row,
/// 12 of `gap-3` and the 20-pixel date row -- `36 + 8 + 12 + 20 + 12 + 20 = 108`,
/// and the one pixel the tags row's last row takes is the 109th.
const CARD_CONTENT: f32 = 109.0;
/// The card's top padding, which is [`ui::CARD_PAD`] and one more.
///
/// iced draws a container's border *inside* its bounds, so a card padded at 16
/// starts its content at `border + 16` where CSS starts it at `border + 1 + 16`:
/// measured, our icon was at x=105 against the reference's 105 and our card's own
/// rule 1 pixel narrower on the left, because the whole content column is a pixel
/// narrower -- but its *rows* were each a pixel high, because the border ate a pixel
/// of padding above them and none below. Seventeen above, sixteen below, is the
/// pair that puts the content box at y=276..384.
const CARD_PAD_TOP: f32 = ui::CARD_PAD + 1.0;
/// The card's right padding, which is [`ui::CARD_PAD`] and one more.
///
/// The reference's content box is x=105..938 on a card that is x=88..955 -- a
/// border and sixteen on each side -- and this card's own bounds begin at x=89
/// rather than 88, because the shell's page edge draws its one-pixel rule at
/// x=64 and starts the page at x=65 where the reference's page starts at the
/// rule (see `crate::shell`'s page container). So the sixteen on the left
/// already lands the content where the reference has it at 105, and the pixel the
/// narrower card gives back has to be carried on the right: measured before, the
/// *Install* button's ring ended at x=939 against the reference's 938 and its own
/// content box's 938, which is the whole of what this constant is.
const CARD_PAD_RIGHT: f32 = ui::CARD_PAD + 1.0;
/// Where the tags row starts inside the content box: `358 - 277 = 81`.
///
/// Measured as the row's own first row, not as the bottom of its tallest pill.
/// The row is `mt-auto` into the content box and it is 26 pixels tall whatever it
/// holds, so the row's *top* is the number and the pills inside it are placed by
/// `items-center`: the reference's own `<div class="flex items-center gap-1">`
/// around `ProjectCardEnvironment` and `ProjectCardTags`, which is what puts a
/// 24-row pill one pixel lower than a 26-row one on the same row. Both readings
/// were measured on `/tmp/ref/user-ref.png` at the first card: the three pills
/// that carry an `h-4` glyph are y=358..383 and the three that do not are
/// y=359..382.
const CARD_TAGS_TOP: f32 = 81.0;

/// The gap between the header and the strip, which is [`GAP`] plus four.
///
/// [`page::body`] spaces a page's blocks with [`GAP`] -- twelve -- and this page
/// has one boundary where that is not the number the reference uses.
///
/// `NormalPage` is `gap-y-4` -- sixteen -- and `NavTabs`' `page-nav` wrapper
/// carries `-mx-6 -mt-2 mb-1 px-6 py-2`, whose `-8` and `+8` cancel: the strip's
/// own pill therefore begins exactly one `gap-y-4` below the block above it.
/// Measured at the reference's own 1280x720: the header's rule is y=184 and the
/// strip's border is y=201, sixteen rows, and the strip's border to the first
/// card's is twelve -- which is [`GAP`]. So the rule-to-strip boundary is the one
/// that is sixteen and everything below the strip stays at twelve.
const HEADER_STRIP_GAP: f32 = GAP + 4.0;

/// `gap-1` on the tags row: `ProjectCard.vue`'s own
/// `<div class="flex items-center gap-1">` around `ProjectCardEnvironment` and
/// `ProjectCardTags`. Measured: six pills at 217..369, 374..469, 474..544,
/// 549..625, 630..729 and 734..767 -- four pixels between each pair.
const CARD_TAG_GAP: f32 = 4.0;
/// The row's own height, which is the reference's tallest pill rather than a
/// number of its own.
///
/// `TagItem.vue`'s `baseClass` is `py-1 leading-none text-sm` around a
/// `[&>svg]:h-4` glyph, so a pill carrying an icon is 26 rows and one that does
/// not is 24; the row is `items-center`, so the shorter pills sit a pixel lower
/// inside it. Measured on the reference's first card: y=358..383 for the
/// environment, *Forge* and *Modpack* pills and y=359..382 for *Challenging*,
/// *Combat* and *+1*.
const CARD_TAG_ROW: f32 = 26.0;

/// The width the reference lays a profile's content column out in.
///
/// Measured off `/tmp/ref/user-ref.png` at the reference's own 1280x720, where a
/// project card is `x=88..955` -- 868 pixels -- and the sidebar's own panel starts
/// at x=979. The page never learns this number at run time (iced hands `view` a
/// theme and a state and no width), so it is the measured constant rather than an
/// `auto-fill`, and [`collection_columns`] is what the reference's grid rule is
/// resolved against.
const COLLECTION_CONTENT: f32 = 868.0;
/// `minmax(350px, 1fr)`: the narrowest a collection card's track may be.
const COLLECTION_MIN: f32 = 350.0;
/// `ProjectCardList`'s own `gap-3` -- twelve -- between its tracks and between its
/// rows, which is [`GAP`] under a different name.
const COLLECTION_GAP: f32 = GAP;
/// `Avatar size="64px"`: the collection card's picture.
const COLLECTION_AVATAR: f32 = 64.0;
/// The card's `gap-4` between the head, the description and the foot -- sixteen,
/// which is [`page::GRID_GAP`] rather than this page's [`GAP`].
const COLLECTION_INNER_GAP: f32 = page::GRID_GAP;
/// `text-lg font-semibold`: the collection's own name.
const COLLECTION_NAME: f32 = 18.0;
/// `text-lg`'s own line, which is [`theme_gen`]'s `(18.0, 28.0)` for that class --
/// the name is set on twenty-eight rows and not on its own eighteen, which is what
/// `leading-normal` is.
const COLLECTION_NAME_LINE: f32 = 28.0;
/// `gap-2` between the name and the `LibraryIcon` line under it.
const COLLECTION_NAME_GAP: f32 = 8.0;
/// The body size, which is what every other line of a collection card is.
///
/// `layout.vue:276-316` is the whole of it: the name's `h2` carries `text-lg` on
/// `:277`, and *nothing else on the card names a size at all* -- the
/// `LibraryIcon` line is `<div class="flex items-center gap-1">` (`:281`), the
/// description is `<div class="grow text-primary">` (`:287`) and the foot is
/// `<div class="mt-auto flex flex-wrap items-center gap-4">` (`:290`), and
/// `text-primary` is this preset's alias for `--color-text-primary`
/// (`tailwind-preset.ts:23`), a colour rather than a scale.
/// `ProjectCardList.vue` and the card's own wrapper div (`:268-273`) name none
/// either, so all four inherit the body size, which is [`HEADER_TEXT`] and the
/// same sixteen the reference's own header words measure.
const COLLECTION_LABEL: f32 = HEADER_TEXT;
/// The line those four lines are set on: the stylesheet's own `1.15` at sixteen --
/// [`HEADER_SUMMARY_LINE`]'s reading, for the same rule and the same number. The
/// card's rows are set by its own `h-full` track rather than by centring against
/// the avatar, so nothing here needs the CSS line and nothing here is measured.
const COLLECTION_LABEL_LINE: f32 = HEADER_SUMMARY_LINE;
/// The glyphs in the card's head and foot, which the reference asks for at its
/// default `size-4`.
const COLLECTION_ICON: f32 = ui::CONTROL_ICON;

/// The loaders `sortTagsForDisplay` puts ahead of every other loader.
///
/// `DEFAULT_MOD_LOADERS`, `DEFAULT_SHADER_LOADERS` and the two more that make
/// `DEFAULT_LOADER_NAMES` (`tag-messages.ts:576-582`) -- which is why a card
/// whose loaders are `datapack, fabric, forge, neoforge, quilt` draws *Fabric*
/// and *Forge* before the *Data Pack*, and counts the other three in its `+N`.
const DEFAULT_LOADERS: [&str; 6] = ["fabric", "forge", "neoforge", "iris", "optifine", "vanilla"];

/// The header's own action.
///
/// `UserPageHeader.vue`'s `#actions` slot carries *Edit* -- but only when
/// `isSelf`, which is decided by comparing the profile's id against a signed-in
/// Modrinth account's -- and then always a `TeleportOverflowMenu` at `size="xl"`,
/// the 48-pixel quiet plate with three vertical dots. So the control on a profile
/// this launcher can draw is the overflow, and there is no refresh button: the
/// reference has none, and the one this page used to draw said it reloaded a page
/// that reloads itself.
const MORE_KEY: &str = "user:more";

/// A card's *Install*, whose identity is the project rather than the row.
///
/// Namespaced per project with [`ui::scoped`], because a card's own hover and its
/// button's hover are two controls: one key for both would light a button the
/// pointer has not reached.
const INSTALL_KEY: &str = "user:install";

/// What the overflow holds here.
///
/// The reference's menu is *Manage projects* (self only), a rule, *Report*,
/// *Block*, *Copy ID* and *Copy permalink*, then the staff actions. Two of the six
/// need a Modrinth session this launcher does not hold, one is the reader's own
/// dashboard, and the two copy actions want a clipboard the page has no route to.
/// The button is here because the reference draws one and its plate is 48 pixels
/// across; the press says what is behind it rather than drawing a menu of things
/// that would not work.
const MORE_NOTICE: &str =
    "This profile's menu holds Report, Block, Copy ID and Copy permalink, which need a \
     Modrinth account this launcher does not have.";

/// The strip's *All* tab, and one name per project type in
/// [`ProjectType::PROFILE_ORDER`]' order.
///
/// Not derived from the label: a label is the locale's and may be retranslated,
/// and a tab that changed its key on a language change would be a tab whose hover
/// tween is forgotten mid-flight.
const ALL_TAB_KEY: &str = "user:tab:all";
const TYPE_TAB_KEYS: [&str; 7] = [
    "user:tab:mod",
    "user:tab:resourcepack",
    "user:tab:datapack",
    "user:tab:shader",
    "user:tab:modpack",
    "user:tab:plugin",
    "user:tab:server",
];

/// The strip's *Collections* tab, which is not in [`TYPE_TAB_KEYS`] because it is
/// not a project type: `PROJECT_TYPE_ORDER` puts `collection` last and
/// `catalogProjectTypes` never returns it, so it is appended after the types rather
/// than sorted among them.
///
/// Its own name rather than an index into a shared list, for [`ALL_TAB_KEY`]'s
/// reason.
const COLLECTIONS_TAB_KEY: &str = "user:tab:collection";

/// One user's profile: their own document, the projects they own, their
/// collections, and their avatar.
///
/// Assembled from four answers rather than one, which is why it is a type of its
/// own: the reference reads the same four (`useQuery` for the user, for their
/// projects, for their collections, and an `img` for the avatar) and the page draws
/// the union.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Profile {
    /// The account's own document: the name, the handle, the bio, the join date.
    pub user: ModrinthUser,
    /// The projects they own, in the order the service lists them.
    pub projects: Vec<ModrinthUserProject>,
    /// The same projects from the **v3** document, read for one field.
    ///
    /// The type array and the id it belongs to, which is why it joins the v2 list by
    /// id rather than by position: v3 orders its answer differently, and a type
    /// counted against the wrong project would put a tab on the strip that filters
    /// to nothing. See [`Profile::type_of`].
    ///
    /// Empty when the v3 read failed, which is a degradation rather than a hole:
    /// every project then falls back to the v2 string.
    pub projects_v3: Vec<ModrinthV3Project>,
    /// The collections they own, as `GET /v3/user/{id}/collections` writes them.
    ///
    /// The fourth document the page is drawn from, and the one that decides whether
    /// the strip's last tab exists at all: `layout.vue:765` appends `'collection'`
    /// exactly when this list is not empty. Public and anonymous -- the route answers
    /// 200 for an account nobody has signed in as -- so this is a read like the
    /// other three rather than one that needs a session.
    ///
    /// Empty when the read failed, and empty is the reference's own answer for an
    /// account with no collections: in both cases the tab is not drawn.
    pub collections: Vec<ModrinthCollection>,
    /// Their avatar, when it could be fetched and decoded -- already rounded into
    /// the circle `UserPageHeader.vue` asks for, by [`crate::avatar`].
    pub avatar: Option<crate::avatar::Icon>,
    /// Each project's own icon, as it arrived: the URL it answers and the picture.
    ///
    /// Keyed by URL rather than by position because that is the only key a project
    /// document carries, and fetched by [`crate::store::Store::project_icons`] on
    /// the frame thread's behalf -- a PNG decode per card per frame is the work
    /// [`crate::avatar`]'s module docs warn about.
    pub icons: Vec<crate::avatar::Fetched>,
    /// Why there is no avatar, when there is none.
    pub note: Option<String>,
}

impl Profile {
    /// Assemble a profile from the service's answers.
    ///
    /// The avatar's failure is deliberately *not* this call's failure, for
    /// [`crate::skin::Appearance::of`]'s reason read against a smaller thing: a
    /// reader on a machine with no connection still has a profile -- a name, a bio,
    /// a list of projects -- and a page that threw those away because a picture
    /// would not come back would be showing less than it knows. The reference's own
    /// avatar does the same thing quietly, falling back to its placeholder, and the
    /// sentence this keeps is drawn where the picture would be.
    ///
    /// `projects_v3` is the half that can be missing without anything being drawn
    /// wrong about it -- see [`Profile::type_of`]. So is `collections`: an account
    /// with none, and a collections read that failed, draw the same page.
    pub fn of(
        user: ModrinthUser,
        projects: Vec<ModrinthUserProject>,
        projects_v3: Vec<ModrinthV3Project>,
        collections: Vec<ModrinthCollection>,
        avatar: Result<Vec<u8>, String>,
        icons: Vec<crate::avatar::Fetched>,
    ) -> Profile {
        let (avatar, note) = match avatar {
            Ok(picture) => match crate::avatar::Icon::circle(&picture, AVATAR as u32) {
                Some(avatar) => (Some(avatar), None),
                None => (
                    None,
                    Some("The avatar is not an image this launcher can draw.".to_string()),
                ),
            },
            Err(reason) => (None, Some(reason)),
        };
        Profile { user, projects, projects_v3, collections, avatar, note, icons }
    }

    /// Whether the strip ends in a *Collections* tab.
    ///
    /// `layout.vue:763-766`: `catalogProjectTypes(projects)`, then
    /// `if (collections.value.length > 0) types.push('collection')`. The condition is
    /// on the *list*, not on the account: a tab for a list that is empty is a tab
    /// whose page can only ever be the empty state, and the reference does not offer
    /// it.
    pub fn has_collections(&self) -> bool {
        !self.collections.is_empty()
    }

    /// The collections, in the order the reference draws them in.
    ///
    /// `sortedCollections` (`layout.vue:751`) is `updated` descending with `created`
    /// descending as the tie-break, which is what [`ModrinthCollection::sort_key`]
    /// reads. `sort_by` is stable, so a pair that ties in both halves keeps the
    /// service's own order rather than an invented one.
    pub fn sorted_collections(&self) -> Vec<&ModrinthCollection> {
        let mut sorted: Vec<&ModrinthCollection> = self.collections.iter().collect();
        sorted.sort_by(|first, second| second.sort_key().cmp(&first.sort_key()));
        sorted
    }

    /// One collection's icon, when it arrived.
    ///
    /// `None` for a collection with no `icon_url`, which is what every collection
    /// measured on the live service answers -- `null` for all four of FlameFire's --
    /// and the reference's own `Avatar` draws its placeholder there rather than
    /// nothing. Fetched by [`crate::store::Store::project_icons`] beside the
    /// projects' own and keyed by the same URL.
    pub fn collection_icon(&self, collection: &ModrinthCollection) -> Option<&crate::avatar::Icon> {
        if collection.icon_url.is_empty() {
            return None;
        }
        self.icons
            .iter()
            .find(|fetched| fetched.url == collection.icon_url)
            .map(|fetched| &fetched.icon)
    }

    /// Everything this user's projects have been downloaded.
    ///
    /// The reference's own `sumDownloads`: a `reduce` over the list it already has,
    /// because the API publishes no such field.
    pub fn downloads(&self) -> u64 {
        self.projects.iter().map(|project| project.downloads).sum()
    }

    /// The summary line under the name.
    ///
    /// Their own sentence about themselves, or the reference's two fallbacks:
    /// *"A Modrinth creator."* once they have published something, *"A Modrinth
    /// user."* until then. The pair is `profile.bio.fallback.*`, and the choice
    /// between them is the reference's own reading of an empty bio.
    pub fn summary(&self) -> &str {
        if !self.user.bio.trim().is_empty() {
            return self.user.bio.as_str();
        }
        if self.projects.is_empty() {
            Key::ProfileBioFallbackUser.message()
        } else {
            Key::ProfileBioFallbackCreator.message()
        }
    }

    /// The project types this user actually has, in the order the reference's strip
    /// sorts them into.
    ///
    /// A type with no projects is not a filter: the reference builds its strip from
    /// the catalogue of the projects it has (`catalogProjectTypes(projects)`), so an
    /// unused tab is never offered. The order is not [`ProjectType::TABS`]:
    /// `PROJECT_TYPE_ORDER` in `ui/src/utils/project-types.ts` puts mods first and
    /// modpacks fifth, where Discover's tabs put modpacks first.
    ///
    /// A project whose type this tree cannot name is counted by the header and drawn
    /// under *All*, and no filter claims it -- which is what the reference does with a
    /// type its own order does not list (`getProjectSortIndex` returns
    /// `PROJECT_TYPE_ORDER.length` for one it does not know, and `catalogProjectTypes`
    /// deletes `'project'` outright).
    pub fn types(&self) -> Vec<ProjectType> {
        ProjectType::PROFILE_ORDER
            .iter()
            .copied()
            .filter(|kind| self.projects.iter().any(|project| self.type_of(project) == Some(*kind)))
            .collect()
    }

    /// The one type the reference calls this project's own.
    ///
    /// `getPrimaryProjectType` reads the head of v3's `project_types`, so that is what
    /// this reads first, and the v2 string is the fallback rather than a second
    /// opinion. The order matters because the two disagree on real accounts: v2 calls
    /// four of FlameFire's six projects `mod` where v3 calls them
    /// `["datapack", "mod"]`, and v2 calls apace's *Origins-Paper* a `mod` where v3
    /// calls it a `["plugin"]`. A strip counted from the v2 string is missing *Data
    /// Packs* and *Plugins* where the reference draws them.
    ///
    /// The fallback is what a profile whose v3 read failed is left with, and it is
    /// chosen over failing: a machine that could not reach one more document still
    /// has the strip it had, which is wrong for a project of two types rather than
    /// absent. `None` when neither document names a type this tree knows, which is
    /// the reference's own `'project'` answer: counted by the header, claimed by no
    /// tab.
    pub fn type_of(&self, project: &ModrinthUserProject) -> Option<ProjectType> {
        self.projects_v3
            .iter()
            .find(|entry| entry.id == project.id)
            .and_then(ModrinthV3Project::primary_type)
            .and_then(ProjectType::from_token)
            .or_else(|| ProjectType::from_token(&project.project_type))
    }


    /// One project's icon, when it arrived.
    ///
    /// `None` is the reference's own case rather than a hole: `Avatar.vue` draws a
    /// placeholder there, and [`crate::ui::icon_box`]'s empty box is the same box,
    /// its same background and its same hairline without the hexagon.
    pub fn icon(&self, project: &ModrinthUserProject) -> Option<&crate::avatar::Icon> {
        // A project with no `icon_url` matches nothing, including another project
        // with no `icon_url`: the empty string is not an address, and a list where
        // every unanswered project drew the first answer's picture would be worse
        // than no pictures at all.
        if project.icon_url.is_empty() {
            return None;
        }
        self.icons
            .iter()
            .find(|fetched| fetched.url == project.icon_url)
            .map(|fetched| &fetched.icon)
    }

    /// The projects one filter shows.
    ///
    /// `None` is the strip's *All* tab, which is the reference's own reading of an
    /// address with no type in it. The filter is compared against the project's own
    /// type, as the reference's `filterProjectsByType` does -- against
    /// [`Self::type_of`], which is the same `getPrimaryProjectType` that function
    /// calls, so a tab and the list under it cannot disagree about what a project is.
    ///
    /// The order is the reference's too, and it is not the service's:
    /// `layout.vue:746` is `filterProjectsByType(...).slice().sort(projectUserSorting)`,
    /// and `user-profile/utils.ts:33` orders by a `status` priority first and by
    /// `downloads` descending within a priority. This tree's
    /// `ModrinthUserProject` does not read `status` -- that field lives in
    /// `palantir-net`, which this pass does not own -- so the priority table cannot
    /// be evaluated and the comparator collapses to its `downloads` branch, which is
    /// what every project on *somebody else's* profile is: `/v2/user/{id}/projects`
    /// answers an unlisted or private project only to its owner. What is left is
    /// still the reference's rule rather than the service's, and it matters: the API
    /// returns the list ordered by id, so without it the first card was whichever
    /// project happened to sort first in the alphabet -- measured as *Random Island*
    /// where the reference draws *Zombie Invade 100 Days*, the account's largest.
    pub fn shown(&self, filter: Option<ProjectType>) -> Vec<&ModrinthUserProject> {
        let mut shown: Vec<&ModrinthUserProject> = self
            .projects
            .iter()
            .filter(|project| match filter {
                Some(kind) => self.type_of(project) == Some(kind),
                None => true,
            })
            .collect();
        // `getProjectSortValue(second, ..) - getProjectSortValue(first, ..)`: the
        // larger count first. The sort is stable, so a tie keeps the service's own
        // order rather than inventing one.
        shown.sort_by_key(|project| std::cmp::Reverse(project.downloads));
        shown
    }
}

/// The request the page makes: one user's profile.
///
/// The *name* travels, not an id: a reader arrives from an address that spells a
/// name, and the published API takes either. Which document the projects are then
/// asked for by is the id inside the answer, which the service is the authority
/// on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    /// Whose profile, as the address spells it.
    pub user: String,
    /// Which request this is, counting from one.
    pub round: u64,
}

/// Which of the strip's tabs is on screen.
///
/// Not `Option<ProjectType>`, because the strip has a fourth tab that is not a
/// project type: `layout.vue:765` pushes the string `'collection'` into the same
/// list the types go into, and `parseProjectTypeRouteParam` reads it back the same
/// way. `ProjectType` has no variant for it and cannot grow one from this page --
/// `route.rs` owns that enum, and `/user/{name}/collections` is deliberately not an
/// address here ([`ProjectType::from_profile_token`] refuses the token) -- so the
/// branch is carried by the page instead.
///
/// The two project-type arms keep their navigation: [`Message::Filter`] is turned by
/// [`crate::pages::Screen`] into an address, and the address comes back as
/// [`State::filter`]. This arm has no address to go out on, so
/// [`Message::Collections`] is applied here and stays here. That is the one place
/// this page keeps state the address does not record, and it is a gap in the routing
/// rather than a choice: leaving the tab and coming back by address lands on *All*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    /// No type in the address: the strip's *All* tab.
    #[default]
    All,
    /// One project type, which is what the address's third segment names.
    Type(ProjectType),
    /// The strip's *Collections* tab, and the reference's separate branch.
    Collections,
}

impl Filter {
    /// The tab an address's project type selects.
    ///
    /// `None` is *All*, which is `parseProjectTypeRouteParam`'s own reading of an
    /// address with nothing in its third segment.
    pub fn of(project_type: Option<ProjectType>) -> Filter {
        match project_type {
            Some(kind) => Filter::Type(kind),
            None => Filter::All,
        }
    }

    /// Whether the project list is on screen under this tab.
    ///
    /// `layout.vue:220` is `v-if="selectedProjectType !== 'collection'"`, so the
    /// Collections tab replaces the project list rather than joining it.
    pub fn shows_projects(self) -> bool {
        self != Filter::Collections
    }

    /// The project type the project list is filtered by.
    ///
    /// `None` for *All* and for *Collections*, which is what `filterProjectsByType`
    /// returns for the second: it compares `getPrimaryProjectType(project)` against
    /// the address's type, and no project of any type answers `'collection'`. The
    /// list is not drawn at all under that tab, so the empty answer is never used.
    pub fn project_type(self) -> Option<ProjectType> {
        match self {
            Filter::Type(kind) => Some(kind),
            Filter::All | Filter::Collections => None,
        }
    }
}

/// What the page can be told.
#[derive(Debug, Clone)]
pub enum Message {
    /// The shell's answer to a request: which request it answers, and what arrived.
    ///
    /// Boxed at the crossing, for the reason `project::Message::Found` is: this is
    /// the one variant that is a page's worth of data beside a handful of unit ones,
    /// and boxing it here keeps the page's own arms from boxing anything.
    Found {
        /// Which request this answers, so an answer to a question the page has
        /// replaced is dropped rather than drawn.
        round: u64,
        /// The profile, or why it could not be read.
        result: Result<Box<Profile>, String>,
    },
    /// A project in the list was pressed.
    ///
    /// Reported rather than applied, like the library's cards: opening a project is
    /// a change of address, and only the shell owns the history.
    Project(String),
    /// One of the strip's filters was chosen.
    ///
    /// The same report as [`Message::Project`], because in the reference the strip
    /// *is* links -- `NavTabs` renders one `href` per type -- so choosing one is a
    /// navigation rather than a change of state, and the address stays the single
    /// record of what is on screen. [`State::filter`] is where the page then follows
    /// the address.
    Filter(Option<ProjectType>),
    /// The strip's *Collections* tab was chosen.
    ///
    /// Applied here rather than reported, which is the one place this page does not
    /// do what [`Message::Filter`] does, and it is forced by the routing rather than
    /// chosen: `Route::User` carries an `Option<ProjectType>`, and a collection is
    /// not a project type, so there is no address for this tab to be. See
    /// [`Filter`].
    Collections,
    /// A filter chosen while the page is on the collections branch.
    ///
    /// [`Message::Filter`] cannot leave that branch by itself, and this is why:
    /// that message becomes an `Open::User`, the shell turns it into an address, and
    /// the shell's `go` returns early when that address is the one the page is
    /// already at. Choosing *All* from the collections branch is exactly that case --
    /// the collections branch has no address of its own, so the page is still at
    /// `/user/{name}` -- and the press would have left the reader on the branch they
    /// were trying to leave. So this arm is applied here, and the address moves only
    /// when it can.
    LeaveCollections(Option<ProjectType>),
    /// The header's overflow was pressed.
    More,
    /// A card's *Install* was pressed.
    ///
    /// Reported rather than performed, like the project page's own button: which
    /// instances exist, which version fits one and where its folder is are all the
    /// shell's, and [`crate::pages::Ask::Install`] is the ask that already says so.
    Install(String, String, bool),
    /// The pointer entered or left one of the page's controls, for the clock that
    /// carries a hover's 150 ms (see [`crate::ui`]).
    Hover {
        /// The control's stable name, one per control.
        key: &'static str,
        /// Whether the pointer arrived or left.
        over: bool,
        /// The hover end, where the control declares one of its own.
        hover: Option<f32>,
    },

    /// A wheel over this page's scroll region.
    ///
    /// Reported rather than applied: iced moves a scrollable with a `scroll_to`
    /// command, so which region glides, and how far, is the shell's -- see
    /// `crate::scroll`. This page's part is to hand the wheel on, and the name it
    /// carries is the region the widget was built with.
    Wheel(&'static str, crate::scroll::Wheel),
}

crate::hovered!(Message);

/// The page's own state.
#[derive(Debug, Clone)]
pub struct State {
    /// Whose profile this is.
    ///
    /// The page keeps the name rather than being handed it at drawing time, because
    /// it is what the page *asks* by: `update` and `opening` see no address, and a
    /// page that could not name the user it is about could not ask for it.
    pub user: String,
    /// The project type the address names, which the list is filtered by.
    ///
    /// The address's own field and left as it was, because the two project-type arms
    /// of the strip are navigations and this is what they navigate *by*:
    /// [`Message::Filter`] becomes an `Open::User`, and the address that comes back
    /// is written here by [`Self::filter`] and read back by [`Self::selected`]. It
    /// cannot carry the strip's fourth tab -- see [`Self::collections`] and
    /// [`Filter`].
    pub project_type: Option<ProjectType>,
    /// Whether the strip's *Collections* tab is the one on screen.
    ///
    /// Page state rather than address state, and the only thing on this page that
    /// is: `Route::User` carries an `Option<ProjectType>` and a collection is not a
    /// project type, so there is no `/user/{name}/collections` for this tab to be --
    /// [`ProjectType::from_profile_token`] refuses the token on purpose. The
    /// consequence is that leaving the tab and coming back by address lands on
    /// *All*, which is what the address says.
    ///
    /// [`Self::filter`] clears it whenever the address is followed, so the two can
    /// never disagree about which tab is drawn.
    pub collections: bool,
    /// The profile, as the service answered it. A `Load`, because the page is drawn
    /// before the answer arrives and its four arms are what says so.
    pub profile: Load<Profile>,
    /// The last thing the page could not do, shown rather than swallowed.
    pub notice: Option<String>,
    /// Which request the page is waiting for, so an answer to a question it has
    /// replaced is dropped rather than drawn.
    round: u64,
}

impl State {
    /// A page for one user, filtered by whatever the address asked for.
    pub fn new(user: String, project_type: Option<ProjectType>) -> State {
        State {
            user,
            project_type,
            collections: false,
            profile: Load::Idle,
            notice: None,
            round: 0,
        }
    }

    /// Which tab the strip is on, as the one value both arms can be read through.
    ///
    /// Named apart from [`Self::filter`], which is the one that *follows an address*:
    /// this one reads what the page is holding, that one changes it.
    pub fn selected(&self) -> Filter {
        if self.collections {
            Filter::Collections
        } else {
            Filter::of(self.project_type)
        }
    }

    /// Apply a message, and answer with what the shell has to do about it.
    pub fn update(&mut self, message: Message) -> Option<Asked> {
        match message {
            // A wheel is not this page's to apply: see `crate::scroll`.
            Message::Wheel(..) => {},

            Message::Found { round, result } => {
                if round == self.round {
                    self.profile = match result {
                        Ok(profile) => Load::Ready(*profile),
                        Err(reason) => Load::Failed(reason),
                    };
                }
            }
            // Neither of these is this page's to perform: `pages::Screen::update`
            // takes both before they reach here, turning a project into
            // `Open::Project` and a filter into `Open::User` -- which comes back as
            // [`State::filter`] once the address has moved. Reaching this arm
            // directly means the shell did not route them, and the answer to that is
            // to change nothing rather than to invent a navigation.
            Message::Project(_) | Message::Filter(_) => {}
            // The Collections tab, applied here: see [`Message::Collections`]. There
            // is no address for it, so this is the whole of the message.
            Message::Collections => self.collections = true,
            // Leaving the branch for one of the two arms the address *can* say. The
            // address moves by itself when it differs from where the page already is
            // -- a named type is a different path -- and *All* is the arm that cannot,
            // which is what [`Message::LeaveCollections`] is for.
            Message::LeaveCollections(project_type) => self.filter(project_type),
            // A card's own button, reported rather than performed. `pages::mod`
            // turns it into the same `Ask::Install` the project page's button
            // makes, so this launcher installs a mod from a profile exactly as it
            // does from a project.
            Message::Install(..) => {}
            // The overflow's press: the reference's menu is drawn by
            // `TeleportOverflowMenu`, which needs a popover this page has no route
            // to, so what the reader gets is the sentence naming what is behind it.
            Message::More => self.notice = Some(MORE_NOTICE.to_string()),
            Message::Hover { key, over, hover } => crate::ui::pointer_with(
                key,
                over,
                hover.unwrap_or_else(crate::theme::hover_brightness),
            ),
        }
        None
    }

    /// Follow the address to another filter, keeping the profile that is already
    /// loaded.
    ///
    /// The strip's own tabs go through here rather than through a request: which
    /// projects are on screen changed, the list they come from did not, and asking
    /// the service again for an answer the page is holding would be a round trip
    /// that redraws the same page. A *different* user is not this: that is a new
    /// page, and [`crate::pages::Screen::retarget`] builds one.
    ///
    /// An address that names no type is the strip's *All* tab, so it clears the
    /// collections branch as well: arriving at `/user/x` from `/user/x/collections`
    /// lands on *All*, which is what the address says.
    pub fn filter(&mut self, project_type: Option<ProjectType>) {
        self.project_type = project_type;
        self.collections = false;
    }

    /// The request the page owes because nothing has been asked for yet.
    ///
    /// The reference reads the profile as the page mounts; this is the same rule
    /// stated where the shell can see it, so a window opened straight on a profile
    /// draws it rather than an empty header. A page that already has an answer owes
    /// nothing, which is what makes the strip's navigation free.
    pub fn opening(&mut self) -> Option<Asked> {
        if self.profile == Load::Idle {
            Some(self.ask())
        } else {
            None
        }
    }

    /// Bump the round, mark the page as waiting, and describe the request.
    fn ask(&mut self) -> Asked {
        self.round += 1;
        self.profile = Load::Loading;
        Asked { user: self.user.clone(), round: self.round }
    }

    /// The sentence to draw for a user with no projects.
    ///
    /// The reference has one sentence for *somebody else has none* and one for *you
    /// have none*, and the difference is decided by comparing the profile's id
    /// against the signed-in Modrinth account's. This launcher does not hold a
    /// Modrinth credential -- G118, a decision rather than a gap -- so the
    /// reader's-own arm stays unreachable and the page draws the other one. Both
    /// are the reference's copy, and the comparison it is missing is the whole of
    /// what would select between them; the arm is kept rather than deleted so that
    /// the page is still the reference's page if that decision is ever reversed.
    ///
    /// The G105 citation that used to sit here was wrong and has been dropped rather
    /// than narrowed: it said the credential was needed to *reach* the reference's
    /// v3 user service, and the two v3 routes this page reads answer 200 with none
    /// (see the module docs). A credential is still wanted -- for the reader's own
    /// arm of this sentence -- so the sentence above stands on G118 alone.
    pub fn empty_sentence(own_profile: bool) -> &'static str {
        if own_profile {
            Key::ProfileLabelNoProjectsAuthDescription.message()
        } else {
            Key::ProfileLabelNoProjects.message()
        }
    }

    /// The heading to draw for a user with no collections.
    ///
    /// The same pair of arms as [`Self::empty_sentence`] and the same reason they
    /// are one arm here and one there: `showCollectionsEmptyState`'s `isSelf` is
    /// false, so `profile.label.no-collections-auth-description` is not drawn.
    ///
    /// The reference's `EmptyState` for this one also carries a *Create a collection*
    /// button, and that is the one control in this branch this page does not draw.
    /// It is behind the same `isSelf` as the description, so it is not reached for
    /// the same reason -- and were it ever reached it would be a *write* to Modrinth,
    /// which is what [`crate::store::needs_account`] is for rather than a button that
    /// silently does nothing.
    pub fn no_collections_sentence(own_profile: bool) -> &'static str {
        if own_profile {
            Key::ProfileLabelNoCollectionsAuthDescription.message()
        } else {
            Key::ProfileLabelNoCollections.message()
        }
    }
}

/// Draw the page.
pub fn view<'a>(theme: Gen, state: &'a State, _store: &'a Store) -> Element<'a, Message> {
    let mut blocks: Vec<Element<'a, Message>> = Vec::new();
    if let Some(notice) = &state.notice {
        blocks.push(ui::admonition(
            theme,
            ui::Severity::Info,
            Key::AppNavViewProfile.message(),
            notice,
        ));
    }
    // Why there is no picture, when the fetch failed: the reference has no line for
    // it -- `Avatar.vue` falls back to its own placeholder, quietly -- so the
    // sentence goes where this launcher's other failures go rather than under the
    // name it belongs to.
    if let Some(note) = state.profile.ready().and_then(|profile| profile.note.as_deref()) {
        blocks.push(ui::admonition(
            theme,
            ui::Severity::Info,
            Key::AppNavViewProfile.message(),
            note,
        ));
    }
    // The header and the list arrive together -- one request, one answer -- so they
    // are one block: a page that drew a header from one answer and a list from
    // another could show a name over somebody else's projects.
    blocks.push(page::draw(theme, &state.profile, "this profile", |profile| {
        loaded(theme, state, profile)
    }));
    page::body(blocks, GAP, Message::Wheel)
}

/// The header, the filter strip and the list, once there is a profile.
///
/// The header and the strip are one block rather than two because they are one
/// gap apart and the rest of the page is not: [`HEADER_STRIP_GAP`] between the
/// header's rule and the strip's own border, [`GAP`] everywhere below it, which
/// is `NormalPage`'s `gap-y-4` and `NavTabs`' `mb-1` against the list.
fn loaded<'a>(theme: Gen, state: &'a State, profile: &'a Profile) -> Element<'a, Message> {
    let mut head = column![].spacing(HEADER_STRIP_GAP).width(Length::Fill);
    head = head.push(header(theme, profile));
    if let Some(strip) = filter_strip(theme, state, profile) {
        head = head.push(strip);
    }
    let mut blocks: Vec<Element<'a, Message>> = vec![head.into()];
    // The project list and the collection grid are two branches of the reference's
    // one `<div class="flex flex-col gap-3">`, and which of them is on screen is
    // `selectedProjectType`: `layout.vue:220` draws `ProjectList` only when it is
    // not `'collection'`, and `layout.vue:257` draws the collection `ProjectCardList`
    // when it *is* `'collection'` or nothing at all. So the Collections tab replaces
    // the project list, and the *All* tab carries both with the collections after the
    // projects -- which is where the reference puts them, and why they sit below the
    // fold of the 720-pixel window this page is measured at.
    let filter = state.selected();
    let shown = profile.shown(filter.project_type());
    let collections = collection_grid(theme, profile);
    if filter.shows_projects() {
        if shown.is_empty() {
            // The reference's own empty state, and the same sentence for a user with
            // no projects at all and for one with none of the type being looked at --
            // the reference draws `profile.label.no-projects` for both. The condition
            // is `showProjectsEmptyState`, all three of its terms.
            if shows_projects_empty_state(filter, profile.has_collections(), shown.len()) {
                blocks.push(ui::card(
                    theme,
                    text(State::empty_sentence(false))
                        .size(EMPTY_HEADING)
                        .line_height(iced::Pixels(EMPTY_HEADING_LINE))
                        // `font-semibold`, and not the 800 [`heading`] is for a
                        // real heading: `EmptyState.vue:7` is
                        // `<span class="text-2xl font-semibold text-contrast">`.
                        .font(semibold())
                        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
                ));
            }
        } else {
            let mut list = column![].spacing(LIST_GAP).width(Length::Fill);
            for project in shown {
                list = list.push(project_row(theme, profile, project));
            }
            blocks.push(list.into());
        }
    }
    // `showCollectionsEmptyState` (`layout.vue:833`): the collections tab, and
    // nothing to put under it. The heading alone -- `isSelf` is false without a
    // Modrinth session, so the reference draws no description and no *Create a
    // collection* button either. See [`State::no_collections_sentence`].
    if filter == Filter::Collections && !profile.has_collections() {
        blocks.push(ui::card(
            theme,
            text(State::no_collections_sentence(false))
                .size(EMPTY_HEADING)
                .line_height(iced::Pixels(EMPTY_HEADING_LINE))
                // The same `font-semibold` as the sentence above, for the same
                // `EmptyState.vue:7`.
                .font(semibold())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
        ));
    } else if let Some(grid) = collections {
        blocks.push(grid);
    }
    let mut body = column![].spacing(GAP).width(Length::Fill);
    for block in blocks {
        body = body.push(block);
    }
    body.into()
}

/// The profile's header: who they are, and what they have published.
///
/// `page-header/index.vue` at its defaults -- `divider` and `bottom-padding` both
/// on -- which is a rule under the whole block and 16 pixels of room below it, and
/// *no card*: the reference's header is a bare row on the page's own surface, where
/// this page used to draw one inside a `ui::card`.
///
/// The counts are over *every* project the user owns, not the filtered list, which
/// is what the reference passes its own header (`:projects-count="projects.length"`,
/// `:downloads="sumDownloads"`): a filter chooses what is listed, not what the
/// account is.
fn header<'a>(theme: Gen, profile: &'a Profile) -> Element<'a, Message> {
    // The picture slot, which is the same size whether or not there is a picture in
    // it: a header that grew when an avatar arrived would move the name under the
    // pointer that was about to press something. Drawn round, because
    // `UserPageHeader.vue` asks for `circle`.
    let picture: Element<'a, Message> = match &profile.avatar {
        // The bytes are already the circle: `crate::avatar::Icon::circle` cleared
        // the alpha outside it, which is the one thing a `container`'s border radius
        // cannot do -- it paints the container's own background and leaves what is
        // drawn over it square.
        Some(avatar) => iced::widget::image(avatar.handle())
            .width(Length::Fixed(AVATAR))
            .height(Length::Fixed(AVATAR))
            .into(),
        // The reference's own fallback is a circle tinted by the username with the
        // first letter in it; this launcher has no letter-in-a-circle drawing, so
        // the placeholder is the generic account glyph at the same size.
        None => page::glyph(theme, Glyph::CircleUser, AVATAR),
    };
    let block = row![]
        .width(Length::Fill)
        .spacing(HEADER_GAP)
        .align_items(Alignment::Start)
        .push(
            row![]
                .width(Length::Fill)
                .spacing(HEADER_GAP)
                .align_items(Alignment::Center)
                .push(picture)
                .push(
                    column![]
                        .width(Length::Fill)
                        .spacing(HEADER_GROUP_GAP)
                        .push(
                            column![]
                                .spacing(TITLE_GAP)
                                // `text-2xl font-semibold leading-none`: the
                                // username, once. The reference's title is
                                // `user.username` and nothing else -- the display
                                // name and an `@handle` under it are not on this
                                // page at all.
                                .push(
                                    text(profile.user.username.clone())
                                        .size(TITLE)
                                        .line_height(iced::Pixels(TITLE))
                                        .font(semibold())
                                        .style(iced::theme::Text::Color(theme_gen::ink(
                                            theme,
                                            INK_CONTRAST,
                                        ))),
                                )
                                // `index.vue:20` puts no colour on the summary, so
                                // it inherits the body ink: `--color-text-primary`.
                                // The metadata row below it *does* say
                                // `text-secondary`, and the two measured #B0BAC5 and
                                // #96A2B0 respectively -- which is the pair the audit
                                // caught them swapped by.
                                .push(
                                    text(profile.summary().to_string())
                                        .size(HEADER_TEXT)
                                        .line_height(iced::Pixels(HEADER_SUMMARY_LINE))
                                        .font(medium())
                                        .style(iced::theme::Text::Color(theme_gen::ink(
                                            theme,
                                            INK_DEFAULT,
                                        ))),
                                ),
                        )
                        .push(metadata_row(theme, profile)),
                ),
        )
        // `PageHeaderActions` is `flex flex-wrap items-center gap-2`, and on a
        // profile this launcher can draw it holds the one control: the quiet
        // `xl` overflow, 48 pixels square.
        .push(ui::icon_button_sized(
            theme,
            MORE_KEY,
            Glyph::EllipsisVertical,
            ui::Kind::Quiet,
            ui::Size::Xl,
            Message::More,
        ));
    // The rule is its own element rather than the container's border.
    //
    // `page-header/index.vue:60` is `border-0 border-b border-solid`, and iced's
    // `Border` is symmetric: giving this container `width: 1.0` drew three rules
    // the reference has none of -- 865 pixels across the top at y=73, 110 down
    // each side, and the real one along the bottom -- 1085 spurious pixels against
    // the reference's 868. A `Space` one pixel tall paints the one rule, and it
    // lands *below* the `pb-4` because CSS puts the border outside the padding.
    let rule = container(Space::with_width(Length::Fill).height(Length::Fixed(1.0)))
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(move |_theme: &iced::Theme| iced::widget::container::Appearance {
            background: Some(iced::Background::Color(theme_gen::ink(
                theme,
                theme_gen::Ink::Divider,
            ))),
            ..iced::widget::container::Appearance::default()
        });
    column![]
        .width(Length::Fill)
        .push(block)
        .push(Space::with_height(Length::Fixed(HEADER_PAD_BOTTOM)))
        .push(rule)
        .into()
}

/// The header's metadata row: three facts, and a bullet between each pair.
///
/// `page-header/metadata/index.vue` is `gap-x-[1.625rem] gap-y-2`, and every item
/// but the first carries a `BulletDivider` in a `absolute right-full w-[1.625rem]`
/// span -- so the 26 pixels *are* the divider's slot, not a gap with a dot in it.
/// Measured at the reference's own 1280x720: the first item's words end at x=305,
/// the dot is x=316..321, the next item's icon starts at x=332.
fn metadata_row<'a>(theme: Gen, profile: &'a Profile) -> Element<'a, Message> {
    let now = now_millis();
    let facts: [(Glyph, String); 3] = [
        (
            Glyph::Box,
            // `PageHeaderMetadataNumberItem` puts the value and the label in an
            // `items-baseline gap-1` pair, and formats the value compactly.
            format!(
                "{} {}",
                compact(profile.projects.len() as u64, 1),
                text_gen::profile_label_project_count(profile.projects.len() as u64)
            ),
        ),
        (
            Glyph::Download,
            format!(
                "{} {}",
                compact(profile.downloads(), 1),
                text_gen::profile_label_download_count(profile.downloads())
            ),
        ),
        (Glyph::Calendar, format!("{} {}", Key::ProfileLabelJoined.message(), how_ago(&profile.user.created, now))),
    ];
    // The 26 pixels are the divider's *slot*, not a gap the divider also sits in:
    // `page-header-metadata-item.vue` puts the divider in an `absolute right-full
    // w-[1.625rem]` span, which is the same box as the row's `gap-x`. So the slot
    // is one child -- the dot, centred, in 26 pixels -- and the row's own spacing
    // is zero. Spacing the row at 26 *and* pushing a 6-pixel dot gave 26+6+26, and
    // the audit measured the third item 65 pixels to the right of where the
    // reference draws it.
    let mut row = row![].spacing(0.0).align_items(Alignment::Center);
    for (index, (glyph, label)) in facts.into_iter().enumerate() {
        if index > 0 {
            // The divider is drawn *before* the item it belongs to, which is where
            // `right-full` puts it.
            row = row.push(slot(theme));
        }
        row = row.push(
            row![]
                .spacing(METADATA_TEXT_GAP)
                .align_items(Alignment::Center)
                .push(icon::icon(glyph, METADATA_ICON, theme_gen::ink(theme, INK_SECONDARY)))
                .push(
                    text(label)
                        .size(HEADER_TEXT)
                        .line_height(iced::Pixels(METADATA_LINE))
                        .font(medium())
                        .style(iced::theme::Text::Color(theme_gen::ink(
                            theme,
                            INK_SECONDARY,
                        ))),
                ),
        );
    }
    row.into()
}

/// The 26-pixel slot one item's divider occupies, with the dot centred in it.
///
/// `absolute right-full flex h-full w-[1.625rem] items-center justify-center` --
/// the span is as wide as the gap it replaces, and `justify-center` is the half of
/// that the port had been leaving out: a slot centred only on its vertical axis
/// put the dot against the item that follows it, measured 10 pixels left of where
/// the reference draws it (its own centre at x=308.5 against the reference's 319).
/// Measured at the reference's own 1280x720: the first item's words end at x=305,
/// the dot is x=316..321 and the next item's icon starts at x=332.
fn slot<'a, Message: 'a>(theme: Gen) -> Element<'a, Message> {
    container(bullet(theme))
        .width(Length::Fixed(METADATA_GAP))
        .center_x()
        .center_y()
        .into()
}

/// `BulletDivider`: a 6-pixel `--surface-5` dot.
fn bullet<'a, Message: 'a>(theme: Gen) -> Element<'a, Message> {
    container(Space::with_width(Length::Fixed(METADATA_DOT)).height(Length::Fixed(METADATA_DOT)))
        .width(Length::Fixed(METADATA_DOT))
        .height(Length::Fixed(METADATA_DOT))
        .style(move |_theme: &iced::Theme| iced::widget::container::Appearance {
            background: Some(iced::Background::Color(theme_gen::ink(
                theme,
                theme_gen::Ink::Surface5,
            ))),
            border: Border { radius: (METADATA_DOT / 2.0).into(), ..Default::default() },
            ..iced::widget::container::Appearance::default()
        })
        .into()
}





/// The strip of filters, or nothing when there is only one thing to filter by.
///
/// The reference draws no strip until it has more than two links -- *All* plus one
/// type is two -- because a lone filter beside the list it filters says nothing
/// (`NavTabs v-if="navLinks.length > 2"`). The count is over *links* and not over
/// project types, so a collection counts toward it: an account with one type and one
/// collection has three links and does get a strip.
///
/// The list is `layout.vue:770`'s `navLinks`: *All*, then one entry per type, then
/// the collections -- `layout.vue:765` appends `'collection'` to the sorted types
/// when `collections.value.length > 0`, and `sortProjectTypes` puts it last because
/// `PROJECT_TYPE_ORDER` ends with it. So the tab is appended rather than sorted in,
/// and it is absent for an account with none.
///
/// Each type's label is `getProjectTypeTitleMessage(projectType)` formatted with
/// `{ count: 2 }` -- the *capital* messages, plural -- which are the same seven words
/// as the category messages [`ProjectType::label`] carries. The collections label is
/// the exception and not `getProjectTypeTitleMessage`'s: `layout.vue:777` gives
/// `'collection'` `messages.collectionsLabel`, which is `project-type.collection.
/// plural` -- *Collections*, and not *Collection*.
fn filter_strip<'a>(
    theme: Gen,
    state: &State,
    profile: &Profile,
) -> Option<Element<'a, Message>> {
    let filter = state.selected();
    let types = profile.types();
    let collections = profile.has_collections();
    if 1 + types.len() + usize::from(collections) <= 2 {
        return None;
    }
    let mut keys: Vec<&'static str> = vec![ALL_TAB_KEY];
    let mut labels: Vec<(String, bool)> = vec![(
        Key::ProjectTypeAll.message().to_string(),
        filter == Filter::All,
    )];
    for kind in &types {
        // The keys and the order are one list, so the index of the type in
        // `PROFILE_ORDER` is the index of its name here -- and a type the order does
        // not know never reaches the strip at all, because `Profile::types` is built
        // from that same list.
        if let Some(index) = ProjectType::PROFILE_ORDER.iter().position(|known| known == kind) {
            keys.push(TYPE_TAB_KEYS[index]);
            labels.push((kind.label().to_string(), filter == Filter::Type(*kind)));
        }
    }
    if collections {
        keys.push(COLLECTIONS_TAB_KEY);
        labels.push((
            Key::ProjectTypeCollectionPlural.message().to_string(),
            filter == Filter::Collections,
        ));
    }
    // The two lists are the strip's own index space, so the index that comes back
    // from a press is read against both: the type tabs are the ones the address can
    // carry and report through `Message::Filter`, and the collections tab is the one
    // that cannot, so it reports through `Message::Collections` instead.
    //
    // And the arm the address cannot *leave* by is the reason
    // `Message::LeaveCollections` exists: on the collections branch the page is
    // still at `/user/{name}`, so `All`'s own address is where it already is and the
    // shell would treat the press as no navigation at all.
    let on_collections = filter == Filter::Collections;
    let typed = types.len();
    Some(ui::tabs(theme, &keys, &labels, move |index| {
        let chosen = match index {
            0 => None,
            n if n <= typed => types.get(n - 1).copied(),
            _ => return Message::Collections,
        };
        if on_collections {
            Message::LeaveCollections(chosen)
        } else {
            Message::Filter(chosen)
        }
    }))
}

/// Whether the *no projects* empty state is drawn, which is `showProjectsEmptyState`
/// (`layout.vue:827-832`) term for term.
///
/// ```text
/// selectedProjectType !== 'collection'
///   && filteredProjects.length === 0
///   && (selectedProjectType !== null || collections.value.length === 0)
/// ```
///
/// The third term is the one a port drops, and it is the reason this is a function
/// rather than an `if`: an account with collections and no projects of its own would
/// be told *This user has no projects!* on the *All* tab, immediately above the
/// collection grid that tab is about to draw. A named type is the other way out --
/// the address is what the reader asked for there, so an empty list is the answer --
/// and the collections tab is never it, because `layout.vue:257` draws the grid and
/// `showCollectionsEmptyState` the heading instead.
///
/// `shown` is passed rather than read so the predicate can be gated without a
/// profile; the caller only asks when it is empty.
fn shows_projects_empty_state(filter: Filter, has_collections: bool, shown: usize) -> bool {
    filter.shows_projects()
        && shown == 0
        && (filter.project_type().is_some() || !has_collections)
}

/// The grid of collection cards, or nothing when there is nothing to put in it.
///
/// `layout.vue:257-320`'s `ProjectCardList layout="grid"`, whose own rule is
/// `grid-template-columns: repeat(auto-fill, minmax(350px, 1fr))` over `gap-3`.
/// `auto-fill` needs a width to fill and iced has no grid, so the column count is
/// worked out from the content width the reference lays that grid out in -- measured
/// on `/tmp/ref/user-ref.png` at the reference's own 1280x720 as
/// `x=88..955` for a project card, so [`COLLECTION_CONTENT`] is 868 pixels -- and
/// from the 350-pixel floor and the 12-pixel gap the rule states.
fn collection_grid<'a>(theme: Gen, profile: &'a Profile) -> Option<Element<'a, Message>> {
    if !profile.has_collections() {
        return None;
    }
    let cards = profile.sorted_collections();
    let columns = collection_columns(COLLECTION_CONTENT);
    let mut rows: Vec<Element<'a, Message>> = Vec::new();
    for chunk in cards.chunks(columns) {
        let mut row = row![].spacing(COLLECTION_GAP).width(Length::Fill);
        for collection in chunk {
            row = row.push(collection_card(theme, profile, collection));
        }
        // The last row of a grid whose cards do not divide evenly keeps its columns
        // rather than stretching the cards it does have, which is what
        // `grid-template-columns` does with `1fr` and what a `Row` of `Fill` children
        // does not.
        for _ in chunk.len()..columns {
            row = row.push(Space::new(Length::Fill, Length::Shrink));
        }
        rows.push(row.into());
    }
    let mut grid = column![].spacing(COLLECTION_GAP).width(Length::Fill);
    for line in rows {
        grid = grid.push(line);
    }
    Some(grid.into())
}

/// `repeat(auto-fill, minmax(350px, 1fr))` resolved against a content width.
///
/// `auto-fill` fits as many tracks as the floor allows and shares the rest between
/// them, so this is a floor division on `(width + gap)`: two tracks at the
/// reference's own 868 (2 x 350 + 12 = 712 fits, 3 x 350 + 24 = 1074 does not), and
/// at least one however narrow the column gets, because a grid with no columns
/// draws nothing and the reference's always draws one.
fn collection_columns(width: f32) -> usize {
    let fits = ((width + COLLECTION_GAP) / (COLLECTION_MIN + COLLECTION_GAP)).floor();
    (fits as usize).max(1)
}

/// One collection's card.
///
/// The whole of `layout.vue:268-320`: a `grid-cols-[auto_1fr] gap-4` head holding a
/// 64-pixel `Avatar` beside the name at `text-lg font-semibold` and the
/// `LibraryIcon` + `profile.label.collection` line, then the description, then the
/// foot's `BoxIcon` + `profile.collection.projects-count`. There is no status line,
/// and that is [`collection_status`]'s condition rather than an omission:
/// `canSeeCollectionStatus` is `isSelf || isStaffViewing`, both false for a reader
/// on somebody else's profile with no Modrinth session.
///
/// `flex-col gap-4` with `grow` on the description and `mt-auto` on the foot, so the
/// foot is pinned to the card's bottom and the description takes the slack between.
/// iced has no `grow`, so the slack is a `Fill` spacer instead -- the same
/// substitution `Stack` makes elsewhere on this page.
fn collection_card<'a>(
    theme: Gen,
    profile: &'a Profile,
    collection: &'a ModrinthCollection,
) -> Element<'a, Message> {
    // `grid-cols-[auto_1fr] gap-4`: the picture is the `auto` column and everything
    // beside it shares the `1fr`, which is what truncates the name.
    //
    // `Avatar size="64px" no-shadow` for the picture, and the placeholder rather than
    // nothing for a `null` one -- which is the shape every collection the live
    // service was measured on answers, all four of FlameFire's. A picture that did
    // arrive is drawn through the kit's own icon box, whose corner radius is the
    // avatar module's rule and not a measurement of this card: no collection with an
    // `icon_url` was available to measure against.
    let picture: Element<'a, Message> = match profile.collection_icon(collection) {
        Some(icon) => ui::icon_box(theme, COLLECTION_AVATAR, Some(icon)),
        None => page::glyph(theme, Glyph::CircleUser, COLLECTION_AVATAR),
    };
    let head = row![]
        .width(Length::Fill)
        .spacing(COLLECTION_INNER_GAP)
        .align_items(Alignment::Start)
        .push(picture)
        .push(
            column![]
                .width(Length::Fill)
                .spacing(COLLECTION_NAME_GAP)
                .push(
                    text(collection.name.clone())
                        .size(COLLECTION_NAME)
                        .line_height(iced::Pixels(COLLECTION_NAME_LINE))
                        .font(semibold())
                        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
                )
                .push(
                    row![]
                        .spacing(ui::METADATA_GAP / 2.0)
                        .align_items(Alignment::Center)
                        .push(icon::icon(
                            Glyph::Library,
                            COLLECTION_ICON,
                            theme_gen::ink(theme, INK_SECONDARY),
                        ))
                        .push(
                            text(Key::ProfileLabelCollection.message())
                                .size(COLLECTION_LABEL)
                                .line_height(iced::Pixels(COLLECTION_LABEL_LINE))
                                .font(crate::style::regular())
                                .style(iced::theme::Text::Color(theme_gen::ink(
                                    theme,
                                    INK_SECONDARY,
                                ))),
                        ),
                ),
        );
    // `mt-auto`: the description grows, so the foot is pinned to the card's own
    // bottom rather than to the end of the text above it.
    let mut card = column![]
        .width(Length::Fill)
        .spacing(COLLECTION_INNER_GAP)
        .push(head);
    if !collection.description.is_empty() {
        card = card.push(
            text(collection.description.clone())
                .size(COLLECTION_LABEL)
                .line_height(iced::Pixels(COLLECTION_LABEL_LINE))
                .width(Length::Fill)
                .font(crate::style::regular())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
        );
    }
    let mut foot = row![]
        .spacing(COLLECTION_INNER_GAP)
        .align_items(Alignment::Center)
        .push(icon::icon(
            Glyph::Box,
            COLLECTION_ICON,
            theme_gen::ink(theme, INK_SECONDARY),
        ))
        .push(
            text(text_gen::profile_collection_projects_count(
                collection.project_count() as u64,
            ))
            .size(COLLECTION_LABEL)
            .line_height(iced::Pixels(COLLECTION_LABEL_LINE))
            .font(crate::style::regular())
            .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
        );
    if let Some((glyph, key)) = collection_status(false, &collection.status) {
        foot = foot.push(
            row![]
                .spacing(ui::METADATA_GAP / 2.0)
                .align_items(Alignment::Center)
                .push(icon::icon(glyph, COLLECTION_ICON, theme_gen::ink(theme, INK_SECONDARY)))
                .push(
                    text(key.message())
                        .size(COLLECTION_LABEL)
                        .line_height(iced::Pixels(COLLECTION_LABEL_LINE))
                        .font(crate::style::regular())
                        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
                ),
        );
    }
    card = card.push(Space::new(Length::Fill, Length::Shrink)).push(foot);
    ui::card(theme, card)
}

/// The status line's glyph and label, when the viewer may see it at all.
///
/// `layout.vue:298-316` is `v-if="canSeeCollectionStatus"`, and
/// `canSeeCollectionStatus` is `isSelf.value || isStaffViewing.value` -- the
/// profile's own collections, or a moderator looking at them. Both are false for a
/// reader looking at somebody else's account in a launcher with no Modrinth
/// credential, so the line is parsed off the document and never drawn; the arm is
/// kept because the condition is the reader's, not the document's.
///
/// `None` for a status the reference names no line for, which is the reference's own
/// outcome: its four `v-else-if`s cover `listed` / `unlisted` / `private` /
/// `rejected` and a fifth value prints nothing.
fn collection_status(
    can_see: bool,
    status: &str,
) -> Option<(Glyph, Key)> {
    if !can_see {
        return None;
    }
    match status {
        "listed" => Some((Glyph::Globe, Key::LabelPublic)),
        "unlisted" => Some((Glyph::Link, Key::LabelUnlisted)),
        "private" => Some((Glyph::Lock, Key::CollectionsLabelPrivate)),
        "rejected" => Some((Glyph::X, Key::LabelRejected)),
        _ => None,
    }
}

/// One project of the list.
///
/// `ProjectCard.vue`'s **list** layout, which is what `layout.vue` asks for:
/// `p-4 grid` over `grid-project-card-list`, whose `has-actions` template is
///
/// ```text
/// 'icon info actions actions'
/// 'icon info dummy   stats'
/// 'icon tags  tags   stats'
/// ```
///
/// with `grid-template-columns: auto 1fr auto auto` and `gap-x-3 gap-y-2`.
///
/// That template is the whole reason this function is not a column of two things.
/// The icon column is named in *all three* rows, so it runs the card's full
/// height, and the tags sit on the card's last row -- which is inside the icon's
/// span, not after it. So the head is a [`Stack`]: the first layer is the icon
/// beside the `1fr` info column and the two right-hand columns, the second is the
/// tags at the card's own bottom.
///
/// `__stats` spans rows two *and* three, which is the other thing a row-and-column
/// reading gets wrong. It is one `flex flex-col gap-3 items-end` holding the
/// downloads line and then the date, right-aligned, and its `mt-3` puts it a
/// `gap-y-2` and twelve more pixels below the button's row. Drawn as two
/// independent rows the audit measured the downloads at rel y 95 against the
/// reference's 75, the date at 101 against 106, and the two overlapping by ten.
///
/// Horizontally the same template is what decides where the `1fr` info column
/// stops, which is load-bearing rather than tidiness -- the column is what wraps
/// the summary. It is derived in full at [`project_row`]'s `right`, from
/// `ProjectCard.vue:319-325` and css-grid-1 §11.5.
///
/// Every landmark below is measured off `/tmp/ref/user-ref.png` at the
/// reference's own 1280x720: the card is x=88..955 and y=259..400 (868 x 142), its
/// content box is x=105..938 and y=276..384, the icon is x=105..204 and
/// y=276..375, the *Install* button is x=845..938 and y=276..311, the stats are
/// x=786..937 and y=334..349, the tags are y=358..383 and the date is y=365..382.
fn project_row<'a>(
    theme: Gen,
    profile: &'a Profile,
    project: &'a ModrinthUserProject,
) -> Element<'a, Message> {
    // A row's identity is the project it names rather than its place in the list, so
    // reordering the list must not move a tween from one row to another.
    let key = ui::scoped("user:project", &project.id);
    let (factor, _) = ui::interaction(key);
    let now = now_millis();

    // `__info`, the `1fr` column: the title, then the summary. It is width-
    // constrained rather than laid out at its natural width, because the column it
    // has to stop in is what wraps it: on the reference's own first card the
    // summary's first line ends at x=739, 47 pixels short of the stats.
    let info = column![]
        .width(Length::Fill)
        .spacing(CARD_INFO_GAP)
        .push(
            text(project.title.clone())
                .size(CARD_TITLE)
                .line_height(iced::Pixels(CARD_TITLE))
                .font(semibold())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
        )
        .push(
            // `ProjectCard.vue:120`'s `project-card-summary m-0 font-normal
            // line-clamp-2` carries no size class, so it inherits `text-base` --
            // sixteen pixels on the stylesheet's own 18.4-pixel line. `text-sm` is
            // only reached from the `@container (width < 550px)` block, and an
            // 868-pixel card never trips it.
            //
            // `line-clamp-2` is a *height*, and iced has no line clamp: what it
            // does have is cosmic-text's own line budget, which
            // `LayoutRunIter::new` computes as `(height / line_height) as i32`
            // (vendor/cosmic-text/src/buffer.rs:215). Pinning the box to two lines
            // therefore pins the paragraph to two lines -- and to two lines of
            // *height*, which is what `__info`'s own height is: measured, the
            // reference's summary occupies y=305..340 and nothing below it. Without
            // the box the summary ran to a fourth line and printed a stray word at
            // y=358, inside the tags row.
            container(
                text(project.description.clone())
                    .size(CARD_SUMMARY)
                    .line_height(iced::Pixels(CARD_SUMMARY_LINE))
                    .width(Length::Fill)
                    .font(crate::style::regular())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
            )
            .width(Length::Fill)
            .height(Length::Fixed(CARD_SUMMARY_LINE * 2.0)),
        );
    // `__stats`, the grid's *fourth* column: the downloads-and-followers line and
    // then the date under it, both flush right against the card's own edge.
    let mut lines = column![]
        .spacing(CARD_STATS_GAP)
        .align_items(Alignment::End)
        .push(
            row![]
                .spacing(CARD_STATS_GAP)
                .align_items(Alignment::Center)
                .push(stat(theme, Glyph::Download, &compact_stat(project.downloads)))
                .push(stat(theme, Glyph::Heart, &compact_stat(project.followers))),
        );
    // `ProjectCardDate` with `autoDisplayDate`, which is `'updated'` whenever the
    // document carries one and `HistoryIcon` for that case.
    let date_key = if project.updated.is_empty() {
        (Glyph::Calendar, project.published.as_str())
    } else {
        (Glyph::History, project.updated.as_str())
    };
    let when = how_ago(date_key.1, now);
    if !when.is_empty() {
        lines = lines.push(stat(theme, date_key.0, &when));
    }
    // The `gap-x-3` between the third and fourth columns is a fixed-size track of
    // its own (css-grid-1 §11.2), so the span those two columns make is a row of
    // [the third track, which nothing in this layout sizes, then the stats] --
    // twelve pixels wider than the stats alone whenever the stats are what
    // governs that span.
    let stats = row![]
        .spacing(CARD_GAP_X)
        .push(Space::with_width(Length::Shrink))
        .push(lines);
    // `__actions` and `__stats` are one span of the grid -- columns three and
    // four, the gutter between them included -- so this is the width of that
    // whole span, and it is what the `1fr` column is measured against.
    //
    // It is the *wider of two things*, and which one wins is the reference's
    // whole answer to a card that wraps a word early. `ProjectCard.vue:319-325`:
    //
    // ```css
    // .grid-project-card-list.has-actions {
    //     grid-template:
    //         'icon info actions actions'
    //         'icon info dummy stats'
    //         'icon tags tags stats';
    //     grid-template-columns: auto 1fr auto auto;
    // }
    // ```
    //
    // `__actions` spans the third column, the gutter and the fourth, and css-grid
    // hands a spanning item's width to the tracks it spans (§11.5: "evenly
    // distributing the extra space across those tracks insofar as possible"), so
    // tracks three and four together owe it the button's own 189 pixels *less the
    // 12 the gutter already owes*. Nothing else is in the third column -- the
    // `dummy` area of row two is named and never filled -- so it takes the whole
    // shortfall, and the span comes to exactly the button's width. `__stats` is
    // the one item of the fourth column alone and `auto` is `minmax(auto,
    // max-content)`, so that track is as wide as the wider of the stats line and
    // the date under it. How the 189 splits between the two `auto` tracks is not
    // observable and does not matter: `__actions` is `ml-auto` and `__stats` is
    // `items-end`, so both are flush against the card's own right edge whatever
    // the split is, and only the sum reaches the summary.
    //
    // Measured on the reference's own three cards at 1280x720: the first card's
    // button is 94 (`x=845..938`) against a 153-pixel stats line, so the stats
    // govern, the third track is nothing, the span is 165, and the button sits
    // flush right in it. The second and third cards carry *Install to instance*
    // at 189 (`x=750..938`), wider than their 139- and 119-pixel stats, so there
    // the button governs, the span is 189, and the summary's `1fr` column is
    // `834 - 100 - 3 * 12 - (189 - 12) = 521` -- the 521 that card two's first
    // line needs to carry *A skyblock* (`x=217..737`, its em dashes at
    // `620..651`) and not one word more.
    //
    // What is left of that on the second and third cards is not in this file: our
    // *Install to instance* measures 183 where the reference's measures 189 -- the
    // label's own advance, 136.76 by `Inter-600`'s `hmtx` against the ~143 the
    // reference paints, which is the letter-spacing question and not a number here.
    // So those two cards get `521 + 6 - 1`, the last pixel being this card's own
    // content box being one wider than the reference's.
    let right = column![]
        .spacing(CARD_STATS_LEAD)
        .align_items(Alignment::End)
        // `User.vue`'s `#project-actions` slot: an outlined brand button, a
        // download icon for a pack and a plus for anything else.
        .push(install_button(theme, ui::scoped(INSTALL_KEY, &project.id), project))
        .push(stats);
    let head = row![]
        .width(Length::Fill)
        // One `gap-x-3`: the room between the `1fr` column and the span the two
        // right-hand columns make. The other two gaps are inside the icon row and
        // inside `right`, which is the span itself.
        .spacing(CARD_GAP_X)
        .push(
            row![]
                .width(Length::Fill)
                .spacing(CARD_GAP_X)
                .align_items(Alignment::Start)
                // `Avatar size="100px"`, the icon column every reference card opens
                // with. Measured: x=105..204 and y=276..375, which is the 100 its
                // container query keeps above 850 pixels of card width.
                .push(icon_box(theme, profile.icon(project)))
                .push(info),
        )
        .push(right);

    // Row 3: `__tags`, which spans columns two to four and is pushed to the
    // bottom of its row by `mt-auto`, and so ends flush with the content box.
    // The row is the reference's own composition: the environment pill, then
    // `ProjectCardTags`' visible tags, then its `+N` pill -- see [`card_tags`].
    let tags = tags_row(theme, project);
    // The indent is *inside* this row rather than being the overlay's offset: a
    // translated layer still lays out at the stack's full width, so offsetting the
    // row by 112 gave it 112 pixels more than the card has and pushed whatever sat
    // at its right edge off the card.
    let tail = row![]
        .width(Length::Fill)
        .height(Length::Fixed(CARD_TAG_ROW))
        // `items-center`, off the row's own `flex items-center gap-1`: a 24-row
        // pill in a 26-row row sits one pixel lower, which is what the reference
        // measures for every pill of its tags rows that carries no icon.
        .align_items(Alignment::Center)
        .spacing(CARD_GAP_X)
        // The icon column, with the grid's own `gap-x-3` after it: `100 + 12 = 112`
        // from the content box's left edge, which is `1 + 16 + 100 + 12 = 129` from
        // the card's own -- the reference's x=217. Indenting by `112` *and*
        // spacing by 12 put the first pill at x=229, twelve too far.
        .push(Space::with_width(Length::Fixed(CARD_ICON)))
        .push(tags);

    let body = Stack::at(
        Vector::ZERO,
        container(head).width(Length::Fill).height(Length::Fixed(CARD_CONTENT)),
    )
    // The tags row, `mt-auto` to the bottom of the content box, 26 pixels tall --
    // the reference's own row, which its `h-4` tag icon makes 26 -- so the row's
    // last row is the content box's last row.
    .over(Vector::new(0.0, CARD_TAGS_TOP), tail);

    mouse_area(
        container(body)
            .width(Length::Fill)
            // Seventeen above and sixteen below, because iced's border is inside
            // its bounds and CSS's is not: see [`CARD_PAD_TOP`].
            .padding(Padding {
                top: CARD_PAD_TOP,
                // Sixteen on the left and seventeen on the right, and the extra
                // pixel is the card's *own* one rather than a fudge: see
                // [`CARD_PAD_RIGHT`].
                right: CARD_PAD_RIGHT,
                bottom: ui::CARD_PAD,
                left: ui::CARD_PAD,
            })
            .style(move |_theme: &iced::Theme| container::Appearance {
                background: Some(iced::Background::Color(crate::theme::brightness(
                    theme_gen::ink(theme, theme_gen::Ink::Surface3),
                    factor,
                ))),
                border: Border {
                    color: crate::theme::brightness(
                        theme_gen::ink(theme, theme_gen::Ink::Surface4),
                        factor,
                    ),
                    width: 1.0,
                    radius: theme_gen::span(theme_gen::Span::RadiusLg).into(),
                },
                ..container::Appearance::default()
            }),
    )
    .interaction(Interaction::Pointer)
    .on_enter(Message::hover(key, true))
    .on_exit(Message::hover(key, false))
    .on_press(Message::Project(project.id.clone()))
    .into()
}

/// What one card's tags row draws.
///
/// `ProjectCard.vue`'s list layout composes the row out of three things, in this
/// order and no other: `ProjectCardEnvironment` (when the document names an
/// environment), `ProjectCardTags` (its visible tags), and `TagsOverflow` (the
/// `+N` pill, which stands for everything that did not fit). What used to be
/// drawn here was the project's *type* in a single pill, which is none of the
/// three -- and the reference's first card measures six pills where that was one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tags {
    /// The icon `ProjectCardEnvironment` puts in front of the environment's own
    /// message, and that message: `None` when the document names no environment,
    /// which is the reference's `empty:hidden` on the same `TagItem`.
    pub environment: Option<(Glyph, String)>,
    /// The tags that fit, in the order `ProjectCardTags` draws them.
    pub tags: Vec<Tag>,
    /// How many tags the `+N` pill is standing in for.
    pub overflow: usize,
}

/// One visible tag, with the glyph `TagTagItem` puts in front of its label.
///
/// The three fields are the three things `ProjectCardTags` reads off a tag, and
/// the icon is one of them because `TagTagItem`'s own rule is a *per-tag* one:
///
/// ```ts
/// const icon = computed(() =>
///     props.hideNonLoaderIcon && !isLoader.value ? undefined : getTagIcon(props.tag))
/// ```
///
/// `ProjectCardTags.vue:56` passes `hide-non-loader-icon`, so on a card the icon
/// is [`crate::ui::tag_icon`] for a **loader** and `None` for a category -- which
/// is the whole reason the row is not one height. *Client and server*, *Forge*
/// and *Modpack* are twenty-six rows on the reference's own capture and
/// *Challenging* and *Combat* are twenty-four, and the difference is exactly
/// whether this field is `Some`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    /// The tag as the document publishes it, which is the key both of the
    /// reference's tables are asked with.
    pub id: String,
    /// `formatTag(tag)` (`tag-messages.ts:663`): the loader's message, then the
    /// category's, then the capitalised tag for one neither table has.
    pub label: String,
    /// `getTagIcon(tag)`, and `None` for a category because of
    /// `hide-non-loader-icon`. Also `None` for the four loader icons this port
    /// does not draw -- see [`crate::ui::tag_icon`], which names them.
    pub icon: Option<Glyph>,
}

/// One pill of a card's tags row, as the reference's `<template>` order draws it.
///
/// The row is a [`crate::ui::tag_height`] question rather than a
/// [`crate::ui::tag`] one, and this is the answer the row is built from: three
/// kinds of pill in one list, of which two carry a glyph and one does not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pill {
    /// The label the pill shows.
    pub label: String,
    /// The `h-4` glyph in front of it, if the reference draws one there.
    pub icon: Option<Glyph>,
    /// The tag's own id, and `None` for the two pills that are not a tag.
    ///
    /// It is here because `TagTagItem` is the one place in the reference that
    /// paints a tag in something other than `--color-secondary`, and what it
    /// paints it in is named after this string:
    /// `--_color: var(--color-platform-${tag})`. The environment's pill is a
    /// plain `TagItem` with no `TagTagItem` around it, and `+N` is
    /// `TagsOverflow`'s plain `TagItem` with no slot in it at all, so neither
    /// has a tag to be coloured by.
    pub id: Option<String>,
}

impl Tags {
    /// Every pill of the row, in the order the reference's own `<template>` draws
    /// them: the environment's message, then each visible tag's own, then `+N`.
    ///
    /// The three are built by three different components and they disagree about
    /// the icon, which is why this returns pills rather than labels:
    /// `ProjectCardEnvironment` always puts one there (`GlobeIcon`,
    /// `ClientIcon`, `ServerIcon` or `UserIcon`, per
    /// `ProjectCardEnvironment.vue:115-138`); `ProjectCardTags` passes
    /// `hide-non-loader-icon`, so only a loader's is drawn; and `TagsOverflow`
    /// writes `+{{ tags.length }}` into a plain `TagItem` with no slot in it, so
    /// the count never has one.
    pub fn pills(&self) -> Vec<Pill> {
        let mut pills: Vec<Pill> = Vec::with_capacity(self.tags.len() + 2);
        if let Some((glyph, label)) = &self.environment {
            pills.push(Pill { label: label.clone(), icon: Some(*glyph), id: None });
        }
        pills.extend(self.tags.iter().map(|tag| Pill {
            label: tag.label.clone(),
            icon: tag.icon,
            id: Some(tag.id.clone()),
        }));
        if self.overflow > 0 {
            pills.push(Pill { label: format!("+{}", self.overflow), icon: None, id: None });
        }
        pills
    }

    /// The visible tags' own ids, which is what the row is sorted and sliced on.
    pub fn ids(&self) -> Vec<&str> {
        self.tags.iter().map(|tag| tag.id.as_str()).collect()
    }

    /// The pills' labels, in the order the reference's own `<template>` draws them:
    /// the environment's message, then each visible tag's own, then `+N`.
    ///
    /// Every label is the reference's: `formatTag` (`tag-messages.ts:663`) is
    /// `getTagMessage(tag)` -- the loader's table first, then the category's -- and
    /// `capitalizeString` for a tag neither table has, which is
    /// [`crate::locale`]'s `loader_label`/`category_label` and [`is_loader`]
    /// choosing which of the two tables is meant (`minecraft` is a loader for
    /// resource packs and a category for mods).
    pub fn labels(&self) -> Vec<String> {
        self.pills().into_iter().map(|pill| pill.label).collect()
    }
}

/// One card's tag row, as `ProjectCard.vue`'s list layout composes it.
///
/// The composition is four steps, each from the reference's own source:
///
/// 1. `getProjectCardTags` (`v3-projects.ts:20`): the project's `categories`, then
///    its `loaders`, then its `mrpack_loaders`.
/// 2. `ProjectCardTags.vue`'s `uniqueSorted`: a `Set`, then `sortTagsForDisplay`
///    (`tag-messages.ts:585`) -- the categories alphabetically, then the loaders
///    with [`DEFAULT_LOADERS`] first and the rest after them, each group
///    alphabetically.
/// 3. `slice(0, maxTags)`, where `maxTags` is `(maxTags || (actions ? 4 : 5)) +
///    (!!environment ? 0 : 1)` (`ProjectCard.vue:174`). `ProjectList.vue` passes no
///    `maxTags` and `User.vue` does pass an `#actions` slot -- the *Install*
///    button -- so the first term is four, and an environment pill, which is drawn
///    outside `ProjectCardTags`, is what takes the fifth a card without one shows.
/// 4. `extraTags` (`ProjectCard.vue:288`) is `allTags` less `tags`, which is
///    `additional_categories` (`getProjectCardAllTags`), and `overflowTags`
///    (`ProjectCardTags.vue:44`) is what is left of both -- so a card with no
///    overflow of its own still shows a `+N` for the extra categories it declares.
///
/// Measured on the reference's own three cards at 1280x720, against the answers
/// `GET /v2/user/FlameFire/projects` gives for the same three projects:
///
/// | card | tags | pills the reference draws |
/// | --- | --- | --- |
/// | Zombie Invade 100 Days | `challenging combat forge` + `mrpack` | *Client and server Challenging Combat Forge Modpack +1* |
/// | Random Island | `minigame worldgen` + `fabric forge neoforge datapack quilt` | *Server Minigame World Generation Fabric Forge +3* |
/// | Zombie Invade Nether End | `mobs datapack` | *Mobs Data Pack +1* |
///
/// The third is the one that says the environment is read as
/// `project.environment?.[0]` and nothing else: its document publishes an empty
/// `environment`, and the reference draws no environment pill for it even though
/// its `client_side`/`server_side` pair would answer *Server*.
pub fn card_tags(project: &ModrinthUserProject) -> Tags {
    let environment = environment_tag(project);
    // `(maxTags || (!!$slots.actions ? 4 : 5)) + (!!environment ? 0 : 1)`.
    let max = if environment.is_some() { 4 } else { 5 };
    let tags = sort_tags_for_display(&unique(&card_tag_ids(project)));
    let shown = tags.len().min(max);
    // `allTags.filter((tag) => !tags.includes(tag))`: the extra categories a
    // project declares, less any the row already shows.
    let extra: Vec<String> = project
        .additional_categories
        .iter()
        .filter(|tag| !tags.contains(tag))
        .cloned()
        .collect();
    Tags {
        environment,
        // The glyph is asked for here, where the tag is built rather than where
        // the row is drawn, because `hide-non-loader-icon` is a property of the
        // tag and not of the surface the row happens to be on: the same *Forge*
        // carries an icon on this card and on a tag list, and carries none on a
        // card that hides non-loader icons. `formatTag` and `getTagIcon` are both
        // keyed off the id.
        tags: tags[..shown]
            .iter()
            .map(|id| Tag {
                label: if is_loader(id) {
                    crate::locale::loader_label(id)
                } else {
                    crate::locale::category_label(id)
                },
                // `TagTagItem.vue:28`: the loader table first, and nothing at all
                // for a category, because `ProjectCardTags` passes
                // `hide-non-loader-icon`.
                icon: if is_loader(id) { ui::tag_icon(id) } else { None },
                id: id.clone(),
            })
            .collect(),
        // `[...new Set([...tags - visible, ...extra - visible])]`, and the two
        // lists cannot share a tag by the filter above.
        overflow: tags.len() - shown + extra.len(),
    }
}

/// `getProjectCardTags`: the tags a card is composed from, before it is sorted.
///
/// The reference reads a **v3** project document here, which splits a modpack's
/// loaders in two -- `loaders: ["mrpack"]` and `mrpack_loaders: ["forge"]` -- and
/// the v2 document this page reads publishes neither key. What it does publish
/// for the same project is `project_type: "modpack"` and `loaders: ["forge"]`,
/// which is the same pair with the type spelled out, so the loader tag a modpack's
/// own row is missing is put back from it. That is a derivation and it is
/// measured rather than guessed: of the hundred most-downloaded modpacks
/// `GET /v2/search?facets=[["project_type:modpack"]]` returns, all hundred
/// publish `mrpack` in their v3 `loaders`, and for the thirty sampled against
/// `GET /v2/project/{id}` those loaders are exactly `["mrpack"]` while the v2
/// `loaders` are exactly the v3 `mrpack_loaders` -- so for every one of them
/// this function returns what `getProjectCardTags` returns.
///
/// What it cannot be is a read, and the case it would get wrong is a project v2
/// calls a mod that v3 also calls one: there the reference draws *Modpack* and
/// this draws nothing, because v2's single `project_type` string has already lost
/// the second type. The v3 `project_types` array now *is* read, for the strip's own
/// filters (see [`Profile::type_of`]), and it does not reach here: `mrpack` is one
/// of the two types a modpack leads with rather than a loader it publishes, and
/// deciding that from an array this row does not receive would be a second reading
/// of the same document in two places.
fn card_tag_ids(project: &ModrinthUserProject) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    tags.extend(project.categories.iter().cloned());
    tags.extend(project.loaders.iter().cloned());
    if project.project_type == "modpack" {
        tags.push(MRPACK_LOADER.to_string());
    }
    tags
}

/// `mrpack`, the loader tag `tag-messages.ts` gives the message *Modpack*.
///
/// Spelled out rather than taken from a table because it is the one tag this row
/// adds that is in neither of the document's lists -- see [`card_tag_ids`].
const MRPACK_LOADER: &str = "mrpack";

/// `ProjectCardEnvironment.vue`'s `displayEnvironment`, for the value
/// `ProjectList.vue:39` passes it.
///
/// The v3 environment is a *string* on the reference's page, and this function is
/// its own `switch` (`ProjectCardEnvironment.vue:62-81`) with no fall-through:
/// a project whose environment this port does not name draws no pill at all,
/// which is what the reference's `empty:hidden` on the same `TagItem` does.
///
/// The icon is part of what the reference draws and is returned with the label
/// rather than looked up again, because `ProjectCardEnvironment` chooses it per
/// environment and a caller that picked its own would put a globe where the
/// reference puts a drive.
fn environment_tag(project: &ModrinthUserProject) -> Option<(Glyph, String)> {
    // `project.environment?.[0]`, so a project whose `environment` is empty has
    // none. Its `client_side`/`server_side` are deliberately not the fallback:
    // `ProjectCardEnvironment` has a `{ clientSide, serverSide }` arm and
    // `ProjectList.vue` never fills it in.
    let environment = project.environment.first()?;
    let (glyph, key) = match environment.as_str() {
        "client_or_server" | "client_or_server_prefers_both" => {
            (Glyph::Globe, Key::ProjectCardEnvironmentClientOrServer)
        }
        "client_and_server" => (Glyph::Globe, Key::ProjectCardEnvironmentClientAndServer),
        "client_only" | "client_only_server_optional" => {
            // `ClientIcon`, which is `assets/icons/client.svg` -- the monitor on
            // its stand, not `monitor-smartphone.svg`. They are different
            // pictures and only one of them is the one this `<template>` names;
            // the import is `import { ClientIcon, GlobeIcon, ServerIcon,
            // UserIcon } from '@modrinth/assets'` on line 3 and
            // `generated-icons.ts` resolves it to `./icons/client.svg`.
            (Glyph::Client, Key::ProjectCardEnvironmentClient)
        }
        "server_only" | "server_only_client_optional" => {
            (Glyph::Server, Key::ProjectCardEnvironmentServer)
        }
        "singleplayer_only" => (Glyph::User, Key::ProjectCardEnvironmentSingleplayer),
        "dedicated_server_only" => (Glyph::Server, Key::ProjectCardEnvironmentDedicatedServer),
        _ => return None,
    };
    Some((glyph, key.message().to_string()))
}

/// `sortTagsForDisplay`'s own test for a loader.
///
/// `getTagMessage(tag, 'loader')` being a message *is* `tag.loader.<tag>` being a
/// key in the generated table, so the lookup is the test rather than a list of
/// loaders kept beside it -- and a loader the reference's table has not caught up
/// with is one this row sorts as a category, which is what the reference does too.
fn is_loader(tag: &str) -> bool {
    text_gen::from_name(&format!("tag.loader.{tag}")).is_some()
}

/// `sortTagsForDisplay` (`tag-messages.ts:585`): the categories alphabetically,
/// then the loaders with [`DEFAULT_LOADERS`] ahead of the rest, each group
/// alphabetically.
///
/// `localeCompare` is a Unicode collation and this is a byte order, which for
/// the lowercase kebab-case tags Modrinth publishes agree on every ordering the
/// API is measured to answer -- the two part company on interior capitals, which
/// no tag carries.
fn sort_tags_for_display(tags: &[String]) -> Vec<String> {
    let mut categories: Vec<String> =
        tags.iter().filter(|tag| !is_loader(tag)).cloned().collect();
    let mut loaders: Vec<String> = tags.iter().filter(|tag| is_loader(tag)).cloned().collect();
    categories.sort();
    // `aDefault !== bDefault ? aDefault ? -1 : 1 : a.localeCompare(b)`: a stable
    // order either way, so the sort is by membership first and by name second.
    loaders.sort_by(|a, b| {
        let a_default = DEFAULT_LOADERS.contains(&a.as_str());
        let b_default = DEFAULT_LOADERS.contains(&b.as_str());
        b_default.cmp(&a_default).then_with(|| a.cmp(b))
    });
    categories.extend(loaders);
    categories
}

/// `[...new Set(tags)]`, which is what `uniqueSorted` does before it sorts.
fn unique(tags: &[String]) -> Vec<String> {
    let mut seen: Vec<&String> = Vec::with_capacity(tags.len());
    for tag in tags {
        if !seen.contains(&tag) {
            seen.push(tag);
        }
    }
    seen.into_iter().cloned().collect()
}

/// One card's tag row, as the reference's `<template>` order draws it.
///
/// `items-center` is the row's own class, and it is why the row is
/// [`CARD_TAG_ROW`] tall and the pills inside it are centred: a 24-row pill in a
/// 26-row row sits one pixel lower, which is what the reference measures for every
/// pill that carries no icon.
///
/// The pills themselves are [`ui::tag`]'s and [`ui::tag_with_icon`]'s, and which
/// one a pill gets is [`Pill::icon`] rather than a guess here -- three of the six
/// pills on the reference's first card are twenty-six rows and three are
/// twenty-four, and the difference is exactly whether the tag is a loader.
///
/// The one exception is a loader the reference paints in its own platform
/// colour, which is [`platform_tag`] rather than either of them: see
/// [`platform_colour`].
fn tags_row<'a>(theme: Gen, project: &'a ModrinthUserProject) -> Element<'a, Message> {
    let mut row = row![].spacing(CARD_TAG_GAP).align_items(Alignment::Center);
    for pill in card_tags(project).pills() {
        row = row.push(match (pill.icon, platform_colour(&pill)) {
            (icon, Some(ink)) => platform_tag(theme, &pill.label, icon, ink),
            (Some(glyph), None) => ui::tag_with_icon(theme, &pill.label, glyph),
            (None, None) => ui::tag(theme, &pill.label),
        });
    }
    row.into()
}

/// The `--color-platform-*` a loader tag's pill is painted in, if the reference
/// has one for that tag.
///
/// `TagTagItem.vue:2` is the whole rule:
///
/// ```text
/// <TagItem :style="isLoader ? `--_color: var(--color-platform-${tag})` : ''">
/// ```
///
/// so a loader gets its platform's colour and everything else keeps
/// `--color-secondary`. `TagItem`'s own class is
/// `text-[--_color,var(--color-secondary)]`, which is why the fallback is the
/// ink every other pill on this page is already drawn in -- and why the glyph
/// follows the label: `[&>svg]` is given a size and no colour, so the icon
/// inherits the `color` the label is set in.
///
/// Seventeen of the thirty loader tags have a token. Both columns below are the
/// reference's own table, `assets/styles/variables.scss:169` and `:389`, in the
/// order that file declares them:
///
/// | tag | token | light | dark |
/// | --- | --- | --- | --- |
/// | `bta-fabric` | `--color-platform-bta-fabric` | `#5BA938` | `#72CC4A` |
/// | `bukkit` | `--color-platform-bukkit` | `#E78362` | `#F6AF7B` |
/// | `bungeecord` | `--color-platform-bungeecord` | `#C69E39` | `#D2C080` |
/// | `fabric` | `--color-platform-fabric` | `#8A7B71` | `#DBB69B` |
/// | `folia` | `--color-platform-folia` | `#6AA54F` | `#A5E388` |
/// | `forge` | `--color-platform-forge` | `#5B6197` | `#959EEF` |
/// | `liteloader` | `--color-platform-liteloader` | `#4C90DE` | `#7AB0EE` |
/// | `neoforge` | `--color-platform-neoforge` | `#DC895C` | `#F99E6B` |
/// | `nilloader` | `--color-platform-nilloader` | `#DD5088` | `#F45E9A` |
/// | `ornithe` | `--color-platform-ornithe` | `#6097CA` | `#87C7FF` |
/// | `paper` | `--color-platform-paper` | `#E67E7E` | `#EEAAAA` |
/// | `purpur` | `--color-platform-purpur` | `#7763A3` | `#C3ABF7` |
/// | `quilt` | `--color-platform-quilt` | `#8B61B4` | `#C796F9` |
/// | `spigot` | `--color-platform-spigot` | `#CD7A21` | `#F1CC84` |
/// | `sponge` | `--color-platform-sponge` | `#C49528` | `#F9E580` |
/// | `velocity` | `--color-platform-velocity` | `#4B98B0` | `#83D5EF` |
/// | `waterfall` | `--color-platform-waterfall` | `#5F83CB` | `#78A4FB` |
///
/// The tokens themselves are already in [`crate::theme_gen`]: `gen_theme.py`
/// reads every custom property it classifies as a colour and
/// `--color-platform-*` are seventeen of its 144. So this table is only *which
/// token belongs to which tag*, which no stylesheet states -- `TagTagItem`
/// builds the name by interpolation -- and which of the thirty loader tags has
/// one at all.
///
/// The other thirteen (`babric`, `canvas`, `datapack`, `geyser`, `iris`,
/// `java-agent`, `legacy-fabric`, `minecraft`, `modloader`, `mrpack`,
/// `optifine`, `rift`, `vanilla`) have no token, and a `var()` naming a property
/// that does not exist makes `--_color` invalid at computed-value time, which
/// sends `text-[--_color,var(--color-secondary)]` to its own fallback: those
/// pills are `#96A2B0`, the ink they were already drawn in. So `None` here means
/// "leave it alone", not "guess a colour for it".
///
/// One of the seventeen is unreachable as things stand: [`ui::is_loader_tag`]
/// answers false for `bta-fabric`, because it asks the message id
/// `tag.loader.bta-fabric` where the reference asks its own table by key, and the
/// reference's id for that one is `tag.loader.bta-babric` upstream
/// (`tag-messages.ts:11`). The arm is here because the token is, and because that
/// one answer is what also costs that tag its glyph and its `BTA (Babric)`
/// label.
fn platform_ink(tag: &str) -> Option<Ink> {
    Some(match tag {
        "bta-fabric" => Ink::PlatformBtaBabric,
        "bukkit" => Ink::PlatformBukkit,
        "bungeecord" => Ink::PlatformBungeecord,
        "fabric" => Ink::PlatformFabric,
        "folia" => Ink::PlatformFolia,
        "forge" => Ink::PlatformForge,
        "liteloader" => Ink::PlatformLiteloader,
        "neoforge" => Ink::PlatformNeoforge,
        "nilloader" => Ink::PlatformNilloader,
        "ornithe" => Ink::PlatformOrnithe,
        "paper" => Ink::PlatformPaper,
        "purpur" => Ink::PlatformPurpur,
        "quilt" => Ink::PlatformQuilt,
        "spigot" => Ink::PlatformSpigot,
        "sponge" => Ink::PlatformSponge,
        "velocity" => Ink::PlatformVelocity,
        "waterfall" => Ink::PlatformWaterfall,
        _ => return None,
    })
}

/// The `--color-platform-*` one of this page's pills is painted in, if it is
/// painted in one at all: `TagTagItem`'s `isLoader` ternary over one pill, and
/// `None` for every other pill on the page, which is
/// `text-[--_color,var(--color-secondary)]`'s own fallback.
///
/// The two halves are kept apart on purpose: [`platform_ink`] is what the
/// stylesheet declares, and [`ui::is_loader_tag`] is what
/// `getTagMessage(tag, 'loader') !== undefined` answers. A category that shared
/// a loader's name would take the category's answer, which is the reference's
/// rule rather than this table's -- and the two pills that are not tags at all
/// are `None` because there is nothing to name a token after.
fn platform_colour(pill: &Pill) -> Option<Ink> {
    pill.id
        .as_deref()
        .filter(|id| ui::is_loader_tag(id))
        .and_then(platform_ink)
}

/// One of this page's tag pills, in an ink of the caller's choosing.
///
/// A platform-coloured loader is drawn here rather than through [`ui::tag`] or
/// [`ui::tag_with_icon`] because both of those paint `--color-secondary` and
/// nothing else, and `ui.rs` is not this slice's to edit: what the kit wants is
/// an ink parameter, and this is the caller-side equivalent of one.
///
/// Every number in the frame is the kit's own constant, so this cannot drift
/// from the two it stands in for in any of the seven things a pill is made of:
/// [`ui::TAG_HEIGHT_ICON`] or [`ui::TAG_HEIGHT`] for the height -- which is the
/// glyph's business and not the colour's -- half of that for the radius, because
/// `rounded-full` is a rule rather than a number, [`ui::TAG_PAD`] either side,
/// and [`ui::TAG_GAP`], [`ui::TAG_ICON`] and [`ui::TAG_LABEL_SIZE`] for the row
/// and the label in it.
///
/// A platform-coloured loader with no glyph in it is real rather than
/// hypothetical: *Purpur* and *Quilt* have tokens and are two of the four loader
/// icons the generator refuses, so their pills are the 24-row ones and are still
/// painted in the platform colour.
fn platform_tag<'a, Message: 'a>(
    theme: Gen,
    label: &str,
    glyph: Option<Glyph>,
    ink: Ink,
) -> Element<'a, Message> {
    let height = match glyph {
        Some(_) => ui::TAG_HEIGHT_ICON,
        None => ui::TAG_HEIGHT,
    };
    let painted = theme_gen::ink(theme, ink);
    // `TagItem.vue:19`'s `baseClass` is `py-1 leading-none ... font-normal text-sm`,
    // and `leading-none` is `line-height: 1`, so the line is the label's own size and
    // not `text-sm`'s `1.25rem`: the pill is `1 + 4 + 14 + 4 + 1 = 24` rows, which
    // is [`ui::TAG_HEIGHT`], and a twenty-pixel line would ask for thirty. Pinned
    // anyway, because iced's `Relative(1.3)` is 18.2 here and this is the second
    // site on this page that left the line to it.
    //
    // Which is a change no capture will show: the pill centres its content, and a
    // centred box puts the baseline at `top + H/2 + (A - D) * fs / 2` whatever the
    // line is, so the words and the `h-4` glyph land on the rows they were already
    // on. It is right rather than visible.
    let label_text = text(label.to_string())
        .size(ui::TAG_LABEL_SIZE)
        .line_height(iced::Pixels(ui::TAG_LABEL_SIZE))
        .font(regular())
        .style(iced::theme::Text::Color(painted));
    let content: Element<'a, Message> = match glyph {
        Some(glyph) => row![icon::icon(glyph, ui::TAG_ICON, painted)]
            .spacing(ui::TAG_GAP)
            .align_items(Alignment::Center)
            .push(label_text)
            .into(),
        None => label_text.into(),
    };
    container(content)
        .height(Length::Fixed(height))
        .padding(Padding {
            top: 0.0,
            bottom: 0.0,
            left: ui::TAG_PAD,
            right: ui::TAG_PAD,
        })
        .center_y()
        .style(move |_theme: &iced::Theme| container::Appearance {
            background: Some(iced::Background::Color(theme_gen::ink(theme, Ink::ButtonBg))),
            border: Border {
                color: theme_gen::ink(theme, Ink::Surface5),
                width: 1.0,
                radius: (height / 2.0).into(),
            },
            ..container::Appearance::default()
        })
        .into()
}

/// One statistic: an icon at `size-5` and the count beside it, in `font-medium`.
///
/// Drawn here rather than through `ui::icon_label` because the two disagree with
/// the reference: that one draws a 16-pixel icon (the reference's is `size-5`, 20)
/// and paints the text in `--color-text-secondary`, where `ProjectCardStats` names
/// no colour and so inherits `--color-text-primary`. The audit measured both as
/// exact inversions.
///
/// The line is pinned and not left to the default, and that is not tidiness.
/// cosmic-text crops a paragraph to `(height / line_height) as i32` lines
/// (vendor/cosmic-text/src/buffer.rs:215), and iced's own default line height is
/// `Relative(1.3)` -- 20.8 pixels at this size. `__stats`' date row is given 21
/// pixels of height by the grid above it, and 41 - 20.8 leaves it 20.2: with a
/// 20.8-pixel line that is *zero* lines, so the words measured 0 wide, the row
/// measured 28, and the date drew as its icon alone at the card's right edge. An
/// 18-pixel line -- the stylesheet's own `1.15`, the same number
/// [`CARD_SUMMARY_LINE`] is -- gives every row its `size-5` icon's own twenty
/// pixels and one full line of words, which is what the reference measures.
fn stat<'a, Message: 'a>(theme: Gen, glyph: Glyph, value: &str) -> Element<'a, Message> {
    row![]
        .spacing(METADATA_TEXT_GAP)
        .align_items(Alignment::Center)
        .push(icon::icon(glyph, CARD_STAT_ICON, theme_gen::ink(theme, INK_DEFAULT)))
        .push(
            text(value.to_string())
                .size(HEADER_TEXT)
                .line_height(iced::Pixels(CARD_SUMMARY_LINE))
                .font(medium())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
        )
        .into()
}

/// `User.vue`'s `#project-actions`: outlined, brand ink, a brand hairline.
///
/// `User.vue:13-19` is a `Button` with `type="outlined"` and
/// `class="!text-brand [&>svg]:!text-brand !shadow-[inset_0_0_0_1px_var(--color-brand)]"`,
/// which is `ButtonFrame.vue`'s own `outlined` type with its `--button-color` set to
/// the brand: a transparent plate, a 1-pixel brand ring and a brand label and icon.
/// Measured on the reference's own first card: 344 exact `#1BD96A` pixels on the
/// button alone -- ring `x=845..938, y=276..311`, corners rounded at `rounded-xl`'s
/// twelve -- and 1247 across the three cards, none of which `ui::Kind::Outlined`
/// drew, because that kind's ring is `Ink::Surface5` and its label is contrast ink.
///
/// So the frame is built here rather than taken from [`ui::Kind::Outlined`], which
/// is reserved to the control kit. Every number below is `ui::Size::Md`'s own --
/// `ButtonFrame.vue`'s `md` row reads `h-9 gap-1.5 rounded-xl px-2.5 text-base
/// font-semibold leading-5 [&>svg]:size-5`, and `ui.rs` already carries all seven.
/// The ring is a `Border` on the button's own 36-pixel box rather than an outer
/// shadow, which is what the reference's own 36 rows of ring measure.
fn install_button<'a>(
    theme: Gen,
    key: &'static str,
    project: &ModrinthUserProject,
) -> Element<'a, Message> {
    let pack = ProjectType::from_token(&project.project_type) == Some(ProjectType::Modpack);
    // `commonMessages.installButton` for a pack and `messages.installToInstance`
    // for anything else: the reference's own ternary over `project.project_types`,
    // which is why a pack's button reads *Install* and a mod's reads *Install to
    // instance*.
    let label = if pack { Key::ButtonInstall } else { Key::AppUserProjectInstallToInstance };
    let size = ui::Size::Md;
    // One ink for the ring, the label and the icon in front of it, which is what
    // `!text-brand [&>svg]:!text-brand` says and what a CSS `filter` on the element
    // does to all three together.
    let (factor, _) = ui::interaction(key);
    let ink = crate::theme::brightness(theme_gen::ink(theme, theme_gen::Ink::Brand), factor);
    let face = container(
        row![]
            .spacing(size.gap())
            .align_items(Alignment::Center)
            .push(icon::icon(
                if pack { Glyph::Download } else { Glyph::Plus },
                size.icon(),
                ink,
            ))
            .push(
                text(label.message())
                    .size(size.label())
                    .line_height(iced::Pixels(size.line()))
                    .font(size.font())
                    .style(iced::theme::Text::Color(ink)),
            ),
    )
    .height(Length::Fixed(size.height()))
    .padding(Padding {
        top: 0.0,
        bottom: 0.0,
        left: size.pad(),
        right: size.pad(),
    })
    .center_x()
    .center_y()
    .style(move |_theme: &iced::Theme| container::Appearance {
        // `bg-transparent`: the card's own surface shows through, as the reference's
        // 199 interior pixels of `#27292E` between the ring's arms do.
        border: Border { color: ink, width: 1.0, radius: size.radius().into() },
        ..container::Appearance::default()
    });
    mouse_area(face)
        .interaction(Interaction::Pointer)
        // The same crossing `ui::sized_face` publishes, so the 150 ms clock
        // (`crate::ui::pointer_with`, reached through [`Message::hover`]) carries
        // this button's brightness filter exactly as it carries every other's.
        .on_enter(Message::hover(key, true))
        .on_exit(Message::hover(key, false))
        .on_press(Message::Install(project.id.clone(), project.title.clone(), pack))
        .into()
}

/// The 100-pixel square a card's icon is drawn in, whether or not one arrived.
///
/// [`crate::ui::icon_box`] is the control kit's own: the reference's box, its
/// background and its hairline, with the picture composited onto it already
/// rounded by [`crate::avatar`]. An account whose projects have no icons draws the
/// empty box, which is the same box the reference's placeholder sits on.
fn icon_box<'a>(theme: Gen, icon: Option<&crate::avatar::Icon>) -> Element<'a, Message> {
    ui::icon_box(theme, CARD_ICON, icon)
}

// ---- The reference's own formatters ---------------------------------------

/// The current time, in milliseconds since the epoch.
///
/// The only impure thing on this page, and it is impure because the reference's
/// `useRelativeTime` is: *Joined 3 years ago* is a statement about now. Every
/// function that needs it takes the instant as an argument so a test can fix it.
fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_millis() as i64)
        .unwrap_or(0)
}

/// `Intl.NumberFormat('en', { notation: 'compact', maximumFractionDigits })`.
///
/// The header's metadata items are `PageHeaderMetadataNumberItem`s at their
/// default `compact: true`, which is this formatter with one fraction digit: a
/// profile with 15,244,929 downloads reads *15.2M downloads*, which is what the
/// reference draws at `/tmp/ref/user-ref.png`. Below a thousand the compact
/// notation is the plain number, and past a trillion CLDR has no suffix, so the
/// number is written out.
fn compact(value: u64, fraction: u32) -> String {
    const SUFFIXES: [(u64, &str); 4] =
        [(1_000, "K"), (1_000_000, "M"), (1_000_000_000, "B"), (1_000_000_000_000, "T")];
    // The suffix is chosen *after* the rounding, because that is what `Intl` does:
    // 999,999,999 is `1B`, not `1000M`, because a million-scale rendering of it
    // rounds to a thousand and a thousand is the next scale's one.
    let mut index = SUFFIXES.iter().rposition(|(scale, _)| value >= *scale);
    while let Some(at) = index {
        let (scale, suffix) = SUFFIXES[at];
        let (whole, decimals) = scaled_parts(value, scale, fraction);
        if whole < 1_000 {
            let point = if decimals.is_empty() { String::new() } else { format!(".{decimals}") };
            return format!("{whole}{point}{suffix}");
        }
        index = (at + 1 < SUFFIXES.len()).then_some(at + 1);
    }
    crate::text::number(value)
}

/// `formatCompactNumber` from `composables/format-number.ts`, which is what a
/// project card's `ProjectCardStats` prints -- and which is *not* the header's
/// formatter: below ten thousand it writes the grouped number, under a million it
/// allows one fraction digit, and over a million two. That is why the reference's
/// first card reads `14.84M` while its header reads `15.2M` for a larger number.
fn compact_stat(value: u64) -> String {
    if value < 10_000 {
        return crate::text::number(value);
    }
    if value < 1_000_000 {
        return format!("{}K", scaled(value, 1_000, 1));
    }
    compact(value, 2)
}

/// One value over a scale, with `fraction` decimals and no trailing zeros.
///
/// `Intl` rounds half away from zero and then drops the zeros `minimumFraction
/// Digits: 0` does not ask for, so 1,050,000 over a million is `1.05M` and
/// 15,000,000 over a million is `15M`. Hand-rolled rather than reaching for a
/// formatting crate for the same reason [`crate::text::number`] is.
fn scaled(value: u64, scale: u64, fraction: u32) -> String {
    let (whole, decimals) = scaled_parts(value, scale, fraction);
    if decimals.is_empty() {
        return whole.to_string();
    }
    format!("{whole}.{decimals}")
}

/// One value over a scale, as its whole part and as the digits that follow the
/// point with their trailing zeros already dropped.
///
/// Carried in units of 1/10^fraction so the rounding happens once, in integers:
/// 14.84 becomes 1484, not 14.839999999999999. Half away from zero, which is
/// `Intl`'s own default, expressed as the +1 on the numerator.
fn scaled_parts(value: u64, scale: u64, fraction: u32) -> (u128, String) {
    let factor = 10_u128.pow(fraction);
    let rounded = (value as u128 * factor * 2 + scale as u128) / (scale as u128 * 2);
    let decimals = rounded % factor;
    let tail = if decimals == 0 {
        String::new()
    } else {
        format!("{decimals:0>width$}", width = fraction as usize)
    };
    (rounded / factor, tail.trim_end_matches('0').to_string())
}

/// `useRelativeTime` from `composables/how-ago.ts`, which is what the header's
/// *Joined* and a card's date both print.
///
/// The thresholds are the composable's own, in its own order, and they are
/// deliberately not calendar-aware: a month is 2,629,746,000 milliseconds and a
/// year 31,556,952,000, because that is what `dayjs` divides by. An instant that
/// cannot be read comes back empty, exactly as `Number.isNaN(date.getTime())`
/// does, so a row without a date shows nothing there rather than a wrong one.
fn how_ago(iso: &str, now: i64) -> String {
    let Some(then) = iso_millis(iso) else {
        return String::new();
    };
    let diff = then - now;
    let seconds = (diff as f64 / 1_000.0).round() as i64;
    let minutes = (diff as f64 / 60_000.0).round() as i64;
    let hours = (diff as f64 / 3_600_000.0).round() as i64;
    let days = (diff as f64 / 86_400_000.0).round() as i64;
    let weeks = (diff as f64 / 604_800_000.0).round() as i64;
    let months = (diff as f64 / 2_629_746_000.0).round() as i64;
    let years = (diff as f64 / 31_556_952_000.0).round() as i64;
    let (count, unit) = if seconds.abs() < 60 {
        (seconds, "second")
    } else if minutes.abs() < 60 {
        (minutes, "minute")
    } else if hours.abs() < 24 {
        (hours, "hour")
    } else if days.abs() < 7 {
        (days, "day")
    } else if weeks.abs() < 4 {
        (weeks, "week")
    } else if months.abs() < 12 {
        (months, "month")
    } else {
        (years, "year")
    };
    let noun = if count.abs() == 1 { unit } else { plural(unit) };
    let count = count.abs();
    if diff < 0 {
        format!("{count} {noun} ago")
    } else {
        format!("in {count} {noun}")
    }
}

/// The one plural `Intl.RelativeTimeFormat` adds to these seven units.
fn plural(unit: &str) -> &'static str {
    match unit {
        "second" => "seconds",
        "minute" => "minutes",
        "hour" => "hours",
        "day" => "days",
        "week" => "weeks",
        "month" => "months",
        _ => "years",
    }
}

/// An ISO-8601 instant in milliseconds, or `None` when it is not one.
///
/// `YYYY-MM-DDTHH:MM:SS`, with an optional fractional part and an optional `Z`,
/// which is the shape every date Modrinth's API publishes. Days are counted from
/// the civil date by the usual 1970-epoch arithmetic, so a leap year lands where
/// it should without a table.
fn iso_millis(iso: &str) -> Option<i64> {
    let bytes = iso.as_bytes();
    if bytes.len() < 19 {
        return None;
    }
    let number = |from: usize, to: usize| -> Option<i64> { iso.get(from..to)?.parse().ok() };
    let (year, month, day) = (number(0, 4)?, number(5, 7)?, number(8, 10)?);
    let (hour, minute, second) = (number(11, 13)?, number(14, 16)?, number(17, 19)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // Howard Hinnant's `days_from_civil`: the proleptic Gregorian day number of a
    // date, which is the whole of what a calendar-aware formatter would buy here.
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_shift = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_shift + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    Some((days * 86_400 + hour * 3_600 + minute * 60 + second) * 1_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A profile with the two projects the fixtures below describe.
    fn sample() -> Profile {
        let user: ModrinthUser = serde_json::from_str(
            r#"{"id":"2REoufqX","username":"jellysquid3","name":null,
                "avatar_url":"https://cdn.modrinth.com/a.png","bio":"","created":"2023-11-13T23:22:36.604990Z"}"#,
        )
        .expect("the user document");
        let projects: Vec<ModrinthUserProject> = serde_json::from_str(
            r#"[{"id":"AANobbMI","slug":"sodium","project_type":"mod","title":"Sodium",
                 "description":"Modern rendering engine","published":"2020-06-15T00:00:00Z","downloads":41000000,
                 "icon_url":"https://cdn.modrinth.com/data/AANobbMI/icon.png"},
                {"id":"hEOCdOgW","slug":"phosphor","project_type":"mod","title":"Phosphor",
                 "description":"Lighting engine","published":"2021-01-03T00:58:54.900351Z","downloads":865848,
                 "icon_url":"https://cdn.modrinth.com/data/hEOCdOgW/icon.png"},
                {"id":"P7dR8mSH","slug":"fabulously-optimized","project_type":"modpack","title":"Fabulously Optimized",
                 "description":"A pack","published":"2022-02-02T00:00:00Z","downloads":1234}]"#,
        )
        .expect("the project list");
        Profile::of(
            user,
            projects,
            v3_types(),
            Vec::new(),
            Err("no avatar on this machine".to_string()),
            Vec::new(),
        )
    }

    /// The same profile with two collections, which is the shape that puts a fourth
    /// tab on the strip.
    ///
    /// Two of FlameFire's own four, with their real ids, names, null descriptions and
    /// null icons, and their real `projects` lengths: one project in *Plugin* and
    /// three in *Carpet*, so the plural message has both arms in it.
    fn with_collections() -> Profile {
        let mut profile = sample();
        profile.collections = collections();
        profile
    }

    /// Two collections as `GET /v3/user/{id}/collections` answers them, parsed
    /// rather than built, so the null fields are the service's own and not a
    /// hand-written `""`.
    fn collections() -> Vec<ModrinthCollection> {
        serde_json::from_str(
            r#"[{"id":"gvaNtekl","user":"P3U9o13d","name":"Plugin","description":null,
                 "icon_url":null,"color":null,"status":"listed",
                 "created":"2026-06-16T17:07:15.101156Z","updated":"2026-06-16T17:07:15.101149Z",
                 "projects":["gBIw3Gvy"]},
                {"id":"3xabYvo8","user":"P3U9o13d","name":"Carpet","description":null,
                 "icon_url":null,"color":null,"status":"listed",
                 "created":"2025-03-29T16:15:46.075350Z","updated":"2025-03-29T16:20:26.673541Z",
                 "projects":["G26sLP13","TQTTVgYE","UHjbX5mk"]}]"#,
        )
        .expect("the collection list")
    }

    /// The v3 type list a fixture is read against, keyed by the ids the v2 fixture
    /// above uses.
    ///
    /// Deliberately *not* the same types the v2 list spells: these are the arrays
    /// `GET /v3/user/{id}/projects` answers for those projects, and the point of the
    /// pairing is that they disagree.
    fn v3_types() -> Vec<ModrinthV3Project> {
        vec![
            v3("AANobbMI", &["mod"]),
            v3("hEOCdOgW", &["mod"]),
            v3("P7dR8mSH", &["modpack"]),
        ]
    }

    /// One v3 document for one project id, trimmed to the two fields read.
    fn v3(id: &str, project_types: &[&str]) -> ModrinthV3Project {
        ModrinthV3Project {
            id: id.to_string(),
            project_types: project_types.iter().map(|kind| kind.to_string()).collect(),
        }
    }

    /// A PNG of one colour, in memory, the way `skin.rs`'s tests make one.
    fn picture(side: u32) -> Vec<u8> {
        let square = ::image::RgbaImage::from_pixel(side, side, ::image::Rgba([10, 20, 30, 255]));
        let mut bytes = Vec::new();
        ::image::DynamicImage::ImageRgba8(square)
            .write_to(&mut std::io::Cursor::new(&mut bytes), ::image::ImageFormat::Png)
            .expect("a PNG in memory");
        bytes
    }

    #[test]
    fn the_two_empty_sentences_are_both_kept_and_only_one_is_reachable_here() {
        // The reference's pair, and the difference between them kept rather than
        // collapsed: one is about somebody else, the other about the reader. Which
        // is unreachable is a measurement rather than a guess -- `view` passes
        // `false`, because this launcher has no Modrinth account to compare ids
        // with.
        assert_eq!(State::empty_sentence(false), "This user has no projects!");
        assert_eq!(State::empty_sentence(true), "You don't have any projects yet.");
        assert_ne!(State::empty_sentence(true), State::empty_sentence(false));
    }

    #[test]
    fn the_header_s_facts_are_the_reference_s_arithmetic_over_the_list() {
        let profile = sample();
        assert_eq!(profile.downloads(), 41_000_000 + 865_848 + 1_234);
        assert_eq!(profile.projects.len(), 3);
        // An account with no display name is drawn by its handle, once.
        assert_eq!(profile.user.display_name(), "jellysquid3");
        assert!(!profile.user.has_separate_username());
        // No bio and something published: the *creator* fallback, not the other.
        assert_eq!(profile.summary(), "A Modrinth creator.");
        // And the join date is the feed's own long format.
        assert_eq!(profile.user.joined_label(), "November 13, 2023");
        // The avatar failed rather than arriving as something undrawable, and the
        // reason travelled beside the profile instead of replacing it.
        assert!(profile.avatar.is_none());
        assert_eq!(profile.note.as_deref(), Some("no avatar on this machine"));
    }

    #[test]
    fn an_avatar_that_arrives_is_cut_into_the_circle_and_one_that_is_not_an_image_is_refused() {
        // `Avatar.vue`'s own `circle`, which is what `UserPageHeader.vue` passes:
        // the picture is fitted to the 96-pixel box and the alpha outside the disc
        // cleared, so what the header draws needs no frame of its own.
        let avatar = crate::avatar::Icon::circle(&picture(64), AVATAR as u32)
            .expect("a 64x64 PNG is a picture");
        // The widget draws from the pixels rather than from the encoded bytes, so a
        // handle built from the file would let the renderer decode it a second time.
        drop(avatar.handle());
        assert!(crate::avatar::Icon::circle(b"this is not a picture", AVATAR as u32).is_none());
        // An avatar *can* arrive after a note: the two fields are set by the same
        // match, so one of them cannot be stale beside the other.
        let user: ModrinthUser =
            serde_json::from_str(r#"{"username":"jelly"}"#).expect("the user document");
        let arrived =
            Profile::of(user, Vec::new(), Vec::new(), Vec::new(), Ok(picture(32)), Vec::new());
        assert!(arrived.avatar.is_some());
        assert!(arrived.note.is_none());
    }

    /// The environment a card's row draws, as its message.
    fn environment_label(tags: &Tags) -> Option<String> {
        tags.environment.as_ref().map(|(_, label)| label.clone())
    }

    /// A project of FlameFire's own six, by the answers
    /// `GET /v2/user/FlameFire/projects` gives for it.
    fn flamefire(
        project_type: &str,
        categories: &str,
        loaders: &str,
        extra: &str,
        environment: &str,
    ) -> ModrinthUserProject {
        serde_json::from_str(&format!(
            r#"{{"id":"x","title":"A project","project_type":"{project_type}",
                "categories":[{categories}],"loaders":[{loaders}],
                "additional_categories":[{extra}],"environment":[{environment}]}}"#
        ))
        .expect("the project")
    }

    #[test]
    fn a_card_s_tag_row_is_the_reference_s_own_composition_of_the_document() {
        // *Zombie Invade 100 Days*, the reference's first card: two categories,
        // one loader, one extra category and a `client_and_server` environment.
        // `maxTags` is `(maxTags || (actions ? 4 : 5)) + (!!environment ? 0 : 1)`
        // = 4, so the two categories and the two loaders all fit and the extra
        // category is the `+1`. Measured on the reference's own capture: six
        // pills, *Client and server Challenging Combat Forge Modpack +1`, at
        // x=217..369, 374..469, 474..544, 549..625, 630..729 and 734..767.
        let pack = flamefire(
            "modpack",
            r#""challenging", "combat""#,
            r#""forge""#,
            r#""multiplayer""#,
            r#""client_and_server""#,
        );
        let tags = card_tags(&pack);
        // The categories alphabetically, then the loaders -- `forge` is one of
        // the six `DEFAULT_LOADER_NAMES` and `mrpack` is not, so *Forge* comes
        // first even though the modpack's own `loaders` lists it after.
        assert_eq!(tags.ids(), ["challenging", "combat", "forge", "mrpack"]);
        assert_eq!(tags.overflow, 1);
        assert_eq!(environment_label(&tags), Some("Client and server".to_string()));
        assert_eq!(
            tags.labels(),
            ["Client and server", "Challenging", "Combat", "Forge", "Modpack", "+1"]
        );

        // *Random Island*: seven tags, an environment of its own, and nothing
        // extra -- so four are drawn and the other three are counted. The
        // loaders' order is the default six first (`fabric`, `forge`,
        // `neoforge`) and the two that are not (`datapack`, `quilt`) after them.
        let island = flamefire(
            "mod",
            r#""minigame", "worldgen""#,
            r#""datapack", "fabric", "forge", "neoforge", "quilt""#,
            "",
            r#""server_only""#,
        );
        let tags = card_tags(&island);
        assert_eq!(tags.ids(), ["minigame", "worldgen", "fabric", "forge"]);
        assert_eq!(tags.overflow, 3);
        assert_eq!(tags.labels(), ["Server", "Minigame", "World Generation", "Fabric", "Forge", "+3"]);

        // *Zombie Invade Nether End*: the one project of the six whose
        // `environment` is empty, so there is no environment pill *and* the
        // reference's `+1` in `maxTags` applies -- five tags would fit where a
        // card with an environment shows four.
        let end = flamefire("mod", r#""mobs""#, r#""datapack""#, r#""minigame""#, "");
        let tags = card_tags(&end);
        assert!(tags.environment.is_none());
        assert_eq!(tags.ids(), ["mobs", "datapack"]);
        assert_eq!(tags.overflow, 1);
        assert_eq!(tags.labels(), ["Mobs", "Data Pack", "+1"]);

        // A document with no tags at all draws no pills rather than an empty
        // one: `empty:hidden` is on the environment's `TagItem`, and a row of
        // nothing is what the reference leaves behind.
        let bare: ModrinthUserProject =
            serde_json::from_str(r#"{"id":"y","title":"Nothing"}"#).expect("the project");
        let tags = card_tags(&bare);
        assert!(tags.labels().is_empty());
        assert_eq!(tags.overflow, 0);
    }

    #[test]
    fn only_a_loader_carries_a_glyph_and_a_count_never_does() {
        // The whole point of this row, and the reason [`ui::tag_height`] takes a
        // boolean: three of the six pills on the reference's first card are
        // twenty-six rows and three are twenty-four, and every one of the
        // twenty-six drew an icon.
        //
        // `TagTagItem.vue:28` is the rule and `ProjectCardTags.vue:56` passes
        // `hide-non-loader-icon`, so on a card a category's `getTagIcon` answer
        // is thrown away. *Client and server* is the third icon and it comes from
        // `ProjectCardEnvironment`, which has no such prop -- and *+1*, which is
        // `TagsOverflow`'s plain `TagItem` with no slot in it, is the fifth pill
        // that stays short.
        let icons = |project: &ModrinthUserProject| -> Vec<(String, Option<Glyph>)> {
            card_tags(project)
                .pills()
                .into_iter()
                .map(|pill| (pill.label, pill.icon))
                .collect()
        };

        // Card 1, *Zombie Invade 100 Days*: y=358..383 for the three that carry a
        // glyph and y=359..382 for the three that do not.
        let pack = flamefire(
            "modpack",
            r#""challenging", "combat""#,
            r#""forge""#,
            r#""multiplayer""#,
            r#""client_and_server""#,
        );
        assert_eq!(
            icons(&pack),
            vec![
                ("Client and server".to_string(), Some(Glyph::Globe)),
                ("Challenging".to_string(), None),
                ("Combat".to_string(), None),
                ("Forge".to_string(), Some(Glyph::TagLoaderForge)),
                ("Modpack".to_string(), Some(Glyph::TagLoaderMrpack)),
                ("+1".to_string(), None),
            ]
        );

        // Card 3, *Zombie Invade Nether End*: no environment pill, so its loader
        // row has five slots rather than four and *Mobs* and *+1* are all that is
        // left beside *Data Pack*.
        let end = flamefire("mod", r#""mobs""#, r#""datapack""#, r#""minigame""#, "");
        assert_eq!(
            icons(&end),
            vec![
                ("Mobs".to_string(), None),
                ("Data Pack".to_string(), Some(Glyph::TagLoaderDatapack)),
                ("+1".to_string(), None),
            ]
        );

        // And the property, over all three of the reference's cards rather than
        // for one: a pill is twenty-six rows exactly when it has a glyph in it.
        for project in [
            flamefire(
                "modpack",
                r#""challenging", "combat""#,
                r#""forge""#,
                r#""multiplayer""#,
                r#""client_and_server""#,
            ),
            flamefire(
                "mod",
                r#""minigame", "worldgen""#,
                r#""datapack", "fabric", "forge", "neoforge", "quilt""#,
                "",
                r#""server_only""#,
            ),
            flamefire("mod", r#""mobs""#, r#""datapack""#, r#""minigame""#, ""),
        ] {
            for tag in card_tags(&project).tags {
                assert_eq!(
                    tag.icon.is_some(),
                    ui::is_loader_tag(&tag.id) && ui::tag_icon(&tag.id).is_some(),
                    "{}: a card's glyph is a loader's, and only if this port draws one",
                    tag.id
                );
                assert_eq!(
                    tag.icon,
                    if ui::is_loader_tag(&tag.id) { ui::tag_icon(&tag.id) } else { None },
                    "{}",
                    tag.id
                );
            }
        }
    }

    #[test]
    fn the_environment_is_the_v3_strings_own_switch_and_nothing_else() {
        // `ProjectCardEnvironment.vue:62-81`: six environments, two icons and one
        // message each, and the two `*_prefers_*` values folded into the pair
        // they prefer.
        let of = |environment: &str| {
            card_tags(&flamefire("mod", "", "", "", &format!("\"{environment}\"")))
        };
        assert_eq!(environment_label(&of("client_or_server")), Some("Client or server".to_string()));
        // Both `*_prefers_*` values are the pair they prefer.
        assert_eq!(
            environment_label(&of("client_or_server_prefers_both")),
            Some("Client or server".to_string())
        );
        assert_eq!(environment_label(&of("client_and_server")), Some("Client and server".to_string()));
        assert_eq!(environment_label(&of("client_only")), Some("Client".to_string()));
        assert_eq!(environment_label(&of("client_only_server_optional")), Some("Client".to_string()));
        assert_eq!(environment_label(&of("server_only")), Some("Server".to_string()));
        assert_eq!(environment_label(&of("server_only_client_optional")), Some("Server".to_string()));
        assert_eq!(environment_label(&of("singleplayer_only")), Some("Singleplayer".to_string()));
        assert_eq!(environment_label(&of("dedicated_server_only")), Some("Dedicated server".to_string()));
        // A value this port does not name draws nothing, which is what the
        // reference's `empty:hidden` does with an environment it cannot map.
        assert_eq!(environment_label(&of("bedrock_only")), None);
        // The globe is the globe: `client-and-server` and `client-or-server` both
        // take `GlobeIcon`, and *Server* takes `ServerIcon`.
        let glyph_of = |environment: &str| of(environment).environment.map(|(glyph, _)| glyph);
        assert_eq!(glyph_of("client_and_server"), Some(Glyph::Globe));
        assert_eq!(glyph_of("client_or_server"), Some(Glyph::Globe));
        assert_eq!(glyph_of("server_only"), Some(Glyph::Server));
        assert_eq!(glyph_of("singleplayer_only"), Some(Glyph::User));
        // And the `client_side`/`server_side` pair is not the fallback: a
        // document with an empty `environment` and `client_side: "optional"`,
        // `server_side: "required"` draws no pill, because `ProjectList.vue`
        // passes `project.environment?.[0]` and never the pair.
        let legacy: ModrinthUserProject = serde_json::from_str(
            r#"{"id":"z","title":"Legacy","client_side":"optional","server_side":"required",
                "environment":[]}"#,
        )
        .expect("the project");
        assert!(card_tags(&legacy).environment.is_none());
    }

    #[test]
    fn a_loader_is_a_tag_the_loader_table_has_and_the_default_six_come_first() {
        // `getTagMessage(tag, 'loader') !== undefined` is `tag.loader.<tag>` being
        // in the generated table, so `minecraft` is a loader (the reference says
        // *Resource Pack*) and `forge` is one too, while `mobs` is a category.
        assert!(is_loader("forge"));
        assert!(is_loader("mrpack"));
        assert!(is_loader("datapack"));
        assert!(!is_loader("mobs"));
        assert!(!is_loader("multiplayer"));
        assert_eq!(crate::locale::loader_label("mrpack"), "Modpack");
        assert_eq!(crate::locale::loader_label("datapack"), "Data Pack");
        assert_eq!(crate::locale::category_label("worldgen"), "World Generation");
        // And a tag neither table has is capitalised rather than dropped, which
        // is `formatTag`'s own fallback.
        assert_eq!(crate::locale::category_label("not-a-tag-yet"), "Not-a-tag-yet");
        assert_eq!(
            sort_tags_for_display(&[
                "quilt".to_string(),
                "worldgen".to_string(),
                "forge".to_string(),
                "datapack".to_string(),
                "minigame".to_string(),
                "neoforge".to_string(),
                "fabric".to_string(),
            ]),
            ["minigame", "worldgen", "fabric", "forge", "neoforge", "datapack", "quilt"]
        );
        // `uniqueSorted` is a `Set`, so a tag listed twice is one pill.
        assert_eq!(unique(&["a".to_string(), "b".to_string(), "a".to_string()]), ["a", "b"]);
    }

    #[test]
    fn a_loader_pill_is_painted_in_its_own_platform_colour() {
        // The measured defect, on the reference's own profile page at 1280x720:
        // *Fabric* is `#DBB69B` there and *Forge* is `#959EEF`, and both were
        // drawn `#96A2B0`. Those two are the dark column of
        // `--color-platform-fabric` and `--color-platform-forge`
        // (`variables.scss:389` and `:391`), which is what the reference's
        // `var(--color-platform-${tag})` resolves to in the look it opens in.
        assert_eq!(
            theme_gen::ink_rgba(Gen::Dark, Ink::PlatformFabric),
            [0xdb, 0xb6, 0x9b, 0xff]
        );
        assert_eq!(theme_gen::ink_rgba(Gen::Dark, Ink::PlatformForge), [0x95, 0x9e, 0xef, 0xff]);
        // The light look carries its own pair, from the same file's `:169` and
        // `:171`, so what is read is a token and not two transcribed hexes.
        assert_eq!(
            theme_gen::ink_rgba(Gen::Light, Ink::PlatformFabric),
            [0x8a, 0x7b, 0x71, 0xff]
        );
        assert_eq!(theme_gen::ink_rgba(Gen::Light, Ink::PlatformForge), [0x5b, 0x61, 0x97, 0xff]);
        // And the ink the other pills keep, which the audit measured as correct
        // on both sides: the categories, *Modpack* and `+1`.
        assert_eq!(theme_gen::ink_rgba(Gen::Dark, INK_SECONDARY), [0x96, 0xa2, 0xb0, 0xff]);
    }

    #[test]
    fn seventeen_platform_colours_sixteen_of_them_reachable_through_is_loader_tag() {
        // The reference's own loader table, `tag-messages.ts:5`, has thirty tags.
        let loaders = [
            "babric", "bta-fabric", "bukkit", "bungeecord", "canvas", "datapack", "fabric",
            "folia", "forge", "geyser", "iris", "java-agent", "legacy-fabric", "liteloader",
            "minecraft", "modloader", "mrpack", "neoforge", "nilloader", "optifine", "ornithe",
            "paper", "purpur", "quilt", "rift", "spigot", "sponge", "vanilla", "velocity",
            "waterfall",
        ];
        assert_eq!(loaders.len(), 30);
        let coloured: Vec<&str> = loaders
            .iter()
            .copied()
            .filter(|tag| platform_ink(tag).is_some())
            .collect();
        assert_eq!(coloured.len(), 17, "the platform tokens moved: {coloured:?}");
        // `ui::is_loader_tag` is `TagTagItem`'s `isLoader`, and it answers true
        // for all thirty. The one that needed care is `bta-fabric`: this port asks
        // the *message id* `tag.loader.<tag>`, and upstream spells that one
        // `tag.loader.bta-babric` for a key spelled `bta-fabric`
        // (`tag-messages.ts:10-11`), so the plain lookup found nothing and cost
        // that tag its glyph, its `BTA (Babric)` label and its platform ink all
        // at once. The exception is named in `ui::is_loader_tag`; this is the test
        // that says it is still there.
        assert!(ui::is_loader_tag("bta-fabric"), "the upstream id is spelled bta-babric");
        assert_eq!(platform_ink("bta-fabric"), Some(Ink::PlatformBtaBabric));
        let reachable: Vec<&str> = coloured
            .iter()
            .copied()
            .filter(|tag| ui::is_loader_tag(tag))
            .collect();
        assert_eq!(reachable.len(), 17, "the reachable platform colours moved: {reachable:?}");
        for tag in loaders {
            if tag != "bta-fabric" {
                assert!(ui::is_loader_tag(tag), "{tag} stopped being a loader tag");
            }
        }
        // The thirteen tags with no token keep `--color-secondary`, and *Modpack*
        // and *Data Pack* are two of them -- both measured as `#96A2B0` on both
        // sides of the audit, so they stay that way.
        for tag in ["mrpack", "datapack", "minecraft", "geyser", "legacy-fabric"] {
            assert!(ui::is_loader_tag(tag), "{tag} is a loader tag");
            assert_eq!(platform_ink(tag), None, "{tag} gained a token");
        }
        // A loader can be coloured and glyphless: *Purpur* and *Quilt* have
        // tokens and are two of the four loader icons the generator refuses, so
        // their pills are 24 rows and are still painted in the platform colour.
        assert_eq!(platform_ink("purpur"), Some(Ink::PlatformPurpur));
        assert_eq!(platform_ink("quilt"), Some(Ink::PlatformQuilt));
        assert_eq!(ui::tag_icon("purpur"), None, "purpur's glyph is still refused");
        assert_eq!(ui::tag_icon("quilt"), None, "quilt's glyph is still refused");
    }

    #[test]
    fn only_a_loader_pill_is_painted_in_a_platform_colour() {
        // `TagTagItem.vue:2` gates the override on `isLoader`, so a category is
        // `--color-secondary` whatever a loader of a similar name has, and the
        // two pills that are not tags at all take the fallback because there is
        // nothing to name a token after.
        let of = |id: Option<&str>, label: &str, icon: Option<Glyph>| {
            let pill = Pill { label: label.to_string(), icon, id: id.map(str::to_string) };
            platform_colour(&pill)
        };
        assert_eq!(of(Some("forge"), "Forge", None), Some(Ink::PlatformForge));
        assert_eq!(of(Some("fabric"), "Fabric", None), Some(Ink::PlatformFabric));
        // A loader with a token but no glyph in it still gets the colour, and
        // still a 24-row pill: *Purpur* and *Quilt* are the two that are both.
        assert_eq!(of(Some("quilt"), "Quilt", ui::tag_icon("quilt")), Some(Ink::PlatformQuilt));
        assert_eq!(ui::tag_height(ui::tag_icon("quilt").is_some()), ui::TAG_HEIGHT);
        // A category, a loader with no token, and the two pills that are not
        // tags: all four are `None`, which the row draws as `INK_SECONDARY`.
        assert_eq!(of(Some("mobs"), "Mobs", None), None);
        assert_eq!(of(Some("multiplayer"), "Multiplayer", None), None);
        assert_eq!(of(Some("mrpack"), "Modpack", ui::tag_icon("mrpack")), None);
        assert_eq!(of(Some("datapack"), "Data Pack", ui::tag_icon("datapack")), None);
        assert_eq!(of(None, "Client and server", Some(Glyph::Globe)), None);
        assert_eq!(of(None, "+1", None), None);
    }

    #[test]
    fn a_card_s_tag_row_paints_its_loaders_and_nothing_else() {
        // The three cards on the reference's own capture, through the row that
        // draws them: *Zombie Invade 100 Days*, *Random Island* and *Zombie
        // Invade Nether End*.
        let inks = |project: &ModrinthUserProject| -> Vec<(String, Ink)> {
            card_tags(project)
                .pills()
                .iter()
                .map(|pill| {
                    (pill.label.clone(), platform_colour(pill).unwrap_or(INK_SECONDARY))
                })
                .collect()
        };
        assert_eq!(
            inks(&flamefire(
                "modpack",
                r#""challenging", "combat""#,
                r#""forge""#,
                r#""multiplayer""#,
                r#""client_and_server""#,
            )),
            vec![
                ("Client and server".to_string(), INK_SECONDARY),
                ("Challenging".to_string(), INK_SECONDARY),
                ("Combat".to_string(), INK_SECONDARY),
                ("Forge".to_string(), Ink::PlatformForge),
                ("Modpack".to_string(), INK_SECONDARY),
                ("+1".to_string(), INK_SECONDARY),
            ]
        );
        // Card 2 is where both audited pills are: *Fabric* and *Forge*, with
        // *Server*, *Minigame*, *World Generation* and `+3` beside them.
        assert_eq!(
            inks(&flamefire(
                "mod",
                r#""minigame", "worldgen""#,
                r#""datapack", "fabric", "forge", "neoforge", "quilt""#,
                "",
                r#""server_only""#,
            )),
            vec![
                ("Server".to_string(), INK_SECONDARY),
                ("Minigame".to_string(), INK_SECONDARY),
                ("World Generation".to_string(), INK_SECONDARY),
                ("Fabric".to_string(), Ink::PlatformFabric),
                ("Forge".to_string(), Ink::PlatformForge),
                ("+3".to_string(), INK_SECONDARY),
            ]
        );
        // Card 3: *Data Pack* is a loader with no token, so the two pills beside
        // *Mobs* are the fallback and the row's only coloured one is absent.
        assert_eq!(
            inks(&flamefire("mod", r#""mobs""#, r#""datapack""#, r#""minigame""#, "")),
            vec![
                ("Mobs".to_string(), INK_SECONDARY),
                ("Data Pack".to_string(), INK_SECONDARY),
                ("+1".to_string(), INK_SECONDARY),
            ]
        );
    }

    #[test]
    fn the_platform_pill_is_the_kit_s_pill_with_another_ink() {
        // Every number in `platform_tag`'s frame is the kit's own, so the three
        // it stands in for cannot drift apart: the height follows the glyph
        // exactly as [`ui::tag_height`] does, the radius is half of it because
        // `rounded-full` is a rule rather than a number, and the padding, gap,
        // icon and label are the four the reference's own `baseClass` names.
        assert_eq!(ui::tag_height(true), ui::TAG_HEIGHT_ICON);
        assert_eq!(ui::tag_height(false), ui::TAG_HEIGHT);
        assert_eq!(ui::TAG_HEIGHT_ICON / 2.0, 13.0);
        assert_eq!(ui::TAG_HEIGHT / 2.0, 12.0);
        assert_eq!(ui::TAG_PAD, 8.0);
        assert_eq!(ui::TAG_GAP, 4.0);
        assert_eq!(ui::TAG_ICON, 16.0);
        assert_eq!(ui::TAG_LABEL_SIZE, 14.0);
        // And the row it sits in is still `CARD_TAG_ROW` tall whatever the pills
        // in it are, which is the measurement the card's geometry was read from.
        assert_eq!(CARD_TAG_ROW, 26.0);
    }

    #[test]
    fn the_strip_only_offers_the_types_the_user_actually_has() {
        let profile = sample();
        // Two mods and a pack, in `PROJECT_TYPE_ORDER`' own order: mods before
        // modpacks, which is *not* Discover's tab order.
        assert_eq!(profile.types(), vec![ProjectType::Mod, ProjectType::Modpack]);
        // And a user with nothing has no filters at all.
        assert!(Profile::default().types().is_empty());
    }

    /// FlameFire's own six projects, as the two documents disagree about them.
    ///
    /// The v2 half is `GET /v2/user/FlameFire/projects` trimmed to the fields this
    /// reads, and the v3 half is `GET /v3/user/P3U9o13d/projects` trimmed to
    /// `project_types`. Both are the live answers, which is the point: v2 calls four
    /// of the six `mod` where v3 calls them `["datapack", "mod"]`.
    fn flamefire_profile(with_v3: bool) -> Profile {
        let user: ModrinthUser = serde_json::from_str(
            r#"{"id":"P3U9o13d","username":"FlameFire","name":null,"avatar_url":"",
                "bio":"Just some things I like.","created":"2023-07-10T00:00:00Z"}"#,
        )
        .expect("the user document");
        let projects: Vec<ModrinthUserProject> = serde_json::from_str(
            r#"[
                {"id":"Lb4GuFOj","title":"LuckyBlock Island","project_type":"mod","downloads":209072},
                {"id":"y02ASFMI","title":"NineBlock","project_type":"mod","downloads":66531},
                {"id":"zXGThYpi","title":"Random Island","project_type":"mod","downloads":14848763},
                {"id":"O6MnUSQJ","title":"Zombie Invade Nether End","project_type":"mod","downloads":99813},
                {"id":"l9m9tuPN","title":"Zombie Invade 100 Days","project_type":"modpack","downloads":14848763},
                {"id":"mO4OAdvy","title":"FlameVPack","project_type":"modpack","downloads":4882}
            ]"#,
        )
        .expect("the v2 project list");
        let projects_v3: Vec<ModrinthV3Project> = serde_json::from_str(
            r#"[
                {"id":"Lb4GuFOj","project_types":["datapack","mod"]},
                {"id":"y02ASFMI","project_types":["datapack","mod"]},
                {"id":"zXGThYpi","project_types":["datapack","mod"]},
                {"id":"O6MnUSQJ","project_types":["datapack"]},
                {"id":"l9m9tuPN","project_types":["modpack"]},
                {"id":"mO4OAdvy","project_types":["modpack"]}
            ]"#,
        )
        .expect("the v3 project list");
        Profile::of(
            user,
            projects,
            if with_v3 { projects_v3 } else { Vec::new() },
            if with_v3 { collections() } else { Vec::new() },
            Err("no avatar".to_string()),
            Vec::new(),
        )
    }

    #[test]
    fn the_strip_is_counted_off_the_v3_array_and_says_data_packs_where_v2_says_mods() {
        // The defect this fixes, measured: v2 calls four of these six projects `mod`,
        // so a strip counted from v2 offers *Mods* and no *Data Packs* -- and the
        // reference's own strip for this account reads *All · Data Packs · Modpacks ·
        // Collections*. The fourth link is the collections route, which answers 200
        // anonymously, so all four are drawn and this fixture carries two of
        // FlameFire's four real collections.
        let profile = flamefire_profile(true);
        assert_eq!(
            profile.types(),
            vec![ProjectType::Datapack, ProjectType::Modpack],
            "PROJECT_TYPE_ORDER's own order: datapack third, modpack fifth"
        );
        // And *not* the v2 answer, which is the pair this replaced.
        let v2_only = flamefire_profile(false);
        assert_eq!(v2_only.types(), vec![ProjectType::Mod, ProjectType::Modpack]);

        // A project contributes the type it *leads* with and not every type it is:
        // *Random Island* is `["datapack", "mod"]` and is counted once, under
        // `datapack`. That is the reading their reference strip proves -- a set would
        // have put *Mods* on it.
        let random = profile.projects.iter().find(|p| p.id == "zXGThYpi").expect("the project");
        assert_eq!(profile.type_of(random), Some(ProjectType::Datapack));
        assert_eq!(random.project_type, "mod", "and v2 still calls it a mod");
    }

    #[test]
    fn a_tab_and_the_list_under_it_read_the_same_type_and_a_missing_v3_falls_back_to_v2() {
        // The strip offers what `types` counted and `shown` filters by `type_of`, so
        // no tab can be offered that filters to nothing. Every tab is either empty or
        // holds at least one project.
        let profile = flamefire_profile(true);
        for kind in profile.types() {
            assert!(!profile.shown(Some(kind)).is_empty(), "{kind:?} is a tab with nothing under it");
        }
        assert_eq!(profile.shown(Some(ProjectType::Datapack)).len(), 4);
        assert_eq!(profile.shown(Some(ProjectType::Modpack)).len(), 2);
        assert_eq!(profile.shown(Some(ProjectType::Mod)).len(), 0, "nothing leads with `mod` here");
        assert_eq!(profile.shown(None).len(), 6, "*All* is still every project");

        // And the fallback is the degradation rather than a hole: a profile whose v3
        // read failed is left with the strip it had, which is wrong for a project of
        // two types rather than absent.
        let v2_only = flamefire_profile(false);
        assert_eq!(v2_only.shown(Some(ProjectType::Mod)).len(), 4);
        assert_eq!(v2_only.shown(Some(ProjectType::Datapack)).len(), 0);
        assert_eq!(v2_only.shown(None).len(), 6);

        // A v3 list that named a project the v2 list does not have joins by id and
        // claims nothing: the ids are the only key either document carries.
        let mut extra = flamefire_profile(true);
        extra.projects_v3.push(v3("not-in-v2", &["shader"]));
        assert_eq!(extra.types(), vec![ProjectType::Datapack, ProjectType::Modpack]);
    }

    #[test]
    fn every_tab_label_is_the_reference_s_plural_capital_message() {
        // `layout.vue:778` formats `getProjectTypeTitleMessage(projectType)` with
        // `{ count: 2 }`, so the label is the plural arm of
        // `project-type.<kind>.capital` -- and those are the same seven words as the
        // category messages `ProjectType::label` carries. Held here so the two tables
        // cannot drift apart silently: a *Data Packs* that became *Datapacks* would
        // still be a plausible-looking strip.
        let expected = [
            (ProjectType::Mod, "Mods"),
            (ProjectType::ResourcePack, "Resource Packs"),
            (ProjectType::Datapack, "Data Packs"),
            (ProjectType::Shader, "Shaders"),
            (ProjectType::Modpack, "Modpacks"),
            (ProjectType::Plugin, "Plugins"),
            (ProjectType::Server, "Servers"),
        ];
        assert_eq!(expected.len(), ProjectType::PROFILE_ORDER.len(), "one row per ordered type");
        for (kind, label) in expected {
            assert_eq!(kind.label(), label, "{kind:?}");
        }
        // The reference's *Collections* is the one link with no project type behind
        // it, and it is the only one whose label is not
        // `getProjectTypeTitleMessage(...)`: `layout.vue:777` gives it
        // `messages.collectionsLabel`, which is `project-type.collection.plural`, so
        // it is *Collections* and not *Collection*. And it is the last of the four,
        // because `PROJECT_TYPE_ORDER` ends with `collection`.
        assert_eq!(Key::ProjectTypeAll.message(), "All");
        assert_eq!(Key::ProjectTypeCollectionPlural.message(), "Collections");
        let order: Vec<&str> = ProjectType::PROFILE_ORDER.iter().map(|kind| kind.token()).collect();
        assert_eq!(order.last(), Some(&"server"), "seven types, none of them a collection");
        assert_eq!(
            PROFILE_TYPE_ORDER_LAST_IN_REFERENCE,
            "collection",
            "PROJECT_TYPE_ORDER's eighth entry, after `server`"
        );
    }

    /// The token `PROJECT_TYPE_ORDER` sorts by, named here rather than reached for.
    ///
    /// `ui/src/utils/project-types.ts:1` is `['mod', 'resourcepack', 'datapack',
    /// 'shader', 'modpack', 'plugin', 'server', 'collection']`, and this tree's
    /// [`ProjectType::PROFILE_ORDER`] is its first seven. The eighth is `'collection'`
    /// and has no variant here, which is exactly why the tab is appended rather than
    /// sorted among the types.
    const PROFILE_TYPE_ORDER_LAST_IN_REFERENCE: &str = "collection";

    #[test]
    fn the_collections_tab_is_the_last_link_and_only_when_there_are_collections() {
        // `layout.vue:763-766`: `catalogProjectTypes(projects)`, then
        // `if (collections.value.length > 0) types.push('collection')`, then
        // `sortProjectTypes`. So the tab's condition is on the list, not the account.
        let with = with_collections();
        assert!(with.has_collections());
        let without = sample();
        assert!(!without.has_collections());

        // The strip is drawn from `navLinks`, so the four links this account has are
        // *All*, one per type, and the collections. Asserted through the labels the
        // strip would carry, in order.
        let labels = |profile: &Profile| -> Vec<String> {
            let mut labels = vec![Key::ProjectTypeAll.message().to_string()];
            labels.extend(profile.types().iter().map(|kind| kind.label().to_string()));
            if profile.has_collections() {
                labels.push(Key::ProjectTypeCollectionPlural.message().to_string());
            }
            labels
        };
        assert_eq!(labels(&with), vec!["All", "Mods", "Modpacks", "Collections"]);
        assert_eq!(
            labels(&without),
            vec!["All", "Mods", "Modpacks"],
            "the same account without collections is one link shorter"
        );
    }

    #[test]
    fn the_strip_appears_for_three_links_and_not_for_two() {
        // `NavTabs v-if="navLinks.length > 2"` counts *links*, and the collections
        // link is one of them: one type plus one collection is *All*, the type and
        // *Collections* -- three, which the reference draws. This is the case that
        // used to draw no strip at all, because the count was over types alone.
        let mut profile = sample();
        profile.projects.retain(|project| project.project_type == "mod");
        profile.projects_v3.retain(|entry| entry.id == "AANobbMI");
        assert_eq!(profile.types(), vec![ProjectType::Mod]);
        assert_eq!(strip_links(&profile), 2, "no strip: *All* and *Mods*");
        profile.collections = collections();
        assert_eq!(strip_links(&profile), 3, "and now there is one");
    }

    /// How many links the strip would carry: *All*, one per type, and the
    /// collections when there are any.
    fn strip_links(profile: &Profile) -> usize {
        1 + profile.types().len() + usize::from(profile.has_collections())
    }

    #[test]
    fn the_collections_tab_selects_the_collection_branch_and_not_the_project_list() {
        // `layout.vue:220` draws `ProjectList` only when the selected type is not
        // `'collection'`; `layout.vue:257` draws the collection grid when it is, or
        // when nothing is selected. So the two branches, not one list plus a
        // decoration.
        assert!(Filter::All.shows_projects());
        assert!(Filter::Type(ProjectType::Mod).shows_projects());
        assert!(!Filter::Collections.shows_projects());
        // `filterProjectsByType` compares the project's own type against the
        // address's, and no project answers `'collection'` -- so even if the list
        // were drawn it would be empty.
        assert_eq!(Filter::Collections.project_type(), None);
        assert_eq!(Filter::All.project_type(), None);
        assert_eq!(Filter::Type(ProjectType::Datapack).project_type(), Some(ProjectType::Datapack));

        // And the tab's press is applied rather than reported, because there is no
        // address for it: `Route::User` carries an `Option<ProjectType>`.
        let mut state = State::new("FlameFire".to_string(), None);
        assert_eq!(state.selected(), Filter::All);
        state.update(Message::Collections);
        assert_eq!(state.selected(), Filter::Collections);
        assert_eq!(state.collections, true, "page state, because there is no address");
        // An address that names no type is *All*, so arriving at `/user/x` from the
        // collections branch lands back on *All* -- and the two fields cannot
        // disagree, because following the address clears the page state.
        state.filter(None);
        assert_eq!(state.selected(), Filter::All);
        assert_eq!(state.collections, false);
        state.filter(Some(ProjectType::Modpack));
        assert_eq!(state.selected(), Filter::Type(ProjectType::Modpack));

        // And the way off the branch, which is a message of its own: `All`'s address
        // is the one the page is already on, so the shell would not move and the
        // press has to be applied here.
        state.update(Message::Collections);
        assert_eq!(state.selected(), Filter::Collections);
        state.update(Message::LeaveCollections(None));
        assert_eq!(state.selected(), Filter::All, "and the reader is not trapped");
        assert_eq!(state.collections, false);
        state.update(Message::Collections);
        state.update(Message::LeaveCollections(Some(ProjectType::Shader)));
        assert_eq!(state.selected(), Filter::Type(ProjectType::Shader));
    }

    #[test]
    fn the_collections_are_drawn_in_the_reference_s_own_order() {
        // `sortedCollections` is `updated` descending with `created` descending as
        // the tie-break. The fixture's *Plugin* was changed last (2026) and *Carpet*
        // before it (2025), which is also the order they arrive in -- so this test
        // is that the sort is not inverted, and the tie-break is the net crate's.
        let profile = with_collections();
        let drawn: Vec<&str> =
            profile.sorted_collections().iter().map(|c| c.name.as_str()).collect();
        assert_eq!(drawn, vec!["Plugin", "Carpet"]);
        assert_eq!(profile.collections[0].project_count(), 1);
        assert_eq!(profile.collections[1].project_count(), 3);
    }

    #[test]
    fn a_collection_card_s_status_line_is_the_reference_s_own_condition() {
        // `canSeeCollectionStatus` is `isSelf || isStaffViewing`, and both are false
        // here, so the four labels are parsed off the document and never drawn. The
        // arm is kept because the condition is the reader's, not the document's, and
        // each of the four is checked against the reference's own `v-else-if` order.
        for status in ["listed", "unlisted", "private", "rejected"] {
            assert!(collection_status(false, status).is_none(), "{status} is not drawn");
            assert!(collection_status(true, status).is_some(), "{status} when the viewer may");
        }
        assert_eq!(
            collection_status(true, "listed").map(|(_, key)| key.message()),
            Some("Public")
        );
        assert_eq!(
            collection_status(true, "unlisted").map(|(_, key)| key.message()),
            Some("Unlisted")
        );
        assert_eq!(
            collection_status(true, "private").map(|(_, key)| key.message()),
            Some("Private")
        );
        assert_eq!(
            collection_status(true, "rejected").map(|(_, key)| key.message()),
            Some("Rejected")
        );
        // A fifth value prints nothing, which is what four `v-else-if`s do.
        assert_eq!(collection_status(true, "draft"), None);
    }

    #[test]
    fn a_collection_with_no_icon_draws_the_placeholder_and_not_the_first_card_s_picture() {
        // Every collection the live service was measured on answers `icon_url: null`
        // -- all four of FlameFire's -- and `Avatar :src="null"` is its placeholder.
        let profile = with_collections();
        for collection in &profile.collections {
            assert!(profile.collection_icon(collection).is_none(), "{}", collection.id);
        }
        // A collection that names one is keyed by that URL, the same way a project's
        // icon is, so two collections never share a picture by position.
        let mut profile = profile;
        profile.collections[0].icon_url = "https://cdn.modrinth.com/a.png".to_string();
        assert!(profile.collection_icon(&profile.collections[0]).is_none(), "nothing fetched");
    }

    #[test]
    fn the_collection_grid_counts_its_columns_the_way_auto_fill_does() {
        // `repeat(auto-fill, minmax(350px, 1fr))` over `gap-3`, at the content
        // width the reference lays that grid out in.
        assert_eq!(collection_columns(COLLECTION_CONTENT), 2, "868 fits two tracks of 350");
        // Three tracks would need 3 x 350 + 2 x 12 = 1074.
        assert_eq!(collection_columns(1074.0), 3);
        assert_eq!(collection_columns(1073.0), 2);
        // And a column too narrow for one still gets one, because a grid with no
        // columns draws nothing.
        assert_eq!(collection_columns(0.0), 1);
        assert_eq!(collection_columns(120.0), 1);
    }

    #[test]
    fn the_collections_empty_state_is_only_reachable_from_the_tab_that_wants_it() {
        // `showCollectionsEmptyState` is `selectedProjectType === 'collection' &&
        // collections.value.length === 0`. The other way round -- a tab for a list
        // that is empty -- never happens, because the tab is not on the strip at all
        // when the list is empty (`layout.vue:765`). So an account with no
        // collections has no fourth tab to be on, and this branch is only reachable
        // for a profile whose collections read failed after the tab was drawn.
        let empty = sample();
        assert!(!empty.has_collections());
        assert_eq!(strip_links(&empty), 3, "*All*, *Mods* and *Modpacks*, and nothing else");
        assert_eq!(State::no_collections_sentence(false), "This user has no collections!");
        // The reader's-own arm, kept beside it for `empty_sentence`'s reason.
        assert_eq!(State::no_collections_sentence(true), "You don't have any collections yet.");
    }

    #[test]
    fn the_all_tab_says_no_projects_only_when_there_is_no_collection_to_show_instead() {
        // `showProjectsEmptyState` (`layout.vue:827`) is three terms, and the third
        // is the one a port drops: `selectedProjectType !== null ||
        // collections.value.length === 0`. This is the case it exists for.
        assert!(shows_projects_empty_state(Filter::All, true, 0) == false);
        assert!(shows_projects_empty_state(Filter::All, false, 0), "no collections: it is shown");
        assert!(shows_projects_empty_state(Filter::All, false, 1) == false);
        // A named type is the other way out: with a type selected the address is what
        // the reader asked for, so an empty list is the answer.
        assert!(shows_projects_empty_state(Filter::Type(ProjectType::Shader), true, 0));
        assert!(shows_projects_empty_state(Filter::Type(ProjectType::Shader), false, 0));
        // And the collections tab is never it: that is `showCollectionsEmptyState`.
        assert!(!shows_projects_empty_state(Filter::Collections, true, 0));
        assert!(!shows_projects_empty_state(Filter::Collections, false, 0));
    }

    #[test]
    fn the_list_is_the_filter_s_own_and_all_is_everything() {
        let profile = sample();
        assert_eq!(profile.shown(None).len(), 3);
        let mods = profile.shown(Some(ProjectType::Mod));
        assert_eq!(mods.len(), 2);
        assert!(mods.iter().all(|project| project.project_type == "mod"));
        assert_eq!(profile.shown(Some(ProjectType::Shader)).len(), 0);
        // `projectUserSorting`: the most-downloaded project first, which is not the
        // order the service answered in (the fixture's is Sodium, Phosphor, then
        // Fabulously Optimized, and Phosphor has the fewest downloads of the two
        // mods). Measured against the reference on FlameFire, whose API answer was
        // ordered by id and whose reference list led with its 14.8M-download pack.
        let titles: Vec<&str> =
            profile.shown(None).iter().map(|project| project.title.as_str()).collect();
        assert_eq!(titles, vec!["Sodium", "Phosphor", "Fabulously Optimized"]);
        let mods: Vec<&str> =
            profile.shown(Some(ProjectType::Mod)).iter().map(|project| project.title.as_str()).collect();
        assert_eq!(mods, vec!["Sodium", "Phosphor"]);
    }

    #[test]
    fn the_page_asks_once_for_the_user_the_address_names() {
        let mut state = State::new("jellysquid3".to_string(), None);
        let Some(first) = state.opening() else {
            panic!("a freshly drawn profile page owes a request");
        };
        assert_eq!(first.user, "jellysquid3");
        assert_eq!(first.round, 1);
        assert_eq!(state.profile, Load::Loading);
        assert_eq!(state.opening(), None, "asked once, and nothing else on the page asks again");
        // The reference's header carries no refresh, so neither does this one: the
        // only control beside the avatar is the overflow, and its press says what
        // is behind it rather than issuing a request.
        assert_eq!(state.update(Message::More), None);
        assert_eq!(state.notice.as_deref(), Some(MORE_NOTICE));
    }

    #[test]
    fn a_number_is_compact_where_the_reference_compacts_it_and_grouped_where_it_does_not() {
        // `PageHeaderMetadataNumberItem` at its default `compact: true`: one
        // fraction digit, which is where the reference's header reads `15.2M`.
        assert_eq!(compact(0, 1), "0");
        assert_eq!(compact(999, 1), "999");
        assert_eq!(compact(1_000, 1), "1K");
        assert_eq!(compact(1_234, 1), "1.2K");
        assert_eq!(compact(15_244_929, 1), "15.2M");
        assert_eq!(compact(999_999_999, 1), "1B");
        assert_eq!(compact(2_400_000_000_000, 1), "2.4T");
        // Two decimals, over the top: `ProjectCardStats`'s own formatter, which is
        // why the reference's first card reads `14.84M` where its header reads
        // `15.2M` for a larger number.
        assert_eq!(compact_stat(0), "0");
        assert_eq!(compact_stat(9_999), "9,999");
        assert_eq!(compact_stat(10_000), "10K");
        assert_eq!(compact_stat(208_900), "208.9K");
        assert_eq!(compact_stat(14_840_000), "14.84M");
        // A round number keeps no decimal at all: `minimumFractionDigits` is 0.
        assert_eq!(compact_stat(15_000_000), "15M");
        assert_eq!(compact(1_050_000, 2), "1.05M");
    }

    #[test]
    fn a_date_is_relative_on_the_units_the_reference_switches_between() {
        // `composables/how-ago.ts`'s own thresholds, in its own order, with the
        // clock fixed so the answers are the same on every run.
        let now = 1_700_000_000_000_i64;
        let ago = |seconds: i64| how_ago(&iso_at(now - seconds * 1_000), now);
        assert_eq!(ago(5), "5 seconds ago");
        assert_eq!(ago(59), "59 seconds ago");
        assert_eq!(ago(60), "1 minute ago");
        assert_eq!(ago(119), "2 minutes ago");
        assert_eq!(ago(3_600), "1 hour ago");
        assert_eq!(ago(86_400), "1 day ago");
        assert_eq!(ago(6 * 86_400), "6 days ago");
        assert_eq!(ago(7 * 86_400), "1 week ago");
        assert_eq!(ago(20 * 86_400), "3 weeks ago");
        assert_eq!(ago(28 * 86_400), "1 month ago");
        assert_eq!(ago(300 * 86_400), "10 months ago");
        assert_eq!(ago(400 * 86_400), "1 year ago");
        // A future date reads the other way round, which is the same formatter.
        assert_eq!(how_ago(&iso_at(now + 3 * 86_400_000), now), "in 3 days");
        // And a date that is not a date says nothing, as `Number.isNaN` does in the
        // composable: a row without one shows nothing there rather than a wrong one.
        assert_eq!(how_ago("", now), "");
        assert_eq!(how_ago("not a date", now), "");
        assert_eq!(how_ago("2023-13-45T00:00:00Z", now), "");
    }

    #[test]
    fn an_iso_instant_is_read_as_utc_and_the_epoch_is_where_it_belongs() {
        assert_eq!(iso_millis("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(iso_millis("1970-01-01T00:00:01Z"), Some(1_000));
        // A leap day, which is the case a hand-rolled day count gets wrong.
        assert_eq!(
            iso_millis("2024-03-01T00:00:00Z").unwrap_or(0) - iso_millis("2024-02-29T00:00:00Z").unwrap_or(0),
            86_400_000
        );
        // The reference's own fixtures: jellysquid3 joined on 2023-11-13 and Sodium
        // published on 2020-06-15.
        assert_eq!(iso_millis("2023-11-13T23:22:36.604990Z"), Some(1_699_917_756_000));
        assert_eq!(iso_millis("2020-06-15T00:00:00Z"), Some(1_592_179_200_000));
        // A fractional part and a missing `Z` are both the same instant.
        assert_eq!(iso_millis("2020-06-15T00:00:00"), iso_millis("2020-06-15T00:00:00Z"));
    }

    /// An instant `millis` after the epoch, in the shape Modrinth's API publishes.
    fn iso_at(millis: i64) -> String {
        let seconds = millis.div_euclid(1_000);
        let days = seconds.div_euclid(86_400);
        let rest = seconds.rem_euclid(86_400);
        // 1970-01-01 plus `days`, through the civil-date arithmetic in reverse.
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let day_of_era = z - era * 146_097;
        let year_of_era =
            (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let year = year_of_era + era * 400;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let shifted = (5 * day_of_year + 2) / 153;
        let day = day_of_year - (153 * shifted + 2) / 5 + 1;
        let month = if shifted < 10 { shifted + 3 } else { shifted - 9 };
        let year = if month <= 2 { year + 1 } else { year };
        format!(
            "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
            rest / 3_600,
            rest % 3_600 / 60,
            rest % 60
        )
    }

    #[test]
    fn an_answer_to_a_question_the_page_has_replaced_is_dropped() {
        let mut state = State::new("jellysquid3".to_string(), None);
        let Some(_) = state.opening() else {
            panic!("a request");
        };
        let stale = Message::Found {
            round: 0,
            result: Ok(Box::new(Profile::default())),
        };
        assert_eq!(state.update(stale), None);
        assert_eq!(state.profile, Load::Loading, "the answer to a replaced question is not drawn");
        let answer = Message::Found {
            round: 1,
            result: Ok(Box::new(sample())),
        };
        state.update(answer);
        assert_eq!(state.profile.ready().map(|profile| profile.projects.len()), Some(3));
    }

    #[test]
    fn a_profile_that_could_not_be_read_is_a_reason_rather_than_an_empty_header() {
        // The failure mode this page exists to avoid: a user the service refuses and
        // a user with nothing look the same if a reason is swallowed.
        let mut state = State::new("nobody".to_string(), None);
        let _ = state.opening();
        state.update(Message::Found {
            round: 1,
            result: Err("GET /v2/user/nobody failed: HTTP 404".to_string()),
        });
        assert_eq!(
            state.profile.failure(),
            Some("GET /v2/user/nobody failed: HTTP 404")
        );
    }

    #[test]
    fn a_filter_moves_with_the_address_and_does_not_ask_again() {
        let mut state = State::new("jellysquid3".to_string(), None);
        let _ = state.opening();
        state.update(Message::Found { round: 1, result: Ok(Box::new(sample())) });
        // The strip's own press is reported to the shell rather than applied here;
        // what the page does is follow the address it comes back on.
        assert_eq!(state.update(Message::Filter(Some(ProjectType::Modpack))), None);
        state.filter(Some(ProjectType::Modpack));
        assert_eq!(state.project_type, Some(ProjectType::Modpack));
        assert_eq!(state.profile.ready().map(|profile| profile.shown(state.project_type).len()), Some(1));
        assert_eq!(state.opening(), None, "another filter is not another document");
    }

    #[test]
    fn the_page_draws_in_every_theme_and_every_state_it_can_be_in() {
        // The four arms of the load, plus the two things that are about an answer
        // that *did* arrive: an avatar in the header, and a notice above it. A page
        // whose header panicked on one of them would panic on somebody's machine.
        let store = Store::default();
        let idle = State::new("jelly".to_string(), None);
        let mut waiting = idle.clone();
        let _ = waiting.opening();
        let mut loaded = idle.clone();
        let _ = loaded.opening();
        loaded.update(Message::Found { round: 1, result: Ok(Box::new(sample())) });
        let mut filtered = loaded.clone();
        filtered.filter(Some(ProjectType::Modpack));
        let mut with_avatar = loaded.clone();
        if let Load::Ready(profile) = &mut with_avatar.profile {
            profile.avatar = crate::avatar::Icon::circle(&picture(32), AVATAR as u32);
            profile.note = None;
        }
        let mut failed = idle.clone();
        let _ = failed.opening();
        failed.update(Message::Found { round: 1, result: Err("no such user".to_string()) });
        let mut noticed = loaded.clone();
        noticed.notice = Some("something the page could not do".to_string());
        let mut empty = loaded.clone();
        if let Load::Ready(profile) = &mut empty.profile {
            profile.projects.clear();
        }
        let states = [&idle, &waiting, &loaded, &filtered, &with_avatar, &failed, &noticed, &empty];
        for state in states {
            for theme in Gen::ALL {
                drop(view(*theme, state, &store));
            }
        }
    }

    #[test]
    fn the_empty_list_says_the_reference_s_sentence_rather_than_offering_a_filter() {
        // A user with nothing: no strip (there is no type to filter by) and the
        // sentence. The strip's absence is the part worth asserting, because a strip
        // built from an empty list would draw *All* alone.
        let profile = Profile::default();
        assert!(profile.types().is_empty());
        assert!(filter_strip(Gen::Dark, &State::new("jelly".to_string(), None), &profile).is_none());
        // And a single type is not enough for a strip either: *All* beside it would
        // be two links, and the reference draws three at the least.
        let mut one_type = sample();
        one_type.projects.retain(|project| project.project_type == "mod");
        assert_eq!(one_type.types(), vec![ProjectType::Mod]);
        assert!(filter_strip(Gen::Dark, &State::new("jelly".to_string(), None), &one_type).is_none());
        // Two, and the strip is there.
        assert!(filter_strip(Gen::Dark, &State::new("jelly".to_string(), None), &sample()).is_some());
    }

    #[test]
    fn the_card_is_the_height_the_reference_measures_and_not_the_one_the_box_asks_for() {
        // `ProjectCard.vue`'s list card measures 142 including its two 1-pixel
        // borders and its content box y=276..384 *inclusive*, so the box is 109 and
        // the padding above it is 17: iced's border is drawn inside the bounds, so
        // it eats a pixel of padding above the content and none below, and
        // seventeen above with sixteen below is the pair that puts the content
        // where the reference's is. 17 + 109 + 16 is the 142.
        assert_eq!(CARD_CONTENT, 109.0);
        assert_eq!(CARD_PAD_TOP, ui::CARD_PAD + 1.0);
        // The card's own one-pixel border is inside its bounds here and outside
        // them in CSS, so the top padding carries it and the right padding
        // carries it back: seventeen on the right is what puts the card's content
        // box's right edge on 938 rather than 939.
        assert_eq!(CARD_PAD_RIGHT, ui::CARD_PAD + 1.0);
        assert_eq!(CARD_PAD_TOP + CARD_CONTENT + ui::CARD_PAD, 142.0);
        assert_eq!(CARD_ICON, 100.0);
        // The right-hand column's own rows: the button's `h-9`, a `gap-y-2` and an
        // `mt-3` down to `__stats`, the 20-pixel stats row, a `gap-3`, and the
        // 20-pixel date row. They come to 108 of the content box's 109 -- the
        // ninth pixel is the one the tags row's last row takes -- which is why the
        // column needs no offset of its own: the button lands at the content box's
        // top and the date at its bottom.
        assert_eq!(
            ui::Size::Md.height() + CARD_STATS_LEAD + CARD_STAT_ICON + CARD_STATS_GAP + CARD_STAT_ICON,
            CARD_CONTENT - 1.0
        );
        // The tags row is the reference's own 26 and it ends one row above the
        // content box's bottom, which is where `mt-auto` leaves it once the grid's
        // own bottom row is counted. Measured against the reference's first card:
        // its three 26-row pills are y=358..383 and its three 24-row pills are
        // y=359..382, so the row's first row is 358 = the box's top (276) + this.
        assert_eq!(277.0 + CARD_TAGS_TOP, 358.0, "the tags row starts at y=358");
        assert_eq!(ui::tag_height(true), 26.0, "the row is as tall as its tallest pill");
        // And `items-center` is what puts the shorter pills one row lower inside
        // it: a row laid out from the top would draw *Challenging* at y=358
        // against the reference's y=359.
        assert_eq!(
            ui::tag_height(false) + (ui::TAG_HEIGHT_ICON - ui::TAG_HEIGHT) / 2.0,
            25.0,
            "a 24-row pill centred in a 26-row row ends one row before its bottom"
        );
        // And the kit's own 24-pixel pill still sits inside that 26-pixel row.
        assert!(CARD_TAGS_TOP + ui::TAG_HEIGHT <= CARD_CONTENT, "the tags row is inside the box");
        // And the icon column is the indent the tags row starts at, with the
        // grid's own `gap-x-3` after it -- which together put the first pill at the
        // card's left + 129, the reference's x=217. Indenting by the sum *and*
        // spacing again is what put ours at x=229.
        assert_eq!(CARD_ICON + CARD_GAP_X, 112.0);
        assert_eq!(CARD_ICON + CARD_GAP_X + 17.0, 129.0);
        // And `line-clamp-2` is two of the summary's own lines.
        assert_eq!(CARD_SUMMARY_LINE * 2.0, 36.0);
        // The room a card's summary has to stop in, read off the reference's own
        // second card: an 868-pixel card less its `border-1px` and its `p-4` is an
        // 834-pixel content box, the icon column takes 100 of it, three `gap-x-3`
        // take 36, and the span the two right-hand `auto` columns make owes the
        // 189-pixel *Install to instance* button all but the one `gap-x-3` that
        // spans it. What is left is the `1fr` column, and 521 is the width that
        // carries *A skyblock* to x=737 and stops before *with*.
        assert_eq!(
            868.0 - 2.0 - 2.0 * ui::CARD_PAD - CARD_ICON - 3.0 * CARD_GAP_X - (189.0 - CARD_GAP_X),
            521.0
        );
        // And on the first card, where the 94-pixel button is narrower than the
        // 153-pixel stats line, the stats govern instead: the third track is
        // nothing, the span is a `gap-x-3` more than the stats, and the `1fr`
        // column comes out twenty-four wider than it is on the second -- which is
        // what puts the button flush right at the reference's x=845 with room for
        // a word the reference's own line does not take.
        assert_eq!(
            868.0 - 2.0 - 2.0 * ui::CARD_PAD - CARD_ICON - 3.0 * CARD_GAP_X - 153.0,
            545.0
        );
        // The rule-to-strip gap is `NormalPage`'s `gap-y-4` rather than the
        // `gap-y-3` the rest of the page is spaced with: measured, the
        // reference's rule is y=184 and the strip's border y=201, and ours drew
        // twelve rows there against its sixteen. The tag row's own `gap-1` and
        // its height are the same two measurements read off the pills.
        assert_eq!(HEADER_STRIP_GAP, GAP + 4.0);
        assert_eq!(HEADER_STRIP_GAP, 16.0);
        assert_eq!(CARD_TAG_GAP, 4.0);
        assert_eq!(CARD_TAG_ROW, 26.0);
        // The summary carries no size class, so it inherits the body size on the
        // stylesheet's own 18.4-pixel line, which paints as 18 -- measured, the
        // reference's two `6` digits in card two's summary are 461..472 and
        // 479..490, eighteen rows apart with identical profiles. Sixteen and not
        // fourteen: Inter's em dash is `1.0000 em` of ink, and card two's summary
        // ends in a pair of them across thirty-two solid pixels.
        assert_eq!(CARD_SUMMARY, 16.0);
        assert_eq!(CARD_SUMMARY_LINE, 18.0);
        // The metadata row's words are `leading-none`, which is `line-height: 1`, so
        // its line is its own size -- the one site on this page where that is not
        // the inherited `1.15` of [`HEADER_SUMMARY_LINE`], and the reason the
        // metadata band moved 320 pixels closer to the reference when it was set.
        assert_eq!(METADATA_LINE, HEADER_TEXT);
        // Both empty sentences are `EmptyState`'s *heading*, a `text-2xl` span.
        assert_eq!(EMPTY_HEADING, 24.0);
        assert_eq!(EMPTY_HEADING_LINE, 32.0);
        // A collection card's name is the only line on it that names a size, so the
        // other four are the body size on the inherited line rather than a
        // `text-sm` nobody wrote.
        assert_eq!(COLLECTION_LABEL, HEADER_TEXT);
        assert_eq!(COLLECTION_LABEL_LINE, HEADER_SUMMARY_LINE);
        assert_ne!(
            COLLECTION_NAME, COLLECTION_LABEL,
            "only the name names a size"
        );
    }

    #[test]
    fn an_icon_is_looked_up_by_the_url_its_project_carries() {
        // `avatar::Fetched` is keyed by URL because that is the only thing a
        // project document says about its own icon, and a lookup that had to be
        // matched to a project a second time would be a second set of rules.
        let mut profile = sample();
        let url = profile.projects[0].icon_url.clone();
        assert!(profile.icon(&profile.projects[0]).is_none(), "no icon has arrived yet");
        profile.icons.push(crate::avatar::Fetched {
            url: url.clone(),
            icon: crate::avatar::Icon::of(&picture(64), crate::avatar::ICON_SIDE)
                .expect("a picture"),
        });
        assert!(profile.icon(&profile.projects[0]).is_some());
        // A project whose URL nothing answered for draws the same row: the box is
        // reserved either way, which is what keeps a card from reflowing when one
        // icon is late.
        assert!(profile.icon(&profile.projects[1]).is_none());
        let stray: ModrinthUserProject = serde_json::from_str(
            r#"{"id":"y","title":"Other","project_type":"mod","icon_url":"https://example.invalid/x.png"}"#,
        )
        .expect("the project");
        assert!(profile.icon(&stray).is_none());
        assert_eq!(CARD_ICON, crate::avatar::ICON_SIDE as f32);
    }

    #[test]
    fn every_project_row_draws_in_every_theme() {
        for theme in Gen::ALL {
            for project in sample().projects.iter() {
                drop(project_row(*theme, &sample(), project));
            }
            // A row whose project names no type this tree knows still draws: the tag
            // is the part that is absent, not the row.
            let odd: ModrinthUserProject = serde_json::from_str(
                r#"{"id":"x","title":"Something","project_type":"minecraft_java_server"}"#,
            )
            .expect("the project");
            drop(project_row(*theme, &sample(), &odd));
        }
    }
}

