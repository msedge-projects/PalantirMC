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
use iced::widget::image::Handle;
use iced::widget::{column, image, mouse_area, row, text, Space};
use iced::{Alignment, Element, Length};
use palantir_net::modrinth::{ModrinthUser, ModrinthUserProject};

use crate::icons_gen::Glyph;
use crate::page::{self, Load, GAP, ROW_GAP};
use crate::route::ProjectType;
use crate::store::Store;
use crate::style::{heading, medium, semibold, INK_CONTRAST, INK_SECONDARY};
use crate::text_gen::{self, Key};
use crate::theme_gen::{self, Theme as Gen};
use crate::ui::{self, Hovered};

/// The avatar's side on the page.
///
/// `UserPageHeader.vue` draws `:size="isModrinthUser ? '64px' : '96px'"`, and which
/// account is Modrinth's own is a lookup in `@modrinth/utils`, a package this tree
/// does not vendor -- so every profile is drawn at the 96 that a normal account
/// gets. The alternative would be a size that changed on an id nobody here can
/// recognize.
const AVATAR: f32 = 96.0;

/// The dim a project card takes under the pointer: `hover:brightness-90`, the same
/// number Discover's cards use for the same reference class.
const CARD_HOVER: f32 = 0.9;

/// The header's own action.
///
/// The reference's header also carries *Edit* (the reader's own profile) and an
/// overflow menu (report, block, copy id, copy permalink, staff actions). None of
/// them is here: every one acts on a Modrinth account this launcher is not signed
/// in to, which is the same boundary G105 recorded for the page's two missing
/// halves.
const REFRESH_KEY: &str = "user:refresh";

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

/// The account's avatar, decoded: the pixels, and the size they are laid out in.
///
/// RGBA8, exactly as [`Handle::from_pixels`] takes them, and deliberately the same
/// shape [`crate::skin::FrontView`] has -- the two are the same job done on a
/// different picture. What is *not* shared is the work between them: a skin is cut
/// into the parts of a doll ([`crate::skin::cut`]) and an avatar is drawn as
/// it arrives, so only the decode is common and each type keeps its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Avatar {
    /// Its width in pixels.
    pub width: u32,
    /// Its height.
    pub height: u32,
    /// `width * height * 4` bytes of RGBA.
    pub pixels: Vec<u8>,
}

impl Avatar {
    /// The avatar as something an `image` widget can draw.
    pub fn handle(&self) -> Handle {
        Handle::from_pixels(self.width, self.height, self.pixels.clone())
    }

    /// Decode a fetched picture, or `None` when the bytes are not one.
    ///
    /// The decoder is spelled with a leading `::` because this module has
    /// `iced::widget::image` in scope for the widget, and that name would otherwise
    /// resolve to the widget's own module rather than to the crate that decodes.
    pub fn of(picture: &[u8]) -> Option<Avatar> {
        let decoded = ::image::load_from_memory(picture).ok()?.to_rgba8();
        Some(Avatar {
            width: decoded.width(),
            height: decoded.height(),
            pixels: decoded.into_raw(),
        })
    }
}

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
    /// Their avatar, when it could be fetched and decoded.
    pub avatar: Option<Avatar>,
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
            Ok(picture) => match Avatar::of(&picture) {
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
    /// The header's refresh was pressed.
    Refresh,
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
            Message::Refresh => {
                // The sentence about the last thing this page could not do is about
                // a request this one replaces.
                self.notice = None;
                return Some(self.ask());
            }
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
    // The header and the list arrive together -- one request, one answer -- so they
    // are one block: a page that drew a header from one answer and a list from
    // another could show a name over somebody else's projects.
    blocks.push(page::draw(theme, &state.profile, "this profile", |profile| {
        loaded(theme, state, profile)
    }));
    page::body(blocks, GAP)
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
        let mut list = column![].spacing(GAP).width(Length::Fill);
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
/// The counts are over *every* project the user owns, not the filtered list, which
/// is what the reference passes its own header (`:projects-count="projects.length"`,
/// `:downloads="sumDownloads"`): a filter chooses what is listed, not what the
/// account is.
fn header<'a>(theme: Gen, profile: &'a Profile) -> Element<'a, Message> {
    // The picture slot, which is the same size whether or not there is a picture in
    // it: a header that grew when an avatar arrived would move the name under the
    // pointer that was about to press something.
    let picture: Element<'a, Message> = match &profile.avatar {
        Some(avatar) => image(avatar.handle())
            .width(Length::Fixed(AVATAR))
            .height(Length::Fixed(AVATAR))
            .into(),
        // The reference's own fallback is a circle tinted by the username with the
        // first letter in it; this launcher has no letter-in-a-circle drawing, so
        // the placeholder is the generic account glyph at the same size.
        None => page::glyph(theme, Glyph::CircleUser, AVATAR),
    };
    let mut facts = row![].spacing(ROW_GAP).align_items(Alignment::Center);
    facts = facts
        .push(ui::icon_label(
            theme,
            Glyph::Box,
            &format!(
                "{} {}",
                crate::text::number(profile.projects.len() as u64),
                text_gen::profile_label_project_count(profile.projects.len() as u64)
            ),
        ))
        .push(ui::icon_label(
            theme,
            Glyph::Download,
            &text_gen::project_download_count_tooltip(profile.downloads()),
        ))
        .push(ui::icon_label(
            theme,
            Glyph::Calendar,
            &format!(
                "{} {}",
                Key::ProfileLabelJoined.message(),
                profile.user.joined_label()
            ),
        ));
    let mut identity = column![]
        .spacing(4.0)
        .push(
            text(profile.user.display_name().to_string())
                .size(24.0)
                .font(heading())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
        );
    // The handle only when it is a second name: an account with no display name has
    // one string, and printing it twice with an `@` in front of the second is a
    // line nobody needs.
    if profile.user.has_separate_username() {
        identity = identity.push(
            text(format!("@{}", profile.user.username))
                .size(14.0)
                .font(medium())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
        );
    }
    identity = identity.push(
        text(profile.summary().to_string())
            .size(14.0)
            .font(medium())
            .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
    );
    if let Some(note) = &profile.note {
        identity = identity.push(
            text(note.clone())
                .size(13.0)
                .font(medium())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
        );
    }
    identity = identity.push(facts);
    let head = row![]
        .spacing(GAP)
        .align_items(Alignment::Center)
        .push(picture)
        .push(identity)
        .push(Space::with_width(Length::Fill))
        // `button.refresh`, which is the reference's own label for this control.
        // The placeholder this page replaces carried `app.library.sort.label` here,
        // which reads *Sort by* -- a button that said it sorted the page it
        // reloads.
        .push(ui::button(
            theme,
            REFRESH_KEY,
            Key::ButtonRefresh,
            ui::Kind::Quiet,
            Message::Refresh,
        ));
    ui::card(theme, head.width(Length::Fill))
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

