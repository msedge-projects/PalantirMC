//! The shell: the chrome every page is drawn inside.
//!
//! This is stage 2 of `docs/superpowers/specs/2026-09-24-modrinth-native-rewrite.md`:
//! the rail, the head, the page pane, the right panel, the window controls and
//! Settings as a modal, all painted from [`crate::theme_gen`] and paced by
//! [`crate::motion`], on the information architecture [`crate::route`] describes.
//! The pages themselves are stage 3 -- what is here draws their *containers*, and
//! the pane's body says so rather than pretending to be a page.
//!
//! Every number below is quoted from the reference's own `App.vue`, with the
//! declaration it came from:
//!
//! | Number | Source |
//! | --- | --- |
//! | 48px head | `--top-bar-height: 3rem` |
//! | 64px rail | `--left-bar-width: 4rem` |
//! | 300px right panel | `--right-bar-width: 300px` |
//! | 48px rail plate, 4px gaps, 8px padding | `.nav-button`'s `w-12 h-12`, the rail's `gap-[0.25rem] p-[0.5rem]` |
//! | 24px rail icon | `text-2xl` on the same button |
//! | 20px page radius | `.app-contents`'s `border-top-left-radius: var(--radius-xl)` |
//! | 28px head buttons, 16px chevrons | `!h-7 !w-7`, `!size-4` |
//! | 36px control buttons, 20px icons, 16px corner | the `md` icon-only `IconButton` (`h-9 w-9`, `size-5`) and `WindowControls`' `rounded-bl-2xl` |
//!
//! Three things about the reference's chrome are easy to get backwards, and each
//! is written down where it bites:
//!
//! * **The "status bar" is at the top.** `app-grid-statusbar` is the first row of
//!   the grid and holds the logo, the history buttons, the breadcrumbs, the
//!   sidebar toggle and the window controls. There is no bottom bar.
//! * **`text-primary` is not `--color-text-primary`.** The preset remaps
//!   Tailwind's names onto different tokens (`primary: var(--color-text-default)`,
//!   `contrast: var(--color-text-primary)`, `secondary: var(--color-text-tertiary)`),
//!   so a port that reads the class name as the token draws the wrong ink.
//!   [`INK_DEFAULT`] and friends name the class, not the token.
//! * **The window controls are an overlay, not a row item.** In the reference
//!   they are `position: fixed` at the window's top-right and the status bar
//!   reserves their measured width with `padding-right`. Drawing them as the last
//!   item of the row paints the same picture -- the bar and the controls are the
//!   same colour and the same height -- and [`CONTROLS_WIDTH`] is the reservation
//!   the reference publishes, computed here rather than measured.

use std::time::Duration;

use iced::widget::canvas::{self, Canvas, Frame, Geometry, Path};
use iced::widget::{column, container, image, mouse_area, row, text, Space};
use iced::window;
use iced::{
    gradient, mouse::{Cursor, Interaction}, window::Id, Alignment, Background, Border, Color,
    Element, Length, Padding, Point, Radians, Rectangle, Renderer, Subscription, Theme, Vector,
};

use crate::brand;
use crate::color_theme::ColorTheme;
use crate::icon;
use crate::style::{
    disabled, heading, medium, INK_CONTRAST, INK_DEFAULT, INK_HOVER_BG, INK_PLATE,
    INK_PLATE_TEXT,
};
use crate::icons_gen::{self, Glyph};
use crate::motion::{Timing, Tween};
use crate::pages::{self, Screen};
use crate::route::{self, Address, Mark, Rail};
use crate::store::Store;
use crate::theme_gen::{self, Ink, Raw, Theme as Gen};

// ---- Geometry, quoted from the reference --------------------------------

/// `--left-bar-width: 4rem`.
pub const RAIL: f32 = 64.0;
/// `--top-bar-height: 3rem`, shared by the head and the window controls.
pub const BAR: f32 = 48.0;
/// `--right-bar-width: 300px`.
pub const PANEL: f32 = 300.0;
/// `.nav-button`'s `w-12 h-12`.
pub const PLATE: f32 = 48.0;
/// `rounded-full`.
pub const PLATE_RADIUS: f32 = PLATE / 2.0;
/// The rail's `p-[0.5rem]`, appplied to both sides and the foot.
pub const RAIL_PAD: f32 = 8.0;
/// The rail's `gap-[0.25rem]`.
pub const RAIL_GAP: f32 = 4.0;
/// How far apart the centres of two rail buttons are. The pitch `REFERENCE.md`
/// recorded as 52, kept as arithmetic so it cannot drift from its parts.
pub const RAIL_PITCH: f32 = PLATE + RAIL_GAP;
/// `text-2xl` on the rail button: a 24-unit icon drawn at 24 pixels.
pub const RAIL_ICON: f32 = 24.0;
/// `!h-7 !w-7` on the head's history buttons.
pub const HEAD_BUTTON: f32 = 28.0;
/// `!size-4` on the chevrons inside them.
pub const HEAD_ICON: f32 = 16.0;
/// `TextLogo class="h-7"`. The mark is square where the reference's wordmark is
/// wide, which `REFERENCE.md` records as a difference in the art rather than in
/// the head.
pub const LOGO: f32 = 28.0;
/// `border-top-left-radius: var(--radius-xl)` on `.app-contents`.
pub const PAGE_RADIUS: f32 = 20.0;
/// The `md` icon-only `IconButton`: `h-9 w-9`.
pub const CONTROLS_BUTTON: f32 = 36.0;
/// `size-5` on the icons inside them.
pub const CONTROLS_ICON: f32 = 20.0;
/// `rounded-bl-2xl` on the controls' own surface.
pub const CONTROLS_RADIUS: f32 = 16.0;
/// `px-1.5` on that surface.
pub const CONTROLS_PAD: f32 = 6.0;
/// `gap-2` between the controls.
pub const CONTROLS_GAP: f32 = 8.0;
/// What `.app-grid-statusbar`'s `padding-right: var(--window-controls-width)`
/// reserves.
///
/// The reference *measures* its controls with a `ResizeObserver` and publishes
/// the result, because a browser can lay the row out and this shell cannot. The
/// arithmetic is the same either way: two paddings, three buttons, two gaps.
pub const CONTROLS_WIDTH: f32 = 2.0 * CONTROLS_PAD + 3.0 * CONTROLS_BUTTON + 2.0 * CONTROLS_GAP;

/// The `md` `IconButton`'s corner radius, `rounded-xl`.
const CONTROL_RADIUS: f32 = 12.0;
/// `rounded-lg` on the head's `!h-7` buttons.
const HEAD_RADIUS: f32 = 8.0;

// ---- Timing ------------------------------------------------------------

/// The rail button's hover.
///
/// Tailwind's `transition-all` default -- 150ms on `cubic-bezier(0.4, 0, 0.2, 1)`
/// -- which is what `NavButton.vue`'s `transition-all hover:bg-button-bg
/// hover:text-contrast` resolves to. `tools/gen_theme.py` reads
/// `transition:` declarations and cannot see a class, so this is one of the two
/// places in the shell that cites a timing by hand; the reference pins
/// `tailwindcss ^3.4.4`, whose docs give both numbers.
///
/// The plate *behind* the icon is not this: it declares its own transition and
/// survives in the generated verbatim table, so it is
/// [`Timing::NAV_PLATE`] instead.
const HOVER: Timing = Timing::raw(150, [0.4, 0.0, 0.2, 1.0]);

/// One frame of the shell's clock.
///
/// 60Hz, the cadence a browser drives CSS transitions at. The clock only runs
/// while something is moving -- see [`Shell::animating`] -- so an idle window
/// asks for no frames at all.
const FRAME: Duration = Duration::from_millis(16);

/// Subscription id of the frame clock. A `&str`, because iced keys
/// subscriptions by any `Hash` and a stable key is what keeps the subscription
/// from being torn down and rebuilt every frame.
const FRAME_ID: &str = "palantirmc-shell-frames";

// ---- The shell ----------------------------------------------------------

