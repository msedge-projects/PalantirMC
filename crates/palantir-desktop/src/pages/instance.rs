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
//! **A tab's listing is read once, not once per frame.** The tab bodies used to
//! read the filesystem where they were drawn, which made every frame a directory
//! walk: [`crate::scale`] measured the Files tab at 2,304 ms of frame at five
//! thousand entries, 2,311 ms of it the read. The read is a *load* now --
//! [`State::opening`] asks for the tab's listing when the tab is entered, the
//! shell makes the call off the frame thread ([`crate::pages::Ask::Instance`]),
//! and the answer arrives as [`Message::Listed`] -- so a frame draws what is
//! already here and opens no directory to paint itself. Five of the six tabs have
//! a listing; Share is a service's page and has nothing to read.
//!
//! **And a tab draws the rows a reader can see, not all of them.** With the read
//! moved off the frame the remaining cost was the drawing itself -- 32.0 ms of
//! frame at five thousand mods, 6.4 us a row -- and that is a cost no amount of
//! caching removes, because the rows are built and laid out and rasterised once
//! per frame. What removes it is building only the rows in the scroll region's
//! window ([`crate::scroll::window`], a port of the reference's own
//! `useVirtualScroll`, which its content tab and files tab both use): the tab body
//! is the scroll region ([`scrolling`], the reference's `'fixed'` render mode),
//! the region reports where it is, and the rows outside the window are two spacers
//! holding their place. A frame's cost is then a function of the *window*, not of
//! the folder.

use iced::widget::{column, container, row, Space};
use iced::{Alignment, Element, Font, Length, Padding};

use crate::icons_gen::Glyph;
use crate::page::{self, Load, GAP, INSET, ROW_GAP};
use crate::pages::Ask;
use crate::route::InstanceTab;
use crate::scroll::{self, Geometry};
use crate::store::{self, LaunchState, Store};
use crate::style::{semibold, INK_CONTRAST, INK_SECONDARY};
use crate::text_gen::Key;
use crate::theme_gen::{self, Theme as Gen};
use crate::ui::{self, text};

/// What the page can be told.
#[derive(Debug, Clone)]
pub enum Message {
    /// Another tab was chosen.
    Tab(InstanceTab),
    /// The instance was asked to be launched.
    Play,
    /// The header's settings control: open this instance's own settings.
    Settings,
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
    /// The listing the shell read for the tab on screen.
    ///
    /// The answer to [`Asked`], and the only way a tab's rows arrive: the read
    /// happens off the frame thread, and the round it was made in travels with it
    /// so an answer to a tab the reader has left is dropped rather than drawn.
    Listed {
        /// Which read this answers.
        round: u64,
        /// The rows, or the sentence for why there are none.
        listing: Result<store::Listing, String>,
    },
    /// The tab body's scroll region reported where it is.
    ///
    /// The only thing that moves the window ([`scroll::window`]): it arrives on
    /// every wheel, drag and keyboard scroll, and the frame built from it draws
    /// the rows that report puts on screen.
    Scrolled(Geometry),

    /// A wheel over this page's scroll region.
    ///
    /// Reported rather than applied: iced moves a scrollable with a `scroll_to`
    /// command, so which region glides, and how far, is the shell's -- see
    /// `crate::scroll`. This page's part is to hand the wheel on, and the name it
    /// carries is the region the widget was built with.
    Wheel(&'static str, crate::scroll::Wheel),
}

crate::hovered!(Message);

/// One stable name per tab, in `State::TABS`' order.
const TAB_KEYS: [&str; 6] = [
    "instance:tab:content",
    "instance:tab:files",
    "instance:tab:screenshots",
    "instance:tab:worlds",
    "instance:tab:logs",
    "instance:tab:share",
];

/// The icon each tab registers, in [`State::TABS`]' order.
///
/// `layout.vue`'s own `tabs` computed, which pushes *Screenshots* before *Worlds*
/// -- the order the strip is drawn in, and not the order this file's enum
/// declares them in: `BoxesIcon`, `FolderOpenIcon`, `ImageIcon`, `GlobeIcon`,
/// `TerminalSquareIcon`, `UserPlusIcon`.
const TAB_GLYPHS: [Option<Glyph>; 6] = [
    Some(Glyph::Boxes),
    Some(Glyph::FolderOpen),
    Some(Glyph::Image),
    Some(Glyph::Globe),
    Some(Glyph::TerminalSquare),
    Some(Glyph::UserPlus),
];

/// How tall one row of the Content tab's listing is, in pixels.
///
/// A row's *slot*, not its card: [`slot`] gives every row exactly this much room
/// and the card inside it takes what it needs, so a window is arithmetic rather
/// than a measurement of the text in it. The number is the card's own parts added
/// up -- the toggle (`ui::CONTROL`, 40px) plus the card's padding above and below
/// (`ui::CARD_PAD` twice) plus its hairline top and bottom -- and then the gap the
/// tab already put between two cards (`page::GAP`), so a windowed listing is
/// spaced the way the unwindowed one was.
///
/// A name longer than the card is drawn on one line rather than wrapped (see
/// [`listing_body`]), which is what keeps the sum above true: a row that wrapped
/// would need a height that depends on a string, and the reference's own
/// `itemHeight` has the same constant. It is 74px there, for a card that carries
/// two lines and a row of actions; this one is a single line.
/// Public because the measurement in [`crate::scale`] computes the same window the
/// view does, and a row height that had drifted from the one the rows are given
/// would make that measurement describe a page nobody draws.
pub const CONTENT_ROW: f32 = ui::CONTROL + ui::CARD_PAD * 2.0 + 2.0 + GAP;

/// How tall one row of the other listings is, in pixels.
///
/// The same idea at the other size: an icon and a label, which is about 20px of
/// content, in a slot with the 4px the tab already spaced its rows by. Public for
/// [`CONTENT_ROW`]'s reason: the measurement computes the window these rows are
/// drawn in.
pub const PLAIN_ROW: f32 = 24.0;

/// The namespace a Content row's toggle is named in.
///
/// Each row's own name is interned into it when the listing arrives, once per row
/// rather than once per row per frame: [`crate::ui::scoped`] takes a process-wide
/// lock and formats a string on the way in, which `crate::scale` measured at
/// 9.4 ms of every frame for five thousand rows.
const CONTENT_TOGGLE: &str = "instance:content:toggle";

/// The page's own action. The per-file toggle is one control per row, so it names
/// itself from the file it acts on -- see [`crate::ui::scoped`].
const PLAY_KEY: &str = "instance:play";
/// The same control once the game is up: the reference's red Stop, which is the
/// same button in a different state and therefore a control of its own on the
/// clock.
const STOP_KEY: &str = "instance:stop";
/// The header's gear, which opens the instance-settings modal.
const SETTINGS_KEY: &str = "instance:settings";

/// What the page asks the shell for: one tab's own read.
///
/// A value rather than a call, which is the whole seam: the read happens off the
/// frame thread, and the answer travels back as [`Message::Listed`]. The round
/// counts from one and is what lets a slow answer to a tab that is gone be
/// dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    /// Which read this is, counting from one.
    pub round: u64,
    /// Which instance.
    pub id: String,
    /// Which tab's listing.
    pub tab: InstanceTab,
}

