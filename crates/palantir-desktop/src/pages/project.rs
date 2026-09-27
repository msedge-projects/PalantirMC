//! A project: `pages/project/*`, at `/project/:id/:tab?`.
//!
//! Six routes in one page, because that is how the reference has it: a header
//! (the title, the author, the counts, the buttons) over a tab strip, and a body
//! that changes tab by tab. The tabs are its own -- Description, Gallery, Versions
//! -- plus the two the router reaches without a tab of their own (a single version
//! and the changelog).
//!
//! Everything here comes from Modrinth's API, and the page *asks* for it rather
//! than fetching it: [`State::update`] and [`State::opening`] hand the shell an
//! [`Asked`], the shell runs it through the store off the frame thread, and the
//! answer comes back as [`Message::Found`]. The round travels with the request
//! the way Discover's does, so an answer to a project the reader has already left
//! is dropped instead of drawn under the one they are looking at.
//!
//! The description is markdown in the reference -- it renders `project.body`
//! through the same `markdown-body` stylesheet the web app uses. Drawing raw
//! markdown as prose is deliberate and temporary: it shows the text the reference
//! would show, minus its formatting, and a *wrong* rendering (bold as literal
//! asterisks, say) is what a hand-rolled parser produces before it is finished.
//! The subset renderer is the next piece of work on this page and says so here.

use iced::widget::{column, row, text};
use iced::{Element, Length};

use palantir_net::modrinth::{ModrinthProject, ModrinthProjectVersion};

use crate::icons_gen::Glyph;
use crate::page::{self, Load, GAP, ROW_GAP};
use crate::pages::Ask;
use crate::route::{ProjectTab, ProjectType};
use crate::store::Store;
use crate::style::{semibold, INK_CONTRAST, INK_SECONDARY};
use crate::text_gen::{self, Key};
use crate::theme_gen::{self, Theme as Gen};
use crate::ui;

/// A project, as the API describes it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Project {
    /// Project id.
    pub id: String,
    /// Title.
    pub title: String,
    /// The author's name.
    pub author: String,
    /// The short summary.
    pub summary: String,
    /// The long description, in markdown.
    pub body: String,
    /// Total downloads.
    pub downloads: u64,
    /// Followers.
    pub follows: u64,
    /// Versions the project supports.
    pub game_versions: Vec<String>,
    /// Loaders it runs on.
    pub loaders: Vec<String>,
    /// The gallery's images, by name.
    pub gallery: Vec<String>,
    /// The versions it has published.
    pub versions: Vec<Version>,
    /// What kind of project it is, or nothing when the API named a kind this
    /// launcher does not install.
    pub kind: Option<ProjectType>,
}

/// One published version.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Version {
    /// Version number, as published.
    pub number: String,
    /// The version's own name.
    pub name: String,
    /// What it supports.
    pub game_versions: Vec<String>,
    /// What it runs on.
    pub loaders: Vec<String>,
    /// Downloads of this version.
    pub downloads: u64,
    /// The changelog, in markdown.
    pub changelog: String,
}

/// One request, as the shell takes it.
///
/// A value rather than three arguments, for [`crate::pages::discover::Asked`]'s
/// reason: the round is what makes a slow answer harmless, and the id is what
/// the answer is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    /// Which request this is, counting from one.
    pub round: u64,
    /// Which project, as the API names it.
    pub id: String,
}

/// The install the page is asking for: which project, and what to call it in the
/// dialog the shell opens.
///
/// The title travels with the id because the dialog is the shell's and the shell
/// has never seen a project document: a `Key` cannot hold a title, and asking the
/// engine again for a caption the page is already drawing would be a request per
/// press for a word.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Install {
    /// Which project, as the API names it.
    pub id: String,
    /// Its title, as the page is drawing it. Empty when the page has none yet.
    pub title: String,
    /// Whether it is a pack, which is the one kind whose install has no folder to
    /// land in: the dialog asks for a new instance instead of offering one.
    pub pack: bool,
}

