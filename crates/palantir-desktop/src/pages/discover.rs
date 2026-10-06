//! Discover: `pages/Browse.vue`, at `/browse/:projectType`.
//!
//! The reference's layout, in its own order and with its own numbers:
//!
//! 1. the project-type tabs (`NavTabs`), built from `Browse.vue`'s own list and
//!    labelled with its category messages;
//! 2. the search field, whose placeholder is `browse.search.placeholder` --
//!    *"Search {projectType}..."* -- with the tab's own name in it;
//! 3. the two controls of `browse-tab/layout.vue`'s own row, each with its
//!    prefix: `label.sort-by` with the five orders `useSearch` declares, and
//!    `browse.view-prefix` with the view sizes `[5, 10, 15, 20, 50, 100]`, of
//!    which 20 is the reference's default. Nothing else is in that row on the
//!    desktop: the *Filter results...* button is inside a `lg:hidden` div and
//!    the row's right end is `Pagination`, which needs a page count this page
//!    does not have;
//! 4. the results; while the first answer is on the way, the three loading
//!    blocks of `base/LoadingIndicator.vue`; and, with none, the sentence
//!    *"No results found for your query!"*.
//!
//! The five names in the sort control are *not* in the locale: `ui/src/utils/search.ts`
//! writes them as literals (`{ display: 'Relevance', name: 'relevance' }`, and so
//! on). They are literals here too, and quoted from that file, because translating
//! a string the reference does not translate would be inventing a difference
//! rather than removing one.
//!
//! The results themselves come from Modrinth's search API, and the page *asks* for
//! them rather than fetching them: [`State::update`] and [`State::opening`] hand the
//! shell an [`Ask`], the shell runs it through the store off the frame thread, and
//! the answer comes back as [`Message::Found`]. A store with no engine to ask
//! answers with a sentence rather than a list (see [`crate::store`]), and the four
//! states of the answer draw through [`crate::page::draw`] either way.
//!
//! The cards are `ProjectCard.vue` in its *list* layout -- the reference's browse
//! draws `ProjectCardList` with `effectiveLayout`, whose own default is `list` --
//! and a card's icon is a second request rather than part of the first: a search
//! answers with twenty `icon_url`s and no pictures, so the page asks for the icons
//! once the results are on screen ([`Message::Icons`]) and draws each one's box
//! whether the picture has arrived or not. Waiting for twenty PNGs before drawing a
//! single row would put a download in front of a list the reader could already
//! read.
//!
//! The vocabulary -- the five orders, the six view sizes, the three messages -- is
//! declared here rather than in the search code, because the strings the controls
//! show are the reference's own literals and a label invented elsewhere is a label
//! nobody can trace. [`State::request`] is the other half of that: the controls and
//! the query string are one thing read twice.
#![allow(dead_code)]

use std::collections::{BTreeSet, HashMap};

use iced::mouse::Interaction;
use iced::widget::{column, container, mouse_area, row, Space};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Padding};
use palantir_net::engine::Search as ApiSearch;
use palantir_net::ModrinthSearchHit;

use crate::avatar::{self, Fetched, Icon};
use crate::icon;
use crate::icons_gen::Glyph;
use crate::locale;
use crate::page::{self, Load, GAP, INSET, ROW_GAP};
use crate::pages::Ask;
use crate::route::ProjectType;
use crate::scroll::{self, Geometry};
use crate::store::Store;
use crate::style::{
    at_opacity, medium, regular, semibold, INK_CONTRAST, INK_DEFAULT, INK_SECONDARY,
};
use crate::text_gen::{self, Key};
use crate::theme_gen::{self, Ink, Theme as Gen};
// `Hovered` is in scope for the result cards below: a card names its own crossing
// rather than going through one of the kit's controls.
use crate::ui::{self, text, Hovered};

/// One project in the results.
///
/// The fields are the ones the reference's card draws, in the order its API
/// returns them, so the store that fills this in has one shape to fill rather than
/// a page to restructure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// Project id, as the route and the API use it.
    pub id: String,
    /// Title.
    pub title: String,
    /// Author or organization.
    pub author: String,
    /// One-line summary.
    pub summary: String,
    /// Total downloads.
    pub downloads: u64,
    /// Followers.
    pub follows: u64,
    /// Where the project's own icon is, as the search API hands it out.
    ///
    /// Empty when a project has none, which is a state the reference draws
    /// rather than an error: its `Avatar` falls back to an outline box. This
    /// shell draws nothing there, which [`crate::avatar`] names as a departure.
    pub icon_url: String,
    /// The versions the project supports, as the card's tags draw them.
    pub game_versions: Vec<String>,
    /// Its loaders or categories.
    pub loaders: Vec<String>,
}

impl Hit {
    /// One card, from one hit of the search API.
    ///
    /// The translation lives here rather than in the store so that the store can
    /// answer with whatever the API gives it: what a card *is* belongs to the page
    /// that draws it, and a card that grew a field would otherwise change a
    /// module that has never drawn one.
    pub fn from_api(hit: &ModrinthSearchHit) -> Hit {
        Hit {
            id: hit.project_ref().to_string(),
            title: hit.title.clone(),
            author: hit.author.clone(),
            summary: hit.description.clone(),
            downloads: hit.downloads,
            follows: hit.follows,
            // The API calls them `versions` and `categories`; the card calls them
            // the tags a reader compares against their own game (`hit_card` draws
            // the loaders first).
            game_versions: hit.versions.clone(),
            loaders: hit.categories.clone(),
            icon_url: hit.icon_url.clone(),
        }
    }
}

// The pointer's crossings, recorded into the clock every control draws from.
// One impl per page, and the macro is what keeps them all the same shape: a page
// whose `update` records `Hover { key, over }` is a page whose controls can be
// built by `crate::ui`.
crate::hovered!(Message);

/// One stable name per project-type tab, in `ProjectType::TABS`' order.
///
/// Not derived from the label: the label is the locale's and may be retranslated,
/// and a tab that changed its key on a language change would be a tab whose hover
/// tween is forgotten mid-flight.
const TAB_KEYS: [&str; 6] = [
    "discover:tab:modpack",
    "discover:tab:mod",
    "discover:tab:resourcepack",
    "discover:tab:datapack",
    "discover:tab:shader",
    "discover:tab:server",
];

/// The five orders the search can be asked in.
///
/// `ui/src/utils/search.ts`'s own list, literals and all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sort {
    /// Best match for the query, which is what the reference opens on.
    #[default]
    Relevance,
    /// Most downloaded.
    Downloads,
    /// Most followed.
    Followers,
    /// Newest publication date.
    DatePublished,
    /// Newest update.
    DateUpdated,
}

impl Sort {
    /// Every order, in the reference's order.
    pub const ALL: [Sort; 5] = [
        Sort::Relevance,
        Sort::Downloads,
        Sort::Followers,
        Sort::DatePublished,
        Sort::DateUpdated,
    ];

    /// The label the reference's control shows, quoted from `search.ts`.
    pub const fn label(self) -> &'static str {
        match self {
            Sort::Relevance => "Relevance",
            Sort::Downloads => "Downloads",
            Sort::Followers => "Followers",
            Sort::DatePublished => "Date published",
            Sort::DateUpdated => "Date updated",
        }
    }

    /// The API's own name for the order, which is what the query string carries.
    pub const fn token(self) -> &'static str {
        match self {
            Sort::Relevance => "relevance",
            Sort::Downloads => "downloads",
            Sort::Followers => "follows",
            Sort::DatePublished => "newest",
            Sort::DateUpdated => "updated",
        }
    }
}

/// Which of the page's two comboboxes has its panel open.
///
/// One or the other, never both: `Combobox.vue` opens on a trigger press and
/// this page has no way to have two pointers. The panel itself is
/// [`crate::ui::select_menu`]'s, and where it sits on the page is [`menu`]'s.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    /// The order the results are in, the `!w-[16rem]` one.
    Sort,
    /// How many results a page holds, the `!w-[9rem]` one.
    View,
}

/// How many results a page holds.
///
/// `[5, 10, 15, 20, 50, 100]`, of which the reference opens on 20.
pub const VIEW_SIZES: [usize; 6] = [5, 10, 15, 20, 50, 100];
/// The size the reference opens on.
pub const DEFAULT_VIEW: usize = 20;

/// What Discover can be told.
#[derive(Debug, Clone)]
pub enum Message {
    /// The project-type tab changed.
    ProjectType(ProjectType),
    /// The search text changed.
    Query(String),
    /// A different order was chosen.
    Sort(Sort),
    /// A different view size was chosen.
    View(usize),
    /// A later page was asked for.
    Page(usize),
    /// The sidebar's *Hide already installed* switch was pressed.
    ///
    /// The page owns only the *flag*: which projects that hides is the shell's
    /// to read, because they are the instances it holds ([`Asked::hide_installed`]).
    HideInstalled(bool),
    /// A category was chosen, or unchosen.
    ///
    /// The name, not the header: the header is which *section* the row is in and
    /// the request does not know sections -- `search.ts` folds every chosen
    /// category of every section into one `categories = "..."` part.
    Category(String),
    /// A filter option that is not a category was chosen, or unchosen.
    ///
    /// The section's own id beside the option's, because the two are what the
    /// request is built from: `search.ts` folds every chosen option of a filter
    /// into that filter's own part, and the option's id is the value it carries
    /// (`environment:client`, `open_source:true`, `categories:fabric`).
    Filter { filter: String, option: String },
    /// A filter option was excluded, or unexcluded.
    ///
    /// The second press of a row, beside the one that chooses it, and a request
    /// like the first: `supports: ['include', 'exclude']` is on every category
    /// section and on the loaders and the license, and what an exclusion asks for
    /// is a `NOT IN` group rather than nothing at all. The section's id is the
    /// reference's own `category_<project_type>_<header>` where the row is a
    /// category, because that is what `search.ts` keys its filters by.
    Exclude { filter: String, option: String },
    /// A game version was chosen, or unchosen.
    ///
    /// `game_version`'s options are `method: 'or'` over the field
    /// `game_versions`, so two of them are one `IN` -- the same shape as a
    /// `resolutions` section's alternatives and for the same reason, and kept in
    /// a set of its own because a version is not a category.
    Version(String),
    /// The panel's own search field was typed in.
    ///
    /// Not a request: it filters the section's rows, exactly as `query` does in
    /// `SearchSidebarFilter.vue`, and the version list behind it does not move.
    VersionQuery(String),
    /// The section's *Show all versions* box was pressed.
    ///
    /// Also not a request: the box is a `toggle_group`, and what it controls is
    /// whether a non-release version is a row at all.
    AllVersions(bool),
    /// The panel's own scroll region reported where it is.
    ///
    /// A geometry of its own rather than the page's: the version list is a second
    /// scroll region inside the sidebar panel, with its own 256-pixel window, and
    /// one geometry cannot be two offsets at once.
    VersionsScrolled(Geometry),
    /// A sidebar section was opened or closed.
    ///
    /// Not a request: opening a section changes what is on screen and not what
    /// the API is asked, which is the same rule the reference's `Accordion`
    /// follows. The id is the section's own -- a category header, or a filter id
    /// from [`SIDEBAR_FILTERS`] -- which is also what [`State::touched`] is keyed
    /// by.
    Section(String),
    /// The search was asked for again.
    Search,
    /// A section's *Show more* was pressed, or its *Show fewer*.
    ///
    /// Also not a request, and for a different reason than [`Message::Section`]:
    /// the options a press reveals are options the reader can then choose, and
    /// choosing one of those is what asks. The id is the section's own, as it is
    /// for every other press in the panel.
    Expand(String),
    /// A combobox's trigger was pressed: open its panel, or shut it again.
    ///
    /// `Combobox.vue:449`'s `handleTriggerClick`, which does exactly these two
    /// things and nothing else -- the panel is not the page's state to guess at,
    /// it is this message and the [`Menu`] it names.
    Toggle(Menu),
    /// One result was opened. Reported rather than applied: which page is in the
    /// pane is the shell's business, so this comes back out of
    /// [`crate::pages::Screen::update`] as an [`crate::pages::Open`].
    Open(String),
    /// The answer to a request, from the shell.
    ///
    /// The round is what makes a slow answer harmless: a page that has asked
    /// again in the meantime has moved past this one, and putting the older
    /// search's results under the newer search's controls is the kind of wrong a
    /// reader cannot even see.
    Found {
        /// Which request this answers, as [`Asked::round`] numbered it.
        round: u64,
        /// The hits, or the reason there are none.
        result: Result<Vec<Hit>, String>,
    },
    /// The tag lists, from the shell.
    ///
    /// The answer to [`crate::pages::Ask::Tags`], and the one thing the sidebar
    /// cannot draw a single option without. A store with no engine answers with a
    /// sentence, which is a section that knows it has nothing rather than one
    /// that drew itself empty.
    Tags {
        /// The three lists, or the reason there are none.
        result: Result<palantir_net::Tags, String>,
    },
/// The icons of the results on screen, from the shell.
    ///
    /// No round, unlike [`Message::Found`], and that is the type of the answer
    /// rather than an omission: an icon is keyed by the URL it was fetched from, so
    /// an icon from a search the reader has left is one the page can only look *up*,
    /// never draw under another project. What bounds the page's map is the results
    /// that arrive ([`State::icons_ask`]), not this message.
    Icons {
        /// The ones that arrived. An icon that could not be fetched or decoded is
        /// simply absent, and its box draws empty (`crate::ui::icon_box`).
        arrived: Vec<Fetched>,
    },
    /// The pointer entered or left one of the page's controls.
    ///
    /// A hover is a message rather than something a stylesheet reads off the
    /// pointer, because the tween it starts has to begin *before* the frame that
    /// draws it (see [`crate::ui`]). The page records the crossing and draws
    /// from the clock, so nothing here holds the pointer's state itself.
    Hover {
        /// The control's stable name, one per control.
        key: &'static str,
        /// Whether the pointer arrived or left.
        over: bool,
        /// The hover end, where the control declares one of its own.
        hover: Option<f32>,
    },

    /// The page's scroll region reported where it is.
    ///
    /// The only thing that moves the window ([`scroll::window`]): a wheel, a
    /// scrollbar drag and a keyboard scroll all report through it, and the frame
    /// built from the report draws the cards that report puts on screen.
    Scrolled(Geometry),

    /// A wheel over this page's scroll region.
    ///
    /// Reported rather than applied: iced moves a scrollable with a `scroll_to`
    /// command, so which region glides, and how far, is the shell's -- see
    /// `crate::scroll`. This page's part is to hand the wheel on, and the name it
    /// carries is the region the widget was built with.
    Wheel(&'static str, crate::scroll::Wheel),
}

/// A request the page has made, and which one it was.
///
/// Handed out by [`State::ask`] and [`State::opening`] rather than built by the
/// caller, because the round is the page's own bookkeeping: a caller that could
/// pick one could pick a stale one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    /// Which search this is, counting from one.
    pub round: u64,
    /// What was asked for, as the engine takes it.
    pub query: ApiSearch,
    /// Whether the sidebar's *Hide already installed* switch is on.
    ///
    /// Not a facet and not an id list: the reference's *hide installed* is a
    /// server-side filter (`Browse.vue`'s `instanceFilters` pushes
    /// `{ type: 'project_id', option: 'project_id:<id>', negative: true }` for
    /// every installed project, which `search.ts` renders as one
    /// `project_id NOT IN [...]` group), so a page that only dropped the rows
    /// it did not want would show a different *count* from the one the API
    /// counted. The page knows the switch; the ids are the shell's, so the
    /// switch travels with the request and the shell completes it.
    pub hide_installed: bool,
}

/// The icons of one page of results, to fetch.
///
/// A request of its own rather than part of [`Asked`], because it is asked for at a
/// different moment: the search has to land before the page knows which URLs it is
/// missing, and a card has to be drawable while they are still in flight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Icons {
    /// The `icon_url`s the page does not hold an icon for, in the order the
    /// results name them and each one once.
    pub urls: Vec<String>,
}

/// Discover's own state: what the user is asking for, and what came back.
#[derive(Debug, Clone)]
pub struct State {
    /// Which tab is showing.
    pub project_type: ProjectType,
    /// The query, as typed.
    pub query: String,
    /// The chosen order.
    pub sort: Sort,
    /// How many results to a page.
    pub view: usize,
    /// Which combobox panel is open, if any.
    ///
    /// The whole of [`Menu`]'s state: the panel's own contents are derived from
    /// it every frame, so there is nothing else to keep in step.
    pub menu: Option<Menu>,
    /// The current page, one-based.
    pub page: usize,
    /// Whether the sidebar's *Hide already installed* switch is on.
    ///
    /// The reference's `hideInstalledModpacks`, kept in the same place: it is a
    /// feature flag of the settings store, and Discover opens on whatever that
    /// store says. It travels with the request ([`Asked::hide_installed`]) rather
    /// than filtering the answer here.
    pub hide_installed: bool,
    /// The categories chosen in the sidebar, by name.
    ///
    /// A set rather than one choice per section because that is the shape the
    /// reference keeps: `currentFilters` is a flat list, and choosing a category is
    /// adding to it.
    pub categories: BTreeSet<String>,
    /// The chosen options of the sections that are not categories, as the
    /// section's own id beside the option's.
    ///
    /// [`categories`](Self::categories) cannot hold them: a category's id is the
    /// value the request carries, while a loader's and an environment option's
    /// are carried under fields of their own, so the section has to travel with
    /// the choice or two of them are the same string meaning two things.
    pub filters: BTreeSet<(String, String)>,
    /// The options that are *excluded*, by the same key as [`filters`](Self::filters).
    ///
    /// A set of its own rather than a flag on [`filters`](Self::filters) because
    /// the two are the same question with opposite answers and never both: the
    /// reference's row shows one mark or the other, and the press that excludes
    /// an option unchooses it (`primaryAction === 'exclude'` makes the row's own
    /// press the exclusion once it is excluded). One option is in one set or the
    /// other, never both.
    pub excluded: BTreeSet<(String, String)>,
    /// The game versions chosen in the sidebar, by version string.
    ///
    /// Its own set because a version is not a category: the request carries it
    /// under `game_versions`, and it is an `'or'` option, so two of them are one
    /// list rather than two parts.
    pub versions: BTreeSet<String>,
    /// What has been typed into the game-version section's own search field.
    ///
    /// The panel's own `query`, not the page's: it filters the rows of one
    /// section and nothing else, which is what `isVisible`'s `matchesQuery` does.
    pub version_query: String,
    /// Whether the section's *Show all versions* box is ticked, which is what
    /// `toggledGroups` holds for the one toggle group the filter declares.
    ///
    /// False on arrival, because the box is unticked in the reference and a
    /// snapshot row is a row the reader has to ask for.
    pub all_versions: bool,
    /// Where the version panel's own scroll region is, as it last reported.
    versions_at: Geometry,
    /// The sidebar sections whose open state the reader has changed.
    ///
    /// One set for both kinds of default, because a default is a fact about the
    /// filter and not about the reader: the reference's `getFilterOpenByDefault`
    /// opens every category section plus `environment` and `license` and nothing
    /// else, so what a press changes is that default rather than the other way
    /// round. [`State::is_open`] is the whole rule.
    pub touched: BTreeSet<String>,
    /// The sections whose *Show more* has been pressed.
    ///
    /// Only the sections with `display: 'expandable'` have one, and what it holds
    /// is the reference's `showMore`: before the press such a section shows the
    /// filter's own `default_values` and nothing else, and after it the section
    /// shows every option it has. A separate set from [`touched`](Self::touched)
    /// because the two are different questions -- a section can be open and shut
    /// again over three options, and a section can be shut with every option in
    /// it chosen.
    pub expanded: BTreeSet<String>,
    /// The results, which arrive from the search API.
    pub results: Load<Vec<Hit>>,
    /// The tag lists, which is what the sidebar's filter options are made of.
    ///
    /// Asked before the first search and kept for the life of the page: they
    /// change when Modrinth ships a release, and re-asking on every tab change
    /// would be three requests per click for an answer that was the same all
    /// afternoon.
    pub tags: Load<palantir_net::Tags>,
    /// The icons of those results, decoded, by the `icon_url` each answers.
    ///
    /// A map rather than a field on the [`Hit`] it belongs to, and by the URL rather
    /// than by the project: the URL is the only name an answer carries, so an icon
    /// that arrived late -- or for a page the reader has left and come back to -- can
    /// always find the card it belongs to, and can never land on another one.
    icons: HashMap<String, Icon>,
    /// How many times this page has asked, which is how an answer is told apart
    /// from an answer to a question it has since replaced.
    round: u64,
    /// Where the page's scroll region is, as it last reported.
    ///
    /// Defaulted rather than measured, because a region nobody has scrolled has
    /// never reported: the first frame is drawn inside the window-sized guess
    /// ([`scroll::INITIAL_VIEW`]), and the first event replaces it with the
    /// truth. The *list's* own geometry is this less the head above it, which is
    /// [`list_at`]'s whole job.
    geometry: Geometry,
}

impl State {
    /// A page opened on one project type.
    pub fn new(project_type: ProjectType) -> State {
        State {
            project_type,
            query: String::new(),
            sort: Sort::default(),
            view: DEFAULT_VIEW,
            menu: None,
            page: 1,
            hide_installed: false,
            categories: BTreeSet::new(),
            filters: BTreeSet::new(),
            excluded: BTreeSet::new(),
            versions: BTreeSet::new(),
            version_query: String::new(),
            all_versions: false,
            versions_at: Geometry::default(),
            touched: BTreeSet::new(),
            expanded: BTreeSet::new(),
            // Not `Empty`: nothing has been asked for yet, and an empty *answer*
            // and an unmade *request* are different sentences on the screen. The
            // shell asks for this page as soon as it draws it (`Screen::opening`),
            // so `Idle` is a state that lasts one message rather than a page that
            // sits there.
            results: Load::Idle,
            tags: Load::Idle,
            icons: HashMap::new(),
            round: 0,
            geometry: Geometry::default(),
        }
    }