/// Whether a tab's body is a listing that has to be read.
///
/// Five of the six are: four folders and the log's tail. Share is a service's
/// page, and its card is the same on every frame.
const fn tab_has_listing(tab: &InstanceTab) -> bool {
    !matches!(tab, InstanceTab::Share)
}

/// The page's own state.
#[derive(Debug, Clone)]
pub struct State {
    /// Which instance.
    pub id: String,
    /// Which tab.
    pub tab: InstanceTab,
    /// The last thing the page could not do, shown rather than swallowed.
    pub notice: Option<String>,
    /// The tab's own listing, read when the tab was entered rather than drawn.
    pub listed: Load<store::Listing>,
    /// The clock name of every loaded Content row, in the listing's own order.
    ///
    /// Interned once, when the listing arrived. A content row's toggle is a
    /// control of its own -- two rows must not share a key, or hovering one would
    /// light both -- and its name is the file it acts on.
    keys: Vec<&'static str>,
    /// Which read is in flight, so an answer the page has replaced is dropped by
    /// [`State::update`] rather than drawn.
    round: u64,
    /// Where the tab body's scroll region is, as it last reported.
    ///
    /// Defaulted rather than measured, because a region that nobody has scrolled
    /// has never reported: the first frame of a tab is drawn inside the window a
    /// window-sized guess gives ([`scroll::INITIAL_VIEW`]), and the first wheel
    /// event replaces it with the truth.
    geometry: Geometry,
}

impl State {
    /// A page for one instance, on one tab.
    pub fn new(id: String, tab: InstanceTab) -> State {
        State {
            id,
            tab,
            notice: None,
            listed: Load::Idle,
            keys: Vec::new(),
            round: 0,
            geometry: Geometry::default(),
        }
    }

    /// The scroll region's last report, for the tests that place a window.
    #[cfg(test)]
    pub fn geometry(&self) -> Geometry {
        self.geometry
    }

    /// The loaded Content rows' clock names, for the tests that check when they
    /// are built.
    #[cfg(test)]
    pub fn keys(&self) -> &[&'static str] {
        &self.keys
    }

    /// The read this page owes because nothing has been asked for its tab yet.
    ///
    /// [`crate::pages::Screen::opening`]'s other half, and the same idea as
    /// Discover's: a tab that has been entered has a listing owed to it, and the
    /// shell asks on the page's behalf -- including for the page a window opened
    /// straight onto. Share owes nothing, because it has no listing to read.
    pub fn opening(&mut self) -> Option<Asked> {
        if matches!(self.listed, Load::Idle) && tab_has_listing(&self.tab) {
            Some(self.ask())
        } else {
            None
        }
    }

