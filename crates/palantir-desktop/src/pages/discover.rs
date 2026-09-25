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
//! The results themselves come from Modrinth's search API, which is stage 4. Until
//! then the page draws the control row it will keep and says where the list is
//! ([`crate::store::unavailable`]) -- and it draws the *input* controls live, so
//! the state the user sets is the state the request will be made with.
//!
//! The three controls in that row show the state they hold; the comboboxes that
//! open to change it, the pagination and the request itself are stage 4's. Their
//! vocabulary -- the five orders, the six view sizes, the three messages -- is
//! declared here rather than then, because the strings the controls show are the
//! reference's own literals and a label invented in stage 4 is a label nobody can
//! trace. The attribute below permits what nothing constructs yet.
#![allow(dead_code)]

use iced::mouse::Interaction;
use iced::widget::{column, mouse_area, row, text, Space};
use iced::{Alignment, Element, Length};

use crate::page::{self, Load, GAP, GRID_GAP, ROW_GAP};
use crate::route::ProjectType;
use crate::store::{self, Store};
use crate::style::{medium, semibold, INK_CONTRAST, INK_SECONDARY};
use crate::text_gen::{self, Key};
use crate::theme_gen::{self, Theme as Gen};
use crate::ui;

/// One project in the results.
///
/// The fields are the ones the reference's card draws, in the order its API
/// returns them, so the store that fills this in stage 4 has one shape to fill
/// rather than a page to restructure.
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

/// The orders the search can be asked in.
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
    /// The results, which arrive from the search API in stage 4.
    pub results: Load<Vec<Hit>>,
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
            // and an unmade *request* are different sentences on the screen.
            results: Load::Failed(store::unavailable("Discover's search")),
        }
    }

    /// Apply a message.
    pub fn update(&mut self, message: Message) {
        match message {
            Message::ProjectType(project_type) => {
                if self.project_type != project_type {
                    self.project_type = project_type;
                    // A different tab is a different search, and the reference
                    // resets the page rather than carrying it across.
                    self.page = 1;
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
            Message::Search => {
                self.results = Load::Failed(store::unavailable("Discover's search"));
            }
            // The shell's to do, and not this page's: see the enum.
            Message::Open(_) => {}
        }
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
    ui::tabs(theme, &labels, move |index| {
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
    fn the_results_say_where_they_will_come_from_until_they_can() {
        let state = State::new(ProjectType::Modpack);
        let reason = state.results.failure().expect("a reason");
        assert!(reason.contains("stage 4"), "{reason}");
        // And they are not an empty *answer*: the page must not say "no results"
        // about a request that has not been made.
        assert_ne!(state.results, Load::Empty);
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