impl Project {
    /// One project, from the API's own documents.
    ///
    /// Three of them, because Modrinth splits the answer three ways and each
    /// split is a request of its own: the project document names the team but not
    /// the person on it, the author is therefore a member list, and the versions
    /// are their own list rather than a field of the project. The translation
    /// lives here rather than in the store for
    /// [`crate::pages::discover::Hit::from_api`]'s reason: what a page *is*
    /// belongs to the page that draws it, and a page that grew a field would
    /// otherwise change a module that has never drawn one.
    pub fn from_api(
        project: &ModrinthProject,
        author: &str,
        versions: &[ModrinthProjectVersion],
    ) -> Project {
        Project {
            id: project.id.clone(),
            title: project.title.clone(),
            author: author.to_string(),
            summary: project.description.clone(),
            body: project.body.clone(),
            // What the thing *is*, which the header does not draw and one press
            // does need: a pack is installed by becoming an instance rather than
            // by landing in a folder, and the dialog that asks where to put it
            // asks a different question for one. Kept as the API spelled it, with
            // an unknown type left unknown rather than guessed at -- a project
            // type this launcher has not heard of is not a mod.
            kind: ProjectType::from_token(&project.project_type),
            downloads: project.downloads,
            follows: project.followers,
            game_versions: project.game_versions.clone(),
            loaders: project.loaders.clone(),
            // An image's caption is its title, and a gallery entry with no
            // caption is drawn by its URL rather than as a blank row: both are
            // what the reference's own gallery does.
            gallery: project
                .gallery
                .iter()
                .map(|image| {
                    if image.title.is_empty() {
                        image.url.clone()
                    } else {
                        image.title.clone()
                    }
                })
                .collect(),
            versions: versions.iter().map(Version::from_api).collect(),
        }
    }
}

impl Version {
    /// One version, from the API's own entry.
    pub fn from_api(version: &ModrinthProjectVersion) -> Version {
        Version {
            number: version.version_number.clone(),
            name: version.name.clone(),
            game_versions: version.game_versions.clone(),
            loaders: version.loaders.clone(),
            downloads: version.downloads,
            changelog: version.changelog.clone(),
        }
    }
}

/// What the page can be told.
#[derive(Debug, Clone)]
pub enum Message {
    /// Another tab was chosen.
    Tab(ProjectTab),
    /// The project was asked for again.
    Refresh,
    /// The project's main action was pressed: ask which instance it goes into.
    ///
    /// A request rather than a notice, which is what this was until the install
    /// was real: the page reports the press (see [`Ask`]) and the shell opens a
    /// dialog, because *which instances this launcher has* is not a fact the page
    /// holds -- and neither is where their folders are.
    Install,
    /// The install this page asked for has an outcome to show.
    ///
    /// One string rather than a `Result`: the page draws the sentence and nothing
    /// else, and *which* it was has already been decided by the shell that worded
    /// it -- the half that needs the difference is the dialog, which is the
    /// shell's. It is also the smaller type, which the enum cares about: a
    /// `Result<String, String>` is two `String`s wide and would have made every
    /// message in this family that size.
    Noted(String),
    /// The answer to a request, from the shell.
    ///
    /// The round is what makes a slow answer harmless: a page that has asked
    /// again in the meantime has moved past this one, and drawing an older
    /// project under the newer id is the kind of wrong a reader cannot see.
    Found {
        /// Which request this answers, as [`Asked::round`] numbered it.
        round: u64,
        /// The project, or the reason there is none.
        ///
        /// Boxed because a message is moved on every frame and an enum is as wide
        /// as its widest variant: a `Project` is a dozen strings and three lists,
        /// and carrying one by value would make *every* message of this family
        /// that size -- which clippy says in one line (`large_enum_variant`), and
        /// the test at the foot of this file says in bytes.
        result: Result<Box<Project>, String>,
    },
    /// The pointer entered or left one of the page's controls, for the clock
    /// that carries a hover's 150 ms (see [`crate::ui`]).
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

/// One stable name per tab, in `State::TABS`' order.
const TAB_KEYS: [&str; 3] = ["project:tab:description", "project:tab:gallery", "project:tab:versions"];

/// The page's two actions.
const INSTALL_KEY: &str = "project:install";
const REFRESH_KEY: &str = "project:refresh";

/// The page's own state.
#[derive(Debug, Clone)]
pub struct State {
    /// Which project.
    pub id: String,
    /// Which tab.
    pub tab: ProjectTab,
    /// The project itself.
    pub project: Load<Project>,
    /// The last thing the page could not do.
    pub notice: Option<String>,
    /// How many times this page has asked, which is how an answer is told apart
    /// from an answer to a question it has since replaced.
    round: u64,
}

impl State {
    /// A page for one project, on one tab.
    pub fn new(id: String, tab: ProjectTab) -> State {
        State {
            id,
            tab,
            project: Load::Idle,
            notice: None,
            round: 0,
        }
    }