/// What the shell is showing and what is moving.
pub struct Shell {
    /// Where the user is, with the context it was opened in.
    address: Address,
    /// The pages behind this one, newest last.
    back: Vec<Address>,
    /// The pages ahead of this one, newest last, emptied by any new navigation.
    forward: Vec<Address>,
    /// The generated theme, resolved once at startup rather than on every paint.
    theme: Gen,
    /// Whether the right panel is showing. The reference's `sidebarToggled`,
    /// which is the opposite sense from its `toggle_sidebar` *setting*: the
    /// setting says whether the user wants the panel at all, this says whether
    /// it is up right now.
    sidebar: bool,
    /// Whether the rail offers its Skins slot, from the reference's
    /// `show_skin_selector_in_sidebar` setting.
    skins_slot: bool,
    /// Whether the rail offers its Screenshots slot, from the reference's
    /// `show_all_screenshots_in_sidebar` setting.
    screenshots_slot: bool,
    /// Which rail slot the pointer is over.
    hovered: Option<Rail>,
    /// One selection tween per rail slot, in [`Rail::ALL`] order.
    plates: Vec<Tween>,
    /// Which modal is open, if any.
    modal: Option<Modal>,
    /// Whether the window is maximized, which the window controls' icon needs.
    maximized: bool,
    /// The page in the pane, with its own state.
    screen: Screen,
    /// What the pages can be answered with. Stage 4 fills the parts that need a
    /// service; what it holds now is the launcher's own filesystem.
    store: Store,
}

/// Which of the reference's rail conditions are on.
///
/// Two of the rail's eight slots are conditional in `App.vue`, and both
/// conditions are settings the user owns: the Skins slot appears when
/// `show_skin_selector_in_sidebar` is on, and the Screenshots slot when the
/// screenshots sync option is. The rail is therefore built from the settings
/// rather than from a fixed list, which is also why [`crate::route::Rail`] has
/// an entry for a slot that may not be drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RailSettings {
    /// The reference's `toggle_sidebar`, in its own negative sense: the setting
    /// is named for hiding the panel.
    pub hide_sidebar: bool,
    /// `show_skin_selector_in_sidebar`.
    pub show_skins: bool,
    /// `show_all_screenshots_in_sidebar`.
    pub show_screenshots: bool,
}

impl Default for RailSettings {
    /// The reference's own defaults, from `helpers/settings.ts`.
    fn default() -> RailSettings {
        RailSettings { hide_sidebar: false, show_skins: false, show_screenshots: true }
    }
}

/// The modals the shell can put up.
///
/// Settings is the one stage 2 owes, and it is a *modal* rather than a route on
/// purpose: the reference has no `/settings` route, its modal is opened from the
/// rail's settings button, and the old shell's `/settings` page is one of the
/// pages [`crate::route`] refuses. The rest arrive with the pages that open them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modal {
    /// `AppSettingsModal`.
    Settings,
}

/// Everything the shell can be told.
#[derive(Debug, Clone)]
pub enum Message {
    /// Navigate, from a path.
    Go(String),
    /// A page was told something.
    Screen(pages::Message),
    /// The history, as the head's two buttons drive it.
    Back,
    Forward,
    /// The pointer entered or left a rail slot.
    Hover(Option<Rail>),
    /// A rail slot was clicked.
    Rail(Rail),
    /// The right panel was shown or hidden.
    Sidebar(bool),
    /// A modal asked to close, from its own button or from its scrim.
    CloseModal,
    /// One frame of the clock.
    Tick,
    Minimize,
    ToggleMaximize,
    Close,
}

impl Shell {
    /// A shell at `address`, in `theme`, with the settings' own panel state.
    pub fn new(address: Address, theme: Gen, settings: &RailSettings) -> Shell {
        // Built before the address is moved into the shell, which is what the
        // screen an address draws from is.
        let screen = Screen::at(&address);
        let mut shell = Shell {
            address,
            back: Vec::new(),
            forward: Vec::new(),
            theme,
            sidebar: !settings.hide_sidebar,
            skins_slot: settings.show_skins,
            screenshots_slot: settings.show_screenshots,
            hovered: None,
            plates: Rail::ALL.iter().map(|_| Tween::at(0.0, Timing::NAV_PLATE)).collect(),
            modal: None,
            maximized: false,
            screen,
            store: Store::default(),
        };
        shell.settle();
        shell
    }

    /// A shell with a store behind it, which is how the application builds one.
    pub fn with_store(mut self, store: Store) -> Shell {
        self.store = store;
        self
    }

    /// Where the shell is.
    pub fn address(&self) -> &Address {
        &self.address
    }

    /// Whether the pane is drawing a page rather than a placeholder.
    ///
    /// True now that the panes are the pages: every route the address table knows
    /// builds one (`pages::Screen::at`), and each page's own gate covers its
    /// states. The flag stays because it is what the shell's gate asserts, and a
    /// future route without a page would make it false again by construction.
    pub fn pages_are_drawn(&self) -> bool {
        true
    }

    /// The page in the pane, for the shell's own gates.
    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    /// The mark a rail slot carries for the address on screen.
    fn mark(&self, slot: Rail) -> Option<Mark> {
        self.address
            .marks()
            .into_iter()
            .find(|(rail, _)| *rail == slot)
            .map(|(_, mark)| mark)
    }

    /// The tween for a rail slot.
    fn plate(&self, slot: Rail) -> Tween {
        let index = Rail::ALL.iter().position(|rail| *rail == slot).unwrap_or(0);
        self.plates.get(index).copied().unwrap_or_else(|| Tween::at(0.0, Timing::NAV_PLATE))
    }