    /// Bump the round, mark the tab as waiting, and describe the read.
    fn ask(&mut self) -> Asked {
        self.round += 1;
        self.listed = Load::Loading;
        Asked { round: self.round, id: self.id.clone(), tab: self.tab.clone() }
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
    /// `layout.vue`'s `tabs` computed pushes Content, then Files, then
    /// **Screenshots**, then Worlds, then Logs, then Share -- the two settings-gated
    /// ones in the order that computed pushes them, which is not the order this
    /// crate's enum declares. The strip is fixed here: the reference hides Mods and
    /// Datapacks inside an instance and Servers outside it, which is a rule about
    /// *content filters* rather than about tabs, so the filter lives on the
    /// Content tab.
    pub const TABS: [InstanceTab; 6] = [
        InstanceTab::Content,
        InstanceTab::Files,
        InstanceTab::Screenshots,
        InstanceTab::Worlds,
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
        // [`State::TABS`]' own order, which is `layout.vue`'s: Screenshots is
        // the third tab and Worlds the fourth.
        match index {
            0 => InstanceTab::Content,
            1 => InstanceTab::Files,
            2 => InstanceTab::Screenshots,
            3 => InstanceTab::Worlds,
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
            // A wheel is not this page's to apply: see `crate::scroll`.
            Message::Wheel(..) => {},

            Message::Tab(tab) => {
                // A tab is a different read, and the listing in hand belongs to
                // the tab that is gone. Marked idle rather than rewritten, so the
                // shell's own `opening_command` asks for the new one on this same
                // turn -- which is what Discover does on a project-type change,
                // and why this arm reports nothing.
                self.tab = tab;
                self.listed = Load::Idle;
                self.keys.clear();
            }
            Message::Play => return Some(Ask::Play(self.id.clone())),
            // The modal is the shell's, so the page reports the press rather than
            // opening anything: the read behind it is this instance's file, which
            // a page has never touched.
            Message::Settings => return Some(Ask::InstanceSettings(self.id.clone())),
            Message::Stop => return Some(Ask::Stop(self.id.clone())),
            Message::ToggleContent { file_name, enabled } => {
                let directory = store.instance_dir(&self.id).join("mods");
                self.notice = crate::mods::set_mod_enabled(&directory, &file_name, enabled)
                    .err()
                    .map(|error| format!("Could not change {file_name}: {error}"));
                // The file on disk changed state, so the listing in hand is
                // stale. Asked for again rather than patched in place: the read is
                // what decides what is there and whether it is disabled, and a
                // page that edited its own copy would be a second answer to that
                // question. This is also why a toggle still shows up on the next
                // turn, which was the only good property of reading per frame.
                self.listed = Load::Idle;
                self.keys.clear();
            }
            Message::Listed { round, listing } => {
                // An answer to a read the page has replaced: the reader changed
                // tab since, and drawing this would put one tab's rows under
                // another tab's heading.
                if round != self.round {
                    return None;
                }
                self.listed = match listing {
                    Err(reason) => Load::Failed(reason),
                    Ok(listing) if listing.is_empty() => Load::Empty,
                    Ok(listing) => Load::Ready(listing),
                };
                self.keys = self
                    .listed
                    .ready()
                    .map(content_keys)
                    .unwrap_or_default();
            }
            Message::Hover { key, over, hover } => crate::ui::pointer_with(
                key,
                over,
                hover.unwrap_or_else(crate::theme::hover_brightness),
            ),
            // Nothing else to do with it: where the region is *is* the page's
            // state, and the next frame ([`view`]) is the one that uses it. A tab
            // change deliberately does not reset it -- iced's own scrollable keeps
            // its offset across one, and a page that disagreed with it would draw
            // the wrong rows rather than the ones on screen.
            Message::Scrolled(at) => self.geometry = at,
        }
        None
    }
}

/// Draw the page.
///
/// **The header and the tab strip are pinned, and the tab body is the scroll
/// region.** That is the reference's own arrangement rather than an invention of
/// this port: `instance/Layout.vue` has two render modes, `'scroll'` where the
/// whole page moves inside `.app-viewport` and `'fixed'` where the header and tabs
/// are `shrink-0` and the body is `min-h-0 flex-1 overflow-y-auto`. A windowed
/// listing needs the second one, and for the reason the reference names as well as
/// this one's: rows are placed at their own offsets ([`scroll::window`]) inside a
/// region whose height is reported, and a page that scrolled as a whole could not
/// say where its list starts without measuring everything drawn above it.
pub fn view<'a>(theme: Gen, state: &'a State, store: &'a Store) -> Element<'a, Message> {
    let mut above = column![].spacing(GAP).width(Length::Fill);
    above = above.push(header(theme, state, store));
    if let Some(notice) = &state.notice {
        above = above.push(ui::admonition(theme, ui::Severity::Warning, &state.id, notice));
    }
    above = above.push(ui::tabs_with_glyphs(
        theme,
        &TAB_KEYS,
        &TAB_GLYPHS,
        &state.labels(),
        |index| Message::Tab(State::tab_at(index)),
    ));
    // The pinned part keeps the page's own inset; the body below it keeps the
    // gap that used to separate them, and the inset the whole page used to put
    // inside the scroll region, so what a reader sees is where it was.
    let pinned = container(above).width(Length::Fill).padding(Padding {
        top: INSET,
        right: INSET,
        bottom: 0.0,
        left: INSET,
    });
    column![pinned, scrolling(theme, state)]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// The tab body's scroll region, which reports where it is.
///
/// Its content is the whole body -- one card, or a windowed column of rows -- and
/// its own height is whatever the pinned part above leaves, which is the number
/// [`scroll::window`] is handed on every report. The inset lives inside it, as it
/// did when the page was one scroll region, so the scrollbar is against the pane's
/// edge and the text does not slide under its rounded corner.
fn scrolling<'a>(theme: Gen, state: &'a State) -> Element<'a, Message> {
    crate::scroll::region(
        crate::scroll::CONTENT,
        container(body(theme, state)).width(Length::Fill).padding(Padding {
            top: GAP,
            right: INSET,
            bottom: INSET,
            left: INSET,
        }),
        Message::Wheel,
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .on_scroll(|viewport| Message::Scrolled(scroll::Geometry::of(viewport)))
    .into()
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
            // The reference's own order in `PageHeaderActions`: the launch button
            // first and the gear to its right, both at its `size="xl"` -- a
            // 48-pixel round `IconButton` carrying a 24-pixel icon. The modal the
            // gear opens is the shell's (see [`crate::instance_settings`]).
            .push(launch_control(theme, store.launch_state(&state.id)))
            .push(ui::icon_button_sized(
                theme,
                SETTINGS_KEY,
                Glyph::Settings,
                ui::Kind::Standard,
                ui::Size::Xl,
                Message::Settings,
            )),
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
/// is what [`crate::ui::button_or_sized`] is for.
///
/// All four are the reference's `size="xl"` row -- 48 pixels, `rounded-2xl`, a
/// 24-pixel icon and its one `font-extrabold` label -- which is the same row the
/// page's own *Play* and *Stop* are drawn at (`pages/project/Index.vue`). The
/// icons are the reference's: its `PlayIcon` on the idle button and its
/// `StopCircleIcon` on both stop states, with the disabled *Starting…* the one
/// button it draws without one.
fn launch_control(theme: Gen, state: LaunchState) -> Element<'static, Message> {
    match state {
        LaunchState::Idle => ui::button_with_icon_sized(
            theme,
            PLAY_KEY,
            Glyph::Play,
            Key::AppInstanceActionPlay,
            ui::Kind::Colored,
            ui::Size::Xl,
            Length::Shrink,
            Some(Message::Play),
        ),
        LaunchState::Starting => ui::button_or_sized(
            theme,
            PLAY_KEY,
            Key::InstanceActionStarting,
            ui::Kind::Colored,
            ui::Size::Xl,
            None,
        ),
        LaunchState::Running => ui::button_with_icon_sized(
            theme,
            STOP_KEY,
            Glyph::StopCircle,
            Key::ButtonStop,
            ui::Kind::Danger,
            ui::Size::Xl,
            Length::Shrink,
            Some(Message::Stop),
        ),
        LaunchState::Stopping => ui::button_with_icon_sized(
            theme,
            STOP_KEY,
            Glyph::StopCircle,
            Key::InstanceActionStopping,
            ui::Kind::Danger,
            ui::Size::Xl,
            Length::Shrink,
            None,
        ),
    }
}

/// The chosen tab's own body.
///
/// **Reads nothing.** Every arm draws the listing the shell read when the tab was
/// entered ([`Message::Listed`]), which is what takes the frame's cost off the
/// size of the folder being drawn. The version this replaced walked a directory
/// per frame and rebuilt a row per file in it; [`crate::scale`] has what that
/// cost at five thousand entries, and why it was worth changing.
fn body<'a>(theme: Gen, state: &'a State) -> Element<'a, Message> {
    // The one tab with nothing to read: its card is the same on every frame. What
    // it says is [`store::needs_account`] rather than "not implemented yet":
    // sharing is `shared-instances.modrinth.com` and the reader's own Modrinth
    // account, which this launcher does not hold (G118).
    if let InstanceTab::Share = state.tab {
        return ui::card(
            theme,
            column![]
                .spacing(4.0)
                .push(
                    text(Key::AppInstanceShareLockedSignedOutHeading.message())
                        .size(16.0)
                        .font(semibold())
                        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
                )
                .push(ui::paragraph(theme, &store::needs_account("Sharing an instance"))),
        );
    }
    match &state.listed {
        Load::Ready(listing) => listing_body(theme, state, listing),
        Load::Empty => empty_body(theme, &state.tab),
        Load::Failed(reason) => page::failed(theme, reason),
        // A tab that has just been entered asks for its listing on the same turn,
        // so a reader sees this block for one frame -- except when they arrived by
        // address, where it stands for as long as the read takes.
        Load::Idle | Load::Loading => page::waiting(theme, "this instance's files"),
    }
}