    /// Apply a message, and report the request it asked for.
    ///
    /// One message in, at most one request out: a page cannot ask twice in a turn,
    /// and the shell cannot be handed a request it has already run.
    pub fn update(&mut self, message: Message) -> Option<Ask> {
        // `Combobox.vue` closes its panel on a click outside it -- the
        // `onClickOutside` at the end of that file -- and this kit has no such
        // event. What it does have is one place every message the page is sent
        // passes through, which is this function, so the panel is shut by every
        // message that is not its own trigger, a pointer crossing, a scroll (a
        // browser closes nothing by scrolling past it) or an answer that merely
        // landed while it was open. What is left is every press and every key
        // the reader makes, which is what the click-outside was for.
        if !matches!(
            message,
            Message::Toggle(..)
                | Message::Hover { .. }
                | Message::Wheel(..)
                | Message::Scrolled(..)
                | Message::VersionsScrolled(..)
                | Message::Found { .. }
                | Message::Tags { .. }
                | Message::Icons { .. }
        ) {
            self.menu = None;
        }
        match message {
            // A wheel is not this page's to apply: see `crate::scroll`.
            Message::Wheel(..) => {},

            // The panel is shut by the rule above when anything else is pressed,
            // so this is only the trigger's own two answers: open, or shut the
            // one that is open. Pressing the same trigger twice is how a reader
            // changes their mind without choosing anything.
            Message::Toggle(menu) => {
                self.menu = if self.menu == Some(menu) { None } else { Some(menu) };
            }

            // Nothing to do with it here: where the region is *is* the page's
            // state, and the next frame ([`view`]) is the one that uses it.
            Message::Scrolled(at) => self.geometry = at,
            Message::VersionsScrolled(at) => self.versions_at = at,

            Message::ProjectType(project_type) => {
                if self.project_type != project_type {
                    self.project_type = project_type;
                    // A different tab is a different search, and the reference
                    // resets the page rather than carrying it across. The results
                    // go back to `Idle` rather than staying: what is on screen
                    // belongs to the tab the user has just left. The shell asks
                    // for the new tab on its way out of this message, which is
                    // what `opening` is.
                    self.page = 1;
                    self.results = Load::Idle;
                }
            }
            // Every control that changes the request re-asks on the turn it
            // changes, which is what `use-browse-search.ts` does: it watches the
            // request's parameters (`query`, `maxResults`, the sort, the page) and
            // refreshes on a 200 ms debounce, so in the reference the search
            // follows the controls without a button to apply them. The row here
            // used to carry that button; with it gone, a control that changes and
            // does not ask would leave the search field typing at nothing. A page
            // can report a request but not schedule one -- the shell runs the
            // command, and there is no timer on this side of the seam -- so there
            // is no debounce to rest on and each change asks on its own turn.
            // [`Message::Found`]'s round is what keeps that honest: only the
            // newest answer is drawn, and the rest are dropped.
            Message::Query(query) => {
                if self.query != query {
                    self.query = query;
                    // Typing starts a new search, so the page goes back to the
                    // first.
                    self.page = 1;
                    return Some(Ask::Search(self.ask()));
                }
            }
            Message::Sort(sort) => {
                if self.sort != sort {
                    self.sort = sort;
                    self.page = 1;
                    return Some(Ask::Search(self.ask()));
                }
            }
            Message::View(view) => {
                if self.view != view {
                    self.view = view;
                    return Some(Ask::Search(self.ask()));
                }
            }
            Message::Page(page) => {
                let page = page.max(1);
                if self.page != page {
                    self.page = page;
                    return Some(Ask::Search(self.ask()));
                }
            }
            // A chosen category narrows the request, so it asks on the turn it
            // changes and starts the first page over, for the same reason the
            // sort does.
            Message::Category(name) => {
                if !self.categories.remove(&name) {
                    self.categories.insert(name.clone());
                    // Choosing a category unexcludes it, and the other way round:
                    // the reference's row carries one mark, and the press that
                    // excludes what is chosen drops the choice.
                    let id = category_filter_id(self.project_type, self.header_of(&name));
                    self.excluded.remove(&(id, name.clone()));
                }
                self.page = 1;
                return Some(Ask::Search(self.ask()));
            }
            // The exclusion is the same question with the opposite answer, and it
            // asks the same way. `environment` is the one filter whose rows carry
            // no exclusion at all -- `supports: ['include']` -- so it has no press
            // to send this.
            Message::Exclude { filter, option } => {
                if !self.excluded.remove(&(filter.clone(), option.clone())) {
                    self.excluded.insert((filter.clone(), option.clone()));
                    self.filters.remove(&(filter.clone(), option.clone()));
                    if category_filter_id(self.project_type, &option) == filter {
                        self.categories.remove(&option);
                    }
                }
                self.page = 1;
                return Some(Ask::Search(self.ask()));
            }
            // The same question for a row that is not a category, and the same
            // answer: the choice narrows the request, so it asks on the turn it
            // changes and starts the first page over.
            Message::Filter { filter, option } => {
                if !self.filters.remove(&(filter.clone(), option.clone())) {
                    self.filters.insert((filter.clone(), option.clone()));
                    // And choosing drops the exclusion, as above.
                    self.excluded.remove(&(filter.clone(), option.clone()));
                }
                self.page = 1;
                return Some(Ask::Search(self.ask()));
            }
            // Opening a section is not a change to the request: the reference's
            // `Accordion` keeps its own state and the search does not move.
            Message::Section(section) => {
                if !self.touched.remove(&section) {
                    self.touched.insert(section);
                }
            }
            // The same question for a game version, and the same answer. An exclusion
            // cannot happen here -- `game_version` is `supports: ['include']` --
            // so the choice only ever lands in `versions`.
            Message::Version(version) => {
                if !self.versions.remove(&version) {
                    self.versions.insert(version);
                }
                self.page = 1;
                return Some(Ask::Search(self.ask()));
            }
            // The panel's own two controls are the ones `SearchSidebarFilter`
            // keeps in refs: the box decides whether a snapshot is a row, and the
            // field decides which rows are on screen. Neither moves the request,
            // so neither asks -- which is also why the versions chosen stay
            // chosen while the reader narrows the list they are chosen from.
            Message::VersionQuery(query) => self.version_query = query,
            Message::AllVersions(on) => self.all_versions = on,
            // *Show more* is `showMore`, a `ref(false)` the reference keeps per
            // section, and the one piece of a section's own state that is not its
            // open state: the content is already open when the press is reachable
            // (it is inside the content, which is `inert` while the section is
            // shut), and every option it reveals is an option that could have been
            // chosen from the start. So it is a set of the sections that have been
            // opened the long way, keyed by the section's own id like
            // [`State::touched`] is.
            Message::Expand(section) => {
                if !self.expanded.remove(&section) {
                    self.expanded.insert(section);
                }
            }
            // The switch is a request-shaped control, like the sort and the view
            // size above it: it changes what the API is asked, so it asks again on
            // the turn it changes and starts the first page over, because the page
            // the reader was on means nothing once the set of results is a
            // different one.
            Message::HideInstalled(on) => {
                if self.hide_installed != on {
                    self.hide_installed = on;
                    self.page = 1;
                    return Some(Ask::Search(self.ask()));
                }
            }
            // The explicit ask, for a caller that wants one without changing a
            // control: the flood above is kept honest by the round, and this is the
            // message the tests drive that with.
            Message::Search => return Some(Ask::Search(self.ask())),
            Message::Found { round, result } => {
                // An answer to a question this page has replaced is dropped. It
                // is not an error and not worth a notice: the user asked for
                // something newer and the newer answer is on its way.
                if round == self.round {
                    self.results = match result {
                        Ok(hits) if hits.is_empty() => Load::Empty,
                        Ok(hits) => Load::Ready(hits),
                        Err(reason) => Load::Failed(reason),
                    };
                    // And the pictures those results name, asked for on the same
                    // turn: the cards are drawable now and their icons arrive when
                    // they arrive.
                    return self.icons_ask();
                }
            }
            // The tag lists, kept. Nothing about them is a round: they are three
            // lists with no question in them, so there is nothing for a later
            // answer to be stale against.
            Message::Tags { result } => {
                self.tags = match result {
                    Ok(tags) => {
                        if tags.categories.is_empty() {
                            Load::Empty
                        } else {
                            Load::Ready(tags)
                        }
                    }
                    Err(reason) => Load::Failed(reason),
                };
            }
            // Decoration, and the page keeps it either way: what the map holds is
            // what the cards on screen look up, so an icon no card names is one
            // that is never drawn and is dropped when the next results land.
            Message::Icons { arrived } => {
                if let Load::Ready(hits) = &self.results {
                    let named: std::collections::HashSet<&str> = hits.iter()
                        .map(|hit| hit.icon_url.as_str()).collect();
                    for fetched in arrived {
                        // Old requests may finish after a newer search. Keep
                        // only pictures the current result set actually names.
                        if named.contains(fetched.url.as_str()) {
                            self.icons.insert(fetched.url, fetched.icon);
                        }
                    }
                }
            }
            // The crossing, recorded where the clock lives: the page draws the
            // tween from the clock and keeps no pointer state of its own.
            Message::Hover { key, over, hover } => crate::ui::pointer_with(
                key,
                over,
                hover.unwrap_or_else(crate::theme::hover_brightness),
            ),
            // The shell's to do, and not this page's: see the enum.
            Message::Open(_) => {}
        }
        None
    }

    /// The request this page owes because nothing has been asked for yet.
    ///
    /// `None` once it has asked, which is what keeps a page from asking on every
    /// message: the shell calls this after every message it handles and gets
    /// nothing for the ninety-nine out of a hundred that are not "this page was
    /// just built".
    pub fn opening(&mut self) -> Option<Ask> {
        // The tag lists first, then the search. `SearchSidebarFilter`'s options are
        // all read out of them, and the shell's `opening` hands back one request at
        // a time -- so the page asks in the order it can draw something in, and the
        // first results land a request after the sidebar is able to fill itself.
        // This is the reference's order too: `Browse.vue` fetches `get_categories`,
        // `get_loaders` and `get_game_versions` when it mounts.
        if self.tags == Load::Idle {
            self.tags = Load::Loading;
            return Some(Ask::Tags);
        }
        if self.results == Load::Idle {
            return Some(Ask::Search(self.ask()));
        }
        None
    }

    /// The icons the results on screen are missing, as a request, if any.
    ///
    /// Two things at once, and they belong together: what the page *keeps* is
    /// narrowed to the icons its results name, and what is still missing is
    /// described. The narrowing is what bounds the map -- a reader who searches all
    /// afternoon would otherwise hold a picture of every project they ever saw -- and
    /// it can never drop an icon it is about to ask for, because the two lists are
    /// read from the same results.
    ///
    /// Empty when there is nothing to ask for, which is the common case for a second
    /// search of the same tab: the results are the same projects, so their icons are
    /// already here.
    fn icons_ask(&mut self) -> Option<Ask> {
        let named: Vec<String> = match &self.results {
            Load::Ready(hits) => {
                let mut named: Vec<String> = Vec::new();
                for hit in hits {
                    if !hit.icon_url.is_empty() && !named.contains(&hit.icon_url) {
                        named.push(hit.icon_url.clone());
                    }
                }
                named
            }
            _ => Vec::new(),
        };
        self.icons.retain(|url, _| named.contains(url));
        let missing: Vec<String> = named
            .into_iter()
            .filter(|url| !self.icons.contains_key(url))
            .collect();
        if missing.is_empty() {
            None
        } else {
            Some(Ask::Icons(Icons { urls: missing }))
        }
    }

    /// Bump the round, mark the page as waiting, and describe the request.
    fn ask(&mut self) -> Asked {
        self.round += 1;
        self.results = Load::Loading;
        Asked {
            round: self.round,
            query: self.request(),
            hide_installed: self.hide_installed,
        }
    }

    /// The search this page's controls describe.
    ///
    /// The controls and the request are the same thing read twice: the tab is the
    /// project type, the sort is the API's index, the view size is the limit, and
    /// the page number is the offset. One place, so a control that changes cannot
    /// fail to change the request.
    pub fn request(&self) -> ApiSearch {
        let search = ApiSearch::new(self.query.trim())
            .of_type(self.project_type.token())
            .sorted_by(self.sort.token())
            .with_limit(self.view as u32)
            .from_row((self.page.saturating_sub(1) * self.view) as u32);
        match self.facet_filter() {
            Some(group) => search.with_facets(vec![group]),
            None => search,
        }
    }

    /// Every chosen filter, as the one facet string Modrinth is asked with.
    ///
    /// The order is `search.ts`'s own: the parts it pushes as it walks the chosen
    /// options first, then the `orGroups` it collected, then the environment
    /// group at the end. It does not change what is asked, but it does decide
    /// whether the same set of choices is the same *string*, which is what keeps
    /// one cache entry per question rather than one per order of pressing.
    fn facet_filter(&self) -> Option<String> {
        let mut parts: Vec<String> = self.category_parts();
        // `license`'s only option is an `'and'` option (`method: 'and'`, value
        // `open_source:true`), so it is a part of its own and two of them cannot
        // happen: `open_source = true`, unquoted, because
        // `formatSearchFilterValue` leaves a boolean alone.
        if self.chosen(LICENSE, OPEN_SOURCE) {
            parts.push("open_source = true".to_string());
        }
        // The loaders are `'or'` options and they are asked under `categories`,
        // the field their `value` names -- so a loader joins the alternatives of
        // a `resolutions` section in **one** group rather than becoming a second
        // part. `newFilters` keys `orGroups` by the field, not by the filter, so
        // 16x and fabric are one `categories IN ["16x", "fabric"]`: asking for a
        // mod that is both would find fewer than the reference does.
        let group: BTreeSet<&str> = self
            .categories
            .iter()
            .filter(|name| alternatives(self.header_of(name)))
            .map(|name| name.as_str())
            .chain(self.options_of(MOD_LOADER))
            .chain(self.options_of(MODPACK_LOADER))
            .chain(self.options_of(SHADER_LOADER))
            .collect();
        if let Some(part) = any_of("categories", &group.into_iter().collect::<Vec<&str>>()) {
            parts.push(part);
        }
        // `game_version`'s options are `method: 'or'` over the field
        // `game_versions`, so they are a second group of their own -- a different
        // field from `categories`, so `newFilters` starts a second entry in
        // `orGroups` rather than joining the versions into the categories list.
        let chosen_versions: Vec<&str> = self.versions.iter().map(|version| version.as_str()).collect();
        if let Some(part) = any_of("game_versions", &chosen_versions) {
            parts.push(part);
        }
        // Then the exclusions, which `newFilters` appends after the groups and
        // which are the one place it does not collapse a list of one: a single
        // excluded loader is `categories NOT IN ["fabric"]`, not `= `. The field
        // is the option's own value field -- `categories` for a category and a
        // loader, `open_source` for the license's one option.
        let mut none: Vec<&str> = self
            .excluded
            .iter()
            .filter(|(filter, _)| filter.as_str() != LICENSE && filter.as_str() != ADVANCED)
            .map(|(_, option)| option.as_str())
            .collect();
        none.sort_unstable();
        if let Some(part) = none_of("categories", &none) {
            parts.push(part);
        }
        if self.excluded.contains(&(LICENSE.to_string(), OPEN_SOURCE.to_string())) {
            parts.push("open_source NOT IN [true]".to_string());
        }
        // A disclosure's option is `method: 'or'` over `disclosure_types`, and
        // negative, so `newFilters` files it under `negativeByType` rather than
        // `orGroups` -- which is why an excluded disclosure is a `NOT IN` part of
        // its own field instead of joining the `categories` list above. Being the
        // one negative filter whose field nothing else uses, it cannot disturb
        // that list either way.
        let mut disclosures: Vec<&str> = self
            .excluded
            .iter()
            .filter(|(filter, _)| filter.as_str() == ADVANCED)
            .map(|(_, option)| option.as_str())
            .collect();
        disclosures.sort_unstable();
        if let Some(part) = none_of("disclosure_types", &disclosures) {
            parts.push(part);
        }
        parts.extend(self.environment_parts());
        (!parts.is_empty()).then(|| parts.join(" AND "))
    }

    /// Whether the option `option` of the section `filter` is chosen.
    pub fn chosen(&self, filter: &str, option: &str) -> bool {
        self.filters.iter().any(|(f, o)| f == filter && o == option)
    }

    /// Whether the option `option` of the section `filter` is excluded, which is
    /// what `isExcluded` is in `SearchSidebarFilter.vue`'s visibility test.
    pub fn is_excluded(&self, filter: &str, option: &str) -> bool {
        self.excluded.iter().any(|(f, o)| f == filter && o == option)
    }

    /// The chosen options of the section `filter`, in the order the set keeps
    /// them, which is the option id's own order.
    fn options_of(&self, filter: &str) -> Vec<&str> {
        self.filters
            .iter()
            .filter(|(f, _)| f == filter)
            .map(|(_, option)| option.as_str())
            .collect()
    }

    /// The environment part, or the parts, the chosen rows describe.
    ///
    /// `search.ts` does not read the environment option's `environment:client`
    /// value at all: it collects which of `client` and `server` are chosen and
    /// asks `getEnvironmentFilterGroups` for the list of values that satisfy the
    /// pair, which is a much longer list than the two options and is what makes
    /// *Client* mean "runs on a client, server optional" rather than one value.
    fn environment_parts(&self) -> Vec<String> {
        environment_groups(
            self.chosen(ENVIRONMENT, "client"),
            self.chosen(ENVIRONMENT, "server"),
        )
        .iter()
        .map(|group| match group.as_slice() {
            [one] => format!("environment = \"{one}\""),
            many => format!(
                "({})",
                many.iter()
                    .map(|value| format!("environment = \"{value}\""))
                    .collect::<Vec<String>>()
                    .join(" OR ")
            ),
        })
        .collect()
    }

    /// The chosen categories, as the reference's one `categories = "..."` part.
    ///
    /// `search.ts`'s `newFilters` pushes one part per chosen option and joins the
    /// parts with ` AND ' into a *single* facet string, because the strings inside
    /// one `facets` group are alternatives to Modrinth and two chosen categories
    /// have to both hold. Two chosen ones are therefore
    /// `categories = "a" AND categories = "b"`, and they are sorted so that the
    /// same choice is the same request whichever order the rows were pressed in --
    /// which is also what keeps the engine's cache one entry rather than two.
    ///
    /// **Not every section is that kind of choice.** An option's `method` comes
    /// from its header (`search.ts`: `method: category.header === 'resolutions' ?
    /// 'or' : 'and'`), and an `'or'` option is collected into `orGroups` instead of
    /// pushed as a part: one resolution or another is one answer to one question,
    /// so two of them are `categories IN ["1080p", "1440p"]` -- one part, not two
    /// joined with `AND`, which would ask for a pack that is both.
    fn category_parts(&self) -> Vec<String> {
        self.categories
            .iter()
            .filter(|name| !alternatives(self.header_of(name)))
            .map(|name| format!("categories = \"{name}\""))
            .collect()
    }

    /// The header a chosen category is filed under, for this tab.
    ///
    /// A name the tag document does not carry -- a category the API has since
    /// dropped, or a filter that came from somewhere else -- reads as the
    /// default: `'and'`, the method every header but one uses.
    fn header_of(&self, name: &str) -> &str {
        self.tags
            .ready()
            .and_then(|tags| {
                tags.categories.iter().find(|category| {
                    category.name == name && category.project_type == self.project_type.token()
                })
            })
            .map(|category| category.header.as_str())
            .unwrap_or("")
    }

    /// Whether the sidebar section `id` is open.
///
/// `sidebar.vue`'s `getFilterOpenByDefault` answers it for a section nobody has
/// pressed -- the app variant opens every category section, plus `environment`
/// and `license`, and nothing else -- and a press has it the other way. Both are
/// one comparison, which is what a press is, and which is why the state is the
/// sections that have been touched rather than the ones that are shut.
pub fn is_open(&self, id: &str, default_open: bool) -> bool {
    default_open != self.touched.contains(id)
}

/// The placeholder the search field shows.
    ///
    /// The reference's own message for it: *"Search {projectType}..."*, with the
    /// tab's name in the singular -- `sentence(1)`, which is the arm its
    /// `formatProjectTypeSentence(..., 1)` resolves to for a search of one kind.
    pub fn placeholder(&self) -> String {
        text_gen::browse_search_placeholder(self.project_type.sentence(1))
    }

    /// The label of the view control: *"20 results"* is not the reference's
    /// shape, so this is the number alone with the control's own prefix beside it.
    pub fn view_label(&self) -> String {
        self.view.to_string()
    }

    /// The page's own title, which is the tab's name.
    pub fn title(&self) -> &'static str {
        self.project_type.label()
    }
}

/// Draw the page.
pub fn view<'a>(theme: Gen, state: &'a State, _store: &'a Store) -> Element<'a, Message> {
    // The reference's own order, top to bottom: the tabs, the search field, the
    // controls row, the results. The split into a pinned strip above and a
    // scrolling region below is `pages::instance`'s, kept here for the reason it
    // is kept there: a page that scrolled as a whole could not report where its
    // list starts.
    column![pinned_tabs(theme, state), body(vec![
        // The search field and the controls row are one group, and the gap
        // between them is the page's own rather than the body column's.
        //
        // `browse-tab/layout.vue` is a fragment: the `<Input size="large">` at
        // `:147` and the `flex flex-wrap items-center gap-2` div at `:178` are
        // siblings, and whatever spaces them belongs to a wrapper this port does
        // not draw. The capture settles it. The reference's field ends on y=173
        // and its sort trigger starts on y=182 -- eight rows -- while the trigger
        // ends on y=217 and the results surface begins on y=230 -- twelve. So
        // there are two gaps, not one, and they differ by exactly the `mt-1` on
        // the results block (`layout.vue:258`, `class="search mt-1
        // [overflow-anchor:none]"`) sitting on top of the wrapper's eight.
        {
            let mut group = column![
                // `browse-tab/layout.vue`'s `<Input>` carries `size="large"`, and this is
                // the one search field in the tree that does: 48 pixels, `px-4`, a
                // `rounded-[14px]` frame, `bg-surface-4` inside a `border-surface-5`
                // hairline. A capture of both clients at 1280x720 agrees on every
                // number -- the reference's field runs y 126..173 and so does this
                // one, forty-eight rows of `h-12` where `ui::CONTROL`'s forty would
                // not have been.
                ui::input_sized(
                    theme,
                    ui::InputSize::Large,
                    &state.placeholder(),
                    &state.query,
                    Message::Query,
                ),
                controls(theme, state),
            ]
            .spacing(SEARCH_TO_CONTROLS)
            .width(Length::Fill);
            // The panel, when one of the two comboboxes has it open. Pushed only
            // then: an empty child would still cost the column's own spacing and
            // move the results down, and a closed page has no panel to show.
            if let Some(panel) = menu(theme, state) {
                group = group.push(panel);
            }
            group.into()
        },
        results(theme, state),
    ])]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// The pinned tab strip: the page's own inset, and the pill.
///
/// `browse-tab/layout.vue` puts `<NavTabs>` straight into the page with no
/// `pageNav`, so `NavTabs.vue:3` renders its outer element as `contents` and
/// what shows is the `nav` itself -- `w-fit rounded-full bg-bg-raised`, as wide
/// as its tabs and no wider. A capture of the reference agrees exactly: the
/// raised surface runs x 88..723, and at x 730 and x 900 -- past the last tab --
/// the page is already its own `(22, 24, 28)`. There is no band behind the pill
/// and no full-bleed anything, which is the correction this makes: an earlier
/// draft wrapped the strip in a `Length::Fill` container painted `--bg-raised`,
/// which drew the pill's own fill across the whole pane and made a 633-pixel pill
/// read as a 914-pixel band.
///
/// So the strip is the pill at the page's [`INSET`] from the left, the same
/// twenty-four the body below insets its own content by, and the pill's own
/// border and `card-shadow` ([`ui::tabs`]) are the only ink around it.
fn pinned_tabs<'a>(theme: Gen, state: &'a State) -> Element<'a, Message> {
    // The inset is *around* the pill rather than inside it: padding the pill
    // itself by [`INSET`] would put the page background that sits above it inside
    // the pill's own fill instead.
    column![
        Space::new(Length::Fill, STRIP_ABOVE),
        row![Space::with_width(INSET), tabs(theme, state)]
            .align_items(Alignment::Center)
            .width(Length::Fill),
        Space::new(Length::Fill, STRIP_UNDER)
    ]
    .width(Length::Fill)
    .into()
}

/// The page background above the pill, and the gap under it.
///
/// Measured on a 1280x720 capture of the reference at `/browse/modpack`, where
/// the head bar's own bottom hairline is at y=48, the pill's top border at y=72,
/// its bottom at y=117, and the search field's top hairline at y=126. This port's
/// pill is the reference's forty-six (`1 + 4 + 36 + 4 + 1`, borders included),
/// this gap is [`INSET`], and the eight after it are [`STRIP_UNDER`].
///
/// [`INSET`] and not one less, and the head's hairline is the whole reason.
/// This column begins on the hairline's own row rather than the row below it, so
/// the twenty-three rows that separate y=48 from y=72 are counted *from* y=48 and
/// the pill's top border lands on `48 + 24`. An earlier revision subtracted the
/// hairline here, which drew the pill on y=71 and put the field, the controls and
/// the results one row above the reference for as long as it stood.
///
/// The eight rows under the pill are its own `card-shadow` to fade into and
/// nothing else: the capture has the pill's shadow on y 118..120 and plain page
/// background from 121, and this draws no ink there at all.
const STRIP_ABOVE: f32 = INSET;
const STRIP_UNDER: f32 = 8.0;

