//! Home, the reference's first rail slot: `pages/Index.vue`.
//!
//! Two states, exactly as the reference has two. With no instance yet it draws
//! `WelcomeScreen` -- a title, a description, the two buttons that start the
//! first instance, and a hint about the quick-create flow. With instances it draws
//! the library: the title, its search field and sort control, and a grid of cards.
//!
//! The reference's own `RecentWorldsList` sits above the library when the
//! `worlds_in_home` feature flag is on; it is off in the reference's defaults, and
//! it is not drawn here, which is a decision rather than an omission -- it is the
//! one block of that page whose data is a cross-instance scan of every `saves/`
//! folder, and it would arrive before the page it decorates is worth reading.
//!
//! What is real here is the library itself: the instance list, each instance's
//! loader, game version, playtime and mod count come from disk by way of
//! [`Store`], which is the same reader the old interface used. What is *not* real
//! yet is anything from the network, and the page says what is missing rather than
//! drawing an empty list ([`crate::store::not_implemented`]).
//!
//! The search field filters for real. The sort control shows the chosen order and
//! the seven orders it offers are declared, labelled from the locale and asserted
//! by this module's own gate -- the combobox that opens to pick one is the last
//! piece of the control, and declaring its vocabulary now is what keeps the labels
//! from being invented later. That forward declaration is what the attribute below
//! permits.
#![allow(dead_code)]

use iced::mouse::Interaction;
use iced::widget::{column, container, image, mouse_area, row, text, Space};
use iced::{Alignment, Background, Border, Element, Length, Padding, Theme};

use crate::icon;
use crate::icons_gen::Glyph;
use crate::instances::InstanceCard;
use crate::page::{self, Load, GAP, GRID_GAP, ROW_GAP};
use crate::store::Store;
use crate::style::{heading, medium, semibold, INK_CONTRAST, INK_DEFAULT, INK_SECONDARY};
use crate::text_gen::Key;
use crate::theme_gen::{self, Ink, Span, Theme as Gen};
// `Hovered` is in scope for the instance cards below: a card names its own
// crossing rather than going through one of the kit's controls.
use crate::ui::{self, Hovered};

/// Which way the library is ordered.
///
/// The reference's own list, from `app.library.sort.*`: it is a control the user
/// picks from, so the options are its keys rather than our labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sort {
    /// Newest playtime first, which is what the reference opens on.
    #[default]
    LastPlayed,
    /// Name, A to Z.
    Name,
    /// Newest game version first.
    GameVersion,
    /// By mod loader.
    Loader,
    /// Newest-created first.
    Created,
    /// Most recently modified first.
    Modified,
    /// Most time played first.
    HoursPlayed,
}

impl Sort {
    /// Every sort the control offers, in the reference's order.
    pub const ALL: [Sort; 7] = [
        Sort::LastPlayed,
        Sort::Name,
        Sort::GameVersion,
        Sort::Loader,
        Sort::Created,
        Sort::Modified,
        Sort::HoursPlayed,
    ];

    /// The reference's key for this option's label.
    pub const fn key(self) -> Key {
        match self {
            Sort::LastPlayed => Key::AppLibrarySortLastPlayed,
            Sort::Name => Key::AppLibrarySortName,
            Sort::GameVersion => Key::AppLibrarySortGameVersion,
            Sort::Loader => Key::AppLibrarySortLoader,
            Sort::Created => Key::AppLibrarySortDateCreated,
            Sort::Modified => Key::AppLibrarySortDateModified,
            Sort::HoursPlayed => Key::AppLibrarySortHoursPlayed,
        }
    }

    /// The label, as the reference writes it.
    pub fn label(self) -> &'static str {
        self.key().message()
    }
}