/// One project of the list.
///
/// A row rather than the reference's full card: what its `ProjectList` draws in the
/// app is the title, the summary and the counts, and the card's icon is a picture
/// this launcher would have to fetch and cache per row for a decoration. Everything
/// the row *says* is the service's own document.
fn project_row<'a>(theme: Gen, project: &'a ModrinthUserProject) -> Element<'a, Message> {
    // A row's identity is the project it names rather than its place in the list, so
    // reordering the list must not move a tween from one row to another.
    let key = ui::scoped("user:project", &project.id);
    let (factor, _) = ui::interaction(key);
    let mut facts = row![].spacing(ROW_GAP).align_items(Alignment::Center);
    if let Some(kind) = ProjectType::from_token(&project.project_type) {
        facts = facts.push(ui::tag(theme, kind.label()));
    }
    facts = facts.push(ui::icon_label(
        theme,
        Glyph::Download,
        &text_gen::project_download_count_tooltip(project.downloads),
    ));
    if !project.published.is_empty() {
        facts = facts.push(ui::icon_label(
            theme,
            Glyph::Calendar,
            &palantir_net::date_label(&project.published),
        ));
    }
    mouse_area(ui::card_at(
        theme,
        factor,
        column![]
            .spacing(6.0)
            .push(
                text(project.title.clone())
                    .size(16.0)
                    .font(semibold())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
            )
            .push(ui::paragraph(theme, &project.description))
            .push(facts),
    ))
    .interaction(Interaction::Pointer)
    .on_enter(Message::hover_with(key, true, CARD_HOVER))
    .on_exit(Message::hover_with(key, false, CARD_HOVER))
    .on_press(Message::Project(project.id.clone()))
    .into()
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
    fn an_avatar_that_arrives_is_decoded_and_one_that_is_not_an_image_is_refused() {
        let avatar = Avatar::of(&picture(64)).expect("a 64x64 PNG");
        assert_eq!((avatar.width, avatar.height), (64, 64));
        assert_eq!(avatar.pixels.len(), 64 * 64 * 4);
        // The widget draws from the pixels rather than from the encoded bytes, which
        // is what the length above is about: a handle built from the file would let
        // the renderer decode it a second time.
        drop(avatar.handle());
        assert!(Avatar::of(b"this is not a picture").is_none());
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
        assert_eq!(state.opening(), None, "asked once, and the refresh button is what asks again");
        // The refresh is the page's own action, and it is a second round: the answer
        // to the first is dropped rather than drawn over it.
        let Some(again) = state.update(Message::Refresh) else {
            panic!("the refresh button asks");
        };
        assert_eq!(again.round, 2);
        assert_eq!(again.user, "jellysquid3");
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
            profile.avatar = Avatar::of(&picture(32));
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
