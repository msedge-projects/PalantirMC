//! Home, the reference's first rail slot: `pages/Index.vue`.
//!
//! Two states, exactly as the reference has two. With no instance yet it draws
//! `WelcomeScreen` -- a title, a description, the two buttons that start the
//! first instance, and a hint about the quick-create flow. With instances it draws
//! the library: the title, its search field and sort control, and a grid of cards.
//!
//! The reference's own `RecentWorldsList` sits above the library, and it is not
//! drawn here, which is a decision rather than an omission. Its own gate is
//! `v-if="recentInstances?.length > 0 && appSettings.getFeatureFlag('worlds_in_home')"`
//! (`pages/Index.vue`), and the flag is **on** in the reference's defaults --
//! `worlds_in_home: true` in `composables/use-app-settings.ts`, which the server's
//! `behavior.show_jump_in` then overrides (`App.vue`) -- so the gate that actually
//! decides is the length of the list. This launcher reads no worlds at all: the
//! block is a cross-instance scan of every `saves/` folder, and with nothing to
//! scan the reference would draw nothing here either. A capture of the reference
//! with worlds in it would show a strip this page has no data for.
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
use iced::widget::{column, container, image, mouse_area, row, Space};
use iced::{Alignment, Background, Border, Element, Length, Padding, Theme};

use crate::icon;
use crate::icons_gen::Glyph;
use crate::instances::InstanceCard;
use crate::page::{self, Load, GAP, ROW_GAP};
use crate::store::Store;
use crate::style::{heading, medium, regular, semibold, INK_CONTRAST, INK_DEFAULT, INK_SECONDARY};
use crate::text_gen::Key;
use crate::theme_gen::{self, Ink, Span, Theme as Gen};
// `Hovered` is in scope for the instance cards below: a card names its own
// crossing rather than going through one of the kit's controls.
use crate::ui::{self, text, Hovered};

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

    /// A wheel over this page's scroll region.
    ///
    /// Reported rather than applied: iced moves a scrollable with a `scroll_to`
    /// command, so which region glides, and how far, is the shell's -- see
    /// `crate::scroll`. This page's part is to hand the wheel on, and the name it
    /// carries is the region the widget was built with.
    Wheel(&'static str, crate::scroll::Wheel),
}

crate::hovered!(Message);

/// The library toolbar's create button, and the welcome screen's two.
const CREATE_KEY: &str = "home:create";
const IMPORT_KEY: &str = "home:import";
const WELCOME_CREATE_KEY: &str = "home:welcome:create";
const WELCOME_IMPORT_KEY: &str = "home:welcome:import";

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
            // A wheel is not this page's to apply: see `crate::scroll`.
            Message::Wheel(..) => {},

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
        //
        // **Drawn outside the page's scroll region, and that is the fix for a
        // defect rather than a style.** The reference's welcome is
        // `min-h-full`: the page is the viewport, the hero is centred in what
        // the header leaves, and the foot sits on the page's floor. That
        // minimum is exactly what a scroll region cannot give -- `iced_widget`
        // lays a scrollable's content out with `f32::MAX` on the axis it
        // scrolls, so a column asking for a vertical `Fill` inside one resolves
        // against an unbounded height, and what it draws is nothing. The screen
        // shipped that way: invisible on every platform, and unphotographed
        // because the machine it was written on could not capture its own
        // window (`PrintWindow` returns a surface without the page in it). The
        // pane's own container *is* bounded, so the same `Fill` below means the
        // minimum the reference means, at whatever height the window has. What
        // the page gives up is a scrollbar on a window shorter than the
        // welcome; at every size this shell opens at, the welcome fits, and
        // `min-h-full` draws no scrollbar at those sizes either.
        let mut page = column![].width(Length::Fill).height(Length::Fill).spacing(GAP);
        for block in blocks {
            page = page.push(block);
        }
        return container(page.push(welcome(theme)))
            .width(Length::Fill)
            .height(Length::Fill)
            .into();
    }
    match store.instances() {
        Load::Ready(cards) => {
            blocks.extend(library(theme, state, cards));
            page::body(blocks, GAP, Message::Wheel)
        }
        _ => page::body(
            vec![page::draw(theme, store.instances(), "your instances", |cards| {
                cards_placeholder(theme, cards)
            })],
            GAP,
            Message::Wheel,
        ),
    }
}

