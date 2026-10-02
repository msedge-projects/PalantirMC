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

use iced::mouse::Interaction;
use iced::widget::container;
use iced::widget::{column, mouse_area, row, Space};
use iced::{Alignment, Border, Element, Length, Padding};
use palantir_net::modrinth::{ModrinthUser, ModrinthUserProject};

use crate::icon;
use crate::icons_gen::Glyph;
use crate::page::{self, Load, GAP};
use crate::route::ProjectType;
use crate::store::Store;
use crate::style::{heading, medium, semibold, INK_CONTRAST, INK_SECONDARY};
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
/// `gap-3` on `ProjectCardList`, between the cards of the list.
const LIST_GAP: f32 = 12.0;
/// `text-xl` on `ProjectCardTitle`, which is the list layout's own size.
const CARD_TITLE: f32 = 20.0;
/// `gap-3` between a card's stats and its date, `gap-3` on the stats row.
const CARD_STATS_GAP: f32 = 12.0;

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
    /// Their avatar, when it could be fetched and decoded -- already rounded into
    /// the circle `UserPageHeader.vue` asks for, by [`crate::avatar`].
    pub avatar: Option<crate::avatar::Icon>,
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
    pub fn of(
        user: ModrinthUser,
        projects: Vec<ModrinthUserProject>,
        avatar: Result<Vec<u8>, String>,
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
        Profile { user, projects, avatar, note }
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
    /// unused tab is never offered. The order is not [`ProjectType::TABS`]':
    /// `PROJECT_TYPE_ORDER` in `ui/src/utils/project-types.ts` puts mods first and
    /// modpacks fifth, where Discover's tabs put modpacks first.
    ///
    /// A project whose `project_type` this tree cannot name is counted by the
    /// header and drawn under *All*, and no filter claims it -- which is what the
    /// reference does with a type its own order does not list.
    pub fn types(&self) -> Vec<ProjectType> {
        ProjectType::PROFILE_ORDER
            .iter()
            .copied()
            .filter(|kind| {
                self.projects.iter().any(|project| {
                    ProjectType::from_token(&project.project_type) == Some(*kind)
                })
            })
            .collect()
    }

    /// The projects one filter shows.
    ///
    /// `None` is the strip's *All* tab, which is the reference's own reading of an
    /// address with no type in it. The filter is compared against the project's own
    /// type string, as the reference's `filterProjectsByType` does.
    pub fn shown(&self, filter: Option<ProjectType>) -> Vec<&ModrinthUserProject> {
        self.projects
            .iter()
            .filter(|project| match filter {
                Some(kind) => ProjectType::from_token(&project.project_type) == Some(kind),
                None => true,
            })
            .collect()
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
fn loaded<'a>(theme: Gen, state: &'a State, profile: &'a Profile) -> Element<'a, Message> {
    let mut blocks: Vec<Element<'a, Message>> = vec![header(theme, profile)];
    if let Some(strip) = filter_strip(theme, state, profile) {
        blocks.push(strip);
    }
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
            list = list.push(project_row(theme, project));
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
                                .push(
                                    text(profile.summary().to_string())
                                        .size(HEADER_TEXT)
                                        .font(medium())
                                        .style(iced::theme::Text::Color(theme_gen::ink(
                                            theme,
                                            INK_SECONDARY,
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
    container(block)
        .width(Length::Fill)
        .padding(Padding { top: 0.0, right: 0.0, bottom: HEADER_PAD_BOTTOM, left: 0.0 })
        .style(move |_theme: &iced::Theme| iced::widget::container::Appearance {
            border: Border {
                color: theme_gen::ink(theme, theme_gen::Ink::Divider),
                width: 1.0,
                ..Border::default()
            },
            ..iced::widget::container::Appearance::default()
        })
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
    let mut row = row![].spacing(METADATA_GAP).align_items(Alignment::Center);
    for (index, (glyph, label)) in facts.into_iter().enumerate() {
        if index > 0 {
            // The divider is drawn *before* the item it belongs to, which is where
            // `right-full` puts it.
            row = row.push(bullet(theme));
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

/// One project of the list.
///
/// `ProjectCard.vue`'s **list** layout, which is what `layout.vue` asks for:
/// `p-4 grid` over `grid-project-card-list`, whose `has-actions` template puts the
/// title and summary beside the card's actions, the downloads and followers under
/// them, and the tags and the date on the last row.
fn project_row<'a>(theme: Gen, project: &'a ModrinthUserProject) -> Element<'a, Message> {
    // A row's identity is the project it names rather than its place in the list, so
    // reordering the list must not move a tween from one row to another.
    let key = ui::scoped("user:project", &project.id);
    let (factor, _) = ui::interaction(key);
    let mut info = column![].spacing(HEADER_GROUP_GAP).width(Length::Fill);
    info = info
        .push(
            text(project.title.clone())
                .size(CARD_TITLE)
                .font(semibold())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
        )
        .push(ui::paragraph(theme, &project.description));
    // `ProjectCardTags` at the bottom row's left, and `ProjectCardStats` and
    // `ProjectCardDate` at its right.
    let mut tags = row![].spacing(4.0).align_items(Alignment::Center);
    if let Some(kind) = ProjectType::from_token(&project.project_type) {
        tags = tags.push(ui::tag(theme, kind.label()));
    }
    info = info.push(tags);
    let mut stats = column![]
        .spacing(CARD_STATS_GAP)
        .align_items(Alignment::End);
    stats = stats.push(ui::icon_label(
        theme,
        Glyph::Download,
        &compact_stat(project.downloads),
    ));
    let relative = how_ago(&project.published, now_millis());
    if !relative.is_empty() {
        stats = stats.push(
            row![]
                .spacing(METADATA_TEXT_GAP)
                .align_items(Alignment::Center)
                .push(icon::icon(Glyph::History, METADATA_ICON, theme_gen::ink(theme, INK_SECONDARY)))
                .push(
                    text(relative)
                        .size(HEADER_TEXT)
                        .font(medium())
                        .style(iced::theme::Text::Color(theme_gen::ink(
                            theme,
                            INK_SECONDARY,
                        ))),
                ),
        );
    }
    mouse_area(
        container(
            row![]
                .width(Length::Fill)
                .spacing(CARD_GAP_X)
                .align_items(Alignment::Start)
                .push(info)
                .push(stats),
        )
        .width(Length::Fill)
        .padding(ui::CARD_PAD)
        .style(move |_theme: &iced::Theme| iced::widget::container::Appearance {
            background: Some(iced::Background::Color(crate::theme::brightness(
                theme_gen::ink(theme, theme_gen::Ink::Surface3),
                factor,
            ))),
            border: Border {
                color: crate::theme::brightness(theme_gen::ink(theme, theme_gen::Ink::Surface4), factor),
                width: 1.0,
                radius: theme_gen::span(theme_gen::Span::RadiusLg).into(),
            },
            ..iced::widget::container::Appearance::default()
        }),
    )
    .interaction(Interaction::Pointer)
    .on_enter(Message::hover(key, true))
    .on_exit(Message::hover(key, false))
    .on_press(Message::Project(project.id.clone()))
    .into()
}

/// The strip of filters, or nothing when there is only one thing to filter by.
///
/// The reference draws no strip until it has more than two links -- *All* plus one
/// type is two -- because a lone filter beside the list it filters says nothing
/// (`NavTabs v-if="navLinks.length > 2"`). The *Collections* link the reference adds
/// when a user has collections is not here: collections are read through the v3
/// user service, which this tree cannot reach (G105).
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
                 "description":"Modern rendering engine","published":"2020-06-15T00:00:00Z","downloads":41000000},
                {"id":"hEOCdOgW","slug":"phosphor","project_type":"mod","title":"Phosphor",
                 "description":"Lighting engine","published":"2021-01-03T00:58:54.900351Z","downloads":865848},
                {"id":"P7dR8mSH","slug":"fabulously-optimized","project_type":"modpack","title":"Fabulously Optimized",
                 "description":"A pack","published":"2022-02-02T00:00:00Z","downloads":1234}]"#,
        )
        .expect("the project list");
        Profile::of(user, projects, Err("no avatar on this machine".to_string()))
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
        let arrived = Profile::of(user, Vec::new(), Ok(picture(32)));
        assert!(arrived.avatar.is_some());
        assert!(arrived.note.is_none());
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

    #[test]
    fn the_list_is_the_filter_s_own_and_all_is_everything() {
        let profile = sample();
        assert_eq!(profile.shown(None).len(), 3);
        let mods = profile.shown(Some(ProjectType::Mod));
        assert_eq!(mods.len(), 2);
        assert!(mods.iter().all(|project| project.project_type == "mod"));
        assert_eq!(profile.shown(Some(ProjectType::Shader)).len(), 0);
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
    fn every_project_row_draws_in_every_theme() {
        for theme in Gen::ALL {
            for project in sample().projects.iter() {
                drop(project_row(*theme, project));
            }
            // A row whose project names no type this tree knows still draws: the tag
            // is the part that is absent, not the row.
            let odd: ModrinthUserProject = serde_json::from_str(
                r#"{"id":"x","title":"Something","project_type":"minecraft_java_server"}"#,
            )
            .expect("the project");
            drop(project_row(*theme, &odd));
        }
    }
}