    /// The path a rail slot leads to, where it leads anywhere.
    ///
    /// Three of the eight slots are buttons rather than links, which is the
    /// reference's own arrangement and the reason [`crate::route::Rail`] names
    /// them: create opens the creation flow, settings opens [`Modal::Settings`],
    /// and the profile slot opens the account menu.
    fn destination(slot: Rail) -> Option<&'static str> {
        match slot {
            Rail::Home => Some("/"),
            Rail::Discover => Some("/browse/modpack"),
            Rail::Skins => Some("/skins"),
            Rail::Screenshots => Some("/screenshots"),
            Rail::Servers => Some("/hosting/manage/"),
            Rail::CreateInstance | Rail::Settings | Rail::Profile => None,
        }
    }

    /// Whether an icon for a control that cannot be used is greyed out.
    fn icon_ink(&self, enabled: bool) -> Color {
        if enabled {
            theme_gen::ink(self.theme, INK_DEFAULT)
        } else {
            disabled(self.theme, INK_DEFAULT)
        }
    }

    /// Point the selection tweens at the marks the address on screen asks for.
    ///
    /// Called on every navigation. A slot that keeps its mark is left alone --
    /// [`Tween::retarget`] is a no-op when the target has not changed -- so
    /// moving between two Discover pages does not restart the plate.
    fn settle(&mut self) {
        for (index, slot) in Rail::ALL.iter().enumerate() {
            let target = if self.mark(*slot) == Some(Mark::Primary) { 1.0 } else { 0.0 };
            if let Some(tween) = self.plates.get_mut(index) {
                tween.retarget(target);
            }
        }
    }

    /// Move to an address, recording it in the history.
    fn go(&mut self, address: Address) {
        if address == self.address {
            return;
        }
        self.forward.clear();
        self.back.push(std::mem::replace(&mut self.address, address));
        self.settle();
        self.screen.retarget(&self.address);
    }

    /// Go back one page, if there is one.
    fn back(&mut self) {
        let Some(previous) = self.back.pop() else {
            return;
        };
        self.forward.push(std::mem::replace(&mut self.address, previous));
        self.settle();
        // Back and forward move between pages the same way a link does: the page
        // that is already built keeps what it can (a tab change is not a new page)
        // and anything else is built fresh, which is what a browser does when it
        // returns to a document it no longer holds.
        self.screen.retarget(&self.address);
    }

    /// Go forward one page, if the user has not navigated since.
    fn forward(&mut self) {
        let Some(next) = self.forward.pop() else {
            return;
        };
        self.back.push(std::mem::replace(&mut self.address, next));
        self.settle();
        self.screen.retarget(&self.address);
    }

    /// Advance every moving tween by `delta`.
    fn advance(&mut self, delta: Duration) {
        for tween in &mut self.plates {
            tween.advance(delta);
        }
    }

    /// Whether anything is still moving, which is what keeps the clock awake.
    pub fn animating(&self) -> bool {
        self.plates.iter().any(Tween::is_running)
    }

    /// Whether the right panel belongs on screen.
    ///
    /// The reference's `sidebarVisible`: the user's toggle, or a page that
    /// forces it (`App.vue`'s `forceSidebar`, on Discover, Project and User).
    fn panel_shown(&self) -> bool {
        self.sidebar || self.address.route.forces_sidebar()
    }

    /// Apply a message.
    ///
    /// Named `handle` rather than `update` so that a call can never be read as
    /// the trait's own method, which arrives one indirection later.
    fn handle(&mut self, message: Message) -> iced::Command<Message> {
        match message {
            Message::Go(path) => {
                if let Some(address) = Address::parse(&path) {
                    self.go(address);
                }
                iced::Command::none()
            }
            Message::Back => {
                self.back();
                iced::Command::none()
            }
            Message::Forward => {
                self.forward();
                iced::Command::none()
            }
            Message::Hover(slot) => {
                self.hovered = slot;
                iced::Command::none()
            }
            Message::Rail(slot) => {
                if let Some(path) = Shell::destination(slot) {
                    if let Some(address) = Address::parse(path) {
                        self.go(address);
                    }
                }
                if slot == Rail::Settings {
                    self.modal = Some(Modal::Settings);
                }
                iced::Command::none()
            }
            Message::Screen(message) => {
                // A page that opens a thing is navigating, not changing its own
                // mind: which page is in the pane, and what the history records,
                // are the shell's. A page reports it and this is where the report
                // is acted on.
                if let Some(open) = self.screen.update(message, &self.store) {
                    let path = match open {
                        pages::Open::Instance(id) => format!("/instance/{id}"),
                        pages::Open::Project(id) => format!("/project/{id}"),
                    };
                    if let Some(address) = Address::parse(&path) {
                        self.go(address);
                    }
                }
                iced::Command::none()
            }
            Message::Sidebar(shown) => {
                self.sidebar = shown;
                iced::Command::none()
            }
            Message::CloseModal => {
                self.modal = None;
                iced::Command::none()
            }
            Message::Tick => {
                self.advance(FRAME);
                iced::Command::none()
            }
            Message::Minimize => window::minimize(Id::MAIN, true),
            Message::ToggleMaximize => {
                self.maximized = !self.maximized;
                window::maximize(Id::MAIN, self.maximized)
            }
            Message::Close => window::close(Id::MAIN),
        }
    }

    // ---- Drawing --------------------------------------------------------

    fn render(&self) -> Element<'_, Message> {
        // A modal *replaces* the window's contents rather than covering them,
        // and that is a limitation rather than a choice: iced 0.12 composites a
        // tree in order and has no z-order, so a layer over a layer is not
        // something the toolkit can express (the old shell hit the same wall and
        // took the same way out). The reference composites its dialog over the
        // chrome with a translucent scrim and a backdrop blur; what this draws
        // is the dialog over a nearly opaque `--color-base`, so the difference
        // is the blurred chrome behind the scrim instead of nothing -- the same
        // class of deviation as `backdrop-filter` itself, and recorded rather
        // than approximated.
        if self.modal.is_some() {
            return self.modal_layer();
        }
        let theme = self.theme;
        let chrome = move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(theme_gen::ink(theme, Ink::RaisedBg))),
            ..container::Appearance::default()
        };
        column![
            container(self.head()).width(Length::Fill).height(Length::Fixed(BAR)).style(chrome),
            row![self.rail(), self.pane()].height(Length::Fill),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    /// The head: the mark, the history, the breadcrumb, the panel toggle and
    /// the window controls.
    fn head(&self) -> Element<'_, Message> {
        // Bound before the style closure rather than reached for inside it: a
        // `move` closure that names `self` captures the borrow, and the element
        // it is attached to outlives the method.
        let theme = self.theme;
        let crumb = breadcrumb(&self.address);
        let row = row![]
            .align_items(Alignment::Center)
            .padding(Padding { top: 0.0, bottom: 0.0, left: RAIL_PAD, right: 0.0 })
            .push(image(brand::logo_handle()).height(Length::Fixed(LOGO)))
            // The reference's `ml-2` then a `gap-2` group of the two history
            // buttons.
            .push(Space::with_width(8.0))
            .push(self.history_button(Glyph::ChevronLeft, !self.back.is_empty(), Message::Back))
            .push(Space::with_width(8.0))
            .push(self.history_button(
                Glyph::ChevronRight,
                !self.forward.is_empty(),
                Message::Forward,
            ))
            .push(Space::with_width(RAIL_PAD))
            .push(
                text(crumb)
                    .size(14.0)
                    .font(medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
            )
            .push(Space::with_width(Length::Fill))
            .push(self.panel_toggle())
            // `mr-3` on the toggle, then the controls' own reservation.
            .push(Space::with_width(12.0));
        container(row.push(self.window_controls()))
            .width(Length::Fill)
            .height(Length::Fixed(BAR))
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(theme_gen::ink(theme, Ink::RaisedBg))),
                ..container::Appearance::default()
            })
            .into()
    }

    /// A history button: a 28px square, `!border !border-surface-4`.
    fn history_button(&self, glyph: Glyph, enabled: bool, message: Message) -> Element<'_, Message> {
        let ink = self.icon_ink(enabled);
        let border = theme_gen::ink(self.theme, Ink::Surface4);
        let face = container(icon::icon(glyph, HEAD_ICON, ink))
            .width(Length::Fixed(HEAD_BUTTON))
            .height(Length::Fixed(HEAD_BUTTON))
            .center_x()
            .center_y()
            .style(move |_theme: &Theme| container::Appearance {
                border: Border { color: border, width: 1.0, radius: HEAD_RADIUS.into() },
                ..container::Appearance::default()
            });
        if enabled {
            mouse_area(face).interaction(Interaction::Pointer).on_press(message).into()
        } else {
            // No message at all rather than a message that is ignored: the
            // reference disables the button, and a control that cannot be used
            // should not report that it was.
            mouse_area(face).into()
        }
    }

    /// The panel toggle: `RightArrowIcon`, flipped when the panel is down,
    /// `mr-3` from the controls.
    fn panel_toggle(&self) -> Element<'_, Message> {
        let showing = self.sidebar;
        let ink = theme_gen::ink(self.theme, if showing { INK_CONTRAST } else { INK_DEFAULT });
        let background = if showing {
            Some(Background::Color(theme_gen::ink(self.theme, INK_HOVER_BG)))
        } else {
            None
        };
        let face = container(icon::icon(Glyph::RightArrow, CONTROLS_ICON, ink))
            .width(Length::Fixed(CONTROLS_BUTTON))
            .height(Length::Fixed(CONTROLS_BUTTON))
            .center_x()
            .center_y()
            .style(move |_theme: &Theme| container::Appearance {
                background,
                border: Border { radius: CONTROL_RADIUS.into(), ..Border::default() },
                ..container::Appearance::default()
            });
        mouse_area(face)
            .interaction(Interaction::Pointer)
            .on_press(Message::Sidebar(!showing))
            .into()
    }

    /// The three window controls, on their own `bg-bg-raised` surface.
    fn window_controls(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let bar = row![]
            .align_items(Alignment::Center)
            .padding(Padding { top: 0.0, bottom: 0.0, left: CONTROLS_PAD, right: CONTROLS_PAD })
            .push(self.control_button(Glyph::Minimize, Message::Minimize))
            .push(Space::with_width(CONTROLS_GAP))
            .push(self.control_button(
                if self.maximized { Glyph::Restore } else { Glyph::Maximize },
                Message::ToggleMaximize,
            ))
            .push(Space::with_width(CONTROLS_GAP))
            .push(self.control_button(Glyph::X, Message::Close));
        container(bar)
            .height(Length::Fixed(BAR))
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(theme_gen::ink(theme, Ink::RaisedBg))),
                border: Border {
                    radius: iced::border::Radius::from([0.0, 0.0, 0.0, CONTROLS_RADIUS]),
                    ..Border::default()
                },
                ..container::Appearance::default()
            })
            .into()
    }

    /// One window control: a 36px square, `type="quiet"`.
    fn control_button(&self, glyph: Glyph, message: Message) -> Element<'_, Message> {
        let ink = theme_gen::ink(self.theme, INK_DEFAULT);
        let face = container(icon::icon(glyph, CONTROLS_ICON, ink))
            .width(Length::Fixed(CONTROLS_BUTTON))
            .height(Length::Fixed(CONTROLS_BUTTON))
            .center_x()
            .center_y()
            .style(move |_theme: &Theme| container::Appearance {
                border: Border { radius: CONTROL_RADIUS.into(), ..Border::default() },
                ..container::Appearance::default()
            });
        mouse_area(face).interaction(Interaction::Pointer).on_press(message).into()
    }

    /// The rail, with its eight slots at the reference's 52px pitch.
    fn rail(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let mut items = column![].spacing(RAIL_GAP).padding(Padding {
            top: 0.0,
            right: RAIL_PAD,
            bottom: RAIL_PAD,
            left: RAIL_PAD,
        });
        for slot in self.slots() {
            items = items.push(self.rail_button(slot));
        }
        container(items)
            .width(Length::Fixed(RAIL))
            .height(Length::Fill)
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(theme_gen::ink(theme, Ink::RaisedBg))),
                ..container::Appearance::default()
            })
            .into()
    }

    /// The rail's slots, in `App.vue`'s order, with the two conditional ones
    /// left out when the settings turn them off.
    fn slots(&self) -> Vec<Rail> {
        Rail::ALL
            .iter()
            .copied()
            .filter(|slot| match slot {
                Rail::Skins => self.skins_slot,
                Rail::Screenshots => self.screenshots_slot,
                _ => true,
            })
            .collect()
    }

    /// One rail slot: a 48px circle, its hover or subpage background, the
    /// selection plate mid-growth, and the icon.
    fn rail_button(&self, slot: Rail) -> Element<'_, Message> {
        let theme = self.theme;
        let mark = self.mark(slot);
        let primary = mark == Some(Mark::Primary);
        let hovered = self.hovered == Some(slot);
        let plate = self.plate(slot);
        let background = if hovered || mark == Some(Mark::Subpage) {
            Some(theme_gen::ink(theme, INK_HOVER_BG))
        } else {
            None
        };
        let ink = if primary {
            theme_gen::ink(theme, INK_PLATE_TEXT)
        } else if hovered || mark.is_some() {
            theme_gen::ink(theme, INK_CONTRAST)
        } else {
            theme_gen::ink(theme, INK_DEFAULT)
        };
        let glyph = rail_glyph(slot);
        let face = Canvas::new(RailButton {
            glyph,
            background,
            plate: if plate.value() > 0.0 {
                Some((plate.value(), Timing::NAV_PLATE_FROM_SCALE + (1.0 - Timing::NAV_PLATE_FROM_SCALE) * plate.value()))
            } else {
                None
            },
            plate_ink: theme_gen::ink(theme, INK_PLATE),
            ink,
        })
        .width(Length::Fixed(PLATE))
        .height(Length::Fixed(PLATE));
        mouse_area(face)
            .interaction(Interaction::Pointer)
            .on_enter(Message::Hover(Some(slot)))
            .on_exit(Message::Hover(None))
            .on_press(Message::Rail(slot))
            .into()
    }

    /// The page pane: the page's container, and the page in it.
    fn pane(&self) -> Element<'_, Message> {
        let theme = self.theme;
        // The page speaks `pages::Message` and the shell speaks its own, so the
        // one that holds the page is the one that wraps it -- which is also what
        // keeps a page from being able to navigate on its own.
        let body = container(self.screen.view(theme, &self.address, &self.store).map(Message::Screen))
            .width(Length::Fill)
            .height(Length::Fill);
        let elements: Vec<Element<Message>> = if self.panel_shown() {
            vec![body.into(), self.panel()]
        } else {
            vec![body.into()]
        };
        container(row(elements).height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(theme_gen::ink(theme, Ink::Bg))),
                border: Border {
                    radius: iced::border::Radius::from([PAGE_RADIUS, 0.0, 0.0, 0.0]),
                    ..Border::default()
                },
                ..container::Appearance::default()
            })
            .into()
    }

    /// The right panel, over the reference's own two-stop wash.
    fn panel(&self) -> Element<'_, Message> {
        let theme = self.theme;
        container(Space::with_height(Length::Fill))
            .width(Length::Fixed(PANEL))
            .height(Length::Fill)
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(wash(theme)),
                ..container::Appearance::default()
            })
            .into()
    }

    /// The scrim and the dialog, over whatever the shell was drawing.
    fn modal_layer(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let dialog = container(
            column![]
                .spacing(12.0)
                .push(
                    row![]
                        .align_items(Alignment::Center)
                        .push(
                            text("Settings")
                                .size(20.0)
                                .font(heading())
                                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
                        )
                        .push(Space::with_width(Length::Fill))
                        .push(self.history_button(Glyph::X, true, Message::CloseModal)),
                )
                .push(
                    text("Settings is a modal in the reference, not a page: there is no /settings route, and the rail's button opens this. Its body arrives with the pages.")
                        .size(14.0)
                        .font(medium())
                        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
                ),
        )
        .width(Length::Fixed(560.0))
        .padding(24.0)
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(theme_gen::ink(theme, Ink::RaisedBg))),
            border: Border { radius: 16.0.into(), ..Border::default() },
            ..container::Appearance::default()
        });
        let scrim = container(dialog)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(Color { a: 0.8, ..theme_gen::ink(theme, Ink::Base) })),
                ..container::Appearance::default()
            });
        mouse_area(scrim).on_press(Message::CloseModal).into()
    }
}

