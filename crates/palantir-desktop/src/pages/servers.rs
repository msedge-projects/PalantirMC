//! Servers, the reference's fifth rail slot: the Modrinth Hosting listing at
//! `/hosting/manage/`.
//!
//! The reference's page is a listing of servers the user has bought from Modrinth
//! Hosting, and every fact on it comes from that service: the plans, their state,
//! the backups, the console. There is no local half to draw -- a server is not a
//! file on this machine -- so this page is the shape of the listing and the states
//! around it, in the reference's own words, with the reason the middle is empty
//! said out loud rather than left as a spinner.
//!
//! The rail slot's tooltip is `app.nav.modrinth-hosting` -- "Modrinth Hosting",
//! which is what the button means -- and it is used as this page's heading rather
//! than paraphrasing the reference's own name for it.

use iced::widget::{column, row, Space};
use iced::{Alignment, Element, Length};

use crate::page::{self, GAP, ROW_GAP};
use crate::store::{self, Store};
use crate::text_gen::Key;
use crate::theme_gen::{self, Theme as Gen};
use crate::ui;

/// What the page can be told.
#[derive(Debug, Clone)]
pub enum Message {
    /// A new server was asked for.
    NewServer,
    /// The listing was asked for again.
    Refresh,
    /// Billing was asked for.
    ManageBilling,
}

/// The page's own state.
#[derive(Debug, Clone, Default)]
pub struct State {
    /// The last thing the page could not do.
    pub notice: Option<String>,
}

impl State {
    /// Apply a message.
    pub fn update(&mut self, message: Message) {
        match message {
            Message::NewServer => self.notice = Some(store::unavailable("Creating a server")),
            Message::ManageBilling => self.notice = Some(store::unavailable("Billing")),
            Message::Refresh => self.notice = Some(store::unavailable("The server listing")),
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
            Key::AppNavModrinthHosting.message(),
            notice,
        ));
    }
    blocks.push(
        row![]
            .spacing(ROW_GAP)
            .align_items(Alignment::Center)
            .push(page::title(theme, Key::AppNavModrinthHosting))
            .push(Space::with_width(Length::Fill))
            .push(ui::button(
                theme,
                Key::ServersListingManageBillingLabel,
                ui::Kind::Standard,
                Message::ManageBilling,
            ))
            .push(ui::button(
                theme,
                Key::ServersListingNewLabel,
                ui::Kind::Colored,
                Message::NewServer,
            ))
            .into(),
    );
    // The listing's own two states: the service could not be reached, or it
    // answered with fewer servers than the page can draw a list from. Both are
    // said in the reference's words -- this page has no local data to fall back on,
    // so "no servers" would be a claim it cannot make.
    blocks.push(ui::card(
        theme,
        column![]
            .spacing(4.0)
            .push(
                iced::widget::text(Key::ServersManageErrorTitle.message())
                    .size(16.0)
                    .font(crate::style::heading())
                    .style(iced::theme::Text::Color(theme_gen::ink(
                        theme,
                        crate::style::INK_CONTRAST,
                    ))),
            )
            .push(ui::paragraph(theme, &store::unavailable("The server listing"))),
    ));
    blocks.push(ui::button(
        theme,
        Key::AppLibraryContextMenuCreateInstance,
        ui::Kind::Quiet,
        Message::Refresh,
    ));
    page::body(blocks, GAP)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_this_page_cannot_do_yet_says_which_stage_does_it() {
        let mut state = State::default();
        for message in [Message::NewServer, Message::ManageBilling, Message::Refresh] {
            state.update(message);
            let notice = state.notice.clone().expect("a notice");
            assert!(notice.contains("stage 4"), "{notice}");
        }
    }

    #[test]
    fn the_page_draws_in_every_theme() {
        let store = Store::default();
        for theme in Gen::ALL {
            for notice in [None, Some("x".to_string())] {
                let state = State { notice };
                drop(view(*theme, &state, &store));
            }
        }
    }
}