/// One tab's rows, from the listing that was read for it.
///
/// **Windowed**: only the rows the region reports are on screen are built, and
/// the rest of the list is space. Every row is in a slot of one height ([`slot`])
/// so that the rows that are drawn sit at the offsets they would have in the whole
/// list; the spacers [`rows`] puts above and below them hold the scrollbar where
/// the whole list would have put it, which is what makes the scrollbar's size a
/// property of the listing rather than of the window.
///
/// Two details are deliberate rather than incidental. A row's label does not wrap
/// (`Wrapping::None`): a wrapped name is a row of a height that depends on a
/// string, and the window's arithmetic is only as good as the row height it is
/// given. And the Content tab's card is built with a [`Message::ToggleContent`]
/// that owns its file name, so nothing in a drawn row borrows the listing -- which
/// is what lets the spacers stand in for the rows that are not.
fn listing_body<'a>(
    theme: Gen,
    state: &'a State,
    listing: &'a store::Listing,
) -> Element<'a, Message> {
    match listing {
        store::Listing::Content(mods) => rows(mods.len(), CONTENT_ROW, state.geometry, |index| {
            let entry = &mods[index];
            // `ContentCardTable.vue`'s own control, at the row's own start: a
            // `Checkbox` with `shrink-0` in front of the name, which is where the
            // reference puts it and not a labelled button at the row's end.
            let toggle = ui::checkbox(
                theme,
                // The name came out of the clock's table when the listing
                // arrived rather than on this frame: see [`CONTENT_TOGGLE`].
                state.keys.get(index).copied().unwrap_or(CONTENT_TOGGLE),
                entry.enabled,
                false,
                Message::ToggleContent {
                    file_name: entry.file_name.clone(),
                    enabled: !entry.enabled,
                },
            );
            slot(ui::card(
                theme,
                row![]
                    .spacing(ROW_GAP)
                    .align_items(Alignment::Center)
                    .push(toggle)
                    .push(ui::icon_label(theme, Glyph::Package, &entry.display_name))
                    .push(Space::with_width(Length::Fill)),
            ), CONTENT_ROW)
        }),
        store::Listing::Files(entries) => {
            ui::card(theme, files_rows(theme, entries, state.geometry))
        }
        store::Listing::Worlds(worlds) => ui::card(
            theme,
            rows(worlds.len(), PLAIN_ROW, state.geometry, |index| {
                let world = &worlds[index];
                let state_label = if world.played { "played" } else { "never opened" };
                slot(
                    ui::icon_label(
                        theme,
                        Glyph::Globe,
                        &format!("{} · {state_label}", world.name),
                    ),
                    PLAIN_ROW,
                )
            }),
        ),
        store::Listing::Screenshots(shots) => ui::card(
            theme,
            rows(shots.len(), PLAIN_ROW, state.geometry, |index| {
                slot(ui::icon_label(theme, Glyph::Image, &shots[index]), PLAIN_ROW)
            }),
        ),
        store::Listing::Log(tail) => ui::card(
            theme,
            text(tail.as_str())
                .size(12.0)
                .font(Font::MONOSPACE)
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
        ),
    }
}