/// The gap between the search field and the controls row, which is the page's own
/// and not the body column's [`GAP`].
///
/// Measured on a 1280x720 capture of the reference at `/browse/mod`, where the
/// field's bottom hairline is at y=173 and the sort trigger's top border at
/// y=182: eight rows, against the twelve [`GAP`] puts between everything else in
/// the column. The trigger's bottom is y=217 and the results surface begins at
/// y=230 -- twelve -- so the two joins differ, and the difference is exactly the
/// `mt-1` on the results block (`browse-tab/layout.vue:258`, `class="search mt-1
/// [overflow-anchor:none]"`) sitting on top of this eight. The template is a
/// fragment, so the eight belongs to a wrapper this port does not draw and the
/// capture is the only place it is written down.
const SEARCH_TO_CONTROLS: f32 = 8.0;

/// The page's body: the inset, the spacing, and the scroll region that reports
/// where it is.
///
/// [`crate::page::body`]'s own shape with one thing added, and the addition is
/// why it is written here rather than called: the region is built there and comes
/// back an `Element`, and `on_scroll` is the scrollable's own builder, so the one
/// site that can attach it is the one that makes the scrollable. The report is
/// the whole of the window's policy ([`cards`]): where the region is is what
/// decides which cards a frame builds.
fn body<'a>(blocks: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    let mut items = column![].spacing(GAP).width(Length::Fill);
    for block in blocks {
        items = items.push(block);
    }
    crate::scroll::region(
        crate::scroll::PAGE,
        container(items).width(Length::Fill).padding(Padding {
            // The top inset moved onto the pinned strip above, and so did the gap:
            // the reference's shadow ends on the row before its field begins, so
            // there is nothing between them to pad. Its horizontal inset stays
            // here, which is what puts the field and the results at the
            // reference's own x.
            top: 0.0,
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

/// Discover's own section of the right panel: `BrowseSidebar`'s first block.
///
/// `browse-tab/sidebar.vue`, in the app variant, is a column of sections and each
/// one is a `border-0 border-b-[1px] border-[--brand-gradient-border] p-4
/// last:border-b-0` block. The first is not a filter at all: `showHideInstalled`
/// puts one `<label class="flex cursor-pointer items-center justify-between gap-3
/// text-contrast font-medium">` in it, over a `Toggle small` -- which is
/// [`ui::switch`] at `Toggle.vue`'s own 48x24.
///
/// It is drawn where the reference draws it, which is not in this page's column:
/// `Browse.vue` ends with `<Teleport to="#sidebar-teleport-target">`, and `App.vue`
/// puts that target *between* the onboarding checklist and the panel's own
/// sections. [`crate::shell`]'s panel is where that ordering lives.
///
/// `showHideInstalled` is `projectType === 'modpack' || (isServerContext && !==
/// 'modpack') || !!instance` -- two of the three arms are contexts this shell has
/// no route into, so the one that is left is the modpack tab, and nothing is drawn
/// on the others rather than a switch that would hide nothing.
///
/// **What is under the switch is the category sections** -- one
/// `SearchSidebarFilter` per (project type, header) pair the tag list has
/// categories for, which is what `search.ts` builds its `FilterType` ids out of.
/// Every one of them is open when the page arrives, because the app variant's
/// `getFilterOpenByDefault` opens any id that starts with `category`.
///
/// **And then the filters that are not categories**, in the order `search.ts`
/// declares them and `sort`s them: [`SIDEBAR_FILTERS`] holds the three this port
/// can draw whole -- Environment, the modpack tab's Loader and License -- and
/// names the ones it cannot (see there).
pub fn sidebar<'a>(theme: Gen, state: &'a State) -> Element<'a, Message> {
    // The blocks the panel stacks, in the reference's order. The rule between
    // two of them is drawn once the list is known rather than after each block,
    // because it belongs to the block that *follows* it and the last block has
    // none -- see [`rule_under`].
    let mut blocks: Vec<Element<'a, Message>> = Vec::new();
    if state.project_type == ProjectType::Modpack {
        let label = text(locale::lookup(Key::AppBrowseHideInstalledModpacks)
            .unwrap_or_else(|| Key::AppBrowseHideInstalledModpacks.message())
            .to_string())
        .size(16.0)
        .font(medium())
        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST)));
        let line = row![]
            .spacing(12.0)
            .align_items(Alignment::Center)
            .width(Length::Fill)
            .push(label)
            .push(Space::with_width(Length::Fill))
            .push(ui::switch(
                theme,
                state.hide_installed,
                Message::HideInstalled(!state.hide_installed)
            ));
        blocks.push(
            container(line)
                .width(Length::Fill)
                .padding(Padding { top: 16.0, right: 16.0, bottom: 16.0, left: 16.0 })
                .into(),
        );
    }
    // The category sections, one per header the tag list has categories under for
    // this tab. `SearchSidebarFilter` opens every one of them by default in the
    // app variant (`getFilterOpenByDefault`: `filterId.startsWith('category')`),
    // which is why the state is a set of the ones the reader has *closed*.
    if let Some(tags) = state.tags.ready() {
        for header in tags.headers(state.project_type.token()) {
            let options = tags.categories_under(state.project_type.token(), header);
            if options.is_empty() {
                continue;
            }
            let rows = category_rows(state, header, &options);
            let open = state.is_open(header, true);
            blocks.push(section(
                theme,
                header,
                locale::category_header_label(header),
                &rows,
                open,
                None,
                None,
            ));
        }
        // And then the filters that are not categories, in `search.ts`'s order.
        for filter in filters_for(state.project_type) {
            let rows = filter_rows(state, filter, state.project_type, tags);
            if !draws(filter, &rows) {
                continue;
            }
            let open = state.is_open(filter.id, filter.opens());
            // The *Show more* press belongs to the section that declares it, and
            // only an `expandable` one does: `v-if="filterType.display ===
            // 'expandable'"` in `SearchSidebarFilter.vue`.
            let more = (filter.display == Display::Expandable)
                .then_some((filter.id, state.expanded.contains(filter.id)));
            // And a `scrollable` section brings its own content: the search
            // field, the toggle group's box and the panel that scrolls.
            let panel = (filter.display == Display::Scrollable)
                .then(|| versions(theme, state, tags));
            blocks.push(section(theme, filter.id, message(filter.label), &rows, open, more, panel));
        }
    }
    let count = blocks.len();
    let mut sections = column![].spacing(0.0).width(Length::Fill);
    for (index, block) in blocks.into_iter().enumerate() {
        sections = sections.push(block);
        if rule_under(index, count) {
            sections = sections.push(panel_rule(theme));
        }
    }
    sections.into()
}

/// The chevron a section header draws, which is one icon in two states.
///
/// `Accordion.vue`'s header is a `DropdownIcon` with `:class="{'rotate-180':
/// isOpen}"`, so the reference draws `dropdown.svg` shut and that same path
/// turned over open. A rotation is not a thing this port can ask of a glyph: an
/// icon is a canvas drawing one of the paths in the generated table, and the
/// table holds the set's own files, which do not include a turned-over copy of
/// this one. `chevron-up.svg` is the set's own upward chevron and stands in for
/// it. What that costs, in the reference's own twenty-four-pixel box: turned
/// over about its centre, `dropdown.svg` runs x 5..19 with its point at y 8,
/// where `chevron-up.svg` runs x 6..18 with its point at y 9 -- a pixel and a
/// half of width and one of height, against a chevron that pointed the wrong
/// way on every open section.
fn chevron(open: bool) -> Glyph {
    if open {
        Glyph::ChevronUp
    } else {
        Glyph::Dropdown
    }
}

/// Whether a filter's section is drawn at all.
///
/// A filter with no options has nothing to draw, which is what an empty row list
/// used to mean -- and it is why the game-version section was not on screen at
/// all: its options are not rows but the panel that draws them, so
/// [`filter_rows`] answers an empty list for it and the section was skipped as
/// though the tab had no game versions. A `scrollable` filter is drawn on its
/// panel's account; everything else is drawn on its rows'.
fn draws(filter: &Filter, rows: &[Row]) -> bool {
    !rows.is_empty() || filter.display == Display::Scrollable
}

/// Whether the block at `index` of a stack `blocks` long carries the panel's
/// rule under it.
///
/// `sidebar.vue`'s app-variant `filterClass` ends in `last:border-b-0`, so the
/// rule belongs to the block that *follows* a section and the last block in the
/// panel has none. The toggles block that opens the panel carries the same
/// `last:border-b-0` and the same `p-4` treatment as a filter section, which is
/// why it is one block of the same stack rather than something drawn beside it.
fn rule_under(index: usize, blocks: usize) -> bool {
    index + 1 < blocks
}

/// One category section: `SearchSidebarFilter` at the app variant's own sizes.
///
/// The section is an `Accordion` whose button is the sidebar's \`buttonClass\` --
/// \`flex flex-col gap-1 px-3 py-3 w-full hover:bg-button-bg\` -- around the header
/// row \`flex items-center gap-1 w-full text-contrast\`, which is a \`text-base\`
/// \`h3\` and a \`size-5\` \`DropdownIcon\` at \`ml-auto\` that turns over when the
/// section is open. The first section's button gets \`pt-4\` rather than \`py-3\`'s
/// twelve, which is what \`[&:first-child>button]:pt-4\` is.
///
/// The header slot's own class is \`text-base m-0\` *without* a weight, where the
/// web variant has \`font-semibold\` -- so the class list says the app leans on its
/// own heading weight and does not say what that weight is. The reference's own
/// capture settled it: at 1280x720 *Category* reads heavier than the
/// \`font-medium\` *Hide already installed* above it, so the semibold face is what
/// is drawn here. Where a class list and a capture disagree about a face the
/// capture is the authority, because that is what a pixel-matched port is for.
fn section<'a>(
    theme: Gen,
    id: &'a str,
    label: String,
    rows: &[Row],
    open: bool,
    more: Option<(&'a str, bool)>,
    panel: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let button = row![]
        .align_items(Alignment::Center)
        .width(Length::Fill)
        .push(
            text(label)
                .size(16.0)
                // `text-base` own line: twenty-four pixels, which is what puts the
                // section's options where the capture has them.
                .line_height(iced::Pixels(24.0))
                .font(semibold())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
        )
        .push(Space::with_width(Length::Fill))
        .push(icon::icon(
            chevron(open),
            SECTION_ICON,
            // `ml-auto size-5 transition-transform duration-300 shrink-0
            // text-contrast`: the header's own ink, not the section's, which is
            // what the first draft drew.
            theme_gen::ink(theme, INK_CONTRAST),
        ));
    let mut inner = column![].spacing(4.0).width(Length::Fill);
    if open {
        // A `scrollable` section's content is its panel rather than a column of
        // rows, which is the one thing that distinguishes the third of the
        // reference's three shapes.
        match panel {
            Some(panel) => inner = inner.push(panel),
            None => {
                for row in rows {
                    inner = inner.push(option_row(theme, row));
                }
                if let Some((filter, expanded)) = more {
                    inner = inner.push(show_more(theme, filter, expanded));
                }
            }
        }
    }
    let mut body = column![].spacing(8.0).width(Length::Fill).push(button);
    if open {
        // `mt-2 mb-3` on the content and `ml-2 mr-3` on the panel inside it -- eight on
        // the left and twelve on the right, which is what puts an option's own
        // `px-2` at the reference's measured x 1010 and its label at 1018.
        body = body.push(
            container(inner)
                .width(Length::Fill)
                .padding(Padding { top: 8.0, right: 12.0, bottom: 12.0, left: 8.0 }),
        );
    }
    mouse_area(
        container(body)
            .width(Length::Fill)
            .padding(Padding { top: 12.0, right: 12.0, bottom: 12.0, left: 12.0 }),
    )
    .on_press(Message::Section(id.to_string()))
    .into()
}

/// The *Show more* press under an `expandable` section's options.
///
/// The same button as an option row -- `rounded-xl px-2 py-1 text-sm
/// font-semibold` in `text-secondary`, lifting to `text-contrast` under the
/// pointer -- with a `h-4 w-4` `DropdownIcon` in front of the label instead of a
/// check, turned over while the list is out (`rotate-180` on `showMore`), and no
/// mark at the end.
fn show_more<'a>(theme: Gen, filter: &str, expanded: bool) -> Element<'a, Message> {
    let key = crate::ui::scoped("discover:show-more", filter);
    let (_, hover) = crate::ui::interaction(key);
    let label = if expanded {
        Key::SearchFilterOptionShowFewer
    } else {
        Key::SearchFilterOptionShowMore
    };
    let ink = if hover > 0.0 { INK_CONTRAST } else { INK_SECONDARY };
    let line = row![]
        .align_items(Alignment::Center)
        .width(Length::Fill)
        .push(icon::icon(chevron(expanded), OPTION_CHECK, theme_gen::ink(theme, ink)))
        .push(text(message(label)).size(14.0).line_height(iced::Pixels(20.0)).font(semibold()).style(
            iced::theme::Text::Color(theme_gen::ink(theme, ink)),
        ));
    mouse_area(container(line).width(Length::Fill).padding(Padding {
        top: 4.0,
        right: 8.0,
        bottom: 4.0,
        left: 8.0,
    }))
    .on_press(Message::Expand(filter.to_string()))
    .on_enter(Message::hover_with(key, true, 1.0))
    .on_exit(Message::hover_with(key, false, 1.0))
    .into()
}

/// The game-version section's inside: the box, the search field, and the panel.
///
/// `SearchSidebarFilter`'s `scrollable` branch, which is the only one of the
/// three that draws this: a `Checkbox` per `toggle_groups` entry above a
/// `ScrollablePanel` of `h-[16rem]`, with a search `Input` between them
/// (`mx-2 my-1 w-[calc(100%-1rem)]`, `size="small"`, a search icon and a clear
/// button). Each option in a scrollable section also carries `mr-3`, which is the
/// room the panel's own scrollbar takes.
fn versions<'a>(
    theme: Gen,
    state: &'a State,
    tags: &'a palantir_net::Tags,
) -> Element<'a, Message> {
    // `isVisible`, in the reference's own order: the box first, the query second.
    // `toggle_group` is `version_type !== 'release' ? 'all_versions' : undefined`,
    // so without the box ticked the panel lists the releases and nothing else.
    let query = state.version_query.to_lowercase();
    let rows: Vec<Row> = tags
        .game_versions
        .iter()
        .filter(|version| state.all_versions || version.version_type == "release")
        .filter(|version| query.is_empty() || version.version.to_lowercase().contains(&query))
        .map(|version| Row {
            id: version.version.clone(),
            label: version.version.clone(),
            chosen: state.versions.contains(&version.version),
            excluded: false,
            // `supports: ['include']`, so there is no second press on a version.
            excludes: false,
            press: Message::Version(version.version.clone()),
            exclude: Message::Version(version.version.clone()),
        })
        .collect();

    let drawn = crate::scroll::window(rows.len(), VERSION_ROW, list_at(state.versions_at));
    let mut list = column![].spacing(4.0).width(Length::Fill);
    if drawn.start > 0 {
        list = list.push(space(drawn.start, VERSION_ROW));
    }
    for row in rows.iter().skip(drawn.start).take(drawn.len()) {
        list = list.push(option_row(theme, row));
    }
    if drawn.end < rows.len() {
        list = list.push(space(rows.len() - drawn.end, VERSION_ROW));
    }

    let box_row = row![]
        .align_items(Alignment::Center)
        .spacing(8.0)
        .width(Length::Fill)
        .push(ui::checkbox(
            theme,
            crate::ui::scoped("discover:all-versions", GAME_VERSION),
            state.all_versions,
            false,
            Message::AllVersions(!state.all_versions),
        ))
        .push(
            text(message(Key::SearchFilterTypeGameVersionAllVersions))
                .size(14.0)
                .line_height(iced::Pixels(20.0))
                .font(medium())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
        );
    crate::scroll::region(
        "discover:versions",
        column![]
            .spacing(8.0)
            .width(Length::Fill)
            .push(container(box_row).padding(Padding { top: 0.0, right: 8.0, bottom: 0.0, left: 8.0 }))
            .push(ui::input_sized(
                theme,
                ui::InputSize::Small,
                &message(Key::SearchFilterOptionSearchPlaceholder),
                &state.version_query,
                Message::VersionQuery,
            ))
            .push(
                container(list)
                    .width(Length::Fill)
                    .height(Length::Fixed(VERSIONS_PANEL))
                    // `mr-3` on every option in a scrollable section: the room
                    // the panel's own scrollbar takes.
                    .padding(Padding { top: 0.0, right: 12.0, bottom: 0.0, left: 0.0 }),
            ),
        Message::Wheel,
    )
    .on_scroll(|at| Message::VersionsScrolled(Geometry::of(at)))
    .into()
}

/// The height of the version panel: `h-[16rem]`, which is the only number in
/// this section that is not a class on a row.
const VERSIONS_PANEL: f32 = 256.0;
/// One option row's pitch inside the panel: the row's own height -- `px-2 py-1`
/// around a twenty-pixel line -- plus the `gap-1` the panel's column sets.
const VERSION_ROW: f32 = 32.0;

/// One row of a section, whichever section it is in.
///
/// The reference draws every option with the same component
/// (`SearchFilterOption.vue`) and only what the press *means* differs: a category
/// row adds a `categories = ...` part, an environment row an `environment` part,
/// a loader row another `categories` value. Carrying the message in the row is
/// what lets one drawing serve both kinds without either of them knowing about
/// the other.
#[derive(Clone)]
struct Row {
    /// The option's own id, as the reference's `FilterOption.id`.
    ///
    /// Not decoration: it is the key the section is filed under, what the hover
    /// is named after, and -- for every filter that is not a category -- the
    /// value the request carries.
    id: String,
    /// The label the option's `formatted_name` gives it.
    label: String,
    /// Whether it is in the request already, which is what decides its ink, its
    /// fill and whether its check is drawn at all.
    chosen: bool,
    /// Whether it is excluded instead, which swaps the check for a ban and the
    /// brand highlight for the red one.
    excluded: bool,
    /// Whether the section's filter declares `supports: ['include', 'exclude']`,
    /// which is what puts the second press at the row's end. `environment` is
    /// `['include']` and has none.
    excludes: bool,
    /// The message that chooses it, or unchooses it.
    press: Message,
    /// The message that excludes it, or unexcludes it.
    exclude: Message,
}

/// The id `search.ts` gives a category section's filter, which is what an
/// exclusion of one of its rows is keyed by.
///
/// `` `category_${category.project_type}_${category.header}` `` -- the header, not
/// the project's own spelling of it, so the mod tab's *technical* section is
/// `category_mod_technical` on every project type that has one.
fn category_filter_id(kind: ProjectType, header: &str) -> String {
    format!("category_{}_{}", kind.token(), header)
}

/// The category rows of one section, in the order the API lists them.
///
/// A category section is `supports: ['include', 'exclude']` like the loaders, so
/// every row carries the second press -- keyed by the section's own filter id,
/// which is what `search.ts` would have in `currentFilters`.
fn category_rows(state: &State, header: &str, options: &[&palantir_net::CategoryTag]) -> Vec<Row> {
    let filter = category_filter_id(state.project_type, header);
    options
        .iter()
        .map(|category| Row {
            id: category.name.clone(),
            label: locale::category_label(&category.name),
            chosen: state.categories.contains(&category.name),
            excluded: state.excluded.contains(&(filter.clone(), category.name.clone())),
            excludes: true,
            press: Message::Category(category.name.clone()),
            exclude: Message::Exclude {
                filter: filter.clone(),
                option: category.name.clone(),
            },
        })
        .collect()
}

/// One of the sidebar's filters that is not a category.
///
/// `search.ts` builds these in one array, and the fields carried here are the
/// ones that decide what the panel draws: which tabs the section is on
/// (`supported_project_types`), how much of it there is (`display`), where it
/// sits among the others (`ordering`), and whether it is open when the page
/// arrives (the app variant's `getFilterOpenByDefault`).
struct Filter {
    /// `search.ts`'s `FilterType.id`, which is also what the request is keyed by.
    id: &'static str,
    /// The header's own message, `search.filter_type.*`.
    label: Key,
    /// `supported_project_types`: the kinds of project it is offered for.
    kinds: &'static [ProjectType],
    /// `display`: how many of the options the section shows, and whether it
    /// carries the *Show more* press that shows the rest.
    display: Display,
    /// `default_values`: the options an `expandable` section shows before that
    /// press, and the ones it sorts to the top of the list afterwards.
    ///
    /// Empty for an `All` filter, where the reference never reads them: the sort
    /// that puts them first is guarded on `display === 'expandable'`, so a
    /// `default_values` on an `all` filter is not drawn.
    defaults: &'static [&'static str],
}

/// `search.ts`'s `display`, which is the whole of what decides how many rows a
/// section draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Display {
    /// `'all'`: every option, always, and no press under the list.
    All,
    /// `'expandable'`: the filter's `default_values` and whatever is already
    /// chosen, with a *Show more* press that swaps in the whole list.
    Expandable,
    /// `'scrollable'`: the whole option list, inside a panel of its own that
    /// scrolls, with a search field on top.
    Scrollable,
}

/// The filters the sidebar offers besides its categories, in `search.ts`'s own
/// declaration order -- which is the order they are drawn in unless `ordering`
/// says otherwise (see [`Filter::ordering`]).
///
/// **Not here:** `game_version`, whose `display` is `scrollable` and which is
/// `searchable` -- a 16rem scroll panel with a search field on top, and this
/// page has no second scroll region to put the panel in ([`Message::Scrolled`]
/// carries one geometry and the results list has it); `included_content`, which
/// is a project picker rather than a list; `plugin_loader`, which is
/// `supported_project_types: ['plugin']` and this port has no plugin tab; and
/// the `advanced` exclusions, whose options are disclosure toggles. Each is
/// named here so that the next reader does not have to go back to `search.ts` to
/// learn they were considered.
const SIDEBAR_FILTERS: &[Filter] = &[
    Filter {
        id: GAME_VERSION,
        label: Key::SearchFilterTypeGameVersion,
        kinds: &[
            ProjectType::Mod,
            ProjectType::Modpack,
            ProjectType::ResourcePack,
            ProjectType::Shader,
            ProjectType::Plugin,
            ProjectType::Datapack,
        ],
        display: Display::Scrollable,
        defaults: &[],
    },
    Filter {
        id: ENVIRONMENT,
        label: Key::SearchFilterTypeEnvironment,
        kinds: &[ProjectType::Mod, ProjectType::Modpack],
        display: Display::All,
        defaults: &[],
    },
    Filter {
        id: MOD_LOADER,
        label: Key::SearchFilterTypeModLoader,
        kinds: &[ProjectType::Mod],
        display: Display::Expandable,
        defaults: DEFAULT_MOD_LOADERS,
    },
    Filter {
        id: MODPACK_LOADER,
        label: Key::SearchFilterTypeModpackLoader,
        kinds: &[ProjectType::Modpack],
        // `display: 'all'`, and the only loader filter that is: a modpack's
        // loaders are three or four rows, so the reference draws all of them and
        // the section carries no press under the list.
        display: Display::All,
        defaults: &[],
    },
    Filter {
        id: SHADER_LOADER,
        label: Key::SearchFilterTypeShaderLoader,
        kinds: &[ProjectType::Shader],
        display: Display::Expandable,
        defaults: DEFAULT_SHADER_LOADERS,
    },
    Filter {
        id: ADVANCED,
        label: Key::SearchFilterTypeAdvanced,
        // `supported_project_types: ALL_PROJECT_TYPES` -- the whole tab list, so
        // this section is on every tab and is never narrowed the way a category
        // section is.
        kinds: ALL_TABS,
        display: Display::All,
        defaults: &[],
    },
    Filter {
        id: LICENSE,
        label: Key::SearchFilterTypeLicense,
        kinds: &[
            ProjectType::Mod,
            ProjectType::Modpack,
            ProjectType::ResourcePack,
            ProjectType::Shader,
            ProjectType::Plugin,
            ProjectType::Datapack,
        ],
        display: Display::All,
        defaults: &[],
    },
];