/// What Home can be told.
#[derive(Debug, Clone)]
pub enum Message {
    /// The library's search text changed.
    Search(String),
    /// A different sort was chosen.
    Sort(Sort),
    /// The welcome screen's create button.
    CreateInstance,
    /// The welcome screen's import button.
    ImportFromLauncher,
    /// One instance was opened. Reported rather than applied: which page is in the
    /// pane is the shell's business, so this comes back out of
    /// [`crate::pages::Screen::update`] as an [`crate::pages::Open`].
    Open(String),
    /// The last notice was dismissed.
    DismissNotice,
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

/// The library toolbar's create button, and the welcome screen's two.
const CREATE_KEY: &str = "home:create";
const IMPORT_KEY: &str = "home:import";
const WELCOME_CREATE_KEY: &str = "home:welcome:create";
const WELCOME_IMPORT_KEY: &str = "home:welcome:import";
/// The button inside an instance card, which is the same control on every card.
const INSTANCE_OPEN_KEY: &str = "home:card:open";

/// Home's own state: what the user has typed and chosen, and what could not be
/// done.
#[derive(Debug, Clone, Default)]
pub struct State {
    /// The library's search text, which filters in place.
    pub search: String,
    /// The chosen order.
    pub sort: Sort,
    /// The last thing this page was asked to do and could not, shown as an
    /// admonition rather than swallowed.
    pub notice: Option<String>,
}

impl State {
    /// The instances a search leaves visible, in the chosen order.
    ///
    /// A pure function so that the page's filtering and its gate are the same
    /// code: what the page draws is what this returns.
    pub fn visible<'a>(&self, cards: &'a [InstanceCard]) -> Vec<&'a InstanceCard> {
        let needle = self.search.trim().to_ascii_lowercase();
        let mut visible: Vec<&InstanceCard> = cards
            .iter()
            .filter(|card| {
                needle.is_empty()
                    || card.name.to_ascii_lowercase().contains(&needle)
                    || card.subtitle().to_ascii_lowercase().contains(&needle)
            })
            .collect();
        match self.sort {
            Sort::LastPlayed | Sort::HoursPlayed => visible.sort_by(|left, right| {
                right.playtime_secs.cmp(&left.playtime_secs).then_with(|| left.name.cmp(&right.name))
            }),
            Sort::Name => visible.sort_by(|left, right| left.name.cmp(&right.name)),
            Sort::GameVersion => visible.sort_by(|left, right| {
                right.mc_version.cmp(&left.mc_version).then_with(|| left.name.cmp(&right.name))
            }),
            Sort::Loader => visible.sort_by(|left, right| {
                right
                    .loader
                    .label()
                    .cmp(left.loader.label())
                    .then_with(|| left.name.cmp(&right.name))
            }),
            // The three the launcher's own books cannot answer yet: their order
            // comes from the filesystem's timestamps, which `instances` does not
            // read. Falling back to the name keeps the list stable and in one
            // piece rather than shuffling it on every frame.
            Sort::Created | Sort::Modified => {
                visible.sort_by(|left, right| left.name.cmp(&right.name))
            }
        }
        visible
    }

    /// Apply a message.
    pub fn update(&mut self, message: Message) {
        match message {
            Message::Search(text) => self.search = text,
            Message::Sort(sort) => self.sort = sort,
            // Reported rather than applied, like `Open`: what makes an instance is
            // the store and where the flow goes is the shell, so this comes back
            // out of `Screen::update` as `Ask::Create`.
            Message::CreateInstance => {}
            // Reported as well: what to import, and what to copy, is the shell's
            // and the store's -- see `pages::Ask::Import`.
            Message::ImportFromLauncher => {}
            // Opening is the shell's to do -- see the enum -- so the page has
            // nothing to do with it, and a variant the page acts on would be the
            // page claiming to know where it goes.
            Message::Open(_) => {}
            Message::DismissNotice => self.notice = None,
            Message::Hover { key, over, hover } => crate::ui::pointer_with(
                key,
                over,
                hover.unwrap_or_else(crate::theme::hover_brightness),
            ),
        }
    }
}