/// The Files tab's rows: a directory or a file, with the size of the latter.
///
/// Split out of [`listing_body`] because the closure that builds one row is long
/// enough that inlining it makes the match unreadable, and because the two things
/// that are the same in every windowed listing -- the slot and the window -- are
/// then visible next to each other.
fn files_rows<'a>(
    theme: Gen,
    entries: &'a [store::Entry],
    at: Geometry,
) -> Element<'a, Message> {
    rows(entries.len(), PLAIN_ROW, at, |index| {
        let entry = &entries[index];
        let glyph = if entry.directory { Glyph::Folder } else { Glyph::File };
        let label = if entry.directory {
            entry.name.clone()
        } else {
            format!("{} · {}", entry.name, store::bytes_label(entry.bytes))
        };
        slot(ui::icon_label(theme, glyph, &label), PLAIN_ROW)
    })
}

/// A windowed column of rows: the ones in `at`'s window, and spacers where the
/// rest of them would have been.
///
/// The spacers are the part this is easy to get wrong -- the content's *height* is
/// what the scrollbar is drawn against, so a window that dropped the rows it did
/// not draw would be a list that scrolls as if it were one screen long. Their
/// total is exact because a row is exactly `row_height` tall ([`slot`]).
fn rows<'a>(
    count: usize,
    row_height: f32,
    at: Geometry,
    mut build: impl FnMut(usize) -> Element<'a, Message>,
) -> Element<'a, Message> {
    let drawn = scroll::window(count, row_height, at);
    let mut list = column![].spacing(0.0).width(Length::Fill);
    if drawn.start > 0 {
        list = list.push(space(drawn.start, row_height));
    }
    for index in drawn.clone() {
        list = list.push(build(index));
    }
    if drawn.end < count {
        list = list.push(space(count - drawn.end, row_height));
    }
    list.into()
}

/// One row's slot: exactly `row_height` tall, whatever the row inside it needs.
///
/// The window's arithmetic places each drawn row at `index * row_height`, and the
/// row is drawn at the top of its slot for the same reason the tab's rows were
/// top-aligned before there was a window: the gap between two cards is *under* the
/// upper one. The top of a container is `Vertical::Top` here, not
/// `iced::Alignment::Start`: the two enums name the same end and it is the
/// vertical one the widget asks for.
fn slot<'a>(content: Element<'a, Message>, row_height: f32) -> Element<'a, Message> {
    container(content)
        .width(Length::Fill)
        .height(Length::Fixed(row_height))
        .align_y(iced::alignment::Vertical::Top)
        .into()
}

/// The room `count` rows would have taken, as one empty widget.
fn space<'a>(count: usize, row_height: f32) -> Element<'a, Message> {
    Space::with_height(Length::Fixed(count as f32 * row_height)).into()
}