/// The library: the heading, the toolbar and the tiles.
///
/// `library/index.vue`'s own section, `flex flex-col gap-3`: the h2, the
/// toolbar, and the instances. The section's own `pb-16 min-h-[500px]` is not
/// drawn: the page's inset at the bottom of a scroll region is the same room,
/// and a 500-pixel minimum under two tiles is blank space with nothing in it.
///
/// The reference draws its instances as groups -- an `InstanceGroup` per custom
/// group, the ungrouped ones under a header that hides itself when it is the
/// only one. This launcher has one group's worth of instances and no model for
/// the grouping controls yet, so what is drawn is the ungrouped group: the
/// heading, the toolbar and the tiles, in the reference's own order.
fn library<'a>(
    theme: Gen,
    state: &'a State,
    cards: &'a [InstanceCard],
) -> Vec<Element<'a, Message>> {
    let visible = state.visible(cards);
    let mut blocks: Vec<Element<'a, Message>> = vec![
        drawn_title(theme, Key::AppLibraryTitle),
        toolbar(theme, state),
    ];
    // `library/index.vue`'s own branch:
    //
    // ```html
    // <div v-if="libraryGroupsLoaded && isSearching && visibleInstanceGroups.length === 0"
    //      class="text-base text-primary">{{ formatMessage(messages.noSearchResults) }}</div>
    // <Transition v-else ...>
    // ```
    //
    // `isSearching` is `search.value.length > 0` (`use-library.ts`), so the line
    // belongs to a search that matched nothing and *replaces* the groups rather
    // than joining them -- which is what `visible.is_empty()` says here, because a
    // library with no instances in it is the first run and never reaches this
    // branch. `text-primary` is `--color-text-default` in the app's own preset
    // (`tooling-config/tailwind-preset.ts`, `primary: 'var(--color-text-default)'`,
    // with `contrast` on `--color-text-primary`), which is [`INK_DEFAULT`] and not
    // the white [`INK_CONTRAST`] the word "primary" suggests.
    //
    // Measured on our own frame with the search filled (`/tmp/home-empty3.png`,
    // 1280x720, the library data root): the ink band is y221..234 at x91, in
    // (176,186,197) -- `#b0bac5`, `--color-text-default` -- 12 below the toolbar's
    // own bottom edge at y204, which is the section's `gap-3`, and at the page's
    // own 89-pixel inset (65 of rail plus the `p-6`), the same x the first tile
    // starts at. A 16-pixel line: the band is 14 tall, its ascender to descender.
    if visible.is_empty() {
        blocks.push(
            container(
                text(Key::AppLibrarySearchNoResultsTitle.message())
                    .size(16.0)
                    .font(regular())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
            )
            .width(Length::Fill)
            .into(),
        );
    } else {
        blocks.push(grid(theme, &visible));
    }
    blocks
}