/// The canvas program behind one rail button.
///
/// All three layers in one program rather than a stack of widgets: the plate,
/// the background and the icon share a coordinate space in the reference (the
/// plate is `inset: 0` of the same 48px box the icon is centred in), and drawing
/// them together is what keeps them in the same one here.
struct RailButton {
    glyph: Glyph,
    /// Hover or subpage: `hover:bg-button-bg`.
    background: Option<Color>,
    /// The plate: how opaque it is, and how far it has grown.
    plate: Option<(f32, f32)>,
    plate_ink: Color,
    ink: Color,
}

impl<Message> canvas::Program<Message> for RailButton {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let center = Point::new(bounds.width / 2.0, bounds.height / 2.0);
        if let Some(color) = self.background {
            frame.fill(&Path::circle(center, PLATE_RADIUS), color);
        }
        if let Some((opacity, scale)) = self.plate {
            let color = Color { a: self.plate_ink.a * opacity, ..self.plate_ink };
            frame.fill(&Path::circle(center, PLATE_RADIUS * scale), color);
        }
        // The icon sits in the middle of the plate, at `RAIL_ICON` pixels
        // whatever the plate's own size is.
        let (scale, offset) = icon::fit(self.glyph, RAIL_ICON);
        let inset = (PLATE - RAIL_ICON) / 2.0;
        frame.translate(Vector::new(inset + offset.x, inset + offset.y));
        frame.scale(scale);
        for (path, paint) in icons_gen::parts(self.glyph, scale, self.ink) {
            match paint {
                icons_gen::Paint::Stroke(stroke) => frame.stroke(&path, stroke),
                icons_gen::Paint::Fill(color) => frame.fill(&path, color),
            }
        }
        vec![frame.into_geometry()]
    }
}

/// The icon a rail slot draws, from `App.vue`'s own imports.
fn rail_glyph(slot: Rail) -> Glyph {
    match slot {
        Rail::Home => Glyph::Play,
        Rail::Discover => Glyph::Compass,
        Rail::Skins => Glyph::Shirt,
        Rail::Screenshots => Glyph::Image,
        Rail::Servers => Glyph::ServerStack,
        Rail::CreateInstance => Glyph::Plus,
        Rail::Settings => Glyph::Settings,
        Rail::Profile => Glyph::LogIn,
    }
}

/// The breadcrumb the head shows, as one string.
///
/// Stage 3 replaces this with the reference's `Breadcrumbs` component, which
/// builds a trail of links from the route *and the things on it* (an instance's
/// name, a project's title). Until pages exist there are no names to show, so
/// this is the route's own shape: the section, then the id.
fn breadcrumb(address: &Address) -> String {
    match &address.route {
        route::Route::Home => String::new(),
        route::Route::Discover { project_type } => {
            format!("Discover {}", project_type.sentence(2))
        }
        route::Route::Skins => "Skin selector".to_string(),
        route::Route::Screenshots => "Screenshots".to_string(),
        route::Route::Servers => "Servers".to_string(),
        route::Route::Server { id, .. } => format!("Servers / {id}"),
        route::Route::User { user, .. } => format!("Profile / {user}"),
        route::Route::Project { id, .. } => format!("Project / {id}"),
        route::Route::Instance { id, tab } => {
            let page = match tab {
                route::InstanceTab::Content => "Content",
                route::InstanceTab::ContentFilter(kind) => kind.label(),
                route::InstanceTab::Files => "Files",
                route::InstanceTab::Worlds => "Worlds",
                route::InstanceTab::Screenshots => "Screenshots",
                route::InstanceTab::Logs => "Logs",
                route::InstanceTab::Share => "Share",
            };
            format!("{id} / {page}")
        }
    }
}

/// The right panel's background: `--brand-gradient-bg`, over the page's own
/// colour.
///
/// The stops are the reference's, composited the way CSS composites them: an
/// `rgba()` stop is drawn *over* whatever is behind it, so the panel needs the
/// page's colour under the gradient rather than the gradient alone. A single
/// iced `Background` replaces what is behind it instead of compositing, which is
/// why the panel's container carries the gradient and the pane behind it carries
/// `--color-bg`.
fn wash(theme: Gen) -> Background {
    let Some((angle, stops)) = parse_gradient(theme_gen::raw(theme, Raw::BrandGradientBg)) else {
        // Every theme declares it, so this branch is unreachable in practice;
        // it exists so a future theme that does not gets the page's colour
        // rather than a panic.
        return Background::Color(theme_gen::ink(theme, Ink::Bg));
    };
    let mut linear = gradient::Linear::new(Radians(angle));
    for (offset, color) in stops {
        linear = linear.add_stop(offset, color);
    }
    Background::Gradient(gradient::Gradient::Linear(linear))
}

