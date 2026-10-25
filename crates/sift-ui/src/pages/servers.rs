//! The servers page: the `Modrinth Hosting` heading, its toolbar, the two server sections,
//! the promo banner under them, and the source's own no-servers screen.
//!
//! The route is `app-frontend/src/pages/Servers.vue`, which is 37 lines and hands everything
//! to `ServersManagePageIndex` (`ui/src/layouts/wrapped/hosting/manage/index.vue`). That
//! layout file is the page this one ports:
//!
//! * The container is `relative mx-auto flex w-full flex-col p-6` (`index.vue:2-4`), so the
//!   page's inset is 24 points (`p-6` = `--gap-xl`, `variables.scss:199`).
//! * The heading row (`index.vue:97-121`) is `mb-4` with the title at `text-2xl font-extrabold
//!   text-contrast` (line 101) and the controls `justify-end gap-2` (line 104): an `Input` whose
//!   wrapper is `w-full md:w-72` (line 114) and the primary `Button type="colored" color="brand"`
//!   carrying a `PlusIcon` (lines 116-119). The search placeholder carries the count
//!   (`filteredData.length`, line 113).
//! * The list is two sections -- `Your servers` and `Shared servers` (`index.vue:168-225`) --
//!   each an `h2` at `text-xl font-semibold text-primary` (lines 169-171, 201-203) over a `ul`
//!   with `flex flex-col gap-3` (line 176), and the two sections are separated by `gap-6`
//!   (line 150). A section whose *unfiltered* list is empty does not render at all; a section
//!   whose filtered list is empty prints `text-secondary` copy of its own (lines 195-197,
//!   222-224); and when both filters are empty the page adds `noServersFound` below them
//!   (lines 227-229).
//! * The card is `ServerListing.vue`: `rounded-2xl border-[1px] border-solid border-surface-4
//!   bg-bg-raised p-4` (line 16), a 64-point `ServerIcon` (`ServerIcon.vue:3`), `ml-4` to the
//!   text column (line 37), the name at `text-xl font-bold text-contrast` (line 39), and the
//!   labels row at `text-sm font-medium text-primary` with `gap-2` (line 103).
//! * The player count is `ServerPlayerCount.vue`'s `current / max` (line 5) printed where the
//!   page has a ping, `N online` where the app's own server list has one (`servers-modal.vue:
//!   403-407`), `Offline` where it has none (`servers-modal.vue:409`, `WorldItem.vue:573-575`),
//!   and `Loading` while the fetch is out (`servers-modal.vue:399-401`). [`status_dot`] is that
//!   ladder as a pure function: a count, `Offline`, or `Unknown`.
//! * The version string is `ServerGameLabel.vue`'s `Minecraft` plus the game version (lines
//!   13-14) and `ServerLoaderLabel.vue`'s loader plus its version (lines 14-17), separated by a
//!   6-point `Separator.vue` bullet.
//! * The promo is `ui/src/components/servers/ServersPromo.vue`: `p-4`, a brand border at
//!   `rounded-2xl`, `Want to play with / your friends?` at `text-lg leading-tight
//!   font-extrabold` (lines 31-34), the body at `text-sm font-medium` (line 35), a `View plans`
//!   brand button with a right arrow (lines 37-41), and a `Don't show again` dismiss control at
//!   `top-2 right-2` (lines 42-53).
//! * The no-servers screen is `server-list-empty/ServerListEmpty.vue`: a 320-point column inside
//!   a 896-point box, the title at `text-3xl leading-9 text-contrast` (line 7), the body at
//!   `text-base text-primary` (line 11), three feature rows whose plates are `size-10
//!   rounded-[0.875rem]` on `bg-surface-1` with a brand glyph (lines 17-33), a `size="lg"`
//!   `New server` button (lines 46-49) and a `Learn more` link (lines 50-57).
//!
//! **The layout is arithmetic over those classes, not a guess.** Every number below is a
//! `const` with the file and line that states it, or is marked `// guess: re-measure` where the
//! reference has no element to measure -- three of those: the two-line MOTD (the brief's line
//! count), the initial on the icon plate, and the body copy's wrapped line count on the
//! no-servers screen. [`card_height`], [`section_height`] and [`content_height`] are the
//! source's own boxes added up, and the tests pin them and the shell's content column at
//! 1440x900.
//!
//! **The dot is the one place the theme is not consulted.** [`status_dot`]'s signature is fixed
//! by the brief and takes no `Theme`, so its three inks are the dark scheme's `--color-brand`,
//! `--color-red` and `--color-secondary` (`theme::DARK`, which is the scheme the reference opens
//! in). A light-scheme window still gets those three dots; passing `&Theme` in is the whole of
//! that fix.
//!
//! **What this page does not do.** It paints what it is handed: no ping is sent, no MOTD is
//! fetched, no server is added, and the one pointer target -- the promo's dismiss control --
//! shows the reference's tooltip and has no banner to hide. `State::servers` defaults to the six
//! hand-written rows in [`SERVERS`], so a capture shows all three dot states, and handing it an
//! empty slice shows the source's no-servers screen instead. The search plate is painted only --
//! the field that takes the keyboard is a sibling's -- and the sort is applied to the order
//! without a control of its own, because the manage page has no sort control: its order is
//! `sortServers` (`index.vue:555-565`), which is [`Sort::Status`].
//!
//! **One file, not wired in.** This slice is a single file, so the module is not declared in
//! `pages/mod.rs` yet; adding `pub mod servers;` there is the whole of the wiring, and
//! `cargo test -p sift-ui servers` is what runs the tests below.

use eframe::egui::{self, Align2, Color32, Rect, Sense, Stroke, Ui, Vec2};

use crate::fonts;
use crate::icons::{self, Glyph};
use crate::theme::{self, Theme, STROKE_WIDTH};

// ---------------------------------------------------------------------------
// The rows the page paints
// ---------------------------------------------------------------------------

/// One server, with the fields `ServerListing.vue`'s props carry (lines 412-435).
///
/// The reference's `Server` is `Archon.Servers.v0.Server` off the network; this is the same row
/// with the parts this page draws, and nothing that would need a request to fill in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Server {
    /// The server's name: `text-xl font-bold text-contrast` (`ServerListing.vue:39`).
    pub name: &'static str,
    /// The message of the day, drawn under the name in `text-secondary` (see [`MOTD_LINE`]).
    ///
    /// The reference's card shows a project line here (`Using {projectTitle}`,
    /// `ServerListing.vue:67-79`); a server's MOTD is what a player reads in its place, and it
    /// is the two-line box the brief asks for.
    pub motd: &'static str,
    /// The game version, printed the way `ServerGameLabel.vue:13-14` prints it:
    /// `Minecraft` then this.
    pub mc_version: &'static str,
    /// The loader and its version, printed the way `ServerLoaderLabel.vue:14-17` prints them,
    /// e.g. `Fabric 0.16.10`.
    pub loader: &'static str,
    /// The ping's player payload as `(current, max)` -- `ServerListing.vue:92-94` passes
    /// `playerCount.current` and `.max` in exactly this order.
    ///
    /// `Some((_, max))` with a capacity is an answered ping; `Some((0, 0))` is a ping that came
    /// back with nothing to report, which is the reference's `Offline` row; `None` is a server
    /// nothing has asked about yet, which is the state the reference draws as still loading and
    /// this page draws as `Unknown`.
    pub players: Option<(u32, u32)>,
    /// Whether the server belongs to someone else: the source's `owner_id` split
    /// (`index.vue:577-603`), which is what puts a row under `Shared servers`.
    pub shared: bool,
}

impl Server {
    /// Whether the ping answered with a capacity: the one state that has a count to print.
    fn is_online(&self) -> bool {
        matches!(self.players, Some((_, max)) if max > 0)
    }

    /// The source's `getStatusPriority` (`index.vue:549-553`), mapped onto what a reader who has
    /// not asked the network can know: a reachable server first, an unasked one next, and one
    /// that answered with nothing last -- the same place the source puts a suspended server.
    fn status_rank(&self) -> u8 {
        if self.is_online() {
            0
        } else if self.players.is_none() {
            1
        } else {
            2
        }
    }

    /// The current player count, `0` where there is none: `Sort::Players`'s key.
    fn players_now(&self) -> u32 {
        self.players.map_or(0, |(current, _)| current)
    }
}

/// The sample rows: three online (two of them owned), two offline, one unknown, four owned and
/// two shared. The three dot states are all present on purpose, so one capture shows all three.
///
/// The names, loaders and versions are plausible Modrinth Hosting rows, not a fixture from a
/// live account: this page never asks the network who the reader is.
pub const SERVERS: [Server; 6] = [
    Server {
        name: "Modrinth SMP",
        motd: "The official Modrinth community server: survival, a shopping district at spawn, and an event every weekend.",
        mc_version: "1.21.4",
        loader: "Fabric 0.16.10",
        players: Some((12, 60)),
        shared: false,
    },
    Server {
        name: "All the Mods 10",
        motd: "Kitchen-sink progression with ATM ore doubling, 400+ mods and a public market at spawn.",
        mc_version: "1.21.1",
        loader: "NeoForge 21.1.72",
        players: Some((4, 40)),
        shared: false,
    },
    Server {
        name: "Create: Astral",
        motd: "A hand-built Create skyblock with quest lines, sky villages and a shared factory district that never sleeps.",
        mc_version: "1.20.1",
        loader: "Forge 47.3.0",
        players: None,
        shared: false,
    },
    Server {
        name: "Vanilla+ Survival",
        motd: "Vanilla with a light quality-of-life pack: no claims, no map resets, and a spawn town that grew out of one hut.",
        mc_version: "1.21.4",
        loader: "Fabric 0.16.10",
        players: Some((0, 0)),
        shared: false,
    },
    Server {
        name: "Better MC [FABRIC]",
        motd: "A curated kitchen-sink pack for builders: 250 mods, quests, and a creative plot world beside survival.",
        mc_version: "1.20.1",
        loader: "Fabric 0.15.11",
        players: Some((7, 30)),
        shared: true,
    },
    Server {
        name: "Cobblemon Islands",
        motd: "Catch, train and trade across a hand-made archipelago; the world border opens a new island each season.",
        mc_version: "1.21.1",
        loader: "Fabric 0.16.10",
        players: Some((0, 0)),
        shared: true,
    },
];

