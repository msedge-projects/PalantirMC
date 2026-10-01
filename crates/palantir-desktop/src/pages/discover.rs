//! Discover: `pages/Browse.vue`, at `/browse/:projectType`.
//!
//! The reference's layout, in its own order and with its own numbers:
//!
//! 1. the project-type tabs (`NavTabs`), built from `Browse.vue`'s own list and
//!    labelled with its category messages;
//! 2. the search field, whose placeholder is `browse.search.placeholder` --
//!    *"Search {projectType}..."* -- with the tab's own name in it;
//! 3. the controls row: `label.sort-by` with the five orders `useSearch` declares,
//!    and `browse.view-prefix` with the view sizes `[5, 10, 15, 20, 50, 100]`, of
//!    which 20 is the reference's default;
//! 4. the results, or the sentence for having none: *"No results found for your
//!    query!"*.
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
use iced::widget::{column, mouse_area, row, Space};
use iced::{Alignment, Element, Length};
use palantir_net::engine::Search as ApiSearch;
use palantir_net::ModrinthSearchHit;

use crate::avatar::{self, Fetched, Icon};
use crate::icon;
use crate::icons_gen::Glyph;
use crate::locale;
use crate::page::{self, Load, GAP, ROW_GAP};
use crate::pages::Ask;
use crate::route::ProjectType;
use crate::store::Store;
use crate::style::{medium, regular, semibold, INK_CONTRAST, INK_DEFAULT, INK_SECONDARY};
use crate::text_gen::{self, Key};
use crate::theme_gen::{self, Theme as Gen};
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

/// The sorting control's own name, and the one its results are filtered by.
const FILTER_KEY: &str = "discover:filter";

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
        }
    }

    /// Apply a message, and report the request it asked for.
    ///
    /// One message in, at most one request out: a page cannot ask twice in a turn,
    /// and the shell cannot be handed a request it has already run.
    pub fn update(&mut self, message: Message) -> Option<Ask> {
        match message {
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
            Message::Query(query) => {
                self.query = query;
                // Typing starts a new search, so the page goes back to the first.
                self.page = 1;
            }
            Message::Sort(sort) => {
                self.sort = sort;
                self.page = 1;
            }
            Message::View(view) => self.view = view,
            Message::Page(page) => self.page = page.max(1),
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
    page::body(blocks, GAP)
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

/// The controls row: the order, the view size, and the filter button.
fn controls<'a>(theme: Gen, state: &'a State) -> Element<'a, Message> {
    row![]
        .spacing(ROW_GAP)
        .align_items(Alignment::Center)
        .push(ui::select(theme, Key::LabelSortBy, state.sort.label(), 256.0))
        .push(ui::select(theme, Key::BrowseViewPrefix, &state.view_label(), 144.0))
        .push(ui::button(
            theme,
            FILTER_KEY,
            Key::BrowseFilterResults,
            ui::Kind::Standard,
            Message::Search,
        ))
        .push(Space::with_width(Length::Fill))
        .push(
            text(format!("{} · {}", state.project_type.label(), state.sort.token()))
                .size(13.0)
                .font(medium())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
        )
        .into()
}

/// The results, or the sentence for having none of them.
fn results<'a>(theme: Gen, state: &'a State) -> Element<'a, Message> {
    match &state.results {
        Load::Ready(hits) if hits.is_empty() => page::empty(theme, Key::BrowseNoResults),
        Load::Ready(hits) => cards(theme, state, hits),
        // The empty arm is the reference's own sentence; the other two are the
        // scaffold's.
        other => page::draw(theme, other, "results", |hits| cards(theme, state, hits)),
    }
}

/// The result cards, stacked.
///
/// `ProjectCardList`'s own `gap-3` between the cards of a *list*, which is
/// [`GAP`] -- 12 pixels -- and not the wider [`crate::page::GRID_GAP`] its grid
/// layout is spaced with. The reference's browse opens on the list one
/// (`use-browse-search.ts`'s `effectiveDisplayMode` defaults to `'list'`), so this
/// is the gap a reader sees.
fn cards<'a>(theme: Gen, state: &'a State, hits: &'a [Hit]) -> Element<'a, Message> {
    let mut list = column![].spacing(GAP).width(Length::Fill);
    for hit in hits {
        list = list.push(hit_card(theme, hit, state.icons.get(&hit.icon_url)));
    }
    list.into()
}

/// `ProjectCard.vue`'s list grid: `p-4 grid-project-card-list gap-x-3 gap-y-2`.
///
/// The card's own two gaps. The padding is [`ui::card_at`]'s (`p-4`, the same 16).
const CARD_COLUMN_GAP: f32 = 12.0;
/// `gap-y-2`, between the icon's row and the row of tags under it.
const CARD_ROW_GAP: f32 = 8.0;

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
/// iced has no grid, so the same placement is two rows: the icon, the info column
/// and the stats in one, and the tags indented by the icon's own column in the
/// next, which is where the grid's third row puts them.
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
    // makes a taller card than the reference's.
    let summary = text(hit.summary.clone())
        .size(16.0)
        .font(regular())
        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT)));
    let info = column![]
        .spacing(ROW_GAP)
        .width(Length::Fill)
        .push(row![title, author].spacing(ROW_GAP).align_items(Alignment::Center))
        .push(summary);

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

    let placed = column![]
        .spacing(CARD_ROW_GAP)
        .push(
            row![
                ui::icon_box(theme, avatar::ICON_SIDE as f32, picture),
                info,
                stats
            ]
            .spacing(CARD_COLUMN_GAP)
            // `Start` vertically, which is the grid's own top alignment: the icon's
            // box and the stats sit against the first line of the info column.
            .align_items(Alignment::Start),
        )
        .push(
            // The indent is the icon's own width, and the row's gap is what follows
            // it: 100 + 12, which is the 112 pixels the grid's second column starts
            // at.
            row![Space::with_width(avatar::ICON_SIDE as f32), tags].spacing(CARD_COLUMN_GAP),
        );

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
            panic!("the button asks again");
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
        state.update(Message::Page(4));
        assert_eq!(state.update(Message::ProjectType(ProjectType::Mod)), None);
        assert_eq!(state.page, 1);
        assert_eq!(state.results, Load::Idle);
        let asked = state.opening().expect("the new tab's request");
        assert_eq!(asked.round, 2);
        assert_eq!(asked.query.project_type.as_deref(), Some("mod"));
        // And the tab already on screen is not a change, so nothing is thrown away.
        state.update(Message::Found { round: 2, result: Ok(vec![hit("Sodium")]) });
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
