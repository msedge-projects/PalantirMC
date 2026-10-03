//! A user's profile: `pages/User.vue` and the `UserProfilePageLayout` it renders,
//! at `/user/:user/:projectType?`.
//!
//! The reference's page is a header -- the avatar, the name and handle, a summary,
//! and three facts: how many projects, how many downloads between them, and when
//! the account joined -- over a strip of that user's project types and the list
//! under it. The header's facts and the whole list come from Modrinth's
//! *published* API: `GET /v2/user/{name}` and `GET /v2/user/{id}/projects`. That is
//! what makes this page real without a Modrinth session, and it is also the limit
//! of it: the two halves of the reference's page that are read through its internal
//! v3 user service -- collections and organizations -- stay named as absent
//! (G105 measured that they are unreachable from this tree) rather than drawn as
//! empty lists somebody might believe.
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
use palantir_net::engine::modrinth::ModrinthV3Project;
use palantir_net::modrinth::{ModrinthUser, ModrinthUserProject};

use super::overlay::Stack;
use crate::icon;
use crate::icons_gen::Glyph;
use crate::page::{self, Load, GAP};
use crate::route::ProjectType;
use crate::store::Store;
use crate::style::{heading, medium, semibold, INK_CONTRAST, INK_DEFAULT, INK_SECONDARY};
use crate::text_gen::{self, Key};
use crate::theme_gen::{self, Theme as Gen};
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
/// The line the header's own summary is set on.
///
/// `page-header/index.vue:20` gives the summary no size class, so it inherits the
/// body size (16) and the stylesheet's `line-height: 1.15`, which is 18.4 pixels
/// of CSS and 18 painted -- the same reading [`crate::ui::NAV_LABEL_LINE`] records
/// for a tab label. Measured against the reference: its summary's ink occupies
/// y=115..129 and the metadata row begins 18 pixels below the title's own block,
/// which only works on an 18-pixel line.
const HEADER_SUMMARY_LINE: f32 = 18.0;
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

/// `gap-x-3` and `gap-y-2` on a project card's own grid.
const CARD_GAP_X: f32 = 12.0;
const CARD_GAP_Y: f32 = 8.0;
/// The room between a card's `1fr` info column and its right-hand columns.
///
/// `grid-template-columns: auto 1fr auto auto` with `gap-x-3` puts *three* gaps
/// across a card: icon | info | (empty `actions`/`dummy`) | stats. The third
/// column is `auto` and holds nothing, because `__actions` spans columns three
/// and four and pushes its button to the right with `ml-auto` -- so the space a
/// card's summary actually has to stop in is two `gap-x-3`, not one.
const CARD_COL_GAP: f32 = 24.0;
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
/// `text-base` on a card's summary.
///
/// `ProjectCard.vue:120`'s list layout gives `.project-card-summary` no size class
/// at all, so it inherits the body size; the `@container (width < 550px)` block is
/// the only thing that would make it `text-sm`, and an 868-pixel card is nowhere
/// near it. Measured on the reference's own first card: the summary's ink is 16
/// pixels tall on an 18-pixel line.
const CARD_SUMMARY: f32 = 16.0;
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
/// Where the tags row starts inside the content box: `360 - 276 = 84`, measured as
/// the *bottom* of the row, which is where `mt-auto` puts it -- the row ends flush
/// with the content box and [`ui::tag`] is 24 pixels tall inside the reference's
/// 26-pixel row (the reference's carries an `h-4` icon; see the notes).
const CARD_TAGS_TOP: f32 = 83.0;

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