/// The order the page draws its rows in.
///
/// The manage page has no sort control: it sorts with `sortServers` (`index.vue:555-565`) --
/// status priority, then the intro flow, then the name -- and that order is [`Sort::Status`]
/// here. The other two are the sort a toolbar sibling would set; this page applies whatever the
/// state holds and paints no control of its own, exactly as it paints the search plate without
/// owning the field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    /// The source's own order (`index.vue:555-565`), its default.
    #[default]
    Status,
    /// Alphabetical by name, which is the last key of the source's comparator.
    Name,
    /// Most players first, then name.
    Players,
}

/// The page's own state: what the sibling field typed, which order to draw, and the rows to
/// draw.
///
/// `servers` is a slice rather than a `Vec` because the page paints what it is handed: the six
/// sample rows by default, an empty slice for the source's no-servers screen, and one row per
/// server once a reader exists to hand them over.
#[derive(Debug)]
pub struct State {
    /// The search box's text, untrimmed. The field that types it is a sibling's; this page
    /// filters by it and counts the placeholder with it, the way `searchInput` does
    /// (`index.vue:105-115`).
    pub search: String,
    /// The order the rows are drawn in.
    pub sort: Sort,
    /// The rows, defaulting to [`SERVERS`].
    pub servers: &'static [Server],
}

impl Default for State {
    fn default() -> State {
        State {
            search: String::new(),
            sort: Sort::default(),
            servers: &SERVERS,
        }
    }
}

// ---------------------------------------------------------------------------
// Strings
//
// Every string below is the reference's, copied verbatim. The app ships two catalogues and
// merges them at runtime: the app's own (`app-frontend/src/locales/en-US/index.json`) and the
// UI package's (`ui/src/locales/en-US/index.json`). Servers, hosting and the presence dot all
// live in the second one, so most keys below cite it; the key is named with each string so the
// copy can be traced to the catalogue rather than to a screenshot.
// ---------------------------------------------------------------------------

/// `servers.manage.servers-title` — "Modrinth Hosting" (`ui/src/locales/en-US/index.json:5504`).
/// The app's own catalogue carries the same words as `app.nav.modrinth-hosting`
/// (`app-frontend/src/locales/en-US/index.json:1367`).
const HEADING: &str = "Modrinth Hosting";

/// `servers.manage.new-server-button` — "New server" (`ui/src/locales/en-US/index.json:5462`),
/// the page's primary button (`index.vue:116-119`).
///
/// The brief calls this "Add server"; the manage page's own button says `New server`, and
/// `Add server` is a different control in a different dialog
/// (`instance.add-server.add-server`, `app-frontend/src/locales/en-US/index.json:3680`). The
/// source's word wins.
const NEW_SERVER: &str = "New server";

/// `servers.manage.your-servers-title` — "Your servers" (`ui/src/locales/en-US/index.json:5519`).
const YOUR_SERVERS: &str = "Your servers";

/// `servers.manage.shared-servers-title` — "Shared servers"
/// (`ui/src/locales/en-US/index.json:5516`).
const SHARED_SERVERS: &str = "Shared servers";

/// `servers.manage.no-owned-servers-found` — "No servers you own match your search."
/// (`ui/src/locales/en-US/index.json:5465`), printed in place of an emptied owned section
/// (`index.vue:195-197`).
const NO_OWNED_FOUND: &str = "No servers you own match your search.";

/// `servers.manage.no-shared-servers-found` — "No shared servers match your search."
/// (`ui/src/locales/en-US/index.json:5471`).
const NO_SHARED_FOUND: &str = "No shared servers match your search.";

/// `servers.manage.no-servers-found` — "No servers found."
/// (`ui/src/locales/en-US/index.json:5468`), the page-level line under both sections
/// (`index.vue:227-229`).
const NO_SERVERS_FOUND: &str = "No servers found.";

/// `servers.manage.search-placeholder` — "Search {count} {count, plural, one {server}
/// other {servers}}..." (`ui/src/locales/en-US/index.json:5501`). The ICU plural is assembled in
/// [`search_placeholder`]; these are the three pieces of it.
const SEARCH_PREFIX: &str = "Search";
/// The trailing ellipsis of the same message. Three full stops, as the catalogue writes them.
const SEARCH_SUFFIX: &str = "...";

/// The ICU plural's two branches in `servers.manage.search-placeholder`
/// (`ui/src/locales/en-US/index.json:5501`); `app.settings.synced-options.multiplayer-servers.count`
/// states the same two words as `{count, plural, one {# server} other {# servers}}`
/// (`app-frontend/src/locales/en-US/index.json:3054`).
const SERVER_ONE: &str = "server";
const SERVER_MANY: &str = "servers";

/// `project.server.status.online` — "Online" (`ui/src/locales/en-US/index.json:4022`).
const ONLINE: &str = "Online";

/// `project.server.status.offline` — "Offline" (`ui/src/locales/en-US/index.json:4016`). The
/// catalogue also carries the tooltip the reference hangs on the offline tag, "Server is
/// offline" (`project.server.status.offline.tooltip`, `:4019`); this page's labels row has no
/// hover target, so the word is all it prints.
const OFFLINE: &str = "Offline";

/// `label.unknown` — "Unknown" (`ui/src/locales/en-US/index.json:2754`). The reference has no
/// word of its own for "not asked yet": it prints `Loading` there (`commonMessages.loadingLabel`,
/// `servers-modal.vue:400`) and shows a pulse skeleton on the card. This page has no fetch to
/// wait on, so the honest word is the catalogue's `Unknown`.
const UNKNOWN: &str = "Unknown";

/// `servers.list-empty.modrinth-hosting-label` — "Modrinth Hosting"
/// (`ui/src/locales/en-US/index.json:5324`), the no-servers screen's title
/// (`ServerListEmpty.vue:7-9`).
const EMPTY_HEADING: &str = "Modrinth Hosting";

/// `servers.list-empty.no-servers-description` — "Install mods, invite friends, and play
/// together all from the Modrinth App." (`ui/src/locales/en-US/index.json:5330`).
const EMPTY_BODY: &str = "Install mods, invite friends, and play together all from the Modrinth App.";

/// `servers.list-empty.one-click-mod-installs-title` — "One-click mod installs"
/// (`ui/src/locales/en-US/index.json:5336`).
const EMPTY_FEATURE_ONE_TITLE: &str = "One-click mod installs";

/// `servers.list-empty.one-click-mod-installs-description` — "Pick your favorite mods and we
/// handle the rest." (`ui/src/locales/en-US/index.json:5333`).
const EMPTY_FEATURE_ONE_BODY: &str = "Pick your favorite mods and we handle the rest.";

/// `servers.list-empty.simple-setup-title` — "Simple setup"
/// (`ui/src/locales/en-US/index.json:5351`).
const EMPTY_FEATURE_TWO_TITLE: &str = "Simple setup";

/// `servers.list-empty.simple-setup-description` — "Set up your server just like a single
/// player world." (`ui/src/locales/en-US/index.json:5348`).
const EMPTY_FEATURE_TWO_BODY: &str = "Set up your server just like a single player world.";

/// `servers.list-empty.play-with-friends-title` — "Play with friends"
/// (`ui/src/locales/en-US/index.json:5342`).
const EMPTY_FEATURE_THREE_TITLE: &str = "Play with friends";

/// `servers.list-empty.play-with-friends-description` — "Invite friends to play your server with
/// one-click play from the Modrinth App." (`ui/src/locales/en-US/index.json:5339`).
const EMPTY_FEATURE_THREE_BODY: &str =
    "Invite friends to play your server with one-click play from the Modrinth App.";

/// `servers.list-empty.new-server-button` — "New server"
/// (`ui/src/locales/en-US/index.json:5327`).
const EMPTY_ACTION: &str = "New server";

/// `servers.list-empty.learn-more-link` — "Learn more"
/// (`ui/src/locales/en-US/index.json:5321`).
const EMPTY_LINK: &str = "Learn more";

/// `ServersPromo.vue:31-34`: "Want to play with" then a `<br/>` then "your friends?" in
/// `text-brand`.
///
/// **The promo's copy has no catalogue key.** It is written into the component
/// (`ServersPromo.vue:31-40`) rather than through `defineMessages`, which is why the four
/// strings in this group cite the component's lines instead of an `index.json` entry -- the
/// brief expected keys here, and there are none to cite. The one key the promo's controls do
/// share with a catalogue entry is the dismiss label below.
const PROMO_LINE_ONE: &str = "Want to play with";

/// The second line of the same heading, `text-brand` (`ServersPromo.vue:33`).
const PROMO_LINE_TWO: &str = "your friends?";

/// `ServersPromo.vue:35` — "Create a server with Modrinth in just a few clicks."
const PROMO_BODY: &str = "Create a server with Modrinth in just a few clicks.";

/// `ServersPromo.vue:38-40` — the brand button's label, followed by a right arrow.
const PROMO_ACTION: &str = "View plans";

/// `ServersPromo.vue:45` and `:48` — the dismiss control's tooltip and accessible label. The
/// same words are the catalogue's `servers.manage.settings-hint.dismiss`
/// (`ui/src/locales/en-US/index.json:5510`), which is how the app dismisses its own banner.
const PROMO_DISMISS_LABEL: &str = "Don't show again";

// ---------------------------------------------------------------------------
// Page and toolbar geometry
// ---------------------------------------------------------------------------

/// The route container's inset: `p-6` (`index.vue:4`), i.e. 1.5rem = `--gap-xl`
/// (`variables.scss:199`).
const PAGE_PADDING: f32 = 24.0;

/// The page title's size and line box: `text-2xl` (`index.vue:101`) is 1.5rem with a 2rem line
/// height, at the `font-extrabold` that line asks for.
const HEADING_SIZE: f32 = 24.0;
const HEADING_LINE: f32 = 32.0;

/// The space under the heading row: `mb-4` (`index.vue:99`), and the same box again between the
/// heading row's own text and the toolbar below it.
const HEADING_GAP: f32 = 16.0;

/// A toolbar control's height. The search field is a default `Input`, whose `standard` size is
/// `h-9` (`InputFrame.vue:47`); the button beside it is a default `Button`, whose `md` size is
/// `h-9` (`ButtonFrame.vue:31`).
const CONTROL_HEIGHT: f32 = 36.0;