/// `LibraryToolbar`: the search field with the create button, over the sort row.
///
/// The reference's toolbar is `flex flex-col gap-2` of two rows: the search
/// (`min-w-[16rem] flex-1`), *New group* and the brand *New instance*; then the
/// sort and group comboboxes, a `h-6 w-px` divider and the filter bar. Of those,
/// two need a model this launcher has not got and are not drawn rather than drawn
/// dead: *New group* needs the group store (`InstanceCard.group` is read from
/// disk but nothing writes one), and what the filter bar opens -- the
/// instance-type, game-version and loader dropdowns -- needs the three filters.
/// The divider and the filter mark around them are geometry, and are drawn; what
/// the page can act on is the search, the create button and the sort control, on
/// the rows the reference puts them on.
fn toolbar<'a>(theme: Gen, state: &'a State) -> Element<'a, Message> {
    let first = row![
        ui::search(
            theme,
            Key::AppLibrarySearchPlaceholder.message(),
            &state.search,
            Message::Search,
        ),
        ui::button_with_icon(
            theme,
            CREATE_KEY,
            Glyph::Plus,
            Key::AppLibraryInstanceNew,
            ui::Kind::Colored,
            Length::Shrink,
            Some(Message::CreateInstance),
        )
    ]
    .spacing(ROW_GAP)
    .align_items(Alignment::Center);
    // The sort control shows the chosen order; the combobox that opens to pick
    // another is the piece of the control this page still draws as a display
    // (see the module note above).
    //
    // The row is the reference's three children and not one: `library-toolbar/
    // index.vue` writes `<SortMenu />`, then
    // `<div class="mx-2 h-6 w-px bg-surface-5" />`, then `<FilterMenu />`. The
    // rule and the filter mark are the two pieces of that row that need no model
    // to draw, so they are drawn; what the filter *opens* -- the instance-type,
    // game-version and loader dropdown -- is the part with no model here, and is
    // named in the comment on [`toolbar`] rather than faked with a menu of
    // nothing.
    //
    // The rule is 1 wide and 24 tall (`h-6 w-px`, which is 1.5rem of `surface-5`).
    // Its `mx-2` is not drawn around it: the row's own `gap-2` ([`ROW_GAP`], 8)
    // already puts 8 on either side of a 1-pixel child, which is the same 8 the
    // margins ask for and one number instead of two.
    // A vertical rule fills whatever height it is given, and this one is `h-6`, so
    // the box that gives it that height is a container 1 wide and 24 tall -- which
    // is also the `w-px` the class asks for, so the two numbers are one box.
    let rule_line: Element<'a, Message> = iced::widget::Rule::vertical(TOOLBAR_RULE_WIDTH)
        .style(move |_theme: &Theme| iced::widget::rule::Appearance {
            color: theme_gen::ink(theme, Ink::Surface5),
            // The line's thickness is `Appearance::width`; `Rule::vertical`'s
            // own width is only the slot it is given.
            width: 1,
            radius: 0.0.into(),
            fill_mode: iced::widget::rule::FillMode::Full,
        })
        .into();
    let rule: Element<'a, Message> = container(rule_line)
        .width(Length::Fixed(TOOLBAR_RULE_WIDTH))
        .height(Length::Fixed(TOOLBAR_RULE_HEIGHT))
        .into();
    // `DropdownFilterBar.vue` with `use-filter-icon` set (which `filter-menu.vue`
    // sets) and nothing applied: the `v-if="showLabel"` span is
    // `flex h-10 items-center text-nowrap text-base font-medium text-primary`
    // holding `<FilterIcon class="size-5 text-primary" />` *instead of* the label
    // (`<template v-else>{{ effectiveLabel }}</template>`). So the whole control,
    // with no filter chosen, is a 20-pixel mark in `--color-text-default` --
    // [`INK_DEFAULT`], not the white [`INK_CONTRAST`], the class names aside (see
    // the preset quote on the empty state). The applied-filter chips that would
    // follow it are `size="lg"` outlined buttons, one per chosen filter, and
    // there are none.
    let filter = icon::icon(Glyph::Filter, FILTER_MARK, theme_gen::ink(theme, INK_DEFAULT));
    // `SortMenu`'s *second* combobox, which the row had been missing. It groups
    // by `displayState.group`, whose stored default is `'Group'`
    // (`use-library.ts`: `{ group: 'Group', sortBy: 'Last played', ... }`), and
    // `sort-menu.vue`'s own `groupLabels` maps `Group` to
    // `app.library.group-by.custom-group` -- "Custom group". So on a fresh
    // profile the reference's row reads *Last played* then *Custom group*, and
    // ours now says the same two things.
    //
    // Two halves of it are still missing and both are named here rather than
    // faked: the `#prefix` slot is `<LayoutGridIcon class="size-5 text-primary" />`,
    // a 20-pixel mark drawn *inside* the control's own frame, and `ui::select` --
    // `ui.rs`, another file -- has nowhere to put one, so both comboboxes here are
    // missing their mark (the sort one wants `ArrowUpDownIcon`); and the grouping
    // itself has no model, so this is a display beside the sort control, which is
    // one for the same reason.
    let group_by = ui::select(
        theme,
        Key::AppLibraryGroupByLabel,
        Key::AppLibraryGroupByCustomGroup.message(),
        COMBOBOX_WIDTH,
    );
    let second = row![
        ui::select(
            theme,
            Key::AppLibrarySortLabel,
            state.sort.label(),
            COMBOBOX_WIDTH
        ),
        group_by,
        rule,
        filter
    ]
    .spacing(ROW_GAP)
    .align_items(Alignment::Center);
    column![first, second].spacing(ROW_GAP).width(Length::Fill).into()
}