/// `Index.vue`'s own gate, in one place: `v-if="isReady && !hasCreatedInstance"`.
///
/// [`Load::Empty`] is the store saying "read, and there is nothing to play", and
/// `Ready` with an empty list is the same answer from a library built by hand:
/// both are the first run. The shell reads this same function for the quick-create
/// key, so the page drawn and the key listened for cannot come apart. Every other
/// state is not the first run: `Idle` and `Loading` are a page that has not
/// answered yet, and `Failed` is the not-implemented page's own.
pub(crate) fn first_run(instances: &Load<Vec<InstanceCard>>) -> bool {
    match instances {
        Load::Empty => true,
        Load::Ready(cards) => cards.is_empty(),
        _ => false,
    }
}

/// Draw Home.
pub fn view<'a>(theme: Gen, state: &'a State, store: &'a Store) -> Element<'a, Message> {
    let mut blocks: Vec<Element<'a, Message>> = Vec::new();
    // A notice is something the page could not do, and it stays until it is read:
    // silently clearing it on the next repaint would hide the sentence exactly
    // when the user looked back at it.
    if let Some(notice) = &state.notice {
        blocks.push(page::notice(
            theme,
            Key::AppLibraryActionsLabel,
            notice,
            Message::DismissNotice,
        ));
    }
    if first_run(store.instances()) {
        // The welcome screen's two buttons are the only way out of it, and there
        // is nothing else on the page: no toolbar, no search, nothing to sort.
        blocks.push(welcome(theme));
        return page::body(blocks, GAP);
    }
    match store.instances() {
        Load::Ready(cards) => {
            let visible = state.visible(cards);
            blocks.push(header(theme, state));
            if visible.is_empty() {
                blocks.push(page::empty(theme, Key::AppLibrarySearchNoResultsTitle));
            } else {
                let mut grid = column![].spacing(GAP).width(Length::Fill);
                for card in visible {
                    grid = grid.push(instance_card(theme, card));
                }
                blocks.push(grid.into());
            }
            page::body(blocks, GAP)
        }
        _ => page::body(
            vec![page::draw(theme, store.instances(), "your instances", |cards| {
                cards_placeholder(theme, cards)
            })],
            GAP,
        ),
    }
}

/// The title, the search field and the sort control of the library.
fn header<'a>(theme: Gen, state: &'a State) -> Element<'a, Message> {
    let sort = row![]
        .spacing(ROW_GAP)
        .align_items(Alignment::Center)
        .push(ui::select(theme, Key::AppLibrarySortLabel, state.sort.label(), 200.0))
        .push(ui::button(
            theme,
            CREATE_KEY,
            Key::AppLibraryContextMenuCreateInstance,
            ui::Kind::Colored,
            Message::CreateInstance,
        ));
    row![]
        .spacing(ROW_GAP)
        .align_items(Alignment::Center)
        .push(page::title(theme, Key::AppLibraryTitle))
        .push(Space::with_width(Length::Fill))
        .push(sort)
        .push(ui::search(
            theme,
            Key::AppLibrarySearchPlaceholder.message(),
            &state.search,
            Message::Search,
        ))
        .into()
}

/// The welcome screen's own numbers, all out of `WelcomeScreen.vue`: the hero's
/// icon at `size-[6.25rem]`, the column under it at `w-72`, `gap-6` between the
/// hero's parts and `gap-4` inside the column, `gap-2` in the title's own block,
/// the shortcut chip's `h-5`, and the page's own `px-6 pb-6 pt-16`.
const WELCOME_ART: f32 = 100.0;
const WELCOME_COLUMN: f32 = 288.0;
const HERO_GAP: f32 = 24.0;
const WELCOME_GAP: f32 = 16.0;
const TITLE_GAP: f32 = 8.0;
const WELCOME_TOP: f32 = 64.0;
const WELCOME_SIDE: f32 = 24.0;
const SHORTCUT_HEIGHT: f32 = 20.0;