/// The gap between the two toolbar controls: `gap-2` on the heading row's control cluster
/// (`index.vue:104`), i.e. `--gap-sm` (`variables.scss:196`).
const CONTROL_GAP: f32 = 8.0;

/// Both controls' radius: `rounded-xl` (`InputFrame.vue:47`, `ButtonFrame.vue:31`) is 1rem = 16
/// points, which is `--radius-md` (`variables.scss:203`).
const CONTROL_RADIUS: u8 = 12;

/// The search field's width above `md`: `md:w-72` (`index.vue:114`) is 18rem = 288 points.
/// Below `md` the source takes `w-full`, which in a content column this narrow is the same box
/// either way.
const SEARCH_WIDTH: f32 = 288.0;

/// The search field's horizontal padding: `px-3` (`InputFrame.vue:47`).
const SEARCH_PADDING: f32 = 12.0;

/// The search field's leading glyph: `[&>svg]:size-5` inside a `size-5` box (`Input.vue:12`).
const SEARCH_GLYPH: f32 = 20.0;

/// The gap between that glyph and the text: `gap-2` (`InputFrame.vue:47`).
const SEARCH_TEXT_GAP: f32 = 8.0;

/// The search text's size: `text-base` (`Input.vue:55`), in the `font-medium` the field sets
/// (`Input.vue:51`).
const SEARCH_TEXT: f32 = 16.0;

/// A button's horizontal padding: `px-2.5` on `md` (`ButtonFrame.vue:31`).
const BUTTON_PADDING: f32 = 10.0;

/// A button's inner gap between label and glyph: `gap-1.5` (`ButtonFrame.vue:31`).
const BUTTON_GAP: f32 = 6.0;

/// A button's glyph box: `[&>svg]:size-5` (`ButtonFrame.vue:31`).
const BUTTON_GLYPH: f32 = 20.0;

/// A button label's size: `text-base font-semibold` (`ButtonFrame.vue:31`).
const BUTTON_TEXT: f32 = 16.0;

/// The gap between the page title and the count beside it. The source has no count there -- its
/// count is inside the search placeholder (`index.vue:113`) -- so this is the same `gap-2` the
/// toolbar uses, at 8 points.
const COUNT_GAP: f32 = 8.0;

/// Small print: `text-sm` (`index.vue:161`, `ServerListing.vue:103`) = 14 points.
const LABEL_SIZE: f32 = 14.0;

// ---------------------------------------------------------------------------
// Card geometry
// ---------------------------------------------------------------------------

/// The card's radius: `rounded-2xl` (`ServerListing.vue:16`) = `--radius-lg`
/// (`variables.scss:204`).
const CARD_RADIUS: u8 = 16;

/// The card's padding: `p-4` (`ServerListing.vue:16`) = `--gap-lg` (`variables.scss:198`).
const CARD_PADDING: f32 = 16.0;

/// The icon plate's side: `size-16` (`ServerIcon.vue:3`), the same box the provisioned card
/// draws for its overlay (`ServerListing.vue:26`).
const ICON_SIDE: f32 = 64.0;

/// The icon plate's radius: `rounded-2xl` (`ServerIcon.vue:3`).
const ICON_RADIUS: u8 = 16;

/// The invoice the plate shows when the server has no uploaded icon: the name's initial, at a
/// size that reads as a mark rather than as text. The reference uploads (or synthesises) a
/// 64-point `server-icon.png` (`ServerListing.vue:552-610`); this reader has no texture, so the
/// plate keeps its fill and carries the initial, the way the library card does
/// (`pages/library.rs:574-586`).
// guess: re-measure
const ICON_INITIAL: f32 = 18.0;

/// The space between the plate and the text column: `ml-4` (`ServerListing.vue:37`).
const ICON_GAP: f32 = 16.0;

/// The gap between the name, the MOTD and the labels row: `gap-1.5` (`ServerListing.vue:37`).
const CARD_TEXT_GAP: f32 = 6.0;

/// The name's size and line box: `text-xl` (`ServerListing.vue:39`) is 1.25rem with a 1.75rem
/// line height, so 20 points on 28, at the `font-bold` that line asks for.
const NAME_SIZE: f32 = 20.0;
const NAME_LINE: f32 = 28.0;

/// The labels row's box: `text-sm` (`ServerPlayerCount.vue:2`, `ServerListing.vue:103`) = 14
/// points on a 20-point line.
const META_LINE: f32 = 20.0;

/// The MOTD's line box: `text-sm` again, on the `leading-5` the reference's server lines use
/// (`WorldItem.vue:578` is the `text-sm text-secondary` row this copy takes its colour from).
const MOTD_LINE: f32 = 20.0;

/// How many lines of MOTD the card shows. The brief asks for two.
///
/// The reference has no MOTD element to measure, so this is the one box on the page with no
/// class to cite: two lines of `text-sm` under the name, clipped with an ellipsis.
// guess: re-measure
const MOTD_LINES: usize = 2;

/// The gap between two cards: `gap-3` on the list (`index.vue:176`, `:208`).
const LIST_GAP: f32 = 12.0;

/// A section heading's size and line box: `text-xl` (`index.vue:169`) is 20 points on 28, at the
/// `font-semibold` that line asks for.
const SECTION_HEADING_SIZE: f32 = 20.0;
const SECTION_HEADING_LINE: f32 = 28.0;

/// The gap between a section heading and its list: `gap-3` (`index.vue:168`).
const SECTION_GAP: f32 = 12.0;

/// The gap between the list's blocks -- the two sections, the page-level "no servers" line and
/// the promo: `gap-6` (`index.vue:150`).
const BLOCK_GAP: f32 = 24.0;

// ---------------------------------------------------------------------------
// The presence dot and the labels row
// ---------------------------------------------------------------------------

/// The dot's radius: the reference's presence dot is `size-2` (`AppActionBar.vue:117`,
/// `ServerListEmptyPreview.vue:50`) = 8 points, so 4 from centre to edge.
const DOT_RADIUS: f32 = 4.0;

/// The gap between the dot and the count: `gap-1` on the player-count row
/// (`ServerPlayerCount.vue:2`).
const DOT_GAP: f32 = 4.0;

/// The bullet between the labels: `h-1.5 w-1.5` in `--color-button-border` (`Separator.vue:2`)
/// = 6 points.
const SEPARATOR: f32 = 6.0;

/// The gap between the labels in the row: `gap-2` (`ServerListing.vue:103`).
const LABEL_GAP: f32 = 8.0;

// ---------------------------------------------------------------------------
// Promo geometry
// ---------------------------------------------------------------------------

/// The promo's padding: `p-4` (`ServersPromo.vue:22`).
const PROMO_PADDING: f32 = 16.0;

/// The promo's radius: `rounded-2xl` (`ServersPromo.vue:22`).
const PROMO_RADIUS: u8 = 16;

/// The promo heading's line box: `text-lg leading-tight` (`ServersPromo.vue:31`) is 18 points on
/// 1.25 lines = 22.5.
const PROMO_TITLE_LINE: f32 = 22.5;

/// The promo heading's size: `text-lg` (`ServersPromo.vue:31`), at the `font-extrabold` the
/// same line asks for.
const PROMO_TITLE_SIZE: f32 = 18.0;

/// The promo body's line box: `text-sm` (`ServersPromo.vue:35`) = 14 on 20.
const PROMO_BODY_LINE: f32 = 20.0;

/// The gap between the heading and the body: `gap-2` (`ServersPromo.vue:30`).
const PROMO_TEXT_GAP: f32 = 8.0;

/// The dismiss control's box: `!size-6` (`ServersPromo.vue:46`) = 24 points, inset `top-2
/// right-2` = 8 from the corner (`ServersPromo.vue:42`).
const PROMO_DISMISS: f32 = 24.0;
const PROMO_DISMISS_INSET: f32 = 8.0;

/// The dismiss glyph inside that box: an `XIcon` at the `xs` size's `[&>svg]:size-4`
/// (`ButtonFrame.vue:29`).
const PROMO_DISMISS_GLYPH: f32 = 16.0;

/// The promo button's height: a default `Button` at `md` (`ButtonFrame.vue:31`).
const PROMO_BUTTON_HEIGHT: f32 = 36.0;

/// The right arrow in the promo button: `@modrinth/assets`'s `RightArrowIcon`, at the button's
/// own `size-5` (`ButtonFrame.vue:31`).
const PROMO_BUTTON_GLYPH: f32 = 20.0;

// ---------------------------------------------------------------------------
// No-servers screen geometry
// ---------------------------------------------------------------------------

/// The text column's width: `max-w-[20rem]` (`ServerListEmpty.vue:4`) = 320 points.
const EMPTY_COLUMN: f32 = 320.0;

/// The box that column sits in: `max-w-[56rem]` (`ServerListEmpty.vue:2`) = 896 points.
const EMPTY_MAX: f32 = 896.0;

/// The screen's vertical padding: `py-6` (`ServerListEmpty.vue:2`).
const EMPTY_PAD_Y: f32 = 24.0;

/// The title's line box and size: `text-3xl ... leading-9` (`ServerListEmpty.vue:7`) is 30
/// points on 36, at `font-semibold`.
const EMPTY_HEADING_LINE: f32 = 36.0;
const EMPTY_HEADING_SIZE: f32 = 30.0;

/// The body's line box: `text-base` (`ServerListEmpty.vue:11`) = 16 on 24.
const EMPTY_BODY_LINE: f32 = 24.0;

/// How many lines the body copy takes in the 320-point column: the sentence is 76 characters,
/// which is two lines at 16 points. The source sizes the paragraph by its text and not by a
/// fixed box, so this is a measurement of the copy rather than of a class.
// guess: re-measure
const EMPTY_BODY_LINES: f32 = 2.0;

/// The body's size: `text-base` (`ServerListEmpty.vue:11`).
const EMPTY_BODY_SIZE: f32 = 16.0;

/// The gap between the title and the body: `gap-2` (`ServerListEmpty.vue:5`).
const EMPTY_TEXT_GAP: f32 = 8.0;

/// The gap between the blocks -- title/body, features, actions: `gap-8` (`ServerListEmpty.vue:4`
/// and `:16`).
const EMPTY_BLOCK_GAP: f32 = 32.0;

