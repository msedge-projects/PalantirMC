//! Servers, the reference's fifth rail slot: the Modrinth Hosting listing at
//! `/hosting/manage/`.
//!
//! The reference's page is a listing of servers the user has bought from Modrinth
//! Hosting, and every fact on it comes from that service: the plans, their state,
//! the backups, the console. There is no local half to draw -- a server is not a
//! file on this machine -- and no account half either, because this launcher does
//! not hold a Modrinth credential (G118), so the middle of the page says which
//! service it is waiting for rather than pretending a slice is coming for it.
//!
//! The rail slot's tooltip is `app.nav.modrinth-hosting` -- "Modrinth Hosting",
//! which is what the button means -- and it is used as this page's heading rather
//! than paraphrasing the reference's own name for it. A launcher that runs a server
//! *of its own* -- a jar in an instance's folder, started like a game -- is a
//! different feature from this page and would not be this route.

use iced::widget::{column, row, Space};
use iced::{Alignment, Element, Length};

use crate::page::{self, GAP, ROW_GAP};
use crate::store::{self, Store};
use crate::text_gen::Key;
use crate::theme_gen::{self, Theme as Gen};
use crate::ui::{self, text};

/// What the page can be told.
#[derive(Debug, Clone)]
pub enum Message {
    /// A new server was asked for.
    NewServer,
    /// The listing was asked for again.
    Refresh,
    /// Billing was asked for.
    ManageBilling,
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

    /// A wheel over this page's scroll region.
    ///
    /// Reported rather than applied: iced moves a scrollable with a `scroll_to`
    /// command, so which region glides, and how far, is the shell's -- see
    /// `crate::scroll`. This page's part is to hand the wheel on, and the name it
    /// carries is the region the widget was built with.
    Wheel(&'static str, crate::scroll::Wheel),
}

crate::hovered!(Message);

/// The listing's actions.
const MANAGE_BILLING_KEY: &str = "servers:manage-billing";
const NEW_SERVER_KEY: &str = "servers:new";
const REFRESH_KEY: &str = "servers:refresh";

/// The page's own state.
#[derive(Debug, Clone, Default)]
pub struct State {
    /// The last thing the page could not do.
    pub notice: Option<String>,
}

impl State {
    /// Apply a message.
    ///
    /// Every one of the three answers with [`store::needs_account`] rather than
    /// with "not implemented yet": the listing, a new server and the billing page
    /// are all Modrinth Hosting, which is an account service, and the sentence a
    /// reader gets has to say which of the two kinds of gap this is.
    pub fn update(&mut self, message: Message) {
        match message {
            // A wheel is not this page's to apply: see `crate::scroll`.
            Message::Wheel(..) => {},

            Message::NewServer => self.notice = Some(store::needs_account("Creating a server")),
            Message::ManageBilling => self.notice = Some(store::needs_account("Billing")),
            Message::Refresh => self.notice = Some(store::needs_account("The server listing")),
            Message::Hover { key, over, hover } => crate::ui::pointer_with(
                key,
                over,
                hover.unwrap_or_else(crate::theme::hover_brightness),
            ),
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
            // Both are the frame's default row: the reference states no `size`
            // on these controls, and a button without one is `md`, 36 pixels.
            .push(ui::button_sized(
                theme,
                MANAGE_BILLING_KEY,
                Key::ServersListingManageBillingLabel,
                ui::Kind::Standard,
                ui::Size::Md,
                Message::ManageBilling,
            ))
            .push(ui::button_sized(
                theme,
                NEW_SERVER_KEY,
                Key::ServersListingNewLabel,
                ui::Kind::Colored,
                ui::Size::Md,
                Message::NewServer,
            ))
            .into(),
    );
    // The listing's own two states: the service could not be reached, or it
    // answered with fewer servers than the page can draw a list from. Both are
    // said in the reference's words -- this page has no local data to fall back
    // on, so "no servers" would be a claim it cannot make -- and the reason there
    // is nothing behind either of them is the account this launcher does not hold.
    blocks.push(ui::card(
        theme,
        column![]
            .spacing(4.0)
            .push(
                text(Key::ServersManageErrorTitle.message())
                    .size(16.0)
                    .font(crate::style::heading())
                    .style(iced::theme::Text::Color(theme_gen::ink(
                        theme,
                        crate::style::INK_CONTRAST,
                    ))),
            )
            .push(ui::paragraph(theme, &store::needs_account("The server listing"))),
    ));
    blocks.push(ui::button_sized(
        theme,
        REFRESH_KEY,
        Key::AppLibraryContextMenuCreateInstance,
        ui::Kind::Quiet,
        ui::Size::Md,
        Message::Refresh,
    ));
    page::body(blocks, GAP, Message::Wheel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_says_the_service_it_needs_rather_than_that_it_is_unbuilt() {
        // The distinction this page exists to keep now: a Modrinth Hosting action
        // is not waiting for a slice, it is waiting for an account this launcher
        // does not hold. A notice that read "is not implemented yet" would be a
        // promise nothing is keeping.
        let mut state = State::default();
        for message in [Message::NewServer, Message::ManageBilling, Message::Refresh] {
            state.update(message);
            let notice = state.notice.clone().expect("a notice");
            assert!(notice.contains("Modrinth account"), "{notice}");
            assert!(!notice.contains("is not implemented yet"), "{notice}");
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