/// The reference's welcome screen: the whole page on a first run, not a card in
/// the middle of an empty library.
///
/// `WelcomeScreen.vue`, drawn by `pages/Index.vue` as
/// `v-if="isReady && !hasCreatedInstance"`: `flex flex-col min-h-full px-6 pb-6
/// pt-16`, a hero centred in what is left and a foot block under it. The hero is
/// the icon, a `gap-2` column of the title (`text-2xl font-semibold`, which is
/// [`page::title`]'s own size) and the description at `text-base`, and then a
/// `w-72` column of the create button and the quick-create hint. The foot is
/// *Escaping another launcher?* over the import button.
///
/// Three things here are this launcher's rather than the reference's, each
/// written down rather than approximated:
///
/// * **The hero's icon is this launcher's own art.** The reference's is
///   `assets/welcome/modrinth-social-icon.png`, and the vendored `assets/` holds
///   `branding/` and `external/` and no `welcome/`: the picture is not in this
///   tree, so the logo the rail already draws is drawn here rather than a
///   borrowed one standing in for it.
/// * **The dot pattern behind the hero is not drawn.** It is an
///   absolutely-positioned decorative block that the reference draws *behind* the
///   icon and the title, and iced 0.12 has no overlay widget -- a `Column` places
///   its children one after another, so a pattern here could only be above or
///   below the hero rather than under it. It is texture with no state behind it,
///   so what the page loses is a grid of dots.
/// * **Neither button is ever disabled.** The reference draws both
///   `:disabled="offline"` from `navigator.onLine`, and this launcher has no online
///   signal anywhere: what an offline launcher finds instead is the flow's own
///   failure, said where the flow asks for something over the network.
fn welcome<'a>(theme: Gen) -> Element<'a, Message> {
    let hero = column![]
        .width(Length::Fill)
        .align_items(Alignment::Center)
        .spacing(HERO_GAP)
        .push(image(crate::brand::logo_handle()).height(Length::Fixed(WELCOME_ART)))
        .push(
            column![]
                .align_items(Alignment::Center)
                .spacing(TITLE_GAP)
                .push(page::title(theme, Key::AppWelcomeScreenTitle))
                .push(
                    text(Key::AppWelcomeScreenDescription.message())
                        .size(16.0)
                        .font(medium())
                        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
                ),
        )
        .push(
            column![]
                .width(Length::Fixed(WELCOME_COLUMN))
                .align_items(Alignment::Center)
                .spacing(WELCOME_GAP)
                .push(ui::button_with_icon(
                    theme,
                    WELCOME_CREATE_KEY,
                    Glyph::Plus,
                    Key::AppWelcomeScreenCreateInstance,
                    ui::Kind::Colored,
                    Length::Fill,
                    Some(Message::CreateInstance),
                ))
                .push(quick_create_hint(theme)),
        );
    let foot = column![]
        .width(Length::Fill)
        .align_items(Alignment::Center)
        .spacing(WELCOME_GAP)
        .push(
            text(Key::AppWelcomeScreenImportPrompt.message())
                .size(14.0)
                .font(medium())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
        )
        .push(ui::button_with_icon(
            theme,
            WELCOME_IMPORT_KEY,
            Glyph::Import,
            Key::AppWelcomeScreenImportFromLauncher,
            ui::Kind::Standard,
            Length::Shrink,
            Some(Message::ImportFromLauncher),
        ));
    column![]
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding {
            top: WELCOME_TOP,
            bottom: WELCOME_SIDE,
            left: WELCOME_SIDE,
            right: WELCOME_SIDE,
        })
        // `justify-center` on the column that holds the hero: what is left of the
        // page after the padding and the foot is the hero's box, and the hero sits
        // in the middle of it.
        .push(container(hero).width(Length::Fill).height(Length::Fill).center_y())
        .push(foot)
        .into()
}

