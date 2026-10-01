//! Screenshots, the reference's fourth rail slot: `pages/Screenshots.vue`.
//!
//! This is the one page of the reference whose data is *entirely* local: it lists
//! every screenshot in every instance, which is a directory read per instance and
//! needs no service at all. So it is real here -- the images themselves, their
//! instance and their order are read from disk by [`crate::store`] -- and the one
//! part that is not real is the thumbnails, which are the engine's image cache and
//! are named as such rather than decoded on every frame.
//!
//! The reference's own `screenshots::scan` and its thumbnail generator are in
//! `crate::screenshots`; what this page adds is the page around them.

use std::path::PathBuf;

use iced::widget::{column, row, Space};
use iced::{Alignment, Element, Length};

use crate::page::{self, GAP, ROW_GAP};
use crate::store::{self, Store};
use crate::style::{heading, medium, semibold, INK_CONTRAST, INK_SECONDARY};
use crate::text_gen::Key;
use crate::theme_gen::{self, Theme as Gen};
use crate::ui::{self, text};

/// One screenshot found on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shot {
    /// The instance it belongs to.
    pub instance: String,
    /// The file's name.
    pub name: String,
    /// Where it is, for the reader that will open it.
    pub path: PathBuf,
}

/// What the page can be told.
#[derive(Debug, Clone)]
pub enum Message {
    /// The search text changed.
    Search(String),
    /// A screenshot was asked to be opened.
    ///
    /// Which one is not in the message: nothing can open it yet, and a payload
    /// the page cannot act on would be a promise about a viewer that does not
    /// exist. The variant gains the shot it names when the viewer does.
    Open,
    /// The last notice was dismissed.
    DismissNotice,
    /// The pointer entered or left one of the page's controls, for the clock
    /// that carries a hover's 150 ms (see [`crate::ui`]). This page has no
    /// control of that kind yet -- a screenshot tile is a picture, not a button
    /// -- and the variant is here because the crossing is the page's to route
    /// whichever control grows it first.
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
#[derive(Debug, Clone, Default)]
pub struct State {
    /// The search text, which filters in place.
    pub search: String,
    /// The last thing the page could not do.
    pub notice: Option<String>,
}

impl State {
    /// Apply a message.
    pub fn update(&mut self, message: Message) {
        match message {
            // A wheel is not this page's to apply: see `crate::scroll`.
            Message::Wheel(..) => {},

            Message::Search(text) => self.search = text,
            Message::Open => {
                self.notice = Some(store::not_implemented("Opening a screenshot in a viewer"));
            }
            Message::DismissNotice => self.notice = None,
            Message::Hover { key, over, hover } => crate::ui::pointer_with(
                key,
                over,
                hover.unwrap_or_else(crate::theme::hover_brightness),
            ),
        }
    }

    /// The shots a search leaves visible.
    ///
    /// Matched against the instance's name as well as the file's, which is how a
    /// person looks for one: by the instance they took it in.
    pub fn visible<'a>(&self, shots: &'a [Shot]) -> Vec<&'a Shot> {
        let needle = self.search.trim().to_ascii_lowercase();
        shots
            .iter()
            .filter(|shot| {
                needle.is_empty()
                    || shot.name.to_ascii_lowercase().contains(&needle)
                    || shot.instance.to_ascii_lowercase().contains(&needle)
            })
            .collect()
    }
}

/// Every instance's screenshots, newest name first within each instance.
pub fn scan(store: &Store) -> Vec<Shot> {
    let mut shots = Vec::new();
    let page::Load::Ready(cards) = store.instances() else {
        return shots;
    };
    for card in cards {
        let folder = store.instance_dir(&card.id).join("screenshots");
        for name in store::screenshots(&store.instance_dir(&card.id)) {
            let path = folder.join(&name);
            shots.push(Shot { instance: card.name.clone(), name, path });
        }
    }
    shots
}