/// The reference's tile grid, as measured at the shell's own 1280-pixel window.
///
/// `instance-group/index.vue` measures its container and lays the tiles out at
/// `floor((width + gap) / (10rem + gap))` columns of
/// `(width - gap * (n - 1)) / n` with a `0.75rem` (12) gap, and a tile is
/// `cardWidth + 3.375rem` (54) tall: `p-3` (12 each side), the square art
/// (`cardWidth - 24`), `gap-3` (12) and the two text lines (a 20-pixel name and
/// an 18-pixel meta under a 4-pixel gap). At 1280 the pane is 915 wide -- 65 of
/// rail, 300 of right panel -- and the page's `p-6` leaves 867: five columns of
/// 163.8 and tiles 217.8 tall.
///
/// The page is handed no width -- a scroll region reports where it is, not how
/// wide it is (`scroll::Geometry`) -- so the count below is the reference's at
/// that window rather than a function of the window's own size. A wider window
/// keeps five wider tiles where the reference would add a column; the fix is a
/// width on the geometry the shell reports, not a second guess here.
/// `library-toolbar/index.vue`'s own rule between the sort group and the filter
/// bar: `<div class="mx-2 h-6 w-px bg-surface-5" />`. `w-px` is one pixel and
/// `h-6` is 1.5rem; the colour is `--surface-5` ([`Ink::Surface5`]).
const TOOLBAR_RULE_WIDTH: f32 = 1.0;
/// The same rule's height, `h-6` -- 24.
const TOOLBAR_RULE_HEIGHT: f32 = 24.0;

/// The width both of the toolbar's comboboxes are drawn at. `sort-menu.vue` gives
/// each `class="w-max"`, which is the control's own content -- the 20-pixel prefix
/// mark, the label and the chevron -- and this port's [`ui::select`] is a fixed
/// box, so one number stands for both.
const COMBOBOX_WIDTH: f32 = 200.0;

/// `DropdownFilterBar.vue`'s mark with `use-filter-icon` set and nothing applied:
/// `<FilterIcon class="size-5 text-primary" />`, and `size-5` is 1.25rem.
const FILTER_MARK: f32 = 20.0;

const TILE_GAP: f32 = 12.0;
const TILE_COLUMNS: usize = 5;
/// 1280 - 65 (rail) - 300 (panel) - 2 * 24 (the page's inset).
const GRID_WIDTH: f32 = 867.0;
const TILE_WIDTH: f32 =
    (GRID_WIDTH - TILE_GAP * (TILE_COLUMNS as f32 - 1.0)) / TILE_COLUMNS as f32;
/// `p-3`, which is also the art's inset inside the tile.
const TILE_PAD: f32 = 12.0;
/// `aspect-square min-w-full`: the art is as wide as the tile's inside.
const TILE_ART: f32 = TILE_WIDTH - TILE_PAD * 2.0;
/// `rounded-[20px]` on the tile, where the project cards' `rounded-lg` is 16.
const TILE_RADIUS: f32 = 20.0;