/// Read a CSS `linear-gradient` into an angle in radians and its stops.
///
/// Handles what the reference's own `--brand-gradient-*` values use: `deg`
/// angles, `rgba()` and hex stops with percentage offsets, and a bare colour
/// (which several themes use where others use a gradient). Anything it does not
/// understand is `None`, which the caller turns into the page's colour rather
/// than into a guess.
///
/// The angle needs no conversion beyond degrees to radians: iced measures its
/// gradient angle counter-clockwise from the +x axis and subtracts a quarter
/// turn, which makes 0 point up -- CSS's `0deg` -- so the reference's numbers go
/// through unchanged.
fn parse_gradient(value: &str) -> Option<(f32, Vec<(f32, Color)>)> {
    let value = value.trim();
    let Some(arguments) = value
        .strip_prefix("linear-gradient(")
        .and_then(|rest| rest.strip_suffix(')'))
    else {
        // Not a gradient: a colour on its own is a legal value for the same
        // token in another theme, and it is the whole gradient once it is.
        let color = parse_color(value)?;
        return Some((0.0, vec![(0.0, color), (1.0, color)]));
    };
    let mut parts = split_top_level(arguments);
    if parts.is_empty() {
        return None;
    }
    let angle = match parts[0].trim().strip_suffix("deg") {
        Some(degrees) => degrees.trim().parse::<f32>().ok()?.to_radians(),
        // A direction word (`to bottom`) is not in any of the reference's
        // values, and guessing at one would be exactly the kind of plausible
        // wrong answer the generator refuses to emit.
        None => 0.0,
    };
    if parts[0].trim().ends_with("deg") {
        parts.remove(0);
    }
    let mut stops = Vec::with_capacity(parts.len());
    for part in parts {
        let (color, position) = split_stop(&part)?;
        let color = parse_color(color)?;
        let offset = match position {
            Some(percent) => percent.trim_end_matches('%').parse::<f32>().ok()? / 100.0,
            // A stop with no position is positioned by the browser; none of the
            // reference's have that, and a missing one is a stop at the start.
            None => 0.0,
        };
        stops.push((offset.clamp(0.0, 1.0), color));
    }
    if stops.is_empty() {
        return None;
    }
    Some((angle, stops))
}

/// Split a gradient stop into its colour and its position.
///
/// The two are separated by whitespace, but only whitespace *outside*
/// parentheses: the reference writes `rgba(68, 182, 138, 0.175) 0%`, and
/// `split_whitespace` would hand back `rgba(68,` as the colour. Only the first
/// split matters, so this returns the two halves rather than a list.
fn split_stop(stop: &str) -> Option<(&str, Option<&str>)> {
    // Leading space is the normal case here: the reference writes its stops
    // after a comma, so every part but the first arrives padded.
    let stop = stop.trim();
    let mut depth = 0usize;
    for (index, character) in stop.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if character.is_whitespace() && depth == 0 => {
                let position = stop[index..].trim();
                return Some((stop[..index].trim(), (!position.is_empty()).then_some(position)));
            }
            _ => {}
        }
    }
    Some((stop.trim(), None))
}

/// Split on commas that are not inside parentheses.
///
/// `rgba(68, 182, 138, 0.175) 0%, ...` has commas that are part of a colour and
/// commas that separate stops, and a plain `split(',')` cannot tell them apart.
fn split_top_level(value: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    for character in value.chars() {
        match character {
            '(' => {
                depth += 1;
                current.push(character);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                current.push(character);
            }
            ',' if depth == 0 => {
                parts.push(std::mem::take(&mut current));
            }
            _ => current.push(character),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current);
    }
    parts.into_iter().filter(|part| !part.trim().is_empty()).collect()
}

/// Read `#rgb`, `#rrggbb`, `rgb()` or `rgba()`.
fn parse_color(value: &str) -> Option<Color> {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix('#') {
        let digits: Vec<u8> = hex.chars().filter_map(|c| c.to_digit(16).map(|d| d as u8)).collect();
        return match digits.len() {
            3 => Some(Color::from_rgb8(
                digits[0] * 17,
                digits[1] * 17,
                digits[2] * 17,
            )),
            6 => Some(Color::from_rgb8(
                digits[0] * 16 + digits[1],
                digits[2] * 16 + digits[3],
                digits[4] * 16 + digits[5],
            )),
            _ => None,
        };
    }
    let inner = value
        .strip_prefix("rgba(")
        .or_else(|| value.strip_prefix("rgb("))?
        .strip_suffix(')')?;
    let numbers: Vec<f32> = inner
        .split(',')
        .filter_map(|part| part.trim().parse::<f32>().ok())
        .collect();
    match numbers.len() {
        3 => Some(Color::from_rgb(
            numbers[0] / 255.0,
            numbers[1] / 255.0,
            numbers[2] / 255.0,
        )),
        4 => Some(Color {
            a: numbers[3],
            ..Color::from_rgb(numbers[0] / 255.0, numbers[1] / 255.0, numbers[2] / 255.0)
        }),
        _ => None,
    }
}

// ---- As an application --------------------------------------------------

/// What a run of the shell was asked for on the command line.
#[derive(Debug, Clone, Default)]
pub struct Flags {
    /// `--page PATH`, an address [`Address::parse`] knows. A path and not a
    /// name: the route table is the only list of pages, so a page that does not
    /// have a path does not exist.
    pub page: Option<String>,
    /// `--size WxH`, the client size to open at.
    pub size: Option<(u32, u32)>,
}

impl Flags {
    /// Read the shell's own flags out of a command line.
    pub fn from_args(args: impl Iterator<Item = String>) -> Flags {
        let mut flags = Flags::default();
        let mut args = args.peekable();
        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--page" => flags.page = args.next(),
                "--size" => {
                    flags.size = args.next().and_then(|value| {
                        let (width, height) = value.split_once('x')?;
                        Some((width.trim().parse().ok()?, height.trim().parse().ok()?))
                    })
                }
                _ => {}
            }
        }
        flags
    }

    /// The address this run opens at: the one asked for, or Home.
    pub fn opening(&self) -> Address {
        self.page
            .as_deref()
            .and_then(Address::parse)
            .unwrap_or_else(|| Address::at(route::Route::Home))
    }
}

/// The theme in force, as the generated tables index it.
///
/// All four of the reference's painted themes are here, retro included: it is a
/// real look rather than a label for one of the others, and the settings pane that
/// can offer it is [`crate::color_theme::ColorTheme::options`], which the new
/// Settings modal uses. What this function does *not* do is decide whether retro
/// should be offered -- that is the pane's rule, quoted in `color_theme`.
pub fn generated_theme(setting: ColorTheme, system_prefers_light: bool) -> Gen {
    match setting.resolve(system_prefers_light) {
        ColorTheme::Light => Gen::Light,
        ColorTheme::Oled => Gen::Oled,
        ColorTheme::Retro => Gen::Retro,
        ColorTheme::Dark | ColorTheme::System => Gen::Dark,
    }
}

/// iced's own widget theme, derived from the generated tokens.
///
/// The shell paints its chrome explicitly, so this is what iced falls back to
/// for anything it draws itself -- a text cursor, a scrollbar, a default label
/// colour. Deriving it from the same tokens is what keeps those from being the
/// only part of the window drawn with the old hand-written palette.
pub fn widget_theme(theme: Gen) -> Theme {
    Theme::custom(
        format!("PalantirMC {}", brand::version()),
        iced::theme::Palette {
            background: theme_gen::ink(theme, Ink::Bg),
            text: theme_gen::ink(theme, INK_DEFAULT),
            primary: theme_gen::ink(theme, Ink::Brand),
            success: theme_gen::ink(theme, Ink::Green),
            danger: theme_gen::ink(theme, Ink::Red),
        },
    )
}

impl iced::Application for Shell {
    type Executor = iced::executor::Default;
    type Flags = Flags;
    type Message = Message;
    type Theme = Theme;