    /// Apply a message, and answer with what the shell has to do about it.
    ///
    /// The answer is an [`Ask`] rather than this page's own [`Asked`] because the
    /// page has two kinds of request now: re-read me (which the round numbers), and
    /// install me somewhere (which names an instance the page has never heard of).
    /// The instance page answers with an `Ask` for the same reason.
    pub fn update(&mut self, message: Message) -> Option<Ask> {
        match message {
            Message::Tab(tab) => self.tab = tab,
            // The button asks again rather than standing in for an answer: what
            // the page knows is dropped, and the shell is told to go and get it.
            Message::Refresh => return Some(Ask::Project(self.ask())),
            Message::Install => {
                // The sentence about the last install is about a press this one
                // replaces, and the dialog that opens says what happens next.
                self.notice = None;
                return Some(Ask::Install(Install {
                    id: self.id.clone(),
                    title: self
                        .project
                        .ready()
                        .map(|project| project.title.clone())
                        .unwrap_or_default(),
                    // An unloaded page is not a pack: the dialog it opens refuses
                    // nothing, and the press that would have installed a file
                    // still has a folder to offer.
                    pack: self
                        .project
                        .ready()
                        .map(|project| project.kind == Some(ProjectType::Modpack))
                        .unwrap_or(false),
                }));
            }
            // A sentence either way, in the slot every other thing the page could
            // not do is drawn in: what worked is worth saying for the same reason
            // what failed is.
            Message::Noted(line) => self.notice = Some(line),
            Message::Found { round, result } => {
                // An answer to a request this page has replaced is dropped. It is
                // not an error and not worth a notice: the reader asked for
                // something newer and the newer answer is on its way.
                if round == self.round {
                    self.project = match result {
                        Ok(project) => Load::Ready(*project),
                        Err(reason) => Load::Failed(reason),
                    };
                    // The notice belongs to the page the answer just replaced:
                    // what a button said about the project that is no longer on
                    // screen is not about the one that is.
                    self.notice = None;
                }
            }
            Message::Hover { key, over, hover } => crate::ui::pointer_with(
                key,
                over,
                hover.unwrap_or_else(crate::theme::hover_brightness),
            ),
        }
        None
    }

    /// The request this page owes because nothing has been asked for yet.
    ///
    /// `None` once it has asked, which is what keeps the page from asking on
    /// every message the shell routes to it.
    pub fn opening(&mut self) -> Option<Asked> {
        if self.project == Load::Idle {
            Some(self.ask())
        } else {
            None
        }
    }

    /// Bump the round, mark the page as waiting, and describe the request.
    fn ask(&mut self) -> Asked {
        self.round += 1;
        self.project = Load::Loading;
        Asked { round: self.round, id: self.id.clone() }
    }

    /// The tabs the page offers, in the reference's order.
    ///
    /// `ProjectTab::ALL` is the reference's own list; the two entries it leaves
    /// out (a single version, the changelog) are reachable by address rather than
    /// by tab, which is what `routes.js` does with them.
    pub const TABS: [ProjectTab; 3] =
        [ProjectTab::Description, ProjectTab::Gallery, ProjectTab::Versions];

