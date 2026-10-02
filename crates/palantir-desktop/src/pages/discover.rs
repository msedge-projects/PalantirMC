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

use std::collections::HashMap;

use iced::mouse::Interaction;
use iced::widget::{column, container, mouse_area, row, Space};
use iced::{Alignment, Background, Border, Element, Font, Length, Padding};
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
    /// The search was asked for again.
    Search,
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
    /// The current page, one-based.
    pub page: usize,
    /// The results, which arrive from the search API.
    pub results: Load<Vec<Hit>>,
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
            page: 1,
            // Not `Empty`: nothing has been asked for yet, and an empty *answer*
            // and an unmade *request* are different sentences on the screen. The
            // shell asks for this page as soon as it draws it (`Screen::opening`),
            // so `Idle` is a state that lasts one message rather than a page that
            // sits there.
            results: Load::Idle,
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
        match message {
            // A wheel is not this page's to apply: see `crate::scroll`.
            Message::Wheel(..) => {},

            // Nothing to do with it here: where the region is *is* the page's
            // state, and the next frame ([`view`]) is the one that uses it.
            Message::Scrolled(at) => self.geometry = at,

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
            // Decoration, and the page keeps it either way: what the map holds is
            // what the cards on screen look up, so an icon no card names is one
            // that is never drawn and is dropped when the next results land.
            Message::Icons { arrived } => {
                for fetched in arrived {
                    self.icons.insert(fetched.url, fetched.icon);
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
    pub fn opening(&mut self) -> Option<Asked> {
        if self.results == Load::Idle {
            Some(self.ask())
        } else {
            None
        }
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
        Asked { round: self.round, query: self.request() }
    }

    /// The search this page's controls describe.
    ///
    /// The controls and the request are the same thing read twice: the tab is the
    /// project type, the sort is the API's index, the view size is the limit, and
    /// the page number is the offset. One place, so a control that changes cannot
    /// fail to change the request.
    pub fn request(&self) -> ApiSearch {
        ApiSearch::new(self.query.trim())
            .of_type(self.project_type.token())
            .sorted_by(self.sort.token())
            .with_limit(self.view as u32)
            .from_row((self.page.saturating_sub(1) * self.view) as u32)
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
    // controls row, the results.
    let blocks: Vec<Element<'a, Message>> = vec![
        tabs(theme, state),
        ui::search(theme, &state.placeholder(), &state.query, Message::Query),
        controls(theme, state),
        results(theme, state),
    ];
    body(blocks)
}

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
            top: INSET,
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
fn controls<'a>(theme: Gen, state: &'a State) -> Element<'a, Message> {
    row![]
        .spacing(ROW_GAP)
        .align_items(Alignment::Center)
        .push(ui::select(theme, Key::LabelSortBy, state.sort.label(), 256.0))
        .push(ui::select(theme, Key::BrowseViewPrefix, &state.view_label(), 144.0))
        .into()
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

/// What the page shows while its first answer is on the way:
/// `base/LoadingIndicator.vue`'s own three blocks under its label.
///
/// The component is `w-full flex items-center justify-center flex-col gap-2`
/// around a `font-bold text-contrast` *Loading* and three placeholders: `h-16`
/// (64px) each, `rounded-lg` (16px), `opacity-25`, in `--color-raised-bg`. The
/// label's dots are the component's own animation resting at `'...'` (its
/// `::after` keyframes start and end there), so *Loading...* is the frame a
/// still capture shows and the frame drawn here. The reference positions the
/// label *over* the blocks (`position: absolute`); this toolkit has no stacking
/// widget, so the label is drawn above them instead -- the one visible
/// departure, and the one worth spending a stack on when a stack exists.
fn loading<'a>(theme: Gen) -> Element<'a, Message> {
    // The label is the reference's own literal rather than a locale key: its
    // template writes `Loading` and the dots are CSS.
    let label = text("Loading...".to_string())
        .size(16.0)
        // `font-bold` is weight 700, which is `Weight::Bold`; the kit's
        // `semibold` is 600.
        .font(Font { weight: iced::font::Weight::Bold, ..semibold() })
        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST)));
    column![
        label,
        loading_block(theme),
        loading_block(theme),
        loading_block(theme)
    ]
    .spacing(ROW_GAP)
    .align_items(Alignment::Center)
    .width(Length::Fill)
    .into()
}

/// One of [`loading`]'s three blocks: `h-16 rounded-lg opacity-25` in
/// `--color-raised-bg`.
fn loading_block<'a>(theme: Gen) -> Element<'a, Message> {
    container(Space::new(Length::Fill, Length::Fixed(LOADING_BLOCK)))
        .width(Length::Fill)
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
        let asked = state.opening().expect("the request the page owes");
        assert_eq!(asked.round, 1);
        assert_eq!(asked.query.query, "");
        assert_eq!(asked.query.project_type.as_deref(), Some("modpack"));
        assert_eq!(asked.query.index.as_deref(), Some("relevance"));
        assert_eq!(asked.query.limit, 20);
        assert_eq!(asked.query.offset, 0);
        assert_eq!(state.results, Load::Loading);
        // Asking is once: a page that has asked does not ask again on every
        // message the shell handles.
        assert!(state.opening().is_none());
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
        let first = state.opening().expect("a request");
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
        state.opening();
        state.update(Message::Found { round: 1, result: Ok(vec![hit("Sodium")]) });
        // A page change is a request of its own now -- the reference's watcher
        // refreshes on it -- so that is round 2, and the tab change below is 3.
        state.update(Message::Page(4));
        assert_eq!(state.update(Message::ProjectType(ProjectType::Mod)), None);
        assert_eq!(state.page, 1);
        assert_eq!(state.results, Load::Idle);
        let asked = state.opening().expect("the new tab's request");
        assert_eq!(asked.round, 3);
        assert_eq!(asked.query.project_type.as_deref(), Some("mod"));
        // And the tab already on screen is not a change, so nothing is thrown away.
        state.update(Message::Found { round: asked.round, result: Ok(vec![hit("Sodium")]) });
        state.update(Message::ProjectType(ProjectType::Mod));
        assert_eq!(state.results, Load::Ready(vec![hit("Sodium")]));
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
        let first = state.opening().expect("a request");
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
}