/// One user's profile: their own document, the projects they own, and their
/// avatar.
///
/// Assembled from three answers rather than one, which is why it is a type of its
/// own: the reference reads the same three (`useQuery` for the user, for their
/// projects, and an `img` for the avatar) and the page draws the union.
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
    /// wrong about it -- see [`Profile::type_of`].
    pub fn of(
        user: ModrinthUser,
        projects: Vec<ModrinthUserProject>,
        projects_v3: Vec<ModrinthV3Project>,
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
        Profile { user, projects, projects_v3, avatar, note, icons }
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
    pub project_type: Option<ProjectType>,
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
        State { user, project_type, profile: Load::Idle, notice: None, round: 0 }
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
    pub fn filter(&mut self, project_type: Option<ProjectType>) {
        self.project_type = project_type;
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
    /// Modrinth credential -- a decision rather than a gap (G105 measured what one
    /// would reach, G118 is where the launcher decides not to want it) -- so the
    /// reader's-own arm stays unreachable and the page draws the other one. Both
    /// are the reference's copy, and the comparison it is missing is the whole of
    /// what would select between them; the arm is kept rather than deleted so that
    /// the page is still the reference's page if that decision is ever reversed.
    pub fn empty_sentence(own_profile: bool) -> &'static str {
        if own_profile {
            Key::ProfileLabelNoProjectsAuthDescription.message()
        } else {
            Key::ProfileLabelNoProjects.message()
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
    let shown = profile.shown(state.project_type);
    if shown.is_empty() {
        // The reference's own empty state, and the same sentence for a user with no
        // projects at all and for one with none of the type being looked at -- the
        // reference draws `profile.label.no-projects` for both.
        blocks.push(ui::card(
            theme,
            text(State::empty_sentence(false))
                .size(16.0)
                .font(heading())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
        ));
    } else {
        let mut list = column![].spacing(LIST_GAP).width(Length::Fill);
        for project in shown {
            list = list.push(project_row(theme, profile, project));
        }
        blocks.push(list.into());
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
                        .font(medium())
                        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
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
/// (`NavTabs v-if="navLinks.length > 2"`). The *Collections* link the reference adds
/// when a user has collections is not here: collections are read through the v3
/// user service, which this tree cannot reach (G105).
///
/// Each label is `getProjectTypeTitleMessage(projectType)` formatted with
/// `{ count: 2 }` -- the *capital* messages, plural -- which are the same seven words
/// as the category messages [`ProjectType::label`] carries.
fn filter_strip<'a>(
    theme: Gen,
    state: &State,
    profile: &Profile,
) -> Option<Element<'a, Message>> {
    let types = profile.types();
    if types.len() < 2 {
        return None;
    }
    let mut keys: Vec<&'static str> = vec![ALL_TAB_KEY];
    let mut labels: Vec<(String, bool)> = vec![(
        Key::ProjectTypeAll.message().to_string(),
        state.project_type.is_none(),
    )];
    for kind in &types {
        // The keys and the order are one list, so the index of the type in
        // `PROFILE_ORDER` is the index of its name here -- and a type the order does
        // not know never reaches the strip at all, because `Profile::types` is built
        // from that same list.
        if let Some(index) = ProjectType::PROFILE_ORDER.iter().position(|known| known == kind) {
            keys.push(TYPE_TAB_KEYS[index]);
            labels.push((kind.label().to_string(), state.project_type == Some(*kind)));
        }
    }
    Some(ui::tabs(theme, &keys, &labels, |index| {
        let chosen = if index == 0 { None } else { types.get(index - 1).copied() };
        Message::Filter(chosen)
    }))
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
/// Every landmark below is measured off `/tmp/ref/user-ref.png` at the
/// reference's own 1280x720: the card is x=88..955 and y=259..400 (868 x 142), its
/// content box is y=276..384, the icon is y=276..375, the *Install* button is
/// y=276..311, the stats are y=334..349, the tags are y=358..383 and the date is
/// y=365..382.
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
    // `__stats`: downloads and then followers on one line, the date under it, the
    // pair right-aligned against the card's own right edge.
    let stats = column![]
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
    let stats = if when.is_empty() {
        stats
    } else {
        stats.push(stat(theme, date_key.0, &when))
    };
    // `__actions` and `__stats` are the grid's third and fourth columns, both
    // flush right, so they share one column here and one column's width: the
    // wider of the two, which is what an `auto` track takes. Measured on the
    // reference's first card: the button's ring is 94 pixels and the stats' ink
    // 152, so the track is 152 and the button sits at x=845..938.
    let right = column![]
        .spacing(CARD_STATS_LEAD)
        .align_items(Alignment::End)
        // `User.vue`'s `#project-actions` slot: an outlined brand button, a
        // download icon for a pack and a plus for anything else.
        .push(install_button(theme, ui::scoped(INSTALL_KEY, &project.id), project))
        .push(stats);
    let head = row![]
        .width(Length::Fill)
        // Two `gap-x-3`: the grid's third column is `auto` and empty, because
        // `__actions` spans columns three and four and pushes itself right.
        .spacing(CARD_COL_GAP)
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
    pub tags: Vec<String>,
    /// How many tags the `+N` pill is standing in for.
    pub overflow: usize,
}

impl Tags {
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
        let mut labels: Vec<String> = Vec::with_capacity(self.tags.len() + 2);
        if let Some((_, label)) = &self.environment {
            labels.push(label.clone());
        }
        labels.extend(self.tags.iter().map(|tag| {
            if is_loader(tag) {
                crate::locale::loader_label(tag)
            } else {
                crate::locale::category_label(tag)
            }
        }));
        if self.overflow > 0 {
            labels.push(format!("+{}", self.overflow));
        }
        labels
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
        tags: tags[..shown].to_vec(),
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
            (Glyph::MonitorSmartphone, Key::ProjectCardEnvironmentClient)
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
/// The pills themselves are [`ui::tag`]'s and their labels are [`Tags::labels`],
/// so what this adds is the row and nothing else.
fn tags_row<'a>(theme: Gen, project: &'a ModrinthUserProject) -> Element<'a, Message> {
    let mut row = row![].spacing(CARD_TAG_GAP).align_items(Alignment::Center);
    for label in card_tags(project).labels() {
        row = row.push(ui::tag(theme, &label));
    }
    row.into()
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
            Err("no avatar on this machine".to_string()),
            Vec::new(),
        )
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
        let arrived = Profile::of(user, Vec::new(), Vec::new(), Ok(picture(32)), Vec::new());
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
        assert_eq!(tags.tags, ["challenging", "combat", "forge", "mrpack"]);
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
        assert_eq!(tags.tags, ["minigame", "worldgen", "fabric", "forge"]);
        assert_eq!(tags.overflow, 3);
        assert_eq!(tags.labels(), ["Server", "Minigame", "World Generation", "Fabric", "Forge", "+3"]);

        // *Zombie Invade Nether End*: the one project of the six whose
        // `environment` is empty, so there is no environment pill *and* the
        // reference's `+1` in `maxTags` applies -- five tags would fit where a
        // card with an environment shows four.
        let end = flamefire("mod", r#""mobs""#, r#""datapack""#, r#""minigame""#, "");
        let tags = card_tags(&end);
        assert!(tags.environment.is_none());
        assert_eq!(tags.tags, ["mobs", "datapack"]);
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
            Err("no avatar".to_string()),
            Vec::new(),
        )
    }

    #[test]
    fn the_strip_is_counted_off_the_v3_array_and_says_data_packs_where_v2_says_mods() {
        // The defect this fixes, measured: v2 calls four of these six projects `mod`,
        // so a strip counted from v2 offers *Mods* and no *Data Packs* -- and the
        // reference's own strip for this account reads *All · Data Packs · Modpacks ·
        // Collections*. The reference's *Collections* is out of reach (G105), so what
        // is left has to be the other three.
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
        // it, and it is out of reach here -- so the strip is one shorter than theirs
        // for an account that has collections, by decision rather than by oversight.
        assert_eq!(Key::ProjectTypeAll.message(), "All");
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
        // The tags row is the reference's own 26, `mt-auto` to the content box's
        // bottom, so it starts one pixel above our 24-pixel pill's own bottom.
        assert_eq!(CARD_TAGS_TOP + 26.0, CARD_CONTENT);
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
        // Two `gap-x-3` between the `1fr` column and the right-hand ones, because
        // the grid's third `auto` column is empty.
        assert_eq!(CARD_COL_GAP, CARD_GAP_X * 2.0);
        // The rule-to-strip gap is `NormalPage`'s `gap-y-4` rather than the
        // `gap-y-3` the rest of the page is spaced with: measured, the
        // reference's rule is y=184 and the strip's border y=201, and ours drew
        // twelve rows there against its sixteen. The tag row's own `gap-1` and
        // its height are the same two measurements read off the pills.
        assert_eq!(HEADER_STRIP_GAP, GAP + 4.0);
        assert_eq!(HEADER_STRIP_GAP, 16.0);
        assert_eq!(CARD_TAG_GAP, 4.0);
        assert_eq!(CARD_TAG_ROW, 26.0);
        // The summary inherits `text-base` on the stylesheet's own 18.4-pixel line,
        // which paints as 18 -- measured, the reference's two summary lines are 18
        // pixels apart.
        assert_eq!(CARD_SUMMARY, 16.0);
        assert_eq!(CARD_SUMMARY_LINE, 18.0);
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