/// The tiles, five to a row.
fn grid<'a>(theme: Gen, cards: &[&'a InstanceCard]) -> Element<'a, Message> {
    let mut rows = column![].spacing(TILE_GAP).width(Length::Fill);
    for chunk in cards.chunks(TILE_COLUMNS) {
        let mut line = row![].spacing(TILE_GAP).width(Length::Fill);
        for card in chunk {
            line = line.push(tile(theme, card));
        }
        // The slots a short last row does not fill, so its tiles keep the
        // column's width instead of stretching across the row.
        for _ in chunk.len()..TILE_COLUMNS {
            line = line.push(Space::with_width(Length::Fixed(TILE_WIDTH)));
        }
        rows = rows.push(line);
    }
    rows.into()
}

/// One instance tile: `instance-card-view.vue`'s `flex-col items-start gap-3
/// rounded-[20px] p-3 bg-surface-3 border-surface-4`, and the whole tile opens
/// the instance.
///
/// The reference draws the instance's own icon in the art's square (`Avatar`,
/// tinted by the instance's id for the instances that have none); this page
/// cannot read a file -- a view gets no disk, and the launcher has no loader for
/// the icons directory yet -- so the box is drawn in the raised surface rather
/// than with borrowed art. The name and the `loader game-version` line under it
/// are the reference's two `truncate` lines; iced has no ellipsis, so a name
/// longer than the tile wraps and makes the tile taller, which is noted rather
/// than hidden.
fn tile<'a>(theme: Gen, card: &'a InstanceCard) -> Element<'a, Message> {
    let key = crate::ui::scoped("home:tile", &card.id);
    let (factor, _) = ui::interaction(key);
    let fill = crate::theme::brightness(theme_gen::ink(theme, Ink::Surface3), factor);
    let border = crate::theme::brightness(theme_gen::ink(theme, Ink::Surface4), factor);
    let art_fill = crate::theme::brightness(theme_gen::ink(theme, Ink::Surface5), factor);
    let name_ink = crate::theme::brightness(theme_gen::ink(theme, INK_CONTRAST), factor);
    let meta_ink = crate::theme::brightness(theme_gen::ink(theme, INK_DEFAULT), factor);
    let art = container(Space::new(Length::Fixed(TILE_ART), Length::Fixed(TILE_ART))).style(
        move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(art_fill)),
            border: Border {
                radius: theme_gen::span(Span::RadiusLg).into(),
                ..Border::default()
            },
            ..container::Appearance::default()
        },
    );
    let lines = column![]
        .spacing(4.0)
        .width(Length::Fill)
        .push(
            text(card.name.clone())
                .size(16.0)
                .font(semibold())
                .style(iced::theme::Text::Color(name_ink)),
        )
        .push(
            text(format!("{} {}", card.loader.label(), card.mc_version))
                .size(14.0)
                .font(medium())
                .style(iced::theme::Text::Color(meta_ink)),
        );
    let body = container(column![art, lines].spacing(TILE_GAP).align_items(Alignment::Start))
        .width(Length::Fixed(TILE_WIDTH))
        .padding(TILE_PAD)
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(fill)),
            border: Border { color: border, width: 1.0, radius: TILE_RADIUS.into() },
            ..container::Appearance::default()
        });
    mouse_area(body)
        .interaction(Interaction::Pointer)
        .on_enter(Message::hover_with(key, true, crate::theme::INSTANCE_CARD_HOVER_BRIGHTNESS))
        .on_exit(Message::hover_with(key, false, crate::theme::INSTANCE_CARD_HOVER_BRIGHTNESS))
        .on_press(Message::Open(card.id.clone()))
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
                .push(drawn_title(theme, Key::AppWelcomeScreenTitle))
                .push(
                    text(Key::AppWelcomeScreenDescription.message())
                        .size(16.0)
                        // `text-base leading-6 text-primary`: the preset's
                        // `primary` is `--color-text-default`, so this is the
                        // default ink at body weight, not white and not medium.
                        .font(regular())
                        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
                ),
        )
        .push(
            column![]
                .width(Length::Fixed(WELCOME_COLUMN))
                .align_items(Alignment::Center)
                .spacing(WELCOME_GAP)
                .push(welcome_button(
                    theme,
                    WELCOME_CREATE_KEY,
                    Glyph::Plus,
                    Key::AppWelcomeScreenCreateInstance,
                    true,
                    semibold(),
                    Message::CreateInstance,
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
                .font(regular())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
        )
        .push(welcome_button(
            theme,
            WELCOME_IMPORT_KEY,
            Glyph::Import,
            Key::AppWelcomeScreenImportFromLauncher,
            false,
            // The reference overrides this one with `!font-medium`, which is
            // where a coloured button's `font-semibold` label differs from a
            // standard one's.
            medium(),
            Message::ImportFromLauncher,
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

/// The welcome screen's title: `WelcomeScreen.vue`'s h1 is `text-2xl
/// font-semibold text-contrast` -- 24 at weight 600.
///
/// Drawn here rather than through [`page::title`] because that helper draws at
/// `--font-weight-heading` (800), the reference's default for a heading, which
/// this screen and the library's own h2 both override with `font-semibold`.
fn drawn_title<'a>(theme: Gen, key: Key) -> Element<'a, Message> {
    text(key.message())
        .size(24.0)
        .font(semibold())
        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST)))
        .into()
}

/// One of the welcome screen's two buttons, at the reference's own `lg` size.
///
/// `ButtonFrame.vue`'s `lg` row is `h-10 gap-2 rounded-[14px] px-4 text-base
/// font-semibold leading-5 [&>svg]:size-5`: 40 high, 16 of padding a side, 8
/// between the icon and the label, a 20-pixel icon and a 16-pixel label, a
/// 14-pixel corner. The kit's [`ui::button_with_icon`] draws the reference's
/// `md` row instead -- a 14-pixel label at 800, a 12-pixel corner, a 6-pixel
/// gap -- and draws it at whatever width it is handed, which is why the
/// reference's own 214x40 call to action was 289x41 here. `crate::ui` is
/// another agent's file in this slice, so the welcome's two buttons are built
/// at their own size here; when the kit grows the reference's five sizes, this
/// belongs inside it.
fn welcome_button<'a>(
    theme: Gen,
    key: &'static str,
    glyph: Glyph,
    label: Key,
    brand: bool,
    label_font: iced::Font,
    message: Message,
) -> Element<'a, Message> {
    let (factor, _) = ui::interaction(key);
    let ink = if brand {
        theme_gen::ink(theme, Ink::AccentContrast)
    } else {
        theme_gen::ink(theme, INK_CONTRAST)
    };
    let ink = crate::theme::brightness(ink, factor);
    let fill = if brand {
        theme_gen::ink(theme, Ink::Brand)
    } else {
        theme_gen::ink(theme, Ink::ButtonBg)
    };
    let face = container(
        row![
            icon::icon(glyph, 20.0, ink),
            text(label.message())
                .size(16.0)
                .font(label_font)
                .style(iced::theme::Text::Color(ink))
        ]
        .spacing(8.0)
        .align_items(Alignment::Center),
    )
    .height(Length::Fixed(ui::CONTROL))
    .padding(Padding { top: 0.0, bottom: 0.0, left: ui::BUTTON_PAD, right: ui::BUTTON_PAD })
    // `Shrink`, which is what content-sized means: the box is the row's own
    // width rather than the column's 288.
    .center_y()
    .style(move |_theme: &Theme| container::Appearance {
        background: Some(Background::Color(crate::theme::brightness(fill, factor))),
        border: Border { radius: 14.0.into(), ..Border::default() },
        ..container::Appearance::default()
    });
    mouse_area(face)
        .interaction(Interaction::Pointer)
        .on_enter(Message::hover(key, true))
        .on_exit(Message::hover(key, false))
        .on_press(message)
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
        .font(regular())
        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY)))
        .into()
}

