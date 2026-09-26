//! One instance: `pages/instance/layout.vue`, at `/instance/:id/:tab?`.
//!
//! This is the page the reference puts the most inside: its own header (the icon,
//! the name, the loader and game version, the Play button) and its own navigation,
//! with six tabs -- Content, Files, Worlds, Screenshots, Logs, Share.
//!
//! Five of those six are *local*, which is why this page is the most real one in
//! the interface before the engine exists: an instance's mods, worlds, files,
//! screenshots and newest log are all on this machine, and [`crate::store`] reads
//! them. Only Share needs a service, and it says so.
//!
//! The header's own numbers come from the reference's layout: `p-6 pr-2 pb-4` for
//! the header, `px-6` for the tab strip, and `p-6 pt-4` for the body -- 24px, with
//! the tab strip flush to the sides and the body starting 16px below it.
//!
//! **What is read on every frame, and why that is temporary.** The tab bodies read
//! the filesystem when they are drawn, so a mod that was toggled shows its new
//! state immediately without a cache to invalidate. That is a directory read per
//! frame, which is fine for the tens of files an instance has and would not be for
//! a folder of screenshots -- which is one of the things the store exists to fix,
//! and why the screenshots tab lists names rather than decoding images.

use iced::widget::{column, row, text, Space};
use iced::{Alignment, Element, Font, Length};

use crate::icons_gen::Glyph;
use crate::page::{self, GAP, ROW_GAP};
use crate::pages::Ask;
use crate::route::InstanceTab;
use crate::store::{self, LaunchState, Store};
use crate::style::{semibold, INK_CONTRAST, INK_SECONDARY};
use crate::text_gen::Key;
use crate::theme_gen::{self, Theme as Gen};
use crate::ui;