    /// The key for a tab's label.
    ///
    /// The fourth variant, a single version, has no tab of its own --
    /// `routes.js` addresses it under `versions` and marks the versions tab
    /// selected, which is what this does by giving it the same key.
    pub fn key_for(tab: &ProjectTab) -> Key {
        match tab {
            ProjectTab::Description => Key::AppProjectTabDescription,
            ProjectTab::Gallery => Key::AppProjectTabGallery,
            ProjectTab::Versions | ProjectTab::Version(_) => Key::AppProjectTabVersions,
        }
    }

    /// What the tab strip shows.
    pub fn labels(&self) -> Vec<(String, bool)> {
        State::TABS
            .iter()
            .map(|tab| (State::key_for(tab).message().to_string(), *tab == self.tab))
            .collect()
    }
}

/// Draw the page.
pub fn view<'a>(theme: Gen, state: &'a State, _store: &'a Store) -> Element<'a, Message> {
    let project = &state.project;
    let mut blocks: Vec<Element<'a, Message>> = Vec::new();
    if let Some(notice) = &state.notice {
        blocks.push(ui::admonition(theme, ui::Severity::Info, &state.id, notice));
    }
    blocks.push(page::draw(theme, project, "this project", |project| header(theme, project)));
    blocks.push(ui::tabs(theme, &TAB_KEYS, &state.labels(), |index| {
        Message::Tab(State::TABS.get(index).cloned().unwrap_or(ProjectTab::Description))
    }));
    blocks.push(match project {
        Load::Ready(project) => body(theme, state.tab.clone(), project),
        other => page::draw(theme, other, "this project", |_project| {
            page::waiting(theme, "this project")
        }),
    });
    page::body(blocks, GAP)
}

/// The header: the title, the author, the summary and the counts.
fn header<'a>(theme: Gen, project: &'a Project) -> Element<'a, Message> {
    let mut facts = row![].spacing(ROW_GAP);
    facts = facts.push(ui::icon_label(
        theme,
        Glyph::Download,
        &text_gen::project_download_count_tooltip(project.downloads),
    ));
    facts = facts.push(ui::icon_label(
        theme,
        Glyph::Heart,
        &text_gen::project_follower_count_tooltip(project.follows),
    ));
    let mut tags = row![].spacing(6.0);
    for loader in project.loaders.iter().take(4) {
        tags = tags.push(ui::tag(theme, loader));
    }
    for version in project.game_versions.iter().take(3) {
        tags = tags.push(ui::tag(theme, version));
    }
    ui::card(
        theme,
        column![]
            .spacing(6.0)
            .push(
                text(project.title.clone())
                    .size(24.0)
                    .font(crate::style::heading())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
            )
            .push(
                text(project.author.clone())
                    .size(13.0)
                    .font(semibold())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
            )
            .push(ui::paragraph(theme, &project.summary))
            .push(facts)
            .push(tags)
            .push(
                row![]
                    .spacing(ROW_GAP)
                    .push(ui::button(
                        theme,
                        INSTALL_KEY,
                        Key::AppLibraryContextMenuCreateInstance,
                        ui::Kind::Colored,
                        Message::Install,
                    ))
                    .push(ui::button(
                        theme,
                        REFRESH_KEY,
                        Key::AppLibrarySortLabel,
                        ui::Kind::Quiet,
                        Message::Refresh,
                    )),
            ),
    )
}

/// The tab's own body.
fn body<'a>(theme: Gen, tab: ProjectTab, project: &'a Project) -> Element<'a, Message> {
    match tab {
        // The reference renders the body as markdown. Until the subset renderer
        // lands, this is the text itself, split into its paragraphs, which is the
        // same words in the same order with the formatting left visible.
        ProjectTab::Description => {
            let source = project.body.as_str();
            let mut blocks = column![].spacing(GAP).width(Length::Fill);
            for paragraph in source.split("\n\n").filter(|part| !part.trim().is_empty()) {
                blocks = blocks.push(ui::card(theme, ui::paragraph(theme, paragraph.trim())));
            }
            if source.trim().is_empty() {
                blocks = blocks.push(page::empty(theme, Key::BrowseNoResults));
            }
            blocks.into()
        }
        ProjectTab::Gallery => {
            if project.gallery.is_empty() {
                return page::empty(theme, Key::BrowseNoResults);
            }
            let mut list = column![].spacing(GAP).width(Length::Fill);
            for name in &project.gallery {
                list = list.push(ui::card(theme, ui::icon_label(theme, Glyph::Image, name)));
            }
            list.into()
        }
        ProjectTab::Versions | ProjectTab::Version(_) => {
            if project.versions.is_empty() {
                return page::empty(theme, Key::BrowseNoResults);
            }
            let mut list = column![].spacing(GAP).width(Length::Fill);
            for version in &project.versions {
                list = list.push(version_card(theme, version));
            }
            list.into()
        }
    }
}