/// `search.ts`'s `environment` filter id.
const ENVIRONMENT: &str = "environment";
/// `search.ts`'s `advanced` filter id -- *Advanced exclusions*, and the only
/// filter in the table whose rows are exclude-only.
const ADVANCED: &str = "advanced";
/// `search.ts`'s `game_version` filter id -- the *Game version* section, and the
/// one filter in this table that is neither a list of rows nor an expandable
/// one: `display: 'scrollable'`, `searchable: true`, every game version Modrinth
/// publishes in a 256-pixel panel with a search field above it.
const GAME_VERSION: &str = "game_version";
/// `search.ts`'s `mod_loader` filter id -- the *Loader* section of the mod tab.
const MOD_LOADER: &str = "mod_loader";
/// `search.ts`'s `modpack_loader` filter id -- the *Loader* section of the tab
/// whose loaders are offered for modpacks. It is the one loader filter the
/// reference declares `display: 'all'` for, so it is the only one of the three
/// that draws its whole list and carries no *Show more*.
const MODPACK_LOADER: &str = "modpack_loader";
/// `search.ts`'s `shader_loader` filter id -- the *Loader* section of the
/// shader tab.
const SHADER_LOADER: &str = "shader_loader";
/// `search.ts`'s `license` filter id.
const LICENSE: &str = "license";
/// The one option of the `license` filter, as `search.ts` spells it.
const OPEN_SOURCE: &str = "open_source";

/// The disclosures `advanced` offers, in `PROJECT_DISCLOSURE_TYPES`' own order:
/// each one's id, its label, and the project types it applies to
/// (`DISCLOSURE_SUPPORTED_PROJECT_TYPES`).
///
/// **`derivative_work` is not here** because
/// `createDisclosureFilterOptions` drops it before the list is built, and the
/// project-type rows beside them (`all_project_types:...`) are here but not drawn:
/// `newFilters` skips every `advanced` option that `isProjectTypeExclusionOption`
/// recognises, so the reference draws rows that change nothing.
///
/// Two of the disclosures carry `sub_options` in the reference -- *AI-generated
/// content* splits by AI usage and *Telemetry* by consent -- behind a third press
/// on the row. Those are not drawn: the sub-options are another `or` group under
/// the same `disclosure_types` field, and the row's third press is a disclosure
/// chevron that has no place in a row this port draws with two presses at most.
const DISCLOSURES: &[(&str, Key, &[ProjectType])] = &[
    ("ai_content", Key::SearchFilterTypeAdvancedDisclosureAiContent, ALL_TABS),
    ("ai_functionality", Key::SearchFilterTypeAdvancedDisclosureAiFunctionality, ALL_TABS),
    ("advertisements", Key::SearchFilterTypeAdvancedDisclosureAdvertisements, ALL_TABS),
    ("epilepsy_triggers", Key::SearchFilterTypeAdvancedDisclosureEpilepsyTriggers, ALL_TABS),
    // `['mod', 'plugin', 'modpack']`.
    ("system_interactions", Key::SearchFilterTypeAdvancedDisclosureSystemInteractions, &[
        ProjectType::Mod,
        ProjectType::Plugin,
        ProjectType::Modpack,
    ]),
    // `['mod', 'plugin', 'modpack', 'server']`.
    ("telemetry", Key::SearchFilterTypeAdvancedDisclosureTelemetry, &[
        ProjectType::Mod,
        ProjectType::Plugin,
        ProjectType::Modpack,
        ProjectType::Server,
    ]),
    ("paid_features", Key::SearchFilterTypeAdvancedDisclosurePaidFeatures, ALL_TABS),
    ("archived", Key::SearchFilterTypeAdvancedDisclosureArchived, ALL_TABS),
];

/// `ALL_CONTENT_PROJECT_TYPES`, which is what seven of the nine disclosures
/// support.
const ALL_TABS: &[ProjectType] = &[
    ProjectType::Mod,
    ProjectType::ResourcePack,
    ProjectType::Datapack,
    ProjectType::Shader,
    ProjectType::Modpack,
    ProjectType::Plugin,
    ProjectType::Server,
];

/// `DEFAULT_MOD_LOADERS` (`tag-messages.ts:575`): what the mod tab's *Loader*
/// shows before *Show more*.
///
/// Three names, and the set is what makes the section worth expanding at all:
/// Modrinth lists a dozen mod loaders, and `fabric`, `forge` and `neoforge` are
/// the ones the reference offers without being asked.
const DEFAULT_MOD_LOADERS: &[&str] = &["fabric", "forge", "neoforge"];
/// `DEFAULT_SHADER_LOADERS` (`tag-messages.ts:577`), the same for the shader
/// tab.
const DEFAULT_SHADER_LOADERS: &[&str] = &["iris", "optifine", "vanilla"];

impl Filter {
    /// `search.ts`'s `ordering` for this filter on `kind`.
    ///
    /// The reference reads it off the expression it builds the filter with, so
    /// it is a per-tab number where it is not zero: `game_version` is `2` on the
    /// mod tab and `-1` on the shader one, and `mod_loader` is `1` on the mod
    /// tab. Everything else is `undefined`, which the sort reads as zero, and
    /// which is the whole of the modpack tab's order -- its three sections keep
    /// the declaration order below.
    ///
    /// The `1` belongs to `mod_loader` and to nothing else. The first draft gave
    /// it to `modpack_loader`, which reads the same expression but sits on the
    /// modpack tab, where no ordering is set at all -- so it was a number on a
    /// filter the mod tab does not have, and the mod tab's own *Loader* sorted
    /// below *License* instead of above it.
    fn ordering(&self, kind: ProjectType) -> i32 {
        match self.id {
            // `game_version` is `2` on the mod tab and `-1` on the shader one, so
            // the shader tab's *Game version* sits below *License* while the mod
            // tab's sits above its *Loader*.
            GAME_VERSION if kind == ProjectType::Mod => 2,
            GAME_VERSION if kind == ProjectType::Shader => -1,
            MOD_LOADER if kind == ProjectType::Mod => 1,
            // `ordering: -1000`, the only negative number big enough to be about
            // the position rather than the tab: *Advanced exclusions* is always
            // the last section, on every tab.
            ADVANCED => -1000,
            _ => 0,
        }
    }

    /// Whether the app variant opens this section when the page arrives.
    ///
    /// The last two arms of `getFilterOpenByDefault`: the app opens an id that
    /// starts with `category`, plus `environment` and `license`, and nothing
    /// else. The loaders are not among them, which is why a modpack tab's
    /// *Loader* is a header until it is pressed.
    ///
    /// `advanced` is absent for a reason that is *not* that arm. It is settled
    /// by the arm above it, which tests the id before the `isApp` arm is ever
    /// reached (`browse-tab/sidebar.vue:103`): the answer is
    /// `!advancedFiltersCollapsed`, and `use-app-settings.ts:18` sets
    /// `advanced_filters_collapsed: true`. Collapsed is the default, so
    /// *Advanced exclusions* arrives shut like the loaders.
    fn opens(&self) -> bool {
        matches!(self.id, ENVIRONMENT | LICENSE)
    }
}

/// The sections the sidebar draws after its categories, in the order the
/// reference stacks them.
///
/// `sort((a, b) => (b.ordering ?? 0) - (a.ordering ?? 0))` over the declaration
/// order, so sorting by the negated key keeps it where the numbers tie -- which
/// is a stable sort, and what the reference's is as well.
fn filters_for(kind: ProjectType) -> Vec<&'static Filter> {
    let mut filters: Vec<&Filter> =
        SIDEBAR_FILTERS.iter().filter(|filter| filter.kinds.contains(&kind)).collect();
    filters.sort_by_key(|filter| -filter.ordering(kind));
    filters
}

/// The rows of `filter` on `kind`, in the reference's own order.
fn filter_rows(
    state: &State,
    filter: &Filter,
    kind: ProjectType,
    tags: &palantir_net::Tags,
) -> Vec<Row> {
    let press = |option: &str| Message::Filter {
        filter: filter.id.to_string(),
        option: option.to_string(),
    };
    match filter.id {
        ENVIRONMENT => vec![
            Row {
                id: "client".to_string(),
                label: message(Key::SearchFilterTypeEnvironmentClient),
                chosen: state.chosen(ENVIRONMENT, "client"),
                excluded: state.is_excluded(ENVIRONMENT, "client"),
                // `supports: ['include']` is the whole of the reference's answer
                // for this filter, so its rows have no second press at all.
                excludes: false,
                press: press("client"),
                exclude: Message::Filter { filter: ENVIRONMENT.to_string(), option: "client".to_string() },
            },
            Row {
                id: "server".to_string(),
                label: message(Key::SearchFilterTypeEnvironmentServer),
                chosen: state.chosen(ENVIRONMENT, "server"),
                excluded: state.is_excluded(ENVIRONMENT, "server"),
                excludes: false,
                press: press("server"),
                exclude: Message::Filter { filter: ENVIRONMENT.to_string(), option: "server".to_string() },
            },
        ],
        LICENSE => vec![Row {
            id: OPEN_SOURCE.to_string(),
            label: message(Key::SearchFilterTypeLicenseOpenSource),
            chosen: state.chosen(LICENSE, OPEN_SOURCE),
            excluded: state.is_excluded(LICENSE, OPEN_SOURCE),
            excludes: true,
            press: press(OPEN_SOURCE),
            exclude: Message::Exclude { filter: LICENSE.to_string(), option: OPEN_SOURCE.to_string() },
        }],
        // `game_version` has no rows here: its options go in the panel, which
        // draws them itself because it is the only section with a search field,
        // a toggle group and a window of its own on top of them.
        GAME_VERSION => Vec::new(),
        // The disclosures, in `PROJECT_DISCLOSURE_TYPES`' own order, minus the
        // one `createDisclosureFilterOptions` drops and minus the ones this tab
        // cannot have: `isDisclosureCompatibleWithProjectTypes` asks whether the
        // disclosure's supported project types and the tab's overlap, and only
        // *Telemetry* and *External system interactions* are narrower than "all".
        ADVANCED => DISCLOSURES
            .iter()
            .filter(|(_, _, kinds)| kinds.contains(&kind))
            .map(|(id, label, _)| Row {
                id: (*id).to_string(),
                label: message(*label),
                chosen: false,
                excluded: state.is_excluded(ADVANCED, id),
                // `supports: ['exclude']` and nothing else, so
                // `primaryAction === 'exclude'` from the first frame: there is no
                // second button on these rows at all, and the row's own press is
                // the exclusion.
                excludes: false,
                press: Message::Exclude {
                    filter: ADVANCED.to_string(),
                    option: (*id).to_string(),
                },
                exclude: Message::Exclude {
                    filter: ADVANCED.to_string(),
                    option: (*id).to_string(),
                },
            })
            .collect(),
        // Every loader filter is `tags.value.loaders` narrowed to the project
        // types the filter declares, which is what `Tags::loaders_for` answers,
        // with `formatLoader` for the label. The one narrowing it cannot answer
        // is `mod_loader`'s, which drops the loaders that are also plugins or
        // datapacks -- see [`loaders_of`].
        id @ (MOD_LOADER | MODPACK_LOADER | SHADER_LOADER) => {
            let chosen: Vec<Row> = loaders_of(id, kind, tags)
                .into_iter()
                .map(|loader| Row {
                    id: loader.clone(),
                    label: locale::loader_label(&loader),
                    chosen: state.chosen(filter.id, &loader),
                    excluded: state.is_excluded(filter.id, &loader),
                    excludes: true,
                    press: press(&loader),
                    exclude: Message::Exclude {
                        filter: filter.id.to_string(),
                        option: loader.clone(),
                    },
                })
                .collect();
            visible_rows(filter, state, chosen)
        }
        // Every id in [`SIDEBAR_FILTERS`] has an arm above; this keeps a filter
        // added to the table without its rows from drawing an empty section.
        _ => Vec::new(),
    }
}

/// The loaders `filter` lists, in the order the tag document has them.
///
/// `Tags::loaders_for` answers the whole of each filter's narrowing except one:
/// `mod_loader`'s options are the loaders that support `mod` **and neither**
/// `plugin` **nor** `datapack`, and a loader can be in both lists -- `paper` is
/// a plugin and a datapack loader, and a loader that also served mods would be
/// offered on the mod tab by the plain narrowing and not by the reference's.
/// The extra two clauses are here rather than in `palantir-net` because they are
/// this filter's arithmetic and not the tag document's.
fn loaders_of(
    filter: &str,
    kind: ProjectType,
    tags: &palantir_net::Tags,
) -> Vec<String> {
    tags.loaders_for(kind.token())
        .into_iter()
        .filter(|loader| {
            filter != MOD_LOADER
                || !loader
                    .supported_project_types
                    .iter()
                    .any(|project| project == "plugin" || project == "datapack")
        })
        .map(|loader| loader.name.clone())
        .collect()
}

/// The rows of a section that are `expandable`, as `visibleOptions` hands them
/// over.
///
/// The reference filters the options by `isVisible(option) || isIncluded(option)
/// || isExcluded(option) || hasSelectedSubOption(option)`, which for an
/// `expandable` filter reads: the option is shown when the section is opened the
/// long way, or when the filter lists it in `default_values`. The two `is*`
/// tests then keep a chosen option on screen even when nothing else would -- so
/// a loader the reader chose stays a row they can unchoose, which is the whole
/// point of a filter that hides its options. Then the list is sorted so the
/// `default_values` come first, which is a stable sort and so leaves the tag
/// document's own order inside each half of it.
fn visible_rows(filter: &Filter, state: &State, rows: Vec<Row>) -> Vec<Row> {
    if filter.display == Display::All {
        return rows;
    }
    let shown: Vec<Row> = if state.expanded.contains(filter.id) {
        rows
    } else {
        rows.iter()
            .filter(|row| filter.defaults.contains(&row.id.as_str()) || row.chosen || row.excluded)
            .cloned()
            .collect()
    };
    let (mut defaults, mut rest): (Vec<Row>, Vec<Row>) =
        shown.into_iter().partition(|row| filter.defaults.contains(&row.id.as_str()));
    defaults.append(&mut rest);
    defaults
}

/// The reference's own message for `key`, as the panel shows it.
fn message(key: Key) -> String {
    locale::lookup(key).unwrap_or_else(|| key.message()).to_string()
}

/// One option row: `SearchFilterOption`'s own button.
///
/// \`flex ... rounded-xl px-2 py-1 text-sm font-semibold\` around the label, with
/// the check at \`ml-auto\`. Chosen is \`bg-brand-highlight text-contrast\`;
/// unchosen is transparent with the label in \`text-secondary\` and a
/// \`bg-button-bg\` under the pointer.
///
/// **The check is sixteen pixels and it is not drawn at all until the row is
/// chosen or the pointer is on it.** \`SearchFilterOption.vue\` gives it \`h-4 w-4\`
/// (not the section's own \`size-5\`) and the class
/// \`transition-opacity group-hover:opacity-100\`, with the parent's
/// \`[@media(hover:hover)]:opacity-0\` on the unchosen arm -- so on a machine with a
/// pointer, which is the machine this port is measured on, an unchosen row shows
/// no mark at all. Drawing one anyway puts sixteen pixels of ink where the
/// reference has none, on every row of every open section.
///
/// **The category's own icon is not drawn.** \`getCategoryIcon\` hands out an SVG
/// per category from \`@modrinth/assets\`, and this launcher ships no icon set for
/// the three hundred tags Modrinth publishes; a row that had a box where a
/// drawing belongs would be worse than a row whose label starts eight pixels
/// further left.
/// The space an option gives the icon the reference draws in front of its label.
///
/// The reference's rows carry a sixteen-pixel drawing at x 998..1011 and their
/// labels start at 1021; this launcher draws no icon, so the space is here to put
/// the label at the reference's own x rather than to fill a gap. It is measured
/// off the plate rather than added up, which is the only honest way to set it: the
/// classes (\`px-2\` on the row, \`ml-2\` on the panel, \`gap-2\` between icon and
/// label) add up to a slightly different number than the pixels do.
const OPTION_ICON: f32 = 20.0;

/// The fill behind an option row, as the reference's three arms name them.
///
/// `SearchFilterOption.vue`'s button class is one expression with three arms:
/// `included ? 'bg-brand-highlight text-contrast' : excluded ?
/// 'bg-highlight-red text-contrast' : 'bg-transparent … hover:bg-button-bg'`. The
/// third arm is the one that was missing -- the row drew nothing under the
/// pointer, so a reader could not tell which row the press would land on except
/// by its label lifting to `text-contrast`.
///
/// The unchosen fill fades in with the hover rather than arriving with it, which
/// is what `crate::ui::switch` does with the same token: the tween's travel is
/// the clock, and the pointer's own fraction is how far through it the frame is.
fn row_fill(chosen: bool, excluded: bool, hover: f32) -> Option<(Ink, f32)> {
    if chosen {
        Some((Ink::ColorBrandHighlight, 1.0))
    } else if excluded {
        Some((Ink::RedHighlight, 1.0))
    } else if hover > 0.0 {
        Some((Ink::ButtonBg, hover.min(1.0)))
    } else {
        None
    }
}

fn option_row<'a>(theme: Gen, row: &Row) -> Element<'a, Message> {
    let key = crate::ui::scoped("discover:option", &row.id);
    let (_, hover) = crate::ui::interaction(key);
    // `hover:text-contrast` on the unchosen arm: the pointer lifts the label to
    // the ink a chosen row has, which is the only thing the hover changes here
    // -- the fill it also names (`hover:bg-button-bg`) is a background and this
    // row has none to tint, which the section's own comment says.
    let lifted = hover > 0.0;
    let ink = if row.chosen || lifted { INK_CONTRAST } else { INK_SECONDARY };
    let label = text(row.label.clone())
        .size(14.0)
        // `text-sm`'s own line, `1.25rem`: twenty pixels, and the reason the
        // reference's option rows are thirty-three pixels apart rather than the
        // twenty-nine a tighter line would give.
        .line_height(iced::Pixels(20.0))
        .font(semibold())
        .style(iced::theme::Text::Color(theme_gen::ink(theme, ink)));
    let check = if row.excluded {
        // `<BanIcon v-if="excluded || primaryAction === 'exclude'">`, and it
        // takes the button's own ink, which on the excluded arm is `text-contrast`
        // over `bg-highlight-red`.
        Some(icon::icon(Glyph::Ban, OPTION_CHECK, theme_gen::ink(theme, INK_CONTRAST)))
    } else if row.chosen {
        // The chosen arm is `bg-brand-highlight text-contrast`, so the check
        // inherits that ink -- not the section ink it was drawn in before.
        Some(icon::icon(Glyph::Check, OPTION_CHECK, theme_gen::ink(theme, INK_CONTRAST)))
    } else {
        lifted.then(|| {
            icon::icon(Glyph::Check, OPTION_CHECK, theme_gen::ink(theme, INK_SECONDARY))
        })
    };
    let mut line = row![]
        .align_items(Alignment::Center)
        .width(Length::Fill)
        .push(Space::with_width(OPTION_ICON))
        .push(label)
        .push(Space::with_width(Length::Fill));
    if let Some(check) = check {
        line = line.push(check);
    }
    // The second press, at the row's end: a one-pixel divider and a ban button
    // with their own `px-2 py-1` face, both of which the reference keeps at zero
    // opacity until the row is hovered (`[@media(hover:hover)]:opacity-0` and the
    // component's own `:hover button { opacity: 1 }`), and the divider is hidden
    // outright once the row is chosen (`{'opacity-0': included}`).
    if row.excludes && !row.excluded && lifted {
        line = line.push(
            container(Space::new(Length::Fill, OPTION_DIVIDER))
                .width(Length::Fixed(1.0))
                .height(Length::Fixed(OPTION_DIVIDER))
                .style(move |_t: &iced::Theme| container::Appearance {
                    background: Some(Background::Color(theme_gen::ink(theme, Ink::ButtonBg))),
                    ..container::Appearance::default()
                }),
        );
        line = line.push(exclude_button(theme, row));
    }
    let background = row_fill(row.chosen, row.excluded, hover).map(|(ink, alpha)| {
        Background::Color(Color { a: alpha, ..theme_gen::ink(theme, ink) })
    });
    mouse_area(
        container(line)
            .width(Length::Fill)
            .padding(Padding { top: 4.0, right: 8.0, bottom: 4.0, left: 0.0 })
            .style(move |_t: &iced::Theme| container::Appearance {
                background,
                border: Border {
                    radius: 12.0.into(),
                    ..Border::default()
                },
                ..container::Appearance::default()
            }),
    )
    // The hover end is the factor the row already draws at: an option row tints
    // nothing, so the tween's job here is only to say how far *through* the hover
    // it is, which is the number the label and the check read.
    .on_enter(Message::hover_with(key, true, 1.0))
    .on_exit(Message::hover_with(key, false, 1.0))
    .on_press(row.press.clone())
    .into()
}

/// \`h-4 w-4\` on \`SearchFilterOption.vue\`'s check, which is a quarter of the
/// section's own \`size-5\`.
const OPTION_CHECK: f32 = 16.0;

/// The height of the divider that separates a row from its exclude press:
/// `h-[1.75rem]`, twenty-eight pixels, which is a little taller than the row's
/// own twenty-eight pixels of padding and label and so is the row's own height.
const OPTION_DIVIDER: f32 = 28.0;

/// The second press of an option row: `SearchFilterOption.vue`'s exclude button.
///
/// A ban icon at sixteen pixels in a `px-2 py-1` button of its own, in
/// `text-secondary` and turning `--color-red` under its own pointer -- so it
/// takes a hover key of its own rather than sharing the row's, which is what the
/// reference's separate button is for.
fn exclude_button<'a>(theme: Gen, row: &Row) -> Element<'a, Message> {
    let key = crate::ui::scoped("discover:exclude", &row.id);
    let (_, hover) = crate::ui::interaction(key);
    let ink = if hover > 0.0 { Ink::Red } else { INK_SECONDARY };
    let ban = icon::icon(Glyph::Ban, OPTION_CHECK, theme_gen::ink(theme, ink));
    mouse_area(
        container(row![].align_items(Alignment::Center).push(ban))
            .padding(Padding { top: 4.0, right: 8.0, bottom: 4.0, left: 8.0 }),
    )
    .on_enter(Message::hover_with(key, true, 1.0))
    .on_exit(Message::hover_with(key, false, 1.0))
    .on_press(row.exclude.clone())
    .into()
}

/// \`size-5\`: the section's own dropdown icon.
const SECTION_ICON: f32 = 20.0;

/// Whether two choices of the section filed under `header` are alternatives to
/// each other rather than two things that both have to hold.
///
/// `search.ts` builds every category option with
/// `method: category.header === 'resolutions' ? 'or' : 'and'`, so
/// `resolutions` is the one header whose rows join into `categories IN [...]`.
/// Everything else -- `technical`, `gameplay`, and every header the API invents
/// after this port was written -- is `'and'`.
fn alternatives(header: &str) -> bool {
    header == RESOLUTIONS_HEADER
}