/// What a tab draws when its listing came back with nothing in it.
///
/// Three of the five have the reference's own copy for this -- a heading and the
/// sentence under it -- and the two that do not get the scaffold's card. The
/// emptiness is asked of the listing ([`store::Listing::is_empty`]) rather than of
/// a built column, because `Column`'s children are not public and a view that had
/// to look inside its own widget would be a view that cannot be read on its own.
fn empty_body<'a>(theme: Gen, tab: &InstanceTab) -> Element<'a, Message> {
    match tab {
        InstanceTab::Content | InstanceTab::ContentFilter(_) => titled(
            theme,
            Key::ContentPageLayoutEmptyNoContentInstalled,
            &store::not_implemented("Browsing for content to install"),
        ),
        InstanceTab::Worlds => titled(
            theme,
            Key::AppInstanceWorldsNoWorldsHeading,
            Key::AppInstanceWorldsNoWorldsDescription.message(),
        ),
        InstanceTab::Screenshots => titled(
            theme,
            Key::AppScreenshotsEmptyHeading,
            Key::AppScreenshotsEmptyDescription.message(),
        ),
        _ => page::empty(theme, Key::BrowseNoResults),
    }
}

/// The reference's own empty card: a heading, and the sentence under it.
fn titled<'a>(theme: Gen, heading: Key, body: &str) -> Element<'a, Message> {
    ui::card(
        theme,
        column![]
            .spacing(4.0)
            .push(
                text(heading.message())
                    .size(16.0)
                    .font(semibold())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
            )
            .push(ui::paragraph(theme, body)),
    )
}

