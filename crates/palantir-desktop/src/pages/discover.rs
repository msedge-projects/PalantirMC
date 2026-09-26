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
//! shell an [`Asked`], the shell runs it through the store off the frame thread, and
//! the answer comes back as [`Message::Found`]. A store with no engine to ask
//! answers with a sentence rather than a list (see [`crate::store`]), and the four
//! states of the answer draw through [`crate::page::draw`] either way.
//!
//! The vocabulary -- the five orders, the six view sizes, the three messages -- is
//! declared here rather than in the search code, because the strings the controls
//! show are the reference's own literals and a label invented elsewhere is a label
//! nobody can trace. [`State::request`] is the other half of that: the controls and
//! the query string are one thing read twice.
#![allow(dead_code)]

use iced::mouse::Interaction;
use iced::widget::{column, mouse_area, row, text, Space};
use iced::{Alignment, Element, Length};
use palantir_net::engine::Search as ApiSearch;
use palantir_net::ModrinthSearchHit;

use crate::page::{self, Load, GAP, GRID_GAP, ROW_GAP};
use crate::route::ProjectType;
use crate::store::Store;
use crate::style::{medium, semibold, INK_CONTRAST, INK_SECONDARY};
use crate::text_gen::{self, Key};
use crate::theme_gen::{self, Theme as Gen};
use crate::ui;

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
            round: 0,
        }
    }

    /// Apply a message, and report the request it asked for.
    ///
    /// One message in, at most one request out: a page cannot ask twice in a turn,
    /// and the shell cannot be handed a request it has already run.
    pub fn update(&mut self, message: Message) -> Option<Asked> {
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
            Message::Search => return Some(self.ask()),
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
        Load::Ready(hits) => {
            let mut list = column![].spacing(GRID_GAP).width(Length::Fill);
            for hit in hits {
                list = list.push(hit_card(theme, hit));
            }
            list.into()
        }
        // The empty arm is the reference's own sentence; the other two are the
        // scaffold's.
        other => page::draw(theme, other, "results", |hits| {
            let mut list = column![].spacing(GRID_GAP).width(Length::Fill);
            for hit in hits {
                list = list.push(hit_card(theme, hit));
            }
            list.into()
        }),
    }
}

/// One project card: its title, its author, its summary and its counts.
///
/// The whole card is pressable, which is what the reference does and what the
/// absence of controls inside it makes safe: iced's `mouse_area` does not forward
/// a press to its content, so nothing interactive may be drawn inside one.
pub fn hit_card<'a>(theme: Gen, hit: &Hit) -> Element<'a, Message> {
    let mut tags = row![].spacing(6.0);
    for loader in hit.loaders.iter().take(3) {
        tags = tags.push(ui::tag(theme, loader));
    }
    for version in hit.game_versions.iter().take(2) {
        tags = tags.push(ui::tag(theme, version));
    }
    mouse_area(ui::card(
        theme,
        column![]
            .spacing(6.0)
            .push(
                text(hit.title.clone())
                    .size(16.0)
                    .font(semibold())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
            )
            .push(
                text(hit.author.clone())
                    .size(13.0)
                    .font(medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
            )
            .push(ui::paragraph(theme, &hit.summary))
            .push(tags)
            .push(
                row![]
                    .spacing(ROW_GAP)
                    .push(ui::icon_label(
                        theme,
                        crate::icons_gen::Glyph::Download,
                        &text_gen::project_download_count_tooltip(hit.downloads),
                    ))
                    .push(ui::icon_label(
                        theme,
                        crate::icons_gen::Glyph::Heart,
                        &text_gen::project_follower_count_tooltip(hit.follows),
                    )),
            ),
    ))
    .interaction(Interaction::Pointer)
    .on_press(Message::Open(hit.id.clone()))
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
            game_versions: vec!["1.21".to_string()],
            loaders: vec!["fabric".to_string()],
        }
    }

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
        let second = state.update(Message::Search).expect("the button asks again");
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
        // The counts are the reference's own messages, so a card with 12,345
        // downloads shows `12,345 downloads` rather than a number on its own.
        assert_eq!(text_gen::project_download_count_tooltip(12345u64), "12,345 downloads");
        assert_eq!(text_gen::project_download_count_tooltip(1u64), "1 download");
        assert_eq!(text_gen::project_follower_count_tooltip(678u64), "678 followers");
        drop(hit_card(Gen::Dark, &Hit { downloads: 0, follows: 0, ..hit("Empty") }));
    }
}