/// The gap between two feature rows: `gap-6` (`ServerListEmpty.vue:16`).
const EMPTY_FEATURE_GAP: f32 = 24.0;

/// A feature's icon plate: `size-10` on `rounded-[0.875rem]` (`ServerListEmpty.vue:19`) = 40
/// points at radius 14.
const EMPTY_FEATURE_PLATE: f32 = 40.0;
const EMPTY_PLATE_RADIUS: u8 = 14;

/// The glyph inside that plate: `size-5` (`ServerListEmpty.vue:30`).
const EMPTY_FEATURE_GLYPH: f32 = 20.0;

/// The gap between the plate and the feature's text: `gap-4` (`ServerListEmpty.vue:17`).
const EMPTY_FEATURE_GAP_X: f32 = 16.0;

/// A feature title's line box and size: `text-lg ... leading-6`
/// (`ServerListEmpty.vue:35`) = 18 points on 24, at `font-semibold`.
const EMPTY_FEATURE_TITLE: f32 = 24.0;
const EMPTY_FEATURE_TITLE_SIZE: f32 = 18.0;

/// A feature body's line box: `text-sm leading-5` (`ServerListEmpty.vue:38`) = 14 on 20. The
/// box reserves two lines: the longest description is 77 characters, which wraps to two at the
/// column's width, and a row whose text runs to one line keeps the same height so the three
/// plates stay on a grid.
const EMPTY_FEATURE_BODY: f32 = 20.0;

/// The gap between a feature's title and its body: `gap-0.5` (`ServerListEmpty.vue:34`).
const EMPTY_FEATURE_INNER: f32 = 2.0;

/// The actions' button: `size="lg"` (`ServerListEmpty.vue:46`) is `h-10 rounded-[14px] px-4
/// gap-2` (`ButtonFrame.vue:32`) = 40 points tall at radius 14.
const EMPTY_ACTION_HEIGHT: f32 = 40.0;
const EMPTY_ACTION_RADIUS: u8 = 14;
const EMPTY_ACTION_PADDING: f32 = 16.0;
const EMPTY_ACTION_GAP: f32 = 8.0;

/// The gap between those actions: `gap-4` (`ServerListEmpty.vue:45`).
const EMPTY_ACTIONS_GAP: f32 = 16.0;

/// The gap between the `Learn more` link's label and its arrow: `gap-1`
/// (`ServerListEmpty.vue:53`).
const EMPTY_LINK_GAP: f32 = 4.0;

// ---------------------------------------------------------------------------
// The status rule
// ---------------------------------------------------------------------------

/// The dot and the state's own word, from the ping's player payload.
///
/// The reference has three rows and this is all three:
///
/// * **Online** — the count, and the green the app's server list paints its signal icon
///   (`servers-modal.vue:393-398`: `text-green`) and its presence dot
///   (`ServerListEmptyPreview.vue:51`: `bg-brand`).
/// * **Offline** — `project.server.status.offline`, and the red the project page's offline tag
///   uses (`ServerPing.vue:61-68`: `border-red bg-highlight-red text-red`).
/// * **Unknown** — `label.unknown`, in the secondary text colour, which is how the reference
///   paints everything it has not been told about.
///
/// The three inks are `theme::DARK`'s tokens because the signature takes no `Theme` (see the
/// module docs); `DARK` is the scheme the reference opens in.
///
/// `Some((_, max))` with `max > 0` is an answered ping with a capacity; `Some` without one is a
/// ping that came back empty, which the reference treats as `Offline`; `None` is a server
/// nothing has asked about yet.
fn status_dot(online: Option<(u32, u32)>) -> (Color32, &'static str) {
    match online {
        Some((_, max)) if max > 0 => (theme::DARK.BRAND, ONLINE),
        Some(_) => (theme::DARK.RED, OFFLINE),
        None => (theme::DARK.SECONDARY, UNKNOWN),
    }
}

/// The text beside the dot: the count where the ping gave one (`ServerPlayerCount.vue:5` prints
/// `current / max`), and the state's word otherwise (`servers-modal.vue:409`,
/// `WorldItem.vue:573-575`).
///
/// The separator is the slash the reference writes between the two numbers; the `Players` noun
/// that follows it there (`ServerPlayerCount.vue:5`) is left off because the brief asks for
/// `online/max` and the dot already says which state it is.
fn status_label(online: Option<(u32, u32)>) -> String {
    match online {
        Some((current, max)) if max > 0 => format!("{current}/{max}"),
        // The other two states are the words `status_dot` names, so the vocabulary lives in
        // one place.
        other => status_dot(other).1.to_owned(),
    }
}

// ---------------------------------------------------------------------------
// The count, the placeholder, and the filter
// ---------------------------------------------------------------------------

/// `{count, plural, one {# server} other {# servers}}`, the ICU plural `servers.manage.
/// search-placeholder` and `app.settings.synced-options.multiplayer-servers.count` both carry.
fn count_phrase(count: usize) -> String {
    let noun = if count == 1 { SERVER_ONE } else { SERVER_MANY };
    format!("{count} {noun}")
}

/// `servers.manage.search-placeholder` with the count filled in: the source passes
/// `filteredData.length` (`index.vue:113`), so the number is the rows the query leaves, not the
/// rows the page holds.
fn search_placeholder(count: usize) -> String {
    // No space before the ellipsis: the catalogue's message is
    // `Search {count} {count, plural, ...}...`, and a space there reads as a typo on screen.
    format!("{SEARCH_PREFIX} {}{SEARCH_SUFFIX}", count_phrase(count))
}

/// Whether a row matches the query.
///
/// The reference searches name, loader, game version, game, state and the owner's username
/// through Fuse.js at a 0.4 threshold (`index.vue:605-617`). This is a case-insensitive substring
/// test over the fields this reader has -- name, MOTD, version and loader -- which matches a
/// typed word exactly and gives up the fuzzy near-misses Fuse would return.
fn matches_query(server: &Server, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    [server.name, server.motd, server.mc_version, server.loader]
        .iter()
        .any(|field| field.to_lowercase().contains(needle))
}

/// The rows a section draws: its own half of the list, filtered and ordered.
fn listed<'a>(
    servers: &'a [Server],
    shared: bool,
    query: &str,
    sort: Sort,
) -> Vec<&'a Server> {
    let needle = query.trim().to_lowercase();
    let mut rows: Vec<&Server> = servers
        .iter()
        .filter(|server| server.shared == shared)
        .filter(|server| matches_query(server, &needle))
        .collect();
    match sort {
        Sort::Status => rows.sort_by(|a, b| {
            a.status_rank()
                .cmp(&b.status_rank())
                .then_with(|| a.name.cmp(b.name))
        }),
        Sort::Name => rows.sort_by(|a, b| a.name.cmp(b.name)),
        Sort::Players => rows.sort_by(|a, b| {
            b.players_now()
                .cmp(&a.players_now())
                .then_with(|| a.name.cmp(b.name))
        }),
    }
    rows
}

// ---------------------------------------------------------------------------
// The heights, as arithmetic over the boxes above
// ---------------------------------------------------------------------------

/// The text column inside a card: the name, the two-line MOTD and the labels row, with the
/// card's own 6-point gap between them (`ServerListing.vue:37`).
fn text_column_height() -> f32 {
    NAME_LINE + CARD_TEXT_GAP + MOTD_LINE * MOTD_LINES as f32 + CARD_TEXT_GAP + META_LINE
}

/// One card's height: the source's `p-4` around a row of `items-center` (`ServerListing.vue:16`),
/// whose taller side is whichever of the 64-point plate and [`text_column_height`] wins.
fn card_height() -> f32 {
    let by_text = 2.0 * CARD_PADDING + text_column_height();
    let by_icon = 2.0 * CARD_PADDING + ICON_SIDE;
    if by_text > by_icon {
        by_text
    } else {
        by_icon
    }
}

/// One of the two sections, with the rows it will draw.
struct Section<'a> {
    /// `Your servers` or `Shared servers` (`index.vue:169-171`, `:201-203`).
    heading: &'static str,
    /// The line printed in place of the list when the filter empties it (`index.vue:195-197`,
    /// `:222-224`).
    empty: &'static str,
    /// How many rows this section has before the filter: the source renders a section only when
    /// its own list is non-empty (`index.vue:168`, `:200`).
    total: usize,
    /// The rows to draw, already filtered and ordered.
    cards: Vec<&'a Server>,
}

/// A section's height: its heading, the `gap-3` under it, then the list or the one-line
/// no-match copy.
fn section_height(section: &Section<'_>) -> f32 {
    let count = section.cards.len();
    let list = if count == 0 {
        META_LINE
    } else {
        count as f32 * card_height() + count.saturating_sub(1) as f32 * LIST_GAP
    };
    SECTION_HEADING_LINE + SECTION_GAP + list
}