/// One version row.
fn version_card<'a>(theme: Gen, version: &'a Version) -> Element<'a, Message> {
    let mut tags = row![].spacing(6.0);
    for loader in version.loaders.iter().take(3) {
        tags = tags.push(ui::tag(theme, loader));
    }
    for game in version.game_versions.iter().take(2) {
        tags = tags.push(ui::tag(theme, game));
    }
    ui::card(
        theme,
        column![]
            .spacing(6.0)
            .push(
                text(format!("{} · {}", version.number, version.name))
                    .size(16.0)
                    .font(semibold())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
            )
            .push(tags)
            .push(ui::icon_label(
                theme,
                Glyph::Download,
                &text_gen::project_download_count_tooltip(version.downloads),
            )),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> Project {
        Project {
            id: "sodium".to_string(),
            title: "Sodium".to_string(),
            author: "jellysquid3".to_string(),
            summary: "A rendering engine".to_string(),
            body: "# Sodium\n\nFaster.\n\n- one\n- two".to_string(),
            kind: Some(ProjectType::Mod),
            downloads: 1234567,
            follows: 4321,
            game_versions: vec!["1.21".to_string()],
            loaders: vec!["fabric".to_string()],
            gallery: vec!["shot-1.png".to_string()],
            versions: vec![Version {
                number: "0.6.0".to_string(),
                name: "Sodium 0.6.0".to_string(),
                game_versions: vec!["1.21".to_string()],
                loaders: vec!["fabric".to_string()],
                downloads: 999,
                changelog: "Fixed a thing.".to_string(),
            }],
        }
    }

    #[test]
    fn the_tabs_are_the_reference_s_and_its_labels_are_its_own() {
        assert_eq!(State::TABS.len(), 3);
        assert_eq!(State::key_for(&ProjectTab::Description).message(), "Description");
        assert_eq!(State::key_for(&ProjectTab::Gallery).message(), "Gallery");
        assert_eq!(State::key_for(&ProjectTab::Versions).message(), "Versions");
        // A single version is addressed under `versions`, so its key is that
        // tab's: the address is a version and the tab strip must say so.
        assert_eq!(
            State::key_for(&ProjectTab::Version("1.0".to_string())).message(),
            "Versions"
        );
        let state = State::new("sodium".to_string(), ProjectTab::Gallery);
        let labels = state.labels();
        assert_eq!(labels.len(), 3);
        assert_eq!(labels[0].0, "Description");
        assert!(!labels[0].1, "the tab on screen is the selected one");
        assert!(labels[1].1);
    }

    #[test]
    fn a_fresh_page_owes_a_request_and_a_failed_one_keeps_its_reason() {
        let mut state = State::new("sodium".to_string(), ProjectTab::Description);
        // Nothing has been asked for yet, which is not the same as an empty
        // project: the shell asks for what `opening` describes.
        assert_eq!(state.project, Load::Idle);
        let asked = state.opening().expect("the first request");
        assert_eq!(asked.id, "sodium");
        assert_eq!(asked.round, 1);
        assert_eq!(state.project, Load::Loading, "the page waits once it has asked");
        assert!(state.opening().is_none(), "and does not ask twice");
        // The answer, or the reason there is none -- never an empty project.
        state.update(Message::Found { round: 1, result: Err("no such project".to_string()) });
        // The failure arm is the one a page is drawn from before a project is,
        // so it is the arm this test needs; the ready arm is the line below.
        assert_eq!(state.project.failure(), Some("no such project"));
    }

    #[test]
    fn an_answer_to_a_request_the_page_has_replaced_is_dropped() {
        let mut state = State::new("sodium".to_string(), ProjectTab::Description);
        let first = state.opening().expect("the first request");
        let second = match state.update(Message::Refresh) {
            Some(Ask::Project(asked)) => asked,
            other => panic!("a refresh asks for the project again: {other:?}"),
        };
        assert_eq!(second.round, first.round + 1);
        // The first answer arrives late: it is dropped rather than drawn under
        // the request that replaced it, and it is not an error either.
        state.update(Message::Found { round: first.round, result: Ok(Box::new(project())) });
        assert_eq!(state.project, Load::Loading, "the stale answer was dropped");
        state.update(Message::Found { round: second.round, result: Ok(Box::new(project())) });
        assert!(state.project.ready().is_some(), "the answer in force was taken");
    }

    #[test]
    fn a_page_message_is_small() {
        // Every message of this page is built and moved per frame, and an enum is
        // as wide as its widest variant. `Found`'s project is boxed for this
        // reason; this is the assertion that keeps a later field from quietly
        // undoing it, and the measurement is in the message rather than in a
        // comment so a failure says how far off it is.
        //
        // The bound is 40 rather than less because 40 is the floor for any page
        // of this launcher: `Hover` is a `&'static str`, a flag and a hover
        // reading, which is 32 bytes plus the tag every enum carries, and the
        // `hovered!` macro hands every page that shape. So the claim this makes
        // is that the box holds `Found` *at* that floor instead of above it -- a
        // `Project` carried by value would be the width of every message here,
        // which is the `large_enum_variant` clippy names.
        assert!(
            std::mem::size_of::<Message>() <= 40,
            "project::Message is {} bytes",
            std::mem::size_of::<Message>()
        );
    }

    #[test]
    fn the_api_documents_become_what_the_page_draws() {
        // Three documents in, one page out: the project's own, the team's, and the
        // version list's. The author comes from the members because the project
        // document does not carry one, and a gallery entry with no caption is
        // drawn by its URL rather than as a blank row.
        let api_project = ModrinthProject {
            id: "AANobbMI".to_string(),
            slug: "sodium".to_string(),
            project_type: "mod".to_string(),
            title: "Sodium".to_string(),
            description: "Modern rendering engine".to_string(),
            body: "# Sodium".to_string(),
            downloads: 41_000_000,
            followers: 9_000,
            game_versions: vec!["1.21.4".to_string()],
            loaders: vec!["fabric".to_string()],
            gallery: vec![
                palantir_net::modrinth::ModrinthGalleryImage {
                    url: "https://cdn.modrinth.com/shot.png".to_string(),
                    title: "In the nether".to_string(),
                    description: String::new(),
                },
                palantir_net::modrinth::ModrinthGalleryImage {
                    url: "https://cdn.modrinth.com/uncaptioned.png".to_string(),
                    title: String::new(),
                    description: String::new(),
                },
            ],
        };
        let api_versions = [ModrinthProjectVersion {
            id: "abc".to_string(),
            project_id: "AANobbMI".to_string(),
            name: "Sodium 0.6.5".to_string(),
            version_number: "mc1.21.4-0.6.5".to_string(),
            version_type: "release".to_string(),
            downloads: 1234,
            changelog: "Fixed a thing.".to_string(),
            game_versions: vec!["1.21.4".to_string()],
            loaders: vec!["fabric".to_string()],
            files: Vec::new(),
            dependencies: Vec::new(),
        }];

        let page = Project::from_api(&api_project, "jellysquid3", &api_versions);
        assert_eq!(page.id, "AANobbMI");
        assert_eq!(page.title, "Sodium");
        assert_eq!(
            page.kind,
            Some(ProjectType::Mod),
            "the type is read, because the install dialog needs it"
        );
        assert_eq!(page.author, "jellysquid3");
        assert_eq!(page.summary, "Modern rendering engine");
        assert_eq!(page.downloads, 41_000_000);
        assert_eq!(page.follows, 9_000);
        assert_eq!(page.gallery, vec!["In the nether", "https://cdn.modrinth.com/uncaptioned.png"]);
        assert_eq!(page.versions.len(), 1);
        assert_eq!(page.versions[0].number, "mc1.21.4-0.6.5");
        assert_eq!(page.versions[0].name, "Sodium 0.6.5");
        assert_eq!(page.versions[0].downloads, 1234);
        assert_eq!(page.versions[0].changelog, "Fixed a thing.");
        assert_eq!(page.versions[0].loaders, vec!["fabric"]);
    }

    #[test]
    fn the_page_draws_in_every_theme_on_every_tab_and_in_every_state() {
        let store = Store::default();
        let tabs = [
            ProjectTab::Description,
            ProjectTab::Gallery,
            ProjectTab::Versions,
            ProjectTab::Version("1.0".to_string()),
        ];
        for theme in Gen::ALL {
            for tab in tabs.clone() {
                let mut state = State::new("sodium".to_string(), tab);
                // Every state the page can be in, including the one it opens in.
                drop(view(*theme, &state, &store));
                state.project = Load::Loading;
                drop(view(*theme, &state, &store));
                state.project = Load::Ready(project());
                drop(view(*theme, &state, &store));
                state.project = Load::Ready(Project::default());
                drop(view(*theme, &state, &store));
                state.project = Load::Empty;
                drop(view(*theme, &state, &store));
                state.notice = Some("x".to_string());
                state.project = Load::Ready(project());
                drop(view(*theme, &state, &store));
            }
        }
    }

    #[test]
    fn the_install_button_asks_the_shell_for_a_place_to_put_the_project() {
        let mut state = State::new("sodium".to_string(), ProjectTab::Description);
        state.project = Load::Ready(project());
        match state.update(Message::Install) {
            Some(Ask::Install(install)) => {
                assert_eq!(install.id, "sodium");
                assert_eq!(
                    install.title, "Sodium",
                    "the dialog is the shell's and has never seen a project document"
                );
                assert!(!install.pack, "and this one lands in a folder");
            }
            other => panic!("the install button asks which instance: {other:?}"),
        }
        assert!(
            state.notice.is_none(),
            "and says nothing about a transfer that has not started"
        );
        // A pack says so, because the shell's dialog asks a different question for
        // one: a pack has no folder to land in, it becomes an instance.
        let mut pack = State::new("cobblemon".to_string(), ProjectTab::Description);
        pack.project = Load::Ready(Project {
            kind: Some(ProjectType::Modpack),
            ..project()
        });
        match pack.update(Message::Install) {
            Some(Ask::Install(install)) => assert!(install.pack, "a pack is named as one"),
            other => panic!("a pack asks too: {other:?}"),
        }
        // Pressed before the project has arrived, the ask still goes out and names
        // no title rather than asking the service for one.
        let mut fresh = State::new("sodium".to_string(), ProjectTab::Description);
        match fresh.update(Message::Install) {
            Some(Ask::Install(install)) => assert_eq!(install.title, ""),
            other => panic!("still an ask: {other:?}"),
        }
        // What comes back is a sentence in the slot everything the page could not
        // do is drawn in, and a failure replaces the success that preceded it.
        state.update(Message::Noted("Installed Sodium 0.6.5 into atm10".to_string()));
        assert_eq!(state.notice.as_deref(), Some("Installed Sodium 0.6.5 into atm10"));
        state.update(Message::Noted("no version of Sodium is for Fabric 1.21.4".to_string()));
        assert_eq!(
            state.notice.as_deref(),
            Some("no version of Sodium is for Fabric 1.21.4"),
            "and what failed replaces what worked"
        );
    }

    #[test]
    fn a_tab_change_keeps_the_project_on_screen() {
        let mut state = State::new("sodium".to_string(), ProjectTab::Description);
        state.project = Load::Ready(project());
        state.update(Message::Tab(ProjectTab::Versions));
        assert_eq!(state.tab, ProjectTab::Versions);
        // A tab is navigation, not a request: the project on screen stays.
        assert!(state.project.ready().is_some());
        assert!(state.update(Message::Tab(ProjectTab::Gallery)).is_none());
    }
}
