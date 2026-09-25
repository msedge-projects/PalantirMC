//! A project: `pages/project/*`, at `/project/:id/:tab?`.
//!
//! Six routes in one page, because that is how the reference has it: a header
//! (the title, the author, the counts, the buttons) over a tab strip, and a body
//! that changes tab by tab. The tabs are its own -- Description, Gallery, Versions
//! -- plus the two the router reaches without a tab of their own (a single version
//! and the changelog).
//!
//! Everything here comes from Modrinth's API. What this page therefore does today
//! is draw the *shape*: the header with its facts filled in when they arrive, the
//! tab strip live (a tab is navigation, not data), and each tab's four states.
//!
//! The description is markdown in the reference -- it renders `project.body`
//! through the same `markdown-body` stylesheet the web app uses. Drawing raw
//! markdown as prose is deliberate and temporary: it shows the text the reference
//! would show, minus its formatting, and a *wrong* rendering (bold as literal
//! asterisks, say) is what a hand-rolled parser produces before it is finished.
//! The subset renderer is the next piece of work on this page and says so here.

use iced::widget::{column, row, text};
use iced::{Element, Length};

use crate::icons_gen::Glyph;
use crate::page::{self, Load, GAP, ROW_GAP};
use crate::route::ProjectTab;
use crate::store::{self, Store};
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

/// What the page can be told.
#[derive(Debug, Clone)]
pub enum Message {
    /// Another tab was chosen.
    Tab(ProjectTab),
    /// The project was asked for again.
    Refresh,
    /// The project's main action was pressed.
    Install,
}

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
}

impl State {
    /// A page for one project, on one tab.
    pub fn new(id: String, tab: ProjectTab) -> State {
        State {
            id,
            tab,
            project: Load::Failed(store::not_implemented("This project")),
            notice: None,
        }
    }

    /// Apply a message.
    pub fn update(&mut self, message: Message) {
        match message {
            Message::Tab(tab) => self.tab = tab,
            Message::Refresh => self.project = Load::Failed(store::not_implemented("This project")),
            Message::Install => self.notice = Some(store::not_implemented("Installing a project")),
        }
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
    blocks.push(ui::tabs(theme, &state.labels(), |index| {
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
                        Key::AppLibraryContextMenuCreateInstance,
                        ui::Kind::Colored,
                        Message::Install,
                    ))
                    .push(ui::button(
                        theme,
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
    fn a_project_that_cannot_be_read_says_so_rather_than_showing_an_empty_one() {
        let state = State::new("sodium".to_string(), ProjectTab::Description);
        let reason = state.project.failure().expect("a reason");
        assert!(reason.contains("is not implemented yet"), "{reason}");
        assert_ne!(state.project, Load::Empty, "an unmade request is not an empty project");
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
                // The failed arm, then the ready one with data, then empty.
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
    fn installing_says_what_arrives_later_and_refreshing_asks_again() {
        let mut state = State::new("sodium".to_string(), ProjectTab::Description);
        state.update(Message::Install);
        assert!(state.notice.as_deref().unwrap_or_default().contains("is not implemented yet"));
        state.update(Message::Tab(ProjectTab::Versions));
        assert_eq!(state.tab, ProjectTab::Versions);
        state.update(Message::Refresh);
        assert!(state.project.failure().is_some());
    }
}