/// The quick-create hint, with its `<shortcut>` slot drawn as the chip it is.
///
/// `WelcomeScreen.vue`'s `Press <shortcut>N</shortcut> to quick create an
/// instance`, whose markup the generated table keeps verbatim -- the tag's *name*
/// is the slot the component fills -- so the sentence is split by
/// [`crate::text::tagged`] and the slot becomes the reference's own chip: `h-5
/// min-w-5 rounded-md border-surface-5 bg-button-bg px-1 text-xs`. A version of
/// this screen that drew the string whole was in this tree before this, and it
/// showed the tags to the first reader of the first run.
fn quick_create_hint<'a>(theme: Gen) -> Element<'a, Message> {
    let sentence = Key::AppWelcomeScreenQuickCreateHint.message();
    let Some((before, slot, after)) = crate::text::tagged(sentence, "shortcut") else {
        // No slot: a message the table is no longer tagged with is drawn whole
        // rather than with an invented chip.
        return hint_run(theme, sentence);
    };
    row![]
        .align_items(Alignment::Center)
        .spacing(4.0)
        .push(hint_run(theme, before))
        .push(shortcut_chip(theme, slot))
        .push(hint_run(theme, after))
        .into()
}

/// One run of the hint's prose: the reference's `text-sm leading-5
/// text-secondary`.
fn hint_run<'a>(theme: Gen, run: &str) -> Element<'a, Message> {
    text(run.to_string())
        .size(14.0)
        .font(medium())
        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY)))
        .into()
}

/// The `<shortcut>` slot: a key, in the reference's chip.
fn shortcut_chip<'a>(theme: Gen, key: &str) -> Element<'a, Message> {
    container(
        text(key.to_string())
            .size(12.0)
            .font(medium())
            .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
    )
    .height(Length::Fixed(SHORTCUT_HEIGHT))
    .padding(Padding { top: 0.0, bottom: 0.0, left: 4.0, right: 4.0 })
    .center_x()
    .center_y()
    .style(move |_theme: &Theme| container::Appearance {
        background: Some(Background::Color(theme_gen::ink(theme, Ink::ButtonBg))),
        border: Border {
            color: theme_gen::ink(theme, Ink::Surface5),
            width: 1.0,
            radius: theme_gen::span(Span::RadiusMd).into(),
        },
        ..container::Appearance::default()
    })
    .into()
}

/// One instance, as the library draws it: its plate, its name, and what it is.
///
/// The *body* of the card is what opens the instance, and the button beside it is
/// not inside that pressable region -- a deliberate difference from the reference,
/// which makes the whole card a clickable `div` and stops the event inside its
/// buttons. iced's [`mouse_area`] is not that: its own `update` never visits its
/// content, so a button drawn inside one would never see a press. What is
/// pressable is therefore the part of the card that is not a control, which draws
/// the same picture and is a region a user can actually hit.
fn instance_card<'a>(theme: Gen, card: &'a InstanceCard) -> Element<'a, Message> {
    let open = Message::Open(card.id.clone());
    // The pressable region is the card's body, so that is what names itself and
    // what tweens; the button beside it has a key of its own and does not move
    // when the body is hovered.
    let key = crate::ui::scoped("home:card", &card.id);
    let (factor, _) = crate::ui::interaction(key);
    let plate = crate::ui::framed(
        theme,
        column![]
            .align_items(Alignment::Center)
            .push(icon::icon(Glyph::Play, 20.0, theme_gen::ink(theme, INK_CONTRAST))),
    );
    let mut details = column![]
        .spacing(2.0)
        .push(
            text(card.name.clone())
                .size(16.0)
                .font(semibold())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
        )
        .push(
            text(card.subtitle())
                .size(13.0)
                .font(medium())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
        );
    // A playtime of zero is the reference's own `never-played` state rather than
    // `0m`, which is why the label comes from `InstanceCard` rather than from a
    // formatter here.
    let mut facts = row![].spacing(ROW_GAP);
    facts = facts.push(ui::icon_label(theme, Glyph::Clock, &card.playtime_label()));
    if card.mods_total > 0 {
        facts = facts.push(ui::icon_label(
            theme,
            Glyph::Package,
            &crate::text_gen::project_type_mod_lowercase(card.mods_total as u64),
        ));
    }
    if let Some(problem) = &card.problem {
        facts = facts.push(ui::icon_label(theme, Glyph::TriangleAlert, problem));
    }
    details = details.push(facts);
    let body = row![]
        .spacing(GRID_GAP)
        .align_items(Alignment::Center)
        .width(Length::Fill)
        .push(plate)
        .push(details.width(Length::Fill));
    // `jump-back-in.view-instance` is the reference's label for the control that
    // opens an instance, so the button and the card body say the same thing with
    // the reference's own words.
    let open_button = ui::button(
        theme,
        INSTANCE_OPEN_KEY,
        Key::AppHomeJumpBackInViewInstance,
        ui::Kind::Standard,
        open.clone(),
    );
    ui::card_at(
        theme,
        factor,
        row![]
            .spacing(GRID_GAP)
            .align_items(Alignment::Center)
            .push(
                mouse_area(body)
                    .interaction(Interaction::Pointer)
                    .on_enter(Message::hover_with(
                        key,
                        true,
                        crate::theme::INSTANCE_CARD_HOVER_BRIGHTNESS,
                    ))
                    .on_exit(Message::hover_with(
                        key,
                        false,
                        crate::theme::INSTANCE_CARD_HOVER_BRIGHTNESS,
                    ))
                    .on_press(open),
            )
            .push(open_button),
    )
}