/// The height the scroll area reserves: every section that renders, the page-level no-servers
/// line when it applies, and the promo, separated by the source's `gap-6` (`index.vue:150`).
fn content_height(sections: &[Section<'_>], show_no_match: bool) -> f32 {
    let mut height = 0.0;
    let mut blocks = 0;
    for section in sections.iter().filter(|section| section.total > 0) {
        if blocks > 0 {
            height += BLOCK_GAP;
        }
        height += section_height(section);
        blocks += 1;
    }
    if show_no_match {
        if blocks > 0 {
            height += BLOCK_GAP;
        }
        height += META_LINE;
        blocks += 1;
    }
    if blocks > 0 {
        height += BLOCK_GAP;
    }
    height + promo_height()
}

/// The promo's height: its padding, and the taller of its text column and its button
/// (`ServersPromo.vue:22-41`).
fn promo_height() -> f32 {
    let text = 2.0 * PROMO_TITLE_LINE + PROMO_TEXT_GAP + PROMO_BODY_LINE;
    let taller = if text > PROMO_BUTTON_HEIGHT {
        text
    } else {
        PROMO_BUTTON_HEIGHT
    };
    2.0 * PROMO_PADDING + taller
}

/// The no-servers screen's height: the title, the body, the three feature rows and the actions.
fn empty_height() -> f32 {
    let feature = EMPTY_FEATURE_TITLE + EMPTY_FEATURE_INNER + 2.0 * EMPTY_FEATURE_BODY;
    EMPTY_HEADING_LINE
        + EMPTY_TEXT_GAP
        + EMPTY_BODY_LINES * EMPTY_BODY_LINE
        + EMPTY_BLOCK_GAP
        + 3.0 * feature
        + 2.0 * EMPTY_FEATURE_GAP
        + EMPTY_BLOCK_GAP
        + EMPTY_ACTION_HEIGHT
}

// ---------------------------------------------------------------------------
// The page
// ---------------------------------------------------------------------------

/// Draw the servers page into `rect`. Returns whether another frame is owed: nothing here
/// animates, so it is always `false`.
pub fn draw(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    theme: &crate::theme::Theme,
    images: &mut crate::images::Images,
    state: &mut State,
) -> bool {
    // The page owns its rect (`pages/mod.rs`'s seam); paint the page colour first so the cards
    // sit on `--color-bg` even if the caller did not clear the column.
    ui.painter().rect_filled(rect, 0, theme.BG);

    // The reference fetches `/server-icon.png` through the filesystem API for each available
    // server (`ServerListing.vue:552-610`). `Server` carries no icon URL, so there is nothing to
    // ask `Images::get` for and the plate draws the initial instead. When the reader grows an
    // icon field this is the one call that changes.
    let _ = images;

    if rect.width() <= 2.0 * PAGE_PADDING || rect.height() <= 2.0 * PAGE_PADDING {
        return false;
    }

    // The source's first branch: an account with no servers at all gets `ServerListEmpty`
    // *instead of* the heading, the toolbar and the list (`index.vue:139-148`, `:530-536`).
    if state.servers.is_empty() {
        let painter = ui.painter();
        empty_state(painter, rect, theme);
        return false;
    }

    let query = state.search.as_str();
    let sections = [
        Section {
            heading: YOUR_SERVERS,
            empty: NO_OWNED_FOUND,
            total: state.servers.iter().filter(|server| !server.shared).count(),
            cards: listed(state.servers, false, query, state.sort),
        },
        Section {
            heading: SHARED_SERVERS,
            empty: NO_SHARED_FOUND,
            total: state.servers.iter().filter(|server| server.shared).count(),
            cards: listed(state.servers, true, query, state.sort),
        },
    ];
    let shown = sections[0].cards.len() + sections[1].cards.len();
    let show_no_match = shown == 0;

    let list_top = {
        let painter = ui.painter();
        heading_and_toolbar(painter, rect, theme, state.servers.len(), shown)
    };

    let view = Rect::from_min_max(
        egui::pos2(rect.left() + PAGE_PADDING, list_top),
        egui::pos2(rect.right() - PAGE_PADDING, rect.bottom() - PAGE_PADDING),
    );
    if view.width() < 1.0 || view.height() < 1.0 {
        return false;
    }

    let height = content_height(&sections, show_no_match);

    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(view));
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(&mut child, |ui| {
            // Reserve the whole height so the scroll area knows what is below the fold; the
            // blocks are then painted at absolute positions inside that allocation.
            let (area, _) = ui.allocate_exact_size(
                egui::vec2(view.width(), height.max(1.0)),
                Sense::hover(),
            );
            let width = area.width();
            let mut top = area.top();
            let mut blocks = 0;

            for section in &sections {
                if section.total == 0 {
                    continue;
                }
                if blocks > 0 {
                    top += BLOCK_GAP;
                }
                draw_section(ui.painter(), area.left(), width, top, theme, section);
                top += section_height(section);
                blocks += 1;
            }

            // The source's page-level line, printed under both sections when the filter matched
            // nothing at all (`index.vue:227-229`).
            if show_no_match {
                if blocks > 0 {
                    top += BLOCK_GAP;
                }
                ui.painter().text(
                    egui::pos2(area.left(), top),
                    Align2::LEFT_TOP,
                    NO_SERVERS_FOUND,
                    fonts::body(LABEL_SIZE),
                    theme.SECONDARY,
                );
                blocks += 1;
            }

            if blocks > 0 {
                top += BLOCK_GAP;
            }
            let promo = Rect::from_min_size(
                egui::pos2(area.left(), top),
                egui::vec2(width, promo_height()),
            );
            draw_promo(ui, promo, theme);
        });

    false
}

// ---------------------------------------------------------------------------
// Heading and toolbar
// ---------------------------------------------------------------------------

/// Draw the heading, its count and the toolbar, and answer the y where the list begins.
///
/// The row is the source's `justify-end gap-2` cluster (`index.vue:104-120`): the search plate
/// first and the primary button last, right-aligned against the page's inset. The plate is
/// painted, not owned -- the field that takes keystrokes is a sibling's
/// (`widgets/search.rs`), and this page only draws what the source draws behind it.
fn heading_and_toolbar(
    painter: &egui::Painter,
    rect: Rect,
    theme: &Theme,
    count: usize,
    shown: usize,
) -> f32 {
    let heading_at = egui::pos2(rect.left() + PAGE_PADDING, rect.top() + PAGE_PADDING);
    // `h1.w-full.text-2xl.font-extrabold.text-contrast` (`index.vue:101`).
    let heading = painter.text(
        heading_at,
        Align2::LEFT_TOP,
        HEADING,
        fonts::heading(HEADING_SIZE),
        theme.CONTRAST,
    );

    // The count, in the catalogue's own plural (`app.settings.synced-options.multiplayer-servers.
    // count`). The source prints it inside the search placeholder instead (`index.vue:113`);
    // this line is beside the heading so a capture reads the size of the list without looking
    // into a field.
    painter.text(
        egui::pos2(heading.right() + COUNT_GAP, heading_at.y + HEADING_LINE / 2.0),
        Align2::LEFT_CENTER,
        count_phrase(count),
        fonts::label(LABEL_SIZE),
        theme.SECONDARY,
    );

    let row_top = heading_at.y + HEADING_LINE + HEADING_GAP;
    let width = button_width(painter, theme, NEW_SERVER);
    let button = Rect::from_min_size(
        egui::pos2(rect.right() - PAGE_PADDING - width, row_top),
        egui::vec2(width, CONTROL_HEIGHT),
    );
    let search = Rect::from_min_size(
        egui::pos2(button.left() - CONTROL_GAP - SEARCH_WIDTH, row_top),
        egui::vec2(SEARCH_WIDTH, CONTROL_HEIGHT),
    );
    search_plate(painter, search, theme, shown);
    primary_button(painter, button, theme, NEW_SERVER, Glyph::Plus);

    row_top + CONTROL_HEIGHT + HEADING_GAP
}

/// A brand button's width: `px-2.5 gap-1.5` around a 20-point glyph and its label
/// (`ButtonFrame.vue:31`), measured from the shaped label so a longer word is a wider button.
fn button_width(painter: &egui::Painter, theme: &Theme, label: &str) -> f32 {
    let text = painter.layout_no_wrap(
        label.to_owned(),
        fonts::semibold(BUTTON_TEXT),
        theme.ACCENT_CONTRAST,
    );
    2.0 * BUTTON_PADDING + BUTTON_GLYPH + BUTTON_GAP + text.size().x
}

/// The search field's plate: `bg-surface-4` inside `border-surface-5` at `rounded-xl`
/// (`InputFrame.vue:47-57`), a 20-point magnifier in `text-secondary` (`Input.vue:12-16`) and
/// the source's placeholder in the same muted ink (`Input.vue:51`).
fn search_plate(painter: &egui::Painter, rect: Rect, theme: &Theme, count: usize) {
    // `--surface-4` is the value `--color-button-bg` resolves to in both schemes, which is why
    // the crate's `BUTTON_BG` is the input's fill (`theme.rs`'s `colors()` says the same for
    // egui's own `extreme_bg_color`).
    painter.rect_filled(rect, CONTROL_RADIUS, theme.BUTTON_BG);
    painter.rect_stroke(
        rect,
        CONTROL_RADIUS,
        Stroke::new(STROKE_WIDTH, theme.SURFACE_5),
        egui::StrokeKind::Inside,
    );

    icons::draw(
        painter,
        Glyph::Search,
        Rect::from_center_size(
            egui::pos2(
                rect.left() + SEARCH_PADDING + SEARCH_GLYPH / 2.0,
                rect.center().y,
            ),
            Vec2::splat(SEARCH_GLYPH),
        ),
        theme.SECONDARY,
    );

    let text_left = rect.left() + SEARCH_PADDING + SEARCH_GLYPH + SEARCH_TEXT_GAP;
    let text_right = rect.right() - SEARCH_PADDING;
    let text_painter = painter.with_clip_rect(Rect::from_min_max(
        egui::pos2(text_left, rect.top()),
        egui::pos2(text_right, rect.bottom()),
    ));
    let galley = text_painter.layout_no_wrap(
        search_placeholder(count),
        fonts::label(SEARCH_TEXT),
        theme.SECONDARY,
    );
    text_painter.galley(
        egui::pos2(text_left, rect.center().y - galley.size().y / 2.0),
        galley,
        theme.SECONDARY,
    );
}

/// A `type="colored" color="brand"` button at `md`: the brand fill, the accent-contrast ink the
/// coloured variant sets (`ButtonFrame.vue:47`), the glyph first and the label after it
/// (`index.vue:116-119`).
fn primary_button(
    painter: &egui::Painter,
    rect: Rect,
    theme: &Theme,
    label: &str,
    glyph: Glyph,
) {
    painter.rect_filled(rect, CONTROL_RADIUS, theme.BRAND);
    icons::draw(
        painter,
        glyph,
        Rect::from_center_size(
            egui::pos2(rect.left() + BUTTON_PADDING + BUTTON_GLYPH / 2.0, rect.center().y),
            Vec2::splat(BUTTON_GLYPH),
        ),
        theme.ACCENT_CONTRAST,
    );
    painter.text(
        egui::pos2(
            rect.left() + BUTTON_PADDING + BUTTON_GLYPH + BUTTON_GAP,
            rect.center().y,
        ),
        Align2::LEFT_CENTER,
        label,
        fonts::semibold(BUTTON_TEXT),
        theme.ACCENT_CONTRAST,
    );
}

// ---------------------------------------------------------------------------
// The sections
// ---------------------------------------------------------------------------