/// The header id `search.ts` names for the one whose options are alternatives.
const RESOLUTIONS_HEADER: &str = "resolutions";

/// One chosen value, as `search.ts` writes it: `field = "value"`, or a list of
/// them when more than one of the field's options is chosen.
///
/// The quoted form is this tree's own, and it is what the engine's tests are
/// written in; the reference quotes with backticks, which its parser takes too.
/// The difference is a character in a string Modrinth reads, not a different
/// question.
fn any_of(field: &str, values: &[&str]) -> Option<String> {
    match values {
        [] => None,
        [one] => Some(format!("{field} = \"{one}\"")),
        _ => Some(format!("{field} IN [{}]", quoted(values))),
    }
}

/// The excluded values of one field, as `newFilters` writes `negativeByType`.
///
/// A list even for one value: this is the only part of the facet string where
/// the reference does not collapse to `field = value` for a single option, and
/// an exclusion of exactly one is the case that reads as a typo otherwise.
fn none_of(field: &str, values: &[&str]) -> Option<String> {
    (!values.is_empty()).then(|| format!("{field} NOT IN [{}]", quoted(values)))
}

/// A list of values the way `formatSearchFilterValue` writes one.
///
/// The quoted form is this tree's own, and it is what the engine's tests are
/// written in; the reference quotes with backticks, which its parser takes too.
/// The difference is a character in a string Modrinth reads, not a different
/// question.
fn quoted(values: &[&str]) -> String {
    values.iter().map(|value| format!("\"{value}\"")).collect::<Vec<String>>().join(", ")
}

/// The environment values that satisfy the chosen pair, as `search.ts`'s
/// `getEnvironmentFilterGroups` hands them over.
///
/// Each group is one alternative to the next, and the whole list is one part of
/// the request. The names are the API's, and they are longer than the two rows
/// that choose them: *Client* does not mean "the project says client", it means
/// any of the four ways a project can be a client, which is why choosing both
/// rows is a longer list rather than the union of the two.
fn environment_groups(client: bool, server: bool) -> Vec<Vec<&'static str>> {
    if client && server {
        vec![vec![
            "client_only_server_optional",
            "server_only_client_optional",
            "client_and_server",
            "client_or_server",
            "client_or_server_prefers_both",
        ]]
    } else if client {
        vec![vec![
            "client_only",
            "client_only_server_optional",
            "client_or_server_prefers_both",
            "client_or_server",
        ]]
    } else if server {
        vec![vec![
            "server_only",
            "dedicated_server_only",
            "server_only_client_optional",
            "client_or_server_prefers_both",
            "client_or_server",
        ]]
    } else {
        Vec::new()
    }
}

/// The `border-b border-[--brand-gradient-border]` every one of the sidebar's
/// sections carries, as the one thing iced will draw: a one-pixel line under the
/// block.
///
/// `--brand-gradient-border` rather than `--divider`, which is what the first
/// draft of this drew and what the reference's capture settles: the rule under
/// the switch section measures (32, 48, 40) at 1280x720 -- the same green as the
/// panel's own left edge -- and `--surface-5` would be (66, 68, 74). It is the
/// panel's [`crate::shell`] edge colour because it is the same token, drawn the
/// same one-pixel way.
///
/// One colour for a gradient: the reference's is a vertical gradient running from
/// (33, 50, 40) at the top of the panel to (29, 41, 34) at the bottom, and a
/// single colour in the middle of that range is as close as this toolkit can get.
/// The panel's own edge has the same limit and says so.
fn panel_rule<'a, Message: 'a>(theme: Gen) -> Element<'a, Message> {
    container(Space::new(Length::Fill, 1.0))
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(move |_t: &iced::Theme| container::Appearance {
            background: Some(Background::Color(theme_gen::ink(
                theme,
                Ink::BrandGradientBorder,
            ))),
            ..container::Appearance::default()
        })
        .into()
}

/// The project-type tabs: `Browse.vue`'s own list, its own labels.
///
/// The label is `ProjectType::label`, which `route.rs` asserts against the
/// reference's category message, so the tab text cannot drift from the locale
/// without a test failing.
fn tabs<'a>(theme: Gen, state: &'a State) -> Element<'a, Message> {
    let labels: Vec<(String, bool)> = ProjectType::TABS
        .iter()
        .map(|kind| (kind.label().to_string(), *kind == state.project_type))
        .collect();
    ui::tabs(theme, &TAB_KEYS, &labels, move |index| {
        Message::ProjectType(ProjectType::TABS.get(index).copied().unwrap_or(ProjectType::Modpack))
    })
}

/// The controls row: the order and the view size, and nothing else.
///
/// `browse-tab/layout.vue`'s own row (`flex flex-wrap items-center gap-2`)
/// holds exactly the two Comboboxes on the desktop -- `!w-[16rem]` (256) for
/// the sort and `!w-[9rem]` (144) for the view size -- each with its prefix
/// (`commonMessages.sortByLabel` and `browse.view-prefix`). The row here carried
/// two more things than the reference has: a *Filter results...* button, which
/// is inside the reference's `lg:hidden` div (it is the narrow layout's), and a
/// "*Modpacks* · relevance" caption, which is nobody's. Both are gone, and what
/// is left is the reference's own pair.
///
/// Both prefixes here are **text**, which is the line that separates this row
/// from the library toolbar's: `browse-tab/layout.vue:190` and `:209` put
/// `<span class="font-semibold text-primary">` in the `#prefix` slot, where
/// `sort-menu.vue` puts a `size-5` glyph. So [`ui::Prefix::Text`] here and
/// [`ui::Prefix::Glyph`] there, and neither page can be read off the other --
/// the same slot with two contents is the whole of [`ui::Prefix`].
fn controls<'a>(theme: Gen, state: &'a State) -> Element<'a, Message> {
    row![]
        .spacing(ROW_GAP)
        .align_items(Alignment::Center)
        .push(ui::select(
            theme,
            ui::Prefix::Text(Key::LabelSortBy),
            state.sort.label(),
            SORT_WIDTH,
            state.menu == Some(Menu::Sort),
            Message::Toggle(Menu::Sort),
        ))
        .push(ui::select(
            theme,
            ui::Prefix::Text(Key::BrowseViewPrefix),
            &state.view_label(),
            VIEW_WIDTH,
            state.menu == Some(Menu::View),
            Message::Toggle(Menu::View),
        ))
        .into()
}

/// The sort trigger's width: `!w-[16rem]` on the reference's own class.
const SORT_WIDTH: f32 = 256.0;

/// The view trigger's width: `!w-[9rem]` on the same class.
const VIEW_WIDTH: f32 = 144.0;

/// The panel one of the two comboboxes has open, under the trigger that opened it.
///
/// The panel's vertical place is this column's next child after [`controls`],
/// which gives it [`SEARCH_TO_CONTROLS`] of air above it -- the same eight
/// `Combobox.vue` calls `DROPDOWN_GAP` -- and leaves the results below the gap
/// they were drawn with. It is pushed only when a panel is open: a child with
/// nothing in it would still cost its spacing, and the capture this page is
/// measured against has none.
///
/// Its horizontal place is the trigger's own offset in [`controls`]'s row, so it
/// lands under the control that opened it rather than under the page: 0 for the
/// sort, one trigger and one gap for the view.
fn menu<'a>(theme: Gen, state: &'a State) -> Option<Element<'a, Message>> {
    let (namespace, offset, width, options, chosen) = match state.menu? {
        Menu::Sort => (
            "discover:sort",
            0.0,
            SORT_WIDTH,
            Sort::ALL
                .iter()
                .map(|order| (order.label().to_string(), Message::Sort(*order)))
                .collect::<Vec<_>>(),
            state.sort.label().to_string(),
        ),
        Menu::View => (
            "discover:view",
            SORT_WIDTH + ROW_GAP,
            VIEW_WIDTH,
            VIEW_SIZES
                .iter()
                .map(|size| (size.to_string(), Message::View(*size)))
                .collect::<Vec<_>>(),
            state.view_label(),
        ),
    };
    Some(
        row![Space::with_width(offset), ui::select_menu(theme, namespace, width, &options, &chosen)]
            .align_items(Alignment::Start)
            .width(Length::Fill)
            .into(),
    )
}

/// The results, the blocks for an answer still on the way, or the sentence for
/// having none of them.
///
/// The empty arm is the reference's own sentence. The waiting one is its own
/// blocks rather than [`crate::page::draw`]'s *Loading results…*, because
/// `browse-tab/layout.vue` draws `LoadingIndicator` for that state and this is
/// the page that owns the shape -- `crate::page`'s sentence stays the scaffold
/// for the pages that have none of their own.
fn results<'a>(theme: Gen, state: &'a State) -> Element<'a, Message> {
    match &state.results {
        Load::Ready(hits) if hits.is_empty() => page::empty(theme, Key::BrowseNoResults),
        Load::Ready(hits) => cards(theme, state, hits),
        Load::Empty => page::empty(theme, Key::BrowseNoResults),
        Load::Failed(reason) => page::failed(theme, reason),
        Load::Idle | Load::Loading => loading(theme),
    }
}

/// `h-16` on each block of the reference's loading indicator, in pixels.
const LOADING_BLOCK: f32 = 64.0;
/// `opacity-25` on each of them.
const LOADING_BLOCK_OPACITY: f32 = 0.25;

/// What the whole indicator is tall: three `h-16` blocks and the two `gap-2`
/// between them, and **not** a fourth gap for the label -- see [`loading`].
const LOADING_STACK: f32 = LOADING_BLOCK * 3.0 + ROW_GAP * 2.0;

/// Which of the three blocks the label is drawn over. The middle one, because
/// that is where the reference's lands.
const LOADING_LABEL_BLOCK: usize = 1;

/// What the page shows while its first answer is on the way:
/// `base/LoadingIndicator.vue`'s own three blocks under its label.
///
/// The component is `w-full flex items-center justify-center flex-col gap-2`
/// around a `font-bold text-contrast` *Loading* and three placeholders: `h-16`
/// (64px) each, `rounded-lg` (16px), `opacity-25`, in `--color-raised-bg`. The
/// label's dots are the component's own animation resting at `'...'` (its
/// `::after` keyframes start and end there), so *Loading...* is the frame a
/// still capture shows and the frame drawn here.
///
/// **The label is not in the flow.** Its rule is `position: absolute; z-index: 1`
/// with no `top`/`left`, so the column's in-flow children are the three blocks
/// alone: [`LOADING_STACK`] is 3 x 64 + 2 x 8 = **208**. Drawing the label above
/// them -- which is what this port used to do -- spent 32 pixels on it, its own
/// 24-pixel line and the `gap-2` above the first block, and put every block 32
/// lower: measured on `/tmp/discover-loading.png` the three sat at y270, y342
/// and y414 with the word at y245..259 above them, and on
/// `/tmp/discover-loading2.png` they are y238, y310 and y382 -- 238..445, which
/// is the 208. (Each block measures 63 or 64 because the stack lands on a
/// fractional y; the fill is (27,28,33), which is `--color-raised-bg` at 25%
/// over the page's (22,24,28).)
///
/// **Where it lands is a reading of flexbox, not a measurement.** An
/// absolutely-positioned child of a flex container with `auto` insets is placed
/// at the static position it *would* have had as the container's only item, and
/// this container is `justify-center` on its main (vertical) axis and
/// `items-center` on the cross one -- so the label is centred over the stack,
/// which puts it over the middle block. That is where it is drawn
/// ([`LOADING_LABEL_BLOCK`]). It could not be checked against the reference: its
/// loading frame needs an answer that never arrives, `:99` belongs to another
/// agent, and the alternative reading -- top of the stack, over the first block
/// -- would put the same word 64 pixels higher. A container paints its fill and
/// then its child, so drawing the label *inside* the middle block is the
/// overlay without a stacking widget.
fn loading<'a>(theme: Gen) -> Element<'a, Message> {
    // The label is the reference's own literal rather than a locale key: its
    // template writes `Loading` and the dots are CSS.
    let label = text("Loading...".to_string())
        .size(16.0)
        // `font-bold` is weight 700, which is `Weight::Bold`; the kit's
        // `semibold` is 600.
        .font(Font { weight: iced::font::Weight::Bold, ..semibold() })
        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST)));
    let mut blocks = column![]
        .spacing(ROW_GAP)
        .align_items(Alignment::Center)
        .width(Length::Fill);
    for index in 0..3 {
        let over: Option<Element<'a, Message>> =
            (index == LOADING_LABEL_BLOCK).then(|| label.clone().into());
        blocks = blocks.push(loading_block(theme, over));
    }
    blocks.into()
}

/// One of [`loading`]'s three blocks: `h-16 rounded-lg opacity-25` in
/// `--color-raised-bg`, with `over` drawn on top of it when it is the block the
/// label belongs to.
fn loading_block<'a>(theme: Gen, over: Option<Element<'a, Message>>) -> Element<'a, Message> {
    // A `Container` holds one child, so the block is the child: the label where
    // the label belongs, and a space that fills the box where it does not. The
    // `h-16` is on the container either way, because a container sized by its
    // child would be as tall as the word.
    let block = match over {
        Some(over) => container(over).center_x().center_y(),
        None => container(Space::new(Length::Fill, Length::Fill)),
    };
    block
        .width(Length::Fill)
        .height(Length::Fixed(LOADING_BLOCK))
        .style(move |_theme: &iced::Theme| container::Appearance {
            background: Some(Background::Color(at_opacity(
                theme_gen::ink(theme, Ink::RaisedBg),
                LOADING_BLOCK_OPACITY,
            ))),
            border: Border {
                // `rounded-lg`, the reference's 16 (`Span::RadiusLg`).
                radius: theme_gen::span(theme_gen::Span::RadiusLg).into(),
                ..Border::default()
            },
            ..container::Appearance::default()
        })
        .into()
}

/// The result cards: the ones the region reports are on screen, the rest as
/// space.
///
/// `ProjectCardList`'s own `gap-3` between the cards of a *list*, which is
/// [`GAP`] -- 12 pixels -- and not the wider [`crate::page::GRID_GAP`] its grid
/// layout is spaced with. The reference's browse opens on the list one
/// (`use-browse-search.ts`'s `effectiveDisplayMode` defaults to `'list'`), so this
/// is the gap a reader sees -- and it is inside [`CARD_ROW`], because the window
/// places rows by multiplying one height, and a gap the layout added between
/// them would be a height it did not account for.
///
/// **Windowed**: only the rows the report puts on screen are built, and the rest
/// of the list is two spacers. The point is the frame's cost ([`scroll::window`],
/// a port of the reference's own `useVirtualScroll`), and the spacers are what
/// keeps the scrollbar honest: the content's *height* is what it is drawn
/// against, so a window that dropped the rows it did not draw would be a list
/// that scrolls as if it were one screen long. Their total is exact because a row
/// is exactly [`CARD_ROW`] tall ([`slot`]).
fn cards<'a>(theme: Gen, state: &'a State, hits: &'a [Hit]) -> Element<'a, Message> {
    let drawn = scroll::window(hits.len(), CARD_ROW, list_at(state.geometry));
    let mut list = column![].spacing(0.0).width(Length::Fill);
    if drawn.start > 0 {
        list = list.push(space(drawn.start, CARD_ROW));
    }
    for hit in &hits[drawn.clone()] {
        list = list.push(slot(
            hit_card(theme, hit, state.icons.get(&hit.icon_url)),
            CARD_ROW,
        ));
    }
    if drawn.end < hits.len() {
        list = list.push(space(hits.len() - drawn.end, CARD_ROW));
    }
    list.into()
}

/// The list's own geometry, out of the page's.
///
/// The region's offset counts from the page's first pixel and the list starts
/// [`LIST_TOP`] below it, so what the window is owed is the page's offset less
/// the head. The view height is left as the region reported it: while the head is
/// still on screen the window computes a row more than it needs and never one
/// fewer, and past the head the two are the same.
fn list_at(page: Geometry) -> Geometry {
    Geometry {
        offset: (page.offset - LIST_TOP).max(0.0),
        view_height: page.view_height,
    }
}

/// One row's slot: exactly `row_height` tall, whatever the card inside it needs.
///
/// The window's arithmetic places each drawn card at `index * row_height`, so a
/// card that took its own content's height would put every card under it
/// somewhere the window did not compute. The card is drawn at the top of its slot
/// and the gap under it is part of the slot ([`CARD_ROW`]), which is the shape a
/// `gap-3` list has as well; the top of a container is `Vertical::Top` here, not
/// `iced::Alignment::Start`, because it is the vertical one the widget asks for.
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

/// `ProjectCard.vue`'s list grid: `p-4 grid-project-card-list gap-x-3 gap-y-2`.
///
/// The card's own two gaps. The padding is [`ui::card_at`]'s (`p-4`, the same 16).
const CARD_COLUMN_GAP: f32 = 12.0;
/// `gap-y-2` between the card's grid rows: the info column's own spacing
/// between the title row, the summary and the tag row.
const CARD_ROW_GAP: f32 = 8.0;

/// One card, as the reference's list layout draws it: 142 pixels of box.
///
/// Quoted from `ProjectCard.vue` and measured on the reference's own window
/// (1280x720, 2026-10-02): the card is `p-4 grid grid-project-card-list gap-x-3
/// gap-y-2` with a `size="100px"` avatar and a `text-sm`/`py-1` tag pill, and
/// the *drawn* height is 142 -- the interior's 110 plus two 16-pixel paddings
/// -- with the cards at y=231, 385, 539 and 693: a 154-pixel pitch, which is
/// 142 plus the `gap-3` between two of them.
///
/// The 110-pixel interior is not a sum of the parts, and that is the point: the
/// grid gives the icon area all three of its rows, so the tag row is drawn
/// *inside* the icon's 100 pixels rather than under them. The card here used to
/// be `100 + 8 + 24 + 32` -- every part in a column of its own -- which is 22
/// pixels taller than the reference draws.
pub const CARD_HEIGHT: f32 = 142.0;

/// The card's inside: what [`CARD_HEIGHT`] leaves under its two `p-4` paddings
/// ([`ui::CARD_PAD`], 16 each).
const CARD_INNER: f32 = CARD_HEIGHT - ui::CARD_PAD * 2.0;

/// One row of the results: a card plus the `gap-3` that separates it from the
/// card under it ([`GAP`]).
///
/// This is the number the window is cut into ([`scroll::window`]), so it is the
/// number the slots are ([`slot`]) and the spacers ([`space`]): a row that was
/// laid out at its content's height would put the rows under it at offsets the
/// window did not compute. Measured from the reference's own cards: 154.
pub const CARD_ROW: f32 = CARD_HEIGHT + GAP;

/// Where the results start inside the page's scroll content, in pixels.
///
/// **Measured**, because there is nothing to quote: the head above the results
/// -- the tab strip, the search field and the two controls, each between the
/// page's own twelve-pixel gaps -- is laid out by [`ui`], and no constant of this
/// page names its height. On a 1280x720 capture of this page (2026-10-02) the
/// first card's top is at y=230 and the region starts at y=49, which is the 181
/// here. What a drift in it costs is cover rather than a hole: the window draws
/// [`scroll::OVERSCAN`] rows on each side of the visible one, a margin of five
/// rows against a head that moves by pixels.
const LIST_TOP: f32 = 181.0;

/// One project card: its icon, its title, its author, its summary and its counts.
///
/// The whole card is pressable, which is what the reference does and what the
/// absence of controls inside it makes safe: iced's `mouse_area` does not forward
/// a press to its content, so nothing interactive may be drawn inside one.
///
/// The arrangement is `ProjectCard.vue`'s list layout, which is a CSS grid:
///
/// ```text
/// grid-project-card-list =
///     'icon info  stats stats'
///     'icon info  stats stats'
///     'icon tags  tags  tags';
/// grid-template-columns: auto 1fr auto auto;
/// ```
///
/// iced has no grid, so the same placement is one row of three columns: the
/// icon, the info column and the stats. The tags are the info column's last
/// child, pinned to the bottom of a fixed interior: the icon's area spans all
/// three of the grid's rows, so the tags are drawn *inside* the icon's own
/// vertical span -- which is why the card is shorter than the icon stack plus a
/// tag row, and why [`CARD_HEIGHT`] is a measurement of the drawn card rather
/// than a sum of its parts.
pub fn hit_card<'a>(theme: Gen, hit: &Hit, picture: Option<&Icon>) -> Element<'a, Message> {
    // A card's identity is the project it names, so its key is derived from that
    // rather than from its position in the list: reordering the results must not
    // move a tween from one card to another.
    //
    // The card is a `ProjectCard.vue` in its list layout, whose hover is
    // `smart-clickable:highlight-on-hover` -- `filter: brightness(1.25)` in the
    // dark theme, the global `--hover-brightness`, not the 0.9 that
    // `LegacyProjectCard.vue` dims a *grid* card by. The two are different cards
    // in the reference and it was the grid one's constant that was ported here.
    //
    // One thing that filter does in the reference and cannot here: it brightens
    // the whole subtree, and [`ui::card_at`] brightens the surface and the hairline
    // only. The ink inside this card is already at `text-contrast`'s white, which a
    // multiplier cannot lift any further, so what is left undrawn is the same
    // brightening applied to the tag pills and to the icon -- and a picture has no
    // filter in this toolkit.
    let key = ui::scoped("discover:card", &hit.id);
    let (factor, _) = ui::interaction(key);

    // `ProjectCardTitle` (`text-xl font-semibold text-contrast`, 20px with the
    // reference's 16px root) and `ProjectCardAuthor` (`text-secondary font-normal`,
    // which is the *tertiary* token -- see `crate::style`), eight pixels apart
    // (`gap-2`).
    let title = text(hit.title.clone())
        .size(20.0)
        .font(semibold())
        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST)));
    let author = text(format!("by {}", hit.author))
        .size(16.0)
        .font(regular())
        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY)));
    // `project-card-summary m-0 font-normal`: the reference's root 16, normal
    // weight, the default ink, and clamped to two lines in CSS. The clamp is not
    // drawn: iced wraps a paragraph and has no line limit, so a very long summary
    // runs past the card's measured interior rather than being cut at two lines.
    let summary = text(hit.summary.clone())
        .size(16.0)
        .font(regular())
        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT)));
    // `ProjectCardStats`: the two counts side by side, twelve pixels apart
    // (`gap-3`), right-aligned because the grid's stats column is `items-end`.
    let stats = row![stat(theme, Glyph::Download, hit.downloads), stat(theme, Glyph::Heart, hit.follows)]
        .spacing(CARD_COLUMN_GAP)
        .align_items(Alignment::Center);

    // `ProjectCardTags`: `flex items-center gap-2` inside a `gap-3` row. The
    // reference lists a hit's own `display_categories` then its loaders, of which
    // the search answer here carries the loaders and the supported versions -- the
    // two the card has drawn all along, and the `maxTags` the reference passes is
    // five for a list card with no actions.
    let mut tags = row![].spacing(ROW_GAP).align_items(Alignment::Center);
    for loader in hit.loaders.iter().take(3) {
        tags = tags.push(ui::tag(theme, loader));
    }
    for version in hit.game_versions.iter().take(2) {
        tags = tags.push(ui::tag(theme, version));
    }

    // The info column with the tags at its bottom: the `Fill` between the
    // summary and the tags is what puts them against the card's own bottom edge
    // whatever the summary's line count, which is where the reference's grid
    // draws them (its icon area spans all three of the grid's rows).
    let info = column![]
        .spacing(CARD_ROW_GAP)
        .width(Length::Fill)
        .height(Length::Fixed(CARD_INNER))
        .push(row![title, author].spacing(ROW_GAP).align_items(Alignment::Center))
        .push(summary)
        .push(Space::with_height(Length::Fill))
        .push(tags);

    // The grid in one iced row: its three columns are the icon, the info column
    // and the stats. `Start` vertically, which is the grid's own top alignment:
    // the icon's box and the stats sit against the first line of the info
    // column.
    let placed = row![
        ui::icon_box(theme, avatar::ICON_SIDE as f32, picture),
        info,
        stats
    ]
    .spacing(CARD_COLUMN_GAP)
    .align_items(Alignment::Start);

    mouse_area(ui::card_at(theme, factor, placed))
        .interaction(Interaction::Pointer)
        .on_enter(Message::hover(key, true))
        .on_exit(Message::hover(key, false))
        .on_press(Message::Open(hit.id.clone()))
        .into()
}