/// The clock names of a Content listing's rows, in the listing's own order.
///
/// Interned once per listing rather than once per row per frame. [`crate::ui::scoped`]
/// keeps a process-wide table behind a mutex and formats a name on the way in, so
/// asking it from the view cost 9.4 ms of every frame at five thousand rows
/// (`crate::scale`); this is the same call, made when the rows arrive.
fn content_keys(listing: &store::Listing) -> Vec<&'static str> {
    match listing {
        store::Listing::Content(rows) => rows
            .iter()
            .map(|entry| crate::ui::scoped(CONTENT_TOGGLE, &entry.file_name))
            .collect(),
        _ => Vec::new(),
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
        // `layout.vue`'s own order, Screenshots before Worlds -- the order its
        // `tabs` computed pushes them in, not the order the enum declares.
        let expected = ["Content", "Files", "Screenshots", "Worlds", "Logs", "Share"];
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
        // Each tab's icon is the one `layout.vue` registers with it, in the same
        // order, so a strip whose glyphs were shuffled would still be the right
        // six glyphs beside the wrong six labels.
        assert_eq!(
            TAB_GLYPHS,
            [
                Some(Glyph::Boxes),
                Some(Glyph::FolderOpen),
                Some(Glyph::Image),
                Some(Glyph::Globe),
                Some(Glyph::TerminalSquare),
                Some(Glyph::UserPlus),
            ]
        );
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

    /// Drive one tab the way the shell does: ask, read, deliver.
    ///
    /// The read goes through [`store::listing`], which is the same call the
    /// shell's worker makes, so a test exercises the whole seam rather than a
    /// hand-built answer.
    fn load(state: &mut State, store: &Store) -> Option<Asked> {
        let asked = state.opening()?;
        let directory = store.instance_dir(&asked.id);
        let listing = store::listing(&directory, &asked.tab);
        state.update(Message::Listed { round: asked.round, listing }, store);
        Some(asked)
    }

    fn mod_entry(name: &str) -> crate::mods::ModEntry {
        crate::mods::ModEntry {
            file_name: name.to_string(),
            display_name: name.trim_end_matches(".jar").to_string(),
            enabled: true,
        }
    }

    #[test]
    fn a_tab_is_read_once_and_a_tab_change_asks_again() {
        // The whole shape of the slice: a tab's listing is read when the tab is
        // entered, and drawing it asks for nothing. A page that asked on every
        // turn would be the per-frame read again with extra steps.
        let store = store_at("tabs");
        let mut state = State::new("atm".to_string(), InstanceTab::Content);
        let asked = load(&mut state, &store).expect("the tab owes a read");
        assert_eq!(asked.tab, InstanceTab::Content);
        assert!(state.opening().is_none(), "and owes no second one");

        // Another tab is another listing, so another read -- counted in rounds,
        // so the answer that is in flight for the old tab can be dropped.
        state.update(Message::Tab(InstanceTab::Files), &store);
        let asked = state.opening().expect("the new tab's read");
        assert_eq!(asked.tab, InstanceTab::Files);
        assert_eq!(asked.round, 2, "one round per read, not per tab");

        // Share has no listing to read, so it owes nothing for as long as it is
        // the tab on screen: a page that asked anyway would loop on an answer it
        // has nowhere to put.
        state.update(Message::Tab(InstanceTab::Share), &store);
        assert!(state.opening().is_none(), "Share has no listing");
    }

    #[test]
    fn an_answer_to_a_tab_the_reader_has_left_is_dropped() {
        // The rows of a tab the reader has left must not land under the heading of
        // the one they are on: a slow read of `mods/` arriving after a press on
        // Files would draw jars under the file list.
        let store = store_at("stale");
        let mut state = State::new("atm".to_string(), InstanceTab::Content);
        let first = load(&mut state, &store).expect("a read");
        state.update(Message::Tab(InstanceTab::Files), &store);

        // The shell asks for the new tab's listing on the same turn the tab
        // changed (`opening_command`), so this is the state a slow answer arrives
        // into: the page is waiting for Files, and Content's rows are in the post.
        // A tab change that did not ask would be a page that never fills, which is
        // why the round moves on the *ask* rather than on the tab.
        let second = state.opening().expect("the new tab's read");
        assert_ne!(second.round, first.round, "a tab change is a new read");

        let stale = Ok(store::Listing::Content(vec![mod_entry("sodium.jar")]));
        state.update(Message::Listed { round: first.round, listing: stale }, &store);
        assert_eq!(state.listed, Load::Loading, "the stale answer changed nothing");
        assert!(state.keys().is_empty(), "and brought no row names with it");

        // The answer that *was* asked for lands, and it lands as the tab's own
        // kind of listing -- the two are not interchangeable.
        let fresh = Ok(store::Listing::Files(vec![store::Entry {
            name: "config.toml".to_string(),
            directory: false,
            bytes: 12,
        }]));
        state.update(Message::Listed { round: second.round, listing: fresh }, &store);
        assert!(matches!(state.listed, Load::Ready(store::Listing::Files(_))));
    }

    #[test]
    fn a_toggle_asks_for_the_listing_again() {
        // The one good property of reading the disk on every frame was that a mod
        // toggled shows its new state at once. The listing is asked for again
        // instead, which is the same immediacy with one read rather than one per
        // frame -- and the *read* decides what is disabled, so the page does not
        // patch its own copy of the answer.
        let store = store_at("toggle-asks");
        let mut state = State::new("atm".to_string(), InstanceTab::Content);
        load(&mut state, &store);
        assert!(state.opening().is_none(), "settled before the toggle");

        state.update(
            Message::ToggleContent { file_name: "sodium.jar".to_string(), enabled: false },
            &store,
        );
        let asked = state.opening().expect("the listing is stale, so it is asked for again");
        assert_eq!(asked.tab, InstanceTab::Content);
        assert_eq!(asked.round, 2);
    }

    #[test]
    fn a_listing_with_no_rows_reads_as_empty_rather_than_ready() {
        // `Load`'s `Empty` is not `Ready(vec![])`: the empty arm draws the
        // reference's own "there is nothing here" card, and a page that had to
        // look inside its data to tell the two apart would draw the wrong one.
        let store = store_at("empty-listing");
        let mut state = State::new("atm".to_string(), InstanceTab::Content);
        state.update(
            Message::Listed { round: 0, listing: Ok(store::Listing::Content(Vec::new())) },
            &store,
        );
        assert_eq!(state.listed, Load::Empty);

        // And a refusal is a sentence rather than an empty list: the reader is
        // owed the reason.
        let reason = store::needs_account("Sharing an instance");
        state.update(
            Message::Listed { round: 0, listing: Err(reason.clone()) },
            &store,
        );
        assert_eq!(state.listed, Load::Failed(reason));
    }

    #[test]
    fn the_region_s_report_is_what_the_window_is_computed_from() {
        // The seam this slice is: the page draws a window, and the only thing that
        // moves the window is the report its own scroll region publishes. A page
        // that kept no report would draw the same rows however far the reader had
        // scrolled -- which is the failure that looks like a stuck list.
        let store = store_at("window-report");
        let mut state = State::new("atm".to_string(), InstanceTab::Content);
        assert_eq!(
            state.geometry(),
            Geometry::default(),
            "nothing has reported yet"
        );
        state.update(
            Message::Scrolled(Geometry { offset: 858.0, view_height: 600.0 }),
            &store,
        );
        assert_eq!(state.geometry().offset, 858.0);

        // Ten 86px rows down: the window is that screenful and the margin either
        // side of it, and it is nothing like the five thousand rows the listing
        // holds. Twenty rather than seventeen, because the floor
        // (`scroll::INITIAL_ROWS`) is above a 600px body's seven slots plus the
        // two margins -- which is the floor doing its job rather than a slip.
        let window = scroll::window(5_000, CONTENT_ROW, state.geometry());
        assert_eq!(window.start, 9 - scroll::OVERSCAN);
        let slots = (600.0_f32 / CONTENT_ROW).ceil() as usize;
        assert_eq!(window.len(), (slots + scroll::OVERSCAN * 2).max(scroll::INITIAL_ROWS));
        assert!(window.end < 100, "{}", window.end);
    }

    #[test]
    fn a_tab_change_leaves_the_scroll_region_where_iced_has_it() {
        // iced's own `Scrollable` keeps its offset across a tab change -- it is the
        // same widget in the same place in the tree -- so the page's copy of the
        // geometry is deliberately not reset with the listing. A page that reset it
        // would draw the top of the new tab's list while the region was still
        // scrolled down, which is a blank screen rather than a fresh one.
        let store = store_at("tab-keeps-scroll");
        let mut state = State::new("atm".to_string(), InstanceTab::Files);
        state.update(
            Message::Scrolled(Geometry { offset: 1_200.0, view_height: 600.0 }),
            &store,
        );
        state.update(Message::Tab(InstanceTab::Content), &store);
        assert_eq!(state.geometry().offset, 1_200.0);
    }

    #[test]
    fn five_thousand_rows_draw_in_every_theme_in_both_scroll_states() {
        // The view is what the window is for, so what it has to survive is the two
        // states a frame is drawn in: a tab whose region has reported (the rows are
        // a screenful) and one that has just been opened (the rows are the
        // window-sized guess, `scroll::INITIAL_VIEW`). The cost of each is measured
        // in `crate::scale`; this is the smoke test that both draw, for every tab
        // that has rows and every theme, at the size the plan named.
        let store = store_at("five-thousand");
        let mods: Vec<crate::mods::ModEntry> = (0..5_000)
            .map(|index| mod_entry(&format!("mod-{index:05}.1.0.jar")))
            .collect();
        let files: Vec<store::Entry> = (0..5_000)
            .map(|index| store::Entry {
                name: format!("config-{index:05}.toml"),
                directory: false,
                bytes: 12,
            })
            .collect();
        for theme in Gen::ALL {
            for (tab, listing) in [
                (InstanceTab::Content, store::Listing::Content(mods.clone())),
                (InstanceTab::Files, store::Listing::Files(files.clone())),
            ] {
                let mut state = State::new("atm".to_string(), tab);
                state.update(Message::Listed { round: 0, listing: Ok(listing) }, &store);
                for at in [
                    Geometry::default(),
                    Geometry { offset: 0.0, view_height: 600.0 },
                    Geometry { offset: 200_000.0, view_height: 600.0 },
                ] {
                    state.update(Message::Scrolled(at), &store);
                    drop(view(*theme, &state, &store));
                }
            }
        }
    }

    #[test]
    fn every_loaded_row_has_its_own_clock_name_and_keeps_it_across_a_reload() {
        // Why the names are interned when the listing arrives rather than read on
        // every frame: two rows must not share a key, or hovering one would light
        // both, and the same file must keep its key across a reload, or a hover
        // would land on whatever row moved into that slot.
        let store = store_at("keys");
        let mut state = State::new("atm".to_string(), InstanceTab::Content);
        let rows = vec![mod_entry("sodium.jar"), mod_entry("lithium.jar")];
        state.update(
            Message::Listed { round: 0, listing: Ok(store::Listing::Content(rows.clone())) },
            &store,
        );
        let first: Vec<&'static str> = state.keys().to_vec();
        assert_eq!(first.len(), 2, "one name per row, not one per tab");
        assert_ne!(first[0], first[1], "two rows are two controls");
        assert!(first.iter().any(|key| key.ends_with("sodium.jar")));
        assert!(first.iter().any(|key| key.ends_with("lithium.jar")));

        // The same listing read again: the same names, so a hover in flight does
        // not move to another row.
        state.update(
            Message::Listed { round: 0, listing: Ok(store::Listing::Content(rows)) },
            &store,
        );
        assert_eq!(state.keys().to_vec(), first);

        // A tab that has no rows has no names either, rather than the last tab's.
        state.update(
            Message::Listed { round: 0, listing: Ok(store::Listing::Files(Vec::new())) },
            &store,
        );
        assert!(state.keys().is_empty());
    }

    #[test]
    fn every_tab_draws_in_every_state_a_read_can_be_in() {
        // The view is a function of the page's state now -- the compiler is what
        // says so, since `body` is handed nothing else -- so what it has to
        // survive is a state rather than a disk. All six tabs, all five arms, all
        // four themes, and an instance that does not exist.
        let store = store_at("states");
        for theme in Gen::ALL {
            for tab in State::TABS {
                for listed in [
                    Load::Idle,
                    Load::Loading,
                    Load::Empty,
                    Load::Failed("no such instance".to_string()),
                    Load::Ready(store::Listing::Files(Vec::new())),
                    Load::Ready(store::Listing::Log("a line\nand another".to_string())),
                ] {
                    let mut state = State::new("gone".to_string(), tab.clone());
                    state.listed = listed;
                    state.notice = Some("could not change a.jar: no such file".to_string());
                    drop(view(*theme, &state, &store));
                }
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
    fn the_header_s_gear_reports_the_instance_rather_than_opening_the_modal() {
        // The modal, the read behind it and the write are the shell's -- the
        // page's whole part is the press, exactly as it is for Play.
        let store = store_at("instance-settings-ask");
        let mut state = State::new("atm".to_string(), InstanceTab::Content);
        assert_eq!(
            state.update(Message::Settings, &store),
            Some(Ask::InstanceSettings("atm".to_string()))
        );
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
        store.set_launch(
            "atm",
            store::Launch {
                state: LaunchState::Running,
                line: Some("process started, streaming output…".to_string()),
            },
        );
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
            store.set_launch("atm", store::Launch { state: state_of_run, line: None });
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
        store.set_launch(
            "atm",
            store::Launch {
                state: LaunchState::Idle,
                line: Some("process exited (exit status: 0)".to_string()),
            },
        );
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