/// Draw one section: its heading, then either its cards or the line that says the search emptied
/// it. Returns nothing -- the caller has already added [`section_height`] to its cursor.
fn draw_section(
    painter: &egui::Painter,
    left: f32,
    width: f32,
    top: f32,
    theme: &Theme,
    section: &Section<'_>,
) {
    // `h2.m-0.text-xl.font-semibold.text-primary` (`index.vue:169`).
    painter.text(
        egui::pos2(left, top),
        Align2::LEFT_TOP,
        section.heading,
        fonts::semibold(SECTION_HEADING_SIZE),
        theme.TEXT_PRIMARY,
    );

    let list_top = top + SECTION_HEADING_LINE + SECTION_GAP;
    if section.cards.is_empty() {
        // `div.text-secondary` in place of the list (`index.vue:195-197`).
        painter.text(
            egui::pos2(left, list_top),
            Align2::LEFT_TOP,
            section.empty,
            fonts::body(LABEL_SIZE),
            theme.SECONDARY,
        );
        return;
    }

    for (index, server) in section.cards.iter().enumerate() {
        let card = Rect::from_min_size(
            egui::pos2(left, list_top + index as f32 * (card_height() + LIST_GAP)),
            egui::vec2(width, card_height()),
        );
        draw_card(painter, card, theme, server);
    }
}

// ---------------------------------------------------------------------------
// The card
// ---------------------------------------------------------------------------

/// One server, drawn as `ServerListing.vue` lays it out: a 64-point plate, the name, the MOTD
/// and the labels row in a column `ml-4` to the right of it (lines 15-105).
fn draw_card(painter: &egui::Painter, rect: Rect, theme: &Theme, server: &Server) {
    painter.rect_filled(rect, CARD_RADIUS, theme.RAISED_BG);
    painter.rect_stroke(
        rect,
        CARD_RADIUS,
        Stroke::new(STROKE_WIDTH, theme.BUTTON_BG),
        egui::StrokeKind::Inside,
    );

    // The plate, vertically centred against the text column the way the row's `items-center`
    // centres the two (`ServerListing.vue:16`).
    let column = text_column_height();
    let icon_top = rect.top() + CARD_PADDING + (column - ICON_SIDE).max(0.0) / 2.0;
    let text_top = rect.top() + CARD_PADDING + (ICON_SIDE - column).max(0.0) / 2.0;
    let plate = Rect::from_min_size(
        egui::pos2(rect.left() + CARD_PADDING, icon_top),
        Vec2::splat(ICON_SIDE),
    );
    painter.rect_filled(plate, ICON_RADIUS, theme.BUTTON_BG);
    painter.rect_stroke(
        plate,
        ICON_RADIUS,
        Stroke::new(STROKE_WIDTH, theme.BUTTON_BORDER),
        egui::StrokeKind::Inside,
    );

    // The reference draws the server's uploaded icon here (`ServerListing.vue:36`); this reader
    // has no texture, so the plate carries the name's initial.
    let initial: String = server
        .name
        .chars()
        .next()
        .map_or(String::new(), |first| first.to_uppercase().collect());
    painter.text(
        plate.center(),
        Align2::CENTER_CENTER,
        initial,
        fonts::title(ICON_INITIAL),
        theme.SECONDARY,
    );

    let text_left = plate.right() + ICON_GAP;
    let text_width = (rect.right() - CARD_PADDING - text_left).max(1.0);

    // `text-xl font-bold text-contrast` (`ServerListing.vue:39`).
    let name_bottom = text_top + NAME_LINE;
    clipped_line(
        painter,
        Rect::from_min_size(
            egui::pos2(text_left, text_top),
            egui::vec2(text_width, NAME_LINE),
        ),
        server.name,
        fonts::title(NAME_SIZE),
        theme.CONTRAST,
    );

    // The MOTD: `text-secondary` (`WorldItem.vue:578`), wrapped and clamped to
    // [`MOTD_LINES`] lines with the ellipsis egui's own `max_rows` adds.
    let motd_top = name_bottom + CARD_TEXT_GAP;
    let motd_box = Rect::from_min_size(
        egui::pos2(text_left, motd_top),
        egui::vec2(text_width, MOTD_LINE * MOTD_LINES as f32),
    );
    let mut job = egui::text::LayoutJob::simple(
        server.motd.to_owned(),
        fonts::body(LABEL_SIZE),
        theme.SECONDARY,
        motd_box.width(),
    );
    job.wrap.max_rows = MOTD_LINES;
    job.wrap.overflow_character = Some('…');
    let motd = painter.layout_job(job);
    painter.with_clip_rect(motd_box).galley(motd_box.min, motd, theme.SECONDARY);

    // The labels row: the status dot and its count, then the game and version, then the loader,
    // separated by the reference's 6-point bullets (`Separator.vue:2`, `ServerListing.vue:103`).
    let meta_top = motd_box.bottom() + CARD_TEXT_GAP;
    let meta_centre = meta_top + META_LINE / 2.0;
    let (ink, _) = status_dot(server.players);
    let dot_centre = egui::pos2(text_left + DOT_RADIUS, meta_centre);
    painter.circle_filled(dot_centre, DOT_RADIUS, ink);

    // The reference prints the whole row in `text-primary` and colours only its icon
    // (`ServerListing.vue:103`, `servers-modal.vue:393-398`), so the state is carried by the dot
    // alone and the words stay in the page's body ink.
    let mut x = dot_centre.x + DOT_RADIUS + DOT_GAP;
    x = label(painter, x, meta_top, &status_label(server.players), theme.TEXT_PRIMARY);
    x = bullet(painter, x, meta_centre, theme);
    let game = format!("Minecraft {}", server.mc_version);
    x = label(painter, x, meta_top, &game, theme.TEXT_PRIMARY);
    x = bullet(painter, x, meta_centre, theme);
    label(painter, x, meta_top, server.loader, theme.TEXT_PRIMARY);
}

/// Draw one label of the card's bottom row and answer the x after it.
fn label(painter: &egui::Painter, left: f32, top: f32, text: &str, ink: Color32) -> f32 {
    let galley = painter.layout_no_wrap(text.to_owned(), fonts::label(LABEL_SIZE), ink);
    painter.galley(
        egui::pos2(left, top + (META_LINE - galley.size().y) / 2.0),
        galley.clone(),
        ink,
    );
    left + galley.size().x + LABEL_GAP
}

/// Draw the 6-point separator between two labels and answer the x after it.
fn bullet(painter: &egui::Painter, left: f32, centre_y: f32, theme: &Theme) -> f32 {
    painter.circle_filled(
        egui::pos2(left + SEPARATOR / 2.0, centre_y),
        SEPARATOR / 2.0,
        theme.BUTTON_BORDER,
    );
    left + SEPARATOR + LABEL_GAP
}

/// Draw one line of text in its own box. A name longer than the card ends at the card's edge
/// rather than running under the plate or past the padding; the reference truncates the same
/// line (`ServerListing.vue:39`).
fn clipped_line(
    painter: &egui::Painter,
    rect: Rect,
    text: &str,
    font: egui::FontId,
    ink: Color32,
) {
    let galley = painter.layout_no_wrap(text.to_owned(), font, ink);
    painter.with_clip_rect(rect).galley(
        egui::pos2(rect.left(), rect.center().y - galley.size().y / 2.0),
        galley,
        ink,
    );
}

// ---------------------------------------------------------------------------
// The promo
// ---------------------------------------------------------------------------

/// The `ServersPromo` banner: a brand-bordered card with a two-line heading, one line of body
/// copy, a `View plans` button at the bottom right and a dismiss control at the top right
/// (`ServersPromo.vue:21-54`).
///
/// The sheet fills it with a gradient from `--color-brand-highlight` at `-80%` to `--color-bg`
/// (`ServersPromo.vue:57-64`) and drops the Modrinth mark into the top-right corner at 25%
/// opacity. egui cannot paint a gradient through a rounded rect, so the fill is the page colour
/// with the same brand wash laid flat over it, and the mark is left off -- it would be a
/// 192-point glyph clipped by a rectangular clip rect rather than the card's rounded one.
fn draw_promo(ui: &mut Ui, rect: Rect, theme: &Theme) {
    // The dismiss control first, because it is the promo's only hover target: an `xs` icon
    // button at `top-2 right-2` (`ServersPromo.vue:42-53`) carrying the reference's tooltip
    // (`ServersPromo.vue:45`). The click that hides the banner belongs to whatever owns its
    // visibility, and this page has no such state, so the press is acknowledged and nothing
    // else -- which is why the page still owes no frame.
    let dismiss = Rect::from_min_size(
        egui::pos2(
            rect.right() - PROMO_DISMISS_INSET - PROMO_DISMISS,
            rect.top() + PROMO_DISMISS_INSET,
        ),
        Vec2::splat(PROMO_DISMISS),
    );
    let dismiss = ui
        .interact(
            dismiss,
            egui::Id::new("servers-promo-dismiss"),
            Sense::click(),
        )
        .on_hover_text(PROMO_DISMISS_LABEL);

    let painter = ui.painter();
    painter.rect_filled(rect, PROMO_RADIUS, theme.BG);
    painter.rect_filled(rect, PROMO_RADIUS, theme.BRAND_HIGHLIGHT);
    painter.rect_stroke(
        rect,
        PROMO_RADIUS,
        Stroke::new(STROKE_WIDTH, theme.BRAND),
        egui::StrokeKind::Inside,
    );

    let text_left = rect.left() + PROMO_PADDING;
    let mut top = rect.top() + PROMO_PADDING;
    // The source's heading is two lines with a `<br/>` between them, the second in `text-brand`
    // (`ServersPromo.vue:31-34`), at `text-lg leading-tight font-extrabold`.
    painter.text(
        egui::pos2(text_left, top),
        Align2::LEFT_TOP,
        PROMO_LINE_ONE,
        fonts::heading(PROMO_TITLE_SIZE),
        theme.CONTRAST,
    );
    top += PROMO_TITLE_LINE;
    painter.text(
        egui::pos2(text_left, top),
        Align2::LEFT_TOP,
        PROMO_LINE_TWO,
        fonts::heading(PROMO_TITLE_SIZE),
        theme.BRAND,
    );
    top += PROMO_TITLE_LINE + PROMO_TEXT_GAP;
    painter.text(
        egui::pos2(text_left, top),
        Align2::LEFT_TOP,
        PROMO_BODY,
        fonts::label(LABEL_SIZE),
        theme.BASE,
    );

    // The button, bottom right: `View plans` then the arrow, at the brand button's own metrics
    // (`ServersPromo.vue:37-41`).
    let label_galley = painter.layout_no_wrap(
        PROMO_ACTION.to_owned(),
        fonts::semibold(BUTTON_TEXT),
        theme.ACCENT_CONTRAST,
    );
    let width = 2.0 * BUTTON_PADDING + label_galley.size().x + BUTTON_GAP + PROMO_BUTTON_GLYPH;
    let button = Rect::from_min_size(
        egui::pos2(
            rect.right() - PROMO_PADDING - width,
            rect.bottom() - PROMO_PADDING - PROMO_BUTTON_HEIGHT,
        ),
        egui::vec2(width, PROMO_BUTTON_HEIGHT),
    );
    painter.rect_filled(button, CONTROL_RADIUS, theme.BRAND);
    painter.galley(
        egui::pos2(
            button.left() + BUTTON_PADDING,
            button.center().y - label_galley.size().y / 2.0,
        ),
        label_galley.clone(),
        theme.ACCENT_CONTRAST,
    );
    let arrow_x = button.left()
        + BUTTON_PADDING
        + label_galley.size().x
        + BUTTON_GAP
        + PROMO_BUTTON_GLYPH / 2.0;
    icons::draw(
        painter,
        Glyph::RightArrow,
        Rect::from_center_size(
            egui::pos2(arrow_x, button.center().y),
            Vec2::splat(PROMO_BUTTON_GLYPH),
        ),
        theme.ACCENT_CONTRAST,
    );

    // The X brightens to the contrast ink under the pointer, which is the `IconButton`'s own
    // hover feedback in the reference.
    icons::draw(
        painter,
        Glyph::X,
        Rect::from_center_size(dismiss.rect.center(), Vec2::splat(PROMO_DISMISS_GLYPH)),
        if dismiss.hovered() {
            theme.CONTRAST
        } else {
            theme.SECONDARY
        },
    );
}