/// The `<shortcut>` slot: a key, in the reference's chip.
///
/// `h-5 min-w-5 rounded-md border border-surface-5 bg-button-bg px-1 text-xs
/// font-normal leading-4 text-primary`: 20 tall and at least 20 wide, four of
/// padding a side, and a 12-pixel label at normal weight. `text-primary` is the
/// preset's `primary`, which is `--color-text-default` -- the default ink, not
/// the white `text-contrast` this drew before.
fn shortcut_chip<'a>(theme: Gen, key: &str) -> Element<'a, Message> {
    let label = ui::advance(key, regular(), 12.0);
    container(
        text(key.to_string())
            .size(12.0)
            .font(regular())
            .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
    )
    // `min-w-5` is the chip's own height, so a one-character key is square and a
    // longer one grows rather than wraps.
    .width(Length::Fixed((label + 8.0).max(SHORTCUT_HEIGHT)))
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
            last_launch_millis: 0,
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
    fn the_welcome_call_to_action_is_content_sized_and_no_wider_than_the_reference_s() {
        // The reference's own capture (1280x720, 2026-10-02): the call to action
        // is 214x40 at x415..628 and the hint's chip is 20 tall at y491..510. This
        // page drew the button at `Length::Fill`, which inside the `w-72` column is
        // 289 wide. What is asserted here is the arithmetic that replaced it -- the
        // label, its icon, the 8 between them and twice the button's padding,
        // which is `ButtonFrame.vue`'s `lg` row -- and the chip's own minimum.
        let cta = 2.0 * ui::BUTTON_PAD
            + 20.0
            + 8.0
            + ui::advance("Create an instance", semibold(), 16.0);
        assert!(
            cta <= 214.0,
            "the call to action measures {cta:.1}, wider than the reference's 214"
        );
        assert!(cta >= 190.0, "the call to action measures {cta:.1}, narrower than its own parts");
        // `min-w-5`: a one-character chip is square rather than 16 wide.
        assert!(ui::advance("N", regular(), 12.0) + 8.0 <= SHORTCUT_HEIGHT);
    }

    #[test]
    fn a_library_row_holds_five_tiles_at_the_shell_s_own_width() {
        // `instance-group/index.vue`'s own arithmetic: `floor((width + gap) /
        // (10rem + gap))` columns of `(width - gap * (n - 1)) / n`, tiles
        // `cardWidth + 3.375rem` tall, with a `0.75rem` gap. The pane inside a
        // 1280-pixel window is 915 wide (65 rail, 300 panel) and the page's `p-6`
        // leaves 867, which is five columns of 163.8 where the reference's own
        // capture has its first card at x89 and its last ending at x780.
        assert_eq!(GRID_WIDTH, 1280.0 - 65.0 - 300.0 - 48.0);
        let columns = ((GRID_WIDTH + TILE_GAP) / (160.0 + TILE_GAP)).floor() as usize;
        assert_eq!(columns, TILE_COLUMNS);
        assert!((TILE_WIDTH - 163.8).abs() < 0.05, "five tiles of 163.8");
        assert_eq!(TILE_ART, TILE_WIDTH - 24.0);
        assert!(TILE_ART < TILE_WIDTH, "the art leaves the padding beside it");
        // And every shape of list draws: empty, a short row, rows that are full,
        // and a list that ends in the middle of one.
        for count in [0, 1, 4, 5, 6, 13] {
            let cards: Vec<InstanceCard> = (0..count)
                .map(|index| card(&format!("i{index}"), "Instance", 0, "1.21.4", LoaderKind::Vanilla))
                .collect();
            let refs: Vec<&InstanceCard> = cards.iter().collect();
            for theme in Gen::ALL {
                drop(grid(*theme, &refs));
            }
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
    fn the_toolbar_s_own_rule_and_filter_mark_are_the_size_its_classes_say() {
        // `library-toolbar/index.vue`'s row is three children, and two of them
        // are geometry rather than behaviour: `<div class="mx-2 h-6 w-px
        // bg-surface-5" />` and, with `use-filter-icon` set and nothing applied,
        // `DropdownFilterBar`'s `<FilterIcon class="size-5 text-primary" />`.
        assert_eq!(TOOLBAR_RULE_WIDTH, 1.0, "`w-px` is one pixel");
        assert_eq!(TOOLBAR_RULE_HEIGHT, 24.0, "`h-6` is 1.5rem");
        assert_eq!(FILTER_MARK, 20.0, "`size-5` is 1.25rem");
        // The rule's own `mx-2` is not drawn around it: the row's `gap-2` puts
        // [`ROW_GAP`] either side of a 1-pixel child, which is the 17 the
        // reference's 8 + 1 + 8 comes to.
        assert_eq!(ROW_GAP * 2.0 + TOOLBAR_RULE_WIDTH, 17.0);
        // And the row draws, with the rule and the mark in it, over every theme.
        let cards = sample();
        let state = State::default();
        for theme in Gen::ALL {
            drop(library_view(*theme, &state, &cards));
        }
    }

    #[test]
    fn the_toolbar_s_row_reads_the_two_labels_a_fresh_profile_shows() {
        // `use-library.ts` stores `{ group: 'Group', sortBy: 'Last played', ... }`
        // as the grid's default display state, and `sort-menu.vue`'s own label
        // tables turn those into what the two comboboxes read: *Last played* and
        // `app.library.group-by.custom-group` = "Custom group". Both are drawn on
        // one row, so both are held here.
        assert_eq!(
            Key::AppLibraryGroupByCustomGroup.message(),
            "Custom group",
            "`Group` is the stored default, and this is the label it carries"
        );
        assert_eq!(Key::AppLibraryGroupByLabel.message(), "Group by");
        let state = State::default();
        assert_eq!(
            state.sort.label(),
            Key::AppLibrarySortLastPlayed.message(),
            "and the sort combobox opens on the order the reference stores"
        );
        // Both at one width, because `w-max` is the control's own content and this
        // port's select is a fixed box.
        assert_eq!(COMBOBOX_WIDTH, 200.0);
        let cards = sample();
        for theme in Gen::ALL {
            drop(library_view(*theme, &state, &cards));
        }
    }

    #[test]
    fn a_search_that_matched_nothing_draws_the_reference_s_own_line() {
        // The empty state is `library/index.vue`'s own branch, and it has two
        // halves that a test can hold without a screen: the gate and the words.
        //
        // The gate is `isSearching && visibleInstanceGroups.length === 0`, where
        // `isSearching` is `search.value.length > 0`. A library with instances in
        // it and a search that matches none is the only way to reach it, so the
        // test needs both: cards that exist, and a needle that finds none.
        let cards = sample();
        let mut state = State::default();
        assert!(!state.visible(&cards).is_empty(), "no search, so the grid");
        state.update(Message::Search("zzzqqq".to_string()));
        assert!(
            state.visible(&cards).is_empty(),
            "a needle nothing answers, which is `isSearching` here"
        );

        // The words are the message table's own, with the period the source's
        // `defaultMessage` carries: `app.library.search.no-results.title` is
        // "No instances match your search." A page that wrote its own sentence
        // would still look right in a capture.
        assert_eq!(
            Key::AppLibrarySearchNoResultsTitle.message(),
            "No instances match your search."
        );
        // And the branch is the one that draws: the library with nothing to show
        // renders, over every theme, without a card.
        for theme in Gen::ALL {
            drop(library_view(*theme, &state, &cards));
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
    /// instances without a filesystem that `instances::load` recognises. It is
    /// the page's own [`library`], so a test holds the blocks the page draws
    /// rather than a second copy of them that could drift.
    fn library_view<'a>(
        theme: Gen,
        state: &'a State,
        cards: &'a [InstanceCard],
    ) -> Element<'a, Message> {
        page::body(library(theme, state, cards), GAP, Message::Wheel)
    }
}