/// One count of a card: `ProjectCardStats.vue`.
///
/// A 20px icon (`size-5`) and the count, eight pixels apart (`gap-2`), in the
/// theme's default ink -- the component sets no colour class, so it inherits the
/// page's own, which is what [`INK_DEFAULT`] names.
///
/// The count itself is the reference's *compact* form (`41M`), because that is what
/// it draws: the full number is a tooltip there, and this kit has no tooltip, so the
/// abbreviated one is all a reader gets. [`crate::locale::compact`] is that rule.
fn stat<'a>(theme: Gen, glyph: Glyph, count: u64) -> Element<'a, Message> {
    let ink = theme_gen::ink(theme, INK_DEFAULT);
    row![]
        .spacing(ROW_GAP)
        .align_items(Alignment::Center)
        .push(icon::icon(glyph, 20.0, ink))
        .push(
            text(locale::compact(locale::tag(), count))
                .size(16.0)
                .font(medium())
                .style(iced::theme::Text::Color(ink)),
        )
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The search a fresh page owes, past the tag list it asks for first.
    ///
    /// [`State::opening`] asks for the tags and then, on the next turn, for the
    /// results; a test that only wants the second has to walk the first out of
    /// the way, and this is that walk.
    fn opening_search(state: &mut State) -> Asked {
        assert_eq!(state.opening(), Some(Ask::Tags), "the tag list is asked for first");
        let Some(Ask::Search(asked)) = state.opening() else {
            panic!("and then the search");
        };
        asked
    }

    fn hit(title: &str) -> Hit {
        Hit {
            id: title.to_lowercase(),
            title: title.to_string(),
            author: "jelly".to_string(),
            summary: "a summary".to_string(),
            downloads: 12345,
            follows: 678,
            // No icon unless a test names one: a project that never uploaded one is
            // a state the card draws rather than an error.
            icon_url: String::new(),
            game_versions: vec!["1.21".to_string()],
            loaders: vec!["fabric".to_string()],
        }
    }

    /// A PNG of one colour, in memory, the way `avatar.rs`'s tests make one.
    fn picture(side: u32) -> Vec<u8> {
        let square =
            ::image::RgbaImage::from_pixel(side, side, ::image::Rgba([10, 20, 30, 255]));
        let mut bytes = Vec::new();
        ::image::DynamicImage::ImageRgba8(square)
            .write_to(&mut std::io::Cursor::new(&mut bytes), ::image::ImageFormat::Png)
            .expect("a PNG in memory");
        bytes
    }

    /// One icon, as the shell's fetch would deliver it.
    fn an_icon(url: &str) -> Fetched {
        Fetched {
            url: url.to_string(),
            icon: Icon::of(&picture(64), avatar::ICON_SIDE).expect("a PNG"),
        }
    }

    const SODIUM_ICON: &str = "https://cdn.modrinth.com/sodium.png";
    const LITHIUM_ICON: &str = "https://cdn.modrinth.com/lithium.png";

    #[test]
    fn the_orders_are_the_five_the_reference_declares() {
        assert_eq!(Sort::ALL.len(), 5);
        assert_eq!(Sort::default(), Sort::Relevance);
        let labels: Vec<&str> = Sort::ALL.iter().map(|sort| sort.label()).collect();
        assert_eq!(
            labels,
            vec!["Relevance", "Downloads", "Followers", "Date published", "Date updated"]
        );
        let tokens: Vec<&str> = Sort::ALL.iter().map(|sort| sort.token()).collect();
        assert_eq!(tokens, vec!["relevance", "downloads", "follows", "newest", "updated"]);
    }

    #[test]
    fn each_control_s_panel_opens_on_its_own_trigger_and_shuts_on_any_other_press() {
        // `Combobox.vue:449`'s `handleTriggerClick`: the press names the panel,
        // and the second press on the same trigger shuts it. The reference's
        // panel is one element, so two triggers never have one open each.
        let mut state = state_of(ProjectType::Mod);
        assert_eq!(state.menu, None, "a fresh page draws no panel");
        assert!(state.update(Message::Toggle(Menu::Sort)).is_none(), "opening asks for nothing");
        assert_eq!(state.menu, Some(Menu::Sort));
        assert!(state.update(Message::Toggle(Menu::Sort)).is_none());
        assert_eq!(state.menu, None, "the second press is how a reader backs out");
        state.update(Message::Toggle(Menu::Sort));
        state.update(Message::Toggle(Menu::View));
        assert_eq!(state.menu, Some(Menu::View), "the other trigger swaps rather than stacks");
        // The panel's chosen row is the control's own current value, so the
        // label the trigger reads and the row the panel paints green are the
        // same string.
        assert_eq!(state.sort.label(), "Relevance");
        assert_eq!(state.view_label(), "20");
        // Choosing is a press outside the panel's own two answers, so it shuts
        // the panel as well as changing the control -- which is what
        // `onClickOutside` did for a click that landed on an option.
        assert!(state.update(Message::Sort(Sort::Downloads)).is_some(), "and it re-asks");
        assert_eq!(state.menu, None);
        assert_eq!(state.sort.label(), "Downloads");
        state.update(Message::Toggle(Menu::View));
        state.update(Message::View(10));
        assert_eq!(state.menu, None);
        assert_eq!(state.view_label(), "10");
        // Every other press is the click-outside this kit has no event for, so
        // `update` is where an open panel is shut by it.
        for message in [
            Message::Query("sodium".to_string()),
            Message::Category("technology".to_string()),
            Message::Section("technology".to_string()),
            Message::Page(2),
            Message::Search,
        ] {
            state.menu = Some(Menu::Sort);
            state.update(message);
            assert_eq!(state.menu, None);
        }
        // A pointer crossing, a scroll and an answer that lands while the panel
        // is open are none of them presses: the browser closes nothing by
        // scrolling past it, and an older search's results arriving is not the
        // reader doing anything at all. The answer here is a stale one (round 0
        // is never the page's own), so it is dropped as well as leaving the
        // panel open.
        for message in [
            Message::Hover { key: "discover:sort", over: true, hover: None },
            Message::Scrolled(crate::scroll::Geometry::default()),
            Message::VersionsScrolled(crate::scroll::Geometry::default()),
            Message::Found { round: 0, result: Ok(Vec::new()) },
        ] {
            state.menu = Some(Menu::Sort);
            state.update(message);
            assert_eq!(state.menu, Some(Menu::Sort));
        }
        // And the row with the two triggers, and a panel under it, draw over
        // every theme.
        for theme in Gen::ALL {
            state.menu = None;
            drop(super::view(*theme, &state, &Store::default()));
            state.menu = Some(Menu::View);
            drop(super::view(*theme, &state, &Store::default()));
        }
    }

    #[test]
    fn the_view_sizes_are_the_reference_s_and_it_opens_on_twenty() {
        assert_eq!(VIEW_SIZES, [5, 10, 15, 20, 50, 100]);
        assert_eq!(DEFAULT_VIEW, 20);
        assert!(VIEW_SIZES.contains(&DEFAULT_VIEW));
        let state = State::new(ProjectType::Modpack);
        assert_eq!(state.view, 20);
        assert_eq!(state.view_label(), "20");
    }

    #[test]
    fn the_search_field_asks_for_the_tab_it_is_on() {
        // `browse.search.placeholder` is `Search {projectType}...`, so the two
        // tabs are two different questions.
        let mut state = State::new(ProjectType::Modpack);
        assert_eq!(state.placeholder(), "Search modpack...");
        state.update(Message::ProjectType(ProjectType::Mod));
        assert_eq!(state.placeholder(), "Search mod...");
        state.update(Message::ProjectType(ProjectType::Shader));
        assert_eq!(state.placeholder(), "Search shader...");
        state.update(Message::ProjectType(ProjectType::ResourcePack));
        assert_eq!(state.placeholder(), "Search resource pack...");
    }

    #[test]
    fn changing_what_is_asked_starts_the_answer_again() {
        let mut state = State::new(ProjectType::Modpack);
        state.update(Message::Page(4));
        assert_eq!(state.page, 4);
        // A new sort, a new query and a new tab all go back to the first page: the
        // fourth page of a different search is not a place.
        state.update(Message::Sort(Sort::Downloads));
        assert_eq!(state.page, 1);
        state.update(Message::Page(3));
        state.update(Message::Query("sodium".to_string()));
        assert_eq!(state.page, 1);
        state.update(Message::Page(2));
        state.update(Message::ProjectType(ProjectType::Datapack));
        assert_eq!(state.page, 1);
        assert_eq!(state.project_type, ProjectType::Datapack);
        // Choosing the tab already on screen is not a change.
        state.update(Message::Page(5));
        state.update(Message::ProjectType(ProjectType::Datapack));
        assert_eq!(state.page, 5);
        // And a page number cannot be zero or negative.
        state.update(Message::Page(0));
        assert_eq!(state.page, 1);
    }

    #[test]
    fn a_control_that_changes_the_request_asks_for_it() {
        // `use-browse-search.ts` refreshes whenever the request's parameters
        // change. The page's half of that is the `Ask` each changing control
        // returns: with the *Filter results...* button gone, a control that did
        // not ask would leave the reader with no way to run their own search.
        let mut state = State::new(ProjectType::Modpack);
        let Some(Ask::Search(asked)) = state.update(Message::Query("sodium".to_string())) else {
            panic!("typing asks");
        };
        assert_eq!(asked.query.query, "sodium");
        assert_eq!(state.results, Load::Loading);
        let Some(Ask::Search(asked)) = state.update(Message::Sort(Sort::Downloads)) else {
            panic!("a new order asks");
        };
        assert_eq!(asked.query.index.as_deref(), Some("downloads"));
        let Some(Ask::Search(asked)) = state.update(Message::View(50)) else {
            panic!("a new view size asks");
        };
        assert_eq!(asked.query.limit, 50);
        let Some(Ask::Search(asked)) = state.update(Message::Page(3)) else {
            panic!("a new page asks");
        };
        assert_eq!(asked.query.offset, 100);
        // Choosing what is already chosen is not a change, and asking again for
        // the same request would throw the answer on screen away for nothing.
        assert_eq!(state.update(Message::Sort(Sort::Downloads)), None);
        assert_eq!(state.update(Message::View(50)), None);
        assert_eq!(state.update(Message::Query("sodium".to_string())), None);
        assert_eq!(state.update(Message::Page(3)), None);
    }

    #[test]
    fn the_tag_list_is_asked_before_the_first_search_and_kept() {
        let mut state = State::new(ProjectType::Modpack);
        // Two requests, in the order the page can draw something in.
        assert_eq!(state.opening(), Some(Ask::Tags));
        assert_eq!(state.tags, Load::Loading);
        assert_eq!(state.results, Load::Idle, "and nothing has been searched for yet");

        let tags = palantir_net::Tags {
            categories: vec![palantir_net::CategoryTag {
                name: "kitchen-sink".to_string(),
                project_type: "modpack".to_string(),
                header: "technical".to_string(),
            }],
            ..palantir_net::Tags::default()
        };
        state.update(Message::Tags { result: Ok(tags.clone()) });
        assert_eq!(state.tags, Load::Ready(tags));
        let Some(Ask::Search(first)) = state.opening() else {
            panic!("and then the search");
        };
        assert_eq!(first.query.project_type.as_deref(), Some("modpack"));

        // A second search does not ask for it again: the document changes when
        // Modrinth ships a release, not when the reader types.
        state.update(Message::Query("sodium".to_string()));
        assert_eq!(state.tags, Load::Ready(state.tags.ready().expect("kept").clone()));

        // A store with no engine answers with a reason, which is a section that
        // knows it has nothing rather than one that drew itself empty.
        let mut bare = State::new(ProjectType::Modpack);
        bare.update(Message::Tags { result: Err("no engine".to_string()) });
        assert_eq!(bare.tags, Load::Failed("no engine".to_string()));
        let mut empty = State::new(ProjectType::Modpack);
        empty.update(Message::Tags { result: Ok(palantir_net::Tags::default()) });
        assert_eq!(empty.tags, Load::Empty, "a tag list with nothing in it is Empty");
    }

    #[test]
    fn a_chosen_category_is_one_and_ed_part_of_the_request() {
        let mut state = State::new(ProjectType::Modpack);
        assert!(state.category_parts().is_empty(), "nothing chosen is no facet");

        let Some(Ask::Search(first)) = state.update(Message::Category("technology".to_string()))
        else {
            panic!("choosing asks");
        };
        assert_eq!(
            first.query.facets,
            vec![r#"categories = "technology""#.to_string()],
            "one chosen category is one part"
        );

        // Two chosen categories *both* hold, so they are joined into the one
        // facet string -- two groups would be an "or", which is a different
        // question.
        let Some(Ask::Search(second)) =
            state.update(Message::Category("adventure".to_string()))
        else {
            panic!("the second choice asks");
        };
        assert_eq!(second.query.facets.len(), 1, "one group, not two");
        assert!(
            second.query.facets[0].contains(" AND "),
            "and the parts are joined with AND: {}",
            second.query.facets[0]
        );
        assert_eq!(state.page, 1, "a different set of results starts at the first page");

        // Unchoosing asks too, and the facet goes with it.
        let Some(Ask::Search(third)) = state.update(Message::Category("adventure".to_string()))
        else {
            panic!("unchoosing asks");
        };
        assert_eq!(third.query.facets, vec![r#"categories = "technology""#.to_string()]);

        // The order the rows were pressed in cannot change the request, or the
        // engine's cache would hold two entries for one question.
        let mut other = State::new(ProjectType::Modpack);
        other.update(Message::Category("adventure".to_string()));
        other.update(Message::Category("technology".to_string()));
        assert_eq!(
            other.request().facets,
            second.query.facets,
            "the same two categories are the same request whichever order they were pressed in"
        );

        // And opening a section is not a change to the request at all.
        assert_eq!(state.update(Message::Section("technical".to_string())), None);
        assert!(state.touched.contains("technical"));
        assert_eq!(state.request().facets, vec![r#"categories = "technology""#.to_string()]);
    }

    #[test]
    fn the_category_sections_are_the_headers_the_tab_has_and_all_of_them_start_open() {
        let mut state = State::new(ProjectType::Modpack);
        state.update(Message::Tags {
            result: Ok(palantir_net::Tags {
                categories: vec![
                    palantir_net::CategoryTag {
                        name: "kitchen-sink".to_string(),
                        project_type: "modpack".to_string(),
                        header: "technical".to_string(),
                    },
                    palantir_net::CategoryTag {
                        name: "adventure".to_string(),
                        project_type: "modpack".to_string(),
                        header: "gameplay".to_string(),
                    },
                    palantir_net::CategoryTag {
                        name: "optimization".to_string(),
                        project_type: "mod".to_string(),
                        header: "technology".to_string(),
                    },
                ],
                ..palantir_net::Tags::default()
            }),
        });
        assert!(state.touched.is_empty(), "every category section starts open");
        for theme in Gen::ALL {
            drop(sidebar(*theme, &state));
        }
        // Closing one and the tab's own headers both still draw.
        state.update(Message::Section("technical".to_string()));
        drop(sidebar(Gen::Dark, &state));
        // And a page with no tag list yet draws the sidebar rather than failing.
        drop(sidebar(Gen::Dark, &State::new(ProjectType::Modpack)));
    }

    #[test]
    fn the_hide_installed_switch_is_a_control_the_request_carries() {
        // It changes what the API is asked rather than which rows are drawn, so
        // it asks like the sort and the view size do -- and it asks with the
        // *flag*, because which projects are hidden is the shell's to read (they
        // are the instances it holds).
        let mut state = State::new(ProjectType::Modpack);
        assert!(!state.hide_installed, "the reference opens on the flag's default");
        let Some(Ask::Search(asked)) = state.update(Message::HideInstalled(true)) else {
            panic!("the switch asks");
        };
        assert!(asked.hide_installed);
        assert!(asked.query.facets.is_empty(), "the page does not name the projects");
        assert_eq!(state.page, 1, "a different set of results starts at the first page");

        // Pressing it again is not a change.
        assert_eq!(state.update(Message::HideInstalled(true)), None);

        // And it draws on the tab the reference's `showHideInstalled` covers,
        // which is the modpack one: two of its three arms are contexts this shell
        // has no route into.
        let modpack = State::new(ProjectType::Modpack);
        drop(sidebar(Gen::Dark, &modpack));
        for kind in [ProjectType::Mod, ProjectType::Shader, ProjectType::Server] {
            let other = State::new(kind);
            drop(sidebar(Gen::Dark, &other));
        }
    }

    #[test]
    fn the_panels_last_block_is_the_only_one_without_a_rule_under_it() {
        // `filterClass`'s `last:border-b-0`: the rule belongs to the block that
        // follows, so a panel of one block has no rule at all and a panel of
        // three has two -- not three, which is what drawing a rule after each
        // block leaves on screen.
        assert!(!rule_under(0, 1), "the only block in the panel has nothing under it");
        assert!(rule_under(0, 3), "the first of three does");
        assert!(rule_under(1, 3), "and the second");
        assert!(!rule_under(2, 3), "and not the last");
        assert!(!rule_under(0, 0), "an empty panel draws nothing to divide");

        // And the panel still draws either way: the switch alone, the switch and
        // a section, and a tab that has neither.
        drop(sidebar(Gen::Dark, &State::new(ProjectType::Mod)));
        let mut modpack = State::new(ProjectType::Modpack);
        modpack.update(Message::Tags {
            result: Ok(palantir_net::Tags {
                categories: vec![palantir_net::CategoryTag {
                    name: "kitchen-sink".to_string(),
                    project_type: "modpack".to_string(),
                    header: "technical".to_string(),
                }],
                ..palantir_net::Tags::default()
            }),
        });
        drop(sidebar(Gen::Dark, &modpack));
    }

    #[test]
    fn a_resolutions_row_is_an_alternative_to_its_neighbour_and_not_an_addition() {
        // `search.ts`: `method: category.header === 'resolutions' ? 'or' : 'and'`.
        assert!(alternatives(RESOLUTIONS_HEADER));
        assert!(!alternatives("technical"));
        assert!(!alternatives("gameplay"));
        assert!(!alternatives(""), "and a header nobody knows is the default");

        let mut state = State::new(ProjectType::ResourcePack);
        state.update(Message::Tags {
            result: Ok(palantir_net::Tags {
                categories: vec![
                    palantir_net::CategoryTag {
                        name: "1080p".to_string(),
                        project_type: "resourcepack".to_string(),
                        header: RESOLUTIONS_HEADER.to_string(),
                    },
                    palantir_net::CategoryTag {
                        name: "16x".to_string(),
                        project_type: "resourcepack".to_string(),
                        header: RESOLUTIONS_HEADER.to_string(),
                    },
                    palantir_net::CategoryTag {
                        name: "fantasy".to_string(),
                        project_type: "resourcepack".to_string(),
                        header: "style".to_string(),
                    },
                ],
                ..palantir_net::Tags::default()
            }),
        });

        let Some(Ask::Search(one)) = state.update(Message::Category("1080p".to_string())) else {
            panic!("choosing asks");
        };
        assert_eq!(one.query.facets, vec![r#"categories = "1080p""#.to_string()]);

        // A second resolution is one more way to answer the same question, so it
        // joins the first into a single `IN` part -- asking for a pack that is
        // both 1080p and 16x would find nothing, which is the point.
        let Some(Ask::Search(two)) = state.update(Message::Category("16x".to_string())) else {
            panic!("the second resolution asks");
        };
        assert_eq!(
            two.query.facets,
            vec![r#"categories IN ["1080p", "16x"]"#.to_string()]
        );

        // A category of another header is a second thing that has to hold, and it
        // stays a part of its own in front of the group, which is the order
        // `newFilters` builds them in.
        let Some(Ask::Search(three)) = state.update(Message::Category("fantasy".to_string()))
        else {
            panic!("the other header asks");
        };
        assert_eq!(
            three.query.facets,
            vec![r#"categories = "fantasy" AND categories IN ["1080p", "16x"]"#.to_string()]
        );

        // And the same choice is the same request whichever order it was made in.
        let mut other = State::new(ProjectType::ResourcePack);
        other.update(Message::Tags {
            result: Ok(state.tags.ready().expect("kept").clone()),
        });
        other.update(Message::Category("16x".to_string()));
        other.update(Message::Category("fantasy".to_string()));
        other.update(Message::Category("1080p".to_string()));
        assert_eq!(other.request().facets, three.query.facets);
    }

    #[test]
    fn the_sidebar_stacks_the_sections_search_ts_stacks_and_opens_the_two_it_names() {
        // `getFilterOpenByDefault`'s app arm: an id that starts with `category`,
        // plus `environment` and `license`. Nothing else, which is why a modpack
        // tab's *Loader* is a header until it is pressed.
        for kind in [ProjectType::Modpack, ProjectType::Mod, ProjectType::Shader] {
            let ids: Vec<&str> = filters_for(kind).iter().map(|filter| filter.id).collect();
            let opens: Vec<&str> = filters_for(kind)
                .iter()
                .filter(|filter| filter.opens())
                .map(|filter| filter.id)
                .collect();
            // `environment` is offered for mods and modpacks only, so a shader
            // tab opens its License and nothing else.
            let expected: Vec<&str> = if kind == ProjectType::Shader {
                vec![LICENSE]
            } else {
                vec![ENVIRONMENT, LICENSE]
            };
            assert_eq!(opens, expected, "environment and license open, and only those ({kind:?})");
            // The first section is `search.ts`'s first for the tab, except on the
            // mod tab, where `mod_loader`'s `ordering: 1` puts it above the rest --
            // which is what the one non-zero ordering in the table is for.
            assert_eq!(
                ids.first().copied(),
                match kind {
                    ProjectType::Mod | ProjectType::Modpack => Some(GAME_VERSION),
                    ProjectType::Shader => Some(SHADER_LOADER),
                    _ => Some(ENVIRONMENT),
                },
                "and the first section is the one that sorts first"
            );
            assert!(
                !ids.contains(&MODPACK_LOADER) || kind == ProjectType::Modpack,
                "a loader section is the tab's own"
            );
        }

        // A section nobody has pressed draws its default; one press is the other
        // way, and a second press is back where it started.
        let mut state = State::new(ProjectType::Modpack);
        assert!(state.is_open(LICENSE, true), "license starts open");
        assert!(!state.is_open(MODPACK_LOADER, false), "the loaders start shut");
        assert_eq!(state.update(Message::Section(MODPACK_LOADER.to_string())), None,
            "and opening one is not a request");
        assert!(state.is_open(MODPACK_LOADER, false));
        state.update(Message::Section(MODPACK_LOADER.to_string()));
        assert!(!state.is_open(MODPACK_LOADER, false), "and a second press shuts it");

        // The tag document's own loaders are the rows of the modpack tab's
        // section, and the ones it does not offer are not.
        state.update(Message::Tags {
            result: Ok(palantir_net::Tags {
                loaders: vec![
                    palantir_net::LoaderTag {
                        name: "fabric".to_string(),
                        supported_project_types: vec!["modpack".to_string()],
                    },
                    palantir_net::LoaderTag {
                        name: "quilt".to_string(),
                        supported_project_types: vec!["modpack".to_string()],
                    },
                    palantir_net::LoaderTag {
                        name: "paper".to_string(),
                        supported_project_types: vec!["plugin".to_string()],
                    },
                ],
                categories: vec![palantir_net::CategoryTag {
                    name: "kitchen-sink".to_string(),
                    project_type: "modpack".to_string(),
                    header: "technical".to_string(),
                }],
                ..palantir_net::Tags::default()
            }),
        });
        let tags = state.tags.ready().expect("the document");
        let loaders = SIDEBAR_FILTERS
            .iter()
            .find(|filter| filter.id == MODPACK_LOADER)
            .expect("the section is in the table");
        let rows = filter_rows(&state, loaders, ProjectType::Modpack, tags);
        assert_eq!(
            rows.iter().map(|row| row.id.as_str()).collect::<Vec<&str>>(),
            vec!["fabric", "quilt"],
            "and the API's own order, with no loader the tab does not offer"
        );
        for theme in Gen::ALL {
            drop(sidebar(*theme, &state));
        }
    }

    #[test]
    fn an_environment_row_is_the_groups_search_ts_groups_it_into() {
        // Neither row chosen is no part at all.
        assert!(environment_groups(false, false).is_empty());
        let Some(Ask::Search(none)) = state_of(ProjectType::Modpack).update(Message::Filter {
            filter: ENVIRONMENT.to_string(),
            option: "client".to_string(),
        }) else {
            panic!("choosing asks");
        };
        assert_eq!(
            none.query.facets,
            vec![concat!(
                r#"(environment = "client_only" OR environment = "client_only_server_optional""#,
                r#" OR environment = "client_or_server_prefers_both" OR environment = "client_or_server")"#
            )
            .to_string()],
            "client alone is the four values that make a project a client"
        );

        // Both rows are one part, and it is not the union of the two: the
        // reference hands the pair to `getEnvironmentFilterGroups`, which answers
        // a list of its own.
        let mut state = state_of(ProjectType::Modpack);
        state.update(Message::Filter {
            filter: ENVIRONMENT.to_string(),
            option: "server".to_string(),
        });
        let Some(Ask::Search(both)) = state.update(Message::Filter {
            filter: ENVIRONMENT.to_string(),
            option: "client".to_string(),
        }) else {
            panic!("the second row asks");
        };
        assert_eq!(both.query.facets.len(), 1, "one part");
        assert_eq!(
            both.query.facets[0].matches("environment = ").count(),
            5,
            "and it is the pair's own list: {}",
            both.query.facets[0]
        );

        // The license row and two loaders beside a category: an `'and'` part
        // first, then the `orGroups` the reference collects.
        let mut mixed = state_of(ProjectType::Modpack);
        mixed.update(Message::Category("technology".to_string()));
        mixed.update(Message::Filter {
            filter: MODPACK_LOADER.to_string(),
            option: "quilt".to_string(),
        });
        mixed.update(Message::Filter {
            filter: LICENSE.to_string(),
            option: OPEN_SOURCE.to_string(),
        });
        assert_eq!(
            mixed.request().facets,
            vec!["categories = \"technology\" AND open_source = true AND categories = \"quilt\""
            .to_string()],
            "and the parts are in `newFilters`' own order"
        );

        // Two loaders are alternatives, so they join into one list rather than
        // asking for a modpack that is both.
        mixed.update(Message::Filter {
            filter: MODPACK_LOADER.to_string(),
            option: "fabric".to_string(),
        });
        assert_eq!(
            mixed.request().facets,
            vec!["categories = \"technology\" AND open_source = true AND categories IN [\"fabric\", \"quilt\"]"
            .to_string()]
        );

        // And the same set of rows is the same string whichever order they were
        // pressed in.
        let mut other = state_of(ProjectType::Modpack);
        other.update(Message::Filter {
            filter: LICENSE.to_string(),
            option: OPEN_SOURCE.to_string(),
        });
        other.update(Message::Filter {
            filter: MODPACK_LOADER.to_string(),
            option: "fabric".to_string(),
        });
        other.update(Message::Filter {
            filter: MODPACK_LOADER.to_string(),
            option: "quilt".to_string(),
        });
        other.update(Message::Category("technology".to_string()));
        assert_eq!(other.request().facets, mixed.request().facets);
    }

    /// A page on `kind`, for the tests that only care about what a row asks for.
    fn state_of(kind: ProjectType) -> State {
        State::new(kind)
    }

    #[test]
    fn a_page_that_has_not_asked_yet_is_waiting_rather_than_empty() {
        // The page opens unasked, which draws as "Loading results…" rather than as
        // "no results": the two are different sentences and only one of them is
        // true of a request nobody has made.
        let mut state = State::new(ProjectType::Modpack);
        assert_eq!(state.results, Load::Idle);
        assert!(state.results.waiting());
        assert_ne!(state.results, Load::Empty);

        // And the request it owes is its own controls, read as a query: the tab as
        // the project type, the sort as the API's index, the view size as the
        // limit, the page as the offset.
        let asked = opening_search(&mut state);
        assert_eq!(asked.round, 1);
        assert_eq!(asked.query.query, "");
        assert_eq!(asked.query.project_type.as_deref(), Some("modpack"));
        assert_eq!(asked.query.index.as_deref(), Some("relevance"));
        assert_eq!(asked.query.limit, 20);
        assert_eq!(asked.query.offset, 0);
        assert_eq!(state.results, Load::Loading);
        // Asking is once: a page that has asked does not ask again on every
        // message the shell handles.
        assert!(state.opening().is_none(), "asked once, and the refresh button is what asks again");
    }

    #[test]
    fn the_tab_strip_is_the_pill_alone() {
        // `browse-tab/layout.vue` passes `<NavTabs>` no `pageNav`, so
        // `NavTabs.vue:3` renders its outer element as `contents` and the page
        // shows the `nav` itself: `w-fit rounded-full bg-bg-raised`, as wide as
        // its tabs. A capture of the reference at `/browse/modpack` measures the
        // raised surface at x 88..723 and the page's own background at x 730 and
        // x 900, so there is no band behind the pill to draw.
        //
        // What the strip adds to the pill is the page's inset and nothing else:
        // [`INSET`] to the left, [`STRIP_ABOVE`] above and [`STRIP_UNDER`]
        // below. An earlier draft wrapped the pill in a `Length::Fill` container
        // painted `--bg-raised` and put a one-pixel rule and an eight-row
        // gradient under it, which painted a 914-pixel band where the reference
        // has a 636-pixel pill -- and put the pill's own height wrong twice over,
        // since the band and the rule under it were not the pill's border at all.
        assert_eq!(INSET, 24.0, "the page's own inset, left of the pill");
        assert_eq!(STRIP_ABOVE, 24.0, "page background above the pill, hairline row included");
        assert_eq!(STRIP_UNDER, 8.0, "and the gap its shadow fades into");
        // The pill keeps its own frame and its own shadow, which are what the
        // reference draws around the tabs: `card-shadow border border-solid
        // border-surface-4` on the same element (`NavTabs.vue:11`).
        assert_eq!(ui::TAB_STRIP, 46.0, "1 + 4 + 36 + 4 + 1, borders included");
        assert_eq!(ui::TAB_STRIP_BORDER, 1.0, "the pill's own border");
        // And nothing is left over from the band that used to be here.
        assert_eq!(GAP, 12.0, "the gap the pages below still use");
    }

    #[test]
    fn the_search_field_is_the_size_its_own_template_sets() {
        // `browse-tab/layout.vue`'s `<Input size="large">`, which is `h-12`:
        // forty-eight pixels at `px-4` and a `rounded-[14px]` frame. It is the
        // one search field in the tree that is not the default -- the library's,
        // the screenshots page's and the creation flow's are all `standard` or
        // unstated -- so a field drawn at `ui::CONTROL` (40) is forty pixels of
        // `h-10` where the template asks for `h-12`.
        assert_eq!(ui::InputSize::Large.height(), 48.0);
        assert_eq!(ui::InputSize::Large.pad(), 16.0);
        assert_eq!(ui::InputSize::Large.radius(), 14.0);
        // And the row that is *not* the large one, so the distinction is a
        // distinction rather than a single number asserted twice.
        assert_eq!(ui::InputSize::Standard.height(), 36.0);
        assert_eq!(ui::CONTROL, 40.0, "what this drew before the size was read off");
    }

    #[test]
    fn the_two_gaps_below_the_strip_are_not_the_same_gap() {
        // The column that stacks the search field, the controls row and the
        // results puts [`GAP`] between each pair, and the reference does not: its
        // two joins measure eight and twelve on the same 1280x720 capture, the
        // difference being the `mt-1` the results block carries. Drawn with one
        // gap the trigger sat at y=186 instead of y=182 and the results surface
        // came out at y=238 instead of y=230, so both were low by four and the
        // eight was invisible until the numbers were written down.
        assert_eq!(SEARCH_TO_CONTROLS, 8.0, "field to trigger, on the capture");
        assert_eq!(GAP, 12.0, "trigger to results, which is this plus the `mt-1`");
        assert_ne!(
            SEARCH_TO_CONTROLS, GAP,
            "one gap for the whole column is what put the trigger four rows low"
        );
    }

    #[test]
    fn the_column_lands_where_the_reference_puts_each_row() {
        // The arithmetic, from the head's hairline on y=48 down to the results'
        // own rule. Every number on the right was read off a 1280x720 capture of
        // the reference at `/browse/mod`; this is what says the three gaps and
        // the three heights compose to them, so a change to any one of the six
        // that is wrong on its own still fails here.
        let field_top = 48.0 + STRIP_ABOVE + ui::TAB_STRIP + STRIP_UNDER;
        let field_bottom = field_top + ui::InputSize::Large.height() - 1.0;
        let trigger_top = field_bottom + 1.0 + SEARCH_TO_CONTROLS;
        let trigger_bottom = trigger_top + ui::TRIGGER_HEIGHT - 1.0;
        let results_top = trigger_bottom + 1.0 + GAP;
        assert_eq!(field_top, 126.0, "the capture puts the field's border on y=126");
        assert_eq!(field_bottom, 173.0, "and its last row on y=173");
        assert_eq!(trigger_top, 182.0, "the capture puts the trigger's border on y=182");
        assert_eq!(trigger_bottom, 217.0, "and its last row on y=217");
        assert_eq!(results_top, 230.0, "the capture puts the results' rule on y=230");
    }

    #[test]
    fn the_controls_and_the_request_are_the_same_thing_read_twice() {
        let mut state = State::new(ProjectType::Shader);
        state.update(Message::Query("  complementary  ".to_string()));
        state.update(Message::Sort(Sort::DateUpdated));
        state.update(Message::View(50));
        state.update(Message::Page(3));
        let request = state.request();
        // The query is trimmed, because a search for trailing spaces is a
        // different cache key for the same question.
        assert_eq!(request.query, "complementary");
        assert_eq!(request.project_type.as_deref(), Some("shader"));
        assert_eq!(request.index.as_deref(), Some("updated"));
        assert_eq!(request.limit, 50);
        assert_eq!(request.offset, 100, "the third page of fifty is a hundred rows in");
    }

    #[test]
    fn an_answer_lands_on_the_question_it_was_asked() {
        let mut state = State::new(ProjectType::Modpack);
        let first = opening_search(&mut state);
        state.update(Message::Found { round: first.round, result: Ok(vec![hit("Sodium")]) });
        assert_eq!(state.results, Load::Ready(vec![hit("Sodium")]));

        // An empty answer is `Empty` -- the reference's own "no results" -- and
        // not an empty list.
        state.update(Message::Found { round: first.round, result: Ok(Vec::new()) });
        assert_eq!(state.results, Load::Empty);

        // A failure carries the reason it failed.
        state.update(Message::Found { round: first.round, result: Err("404 for the search".into()) });
        assert_eq!(state.results, Load::Failed("404 for the search".into()));

        // A slow answer to a question the page has replaced is dropped rather than
        // drawn: the newer request's results are the ones that match the controls
        // on screen.
        let Some(Ask::Search(second)) = state.update(Message::Search) else {
            panic!("the message asks again");
        };
        assert!(second.round > first.round);
        state.update(Message::Found { round: first.round, result: Ok(vec![hit("Stale")]) });
        assert_eq!(state.results, Load::Loading, "the stale answer changed nothing");
        state.update(Message::Found { round: second.round, result: Ok(vec![hit("Fresh")]) });
        assert_eq!(state.results, Load::Ready(vec![hit("Fresh")]));
    }

    #[test]
    fn a_tab_change_puts_the_page_back_to_unasked_and_to_the_first_page() {
        // The results on screen belong to the tab the user has left, so they go;
        // the shell asks for the new tab on the way out of the message.
        let mut state = State::new(ProjectType::Modpack);
        opening_search(&mut state);
        state.update(Message::Found { round: 1, result: Ok(vec![hit("Sodium")]) });
        // A page change is a request of its own now -- the reference's watcher
        // refreshes on it -- so that is round 2, and the tab change below is 3.
        state.update(Message::Page(4));
        assert_eq!(state.update(Message::ProjectType(ProjectType::Mod)), None);
        assert_eq!(state.page, 1);
        assert_eq!(state.results, Load::Idle);
        let Some(Ask::Search(asked)) = state.opening() else {
            panic!("the new tab asks for its own results");
        };
        assert_eq!(asked.round, 3);
        assert_eq!(asked.query.project_type.as_deref(), Some("mod"));
        // And the tab already on screen is not a change, so nothing is thrown away.
        state.update(Message::Found { round: asked.round, result: Ok(vec![hit("Sodium")]) });
        state.update(Message::ProjectType(ProjectType::Mod));
        assert_eq!(state.results, Load::Ready(vec![hit("Sodium")]));
    }

    #[test]
    fn late_icons_cannot_repopulate_the_cache_for_old_results() {
        let mut state = State::new(ProjectType::Modpack);
        let mut current = hit("Current");
        current.icon_url = "https://icons/current.png".into();
        state.results = Load::Ready(vec![current]);
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            2, 2, image::Rgba([10, 20, 30, 255]),
        )).write_to(&mut png, image::ImageOutputFormat::Png).unwrap();
        let icon = Icon::of(png.get_ref(), avatar::ICON_SIDE).unwrap();
        state.update(Message::Icons { arrived: vec![
            Fetched { url: "https://icons/old.png".into(), icon: icon.clone() },
            Fetched { url: "https://icons/current.png".into(), icon: icon.clone() },
        ] });
        assert_eq!(state.icons.len(), 1);
        assert!(state.icons.contains_key("https://icons/current.png"));
        state.results = Load::Loading;
        state.update(Message::Icons { arrived: vec![
            Fetched { url: "https://icons/old.png".into(), icon },
        ] });
        assert!(!state.icons.contains_key("https://icons/old.png"));
    }

    #[test]
    fn a_card_is_read_out_of_the_api_s_hit() {
        let api = ModrinthSearchHit {
            project_id: "AANobbMI".to_string(),
            slug: "sodium".to_string(),
            title: "Sodium".to_string(),
            description: "Modern rendering engine".to_string(),
            author: "jellysquid".to_string(),
            downloads: 41_000_000,
            follows: 9_000,
            icon_url: "https://cdn.modrinth.com/icon.png".to_string(),
            latest_version: "mc1.21.4-0.6.5".to_string(),
            versions: vec!["1.21.4".to_string()],
            categories: vec!["fabric".to_string()],
        };
        let card = Hit::from_api(&api);
        assert_eq!(card.id, "AANobbMI", "the id, because the route follows it");
        assert_eq!(card.title, "Sodium");
        assert_eq!(card.summary, "Modern rendering engine");
        assert_eq!(card.game_versions, vec!["1.21.4"]);
        assert_eq!(card.loaders, vec!["fabric"]);
        // The icon travels with the card, because the card is what asks for it.
        assert_eq!(card.icon_url, "https://cdn.modrinth.com/icon.png");
        // A hit with no id falls back to the slug, which is the other thing the
        // route accepts.
        let slug_only = ModrinthSearchHit { project_id: String::new(), ..api };
        assert_eq!(Hit::from_api(&slug_only).id, "sodium");
    }

    #[test]
    fn the_page_draws_in_every_theme_with_and_without_results() {
        let store = Store::default();
        for theme in Gen::ALL {
            let mut state = State::new(ProjectType::Modpack);
            drop(view(*theme, &state, &store));
            state.results = Load::Ready(vec![hit("Sodium"), hit("Lithium")]);
            drop(view(*theme, &state, &store));
            state.results = Load::Ready(Vec::new());
            drop(view(*theme, &state, &store));
            state.results = Load::Loading;
            drop(view(*theme, &state, &store));
            state.results = Load::Empty;
            drop(view(*theme, &state, &store));
        }
    }

    #[test]
    fn the_region_s_report_is_what_the_window_is_computed_from() {
        // The seam this slice is: the page draws a window, and the only thing
        // that moves it is the report its own scroll region publishes. A page
        // that kept no report would draw the same cards however far the reader
        // had scrolled -- which is the failure that looks like a stuck list.
        let mut state = State::new(ProjectType::Modpack);
        assert_eq!(state.geometry, Geometry::default(), "nothing has reported yet");
        state.update(Message::Scrolled(Geometry { offset: 10_000.0, view_height: 600.0 }));
        assert_eq!(state.geometry.offset, 10_000.0);

        // The list begins below the page's head, so its own offset is the
        // page's less that: the report is ten thousand into the page and 9,819
        // into the list.
        let at = list_at(state.geometry);
        assert_eq!(at.offset, 10_000.0 - LIST_TOP);
        assert_eq!(at.view_height, 600.0);

        // And the window is a screenful plus the margin either side of it, not
        // the whole result set: five thousand cards, of which this frame builds
        // twenty.
        let window = scroll::window(5_000, CARD_ROW, at);
        assert_eq!(
            window.start,
            ((10_000.0 - LIST_TOP) / CARD_ROW).floor() as usize - scroll::OVERSCAN
        );
        let slots = (600.0_f32 / CARD_ROW).ceil() as usize;
        assert_eq!(window.len(), (slots + scroll::OVERSCAN * 2).max(scroll::INITIAL_ROWS));
        assert!(window.end < 100, "{}", window.end);
    }

    #[test]
    fn a_region_that_has_not_reached_the_list_opens_it_at_its_top() {
        // The page opens with its head on screen: what a wheel up there has
        // scrolled past is the head, not the list, and a window computed from
        // the raw offset would already be a row into a list nobody has reached.
        for offset in [0.0, 90.0, LIST_TOP] {
            let at = list_at(Geometry { offset, view_height: 600.0 });
            assert_eq!(at.offset, 0.0, "offset {offset} is still the head");
            assert_eq!(at.view_height, 600.0);
        }
        let past = list_at(Geometry { offset: LIST_TOP + 500.0, view_height: 600.0 });
        assert_eq!(past.offset, 500.0);
    }

    #[test]
    fn a_card_is_exactly_one_row_of_the_window() {
        // The window's arithmetic is only as good as the row it is handed: a
        // card is the reference's own 142-pixel box and a row is that plus the
        // `gap-3` between two cards. The capture the head was measured on
        // (1280x720, 2026-10-02) has cards at y=231, 385, 539 and 693 -- a
        // pitch of 154 -- and the first of them at 181 into the region.
        assert_eq!(CARD_HEIGHT, 142.0);
        assert_eq!(CARD_INNER, 110.0, "two p-4 paddings leave the interior");
        assert_eq!(CARD_INNER, CARD_HEIGHT - 2.0 * ui::CARD_PAD);
        assert_eq!(CARD_ROW, 154.0);
        assert_eq!(CARD_ROW - CARD_HEIGHT, GAP);
        assert_eq!(LIST_TOP, 230.0 - 49.0);
        // And the interior is what the card's parts are drawn in: an icon or a
        // tag row taller than it would push the card past its measured height.
        assert!(CARD_INNER >= avatar::ICON_SIDE as f32);
        assert!(CARD_INNER >= ui::TAG_HEIGHT);
    }

    #[test]
    fn the_reference_s_loading_blocks_are_what_the_page_draws_while_it_waits() {
        // `base/LoadingIndicator.vue`: three `h-16` blocks at `opacity-25` in
        // `--color-raised-bg`, under a bold `Loading...`. They are what a frame
        // builds for `Idle` and `Loading` alike -- the page opens on the blocks
        // rather than on a sentence about asking.
        for theme in Gen::ALL {
            drop(loading(*theme));
        }
        assert_eq!(LOADING_BLOCK, 64.0);
        assert_eq!(LOADING_BLOCK_OPACITY, 0.25);
        // The label is `position: absolute`, so it spends no height: three blocks
        // and the two gaps between them is the whole stack. Drawing it above the
        // blocks -- what this port used to do -- added its 24-pixel line and a
        // third gap, 32 pixels, and every block sat that much too low.
        assert_eq!(LOADING_STACK, 208.0);
        assert_eq!(LOADING_STACK, LOADING_BLOCK * 3.0 + ROW_GAP * 2.0);
        assert_eq!(
            LOADING_LABEL_BLOCK, 1,
            "an auto-inset absolute child of a `justify-center` flex column \
             lands on the stack's centre, which is the middle block"
        );
        // And every block draws, with and without the label on it.
        for theme in Gen::ALL {
            drop(loading_block(*theme, None));
            drop(loading_block(
                *theme,
                Some(text("Loading...".to_string()).into()),
            ));
        }
        let store = Store::default();
        let waiting = State::new(ProjectType::Modpack);
        assert!(waiting.results.waiting());
        drop(view(Gen::Dark, &waiting, &store));
    }

    #[test]
    fn five_thousand_cards_draw_a_window_in_every_theme_and_scroll_state() {
        // What the window is for: a card set of five thousand costs a screenful
        // and the margin, drawn in the two states a frame can be in -- before
        // the region has reported (the window-sized guess) and deep in a list
        // that no element of which is anywhere near the screen -- for every
        // theme a frame can be drawn in.
        let store = Store::default();
        let hits: Vec<Hit> = (0..5_000)
            .map(|index| Hit {
                id: format!("p{index:05}"),
                title: format!("Project {index}"),
                ..hit("Sodium")
            })
            .collect();
        for theme in Gen::ALL {
            let mut state = State::new(ProjectType::Modpack);
            state.results = Load::Ready(hits.clone());
            for at in [
                Geometry::default(),
                Geometry { offset: 0.0, view_height: 600.0 },
                Geometry { offset: 200_000.0, view_height: 600.0 },
            ] {
                state.update(Message::Scrolled(at));
                drop(view(*theme, &state, &store));
            }
        }
    }

    #[test]
    fn a_hit_card_draws_the_counts_the_reference_shows_on_one() {
        // The reference's stat is the *compact* count -- `formatCompactNumber` --
        // and the full one, which the tooltip messages still carry, is a hover label
        // it draws and this kit does not. (`crate::locale`'s own test has the rule's
        // cases; this is the card asking for that rule rather than for a sentence.)
        assert_eq!(locale::compact("en", 12_345), "12.3K");
        drop(hit_card(Gen::Dark, &Hit { downloads: 0, follows: 0, ..hit("Empty") }, None));
        // And a card with an icon draws it: the handle travels with the hit, and the
        // box is the same size either way.
        let icon = an_icon(SODIUM_ICON);
        let with_icon = Hit { icon_url: SODIUM_ICON.to_string(), ..hit("Sodium") };
        drop(hit_card(Gen::Dark, &with_icon, Some(&icon.icon)));
    }

    #[test]
    fn the_icons_of_a_page_of_results_are_asked_for_once_and_each_one_alone() {
        let mut state = State::new(ProjectType::Modpack);
        let first = opening_search(&mut state);
        let hits = vec![
            Hit { icon_url: SODIUM_ICON.to_string(), ..hit("Sodium") },
            Hit { icon_url: LITHIUM_ICON.to_string(), ..hit("Lithium") },
            // A project with no icon is not asked for: there is nothing to fetch,
            // and its card draws the empty box the reference's placeholder sits on.
            hit("Phosphor"),
            // The same icon twice in one page is one fetch: a URL names a file.
            Hit { icon_url: SODIUM_ICON.to_string(), ..hit("Sodium Extra") },
        ];
        assert_eq!(
            state.update(Message::Found { round: first.round, result: Ok(hits.clone()) }),
            Some(Ask::Icons(Icons {
                urls: vec![SODIUM_ICON.to_string(), LITHIUM_ICON.to_string()],
            }))
        );

        // Both arrive, and the page keeps them by URL.
        assert_eq!(
            state.update(Message::Icons {
                arrived: vec![an_icon(SODIUM_ICON), an_icon(LITHIUM_ICON)],
            }),
            None
        );
        assert_eq!(state.icons.len(), 2);

        // The same search again: the same projects, so the same icons are already
        // here and nothing is asked for a second time.
        let Some(Ask::Search(again)) = state.update(Message::Search) else {
            panic!("the search is asked again");
        };
        assert_eq!(state.update(Message::Found { round: again.round, result: Ok(hits) }), None);
        assert_eq!(state.icons.len(), 2, "an icon already held is not fetched twice");

        // A different page of results keeps its own icons and drops the rest: what
        // the map holds is what the cards on screen can look up.
        let Some(Ask::Search(third)) = state.update(Message::Search) else {
            panic!("a third search");
        };
        let fresh = Hit { icon_url: "https://cdn.modrinth.com/new.png".to_string(), ..hit("New") };
        assert_eq!(
            state.update(Message::Found { round: third.round, result: Ok(vec![fresh]) }),
            Some(Ask::Icons(Icons {
                urls: vec!["https://cdn.modrinth.com/new.png".to_string()],
            }))
        );
        assert!(state.icons.is_empty(), "the icons of the results that are gone went with them");

        // And an answer to a search the page has replaced is dropped whole: it is
        // not asked for icons for results it does not hold.
        assert_eq!(
            state.update(Message::Found { round: first.round, result: Ok(vec![hit("Stale")]) }),
            None
        );
    }

    #[test]
    fn a_section_points_its_chevron_up_only_while_it_is_open() {
        // `Accordion.vue`'s header carries `:class="{'rotate-180': isOpen}"` on
        // its `DropdownIcon`, so the shut chevron and the open one are the same
        // path turned over. A glyph cannot be turned over here, which is what
        // `chevron` stands in for -- and what would be lost by drawing one
        // chevron for both states is the reference's own cue for which way the
        // section opens.
        assert_eq!(chevron(false), Glyph::Dropdown, "shut, the path as it is in the set");
        assert_eq!(chevron(true), Glyph::ChevronUp, "open, the set's own upward chevron");

        // The press that turns it over is the section's, and one press is
        // enough: the state is the sections whose open state the reader has
        // changed, so a second press is a change back rather than a second
        // change forward.
        let mut state = State::new(ProjectType::Mod);
        state.update(Message::Tags {
            result: Ok(palantir_net::Tags {
                categories: vec![palantir_net::CategoryTag {
                    name: "technology".to_string(),
                    project_type: "mod".to_string(),
                    header: "technical".to_string(),
                }],
                ..palantir_net::Tags::default()
            }),
        });
        let open = state.is_open("technical", true);
        assert_eq!(
            state.update(Message::Section("technical".to_string())),
            None,
            "opening a section is not a change to the request"
        );
        assert_ne!(state.is_open("technical", true), open, "and the press turned it over");
    }

    #[test]
    fn the_mod_tabs_loader_lists_its_defaults_until_show_more_is_pressed() {
        // `display: 'expandable'` with `default_values: DEFAULT_MOD_LOADERS`: the
        // three loaders the reference offers without being asked, and a press
        // that shows the rest of the tag document's list.
        let mut state = state_of(ProjectType::Mod);
        let tags = tags_with(&[("fabric", &["mod"]), ("forge", &["mod"]), ("quilt", &["mod"]), (
            "neoforge",
            &["mod"],
        )]);
        state.update(Message::Tags { result: Ok(tags) });

        let loader = SIDEBAR_FILTERS.iter().find(|f| f.id == MOD_LOADER).expect("the mod tab has a loader filter");
        assert_eq!(loader.display, Display::Expandable);
        assert_eq!(loader.defaults, DEFAULT_MOD_LOADERS);
        assert_eq!(
            filter_rows(&state, loader, ProjectType::Mod, state.tags.ready().expect("tags"))
                .iter()
                .map(|row| row.id.clone())
                .collect::<Vec<String>>(),
            vec!["fabric".to_string(), "forge".to_string(), "neoforge".to_string()],
            "three rows, the defaults, in the tag document's own order"
        );

        // A chosen loader that is not a default stays a row: `isVisible(option) ||
        // isIncluded(option)` keeps it on screen so it can be unchosen.
        state.update(Message::Filter { filter: MOD_LOADER.to_string(), option: "quilt".to_string() });
        assert_eq!(
            filter_rows(&state, loader, ProjectType::Mod, state.tags.ready().expect("tags"))
                .iter()
                .map(|row| row.id.clone())
                .collect::<Vec<String>>(),
            vec!["fabric".to_string(), "forge".to_string(), "neoforge".to_string(), "quilt".to_string()],
            "and the chosen one after them"
        );

        // *Show more* asks for nothing: the loaders it shows were options the
        // reader could have chosen from the start.
        assert_eq!(state.update(Message::Expand(MOD_LOADER.to_string())), None);
        assert_eq!(
            filter_rows(&state, loader, ProjectType::Mod, state.tags.ready().expect("tags"))
                .iter()
                .map(|row| row.id.clone())
                .collect::<Vec<String>>(),
            vec!["fabric".to_string(), "forge".to_string(), "neoforge".to_string(), "quilt".to_string()],
            "which for this list is every row the tag document has"
        );

        // The shader tab has its own filter with its own defaults, and a loader
        // that serves mods and plugins is not on the mod tab's list.
        let shader = SIDEBAR_FILTERS.iter().find(|f| f.id == SHADER_LOADER).expect("the shader tab has a loader filter");
        assert_eq!(shader.defaults, DEFAULT_SHADER_LOADERS);
        let mut plugin = state_of(ProjectType::Mod);
        plugin.update(Message::Tags {
            result: Ok(tags_with(&[("fabric", &["mod"]), ("paper", &["mod", "plugin"])])),
        });
        assert_eq!(
            filter_rows(&plugin, loader, ProjectType::Mod, plugin.tags.ready().expect("tags"))
                .iter()
                .map(|row| row.id.clone())
                .collect::<Vec<String>>(),
            vec!["fabric".to_string()],
            "`mod_loader` drops a loader that is also a plugin"
        );
        drop(show_more(Gen::Dark, MOD_LOADER, true));
    }

    #[test]
    fn a_resolution_and_a_loader_are_one_categories_group() {
        // `newFilters` keys `orGroups` by the *field* an option's value names, and
        // a loader's value is `categories:<name>` -- the same field a resolution
        // uses. So the two are one `IN`, not two parts joined with `AND`: the
        // reference returns a mod that is 16x *or* fabric, and asking for one
        // that is both would find fewer.
        let mut state = state_of(ProjectType::Mod);
        state.update(Message::Tags {
            result: Ok(palantir_net::Tags {
                categories: vec![
                    palantir_net::CategoryTag {
                        name: "16x".to_string(),
                        project_type: "mod".to_string(),
                        header: RESOLUTIONS_HEADER.to_string(),
                    },
                    palantir_net::CategoryTag {
                        name: "technology".to_string(),
                        project_type: "mod".to_string(),
                        header: "technical".to_string(),
                    },
                ],
                ..palantir_net::Tags::default()
            }),
        });
        state.update(Message::Category("16x".to_string()));
        state.update(Message::Filter { filter: MOD_LOADER.to_string(), option: "fabric".to_string() });
        state.update(Message::Category("technology".to_string()));
        let Some(Ask::Search(asked)) = state.update(Message::Search) else {
            panic!("the ask");
        };
        assert_eq!(
            asked.query.facets,
            vec![r#"categories = "technology" AND categories IN ["16x", "fabric"]"#.to_string()],
            "one and-part, then the one categories group, which is the order newFilters appends them in"
        );

        // And the mod tab's *Loader* sorts above *License*; the order of the sections
        // themselves is held by
        // `the_game_version_section_sorts_where_search_ts_says_it_does`.
    }

    #[test]
    fn an_exclusion_is_a_not_in_group_and_never_both_answers_at_once() {
        // `supports: ['include', 'exclude']` is on every category section and on
        // the loaders and the license, and the exclusion is the row's second
        // press: `newFilters` collects it into `negativeByType` and appends
        // `field NOT IN [...]` after the `orGroups`. It is a list even for one
        // value -- the one place the reference does not collapse to `= `.
        let mut state = state_of(ProjectType::Mod);
        state.update(Message::Tags {
            result: Ok(palantir_net::Tags {
                categories: vec![palantir_net::CategoryTag {
                    name: "technology".to_string(),
                    project_type: "mod".to_string(),
                    header: "technical".to_string(),
                }],
                ..tags_with(&[("fabric", &["mod"]), ("forge", &["mod"]), ("quilt", &["mod"]), (
                    "neoforge",
                    &["mod"],
                )])
            }),
        });

        let category = category_filter_id(ProjectType::Mod, "technical");
        let Some(Ask::Search(first)) =
            state.update(Message::Exclude { filter: category.clone(), option: "technology".to_string() })
        else {
            panic!("excluding asks");
        };
        assert_eq!(first.query.facets, vec![r#"categories NOT IN ["technology"]"#.to_string()]);

        // A loader excludes under the same field, and the two go into one group:
        // `newFilters` keys by field, so the list is one `NOT IN`, not two.
        let Some(Ask::Search(second)) =
            state.update(Message::Exclude { filter: MOD_LOADER.to_string(), option: "quilt".to_string() })
        else {
            panic!("excluding a loader asks");
        };
        assert_eq!(
            second.query.facets,
            vec![r#"categories NOT IN ["quilt", "technology"]"#.to_string()],
            "one group, ordered by the option id so the same choices are the same string"
        );

        // Choosing what is excluded is the same as not excluding it: the
        // reference's row carries one mark, and the press that includes drops the
        // exclusion.
        state.update(Message::Filter { filter: MOD_LOADER.to_string(), option: "quilt".to_string() });
        assert!(!state.is_excluded(MOD_LOADER, "quilt"), "choosing it unexcluded it");
        assert!(state.chosen(MOD_LOADER, "quilt"), "and it is chosen instead");

        // And the license's own field, with the boolean left unquoted as
        // `formatSearchFilterValue` leaves it.
        let Some(Ask::Search(third)) =
            state.update(Message::Exclude { filter: LICENSE.to_string(), option: OPEN_SOURCE.to_string() })
        else {
            panic!("excluding the license asks");
        };
        assert_eq!(
            third.query.facets,
            vec![concat!(
                r#"categories = "quilt""#,
                " AND categories NOT IN [\"technology\"]",
                " AND open_source NOT IN [true]",
            )
            .to_string()],
            "the and-part, then the categories group, then the license's own field"
        );

        // An excluded option stays a row even in a section that shows only its
        // defaults, so it can be unexcluded: `isVisible(option) || isExcluded(option)`.
        state.update(Message::Exclude { filter: MOD_LOADER.to_string(), option: "quilt".to_string() });
        let loader = SIDEBAR_FILTERS.iter().find(|f| f.id == MOD_LOADER).expect("the mod tab has a loader filter");
        let ids: Vec<String> =
            filter_rows(&state, loader, ProjectType::Mod, state.tags.ready().expect("tags"))
                .iter()
                .map(|row| row.id.clone())
                .collect();
        assert!(ids.contains(&"quilt".to_string()), "the excluded loader is still a row: {ids:?}");
        drop(sidebar(Gen::Dark, &state));
    }

    #[test]
    fn the_environment_rows_have_no_second_press() {
        // `search.ts`'s `environment` is `supports: ['include']`, the only filter
        // in the table that is: its two rows carry no exclude button at all, which
        // is what the reference draws beside them -- nothing.
        let mut state = state_of(ProjectType::Modpack);
        state.update(Message::Tags { result: Ok(tags_with(&[])) });
        let environment = SIDEBAR_FILTERS.iter().find(|f| f.id == ENVIRONMENT).expect("the filter exists");
        for row in filter_rows(&state, environment, ProjectType::Modpack, state.tags.ready().expect("tags")) {
            assert!(!row.excludes, "{} has no second press", row.id);
        }
        // And the loaders and the license do have one.
        let loaders = SIDEBAR_FILTERS.iter().find(|f| f.id == MODPACK_LOADER).expect("the filter exists");
        for row in filter_rows(&state, loaders, ProjectType::Modpack, state.tags.ready().expect("tags")) {
            assert!(row.excludes, "{} has one", row.id);
        }
        let license = SIDEBAR_FILTERS.iter().find(|f| f.id == LICENSE).expect("the filter exists");
        for row in filter_rows(&state, license, ProjectType::Modpack, state.tags.ready().expect("tags")) {
            assert!(row.excludes, "{} has one", row.id);
        }
    }

    #[test]
    fn the_game_version_section_lists_releases_until_the_box_is_ticked() {
        // `display: 'scrollable'`, `searchable: true`, and one `toggle_group`:
        // `toggle_group: gameVersion.version_type !== 'release' ? 'all_versions'
        // : undefined`, so a snapshot or a beta is a row only while the *Show all
        // versions* box is ticked. Both controls are refs in the reference and
        // neither moves the request.
        let mut state = state_of(ProjectType::Mod);
        state.update(Message::Tags {
            result: Ok(palantir_net::Tags {
                game_versions: vec![
                    palantir_net::GameVersionTag { version: "1.21.4".to_string(), version_type: "release".to_string() },
                    palantir_net::GameVersionTag { version: "24w14a".to_string(), version_type: "snapshot".to_string() },
                    palantir_net::GameVersionTag { version: "1.5.2".to_string(), version_type: "release".to_string() },
                ],
                ..tags_with(&[])
            }),
        });
        let tags = state.tags.ready().expect("tags").clone();
        let listed = |state: &State| version_ids(state, &tags);

        assert_eq!(listed(&state), vec!["1.21.4".to_string(), "1.5.2".to_string()], "the releases, in the document's own order");
        assert_eq!(state.update(Message::AllVersions(true)), None, "ticking the box asks for nothing");
        assert_eq!(listed(&state), vec!["1.21.4".to_string(), "24w14a".to_string(), "1.5.2".to_string()], "and the snapshot joins them, in place");

        // The field filters the rows and nothing else: the chosen versions stay
        // chosen, because they are not on screen to be unchosen from.
        state.update(Message::Version("1.21.4".to_string()));
        state.update(Message::VersionQuery("24".to_string()));
        assert_eq!(listed(&state), vec!["24w14a".to_string()], "one row matches the query");
        assert!(state.versions.contains("1.21.4"), "and the choice is still there");
        assert_eq!(state.update(Message::VersionQuery(String::new())), None, "clearing the field asks for nothing");

        // A chosen version asks, as one `or` group over its own field -- not
        // joined into `categories`, which is the field a category and a loader
        // share.
        let Some(Ask::Search(asked)) = state.update(Message::Version("24w14a".to_string())) else {
            panic!("choosing a version asks");
        };
        assert_eq!(
            asked.query.facets,
            vec![r#"game_versions IN ["1.21.4", "24w14a"]"#.to_string()],
            "two versions are one IN, and it is not mixed in with the categories group"
        );

        // The panel is a window of its own: 256 pixels at a 32-pixel pitch is
        // eight rows, and the window is drawn from the panel's own geometry, not
        // the page's.
        assert_eq!(VERSIONS_PANEL, 256.0);
        let at = Geometry { offset: 0.0, view_height: VERSIONS_PANEL };
        assert!(crate::scroll::window(40, VERSION_ROW, at).len() >= 8, "eight rows fit in the panel");
        assert!(crate::scroll::window(40, VERSION_ROW, list_at(at)).start <= 40);
        drop(versions(Gen::Dark, &state, &tags));
    }

    #[test]
    fn an_advanced_row_is_exclude_only_and_asks_a_disclosure_types_not_in() {
        // `advanced` is the one filter whose `supports` is `['exclude']` and not
        // `['include', 'exclude']`, so `primaryAction` is `'exclude'` from the
        // first frame: the row has no second button, and pressing it once asks.
        let mut state = state_of(ProjectType::Mod);
        state.update(Message::Tags { result: Ok(tags_with(&[])) });

        let rows = filter_rows(&state, filters_for(ProjectType::Mod).iter().find(|f| f.id == ADVANCED).expect("the advanced section"), ProjectType::Mod, &tags_with(&[]));
        assert!(
            rows.iter().all(|row| !row.chosen && !row.excludes),
            "no row can be chosen, and none of them offers a second press"
        );
        assert_eq!(
            rows.iter().map(|row| row.id.as_str()).collect::<Vec<&str>>(),
            vec![
                "ai_content",
                "ai_functionality",
                "advertisements",
                "epilepsy_triggers",
                "system_interactions",
                "telemetry",
                "paid_features",
                "archived",
            ],
            "`PROJECT_DISCLOSURE_TYPES` less `derivative_work`, which \
             `createDisclosureFilterOptions` drops"
        );

        let Some(Ask::Search(first)) =
            state.update(Message::Exclude { filter: ADVANCED.to_string(), option: "telemetry".to_string() })
        else {
            panic!("excluding a disclosure asks");
        };
        assert_eq!(
            first.query.facets,
            vec![r#"disclosure_types NOT IN ["telemetry"]"#.to_string()],
            "the option's own field, and a list even for one value"
        );

        // A category excluded beside it stays its own `categories` part: the two
        // are different fields, so `negativeByType` holds two entries.
        let category = category_filter_id(ProjectType::Mod, "technology");
        let Some(Ask::Search(second)) =
            state.update(Message::Exclude { filter: category, option: "technology".to_string() })
        else {
            panic!("excluding a category asks");
        };
        assert_eq!(
            second.query.facets,
            vec![concat!(
                r#"categories NOT IN ["technology"]"#,
                " AND ",
                r#"disclosure_types NOT IN ["telemetry"]"#,
            )
            .to_string()],
            "the two fields are one request, and the category is not asked as a disclosure"
        );
    }

    #[test]
    fn the_game_version_section_sorts_where_search_ts_says_it_does() {
        // `ordering: projectTypes.includes('mod') ? 2 : includes('shader') ? -1`,
        // so the mod tab's *Game version* is its first section and the shader
        // tab's is its last -- the only filter in the table whose number is
        // negative.
        // `advanced`'s `ordering: -1000` puts it last on every tab that has one,
        // which is why it trails the license rather than leading the table it is
        // written in.
        let order = |kind| filters_for(kind).iter().map(|f| f.id).collect::<Vec<&str>>();
        assert_eq!(
            order(ProjectType::Mod),
            vec![GAME_VERSION, MOD_LOADER, ENVIRONMENT, LICENSE, ADVANCED]
        );
        assert_eq!(
            order(ProjectType::Shader),
            vec![SHADER_LOADER, LICENSE, GAME_VERSION, ADVANCED]
        );
        assert_eq!(
            order(ProjectType::Modpack),
            vec![GAME_VERSION, ENVIRONMENT, MODPACK_LOADER, LICENSE, ADVANCED],
            "and no ordering at all on the tab that sets none"
        );
    }

    /// The version rows the panel would draw for `state`, in the order it draws
    /// them, which is the tag document's own order.
    #[test]
    fn an_option_row_is_tinted_only_while_the_pointer_is_on_it() {
        // The reference's third arm: an unchosen row is `bg-transparent` with
        // `hover:bg-button-bg`, so the fill is the pointer's own fraction of
        // `--color-button-bg` -- the same arithmetic `ui::switch` uses on the same
        // token.
        assert_eq!(row_fill(false, false, 0.0), None, "an unchosen row with no pointer on it");
        assert_eq!(
            row_fill(false, false, 0.4),
            Some((Ink::ButtonBg, 0.4)),
            "and half a hover is half the fill"
        );
        // The chosen and excluded arms are opaque and are not the pointer's to
        // fade: one is `bg-brand-highlight`, the other `bg-highlight-red`.
        assert_eq!(row_fill(true, false, 0.3), Some((Ink::ColorBrandHighlight, 1.0)));
        assert_eq!(row_fill(true, true, 0.0), Some((Ink::ColorBrandHighlight, 1.0)), "and never both at once");
        assert_eq!(row_fill(false, true, 0.0), Some((Ink::RedHighlight, 1.0)));
    }

    fn version_ids(state: &State, tags: &palantir_net::Tags) -> Vec<String> {
        let query = state.version_query.to_lowercase();
        tags.game_versions
            .iter()
            .filter(|version| state.all_versions || version.version_type == "release")
            .filter(|version| query.is_empty() || version.version.to_lowercase().contains(&query))
            .map(|version| version.version.clone())
            .collect()
    }

    #[test]
    fn a_filter_is_drawn_on_its_panel_when_its_rows_live_there() {
        // `filter_rows` answers an empty list for `game_version` -- its options
        // are not rows but the panel that draws them -- and the guard that used to
        // read "no rows, no section" dropped the section with them, so the tab
        // had no game version section at all. A capture is what found it: the
        // panel's own diagnostic said the section was built and never reached
        // the screen.
        let version = SIDEBAR_FILTERS.iter().find(|f| f.id == GAME_VERSION).expect("the filter exists");
        assert_eq!(version.display, Display::Scrollable);
        assert!(draws(version, &[]), "a scrollable filter is drawn on its panel");
        let shut = SIDEBAR_FILTERS.iter().find(|f| f.id == MODPACK_LOADER).expect("the filter exists");
        assert!(!draws(shut, &[]), "and a list filter with no options is not");
        assert!(draws(shut, &[Row {
            id: "mrpack".to_string(),
            label: "MrPack".to_string(),
            chosen: false,
            excluded: false,
            excludes: true,
            press: Message::Filter { filter: MODPACK_LOADER.to_string(), option: "mrpack".to_string() },
            exclude: Message::Exclude {
                filter: MODPACK_LOADER.to_string(),
                option: "mrpack".to_string(),
            },
        }]));
    }

    fn tags_with(loaders: &[(&str, &[&str])]) -> palantir_net::Tags {
        palantir_net::Tags {
            // One category, so the tag list is `Ready`: a tag document with no
            // category in it is the answer that says there are none.
            categories: vec![palantir_net::CategoryTag {
                name: "technology".to_string(),
                project_type: "mod".to_string(),
                header: "technical".to_string(),
            }],
            loaders: loaders
                .iter()
                .map(|(name, kinds)| palantir_net::LoaderTag {
                    name: (*name).to_string(),
                    supported_project_types: kinds.iter().map(|k| (*k).to_string()).collect(),
                })
                .collect(),
            ..palantir_net::Tags::default()
        }
    }
}