// ---------------------------------------------------------------------------
// The no-servers screen
// ---------------------------------------------------------------------------

/// `ServerListEmpty`: a 320-point column centred in the page, the title at 30/36, the body at
/// 16/24, three feature rows with brand glyphs on `bg-surface-1` plates, and the `New server`
/// button with its `Learn more` link (`ServerListEmpty.vue:2-59`).
///
/// The decorative invite-modal preview beside the column (`ServerListEmpty.vue:61`) and the
/// `Already have a server?` sign-in block at the bottom (`:64-72`, shown only when signed out)
/// are not drawn: this reader has no account and the preview is a static mock.
fn empty_state(painter: &egui::Painter, rect: Rect, theme: &Theme) {
    let area = Rect::from_min_max(
        egui::pos2(rect.left() + PAGE_PADDING, rect.top() + EMPTY_PAD_Y),
        egui::pos2(rect.right() - PAGE_PADDING, rect.bottom() - EMPTY_PAD_Y),
    );
    let column = EMPTY_COLUMN.min(area.width()).min(EMPTY_MAX);
    let total = empty_height();
    if area.height() < total || column < 1.0 {
        return;
    }

    let left = area.center().x - column / 2.0;
    let mut top = area.center().y - total / 2.0;

    // `h1.m-0.text-3xl.font-semibold.leading-9.text-contrast` (`ServerListEmpty.vue:7`).
    painter.text(
        egui::pos2(left, top),
        Align2::LEFT_TOP,
        EMPTY_HEADING,
        fonts::semibold(EMPTY_HEADING_SIZE),
        theme.CONTRAST,
    );
    top += EMPTY_HEADING_LINE + EMPTY_TEXT_GAP;

    // `p.m-0.text-base.text-primary` (`ServerListEmpty.vue:11`), in the two lines
    // [`EMPTY_BODY_LINES`] reserves so the block below starts where [`empty_height`] said it
    // would.
    let body_box = Rect::from_min_size(
        egui::pos2(left, top),
        egui::vec2(column, EMPTY_BODY_LINES * EMPTY_BODY_LINE),
    );
    let body = painter.layout(
        EMPTY_BODY.to_owned(),
        fonts::body(EMPTY_BODY_SIZE),
        theme.TEXT_PRIMARY,
        body_box.width(),
    );
    painter
        .with_clip_rect(body_box)
        .galley(body_box.min, body, theme.TEXT_PRIMARY);
    top += body_box.height() + EMPTY_BLOCK_GAP;

    // The three features (`ServerListEmpty.vue:17-42`), each a brand glyph on a `size-10`
    // `bg-surface-1` plate beside a title and a description.
    let text_left = left + EMPTY_FEATURE_PLATE + EMPTY_FEATURE_GAP_X;
    let text_width = (column - EMPTY_FEATURE_PLATE - EMPTY_FEATURE_GAP_X).max(1.0);
    for (glyph, title, description) in [
        (
            Glyph::PackageOpen,
            EMPTY_FEATURE_ONE_TITLE,
            EMPTY_FEATURE_ONE_BODY,
        ),
        (Glyph::Globe, EMPTY_FEATURE_TWO_TITLE, EMPTY_FEATURE_TWO_BODY),
        (
            Glyph::Users,
            EMPTY_FEATURE_THREE_TITLE,
            EMPTY_FEATURE_THREE_BODY,
        ),
    ] {
        let plate = Rect::from_min_size(
            egui::pos2(left, top),
            Vec2::splat(EMPTY_FEATURE_PLATE),
        );
        painter.rect_filled(plate, EMPTY_PLATE_RADIUS, theme.SURFACE_1);
        painter.rect_stroke(
            plate,
            EMPTY_PLATE_RADIUS,
            Stroke::new(STROKE_WIDTH, theme.DIVIDER),
            egui::StrokeKind::Inside,
        );
        icons::draw(
            painter,
            glyph,
            Rect::from_center_size(plate.center(), Vec2::splat(EMPTY_FEATURE_GLYPH)),
            theme.BRAND,
        );
        painter.text(
            egui::pos2(text_left, top),
            Align2::LEFT_TOP,
            title,
            fonts::semibold(EMPTY_FEATURE_TITLE_SIZE),
            theme.CONTRAST,
        );
        let description = painter.layout(
            description.to_owned(),
            fonts::body(LABEL_SIZE),
            theme.TEXT_PRIMARY,
            text_width,
        );
        painter.galley(
            egui::pos2(
                text_left,
                top + EMPTY_FEATURE_TITLE + EMPTY_FEATURE_INNER,
            ),
            description,
            theme.TEXT_PRIMARY,
        );
        top += EMPTY_FEATURE_TITLE
            + EMPTY_FEATURE_INNER
            + 2.0 * EMPTY_FEATURE_BODY
            + EMPTY_FEATURE_GAP;
    }
    // The loop above leaves one feature gap after the last row; the block gap the actions want
    // is measured from the row itself, so the difference is taken back here.
    top += EMPTY_BLOCK_GAP - EMPTY_FEATURE_GAP;

    // `Button type="colored" color="brand" size="lg"` (`ServerListEmpty.vue:46-49`) and the
    // `Learn more` link beside it (`:50-57`).
    let action_label = painter.layout_no_wrap(
        EMPTY_ACTION.to_owned(),
        fonts::semibold(BUTTON_TEXT),
        theme.ACCENT_CONTRAST,
    );
    let action_width = 2.0 * EMPTY_ACTION_PADDING
        + BUTTON_GLYPH
        + EMPTY_ACTION_GAP
        + action_label.size().x;
    let action = Rect::from_min_size(
        egui::pos2(left, top),
        egui::vec2(action_width, EMPTY_ACTION_HEIGHT),
    );
    painter.rect_filled(action, EMPTY_ACTION_RADIUS, theme.BRAND);
    icons::draw(
        painter,
        Glyph::Plus,
        Rect::from_center_size(
            egui::pos2(
                action.left() + EMPTY_ACTION_PADDING + BUTTON_GLYPH / 2.0,
                action.center().y,
            ),
            Vec2::splat(BUTTON_GLYPH),
        ),
        theme.ACCENT_CONTRAST,
    );
    painter.text(
        egui::pos2(
            action.left() + EMPTY_ACTION_PADDING + BUTTON_GLYPH + EMPTY_ACTION_GAP,
            action.center().y,
        ),
        Align2::LEFT_CENTER,
        EMPTY_ACTION,
        fonts::semibold(BUTTON_TEXT),
        theme.ACCENT_CONTRAST,
    );

    let link = painter.layout_no_wrap(
        EMPTY_LINK.to_owned(),
        fonts::semibold(BUTTON_TEXT),
        theme.BRAND,
    );
    let link_left = action.right() + EMPTY_ACTIONS_GAP;
    let link_top = action.center().y - link.size().y / 2.0;
    painter.galley(egui::pos2(link_left, link_top), link.clone(), theme.BRAND);
    icons::draw(
        painter,
        Glyph::RightArrow,
        Rect::from_center_size(
            egui::pos2(
                link_left + link.size().x + EMPTY_LINK_GAP + BUTTON_GLYPH / 2.0,
                action.center().y,
            ),
            Vec2::splat(BUTTON_GLYPH),
        ),
        theme.BRAND,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three dot states the brief asks for, with the ink and the word each one carries.
    ///
    /// Online is the brand green and its state word is `Online`; the count it shows is
    /// [`status_label`]'s job, asserted below. Offline is the red the project page's own offline
    /// tag uses, unknown is the secondary ink.
    #[test]
    fn the_dot_is_green_red_or_secondary_by_ping() {
        assert_eq!(
            status_dot(Some((12, 60))),
            (theme::DARK.BRAND, "Online"),
            "an answered ping with a capacity is brand green"
        );
        assert_eq!(
            status_dot(Some((0, 0))),
            (theme::DARK.RED, "Offline"),
            "a ping that came back empty is the reference's offline row"
        );
        assert_eq!(
            status_dot(None),
            (theme::DARK.SECONDARY, "Unknown"),
            "a server nothing has asked about says so in the muted ink"
        );
    }

    /// The text beside the dot: the count when there is one, the state's word when there is not.
    #[test]
    fn the_count_is_shown_where_the_ping_gave_one() {
        assert_eq!(status_label(Some((12, 60))), "12/60");
        assert_eq!(status_label(Some((0, 0))), "Offline");
        assert_eq!(status_label(None), "Unknown");
    }

    /// A full-capacity server is still online, and a capacity of zero is the empty answer --
    /// the boundary the rule turns on.
    #[test]
    fn a_full_server_is_online_and_no_capacity_is_offline() {
        assert_eq!(status_dot(Some((60, 60))), (theme::DARK.BRAND, "Online"));
        assert_eq!(status_dot(Some((0, 1))), (theme::DARK.BRAND, "Online"));
        assert_eq!(status_dot(Some((4, 0))), (theme::DARK.RED, "Offline"));
    }

    /// The ICU plural both catalogues carry: `one` says `server`, everything else `servers`,
    /// including the zero a search that matched nothing produces.
    #[test]
    fn the_count_reads_as_the_catalogues_plural() {
        assert_eq!(count_phrase(1), "1 server");
        assert_eq!(count_phrase(6), "6 servers");
        assert_eq!(count_phrase(0), "0 servers");
        assert_eq!(search_placeholder(6), "Search 6 servers...");
        assert_eq!(search_placeholder(1), "Search 1 server...");
    }

    /// The order the source's own comparator produces, mapped onto what this reader knows:
    /// reachable first, unasked next, answered-with-nothing last, then by name.
    #[test]
    fn the_source_order_puts_reachable_servers_first() {
        let rows = listed(&SERVERS, false, "", Sort::Status);
        let names: Vec<&str> = rows.iter().map(|server| server.name).collect();
        assert_eq!(
            names,
            vec![
                "All the Mods 10",
                "Modrinth SMP",
                "Create: Astral",
                "Vanilla+ Survival",
            ]
        );
    }

    /// Alphabetical, and most players first with the name breaking the tie.
    #[test]
    fn the_other_sorts_order_by_name_and_by_players() {
        let by_name: Vec<&str> = listed(&SERVERS, false, "", Sort::Name)
            .iter()
            .map(|server| server.name)
            .collect();
        assert_eq!(
            by_name,
            vec![
                "All the Mods 10",
                "Create: Astral",
                "Modrinth SMP",
                "Vanilla+ Survival",
            ]
        );

        let by_players: Vec<&str> = listed(&SERVERS, false, "", Sort::Players)
            .iter()
            .map(|server| server.name)
            .collect();
        assert_eq!(
            by_players,
            vec![
                "Modrinth SMP",
                "All the Mods 10",
                "Create: Astral",
                "Vanilla+ Survival",
            ]
        );
    }

    /// The filter reads the same fields the reference's Fuse index does, less the owner it has
    /// no data for, and it splits the two sections.
    #[test]
    fn the_query_filters_by_name_loader_and_version() {
        let fabric: Vec<&str> = listed(&SERVERS, true, "fabric", Sort::Name)
            .iter()
            .map(|server| server.name)
            .collect();
        assert_eq!(fabric, vec!["Better MC [FABRIC]", "Cobblemon Islands"]);

        let owned_fabric: Vec<&str> = listed(&SERVERS, false, "fabric", Sort::Name)
            .iter()
            .map(|server| server.name)
            .collect();
        assert_eq!(owned_fabric, vec!["Modrinth SMP", "Vanilla+ Survival"]);

        assert_eq!(listed(&SERVERS, false, "1.21.4", Sort::Name).len(), 2);
        assert_eq!(listed(&SERVERS, false, "skyblock", Sort::Name).len(), 1);
        assert!(listed(&SERVERS, false, "zzz", Sort::Name).is_empty());
        assert_eq!(listed(&SERVERS, true, "  ", Sort::Name).len(), 2);
    }

    /// The sample rows exist so one capture shows every state: each of the three appears, and
    /// the six rows are split across both sections.
    #[test]
    fn the_sample_shows_all_three_states_and_both_sections() {
        let mut online: usize = 0;
        let mut offline: usize = 0;
        let mut unknown: usize = 0;
        for server in SERVERS.iter() {
            match status_dot(server.players).1 {
                "Online" => online += 1,
                "Offline" => offline += 1,
                _ => unknown += 1,
            }
        }
        assert!(online >= 1, "a capture needs an online row");
        assert!(offline >= 1, "a capture needs an offline row");
        assert!(unknown >= 1, "a capture needs an unknown row");
        assert_eq!(online + offline + unknown, SERVERS.len());

        assert!(!listed(&SERVERS, false, "", Sort::Status).is_empty());
        assert!(!listed(&SERVERS, true, "", Sort::Status).is_empty());
    }

    /// One card is its padding plus the taller of the text column and the plate: 16 + 100 + 16
    /// at the reference's own boxes, which is what the list arithmetic below is built on.
    #[test]
    fn a_card_is_its_padding_around_the_text_column() {
        assert_eq!(
            text_column_height(),
            100.0,
            "28 name + 6 + 2*20 MOTD + 6 + 20 labels"
        );
        assert_eq!(card_height(), 132.0, "2 * 16 padding + the 100-point column");
        assert!(
            card_height() >= 2.0 * CARD_PADDING + ICON_SIDE,
            "the 64-point plate must not push through the card's padding"
        );
    }

    /// The two sections add up the way the source's `gap-3` and `gap-6` say they do, and the
    /// whole list is taller than the column the shell gives the page at 1440x900 -- which is why
    /// the page scrolls rather than clipping the promo.
    #[test]
    fn the_sections_and_the_promo_add_up_and_overflow_the_column() {
        let sections = [
            Section {
                heading: YOUR_SERVERS,
                empty: NO_OWNED_FOUND,
                total: 4,
                cards: listed(&SERVERS, false, "", Sort::Status),
            },
            Section {
                heading: SHARED_SERVERS,
                empty: NO_SHARED_FOUND,
                total: 2,
                cards: listed(&SERVERS, true, "", Sort::Status),
            },
        ];

        assert_eq!(
            section_height(&sections[0]),
            604.0,
            "28 heading + 12 gap + 4 cards and 3 gaps"
        );
        assert_eq!(
            section_height(&sections[1]),
            316.0,
            "28 heading + 12 gap + 2 cards and 1 gap"
        );
        assert_eq!(
            promo_height(),
            105.0,
            "32 padding + 2*22.5 heading + 8 + 20 body"
        );
        assert_eq!(
            content_height(&sections, false),
            1073.0,
            "604 + 24 + 316 + 24 + 105"
        );

        // The shell's content column (`geometry.rs`): 900 - 40 window, less the page's own
        // insets, heading, toolbar and its gaps.
        let column = crate::geometry::WINDOW.1
            - crate::geometry::TITLE_BAR
            - 2.0 * PAGE_PADDING
            - HEADING_LINE
            - 2.0 * HEADING_GAP
            - CONTROL_HEIGHT;
        assert_eq!(column, 712.0, "the list's own viewport at 1440x900");
        assert!(
            content_height(&sections, false) > column,
            "the sample list is taller than its viewport, so the page scrolls"
        );
    }

    /// A filtered-out section shrinks to its no-match line, and the page-level line is added
    /// only when both sections are empty -- the source's own two rules (`index.vue:195-229`).
    #[test]
    fn an_emptied_section_keeps_its_heading_and_one_line() {
        let sections = [
            Section {
                heading: YOUR_SERVERS,
                empty: NO_OWNED_FOUND,
                total: 4,
                cards: listed(&SERVERS, false, "zzz", Sort::Status),
            },
            Section {
                heading: SHARED_SERVERS,
                empty: NO_SHARED_FOUND,
                total: 2,
                cards: listed(&SERVERS, true, "zzz", Sort::Status),
            },
        ];
        assert_eq!(
            section_height(&sections[0]),
            28.0 + 12.0 + META_LINE,
            "the heading and the gap stay; the list becomes one line"
        );
        assert_eq!(
            content_height(&sections, true),
            2.0 * (28.0 + 12.0 + META_LINE)
                + 3.0 * BLOCK_GAP
                + META_LINE
                + promo_height(),
            "two sections, the page-level line, and the promo between three gaps"
        );
    }

    /// A section nobody owns does not render at all, which is the source's `v-if` on the
    /// unfiltered list (`index.vue:168`).
    #[test]
    fn a_section_with_no_rows_of_its_own_is_not_drawn() {
        let sections = [
            Section {
                heading: YOUR_SERVERS,
                empty: NO_OWNED_FOUND,
                total: 4,
                cards: listed(&SERVERS, false, "", Sort::Status),
            },
            Section {
                heading: SHARED_SERVERS,
                empty: NO_SHARED_FOUND,
                total: 0,
                cards: Vec::new(),
            },
        ];
        assert_eq!(
            content_height(&sections, false),
            section_height(&sections[0]) + BLOCK_GAP + promo_height(),
            "the empty section costs nothing, and only one gap separates the list from the promo"
        );
    }

    /// The no-servers screen is the source's own box arithmetic, and it fits the column.
    #[test]
    fn the_empty_screen_is_a_heading_three_features_and_two_actions() {
        let feature = EMPTY_FEATURE_TITLE + EMPTY_FEATURE_INNER + 2.0 * EMPTY_FEATURE_BODY;
        assert_eq!(feature, 66.0, "24 title + 2 + two 20-point body lines");
        assert_eq!(
            empty_height(),
            442.0,
            "36 title + 8 + 2*24 body + 32 + 3*66 + 2*24 + 32 + 40 actions"
        );
        assert!(
            empty_height() < crate::geometry::WINDOW.1 - crate::geometry::TITLE_BAR,
            "the empty screen has to fit the content column"
        );
    }

    /// The state's defaults are the sample rows, no query and the source's own order, and a
    /// handed-in slice wins over the sample -- the seam a capture uses to show the empty screen.
    #[test]
    fn the_default_state_paints_the_sample_and_a_handed_slice_wins() {
        let state = State::default();
        assert_eq!(state.search, "");
        assert_eq!(state.sort, Sort::Status);
        assert_eq!(state.servers.len(), SERVERS.len());

        let empty = State {
            servers: &[],
            ..State::default()
        };
        assert!(empty.servers.is_empty());
    }
}