/// Draw the page.
pub fn view<'a>(theme: Gen, state: &'a State, store: &'a Store) -> Element<'a, Message> {
    let shots = scan(store);
    let mut blocks: Vec<Element<'a, Message>> = Vec::new();
    if let Some(notice) = &state.notice {
        blocks.push(page::notice(
            theme,
            Key::AppScreenshotsHeading,
            notice,
            Message::DismissNotice,
        ));
    }
    blocks.push(
        row![]
            .spacing(ROW_GAP)
            .align_items(Alignment::Center)
            .push(page::title(theme, Key::AppScreenshotsHeading))
            .push(Space::with_width(Length::Fill))
            .push(ui::search(
                theme,
                Key::AppLibrarySearchPlaceholder.message(),
                &state.search,
                Message::Search,
            ))
            .into(),
    );
    // Three states, in the reference's own words: nothing at all, nothing that
    // matched, and the list.
    if shots.is_empty() {
        blocks.push(ui::card(
            theme,
            column![]
                .spacing(4.0)
                .push(heading_line(theme, Key::AppScreenshotsEmptyHeading))
                .push(body_line(theme, Key::AppScreenshotsEmptyDescription.message())),
        ));
        return page::body(blocks, GAP, Message::Wheel);
    }
    let visible = state.visible(&shots);
    if visible.is_empty() {
        blocks.push(ui::card(
            theme,
            column![]
                .spacing(4.0)
                .push(heading_line(theme, Key::AppScreenshotsNoResultsHeading))
                .push(body_line(theme, &store::not_implemented("Thumbnails"))),
        ));
        return page::body(blocks, GAP, Message::Wheel);
    }
    let mut list = column![].spacing(GAP).width(Length::Fill);
    for shot in visible {
        let row_body = row![]
            .spacing(ROW_GAP)
            .align_items(Alignment::Center)
            .push(
                column![]
                    .spacing(2.0)
                    .push(
                        text(shot.name.clone())
                            .size(14.0)
                            .font(semibold())
                            .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
                    )
                    .push(ui::icon_label(theme, crate::icons_gen::Glyph::Image, &shot.instance)),
            )
            .push(Space::with_width(Length::Fill))
            // The file itself, which is the one fact that needs no service.
            .push(ui::icon_label(
                theme,
                crate::icons_gen::Glyph::Folder,
                &shot.path.display().to_string(),
            ));
        list = list.push(ui::card(
            theme,
            iced::widget::mouse_area(row_body)
                .interaction(iced::mouse::Interaction::Pointer)
                .on_press(Message::Open),
        ));
    }
    blocks.push(list.into());
    // The thumbnails, said once rather than once per tile.
    blocks.push(ui::admonition(
        theme,
        ui::Severity::Info,
        Key::AppScreenshotsGroupBy.message(),
        &store::not_implemented("Screenshot thumbnails"),
    ));
    page::body(blocks, GAP, Message::Wheel)
}

/// A card heading, in the reference's extra-bold face.
fn heading_line<'a>(theme: Gen, key: Key) -> Element<'a, Message> {
    text(key.message())
        .size(16.0)
        .font(heading())
        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST)))
        .into()
}

/// A card's body sentence.
fn body_line<'a>(theme: Gen, sentence: &str) -> Element<'a, Message> {
    text(sentence.to_string())
        .size(14.0)
        .font(medium())
        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY)))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use palantir_core::paths::PalantirPaths;

    fn shot(instance: &str, name: &str) -> Shot {
        Shot {
            instance: instance.to_string(),
            name: name.to_string(),
            path: PathBuf::from(name),
        }
    }

    #[test]
    fn a_search_matches_the_file_and_the_instance_it_was_taken_in() {
        let shots = vec![
            shot("All the Mods 10", "2026-01-01_12.00.00.png"),
            shot("Sodium Test", "2026-02-02_09.30.00.png"),
        ];
        let mut state = State::default();
        assert_eq!(state.visible(&shots).len(), 2);
        state.search = "sodium".to_string();
        assert_eq!(state.visible(&shots)[0].instance, "Sodium Test");
        state.search = "2026-01-01".to_string();
        assert_eq!(state.visible(&shots)[0].instance, "All the Mods 10");
        state.search = "nothing".to_string();
        assert!(state.visible(&shots).is_empty());
    }

    #[test]
    fn a_store_with_nothing_scanned_has_no_shots_and_that_is_not_an_error() {
        let paths = PalantirPaths::at(std::env::temp_dir().join("palantirmc-shots-page"));
        let store = Store::load(&paths);
        assert!(scan(&store).is_empty());
    }

    #[test]
    fn every_state_draws_in_every_theme() {
        let paths = PalantirPaths::at(std::env::temp_dir().join("palantirmc-shots-draw"));
        let store = Store::load(&paths);
        for theme in Gen::ALL {
            for notice in [None, Some("x".to_string())] {
                let state = State { search: String::new(), notice };
                drop(view(*theme, &state, &store));
            }
        }
    }

    #[test]
    fn opening_a_screenshot_says_what_arrives_later() {
        let mut state = State::default();
        state.update(Message::Open);
        assert!(state.notice.as_deref().unwrap_or_default().contains("is not implemented yet"));
        state.update(Message::DismissNotice);
        assert!(state.notice.is_none());
    }
}