/// The loading arm's body, which is the cards' own frame without their contents.
fn cards_placeholder<'a>(theme: Gen, cards: &Vec<InstanceCard>) -> Element<'a, Message> {
    let mut list = column![].spacing(GAP);
    for card in cards {
        list = list.push(ui::card(
            theme,
            text(card.name.clone())
                .size(16.0)
                .font(heading())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
        ));
    }
    list.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::LoaderKind;

    /// `loader_version` is set for the modded instances because that is what the
    /// card's subtitle carries: `1.21` alone is a vanilla instance, and a search
    /// for a loader has to match the line the card actually draws.
    fn card(id: &str, name: &str, playtime: i64, mc: &str, loader: LoaderKind) -> InstanceCard {
        let loader_version = match loader {
            LoaderKind::Vanilla => String::new(),
            LoaderKind::NeoForge => "21.1.0".to_string(),
            _ => "0.15.0".to_string(),
        };
        InstanceCard {
            id: id.to_string(),
            name: name.to_string(),
            icon: String::new(),
            group: None,
            mc_version: mc.to_string(),
            loader,
            loader_version,
            playtime_secs: playtime,
            mods_total: 0,
            mods_enabled: 0,
            max_mem_mb: 4096,
            problem: None,
        }
    }

    fn sample() -> Vec<InstanceCard> {
        vec![
            card("atm", "All the Mods 10", 7200, "1.21", LoaderKind::NeoForge),
            card("sodium", "Sodium Test", 60, "1.20.1", LoaderKind::Fabric),
            card("vanilla", "Vanilla", 0, "1.19.2", LoaderKind::Vanilla),
        ]
    }

    #[test]
    fn the_sort_options_are_the_reference_s_and_carry_its_labels() {
        assert_eq!(Sort::ALL.len(), 7);
        assert_eq!(Sort::default(), Sort::LastPlayed);
        let labels: Vec<&str> = Sort::ALL.iter().map(|sort| sort.label()).collect();
        assert_eq!(labels[0], "Last played");
        assert_eq!(labels[1], "Name");
        // Every option's label is its key's own message, so a page cannot show a
        // sort the reference does not name.
        for sort in Sort::ALL {
            assert_eq!(sort.label(), sort.key().message());
            assert!(!sort.label().is_empty());
        }
    }

    #[test]
    fn the_library_filters_by_name_and_by_what_the_card_says() {
        let mut state = State::default();
        let cards = sample();
        assert_eq!(state.visible(&cards).len(), 3);
        state.search = "sod".to_string();
        let visible = state.visible(&cards);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].id, "sodium");
        // The subtitle is searched too: a loader or a game version is a thing a
        // person types when looking for an instance.
        state.search = "neoforge".to_string();
        let visible = state.visible(&cards);
        assert_eq!(visible.len(), 1, "{visible:?}");
        assert_eq!(visible[0].id, "atm");
        state.search = "1.19".to_string();
        assert_eq!(state.visible(&cards)[0].id, "vanilla");
        // Case does not matter, and a search that matches nothing is empty rather
        // than everything.
        state.search = "ATM10".to_string();
        assert!(state.visible(&cards).is_empty());
        state.search = "  ".to_string();
        assert_eq!(state.visible(&cards).len(), 3);
    }

    #[test]
    fn the_library_orders_itself_the_way_the_control_says() {
        let cards = sample();
        let mut state = State::default();
        // Last played first, which is the reference's opening order.
        assert_eq!(
            state.visible(&cards).iter().map(|card| card.id.as_str()).collect::<Vec<_>>(),
            vec!["atm", "sodium", "vanilla"]
        );
        state.sort = Sort::Name;
        assert_eq!(
            state.visible(&cards).iter().map(|card| card.id.as_str()).collect::<Vec<_>>(),
            vec!["atm", "sodium", "vanilla"]
        );
        state.sort = Sort::Loader;
        let loaders: Vec<String> =
            state.visible(&cards).iter().map(|card| card.loader.label().to_string()).collect();
        // Descending by loader name, and ties broken by the instance's name, so the
        // order is stable rather than depending on the directory walk.
        let mut sorted = loaders.clone();
        sorted.sort_by(|left, right| right.cmp(left));
        assert_eq!(loaders, sorted);
        // The two orders the filesystem has not been asked for fall back to the
        // name: the list stays in one piece instead of shuffling per frame.
        for sort in [Sort::Created, Sort::Modified] {
            state.sort = sort;
            assert_eq!(
                state.visible(&cards).iter().map(|card| card.id.as_str()).collect::<Vec<_>>(),
                vec!["atm", "sodium", "vanilla"]
            );
        }
    }

    #[test]
    fn the_welcome_screen_s_hint_splits_its_shortcut_out_of_the_sentence() {
        // The table keeps the markup verbatim, and the `<shortcut>` slot is the
        // chip the reference draws. A screen that drew the string whole -- which is
        // what this page did until now -- showed `<shortcut>N</shortcut>` to the
        // first reader of the first run.
        let sentence = Key::AppWelcomeScreenQuickCreateHint.message();
        let (before, slot, after) = crate::text::tagged(sentence, "shortcut").expect("a slot");
        assert_eq!((before, slot, after), ("Press ", "N", " to quick create an instance"));
        // Together they are the sentence without its markup, which is also what
        // `text::tagged`'s own gate asserts; here it is the screen's copy that is
        // being held to it.
        assert_eq!(format!("{before}{slot}{after}").matches("<").count(), 0);
        assert_eq!(Key::AppWelcomeScreenImportPrompt.message(), "Escaping another launcher?");
        for theme in Gen::ALL {
            drop(welcome(*theme));
        }
    }

    #[test]
    fn a_launcher_with_no_instances_at_all_draws_the_welcome_screen() {
        // The gate on its own, over every state a store can be in: this is the
        // whole of `Index.vue`'s own `isReady && !hasCreatedInstance` as this page
        // reads it, and the shell reads the same function for its quick-create key.
        assert!(first_run(&Load::Empty), "read, and there is nothing to play");
        assert!(first_run(&Load::Ready(Vec::new())), "the same answer by hand");
        assert!(!first_run(&Load::Ready(sample())), "a library with instances in it");
        assert!(!first_run(&Load::Idle), "a page that has not asked yet");
        assert!(!first_run(&Load::Loading));
        assert!(!first_run(&Load::Failed("no".to_string())));

        // And the page it produces, over a root that really does answer `Empty`:
        // every theme draws it without the toolbar, the search field or a card.
        let root = std::env::temp_dir().join("palantirmc-home-welcome").join("first-run");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a scratch root");
        let store = Store::load(&palantir_core::paths::PalantirPaths::at(root));
        assert!(
            matches!(store.instances(), Load::Empty),
            "nothing to find on a scratch root"
        );
        let state = State::default();
        for theme in Gen::ALL {
            drop(view(*theme, &state, &store));
            // The other side of the gate, drawn the way the page draws it: a test
            // cannot make a `Store` hold cards without a filesystem, so the card's
            // page goes through the same body the library arm builds.
            drop(library_view(*theme, &state, &sample()));
        }
    }

    #[test]
    fn a_first_run_gets_the_welcome_screen_and_nothing_else() {
        // The welcome screen's two actions are the reference's. Create is reported
        // to the shell -- the dialog, the request and the folder are its -- and
        // import is still the page's to answer for, which it does by saying so.
        let mut state = State::default();
        assert!(state.notice.is_none());
        state.update(Message::CreateInstance);
        assert!(
            state.notice.is_none(),
            "creating is the shell's to do now, so the page says nothing about it"
        );
        state.update(Message::ImportFromLauncher);
        assert!(
            state.notice.is_none(),
            "importing is reported to the shell too, and the page keeps its own state"
        );
        state.update(Message::Search("x".to_string()));
        assert_eq!(state.search, "x");
        // Opening an instance is the shell's, and a page that acted on it would be
        // a page that thought it could change which page is on screen: it neither
        // sets a notice nor clears one.
        state.update(Message::Search("atm".to_string()));
        state.notice = Some("something the page was told".to_string());
        state.update(Message::Open("atm".to_string()));
        assert_eq!(state.notice.as_deref(), Some("something the page was told"));
        state.update(Message::DismissNotice);
        assert!(state.notice.is_none(), "dismissing clears what it was told");
    }

    #[test]
    fn every_state_of_the_page_draws_in_every_theme() {
        let cards = sample();
        let store = Store::default();
        let mut state = State::default();
        for theme in Gen::ALL {
            // The welcome screen, with a notice on it.
            state.notice = Some("x".to_string());
            drop(view(*theme, &state, &store));
        }
        // The library, and the empty search result.
        state.notice = None;
        for search in ["", "nothing matches this"] {
            state.search = search.to_string();
            for theme in Gen::ALL {
                let rendered = library_view(*theme, &state, &cards);
                drop(rendered);
            }
        }
    }

    /// The library's body without a `Store`, because a test cannot make one hold
    /// instances without a filesystem.
    /// The library's body without a `Store`, because a test cannot make one hold
    /// instances without a filesystem that `instances::load` recognises. The
    /// cards are built by the same function the page uses, so what this mirrors is
    /// the *grid*, and only that.
    ///
    /// `Column::new()` rather than `column![]`, because the test module globs its
    /// parent's names in and the macro and the function of that name then collide.
    fn library_view<'a>(
        theme: Gen,
        state: &'a State,
        cards: &'a [InstanceCard],
    ) -> Element<'a, Message> {
        let mut blocks: Vec<Element<'a, Message>> = vec![header(theme, state)];
        let visible = state.visible(cards);
        if visible.is_empty() {
            blocks.push(page::empty(theme, Key::AppLibrarySearchNoResultsTitle));
        } else {
            let mut grid = iced::widget::Column::new().spacing(GAP).width(Length::Fill);
            for card in visible {
                grid = grid.push(instance_card(theme, card));
            }
            blocks.push(grid.into());
        }
        page::body(blocks, GAP)
    }
}
