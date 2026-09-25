//! A user's profile: `pages/User.vue`, at `/user/:user`.
//!
//! The page is a header (the name, the avatar, when they joined and how much they
//! have been downloaded) over tabs of their projects and collections. All of it
//! comes from Modrinth's API, so what is here is the header's shape, the tabs, and
//! the reference's own empty sentences -- *"This user has no projects!"* and *"You
//! don't have any projects yet."* -- which are the two states the reference draws
//! for a profile it can see but that has nothing in it, and for the reader's own
//! empty profile.
//!
//! The distinction between those two sentences is kept rather than collapsed: one
//! is about somebody else, the other about the reader, and the reference uses the
//! second only when the profile is the reader's own.

use iced::widget::{column, row, Space};
use iced::{Alignment, Element, Length};

use crate::page::{self, GAP, ROW_GAP};
use crate::store::{self, Store};
use crate::style::{heading, medium, INK_CONTRAST, INK_SECONDARY};
use crate::text_gen::Key;
use crate::theme_gen::{self, Theme as Gen};
use crate::ui;

/// Which list the page is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    /// The user's projects, which the reference opens on.
    #[default]
    Projects,
    /// Their collections.
    Collections,
    /// Their organizations.
    Organizations,
}

impl Tab {
    /// Every tab, in the reference's order.
    pub const ALL: [Tab; 3] = [Tab::Projects, Tab::Collections, Tab::Organizations];

    /// The reference's key for this tab's label.
    pub const fn key(self) -> Key {
        match self {
            Tab::Projects => Key::ProfileLabelNoProjects,
            Tab::Collections => Key::ProfileLabelNoCollections,
            Tab::Organizations => Key::ProfileLabelOrganizations,
        }
    }
}

/// What the page can be told.
#[derive(Debug, Clone)]
pub enum Message {
    /// Another tab was chosen.
    Tab(Tab),
    /// The profile was asked for again.
    Refresh,
}

/// The page's own state.
#[derive(Debug, Clone, Default)]
pub struct State {
    /// Which list is showing.
    pub tab: Tab,
    /// The last thing the page could not do.
    pub notice: Option<String>,
}

impl State {
    /// Apply a message.
    pub fn update(&mut self, message: Message) {
        match message {
            Message::Tab(tab) => self.tab = tab,
            Message::Refresh => self.notice = Some(store::unavailable("This profile")),
        }
    }

    /// The sentence to draw for a profile with nothing in the chosen tab.
    ///
    /// The reference has one sentence for "somebody else has none" and one for
    /// "you have none", and which is right depends on whether the profile belongs
    /// to the reader. That is a parameter rather than a guess.
    pub fn empty_sentence(&self, own_profile: bool) -> &'static str {
        match (self.tab, own_profile) {
            (Tab::Projects, true) => Key::ProfileLabelNoProjectsAuthDescription.message(),
            (Tab::Projects, false) => Key::ProfileLabelNoProjects.message(),
            (Tab::Collections, _) => Key::ProfileLabelNoCollections.message(),
            (Tab::Organizations, _) => Key::ProfileLabelOrganizations.message(),
        }
    }
}

/// Draw the page for one user.
///
/// The store is not read yet, and is taken rather than dropped so that the page's
/// signature is the one every page has: stage 4's followers, projects and
/// organizations all come from it, and a page that had to change shape to start
/// answering from a service would change [`crate::pages::Screen::view`] with it.
pub fn view<'a>(
    theme: Gen,
    state: &'a State,
    _store: &'a Store,
    user: &'a str,
) -> Element<'a, Message> {
    let mut blocks: Vec<Element<'a, Message>> = Vec::new();
    if let Some(notice) = &state.notice {
        blocks.push(ui::admonition(theme, ui::Severity::Info, user, notice));
    }
    blocks.push(
        row![]
            .spacing(ROW_GAP)
            .align_items(Alignment::Center)
            .push(page::title(theme, Key::AppNavViewProfile))
            .push(
                iced::widget::text(user)
                    .size(24.0)
                    .font(heading())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
            )
            .push(Space::with_width(Length::Fill))
            .push(ui::button(
                theme,
                Key::AppLibrarySortLabel,
                ui::Kind::Quiet,
                Message::Refresh,
            ))
            .into(),
    );
    // The labels come from `Tab::key`, which is the one place a tab's copy is
    // named: a strip that spelled its own keys would be a second list to keep in
    // step with `Tab::ALL`.
    let labels: Vec<(String, bool)> = Tab::ALL
        .iter()
        .map(|tab| (tab.key().message().to_string(), *tab == state.tab))
        .collect();
    blocks.push(ui::tabs(theme, &labels, |index| {
        Message::Tab(Tab::ALL.get(index).copied().unwrap_or_default())
    }));
    // The tab's own states: the sentence above, and the reason there is nothing
    // else on the page yet.
    blocks.push(ui::card(
        theme,
        column![]
            .spacing(4.0)
            .push(
                iced::widget::text(state.empty_sentence(false))
                    .size(16.0)
                    .font(heading())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
            )
            .push(
                iced::widget::text(store::unavailable("This profile's projects and collections"))
                    .size(14.0)
                    .font(medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
            ),
    ));
    // The header's own facts, which the reference fills from the profile: named
    // here so the page's gaps are the reference's gaps rather than this file's.
    blocks.push(ui::metadata(theme, Key::ProfileLabelJoined, "—"));
    page::body(blocks, GAP)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_tabs_are_the_reference_s_and_carry_its_labels() {
        assert_eq!(Tab::ALL.len(), 3);
        assert_eq!(Tab::default(), Tab::Projects);
        assert_eq!(Tab::Projects.key().message(), "This user has no projects!");
        assert_eq!(Tab::Organizations.key().message(), "Organizations");
    }

    #[test]
    fn an_empty_profile_says_the_right_sentence_for_who_it_is() {
        let mut state = State::default();
        // Somebody else's empty profile, and the reader's own: two different
        // sentences in the reference, and the difference is kept.
        assert_eq!(state.empty_sentence(false), "This user has no projects!");
        assert_eq!(state.empty_sentence(true), "You don't have any projects yet.");
        assert_ne!(state.empty_sentence(true), state.empty_sentence(false));
        state.update(Message::Tab(Tab::Collections));
        assert_eq!(state.empty_sentence(true), "This user has no collections!");
        state.update(Message::Tab(Tab::Organizations));
        assert_eq!(state.empty_sentence(true), "Organizations");
    }

    #[test]
    fn refreshing_a_profile_says_what_arrives_later() {
        let mut state = State::default();
        assert!(state.notice.is_none());
        state.update(Message::Refresh);
        assert!(state.notice.as_deref().unwrap_or_default().contains("stage 4"));
        state.update(Message::Tab(Tab::Collections));
        assert_eq!(state.tab, Tab::Collections);
    }

    #[test]
    fn the_page_draws_in_every_theme_and_every_tab() {
        let store = Store::default();
        for theme in Gen::ALL {
            for tab in Tab::ALL {
                let state = State { tab, notice: None };
                drop(view(*theme, &state, &store, "jelly"));
            }
            let state = State { tab: Tab::Projects, notice: Some("x".into()) };
            drop(view(*theme, &state, &store, "a name with spaces"));
        }
    }
}