/// What the page can be told.
#[derive(Debug, Clone)]
pub enum Message {
    /// Another tab was chosen.
    Tab(InstanceTab),
    /// The instance was asked to be launched.
    Play,
    /// The running instance was asked to stop.
    Stop,
    /// An instance's mod was enabled or disabled.
    ToggleContent {
        /// The file on disk, which is what the toggle acts on.
        file_name: String,
        /// What it should become.
        enabled: bool,
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
const TAB_KEYS: [&str; 6] = [
    "instance:tab:content",
    "instance:tab:files",
    "instance:tab:worlds",
    "instance:tab:screenshots",
    "instance:tab:logs",
    "instance:tab:share",
];

/// The page's own action. The per-file toggle is one control per row, so it names
/// itself from the file it acts on -- see [`crate::ui::scoped`].
const PLAY_KEY: &str = "instance:play";
/// The same control once the game is up: the reference's red Stop, which is the
/// same button in a different state and therefore a control of its own on the
/// clock.
const STOP_KEY: &str = "instance:stop";

/// The page's own state.
#[derive(Debug, Clone)]
pub struct State {
    /// Which instance.
    pub id: String,
    /// Which tab.
    pub tab: InstanceTab,
    /// The last thing the page could not do, shown rather than swallowed.
    pub notice: Option<String>,
}

impl State {
    /// A page for one instance, on one tab.
    pub fn new(id: String, tab: InstanceTab) -> State {
        State { id, tab, notice: None }
    }

    /// The key for a tab's label.
    pub const fn key_for(tab: InstanceTab) -> Key {
        match tab {
            // A content *filter* is the Content tab with a kind chosen, so it
            // keeps the Content tab's own label, which is what the reference's
            // `is-primary` predicate on that tab does.
            InstanceTab::Content | InstanceTab::ContentFilter(_) => Key::AppInstanceTabContent,
            InstanceTab::Files => Key::AppInstanceTabFiles,
            InstanceTab::Worlds => Key::AppInstanceTabWorlds,
            InstanceTab::Screenshots => Key::AppInstanceTabScreenshots,
            InstanceTab::Logs => Key::AppInstanceTabLogs,
            InstanceTab::Share => Key::AppInstanceTabShare,
        }
    }

    /// The tabs the strip shows, in the reference's order.
    ///
    /// The five always-visible ones: the reference hides Mods and Datapacks
    /// inside an instance and Servers outside it, which is a rule about *content
    /// filters* rather than about tabs, so the strip is fixed and the filter lives
    /// on the Content tab.
    pub const TABS: [InstanceTab; 6] = [
        InstanceTab::Content,
        InstanceTab::Files,
        InstanceTab::Worlds,
        InstanceTab::Screenshots,
        InstanceTab::Logs,
        InstanceTab::Share,
    ];

    /// What the tab strip shows.
    pub fn labels(&self) -> Vec<(String, bool)> {
        State::TABS
            .iter()
            .map(|tab| {
                let selected = match (tab, &self.tab) {
                    (InstanceTab::Content, InstanceTab::ContentFilter(_)) => true,
                    (left, right) => left == right,
                };
                (State::key_for(tab.clone()).message().to_string(), selected)
            })
            .collect()
    }

    /// The tab a strip press selects.
    pub const fn tab_at(index: usize) -> InstanceTab {
        match index {
            0 => InstanceTab::Content,
            1 => InstanceTab::Files,
            2 => InstanceTab::Worlds,
            3 => InstanceTab::Screenshots,
            4 => InstanceTab::Logs,
            _ => InstanceTab::Share,
        }
    }

    /// Apply a message, and report anything only the shell can do.
    ///
    /// The toggle is the one place this page changes the disk, and it is
    /// deliberate: a Content tab that lists mods but cannot switch one off is a
    /// list, not a control. The call is [`crate::mods::set_mod_enabled`], the same
    /// one the old interface uses, and a failure is shown rather than dropped.
    ///
    /// Running the game is *not* one of those: the page reports it
    /// ([`Ask::Play`]) for [`Ask`]'s own reason -- the process, the account, the
    /// memory and the Java are the shell's and the store's, and a page that could
    /// start one would have to know all four.
    pub fn update(&mut self, message: Message, store: &Store) -> Option<Ask> {
        match message {
            Message::Tab(tab) => self.tab = tab,
            Message::Play => return Some(Ask::Play(self.id.clone())),
            Message::Stop => return Some(Ask::Stop(self.id.clone())),
            Message::ToggleContent { file_name, enabled } => {
                let directory = store.instance_dir(&self.id).join("mods");
                self.notice = crate::mods::set_mod_enabled(&directory, &file_name, enabled)
                    .err()
                    .map(|error| format!("Could not change {file_name}: {error}"));
            }
            Message::Hover { key, over, hover } => crate::ui::pointer_with(
                key,
                over,
                hover.unwrap_or_else(crate::theme::hover_brightness),
            ),
        }
        None
    }
}

/// Draw the page.
pub fn view<'a>(theme: Gen, state: &'a State, store: &'a Store) -> Element<'a, Message> {
    let mut blocks: Vec<Element<'a, Message>> = Vec::new();
    blocks.push(header(theme, state, store));
    if let Some(notice) = &state.notice {
        blocks.push(ui::admonition(theme, ui::Severity::Warning, &state.id, notice));
    }
    blocks.push(ui::tabs(theme, &TAB_KEYS, &state.labels(), |index| {
        Message::Tab(State::tab_at(index))
    }));
    blocks.push(body(theme, state, store));
    page::body(blocks, GAP)
}

/// The header: the instance's name, what it is, and Play.
fn header<'a>(theme: Gen, state: &'a State, store: &'a Store) -> Element<'a, Message> {
    let card = store.instance(&state.id);
    let title = match &card {
        page::Load::Ready(card) => card.name.clone(),
        _ => state.id.clone(),
    };
    let mut details = column![]
        .spacing(4.0)
        .push(
            text(title)
                .size(24.0)
                .font(crate::style::heading())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
        );
    if let Some(card) = card.ready() {
        let mut facts = row![].spacing(ROW_GAP);
        facts = facts.push(ui::tag(theme, &card.subtitle()));
        facts = facts.push(ui::icon_label(theme, Glyph::Clock, &card.playtime_label()));
        if card.mods_total > 0 {
            facts = facts.push(ui::icon_label(
                theme,
                Glyph::Package,
                &format!("{} / {}", card.mods_enabled, card.mods_total),
            ));
        }
        details = details.push(facts);
    } else {
        details = details.push(ui::paragraph(
            theme,
            &store::not_implemented("This instance's details"),
        ));
    }
    // The launch's own last word, in the header rather than in a bar along the
    // bottom of the window: the reference puts it in its action bar, which is a
    // surface this shell does not have yet, and the fact is the same one either
    // way. It stays after the run has ended, which is why it is looked up by
    // instance id rather than by "is it running".
    if let Some(line) = store.launch_line(&state.id) {
        details = details.push(
            text(line.to_string())
                .size(12.0)
                .font(crate::style::medium())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
        );
    }
    ui::card(
        theme,
        row![]
            .spacing(GAP)
            .align_items(Alignment::Center)
            .push(details.width(Length::Fill))
            .push(launch_control(theme, store.launch_state(&state.id))),
    )
}

/// The header's launch control, in whichever of the reference's four states the
/// launch is in.
///
/// `page-header/index.vue` draws one of four things in this one place: *Play*
/// while nothing is running, *Starting…* while the launcher is preparing (disabled
/// -- there is nothing to stop yet), a red *Stop* once the game is up, and
/// *Stopping…* while it is being taken down. Two of those are buttons that do
/// something and the other two are the same button with its press removed, which
/// is what [`crate::ui::button_or`] is for.
fn launch_control(theme: Gen, state: LaunchState) -> Element<'static, Message> {
    match state {
        LaunchState::Idle => ui::button(
            theme,
            PLAY_KEY,
            Key::AppInstanceActionPlay,
            ui::Kind::Colored,
            Message::Play,
        ),
        LaunchState::Starting => ui::button_or(
            theme,
            PLAY_KEY,
            Key::InstanceActionStarting,
            ui::Kind::Colored,
            None,
        ),
        LaunchState::Running => ui::button(
            theme,
            STOP_KEY,
            Key::ButtonStop,
            ui::Kind::Danger,
            Message::Stop,
        ),
        LaunchState::Stopping => ui::button_or(
            theme,
            STOP_KEY,
            Key::InstanceActionStopping,
            ui::Kind::Danger,
            None,
        ),
    }
}

/// The chosen tab's own body.
fn body<'a>(theme: Gen, state: &'a State, store: &'a Store) -> Element<'a, Message> {
    let directory = store.instance_dir(&state.id);
    match state.tab {
        InstanceTab::Content | InstanceTab::ContentFilter(_) => {
            let mods = store::content(&directory);
            if mods.is_empty() {
                return ui::card(
                    theme,
                    column![]
                        .spacing(4.0)
                        .push(
                            text(Key::ContentPageLayoutEmptyNoContentInstalled.message())
                                .size(16.0)
                                .font(semibold())
                                .style(iced::theme::Text::Color(theme_gen::ink(
                                    theme,
                                    INK_CONTRAST,
                                ))),
                        )
                        .push(ui::paragraph(
                            theme,
                            &store::not_implemented("Browsing for content to install"),
                        )),
                );
            }
            let mut list = column![].spacing(GAP).width(Length::Fill);
            for entry in mods {
                let toggle = ui::button(
                    theme,
                    crate::ui::scoped("instance:content:toggle", &entry.file_name),
                    if entry.enabled {
                        Key::AppScreenshotsDeselect
                    } else {
                        Key::AppScreenshotsEdit
                    },
                    if entry.enabled { ui::Kind::Standard } else { ui::Kind::Quiet },
                    Message::ToggleContent {
                        file_name: entry.file_name.clone(),
                        enabled: !entry.enabled,
                    },
                );
                list = list.push(ui::card(
                    theme,
                    row![]
                        .spacing(ROW_GAP)
                        .align_items(Alignment::Center)
                        .push(ui::icon_label(theme, Glyph::Package, &entry.display_name))
                        .push(Space::with_width(Length::Fill))
                        .push(toggle),
                ));
            }
            list.into()
        }
        InstanceTab::Files => {
            let entries = store::files(&directory);
            // The emptiness is asked of the slice rather than of the built
            // column: `Column`'s children are not public, and a view that had to
            // look inside its own widget to know whether to draw it would be a
            // view that cannot be read on its own.
            if entries.is_empty() {
                return page::empty(theme, Key::BrowseNoResults);
            }
            let mut list = column![].spacing(4.0).width(Length::Fill);
            for entry in entries {
                let glyph = if entry.directory { Glyph::Folder } else { Glyph::File };
                let label = if entry.directory {
                    entry.name.clone()
                } else {
                    format!("{} · {}", entry.name, store::bytes_label(entry.bytes))
                };
                list = list.push(ui::icon_label(theme, glyph, &label));
            }
            ui::card(theme, list)
        }
        InstanceTab::Worlds => {
            let worlds = store::worlds(&directory);
            if worlds.is_empty() {
                return ui::card(
                    theme,
                    column![]
                        .spacing(4.0)
                        .push(
                            text(Key::AppInstanceWorldsNoWorldsHeading.message())
                                .size(16.0)
                                .font(semibold())
                                .style(iced::theme::Text::Color(theme_gen::ink(
                                    theme,
                                    INK_CONTRAST,
                                ))),
                        )
                        .push(ui::paragraph(
                            theme,
                            Key::AppInstanceWorldsNoWorldsDescription.message(),
                        )),
                );
            }
            let mut list = column![].spacing(4.0).width(Length::Fill);
            for world in worlds {
                let state_label = if world.played { "played" } else { "never opened" };
                list = list.push(ui::icon_label(theme, Glyph::Globe, &format!("{} · {state_label}", world.name)));
            }
            ui::card(theme, list)
        }
        InstanceTab::Screenshots => {
            let shots = store::screenshots(&directory);
            if shots.is_empty() {
                return ui::card(
                    theme,
                    column![]
                        .spacing(4.0)
                        .push(
                            text(Key::AppScreenshotsEmptyHeading.message())
                                .size(16.0)
                                .font(semibold())
                                .style(iced::theme::Text::Color(theme_gen::ink(
                                    theme,
                                    INK_CONTRAST,
                                ))),
                        )
                        .push(ui::paragraph(
                            theme,
                            Key::AppScreenshotsEmptyDescription.message(),
                        )),
                );
            }
            let mut list = column![].spacing(4.0).width(Length::Fill);
            for name in shots {
                list = list.push(ui::icon_label(theme, Glyph::Image, &name));
            }
            ui::card(theme, list)
        }
        InstanceTab::Logs => match store::log_tail(&directory, 500) {
            Some(tail) => ui::card(
                theme,
                text(tail)
                    .size(12.0)
                    .font(Font::MONOSPACE)
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
            ),
            None => page::empty(theme, Key::BrowseNoResults),
        },
        InstanceTab::Share => ui::card(
            theme,
            column![]
                .spacing(4.0)
                .push(
                    text(Key::AppInstanceShareLockedSignedOutHeading.message())
                        .size(16.0)
                        .font(semibold())
                        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
                )
                .push(ui::paragraph(theme, &store::not_implemented("Sharing an instance"))),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use palantir_core::paths::PalantirPaths;

    fn store_at(name: &str) -> Store {
        let root = std::env::temp_dir().join("palantirmc-instance-page").join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a scratch directory");
        Store::load(&PalantirPaths::at(root))
    }

    #[test]
    fn the_strip_is_the_six_tabs_the_reference_shows_and_the_keys_are_its_own() {
        assert_eq!(State::TABS.len(), 6);
        let expected = ["Content", "Files", "Worlds", "Screenshots", "Logs", "Share"];
        let labels: Vec<&str> =
            State::TABS.iter().map(|tab| State::key_for(tab.clone()).message()).collect();
        assert_eq!(labels, expected);
        // A content filter keeps the Content tab selected, which is what the
        // reference's own predicate on that tab does.
        let state = State::new(
            "atm".to_string(),
            InstanceTab::ContentFilter(crate::route::ProjectType::Mod),
        );
        let strip = state.labels();
        assert!(strip[0].1, "the filter keeps Content selected");
        assert!(strip[1..].iter().all(|(_, selected)| !selected));
        assert_eq!(strip.len(), 6);
        // And each strip position maps back to the tab it shows.
        for (index, tab) in State::TABS.iter().enumerate() {
            assert_eq!(State::tab_at(index), *tab);
        }
        assert_eq!(State::tab_at(99), InstanceTab::Share);
    }

    #[test]
    fn a_toggle_that_cannot_reach_the_folder_says_so_rather_than_doing_nothing() {
        // The store's instance directory does not exist in this scratch root, so
        // the rename fails and the page has to say so: a toggle that silently did
        // nothing is the failure this test exists to catch.
        let store = store_at("toggle-missing");
        let mut state = State::new("no-such-instance".to_string(), InstanceTab::Content);
        state.update(
            Message::ToggleContent { file_name: "sodium.jar".to_string(), enabled: false },
            &store,
        );
        assert!(state.notice.is_some(), "a failed toggle must be reported");
        assert_eq!(state.tab, InstanceTab::Content);
    }

    #[test]
    fn every_tab_draws_for_an_instance_that_is_not_there() {
        // The empty arms are the interesting ones: an address can name an instance
        // that has been deleted, and the page has to draw rather than panic.
        let store = store_at("missing-instance");
        for theme in Gen::ALL {
            for tab in State::TABS {
                let mut state = State::new("gone".to_string(), tab);
                drop(view(*theme, &state, &store));
                state.notice = Some("x".to_string());
                drop(view(*theme, &state, &store));
            }
        }
    }

    #[test]
    fn playing_and_stopping_are_reported_rather_than_performed() {
        // The page does not run the game, for the same reason it does not create
        // an instance: the process, the account, the memory and the Java are not
        // its, and the shell is the one that has them.
        let store = store_at("play");
        let mut state = State::new("atm".to_string(), InstanceTab::Content);
        assert_eq!(state.update(Message::Play, &store), Some(Ask::Play("atm".to_string())));
        assert_eq!(state.update(Message::Stop, &store), Some(Ask::Stop("atm".to_string())));
        assert_eq!(state.update(Message::Tab(InstanceTab::Logs), &store), None);
        assert_eq!(state.tab, InstanceTab::Logs);
    }

    #[test]
    fn the_header_s_control_is_the_state_the_launch_is_in() {
        // Four states, four controls, and the four labels are the reference's own
        // (`page-header/index.vue`): *Play*, *Starting...* while the launcher is
        // preparing, *Stop* once the game is up, and *Stopping...* while it is
        // being taken down.
        for (state_of_run, label) in [
            (LaunchState::Idle, "Play"),
            (LaunchState::Starting, "Starting..."),
            (LaunchState::Running, "Stop"),
            (LaunchState::Stopping, "Stopping..."),
        ] {
            assert_eq!(launch_state_label(state_of_run), label, "{state_of_run:?}");
        }
        let mut store = store_at("launch-control");
        let mut state = State::new("atm".to_string(), InstanceTab::Content);
        store.set_launch(store::Launch {
            instance: Some("atm".to_string()),
            state: LaunchState::Running,
            line: Some("process started, streaming output…".to_string()),
        });
        assert_eq!(store.launch_state("atm"), LaunchState::Running);
        assert_eq!(state.update(Message::Stop, &store), Some(Ask::Stop("atm".to_string())));
        // And every state draws in every theme: the control is one of four
        // shapes in a card, and a shape the page cannot draw is a panic in front
        // of a user rather than a gate.
        for state_of_run in [
            LaunchState::Idle,
            LaunchState::Starting,
            LaunchState::Running,
            LaunchState::Stopping,
        ] {
            store.set_launch(store::Launch {
                instance: Some("atm".to_string()),
                state: state_of_run,
                line: None,
            });
            for theme in Gen::ALL {
                drop(view(*theme, &state, &store));
            }
        }
    }

    /// The label a launch state draws: the same four keys the control draws from,
    /// named here so a drift in the copy is a failure rather than a look.
    fn launch_state_label(state: LaunchState) -> &'static str {
        match state {
            LaunchState::Idle => Key::AppInstanceActionPlay.message(),
            LaunchState::Starting => Key::InstanceActionStarting.message(),
            LaunchState::Running => Key::ButtonStop.message(),
            LaunchState::Stopping => Key::InstanceActionStopping.message(),
        }
    }

    #[test]
    fn the_launch_s_own_line_is_shown_for_its_instance_and_no_other() {
        // The line is the launch's last word, and it outlives the run: a user who
        // navigates away and back sees what happened rather than a page that looks
        // as if nothing had.
        let store = store_at("launch-line");
        let mut store = store;
        store.set_launch(store::Launch {
            instance: Some("atm".to_string()),
            state: LaunchState::Idle,
            line: Some("process exited (exit status: 0)".to_string()),
        });
        assert_eq!(
            store.launch_line("atm"),
            Some("process exited (exit status: 0)"),
            "the page draws this under the facts"
        );
        assert_eq!(store.launch_line("other"), None);
        let state = State::new("other".to_string(), InstanceTab::Content);
        drop(view(Gen::Dark, &state, &store));
    }
}