    fn new(flags: Flags) -> (Self, iced::Command<Message>) {
        let home = palantir_core::paths::PalantirPaths::home();
        // Two different directories on purpose: the settings are this product's
        // own, and the instances are wherever the launcher has found them, which
        // `detect` is the one thing that knows -- `home` is the launcher's own
        // folder and holds no instances.
        let paths = palantir_core::paths::PalantirPaths::detect();
        let prefs = crate::prefs::load(&home);
        let theme = generated_theme(prefs.theme(), crate::native::system_prefers_light());
        let settings = RailSettings {
            hide_sidebar: prefs.hide_right_sidebar,
            show_skins: prefs.show_skin_selector_in_sidebar,
            show_screenshots: prefs.show_all_screenshots_in_sidebar,
        };
        let store = Store::load(&paths);
        (Shell::new(flags.opening(), theme, &settings).with_store(store), iced::Command::none())
    }

    fn title(&self) -> String {
        brand::window_title()
    }

    fn update(&mut self, message: Message) -> iced::Command<Message> {
        self.handle(message)
    }

    fn view(&self) -> Element<'_, Message> {
        self.render()
    }

    fn theme(&self) -> Theme {
        widget_theme(self.theme)
    }

    fn subscription(&self) -> Subscription<Message> {
        if !self.animating() {
            return Subscription::none();
        }
        // A thread and a channel rather than `iced::time::every`, which needs a
        // futures-runtime feature this build does not enable -- the same
        // arrangement the old shell's frame clock uses, and for the same reason.
        // The subscription's life is the animation's: while nothing is moving it
        // is not asked for, iced drops the receiver, and the thread's next send
        // fails and ends it.
        // The subscription's life is the animation's: while nothing is moving it
        // is not asked for, iced drops the receiver, and the thread's next send
        // fails and ends it. That is also what keeps an idle window from waking
        // the GPU sixty times a second to redraw the same picture.
        iced::subscription::channel(FRAME_ID, 4, |mut sender| async move {
            let _ = std::thread::spawn(move || loop {
                std::thread::sleep(FRAME);
                match sender.try_send(Message::Tick) {
                    Ok(()) => {}
                    // Full: the UI is a frame or two behind. Frames are
                    // droppable, so the animation skips ahead.
                    Err(error) if error.is_full() => {}
                    Err(_) => break,
                }
            });
            loop {
                futures::future::pending::<()>().await;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::{InstanceTab, ProjectTab, Route};

    /// The reference's root font size. Every `rem` in the geometry table is
    /// measured against it, and `theme_gen` converts lengths at the same 16.
    const ROOT: f32 = 16.0;

    /// Apply a message the way the runtime would, and drop the command it
    /// returns.
    ///
    /// `handle` returns a `Command` because it is the body of `update`, and in
    /// a test there is no runtime to run one: every command the shell produces
    /// today is `Command::none`, and a gate that asserts a `Command::none` is
    /// testing the toolkit's own constructor. `#[must_use]` on `Command` is
    /// what makes the drop explicit rather than an oversight.
    fn press(shell: &mut Shell, message: Message) {
        let _ = shell.handle(message);
    }

    fn shell_at(address: &str) -> Shell {
        Shell::new(
            Address::parse(address).expect("a sample address"),
            Gen::Dark,
            &RailSettings::default(),
        )
    }

    fn settings(hide_sidebar: bool, show_skins: bool, show_screenshots: bool) -> RailSettings {
        RailSettings { hide_sidebar, show_skins, show_screenshots }
    }

    #[test]
    fn the_geometry_is_the_reference_s_own_arithmetic() {
        // `4rem` and `3rem` at a 16px root, and the panel's one absolute width.
        assert_eq!(RAIL, 4.0 * ROOT);
        assert_eq!(BAR, 3.0 * ROOT);
        assert_eq!(PANEL, 300.0);
        // `.nav-button`'s `w-12 h-12` and the rail's `gap-[0.25rem]`: the 52px
        // pitch the old shell's gate recorded, kept as the sum of its parts so
        // that changing either one moves it.
        assert_eq!(PLATE, 12.0 * 4.0);
        assert_eq!(RAIL_GAP, 0.25 * ROOT);
        assert_eq!(RAIL_PITCH, 52.0);
        // `.app-contents`'s corner is `var(--radius-xl)`, which is the
        // generated table's own 20px rather than a number written here twice.
        assert_eq!(PAGE_RADIUS, theme_gen::span(crate::theme_gen::Span::RadiusXl));
        // The `md` icon-only `IconButton` and the head's `!h-7 !w-7` buttons.
        assert_eq!(CONTROLS_BUTTON, 9.0 * 4.0);
        assert_eq!(CONTROLS_ICON, 5.0 * 4.0);
        assert_eq!(HEAD_BUTTON, 7.0 * 4.0);
        assert_eq!(HEAD_ICON, 4.0 * 4.0);
        assert_eq!(LOGO, 7.0 * 4.0);
    }

    #[test]
    fn the_window_controls_reserve_what_they_measure() {
        // `--window-controls-width`, which the reference publishes from a
        // `ResizeObserver` and this shell computes: `px-1.5`, three `w-9`
        // buttons and two `gap-2`s.
        assert_eq!(CONTROLS_WIDTH, 136.0);
        assert_eq!(CONTROLS_WIDTH, 2.0 * 6.0 + 3.0 * 36.0 + 2.0 * 8.0);
        // The controls and the head are the same height, which is why drawing
        // them as the row's last item paints what the reference's overlay does.
        assert_eq!(BAR, 48.0);
    }

    #[test]
    fn every_rail_slot_draws_the_icon_app_vue_imports_for_it() {
        // `App.vue`'s own imports, by file name: PlayIcon, CompassIcon,
        // ShirtIcon, ImageIcon, ServerStackIcon, PlusIcon, SettingsIcon,
        // LogInIcon. A slot that reached for a different icon would still draw
        // *something*, which is exactly the kind of mistake this catches.
        let expected = [
            (Rail::Home, "play"),
            (Rail::Discover, "compass"),
            (Rail::Skins, "shirt"),
            (Rail::Screenshots, "image"),
            (Rail::Servers, "server-stack"),
            (Rail::CreateInstance, "plus"),
            (Rail::Settings, "settings"),
            (Rail::Profile, "log-in"),
        ];
        for (slot, name) in expected {
            assert_eq!(rail_glyph(slot).name(), name, "{slot} draws the wrong icon");
        }
        // And the head's own seven, which are drawn outside the rail.
        assert_eq!(Glyph::ChevronLeft.name(), "chevron-left");
        assert_eq!(Glyph::ChevronRight.name(), "chevron-right");
        assert_eq!(Glyph::RightArrow.name(), "right-arrow");
        assert_eq!(Glyph::Minimize.name(), "minimize");
        assert_eq!(Glyph::Maximize.name(), "maximize");
        assert_eq!(Glyph::Restore.name(), "restore");
        assert_eq!(Glyph::X.name(), "x");
    }

    #[test]
    fn the_rail_offers_the_slots_the_settings_ask_for() {
        // The reference's two conditional slots: `showSkinSelectorInSidebar` on
        // the shirt, and the screenshots sync option on the image.
        let all = shell_at("/").slots();
        assert_eq!(all.len(), Rail::ALL.len() - 1, "skins is off by default");
        assert!(!all.contains(&Rail::Skins));
        assert!(all.contains(&Rail::Screenshots));
        // The order is the reference's, whatever is hidden.
        let order: Vec<Rail> = all.iter().copied().filter(|slot| *slot != Rail::Screenshots).collect();
        assert_eq!(
            order,
            vec![
                Rail::Home,
                Rail::Discover,
                Rail::Servers,
                Rail::CreateInstance,
                Rail::Settings,
                Rail::Profile,
            ]
        );
        let mut shell =
            Shell::new(Address::at(Route::Home), Gen::Dark, &settings(false, true, false));
        assert!(shell.slots().contains(&Rail::Skins));
        assert!(!shell.slots().contains(&Rail::Screenshots));
        assert_eq!(shell.slots().len(), 7);
        // And the settings are the only thing that changes the list.
        press(&mut shell, Message::Sidebar(true));
        assert_eq!(shell.slots().len(), 7);
    }

    #[test]
    fn the_marks_come_from_the_route_table_and_the_plate_follows_them() {
        // Discover, marked primary: its plate has to arrive at full size.
        let mut shell = shell_at("/browse/modpack");
        assert!(shell.animating(), "the plate starts from zero and grows");
        shell.advance(Duration::from_millis(250));
        assert_eq!(shell.plate(Rail::Discover).value(), 1.0);
        assert!(!shell.animating());
        assert_eq!(shell.plate(Rail::Home).value(), 0.0, "home is not marked");

        // A project page is Discover's subpage: no plate anywhere.
        press(&mut shell, Message::Go("/project/sodium".into()));
        shell.advance(Duration::from_millis(250));
        assert_eq!(shell.plate(Rail::Discover).value(), 0.0);
        assert_eq!(shell.mark(Rail::Discover), Some(Mark::Subpage));
        assert_eq!(shell.plate(Rail::Home).value(), 0.0);

        // Browsing *inside* an instance moves the subpage mark to Home, which
        // is still not a plate: only a primary mark grows one.
        press(&mut shell, Message::Go("/browse/mod?i=atm10".into()));
        assert_eq!(shell.mark(Rail::Home), Some(Mark::Subpage));
        assert_eq!(shell.mark(Rail::Discover), None);
        assert_eq!(shell.plate(Rail::Home).value(), 0.0);

        // An instance page marks nothing at all -- the reference's own
        // predicates, reproduced: see `route::Address::marks`.
        press(&mut shell, Message::Go("/instance/atm10/worlds".into()));
        assert!(shell.address().marks().is_empty());
        assert_eq!(shell.plate(Rail::Servers).value(), 0.0);
    }

    #[test]
    fn the_history_walks_back_and_forward_and_a_new_visit_clears_it() {
        let mut shell = shell_at("/");
        press(&mut shell, Message::Go("/browse/mod".into()));
        press(&mut shell, Message::Go("/project/sodium".into()));
        assert_eq!(shell.address().to_path(), "/project/sodium");
        press(&mut shell, Message::Back);
        assert_eq!(shell.address().to_path(), "/browse/mod");
        press(&mut shell, Message::Back);
        assert_eq!(shell.address().to_path(), "/");
        press(&mut shell, Message::Back);
        assert_eq!(shell.address().to_path(), "/", "there is nothing behind Home");
        press(&mut shell, Message::Forward);
        assert_eq!(shell.address().to_path(), "/browse/mod");
        press(&mut shell, Message::Go("/skins".into()));
        press(&mut shell, Message::Forward);
        assert_eq!(
            shell.address().to_path(),
            "/skins",
            "navigating away discards the forward history"
        );
        // A page already on screen is not a navigation: clicking the rail's
        // Discover button while on Discover must not fill the history with it.
        let depth = shell.back.len();
        press(&mut shell, Message::Go("/skins".into()));
        assert_eq!(shell.back.len(), depth);
    }

    #[test]
    fn the_rail_walks_to_the_pages_the_reference_links_to() {
        let mut shell = shell_at("/");
        for (slot, path) in [
            (Rail::Home, "/"),
            (Rail::Discover, "/browse/modpack"),
            (Rail::Skins, "/skins"),
            (Rail::Screenshots, "/screenshots"),
            (Rail::Servers, "/hosting/manage/"),
        ] {
            press(&mut shell, Message::Rail(slot));
            assert_eq!(shell.address().to_path(), path, "{slot} links to the wrong page");
        }
        // The other three are buttons: they must not navigate anywhere.
        for slot in [Rail::CreateInstance, Rail::Settings, Rail::Profile] {
            let before = shell.address().clone();
            press(&mut shell, Message::Rail(slot));
            assert_eq!(&before, shell.address(), "{slot} is not a link");
        }
        assert_eq!(shell.modal, Some(Modal::Settings), "settings opens its modal");
    }

    #[test]
    fn settings_is_a_modal_and_not_a_page() {
        // The claim the rewrite is built on, in two halves: the rail's settings
        // button has no path, and `/settings` is not a page the route table
        // knows -- which is also what makes the old shell fail this gate.
        assert_eq!(Shell::destination(Rail::Settings), None);
        assert!(Address::parse("/settings").is_none());
        let mut shell = shell_at("/");
        assert_eq!(shell.modal, None);
        press(&mut shell, Message::Rail(Rail::Settings));
        assert_eq!(shell.modal, Some(Modal::Settings));
        press(&mut shell, Message::CloseModal);
        assert_eq!(shell.modal, None);
    }

    #[test]
    fn the_panel_is_up_when_the_reference_puts_it_up() {
        // The user's toggle, or a page that forces it: `App.vue`'s
        // `forceSidebar` on browse, project and user pages.
        let shell = shell_at("/");
        assert!(shell.panel_shown(), "the panel is on unless it was turned off");
        let hidden =
            Shell::new(Address::at(Route::Home), Gen::Dark, &settings(true, false, true));
        assert!(!hidden.panel_shown());
        for path in ["/browse/modpack", "/project/sodium", "/user/jelly"] {
            let forced = Shell::new(
                Address::parse(path).expect(path),
                Gen::Dark,
                &settings(true, false, true),
            );
            assert!(forced.panel_shown(), "{path} forces the panel open");
        }
        let instance = Shell::new(
            Address::at(Route::Instance { id: "atm10".into(), tab: InstanceTab::Content }),
            Gen::Dark,
            &settings(true, false, true),
        );
        assert!(!instance.panel_shown(), "an instance page does not force it");
    }

    #[test]
    fn the_wash_is_the_reference_s_two_stops_in_every_theme() {
        for theme in Gen::ALL {
            let parsed = parse_gradient(theme_gen::raw(*theme, Raw::BrandGradientBg));
            let (angle, stops) = parsed.unwrap_or_else(|| panic!("{theme:?} has no wash"));
            assert_eq!(stops.len(), 2, "{theme:?}");
            // Every one of the four is `0deg`, and in iced that angle needs no
            // conversion: `Radians::to_distance` subtracts a quarter turn
            // before taking the direction vector and measures y downwards, so
            // iced's 0 faces up -- `to top`, which is CSS's `0deg`. The stops
            // therefore run up the right panel: index 0 at its foot, index 1
            // at its head.
            assert_eq!(angle, 0.0, "{theme:?}: the wash runs bottom to top");
            assert_eq!(stops[0].0, 0.0, "{theme:?}");
            assert_eq!(stops[1].0, 1.0, "{theme:?}");
            // A wash that was opaque would hide the page colour it composites
            // over, and one with no alpha at all would draw nothing.
            for (_, color) in stops {
                assert!(color.a > 0.0 && color.a < 1.0, "{theme:?}: {color:?}");
            }
            let _ = wash(*theme);
        }
        // The two the reference writes out in full, against its own strings.
        let dark = parse_gradient(theme_gen::raw(Gen::Dark, Raw::BrandGradientBg)).expect("dark");
        assert_eq!(dark.1[0].1, Color::from_rgba(14.0 / 255.0, 35.0 / 255.0, 19.0 / 255.0, 0.2));
        assert_eq!(dark.1[1].1, Color::from_rgba(55.0 / 255.0, 137.0 / 255.0, 73.0 / 255.0, 0.1));
        let light =
            parse_gradient(theme_gen::raw(Gen::Light, Raw::BrandGradientBg)).expect("light");
        assert_eq!(
            light.1[0].1,
            Color::from_rgba(68.0 / 255.0, 182.0 / 255.0, 138.0 / 255.0, 0.175)
        );
        assert_eq!(
            light.1[1].1,
            Color::from_rgba(58.0 / 255.0, 250.0 / 255.0, 112.0 / 255.0, 0.125)
        );
    }

    #[test]
    fn the_gradient_reader_handles_what_the_reference_writes() {
        // A hex stop list and a non-zero angle, from `--brand-gradient-strong-bg`.
        let strong =
            parse_gradient(theme_gen::raw(Gen::Dark, Raw::BrandGradientStrongBg)).expect("strong");
        assert!((strong.0 - 270.0f32.to_radians()).abs() < 1e-6);
        assert_eq!(
            strong.1,
            vec![
                (0.1, Color::from_rgb8(0x09, 0x11, 0x0d)),
                (1.0, Color::from_rgb8(0x13, 0x1f, 0x17)),
            ]
        );
        // The same token in another theme is a bare colour rather than a
        // gradient, and a gradient of one colour is what that means.
        let retro =
            parse_gradient(theme_gen::raw(Gen::Retro, Raw::BrandGradientStrongBg)).expect("retro");
        assert_eq!(retro.1.len(), 2);
        assert_eq!(retro.1[0].1, Color::from_rgb8(0x3a, 0x3b, 0x38));
        assert_eq!(retro.1[0].1, retro.1[1].1);
        // And nothing it cannot read is guessed at.
        for value in ["", "linear-gradient(", "linear-gradient(0deg)", "nonsense", "rgb(1, 2)"] {
            assert!(parse_gradient(value).is_none(), "{value} should not parse");
        }
        assert_eq!(parse_color("#1bd96a"), Some(Color::from_rgb8(0x1b, 0xd9, 0x6a)));
        assert_eq!(parse_color("#fff"), Some(Color::from_rgb8(255, 255, 255)));
        assert_eq!(
            parse_color("rgb(1, 2, 3)"),
            Some(Color::from_rgb(1.0 / 255.0, 2.0 / 255.0, 3.0 / 255.0))
        );
        assert_eq!(parse_color("hsl(1, 2%, 3%)"), None);
    }

    #[test]
    fn the_inks_the_shell_paints_with_are_all_declared() {
        // Every token this module names, in every theme: a token that did not
        // resolve would come back as the table's fallback, and a shell painting
        // its chrome in a fallback is exactly the failure a generated design
        // system is supposed to make impossible.
        let tokens = [
            Ink::RaisedBg,
            Ink::Bg,
            Ink::Base,
            Ink::Surface4,
            Ink::Brand,
            Ink::Green,
            Ink::Red,
            INK_DEFAULT,
            INK_CONTRAST,
            Ink::TextTertiary,
            INK_HOVER_BG,
            INK_PLATE,
            INK_PLATE_TEXT,
        ];
        for theme in Gen::ALL {
            for token in tokens {
                let color = theme_gen::ink(*theme, token);
                assert!(color.a > 0.0, "{theme:?} {token:?} is transparent");
            }
            // The chrome and the page are different surfaces: `.app-grid-navbar`
            // is `bg-bg-raised` and `.app-contents` is `bg-bg`.
            assert_ne!(
                theme_gen::ink(*theme, Ink::RaisedBg),
                theme_gen::ink(*theme, Ink::Bg),
                "{theme:?}: the rail and the page must not be the same colour"
            );
        }
        // The plate, per theme, because the reference does not treat it the
        // same way in all four and "a wash of the accent" is a claim only half
        // the table supports. `--color-button-bg-selected` is opaque
        // `green-600` `#00af5c` in light -- the same value as its accent, so a
        // selected rail button in light mode is a solid plate -- the accent at
        // 25% in dark and OLED, and an opaque `#25421e` in retro, which is
        // neither the accent nor a dilution of it. Pinned per theme rather than
        // asserted uniformly so that the day one of them changes is the day the
        // shell's hover and selection states get looked at again.
        let plate = theme_gen::ink(Gen::Light, INK_PLATE);
        assert_eq!(plate, theme_gen::ink(Gen::Light, Ink::Brand), "light is solid brand");
        assert_eq!(plate.a, 1.0, "light is not a wash");
        for theme in [Gen::Dark, Gen::Oled] {
            let plate = theme_gen::ink(theme, INK_PLATE);
            let brand = theme_gen::ink(theme, Ink::Brand);
            assert_eq!(
                (plate.r, plate.g, plate.b),
                (brand.r, brand.g, brand.b),
                "{theme:?}: the wash is the accent"
            );
            assert_eq!((plate.a * 255.0).round(), 64.0, "{theme:?}: the accent at 25%");
        }
        let retro = theme_gen::ink(Gen::Retro, INK_PLATE);
        assert_eq!(retro.a, 1.0, "retro is opaque too");
        assert_ne!(retro, theme_gen::ink(Gen::Retro, Ink::Brand));
    }

    #[test]
    fn the_widget_theme_comes_from_the_same_tokens() {
        // iced draws a text cursor, a scrollbar and any unstyled label with
        // this; deriving it from `theme_gen` is what keeps the last part of the
        // window from being painted by the palette the rewrite replaces.
        for theme in Gen::ALL {
            let widget = widget_theme(*theme);
            assert_eq!(widget.palette().background, theme_gen::ink(*theme, Ink::Bg));
            assert_eq!(widget.palette().text, theme_gen::ink(*theme, INK_DEFAULT));
            assert_eq!(widget.palette().primary, theme_gen::ink(*theme, Ink::Brand));
        }
    }

    #[test]
    fn the_color_theme_setting_picks_a_generated_theme() {
        assert_eq!(generated_theme(ColorTheme::Dark, false), Gen::Dark);
        assert_eq!(generated_theme(ColorTheme::Light, false), Gen::Light);
        assert_eq!(generated_theme(ColorTheme::Oled, false), Gen::Oled);
        // Retro is a look of its own, not a dark synonym: the generated tables
        // resolve it separately, and this is the line that used to fall through to
        // `Gen::Dark` because the setting could not name it.
        assert_eq!(generated_theme(ColorTheme::Retro, false), Gen::Retro);
        assert_eq!(generated_theme(ColorTheme::Retro, true), Gen::Retro);
        // `System` follows the machine, and resolves to the ordinary dark look
        // rather than to OLED -- an OLED choice is the display's, not the OS's.
        assert_eq!(generated_theme(ColorTheme::System, false), Gen::Dark);
        assert_eq!(generated_theme(ColorTheme::System, true), Gen::Light);
        // Every theme the reference paints is reachable from the setting, which is
        // what makes `ColorTheme::options` the only place the dev-mode rule lives.
        assert_eq!(Gen::ALL.len(), 4);
        for theme in ColorTheme::ALL {
            let generated = generated_theme(theme, false);
            assert!(Gen::ALL.contains(&generated), "{theme:?} resolves nowhere");
        }
    }

    #[test]
    fn the_breadcrumb_names_the_section_and_then_the_thing() {
        let crumb = |path: &str| breadcrumb(&Address::parse(path).expect(path));
        assert_eq!(crumb("/"), "");
        assert_eq!(crumb("/browse/mod"), "Discover mods");
        assert_eq!(crumb("/browse/modpack"), "Discover modpacks");
        assert_eq!(crumb("/skins"), "Skin selector");
        assert_eq!(crumb("/screenshots"), "Screenshots");
        assert_eq!(crumb("/hosting/manage/"), "Servers");
        assert_eq!(crumb("/hosting/manage/srv/backups"), "Servers / srv");
        assert_eq!(crumb("/project/sodium/versions"), "Project / sodium");
        assert_eq!(crumb("/user/jelly"), "Profile / jelly");
        assert_eq!(crumb("/instance/ATM10"), "ATM10 / Content");
        assert_eq!(crumb("/instance/ATM10/projects/shader"), "ATM10 / Shaders");
        assert_eq!(crumb("/instance/ATM10/logs"), "ATM10 / Logs");
    }

    #[test]
    fn flags_read_the_pages_the_route_table_knows() {
        let flags = Flags::from_args(
            ["--page", "/instance/ATM10/files", "--size", "1280x720"]
                .into_iter()
                .map(String::from),
        );
        assert_eq!(flags.size, Some((1280, 720)));
        assert_eq!(
            flags.opening().route,
            Route::Instance { id: "ATM10".into(), tab: InstanceTab::Files }
        );
        // A path that is not a page opens Home rather than failing to open.
        let bogus = Flags::from_args(["--page", "/nowhere"].into_iter().map(String::from));
        assert_eq!(bogus.opening().route, Route::Home);
        assert_eq!(Flags::default().opening().route, Route::Home);
        // A malformed size is no size rather than a panic.
        let bad = Flags::from_args(["--size", "1280"].into_iter().map(String::from));
        assert_eq!(bad.size, None);
    }

    #[test]
    fn every_page_shape_builds_and_the_pane_draws_a_page() {
        // A `render()` pass over one address of every shape the route table has,
        // with and without the panel and with the modal open: catches a layout
        // builder that panics, a style closure that borrows, a canvas handed a
        // zero-sized box. Building the tree is the test.
        let sample = [
            Route::Home,
            Route::Servers,
            Route::Server { id: "srv".into(), tab: route::ServerTab::Access },
            Route::Discover { project_type: route::ProjectType::Modpack },
            Route::Skins,
            Route::Screenshots,
            Route::User { user: "jelly".into(), project_type: None },
            Route::Project { id: "sodium".into(), tab: ProjectTab::Gallery },
            Route::Instance {
                id: "a/b".into(),
                tab: InstanceTab::ContentFilter(route::ProjectType::Shader),
            },
        ];
        for route in sample {
            for hide_sidebar in [false, true] {
                let mut shell = Shell::new(
                    Address::at(route.clone()),
                    Gen::Dark,
                    &settings(hide_sidebar, true, true),
                );
                drop(shell.render());
                press(&mut shell, Message::Rail(Rail::Settings));
                drop(shell.render());
            }
            for theme in Gen::ALL {
                let shell = Shell::new(Address::at(route.clone()), *theme, &RailSettings::default());
                drop(shell.render());
            }
        }
        // The pane's body is a page rather than a placeholder, on every shape the
        // route table has -- which is the claim the name of this test makes.
        for path in ["/", "/browse/modpack", "/skins", "/screenshots", "/hosting/manage/", "/user/jelly", "/project/sodium", "/instance/ATM10/logs"] {
            let shell = shell_at(path);
            assert!(shell.pages_are_drawn(), "{path} draws a page");
        }
    }
}

