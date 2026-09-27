//! The shell: the chrome every page is drawn inside.
//!
//! This shell is the reference's own: the rail, the head, the page pane, the right
//! panel, the window controls and Settings as a modal, all painted from
//! [`crate::theme_gen`] and paced by [`crate::motion`], on the information
//! architecture [`crate::route`] describes. Every page [`crate::route`] can address
//! is drawn, from [`crate::pages`].
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
use iced::widget::{column, container, image, mouse_area, row, scrollable, text, text_input, Space};
use iced::window;
use iced::{
    gradient, mouse::{Cursor, Interaction}, window::Id, Alignment, Background, Border, Color,
    Element, Length, Padding, Point, Radians, Rectangle, Renderer, Subscription, Theme, Vector,
};

use crate::accounts::{AccountEntry, AccountsStore};
use crate::anim;
use crate::brand;
use crate::color_theme::ColorTheme;
use crate::launch::{self, ActiveRunData, ChildSlot};

use crate::icon;
use crate::style::{
    disabled, heading, medium, semibold, INK_CONTRAST, INK_DEFAULT, INK_HOVER_BG, INK_PLATE,
    INK_PLATE_TEXT, INK_SECONDARY,
};
use crate::icons_gen::{self, Glyph};
use crate::install;
use crate::motion::{Timing, Tween};
use crate::page::{Load, ROW_GAP};
use crate::text_gen::Key;
use crate::pages::{self, discover, Screen};
use crate::route::{self, Address, Mark, Rail};
use crate::store::{self, Engine, Store};
use crate::ui::Hovered;
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
#[cfg(test)]
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

// ---- The panel's first section -----------------------------------------

/// `p-4` on each of the sidebar's sections, and `text-base` on the heading
/// inside one.
const PANEL_SECTION_PAD: f32 = 16.0;
const PANEL_HEADING: f32 = 16.0;
/// The accounts card's frame: `rounded-xl`, `p-3`, and the `mt-2` that holds it
/// off the heading.
///
/// `p-3` is the empty state's -- it is written on that branch of the card -- and
/// the accordion draws its own padding instead, which is why the frame's padding
/// is an argument rather than a constant of the frame.
const CARD_FRAME_PAD: f32 = 12.0;
const CARD_FRAME_RADIUS: f32 = 12.0;
const CARD_TOP: f32 = 8.0;
/// `gap-3` between what the empty card stacks, and `gap-2` between the parts of
/// the accordion's header and of an account row.
const CARD_STACK_GAP: f32 = 12.0;
const CARD_ROW_GAP: f32 = 8.0;
/// The accordion's header: `px-3 py-2`.
const CARD_HEAD_SIDE: f32 = 12.0;
const CARD_HEAD_PAD: f32 = 8.0;
/// `p-2` on an account row's own button.
const CARD_ROW_PAD: f32 = 8.0;
/// `w-5 h-5` on the radio marks and the header's chevron.
const CARD_MARK: f32 = 20.0;
/// `text-xs` on the card's own "Minecraft account" line.
const CARD_LABEL: f32 = 12.0;
/// `.button-base`'s `filter: brightness(0.85)` under the pointer, which is what
/// the card's two pressable surfaces declare: the accordion's header and an
/// account row.
///
/// Not [`crate::theme::hover_brightness`], which is the global brightening the
/// kit's controls use: `button-base` overrides it, so the card's own controls
/// dim rather than brighten, and they are dimmed by their own key so the two
/// ends are read from the same clock as everything else.
const CARD_PRESS_HOVER: f32 = 0.85;
/// The card's controls, each with its own name for the interaction clock.
const ACCOUNTS_HEADER: &str = "shell:accounts:header";
const ACCOUNTS_SIGN_IN: &str = "shell:accounts:sign-in";
const ACCOUNTS_ADD: &str = "shell:accounts:add";
const ACCOUNTS_NOTE_DISMISS: &str = "shell:accounts:note";

/// The `md` `IconButton`'s corner radius, `rounded-xl`.
const CONTROL_RADIUS: f32 = 12.0;
/// `rounded-lg` on the head's `!h-7` buttons.
const HEAD_RADIUS: f32 = 8.0;

// ---- Timing ------------------------------------------------------------

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
    /// Where a `--shot` run's picture is going, if this is one. The request is in
    /// place before the first frame, the timer [`Shell::capture`] asks for fires
    /// once the window has settled, and writing the frame is what ends the run.
    shot: Option<std::path::PathBuf>,
    /// Whether the frame has already been asked for, so a timer that fires twice
    /// cannot write the same file twice.
    shot_taken: bool,
    /// The page in the pane, with its own state.
    screen: Screen,
    /// What the pages are answered with: the launcher's own filesystem, and the
    /// engine for what has to come from a service.
    store: Store,
    /// The preferences the shell's own settings are drawn from.
    ///
    /// The shell owns the colour theme rather than being handed one, because
    /// Settings is where it changes: a choice made in the modal has to redraw the
    /// window it was made in, and be in the file for the next launch.
    prefs: crate::prefs::Prefs,
    /// Where that file is, once the application has said. `None` in a test, which
    /// is what keeps a test from writing to the real preferences.
    home: Option<palantir_core::paths::PalantirPaths>,
    /// The name in the creation dialog.
    create_name: String,
    /// What the last create could not do, shown in the dialog it was asked from.
    create_error: Option<String>,
    /// A create is in flight, which is what makes the dialog's button unusable
    /// rather than counted twice.
    creating: bool,
    /// The dialog's button was pressed and the request has not left yet.
    ///
    /// A flag rather than a return value because [`Shell::act`]'s only answer is
    /// [`discover::Asked`]: a create is not a page's request, and widening that
    /// return type would make every arm of its match say so.
    create_requested: bool,
    /// What the last import scan found, read once when the dialog opens rather
    /// than on every frame it is drawn: the scan walks other launchers' roots.
    import_found: Vec<crate::instances::ImportCandidate>,
    /// What the last import could not do, shown in the dialog it was asked from.
    import_error: Option<String>,
    /// An import is in flight.
    importing: bool,
    /// The row whose button was pressed, taken by `handle` to build the command.
    import_requested: Option<std::path::PathBuf>,
    /// Mojang's version list, as the creation dialog asked for it: read once per
    /// opening rather than per frame, for [`Shell::import_found`]'s reason -- the
    /// list is every version ever published, and it is a request.
    versions: Load<store::VersionList>,
    /// What the dialog's version search field holds.
    version_query: String,
    /// Whether the picker is showing everything Mojang publishes rather than the
    /// releases: the reference's `showSnapshots`, off until its footer is used.
    version_snapshots: bool,
    /// The version picked in the dialog. `None` is "whatever Mojang says is
    /// current", which is what the picker opens on.
    version_choice: Option<String>,
    /// The dialog was opened and the version request has not left yet. Taken by
    /// `handle` for [`Shell::create_requested`]'s reason: the dialog's own
    /// requests are not a page's, so they do not travel as an `Asked`.
    versions_requested: bool,
    /// Which modloader the dialog's chips are on.
    ///
    /// Fabric rather than Vanilla, because that is what the reference's flow
    /// opens on: vanilla is offered first -- the list is the same five chips in
    /// both -- and the choice the flow *makes* for a user who has not chosen is
    /// the loader most of the library is published for.
    create_loader: crate::catalog::LoaderKind,
    /// Which of the loader-version chips the dialog is on.
    build_choice: BuildChoice,
    /// The build the user picked when the chips are on [`BuildChoice::Other`].
    ///
    /// A version string rather than an index, because the list it came from is a
    /// service's answer that can be read again: an index would name a different
    /// build after a reload, and the row the user pressed is the version itself.
    build: Option<String>,
    /// What the dialog's build search field holds.
    build_query: String,
    /// The loader's own builds for the game version in force, as the dialog asked
    /// for them: read once per loader and game version rather than per frame, for
    /// [`Shell::versions`]'s reason.
    loader_builds: Load<Vec<store::LoaderBuild>>,
    /// The loader and game version the list above was asked for.
    ///
    /// Written down so a slow answer about a choice the user has moved off can be
    /// dropped rather than drawn: without it, a Fabric list that arrived after
    /// the Quilt chip was pressed would be drawn under the Quilt chip, which is a
    /// dialog that lies about what it will install.
    loader_builds_for: Option<(crate::catalog::LoaderKind, String)>,
    /// A chip was pressed and the loader-build request has not left yet. Taken by
    /// `handle` for [`Shell::versions_requested`]'s reason.
    loader_builds_requested: bool,
    /// The launch in flight: what the subscription streams under, and what the
    /// worker needs to start one. `None` is "nothing is running".
    run: Option<ActiveRunData>,
    /// How many runs this shell has asked for.
    ///
    /// It is what tells one run's messages from another's: a done for a run the
    /// shell has already replaced must not clear the one that is going.
    runs: u64,
    /// The game process, for the Stop button. Shared with the worker, which is
    /// the only other thing that touches it.
    child: ChildSlot,
    /// The launcher's accounts, read when a launch needs one and written when a
    /// launch renews a session. `None` in a test, which is what keeps a test from
    /// reading -- or writing -- a real `accounts.json`.
    accounts: Option<AccountsStore>,
    /// What reading the accounts file could not do, shown in Settings.
    ///
    /// [`crate::accounts::AccountsStore::load_with_report`] answers a corrupt file
    /// with an empty store and a warning, and the warning is deliberately not
    /// dropped: a launcher that silently forgot which account was signed in is the
    /// failure that reporting it exists to prevent.
    accounts_warning: Option<String>,
    /// Whether the accounts card's body is open. The reference's accordion is
    /// `open-by-default: false`, so the panel shows a header until it is pressed.
    accounts_open: bool,
    /// What one of the card's own controls could not do, drawn in the panel under
    /// it.
    ///
    /// The card's sign-in and add-account buttons open the Microsoft flow, which
    /// is a later stage's; the sentence is kept here rather than dropped because a
    /// control that does nothing at all is the failure mode this rewrite reports
    /// instead of hiding.
    accounts_note: Option<String>,
    /// The level the run in flight is at: the last [`launch::LaunchEvent::Progress`]
    /// its worker reported, kept as *numbers* rather than only as the sentence the
    /// instance's header draws.
    ///
    /// A bar needs a fraction and a state needs a word, and the same fact cannot
    /// be both if only the formatted line survives. It lives on the shell rather
    /// than on a page because a run outlives the page it was started from: this is
    /// what the action bar's download chip is drawn from, and it is what lets the
    /// launcher be watched from *any* page -- which the per-page line could not
    /// do, and is the whole reason the reference keeps this surface in its status
    /// bar.
    progress: Option<install::Progress>,
    /// Whether the download manager's panel is open: the reference's own toggle,
    /// off until its chip is pressed.
    downloads: bool,
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

/// The creation dialog's own button, which is one control on one dialog.
const CREATE_BUTTON: &str = "shell:create";

/// The action bar's four controls: the download chip (which is also the panel's
/// own toggle), the two the running chip carries, and the panel's close button.
const DOWNLOAD_CHIP: &str = "shell:download-chip";
const BAR_STOP: &str = "shell:bar-stop";
const BAR_LOGS: &str = "shell:bar-logs";
const BAR_PANEL_CLOSE: &str = "shell:bar-panel-close";

/// The reference's `py-1.5 px-3 rounded-xl` on both chips: a 20px icon and 6px
/// of padding on each side, which is 32.
const BAR_CHIP: f32 = 32.0;
/// How much of the download chip is the bar rather than the words. The reference
/// gives its own bar the whole trigger's width and the label to a marquee; a
/// fixed 64px is this toolkit's version of the same arrangement, and it is what
/// keeps the chip from resizing on every percentage.
const DOWNLOAD_BAR: f32 = 64.0;
/// `OnlineIndicatorIcon`'s dot.
const INDICATOR: f32 = 8.0;

/// The version picker's two footer states, each its own control: the same row
/// says one thing when the snapshots are hidden and the other when they are not.
const VERSION_SHOW_ALL: &str = "shell:version-show-all";
const VERSION_HIDE_SNAPSHOTS: &str = "shell:version-hide-snapshots";

/// Which loader build the creation dialog should install.
///
/// The reference's own three chips on its *Loader version* row, and the rule
/// each one names. `Stable` is the default arm in both shells -- it is what a
/// user almost always wants, and it is the one build per loader that has been
/// published as tested -- and `Other` is the only one that carries a version of
/// its own rather than deriving one from the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BuildChoice {
    /// The newest build the loader itself calls stable, or the newest of all
    /// when it has published nothing stable for this game version.
    #[default]
    Stable,
    /// The newest build there is, stable or not.
    Latest,
    /// A build the user picks out of the list.
    Other,
}

impl BuildChoice {
    /// The three chips, in the reference's own order.
    pub const ALL: [BuildChoice; 3] = [BuildChoice::Stable, BuildChoice::Latest, BuildChoice::Other];

    /// The chip's label, which is the reference's own message for it.
    pub fn label(self) -> Key {
        match self {
            BuildChoice::Stable => Key::CreationFlowModalCustomSetupLoaderVersionTypeStable,
            BuildChoice::Latest => Key::CreationFlowModalCustomSetupLoaderVersionTypeLatest,
            BuildChoice::Other => Key::CreationFlowModalCustomSetupLoaderVersionTypeOther,
        }
    }

    /// The chip's stable identity for the interaction clock: one per chip, not
    /// one per label, because two chips that happen to share a word must not
    /// light together.
    fn key(self) -> &'static str {
        match self {
            BuildChoice::Stable => "shell:create-build:stable",
            BuildChoice::Latest => "shell:create-build:latest",
            BuildChoice::Other => "shell:create-build:other",
        }
    }
}

/// The word for the state a run is in, drawn beside its name in the action bar.
///
/// The reference's bar carries no such word -- a process it can see is a process
/// that is running -- and this launcher has two states it cannot see from the
/// outside: preparing, and being stopped. Both are states in which the controls
/// beside the word are missing or about to be, which is exactly when a word is
/// worth drawing. *Running* is the arm with nothing to say, and it says nothing.
fn state_label(state: store::LaunchState) -> Option<&'static str> {
    match state {
        store::LaunchState::Idle | store::LaunchState::Running => None,
        store::LaunchState::Starting => Some(Key::InstanceActionStarting.message()),
        store::LaunchState::Stopping => Some(Key::InstanceActionStopping.message()),
    }
}

/// The name of an instance, for a surface that has only its id.
///
/// The library the pages are drawn from is where a name lives, and a run records
/// an id: an instance the list has not read yet -- or one that was removed while
/// it ran -- falls back to the id itself, which is what the reference's own bar
/// shows for an instance it cannot load.
fn instance_name(store: &Store, id: &str) -> String {
    store
        .instance(id)
        .ready()
        .map(|card| card.name.clone())
        .unwrap_or_else(|| id.to_string())
}

/// The download panel's own sentence: what is being fetched, how many of them are
/// done, and how many bytes it has cost.
///
/// The rate the reference shows beside these is deliberately absent: a rate needs
/// a clock and a window, and a number computed from one frame's difference is a
/// number that jumps. Bytes-to-date is the same fact without the lie.
fn progress_line(progress: &install::Progress) -> String {
    if progress.is_indeterminate() {
        return format!("{}…", progress.label);
    }
    let megabytes = progress.bytes as f64 / (1024.0 * 1024.0);
    format!(
        "{}: {} of {} · {megabytes:.1} MB",
        progress.label, progress.done, progress.total
    )
}

/// The modloader chips' identities, one per chip.
///
/// Written out rather than derived from the label, for [`BuildChoice::key`]'s
/// reason -- and because five names a chip can be is five names worth being able
/// to read in one place.
fn loader_chip_key(loader: crate::catalog::LoaderKind) -> &'static str {
    use crate::catalog::LoaderKind;
    match loader {
        LoaderKind::Vanilla => "shell:create-loader:vanilla",
        LoaderKind::Fabric => "shell:create-loader:fabric",
        LoaderKind::NeoForge => "shell:create-loader:neoforge",
        LoaderKind::Forge => "shell:create-loader:forge",
        LoaderKind::Quilt => "shell:create-loader:quilt",
    }
}

/// `Combobox.vue`'s `DEFAULT_MAX_HEIGHT`: its options stop growing at 300px and
/// scroll past that. The footers of the dropdown are not part of it -- they are
/// the dropdown's own last rows -- so this bounds the list and nothing else.
const VERSION_LIST_HEIGHT: f32 = 300.0;
/// The reference's option row: `px-4 py-3`.
const VERSION_ROW_SIDE: f32 = 16.0;
const VERSION_ROW_HEIGHT: f32 = 12.0;
/// `getOptionClasses`' `hover:brightness-[115%]` on an option that is not the
/// one in force. Not [`crate::theme::hover_brightness`]: the picker declares its
/// own hover end, which is what [`crate::ui::Hovered::hover_with`] is for.
const VERSION_ROW_HOVER: f32 = 1.15;

/// The modal's controls are the kit's, and the kit asks for a crossing.
///
/// Written by hand rather than through `crate::hovered!` because this message
/// family already has a `Hover` of its own -- the rail's, whose payload is a slot
/// rather than a key -- and a second variant of the same name would not compile.
impl crate::ui::Hovered for Message {
    fn hover(key: &'static str, over: bool) -> Message {
        Message::Control { key, over, hover: None }
    }

    fn hover_with(key: &'static str, over: bool, hover: f32) -> Message {
        Message::Control { key, over, hover: Some(hover) }
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
    /// The creation flow (`CreationFlowModal`): a name, and the button that
    /// makes the instance.
    Create,
    /// The same flow's import step: what the other launchers on this machine are
    /// holding, and a button per instance.
    Import,
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
    /// A colour theme was chosen in Settings.
    ColorTheme(ColorTheme),
    /// A control drawn by the shell itself published a pointer crossing.
    ///
    /// The rail has its own tween and its own message ([`Message::Hover`]); the
    /// modal's controls are built by [`crate::ui`], which asks for this.
    Control {
        /// The control's stable name, one per control.
        key: &'static str,
        /// Whether the pointer arrived or left.
        over: bool,
        /// The hover end, where the control declares one of its own.
        hover: Option<f32>,
    },
    /// The right panel was shown or hidden.
    Sidebar(bool),
    /// The accounts card's header: open its body or close it.
    ToggleAccounts,
    /// One of the card's rows: sign a launch in as this account.
    ///
    /// The uuid rather than the row's index, because the file is what the
    /// selection means -- the profile id is the account, and a list that was read
    /// again between the press and the frame cannot renumber it.
    SelectAccount(String),
    /// One of the card's rows: take this account away.
    RemoveAccount(String),
    /// The card's sign-in and add-account controls, which open the Microsoft
    /// flow this stage does not have yet.
    SignIn,
    /// The sentence under the card was read.
    DismissAccountsNote,
    /// The action bar's download chip: open or close the panel under the head.
    ToggleDownloads,
    /// The action bar's own stop control: end the run this launcher is in.
    StopRun,
    /// A modal asked to close, from its own button or from its scrim.
    CloseModal,
    /// The name in the creation dialog changed.
    CreateName(String),
    /// The creation dialog's own button: make the instance.
    Create,
    /// The answer to a create: the new instance's id, or why there is none.
    Created(Result<String, String>),
    /// One of the import dialog's rows: bring this instance in.
    Import(std::path::PathBuf),
    /// The answer to an import: the instance's new id, or why there is none.
    Imported(Result<String, String>),
    /// The answer to the dialog's version request: Mojang's list, or why there is
    /// none.
    Versions(Result<store::VersionList, String>),
    /// The dialog's version search field changed.
    VersionQuery(String),
    /// The picker's footer: show everything Mojang publishes, or only releases.
    VersionSnapshots(bool),
    /// A version was picked in the dialog.
    VersionChoice(String),
    /// A modloader chip was pressed: make the instance with this loader.
    LoaderChoice(crate::catalog::LoaderKind),
    /// One of the loader-version chips was pressed.
    BuildChoice(BuildChoice),
    /// The dialog's build search field changed.
    BuildQuery(String),
    /// A build was picked in the dialog's list.
    BuildPicked(String),
    /// The answer to the dialog's loader-build request: the loader's own builds
    /// for one game version, or why there are none.
    ///
    /// The question travels with the answer rather than being read from the
    /// dialog when it arrives, because the dialog may have moved on: `loader` and
    /// `game` are what tell an answer about the choice in force from one about a
    /// choice the user has replaced.
    LoaderBuilds {
        /// The loader the request was made for.
        loader: crate::catalog::LoaderKind,
        /// The game version it was made for.
        game: String,
        /// The builds, newest first, or the reason there are none.
        builds: Result<Vec<store::LoaderBuild>, String>,
    },
    /// One thing a launch reported: a batch of lines, a level, the game coming
    /// up, the run ending, or a session it renewed.
    ///
    /// The worker speaks [`launch::LaunchEvent`] rather than this enum because
    /// two shells watch a launch, and this variant is the whole of the crossing.
    Launched(launch::LaunchEvent),
    /// One frame of the clock.
    Tick,
    Minimize,
    ToggleMaximize,
    Close,
    /// The window's own state changed, reported by the window procedure: it is
    /// maximized now, or it is not, or the pointer moved on or off the maximize
    /// control. See [`window_state`] for why neither can be a widget's message.
    WindowStateChanged,
    /// A `--shot` run's settle timer fired: ask the window for its frame.
    ShotDue,
    /// The frame [`Message::ShotDue`] asked for, to be written and to end the run.
    ShotTaken(iced::window::Screenshot),
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
            shot: None,
            shot_taken: false,
            create_name: String::new(),
            create_error: None,
            creating: false,
            create_requested: false,
            import_found: Vec::new(),
            import_error: None,
            importing: false,
            import_requested: None,
            versions: Load::Idle,
            version_query: String::new(),
            version_snapshots: false,
            version_choice: None,
            versions_requested: false,
            create_loader: crate::catalog::LoaderKind::Fabric,
            build_choice: BuildChoice::default(),
            build: None,
            build_query: String::new(),
            loader_builds: Load::Idle,
            loader_builds_for: None,
            loader_builds_requested: false,
            run: None,
            runs: 0,
            child: std::sync::Arc::new(std::sync::Mutex::new(None)),
            accounts: None,
            accounts_warning: None,
            accounts_open: false,
            accounts_note: None,
            progress: None,
            downloads: false,
            screen,
            store: Store::default(),
            prefs: crate::prefs::Prefs::default(),
            home: None,
        };
        shell.settle();
        shell
    }

    /// A shell with a store behind it, which is how the application builds one.
    pub fn with_store(mut self, store: Store) -> Shell {
        self.store = store;
        self
    }

    /// The preferences the shell's own settings come from, and the file behind
    /// them.
    pub fn with_prefs(
        mut self,
        home: palantir_core::paths::PalantirPaths,
        prefs: crate::prefs::Prefs,
    ) -> Shell {
        self.prefs = prefs;
        self.home = Some(home);
        self
    }

    /// The launcher's accounts, which a launch signs in with, and what reading
    /// them could not do.
    pub fn with_accounts(mut self, accounts: AccountsStore, warning: Option<String>) -> Shell {
        self.accounts = Some(accounts);
        self.accounts_warning = warning;
        self
    }

    /// Take a colour theme: the window, the setting, and the file.
    ///
    /// A write that fails does not undo the choice, for [`crate::prefs`]'s
    /// reason: the window should still look the way it was just asked to, and a
    /// settings file that cannot be written is a problem for the next launch
    /// rather than a reason to refuse this one.
    fn choose_theme(&mut self, choice: ColorTheme) {
        self.prefs.color_theme = choice.id().to_string();
        self.theme = generated_theme(choice, crate::theme::os_prefers_light());
        if let Some(home) = &self.home {
            let _ = crate::prefs::save(home, &self.prefs);
        }
    }

    /// Where the shell is.
    #[cfg(test)]
    pub fn address(&self) -> &Address {
        &self.address
    }

    /// Whether the pane is drawing a page rather than a placeholder.
    ///
    /// True now that the panes are the pages: every route the address table knows
    /// builds one (`pages::Screen::at`), and each page's own gate covers its
    /// states. The flag stays because it is what the shell's gate asserts, and a
    /// future route without a page would make it false again by construction.
    #[cfg(test)]
    pub fn pages_are_drawn(&self) -> bool {
        true
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
        self.forget_pointer();
    }

    /// Forget where the pointer was on the page that is being left.
    ///
    /// A control the pointer was on is not drawn on the page that arrives, and
    /// the page's controls draw from the clock rather than keeping a pointer
    /// state of their own -- so without this a control the new page happens to
    /// name the same way would arrive lit.
    fn forget_pointer(&self) {
        if let Ok(mut clock) = anim::clock().lock() {
            clock.forget_pointer();
        }
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
        self.forget_pointer();
    }

    /// Go forward one page, if the user has not navigated since.
    fn forward(&mut self) {
        let Some(next) = self.forward.pop() else {
            return;
        };
        self.back.push(std::mem::replace(&mut self.address, next));
        self.settle();
        self.screen.retarget(&self.address);
        self.forget_pointer();
    }

    /// Advance every moving tween by `delta`.
    fn advance(&mut self, delta: Duration) {
        for tween in &mut self.plates {
            tween.advance(delta);
        }
    }

    /// Whether anything is still moving, which is what keeps the clock awake.
    ///
    /// Two clocks now, and both have to be asked. The rail's plates are the
    /// shell's own, and the pages' controls are [`crate::anim`]'s process-wide
    /// interaction clock -- a hover that started on a control is a frame
    /// subscription the shell owes it, or the tween would paint its first frame
    /// and sit there (see [`crate::hover`]).
    pub fn animating(&self) -> bool {
        if self.plates.iter().any(Tween::is_running) {
            return true;
        }
        anim::clock().lock().map(|clock| clock.animating()).unwrap_or(false)
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
    ///
    /// A message goes in and at most one command comes out, in two steps: the
    /// window controls, which are commands of their own rather than page state;
    /// then [`Shell::act`], whose only kind of answer is a request. A request is
    /// either the one the message asked for or the one the page it left behind
    /// owes -- never both, and never two.
    fn handle(&mut self, message: Message) -> iced::Command<Message> {
        // Hand the window's frame to Windows, as soon as there is a window.
        //
        // `native::install_hit_test` takes over `WM_NCHITTEST`, which is what
        // makes Windows run its own resize loop (with its own cursors, including
        // the diagonals iced has no way to ask for) and what puts Windows 11's
        // Snap Layouts on the maximize control. It cannot happen in
        // [`Shell::new`]: iced runs `Application::new` *before* it builds the
        // window. The first dispatched message is the earliest safe moment, and
        // there is one before the window is ever painted. Once it has taken
        // effect this is an atomic load, so it is called from every message
        // rather than tracked. A failure is not reported and not retried forever:
        // the window stays usable, and what is lost is the native cursors, the
        // resize edges and the flyout.
        let _ = crate::native::install_hit_test();
        match message {
            Message::Minimize => return window::minimize(Id::MAIN, true),
            Message::ToggleMaximize => {
                // With the shim in place this arm is not what the button does:
                // Windows answers that point as a caption button once
                // [`caption_target`] names it, so the click goes to the window
                // rather than to iced. This is what a window without the shim
                // has, and asking the window instead of inverting a flag keeps
                // the two paths from disagreeing.
                self.maximized = !crate::native::window_maximized().unwrap_or(self.maximized);
                return window::maximize(Id::MAIN, self.maximized);
            }
            Message::Close => return window::close(Id::MAIN),
            // The caption reads the window itself, so this arm's real job is to
            // be a message at all: it is what makes iced rebuild the view, and
            // the rebuild is what shows a maximize the user performed with Snap,
            // the taskbar or the keyboard.
            Message::WindowStateChanged => {
                self.maximized = crate::native::window_maximized().unwrap_or(self.maximized);
                return iced::Command::none();
            }
            // The capture is taken here, at the point the runtime is asked for
            // the window's frame, rather than by a tool outside the process:
            // iced draws this window, so it is the only thing that can hand back
            // exactly what was drawn. `write_shot` then writes it and the window
            // closes, which is what ends a capture run.
            Message::ShotDue => {
                if self.shot.is_some() && !self.shot_taken {
                    self.shot_taken = true;
                    return window::screenshot(Id::MAIN, Message::ShotTaken);
                }
                return iced::Command::none();
            }
            Message::ShotTaken(shot) => {
                if let Some(path) = self.shot.clone() {
                    if let Err(error) = write_shot(&path, &shot) {
                        eprintln!("shot failed: {error}");
                    }
                }
                return window::close(Id::MAIN);
            }
            _ => {}
        }
        if let Some(asked) = self.act(message) {
            return self.search(asked);
        }
        // A create and an import are not a page's requests and do not go through
        // `Asked`, so they are raised as flags by `act` and taken here: one press,
        // one command.
        if std::mem::take(&mut self.create_requested) {
            return self.create();
        }
        if let Some(source) = self.import_requested.take() {
            return self.import(source);
        }
        if std::mem::take(&mut self.versions_requested) {
            return self.versions_command();
        }
        if std::mem::take(&mut self.loader_builds_requested) {
            return self.loader_builds_command();
        }
        // Nothing was asked for, but a page may be on screen that has never
        // been asked anything -- a tab that was just switched, or the page a
        // window opened on. Both are the same answer: ask on its behalf.
        self.opening_command()
    }

    /// Apply a message, and answer with the request it made.
    ///
    /// Everything that is not a request is a change of state here, which is what
    /// keeps the return type one thing: a `None` means "nothing left this turn",
    /// and the only thing that can leave is a request.
    fn act(&mut self, message: Message) -> Option<discover::Asked> {
        match message {
            Message::Go(path) => {
                if let Some(address) = Address::parse(&path) {
                    self.go(address);
                }
                None
            }
            Message::Back => {
                self.back();
                None
            }
            // Neither is a request and neither reaches this far: `handle`
            // answers the window's own state and the capture's timer before it
            // asks `act` anything. They are matched here so that adding a
            // message is a compile error in the one place that has to decide
            // what to do with it.
            Message::WindowStateChanged | Message::ShotDue | Message::ShotTaken(_) => None,
            Message::Forward => {
                self.forward();
                None
            }
            Message::Hover(slot) => {
                self.hovered = slot;
                None
            }
            Message::ColorTheme(choice) => {
                self.choose_theme(choice);
                None
            }
            Message::CreateName(name) => {
                self.create_name = name;
                // The sentence was about the name that has just changed, so it is
                // not about this one.
                self.create_error = None;
                None
            }
            Message::Create => {
                if !self.creating {
                    // Marked busy as the request leaves rather than when it comes
                    // back: a second press before the answer arrives would create
                    // the instance twice.
                    self.creating = true;
                    self.create_requested = true;
                }
                None
            }
            Message::Import(source) => {
                if !self.importing {
                    self.importing = true;
                    self.import_requested = Some(source);
                }
                None
            }
            Message::Launched(event) => {
                self.launched(event);
                None
            }
            Message::Versions(result) => {
                // A list with nothing in it is `Empty` rather than a list: the
                // picker draws "no versions available" for one and a scrollable
                // of nothing for the other.
                self.versions = match result {
                    Ok(list) if list.versions.is_empty() => Load::Empty,
                    Ok(list) => Load::Ready(list),
                    Err(reason) => Load::Failed(reason),
                };
                // The loader's build list is asked for by game version, and the
                // version list is what tells the dialog which one is in force:
                // a dialog opened a moment ago has its answer here and nowhere
                // earlier, so this is where its own request is raised.
                if self.create_loader.loads_mods() && self.loader_builds_for.is_none() {
                    self.ask_loader_builds();
                }
                None
            }
            Message::VersionQuery(query) => {
                self.version_query = query;
                None
            }
            Message::VersionSnapshots(show) => {
                self.version_snapshots = show;
                None
            }
            Message::VersionChoice(id) => {
                self.version_choice = Some(id);
                // A different game version is a different set of loader builds,
                // so the list the dialog is holding is no longer the answer to
                // anything.
                self.ask_loader_builds();
                None
            }
            Message::LoaderChoice(loader) => {
                self.choose_loader(loader);
                self.ask_loader_builds();
                None
            }
            Message::BuildChoice(choice) => {
                self.build_choice = choice;
                // The list is only read under this chip, and a user who pressed
                // it has been told nothing yet if the request for it was never
                // made -- which is the arm a loader chip pressed before the
                // version list arrived lands in.
                if self.loader_builds_for.is_none() {
                    self.ask_loader_builds();
                }
                None
            }
            Message::BuildQuery(query) => {
                self.build_query = query;
                None
            }
            Message::BuildPicked(build) => {
                self.build = Some(build);
                None
            }
            Message::LoaderBuilds { loader, game, builds } => {
                // The answer is dropped unless it is the answer to the question
                // in force: see [`Shell::loader_builds_for`].
                if self.loader_builds_for.as_ref() == Some(&(loader, game)) {
                    // A list with nothing in it is `Empty` rather than a list,
                    // for [`Shell::versions`]'s reason: one draws "no builds for
                    // this game version" and the other a scrollable of nothing.
                    self.loader_builds = match builds {
                        Ok(builds) if builds.is_empty() => Load::Empty,
                        Ok(builds) => Load::Ready(builds),
                        Err(reason) => Load::Failed(reason),
                    };
                }
                None
            }
            Message::Imported(result) => {
                self.importing = false;
                match result {
                    Ok(id) => {
                        // The same two things a create does, for the same reasons:
                        // the list is read again, and the reader ends up in the
                        // instance that was brought in.
                        self.store.reload();
                        self.import_error = None;
                        self.modal = None;
                        self.go(Address::at(route::Route::Instance {
                            id,
                            tab: route::InstanceTab::Content,
                        }));
                    }
                    Err(reason) => self.import_error = Some(reason),
                }
                None
            }
            Message::Created(result) => {
                self.creating = false;
                match result {
                    Ok(id) => {
                        // The list the pages are drawn from was read at startup, so
                        // an instance that was just written is on disk and not on
                        // the page until it is read again -- and the flow leaves
                        // the reader in the instance it made.
                        self.store.reload();
                        self.create_error = None;
                        self.create_name.clear();
                        self.modal = None;
                        self.go(Address::at(route::Route::Instance {
                            id,
                            tab: route::InstanceTab::Content,
                        }));
                    }
                    // The dialog stays up with the reason in it: a failure the
                    // reader cannot see is a button that does nothing.
                    Err(reason) => self.create_error = Some(reason),
                }
                None
            }
            Message::Control { key, over, hover } => {
                crate::ui::pointer_with(
                    key,
                    over,
                    hover.unwrap_or_else(crate::theme::hover_brightness),
                );
                None
            }
            Message::Rail(slot) => {
                if let Some(path) = Shell::destination(slot) {
                    if let Some(address) = Address::parse(path) {
                        self.go(address);
                    }
                }
                // The two rail buttons that open a flow rather than a page: the
                // reference's `+` and the gear.
                match slot {
                    Rail::Settings => self.modal = Some(Modal::Settings),
                    Rail::CreateInstance => self.open_create(),
                    _ => {}
                }
                None
            }
            Message::Screen(message) => {
                // A page reports what it wants and this is where the report is
                // acted on: opening a thing is a navigation, and a search is a
                // request, and neither is the page's to perform.
                match self.screen.update(message, &self.store) {
                    Some(pages::Ask::Open(open)) => {
                        let path = match open {
                            pages::Open::Instance(id) => format!("/instance/{id}"),
                            pages::Open::Project(id) => format!("/project/{id}"),
                        };
                        if let Some(address) = Address::parse(&path) {
                            self.go(address);
                        }
                        None
                    }
                    Some(pages::Ask::Search(asked)) => Some(asked),
                    Some(pages::Ask::Create) => {
                        self.open_create();
                        None
                    }
                    Some(pages::Ask::Import) => {
                        self.open_import();
                        None
                    }
                    // The two that start and stop a game rather than a request:
                    // neither answers with one, so `None` -- what the shell owes
                    // the page afterwards travels as a launch event instead.
                    Some(pages::Ask::Play(id)) => {
                        self.play(id);
                        None
                    }
                    Some(pages::Ask::Stop(id)) => {
                        self.stop(&id);
                        None
                    }
                    None => None,
                }
            }
            Message::Sidebar(shown) => {
                self.sidebar = shown;
                None
            }
            Message::ToggleAccounts => {
                self.accounts_open = !self.accounts_open;
                None
            }
            Message::SelectAccount(uuid) => {
                self.select_account(&uuid);
                None
            }
            Message::RemoveAccount(uuid) => {
                self.remove_account(&uuid);
                None
            }
            Message::SignIn => {
                // `AccountsCard.vue`'s `login()` opens the reference's sign-in
                // modal. The flow it starts is a later stage's here, and the
                // card is where the press was made, so this is where the sentence
                // goes rather than nowhere.
                self.accounts_note = Some(store::not_implemented("Signing in to Minecraft"));
                None
            }
            Message::DismissAccountsNote => {
                self.accounts_note = None;
                None
            }
            Message::ToggleDownloads => {
                self.downloads = !self.downloads;
                None
            }
            Message::StopRun => {
                // The instance comes from the store rather than from a page: the
                // bar is drawn from what this launcher is running, and a page the
                // user has since navigated away from is not asked.
                if let Some(id) = self.store.launch().instance.clone() {
                    self.stop(&id);
                }
                None
            }
            Message::CloseModal => {
                self.modal = None;
                None
            }
            Message::Tick => {
                self.advance(FRAME);
                // The pages' controls advance on the same frame as the rail's
                // plates: one clock in the window, whichever page is drawing.
                if let Ok(mut clock) = anim::clock().lock() {
                    clock.tick(std::time::Instant::now());
                }
                None
            }
            // Handled before this, in `handle`: they are the messages whose
            // answer is a command rather than a request.
            Message::Minimize | Message::ToggleMaximize | Message::Close => None,
        }
    }

    /// Open the creation dialog, on a name nobody has typed yet and the version
    /// list asked for.
    ///
    /// The list is a request rather than a field read: it is Mojang's whole
    /// version manifest, and the dialog cannot offer a version it has not been
    /// told about. The flag carries it out of here for the reason
    /// [`Shell::create_requested`] exists -- `act`'s only answer is a page's
    /// request, and this is the dialog's.
    fn open_create(&mut self) {
        self.create_name.clear();
        self.create_error = None;
        self.creating = false;
        self.versions = Load::Loading;
        self.versions_requested = true;
        self.version_query.clear();
        self.version_snapshots = false;
        self.version_choice = None;
        // The reference's own opening choice, and no build: which builds exist
        // is a question for the game version, and the version list has not
        // arrived yet. The answer's arm raises the request.
        self.create_loader = crate::catalog::LoaderKind::Fabric;
        self.build_choice = BuildChoice::default();
        self.build = None;
        self.build_query.clear();
        self.loader_builds = Load::Idle;
        self.loader_builds_for = None;
        self.loader_builds_requested = false;
        self.modal = Some(Modal::Create);
    }

    /// Take the dialog's loader to `loader`.
    ///
    /// A build is a version of one *specific* loader, so the choices in force
    /// are not answers to anything once the loader changes -- which is why the
    /// reference resets its own build choice too. Pressing the chip that is
    /// already chosen changes nothing: the row always has one chip chosen, and
    /// `never-empty: false` on the reference's row is what lets its own selection
    /// go missing, which is a dialog with no loader in it that this one does not
    /// have an arm for.
    fn choose_loader(&mut self, loader: crate::catalog::LoaderKind) {
        if self.create_loader == loader {
            return;
        }
        self.create_loader = loader;
        self.build_choice = BuildChoice::default();
        self.build = None;
        self.build_query.clear();
        self.create_error = None;
    }

    /// Ask the loader's own service which builds it published for the game
    /// version in force.
    ///
    /// The request travels as a flag for [`Shell::versions_requested`]'s reason,
    /// and the pair it was asked for is written down here so that a slow answer
    /// about a choice the user has moved off can be dropped rather than drawn.
    ///
    /// A loader that does not load mods asks nothing: vanilla has no service and
    /// no build, so its arm clears the list rather than leaving the previous
    /// loader's on screen.
    fn ask_loader_builds(&mut self) {
        self.loader_builds_for = None;
        self.loader_builds_requested = false;
        if !self.create_loader.loads_mods() {
            self.loader_builds = Load::Idle;
            return;
        }
        let Some(game) = self.chosen_version() else {
            // Nothing to ask about yet: the version list is what names the game
            // version, and its own arm is where this request is raised.
            self.loader_builds = Load::Loading;
            return;
        };
        self.loader_builds = Load::Loading;
        self.loader_builds_for = Some((self.create_loader, game));
        self.loader_builds_requested = true;
    }

    /// The loader build the dialog's chips come to.
    ///
    /// The reference's own rule, read off the list in force: *Stable* is the
    /// newest build the loader calls stable -- or the newest of all when it has
    /// published nothing stable for this game version, which is
    /// `palantir_net::engine::default_build`'s rule and the reason Quilt's list
    /// still opens on something -- *Latest* is simply the newest, and *Other* is
    /// the build the user picked.
    ///
    /// `None` while the list is still coming, or when it failed, or when it came
    /// back with nothing in it: all three are "there is no build to install",
    /// and the dialog draws the button unusable rather than making an instance
    /// other than the one it named.
    fn chosen_build(&self) -> Option<String> {
        let builds = self.loader_builds.ready()?;
        match self.build_choice {
            BuildChoice::Stable => builds
                .iter()
                .find(|build| build.stable)
                .or_else(|| builds.first())
                .map(|build| build.version.clone()),
            BuildChoice::Latest => builds.first().map(|build| build.version.clone()),
            // A build the list no longer carries is not a build: the list is the
            // answer to what exists, and a version chosen against a list that has
            // since been read again is one this dialog cannot install.
            BuildChoice::Other => self
                .build
                .clone()
                .filter(|version| builds.iter().any(|build| &build.version == version)),
        }
    }

    /// Whether the dialog's own button can make the instance the dialog names.
    ///
    /// Unusable while a create is in flight, because a second press before the
    /// answer arrives would create the instance twice.
    ///
    /// Unusable while a loader is chosen whose build list has not answered, and
    /// *usable* once it has -- including when the answer is "none" or "it
    /// failed". That last part is deliberate: the list's own sentence is drawn on
    /// the loader-version row, so a reader who can see *no versions available for
    /// this game version* has been told what choosing this loader comes to, and a
    /// button that stayed dark would leave them with a dialog they cannot leave.
    /// The one arm this does not cover is the store's own: a create with a loader
    /// and no resolved build writes a vanilla instance and returns a warning, and
    /// that warning is dropped by [`crate::store::Store::create_instance`] today.
    /// It is the honest thing to fix next, and it is not this button's doing.
    fn create_usable(&self) -> bool {
        if self.creating {
            return false;
        }
        !self.create_loader.loads_mods() || self.loader_builds.settled()
    }

    /// Whether the *Stable* chip can be pressed at all.
    ///
    /// The reference disables it when the loader has published nothing stable
    /// for this game version (`disabledItems` on its own row, with *No such
    /// versions available* as the tooltip), so the chip stays in its place and
    /// goes dim rather than disappearing from the row.
    fn stable_offered(&self) -> bool {
        match self.loader_builds.ready() {
            Some(builds) => builds.iter().any(|build| build.stable),
            // While the list is coming, nothing is known to be missing: the chip
            // is left pressable rather than dimmed and then undimmed.
            None => true,
        }
    }

    /// Open the import step of the creation flow, on what this machine holds.
    ///
    /// The scan runs here rather than in the view: it walks every launcher root
    /// the launcher knows how to read, and a walk per frame is a walk per frame.
    fn open_import(&mut self) {
        self.import_found = self.store.importable();
        self.import_error = None;
        self.importing = false;
        self.modal = Some(Modal::Import);
    }

    /// Bring one instance in, off the frame thread.
    ///
    /// An import copies a whole instance tree -- mods, worlds, configs and all --
    /// which is why it goes where a create goes: a thread, and back as a message.
    fn import(&self, source: std::path::PathBuf) -> iced::Command<Message> {
        let store = self.store.clone();
        iced::Command::perform(
            crate::store::off_thread(move || store.import_instance(&source)),
            Message::Imported,
        )
    }

    /// Create the instance the dialog names, off the frame thread.
    ///
    /// A create writes a folder, a config and a version profile, so it goes where
    /// a search goes: a thread, and back as a message. What it is for is the
    /// version the picker is on; when there is none -- a dialog whose list never
    /// arrived -- the store asks Mojang itself, which is the same answer as the
    /// picker would have opened on.
    fn create(&self) -> iced::Command<Message> {
        let store = self.store.clone();
        let name = self.create_name.clone();
        let game = self.chosen_version();
        // Read here rather than inside the worker, because the build is chosen
        // against the list in the dialog: a worker that read it again would be
        // resolving the chips against a second answer to the same question.
        let loader = self.create_loader;
        let build = self.chosen_build();
        iced::Command::perform(
            crate::store::off_thread(move || {
                store.create_instance(&name, game.as_deref(), loader, build.as_deref())
            }),
            Message::Created,
        )
    }

    /// Ask Mojang which versions exist, off the frame thread.
    ///
    /// The dialog's own request, so it is built where the dialog is: a `Loading`
    /// state was set by [`Shell::open_create`], and this is what turns it into an
    /// answer or into [`crate::store::not_implemented`]'s sentence.
    fn versions_command(&self) -> iced::Command<Message> {
        let store = self.store.clone();
        iced::Command::perform(crate::store::off_thread(move || store.versions()), Message::Versions)
    }

    /// Ask the loader's own service which builds it published for the game
    /// version in force, off the frame thread.
    ///
    /// The question travels back with the answer, because the dialog can move on
    /// while it is out: without it, a Fabric list that arrived after the Quilt
    /// chip was pressed would be drawn under the Quilt chip.
    fn loader_builds_command(&self) -> iced::Command<Message> {
        let store = self.store.clone();
        let loader = self.create_loader;
        let Some(game) = self.chosen_version() else {
            return iced::Command::none();
        };
        let asked = game.clone();
        iced::Command::perform(
            crate::store::off_thread(move || store.loader_builds(loader, &game)),
            move |builds| Message::LoaderBuilds { loader, game: asked.clone(), builds },
        )
    }

    /// The version the picker is on: what the user chose, or Mojang's own latest
    /// release once the list has arrived.
    ///
    /// One reading rather than two, because the heading and the create command
    /// both need it and a dialog that showed one version while creating another
    /// is the failure this exists to prevent.
    fn chosen_version(&self) -> Option<String> {
        self.version_choice
            .clone()
            .or_else(|| self.versions.ready().map(|list| list.latest_release.clone()))
    }

    /// The request the page on screen owes, if it owes one.
    fn opening_command(&mut self) -> iced::Command<Message> {
        match self.screen.opening() {
            Some(pages::Ask::Search(asked)) => self.search(asked),
            // A navigation, a creation and a launch are not *owed*: nothing is
            // waiting for one, and the page that owes nothing says nothing. A
            // launch in particular is a button's doing rather than a page's
            // arrival, and starting a game because a window opened on its page
            // would be a launcher that plays by itself.
            Some(
                pages::Ask::Open(_)
                | pages::Ask::Create
                | pages::Ask::Import
                | pages::Ask::Play(_)
                | pages::Ask::Stop(_),
            ) => iced::Command::none(),
            None => iced::Command::none(),
        }
    }

    /// Run a search and bring the answer back as a page message.
    ///
    /// The work itself happens in [`crate::store::off_thread`], which is the
    /// crossing between a blocking engine and a toolkit that is not: awaiting the
    /// call here, on the thread that draws, would be a dropped frame for every
    /// millisecond the service took. The ceiling the engine holds is what keeps a
    /// user who types quickly from opening a connection per keystroke.
    ///
    /// The round the request was made in travels with the message, so a slow
    /// answer to a question the page has replaced is dropped by the page rather
    /// than drawn (`discover::State::update`).
    fn search(&self, asked: discover::Asked) -> iced::Command<Message> {
        let store = self.store.clone();
        // Cloned rather than borrowed: the worker takes the query, and the answer
        // still needs the round it came with.
        let query = asked.query.clone();
        iced::Command::perform(crate::store::off_thread(move || store.search(&query)), move |result| {
            Message::Screen(pages::Message::search_result(&asked, result))
        })
    }

    // ---- Launching ------------------------------------------------------

    /// The account this launcher is signed in as, or an anonymous one.
    ///
    /// The whole entry travels rather than just the name, for the old shell's
    /// reason: a Microsoft account launches with its tokens, and renewing them is
    /// the worker's first step.
    fn account(&self) -> launch::AccountRef {
        match self.accounts.as_ref().and_then(AccountsStore::selected_account) {
            Some(account) => launch::AccountRef::from_entry(account),
            // No account file, or no account chosen: the offline session the
            // launcher has always been able to play with.
            None => launch::AccountRef::anonymous(),
        }
    }

    /// Sign this launcher's next launch in as `uuid`, and write the file.
    ///
    /// The file is the record rather than this window: the other launcher reads
    /// the same one, so an account chosen here is the account it signs in as too.
    /// A write that fails is said in the panel, because the alternative is a
    /// selection that looks made and is not.
    fn select_account(&mut self, uuid: &str) {
        let Some(accounts) = self.accounts.as_mut() else {
            return;
        };
        match accounts.select(uuid) {
            Ok(()) => self.save_accounts(),
            Err(problem) => self.accounts_note = Some(problem),
        }
    }

    /// Take this account away, and write the file.
    ///
    /// Removing the account a launch would have signed in as leaves no selection,
    /// which is the anonymous session rather than an error: the store clears it and
    /// the card's header falls back to the reference's own "Select account".
    fn remove_account(&mut self, uuid: &str) {
        let Some(accounts) = self.accounts.as_mut() else {
            return;
        };
        if accounts.remove(uuid) {
            self.save_accounts();
        }
    }

    /// Write the accounts file, leaving in the panel whatever a write could not do.
    fn save_accounts(&mut self) {
        let Some(accounts) = self.accounts.as_ref() else {
            return;
        };
        self.accounts_note = accounts.save().err();
    }

    /// Start the instance `id`, and remember the run.
    ///
    /// The run is what the subscription reads, so this is the whole of starting
    /// a game: everything the worker needs was read here, at the moment the user
    /// asked, because a worker has no business going back to a preferences file
    /// to find out what it was told to use.
    ///
    /// A second press while a run is going is ignored rather than queued: the
    /// reference's Play button is the Stop button by then, so the only way to
    /// send this is a stale frame.
    fn play(&mut self, id: String) {
        if self.run.is_some() || id.trim().is_empty() {
            return;
        }
        let Some(home) = &self.home else {
            // A shell with no data root is a test's shell: there is nowhere for an
            // instance to be, so there is nothing to launch.
            self.store.set_launch(store::Launch {
                instance: Some(id),
                state: store::LaunchState::Idle,
                line: Some(store::not_implemented("Launching an instance")),
            });
            return;
        };
        // A child left over from a previous run is dropped rather than killed:
        // the run is over, and a process that is still there is not one this
        // launcher is tracking any more.
        if let Ok(mut slot) = self.child.lock() {
            *slot = None;
        }
        self.runs += 1;
        self.run = Some(ActiveRunData {
            run_id: self.runs,
            instance_id: id.clone(),
            data_root: home.root.clone(),
            account: self.account(),
            defaults: launch::LaunchDefaults::from_prefs(&self.prefs),
        });
        self.store.set_launch(store::Launch {
            // The same sentence the worker's own first line uses, because it is
            // the same fact: the run is being prepared and nothing has happened
            // yet.
            line: Some(format!("preparing '{id}'")),
            instance: Some(id),
            state: store::LaunchState::Starting,
        });
    }

    /// Stop the instance that is running.
    ///
    /// The kill is on the child the worker put in the slot, which is the only
    /// handle to the game this process has. What comes back is the worker's own
    /// `Done`: killing a process is not the same as reaping it, and the note the
    /// run ends with is the worker's to write.
    fn stop(&mut self, id: &str) {
        if self.run.as_ref().map(|run| run.instance_id.as_str()) != Some(id) {
            return;
        }
        // Nothing to kill while the launcher is still preparing: the child slot
        // is empty until the worker has resolved the version, fetched what was
        // missing and started the process. Setting *Stopping* here would be a
        // word with nothing behind it -- the run would go on to start the game
        // and then report itself running under a chip that says it is being
        // stopped -- so this is the one state where a stop is refused rather than
        // performed. It is refused by *not being drawn*, too: the action bar's own
        // control and the instance page's are both absent while a run prepares.
        if self.store.launch_state(id) != store::LaunchState::Running {
            return;
        }
        self.store.set_launch(store::Launch {
            instance: Some(id.to_string()),
            state: store::LaunchState::Stopping,
            line: Some(Key::InstanceActionStopping.message().to_string()),
        });
        if let Ok(mut slot) = self.child.lock() {
            if let Some(child) = slot.as_mut() {
                let _ = child.kill();
            }
        }
    }

    /// What a launch reported, applied to the run it belongs to.
    ///
    /// Every arm checks the run id first: a done for a run this shell has already
    /// replaced must not clear the one that is going, and a line from the run
    /// before must not appear in the header of the run that replaced it.
    fn launched(&mut self, event: launch::LaunchEvent) {
        let current = self.run.as_ref().map(|run| run.run_id);
        match event {
            launch::LaunchEvent::Log { run_id, lines } => {
                if current != Some(run_id) {
                    return;
                }
                if let Some(line) = lines.last() {
                    self.say(line.clone());
                }
            }
            launch::LaunchEvent::Progress { run_id, progress } => {
                if current != Some(run_id) {
                    return;
                }
                // Kept as well as said: the line is the instance's own, and the
                // numbers are what the action bar's chip is drawn from.
                self.progress = Some(progress.clone());
                self.say(progress.status_line());
            }
            launch::LaunchEvent::Started { run_id } => {
                if current != Some(run_id) {
                    return;
                }
                // What changes here is the control and not the line: the fact is
                // about the game's window being up, and the run's own last line
                // is still the last thing the launcher said.
                //
                // The level goes with it: fetching is over, and a bar left at
                // the last phase's fraction would be a download that never
                // finished. The panel closes for the same reason.
                self.progress = None;
                self.downloads = false;
                self.set_run_state(store::LaunchState::Running);
            }
            launch::LaunchEvent::Done { run_id, note } => {
                if current != Some(run_id) {
                    return;
                }
                let instance = self.run.take().map(|run| run.instance_id);
                if let Ok(mut slot) = self.child.lock() {
                    *slot = None;
                }
                self.progress = None;
                self.downloads = false;
                self.store.set_launch(store::Launch {
                    instance,
                    state: store::LaunchState::Idle,
                    line: Some(note),
                });
                // The run wrote the play time and the last-played stamp into the
                // instance's own files, so the library the pages are drawn from is
                // stale until it is read again.
                self.store.reload();
            }
            launch::LaunchEvent::Tokens(tokens) => self.renewed(tokens),
        }
    }

    /// Record what the launch is doing, for the pages to draw.
    fn say(&mut self, line: String) {
        let Some(run) = &self.run else {
            return;
        };
        let id = run.instance_id.clone();
        let state = self.store.launch_state(&id);
        self.store.set_launch(store::Launch { instance: Some(id), state, line: Some(line) });
    }

    /// Put the running instance in another state, keeping the line it is on.
    ///
    /// The state and the line are two facts rather than one: the game coming up
    /// changes what the button is, and the line is still the last thing the
    /// launcher said.
    fn set_run_state(&mut self, state: store::LaunchState) {
        let Some(run) = &self.run else {
            return;
        };
        let id = run.instance_id.clone();
        let line = self.store.launch_line(&id).map(str::to_string);
        self.store.set_launch(store::Launch { instance: Some(id), state, line });
    }

    /// Write a session a launch renewed back to the account store.
    ///
    /// A renewal happens inside the worker because the launch cannot wait for the
    /// window to notice; storing it is the window's, because the file is the
    /// window's. A write that fails is left for the next launch to renew again
    /// rather than drawn in front of the user: it is the same account, still
    /// signed in, and the failure is about a file.
    fn renewed(&mut self, tokens: launch::RefreshedTokens) {
        let Some(accounts) = self.accounts.as_mut() else {
            return;
        };
        if accounts
            .update_tokens(
                &tokens.uuid,
                &tokens.access_token,
                tokens.refresh_token.as_deref(),
                tokens.expires_at_ms,
            )
            .is_ok()
        {
            let _ = accounts.save();
        }
    }

    /// The launch subscription: nothing while nothing is running, and the run
    /// itself while there is one.
    ///
    /// The worker is blocking and lives on a thread of its own; what crosses back
    /// is a channel of [`launch::LaunchEvent`]s, pumped into this shell's own
    /// messages. A full window is waited for rather than dropped -- a line the
    /// launcher wrote is not a level, and a shell that never hears a run ended
    /// would go on drawing Stop for a game that is gone.
    fn launching(&self) -> Subscription<Message> {
        let Some(run) = self.run.clone() else {
            return Subscription::none();
        };
        let slot = self.child.clone();
        let id = run.run_id;
        iced::subscription::channel(id, 128, move |sender| async move {
            let params = launch::LaunchParams {
                data_root: run.data_root.clone(),
                instance_id: run.instance_id.clone(),
                account: run.account.clone(),
                run_id: run.run_id,
                defaults: run.defaults.clone(),
            };
            let (events, mut stream) =
                futures::channel::mpsc::channel::<launch::LaunchEvent>(128);
            let _ = std::thread::spawn(move || {
                launch::run_launch_worker(params, slot, events);
            });
            let mut sender = sender;
            while let Some(event) = futures::StreamExt::next(&mut stream).await {
                let mut pending = Some(Message::Launched(event));
                while let Some(message) = pending.take() {
                    match sender.try_send(message) {
                        // Delivered, and the next event is fetched.
                        Ok(()) => {}
                        // Full: the window is a frame behind. A fact is not a
                        // frame, so the send waits for room rather than dropping
                        // it.
                        Err(error) if error.is_full() => {
                            pending = Some(error.into_inner());
                            let had_room = futures::future::poll_fn(|cx| sender.poll_ready(cx)).await;
                            if had_room.is_err() {
                                break;
                            }
                        }
                        // The window is gone: there is nobody left to tell.
                        Err(_) => break,
                    }
                }
            }
            loop {
                futures::future::pending::<()>().await;
            }
        })
    }

    // ---- Drawing --------------------------------------------------------

    fn render(&self) -> Element<'_, Message> {
        // Keep the window's hit test pointed at the maximize control: the
        // rectangle is derived from this shell's own constants, so re-publishing
        // it is a store behind a lock rather than a measurement to keep in step.
        crate::native::set_caption_target(caption_target());
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
        let mut window = column![].width(Length::Fill).height(Length::Fill).push(
            container(self.head()).width(Length::Fill).height(Length::Fixed(BAR)).style(chrome),
        );
        // The download manager's panel, between the bar it was opened from and
        // the page it is about: see [`Shell::download_panel`].
        if self.downloads {
            if let Some(progress) = &self.progress {
                window = window.push(self.download_panel(progress));
            }
        }
        window
            .push(row![self.rail(), self.pane()].height(Length::Fill))
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
            // The reference's own order on the right of the status bar: the
            // action bar, then the panel toggle, then the window controls. The
            // action bar is what makes a run watchable from any page -- the
            // reference keeps it here rather than on the instance page for exactly
            // that reason, and this shell used to draw the run only in the header
            // of the instance it belonged to.
            .push(Space::with_width(Length::Fill))
            .push(self.action_bar())
            .push(Space::with_width(8.0))
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

    /// The action bar: what this launcher is running, and how far the run is
    /// through whatever it is fetching.
    ///
    /// `AppActionBar.vue`'s right-hand cluster, in its own order: the download
    /// manager's chip first, then the one chip that says what is running. Two
    /// pieces of it are deliberately not built here, and both are absent features
    /// rather than missing pixels: the offline banner needs an online/offline
    /// source this launcher has no opinion about yet, and the update button
    /// belongs to a self-updater it does not have.
    fn action_bar(&self) -> Element<'_, Message> {
        let mut bar = row![].align_items(Alignment::Center).spacing(8.0);
        if let Some(progress) = &self.progress {
            bar = bar.push(self.download_chip(progress));
        }
        bar.push(self.running_chip()).into()
    }

    /// The download manager's chip: the glyph, the phase's own label and either
    /// its percentage or the fact that it has no length yet.
    ///
    /// The bar under the label is [`crate::ui::progress`] over the phase's own
    /// fraction, so the number and the fill cannot disagree: both are read from
    /// the same `Progress`.
    fn download_chip(&self, progress: &install::Progress) -> Element<'_, Message> {
        let theme = self.theme;
        let (factor, _) = crate::ui::interaction(DOWNLOAD_CHIP);
        // The reference's `bg-brand-highlight` while its panel is open, which is
        // how a chip that toggles something says that it is open.
        let background = self
            .downloads
            .then(|| theme_gen::ink(theme, Ink::ColorBrandHighlight));
        let ink = crate::theme::brightness(theme_gen::ink(theme, INK_CONTRAST), factor);
        let mut body = row![].align_items(Alignment::Center).spacing(8.0);
        body = body.push(icon::icon(Glyph::Download, 16.0, ink));
        body = body.push(
            text(progress.label.clone())
                .size(13.0)
                .font(medium())
                .style(iced::theme::Text::Color(ink)),
        );
        body = body.push(
            text(if progress.is_indeterminate() {
                // No fraction to show, and a percentage would have to be
                // invented: the reference's own arm says "working" with a
                // spinner rather than with a number.
                "…".to_string()
            } else {
                format!("{}%", progress.percent())
            })
            .size(13.0)
            .font(semibold())
            .style(iced::theme::Text::Color(ink)),
        );
        body = body.push(
            container(crate::ui::progress(theme, progress.fraction()))
                .width(Length::Fixed(DOWNLOAD_BAR))
                .center_y(),
        );
        let chip = container(body)
            .height(Length::Fixed(BAR_CHIP))
            .padding(Padding { top: 0.0, bottom: 0.0, left: 12.0, right: 12.0 })
            .center_y()
            .style(move |_theme: &Theme| container::Appearance {
                background: background.map(|background| {
                    Background::Color(crate::theme::brightness(background, factor))
                }),
                border: Border {
                    color: theme_gen::ink(theme, Ink::Surface5),
                    width: 1.0,
                    radius: 12.0.into(),
                },
                ..container::Appearance::default()
            });
        mouse_area(chip)
            .interaction(Interaction::Pointer)
            .on_enter(Message::hover(DOWNLOAD_CHIP, true))
            .on_exit(Message::hover(DOWNLOAD_CHIP, false))
            .on_press(Message::ToggleDownloads)
            .into()
    }

    /// The chip that says what this launcher is running, with the two controls
    /// the reference gives it: stop, and go to the instance's logs.
    ///
    /// Nothing running is a chip of its own rather than no chip: the reference
    /// draws *No instances running* beside a grey dot so that the surface keeps
    /// its place and its shape, and a bar that came and went as processes started
    /// and stopped would move everything beside it.
    fn running_chip(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let launch = self.store.launch();
        let running = launch.state != store::LaunchState::Idle;
        let mut body = row![].align_items(Alignment::Center).spacing(8.0);
        if running {
            let id = launch.instance.clone().unwrap_or_default();
            // A dot, the instance's name, and the state it is in -- which the
            // reference leaves to the instance's own header and this shell can
            // afford to say twice: a run being *stopped* looks exactly like a run
            // being started from the outside.
            body = body.push(self.indicator(Ink::Green));
            body = body.push(
                mouse_area(
                    text(instance_name(&self.store, &id))
                        .size(13.0)
                        .font(semibold())
                        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
                )
                .interaction(Interaction::Pointer)
                .on_press(Message::Go(format!("/instance/{id}"))),
            );
            if let Some(state) = state_label(launch.state) {
                body = body.push(
                    text(state)
                        .size(13.0)
                        .font(medium())
                        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
                );
            }
            // The stop control exists when there is something to stop, which is
            // [`store::LaunchState::Running`] rather than "not idle": the kill
            // goes to the child the worker put in the slot, and while a run is
            // *preparing* there is no such child -- a control that drew itself
            // there would be a button whose press changed a word and nothing
            // else. The reference's own condition is the same one: it draws this
            // for a process it can see.
            if launch.state == store::LaunchState::Running {
                body = body.push(crate::ui::icon_button_kind(
                    theme,
                    BAR_STOP,
                    Glyph::StopCircle,
                    18.0,
                    crate::ui::Kind::Danger,
                    Message::StopRun,
                ));
            }
            body = body.push(crate::ui::icon_button(
                theme,
                BAR_LOGS,
                Glyph::TerminalSquare,
                18.0,
                Message::Go(format!("/instance/{id}/logs")),
            ));
        } else {
            body = body.push(self.indicator(INK_SECONDARY));
            body = body.push(
                text(Key::AppActionBarNoInstancesRunning.message())
                    .size(13.0)
                    .font(medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
            );
        }
        container(body)
            .height(Length::Fixed(BAR_CHIP))
            .padding(Padding { top: 0.0, bottom: 0.0, left: 12.0, right: 12.0 })
            .center_y()
            .style(move |_theme: &Theme| container::Appearance {
                border: Border {
                    color: theme_gen::ink(theme, Ink::Surface5),
                    width: 1.0,
                    radius: 12.0.into(),
                },
                ..container::Appearance::default()
            })
            .into()
    }

    /// The reference's `OnlineIndicatorIcon`: a dot in the colour of the fact it
    /// carries.
    fn indicator(&self, ink: Ink) -> Element<'_, Message> {
        let theme = self.theme;
        container(Space::with_width(INDICATOR))
            .width(Length::Fixed(INDICATOR))
            .height(Length::Fixed(INDICATOR))
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(theme_gen::ink(theme, ink))),
                border: Border { radius: 999.0.into(), ..Border::default() },
                ..container::Appearance::default()
            })
            .into()
    }

    /// The download manager's panel, drawn under the head while it is open.
    ///
    /// The reference floats this in a teleported popup; iced 0.12 has no
    /// z-order and this is the same wall the version picker hit, so it is drawn
    /// in the layout: a row under the bar, which is where a panel the user opened
    /// from the bar belongs. What it holds is the one job this launcher can have
    /// -- the run's own phase -- with the three numbers the reference shows:
    /// what it is doing, how many of them are done, and how many bytes that is.
    fn download_panel(&self, progress: &install::Progress) -> Element<'_, Message> {
        let theme = self.theme;
        let launch = self.store.launch();
        let title = launch
            .instance
            .as_deref()
            .map(|id| instance_name(&self.store, id))
            .unwrap_or_else(|| Key::AppActionBarDownloads.message().to_string());
        let body = column![]
            .spacing(6.0)
            .push(
                row![]
                    .align_items(Alignment::Center)
                    .spacing(8.0)
                    .push(
                        text(title)
                            .size(14.0)
                            .font(semibold())
                            .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
                    )
                    .push(Space::with_width(Length::Fill))
                    .push(crate::ui::icon_button(
                        theme,
                        BAR_PANEL_CLOSE,
                        Glyph::X,
                        16.0,
                        Message::ToggleDownloads,
                    )),
            )
            .push(
                text(progress_line(progress))
                    .size(13.0)
                    .font(medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
            )
            // The fraction again, as a bar: the same `Progress` the chip was
            // drawn from, so a panel and a chip cannot disagree.
            .push(crate::ui::progress(theme, progress.fraction()));
        container(body)
            .width(Length::Fill)
            .padding(Padding { top: 10.0, bottom: 10.0, left: RAIL_PAD, right: CONTROLS_WIDTH })
            .style(move |_theme: &Theme| container::Appearance {
                background: Some(Background::Color(theme_gen::ink(theme, Ink::RaisedBg))),
                border: Border {
                    color: theme_gen::ink(theme, Ink::Surface5),
                    width: 1.0,
                    radius: 0.0.into(),
                },
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

    /// The right panel: the reference's own column, with its first section in it.
    ///
    /// `App.vue`'s `app-sidebar`: a `--right-bar-width` column under the wash,
    /// a hairline down its page edge (`border-l border-[--brand-gradient-border]`),
    /// and one scroll region inside it (`app-sidebar-scrollable`) that the
    /// sections stack in. The sections are the onboarding checklist, this card
    /// ("Playing as", `app.sidebar.playing-as`), the friends list, the fundraiser
    /// banner and the news feed; the second of those is the only one this launcher
    /// can draw anything in, and the others are absent rather than drawn empty.
    fn panel(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let mut sections = column![].width(Length::Fill).push(self.playing_as());
        if let Some(note) = &self.accounts_note {
            sections = sections.push(
                container(self.accounts_note_block(note))
                    .width(Length::Fill)
                    .padding(PANEL_SECTION_PAD),
            );
        }
        // `border-l` over the wash: iced paints a `Border` on all four sides, so
        // the panel's own edge is a one-pixel column rather than a border width.
        container(
            row![hairline(theme, true), scrollable(sections).width(Length::Fill).height(Length::Fill)]
                .height(Length::Fill),
        )
        .width(Length::Fixed(PANEL))
        .height(Length::Fill)
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(wash(theme)),
            ..container::Appearance::default()
        })
        .into()
    }

    /// The panel's first section: what a launch would sign in as.
    ///
    /// `App.vue` draws it `p-4` under a `border-b`, and only when
    /// `hasLoggedIntoMinecraft`. That flag is the onboarding checklist's own -- the
    /// checklist is what this launcher has not built -- so the section is drawn
    /// always here, one step early. The alternative is a panel that stays the wash
    /// until an unbuilt flag is set, which is the gap this stage closes; and the
    /// empty card below is the reference's own picture of a launcher with no
    /// account, so what is drawn early is the reference's shape either way.
    /// `GATES.md` G83 records the difference.
    fn playing_as(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let section = column![]
            .width(Length::Fill)
            .push(
                container(
                    column![]
                        .width(Length::Fill)
                        .push(
                            text(Key::AppSidebarPlayingAs.message())
                                .size(PANEL_HEADING)
                                .font(medium())
                                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
                        )
                        // `mt-2` is the card's own, and the card carries it.
                        .push(container(self.accounts_card()).padding(Padding {
                            top: CARD_TOP,
                            bottom: 0.0,
                            left: 0.0,
                            right: 0.0,
                        })),
                )
                .width(Length::Fill)
                .padding(PANEL_SECTION_PAD),
            )
            .push(hairline(theme, false));
        section.into()
    }

    /// What one of the card's controls could not do, with the control that reads
    /// it -- [`crate::page::notice`]'s shape, drawn where the press was made.
    fn accounts_note_block(&self, note: &str) -> Element<'_, Message> {
        let theme = self.theme;
        row![]
            .spacing(ROW_GAP)
            .align_items(Alignment::Start)
            .push(crate::ui::admonition(
                theme,
                crate::ui::Severity::Info,
                Key::MinecraftAccountSignIn.message(),
                note,
            ))
            .push(Space::with_width(Length::Fill))
            .push(crate::ui::icon_button(
                theme,
                ACCOUNTS_NOTE_DISMISS,
                Glyph::X,
                16.0,
                Message::DismissAccountsNote,
            ))
            .into()
    }

    /// `AccountsCard.vue`: the launcher's accounts, as the panel shows them.
    ///
    /// Two states, which are the reference's own two branches: with no accounts,
    /// a sentence and the sign-in button; with accounts, an accordion whose header
    /// names the account a launch would sign in as and whose body lists them.
    ///
    /// The card's player heads are not drawn. The reference puts a 36px head in
    /// the header and a 24px one on every row, from the skin service or from its
    /// own Steve asset for an offline account, and this launcher has no head
    /// renderer yet -- the Skins page is a placeholder for the same reason -- so a
    /// row is its radio mark and its name. `GATES.md` G83 records it.
    fn accounts_card(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let accounts: &[AccountEntry] = match &self.accounts {
            Some(store) => store.list(),
            None => &[],
        };
        if accounts.is_empty() {
            let body = column![]
                .width(Length::Fill)
                .spacing(CARD_STACK_GAP)
                .push(
                    text(Key::MinecraftAccountNotSignedIn.message())
                        .size(14.0)
                        .font(medium())
                        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
                )
                .push(crate::ui::button_with_icon(
                    theme,
                    ACCOUNTS_SIGN_IN,
                    Glyph::LogIn,
                    Key::MinecraftAccountSignIn,
                    crate::ui::Kind::Colored,
                    Length::Shrink,
                    Some(Message::SignIn),
                ));
            return card_frame(theme, CARD_FRAME_PAD, body);
        }
        let selected = self.accounts.as_ref().and_then(AccountsStore::selected_uuid);
        let title = card_title(accounts, selected);
        let (factor, _) = crate::ui::interaction(ACCOUNTS_HEADER);
        let header = container(
            row![]
                .width(Length::Fill)
                .spacing(CARD_ROW_GAP)
                .align_items(Alignment::Center)
                .push(
                    column![]
                        .width(Length::Fill)
                        .push(
                            text(title)
                                .size(14.0)
                                .font(medium())
                                .style(iced::theme::Text::Color(crate::theme::brightness(
                                    theme_gen::ink(theme, INK_CONTRAST),
                                    factor,
                                ))),
                        )
                        .push(
                            text(Key::MinecraftAccountLabel.message())
                                .size(CARD_LABEL)
                                .font(medium())
                                .style(iced::theme::Text::Color(crate::theme::brightness(
                                    theme_gen::ink(theme, INK_SECONDARY),
                                    factor,
                                ))),
                        ),
                )
                // The Accordion rotates its `DropdownIcon` a half turn when it
                // opens (`class="rotate-180"`). iced cannot rotate a glyph, and
                // the two chevrons are the same picture drawn twice.
                .push(icon::icon(
                    if self.accounts_open { Glyph::ChevronUp } else { Glyph::ChevronDown },
                    CARD_MARK,
                    crate::theme::brightness(theme_gen::ink(theme, INK_CONTRAST), factor),
                )),
        )
        .width(Length::Fill)
        .padding(Padding {
            top: CARD_HEAD_PAD,
            bottom: CARD_HEAD_PAD,
            left: CARD_HEAD_SIDE,
            right: CARD_HEAD_SIDE,
        });
        let header = mouse_area(header)
            .interaction(Interaction::Pointer)
            .on_enter(card_crossing(ACCOUNTS_HEADER, true))
            .on_exit(card_crossing(ACCOUNTS_HEADER, false))
            .on_press(Message::ToggleAccounts);
        let mut card = column![].width(Length::Fill).push(header);
        if self.accounts_open {
            card = card.push(self.accounts_body(accounts, selected));
        }
        card_frame(theme, 0.0, card)
    }

    /// The card's body, open: one row per account, then the add-account button.
    ///
    /// `AccountsCard.vue`'s own arrangement -- a `border-t border-surface-5`
    /// hairline under the header, `pt-1 pb-2` on the body, and the reference's
    /// `flex flex-col gap-2 px-2 pt-2` around the button at its foot.
    fn accounts_body<'a>(
        &'a self,
        accounts: &'a [AccountEntry],
        selected: Option<&'a str>,
    ) -> Element<'a, Message> {
        let theme = self.theme;
        let mut rows = column![]
            .width(Length::Fill)
            .padding(Padding { top: 4.0, bottom: 8.0, left: 0.0, right: 0.0 })
            .push(hairline(theme, false));
        for account in accounts {
            let chosen = selected == Some(account.uuid.as_str());
            let key = crate::ui::scoped("shell:accounts:row", &account.uuid);
            let (factor, _) = crate::ui::interaction(key);
            let name = row![]
                .width(Length::Fill)
                .spacing(CARD_ROW_GAP)
                .align_items(Alignment::Center)
                .push(icon::icon(
                    if chosen { Glyph::RadioButtonChecked } else { Glyph::RadioButton },
                    CARD_MARK,
                    crate::theme::brightness(
                        theme_gen::ink(theme, if chosen { Ink::Brand } else { INK_SECONDARY }),
                        factor,
                    ),
                ))
                .push(
                    text(account.username.clone())
                        .size(14.0)
                        .font(if chosen { semibold() } else { medium() })
                        .style(iced::theme::Text::Color(crate::theme::brightness(
                            theme_gen::ink(theme, if chosen { INK_CONTRAST } else { INK_DEFAULT }),
                            factor,
                        ))),
                );
            let name = mouse_area(
                container(name).width(Length::Fill).padding(CARD_ROW_PAD),
            )
            .interaction(Interaction::Pointer)
            .on_enter(card_crossing(key, true))
            .on_exit(card_crossing(key, false))
            .on_press(Message::SelectAccount(account.uuid.clone()));
            rows = rows.push(
                row![]
                    .width(Length::Fill)
                    .spacing(4.0)
                    .align_items(Alignment::Center)
                    .push(name)
                    .push(crate::ui::icon_button_kind(
                        theme,
                        crate::ui::scoped("shell:accounts:remove", &account.uuid),
                        Glyph::Trash,
                        16.0,
                        crate::ui::Kind::Danger,
                        Message::RemoveAccount(account.uuid.clone()),
                    )),
            );
        }
        rows = rows.push(
            container(crate::ui::button_with_icon(
                theme,
                ACCOUNTS_ADD,
                Glyph::Plus,
                Key::MinecraftAccountAddAccount,
                crate::ui::Kind::Standard,
                Length::Fill,
                Some(Message::SignIn),
            ))
            .width(Length::Fill)
            .padding(CARD_ROW_PAD),
        );
        rows.into()
    }

    /// The themes Settings offers, by the reference's own rule.
    ///
    /// `AppearanceSettings.vue` filters its list on `appSettings.devMode` and on
    /// the theme in force: retro is behind dev mode until it is the theme already
    /// chosen. `ColorTheme::options` owns that rule, and this launcher has no dev
    /// mode to hand it, so what the pane offers is the rule read once, with
    /// `false`.
    fn theme_options(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let current = self.prefs.theme();
        let mut grid = row![].spacing(ROW_GAP);
        for option in ColorTheme::options(false, current) {
            let key = crate::ui::scoped("settings:theme", option.id());
            let kind = if option == current { crate::ui::Kind::Colored } else { crate::ui::Kind::Standard };
            grid = grid.push(crate::ui::button(
                theme,
                key,
                option.label_key(),
                kind,
                Message::ColorTheme(option),
            ));
        }
        grid.into()
    }

    /// The dialog's frame: the same width, padding, surface and close button
    /// whichever modal it holds.
    fn dialog<'a>(
        &'a self,
        title: Key,
        body: Element<'a, Message>,
    ) -> Element<'a, Message> {
        let theme = self.theme;
        container(
            column![]
                .spacing(12.0)
                .push(
                    row![]
                        .align_items(Alignment::Center)
                        .push(
                            text(title.message())
                                .size(20.0)
                                .font(heading())
                                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
                        )
                        .push(Space::with_width(Length::Fill))
                        .push(self.history_button(Glyph::X, true, Message::CloseModal)),
                )
                .push(body),
        )
        .width(Length::Fixed(560.0))
        .padding(24.0)
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(theme_gen::ink(theme, Ink::RaisedBg))),
            border: Border { radius: 16.0.into(), ..Border::default() },
            ..container::Appearance::default()
        })
        .into()
    }

    /// Settings: the appearance pane, which is the themes, and whatever reading
    /// this launcher's own files could not do.
    fn settings_dialog(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let mut body = column![]
            .spacing(12.0)
            .push(
                text(Key::SettingsDisplayThemeDescription.message())
                    .size(14.0)
                    .font(medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
            )
            .push(self.theme_options());
        if let Some(warning) = &self.accounts_warning {
            // A warning rather than a failure: the launcher works, and what the
            // reader needs to know is that it does not know which account was
            // signed in.
            body = body.push(crate::ui::admonition(
                theme,
                crate::ui::Severity::Warning,
                "accounts",
                warning,
            ));
        }
        self.dialog(Key::SettingsAppearanceTitle, body.into())
    }

    /// The creation dialog: a name, the modloader, the version the instance is
    /// for, which build of that loader, and the button that makes it.
    ///
    /// The step is the reference's own (`CustomSetupStage.vue`), field for field:
    /// the name, its loader chips, the searchable game-version combobox with its
    /// snapshots footer, and then -- for a loader that has builds at all -- the
    /// loader-version chips over the list the *Other* one opens. The combobox is
    /// the picker: a searchable field over Mojang's version list, and under it the
    /// dropdown the field belongs to, which is the options and then the footer
    /// that adds the snapshots and old builds to them. What the reference
    /// *teleports* to the window's edge is drawn here in place, which is the same
    /// wall the modal layer hit: iced 0.12 composites a tree in order and has no
    /// z-order.
    fn create_dialog(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let usable = self.create_usable();
        let mut body = column![]
            .spacing(12.0)
            .push(
                text(Key::CreationFlowModalCustomSetupNameLabel.message())
                    .size(14.0)
                    .font(semibold())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
            )
            .push(
                text_input(
                    Key::CreationFlowModalCustomSetupNamePlaceholder.message(),
                    &self.create_name,
                )
                .on_input(Message::CreateName)
                .padding(Padding { top: 10.0, bottom: 10.0, left: 12.0, right: 12.0 })
                .size(14.0)
                .font(medium())
                .style(iced::theme::TextInput::Custom(Box::new(crate::ui::Field::bordered(theme)))),
            )
            // The reference's own order on this step: the loader first, then the
            // game version it has to have been published for, then which build of
            // it.
            .push(self.loader_chips())
            .push(self.version_heading())
            .push(crate::ui::search(
                theme,
                Key::CreationFlowModalCustomSetupGameVersionSearchPlaceholder.message(),
                &self.version_query,
                Message::VersionQuery,
            ))
            .push(self.version_picker());
        // A loader with no build list is a vanilla instance, and the dialog says
        // so rather than drawing a row of chips that would change nothing.
        if self.create_loader.loads_mods() {
            body = body.push(self.loader_version());
        }
        if let Some(reason) = &self.create_error {
            body = body.push(
                text(reason.clone())
                    .size(14.0)
                    .font(medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, Ink::Red))),
            );
        }
        body = body.push(
            row![]
                .align_items(Alignment::Center)
                .push(Space::with_width(Length::Fill))
                .push(crate::ui::button_or(
                    theme,
                    CREATE_BUTTON,
                    Key::CreationFlowButtonCreateInstance,
                    crate::ui::Kind::Colored,
                    usable.then_some(Message::Create),
                )),
        );
        self.dialog(Key::CreationFlowTitleCreateInstance, body.into())
    }

    /// One field's heading: the reference's own label, with the value in force
    /// beside it.
    ///
    /// The reference shows the same value *in* its trigger -- a searchable
    /// combobox mirrors the selection over its own search field -- and a toolkit
    /// with no overlay has to say it somewhere, which is here rather than
    /// nowhere: a field showing a search term and a picker with a highlighted row
    /// are not enough on their own when the list is scrolled past the choice.
    ///
    /// It matters most on the loader-version row, where the chips name a *rule*
    /// rather than a version: without this, the build the rule comes to is one
    /// the user cannot see until the instance exists.
    fn field_heading(&self, label: Key, value: Option<String>) -> Element<'_, Message> {
        let theme = self.theme;
        let mut heading = row![].align_items(Alignment::Center).push(
            text(label.message())
                .size(14.0)
                .font(semibold())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
        );
        if let Some(value) = value {
            heading = heading.push(Space::with_width(Length::Fill)).push(
                text(value)
                    .size(14.0)
                    .font(medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
            );
        }
        heading.into()
    }

    /// The version picker's own heading.
    fn version_heading(&self) -> Element<'_, Message> {
        self.field_heading(Key::LabelGameVersion, self.chosen_version())
    }

    /// The versions the picker's list is drawn from: Mojang's own order, with the
    /// search and the snapshots footer applied.
    ///
    /// A `contains` rather than a prefix match, because the part of a version a
    /// user remembers is as often its end as its beginning: `1.20` finds `1.20.1`,
    /// and `w02a` finds `25w02a`.
    fn version_matches(&self) -> Vec<&store::GameVersion> {
        let Some(list) = self.versions.ready() else {
            return Vec::new();
        };
        let needle = self.version_query.trim().to_ascii_lowercase();
        list.versions
            .iter()
            .filter(|version| self.version_snapshots || version.release)
            .filter(|version| {
                needle.is_empty() || version.id.to_ascii_lowercase().contains(&needle)
            })
            .collect()
    }

    /// The picker's list and its footer: the reference's dropdown body, drawn
    /// under the field it belongs to.
    ///
    /// Every state the request can be in is drawn as a sentence rather than as an
    /// empty list, which is the whole reason the store answers with a reason: a
    /// picker with no options because the request failed looks exactly like a
    /// picker with no options because there are none.
    fn version_picker(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let body: Element<'_, Message> = match &self.versions {
            // Asked, not answered. The reference shows its own loading label in
            // place of the options while that is true.
            Load::Idle | Load::Loading => {
                self.list_note(Key::LabelLoading.message(), INK_SECONDARY)
            }
            // A failure is a sentence and not an empty list -- and it is the
            // reader's own machine that will be fine, so it is not drawn as an
            // error either.
            Load::Failed(reason) => self.list_note(reason, Ink::Red),
            Load::Empty => self.list_note(
                Key::CreationFlowModalCustomSetupOptionsNoVersionsAvailable.message(),
                INK_SECONDARY,
            ),
            Load::Ready(_) => {
                let matches = self.version_matches();
                if matches.is_empty() {
                    // The reference's own arm: a search that found nothing gets
                    // the same sentence as a list that has nothing in it.
                    self.list_note(
                        Key::CreationFlowModalCustomSetupOptionsNoVersionsAvailable.message(),
                        INK_SECONDARY,
                    )
                } else {
                    let rows: Vec<Element<'_, Message>> = matches
                        .into_iter()
                        .map(|version| self.version_row(version))
                        .collect();
                    // The list scrolls past 300px, which is the reference's own
                    // bound on its options -- and it is the options rather than
                    // the dropdown, so the footer stays outside it.
                    container(scrollable(column(rows).width(Length::Fill)))
                        .width(Length::Fill)
                        .max_height(VERSION_LIST_HEIGHT)
                        .into()
                }
            }
        };
        let (key, label, glyph) = if self.version_snapshots {
            (VERSION_HIDE_SNAPSHOTS, Key::ButtonHideSnapshots, Glyph::EyeOff)
        } else {
            (VERSION_SHOW_ALL, Key::ButtonShowAllVersions, Glyph::Eye)
        };
        container(column![body, self.version_toggle(key, label, glyph)])
            .width(Length::Fill)
            .style(move |_theme: &Theme| container::Appearance {
                // The reference's teleported dropdown: `bg-surface-4`, a
                // `--surface-5` hairline and `rounded-[14px]`.
                background: Some(Background::Color(theme_gen::ink(theme, Ink::Surface4))),
                border: Border {
                    color: theme_gen::ink(theme, Ink::Surface5),
                    width: 1.0,
                    radius: 14.0.into(),
                },
                ..container::Appearance::default()
            })
            .into()
    }

    /// The creation dialog's modloader row: the reference's five chips, with
    /// Vanilla offered first.
    ///
    /// The label is the reference's own *Modloader* for an instance flow, and the
    /// chips are `LoaderKind::all()` -- which is also what the store keys its
    /// loader requests by, so a chip and the request it makes cannot drift.
    fn loader_chips(&self) -> Element<'_, Message> {
        let labels: Vec<(String, bool)> = crate::catalog::LoaderKind::all()
            .iter()
            .map(|loader| (loader.label().to_string(), self.create_loader == *loader))
            .collect();
        let keys: Vec<&'static str> = crate::catalog::LoaderKind::all()
            .iter()
            .map(|loader| loader_chip_key(*loader))
            .collect();
        let chips = crate::ui::chips(self.theme, &keys, &labels, |index| {
            Some(Message::LoaderChoice(crate::catalog::LoaderKind::all()[index]))
        });
        column![]
            .spacing(8.0)
            .push(self.field_heading(Key::CreationFlowModalCustomSetupLoaderLabel, None))
            .push(chips)
            .into()
    }

    /// The creation dialog's loader-version row: which *kind* of build to
    /// install, and -- under *Other* -- the builds to choose between.
    ///
    /// Only drawn for a loader that loads mods: vanilla has no builds, and the
    /// reference's own row is inside a `v-if` on the same condition.
    fn loader_version(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let labels: Vec<(String, bool)> = BuildChoice::ALL
            .iter()
            .map(|choice| (choice.label().message().to_string(), self.build_choice == *choice))
            .collect();
        let keys: Vec<&'static str> = BuildChoice::ALL.iter().map(|choice| choice.key()).collect();
        let stable = self.stable_offered();
        let chips = crate::ui::chips(self.theme, &keys, &labels, |index| {
            let choice = BuildChoice::ALL[index];
            // The reference's `disabledItems`: a loader that published nothing
            // stable for this game version leaves the chip where it is and dim.
            (stable || choice != BuildChoice::Stable).then_some(Message::BuildChoice(choice))
        });
        let mut body = column![]
            .spacing(8.0)
            .push(self.field_heading(
                Key::CreationFlowModalCustomSetupLoaderVersionLabel,
                self.chosen_build(),
            ))
            .push(chips);
        match &self.loader_builds {
            Load::Ready(_) if self.build_choice == BuildChoice::Other => {
                // The reference's own search field, over its own dropdown: the
                // build a user is after is usually one they were told the
                // number of.
                body = body
                    .push(crate::ui::search(
                        theme,
                        Key::CreationFlowModalCustomSetupLoaderVersionSearchPlaceholder.message(),
                        &self.build_query,
                        Message::BuildQuery,
                    ))
                    .push(self.build_picker());
            }
            // Under *Stable* and *Latest* there is no list to draw, but the
            // three other states of the request still have to be said: the chips
            // name a rule, and *loading*, *it failed* and *this loader published
            // nothing for this game version* are what happened to it. Without
            // this the row would be a heading, three chips and a dark button.
            Load::Ready(_) => {}
            Load::Idle | Load::Loading => {
                body = body.push(self.list_note(Key::LabelLoading.message(), INK_SECONDARY));
            }
            Load::Empty => {
                body = body.push(self.list_note(
                    Key::CreationFlowModalCustomSetupOptionsNoVersionsAvailable.message(),
                    INK_SECONDARY,
                ));
            }
            Load::Failed(reason) => body = body.push(self.list_note(reason, Ink::Red)),
        }
        body.into()
    }

    /// The builds the *Other* chip's list is drawn from: the loader's own order,
    /// with the search applied.
    ///
    /// The same `contains` rule as the version picker's, for the same reason: a
    /// build is remembered by its end as often as its beginning, and `21.4`
    /// finds `21.4.100`.
    fn build_matches(&self) -> Vec<&store::LoaderBuild> {
        let Some(builds) = self.loader_builds.ready() else {
            return Vec::new();
        };
        let needle = self.build_query.trim().to_ascii_lowercase();
        builds
            .iter()
            .filter(|build| {
                needle.is_empty() || build.version.to_ascii_lowercase().contains(&needle)
            })
            // The dialog's own bound rather than the service's: it is the same
            // sixty the old shell's dropdown offered, and it is applied here so
            // that the list is scrollable rather than endless.
            .take(crate::catalog::MAX_OTHER_BUILDS)
            .collect()
    }

    /// The *Other* chip's list of builds, drawn in the picker's own box.
    fn build_picker(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let body: Element<'_, Message> = match &self.loader_builds {
            Load::Idle | Load::Loading => {
                self.list_note(Key::LabelLoading.message(), INK_SECONDARY)
            }
            Load::Failed(reason) => self.list_note(reason, Ink::Red),
            Load::Empty => self.list_note(
                Key::CreationFlowModalCustomSetupOptionsNoVersionsAvailable.message(),
                INK_SECONDARY,
            ),
            Load::Ready(_) => {
                let matches = self.build_matches();
                if matches.is_empty() {
                    self.list_note(
                        Key::CreationFlowModalCustomSetupOptionsNoVersionsAvailable.message(),
                        INK_SECONDARY,
                    )
                } else {
                    let rows: Vec<Element<'_, Message>> = matches
                        .into_iter()
                        .map(|build| self.build_row(build))
                        .collect();
                    container(scrollable(column(rows).width(Length::Fill)))
                        .width(Length::Fill)
                        .max_height(VERSION_LIST_HEIGHT)
                        .into()
                }
            }
        };
        container(body)
            .width(Length::Fill)
            .style(move |_theme: &Theme| container::Appearance {
                // The same box the version picker draws its options in: one list
                // of options in this dialog, drawn one way.
                background: Some(Background::Color(theme_gen::ink(theme, Ink::Surface4))),
                border: Border {
                    color: theme_gen::ink(theme, Ink::Surface5),
                    width: 1.0,
                    radius: 14.0.into(),
                },
                ..container::Appearance::default()
            })
            .into()
    }

    /// One row of the picker's list: the reference's option, lit green when it is
    /// the version in force.
    ///
    /// `getOptionClasses` in `Combobox.vue` is the whole of it: the option in
    /// force is `bg-highlight-green text-green`, every other one is `bg-surface-4
    /// text-contrast hover:brightness-[115%]`. The brightness is this option's own
    /// hover end, which is why the crossing carries it.
    fn version_row(&self, version: &store::GameVersion) -> Element<'_, Message> {
        let id = version.id.clone();
        let selected = self.chosen_version().as_deref() == Some(id.as_str());
        let key = crate::ui::scoped("shell:version", &id);
        self.option_row(key, id.clone(), selected, Message::VersionChoice(id))
    }

    /// One row of the build picker's list, which is the same option again.
    ///
    /// The version in force here is the *picked* build rather than the resolved
    /// one: the row that is lit is the row the user pressed, and under the two
    /// other chips there is no list to light anything in.
    fn build_row(&self, build: &store::LoaderBuild) -> Element<'_, Message> {
        let version = build.version.clone();
        let selected = self.build.as_deref() == Some(version.as_str());
        let key = crate::ui::scoped("shell:build", &version);
        self.option_row(key, version.clone(), selected, Message::BuildPicked(version))
    }

    /// One option of a picker's list, drawn identically wherever it is offered.
    fn option_row(
        &self,
        key: &'static str,
        label: String,
        selected: bool,
        on_press: Message,
    ) -> Element<'_, Message> {
        let theme = self.theme;
        let (factor, _) = crate::ui::interaction(key);
        let (background, ink) = if selected {
            (theme_gen::ink(theme, Ink::GreenHighlight), theme_gen::ink(theme, Ink::Green))
        } else {
            (
                crate::theme::brightness(theme_gen::ink(theme, Ink::Surface4), factor),
                crate::theme::brightness(theme_gen::ink(theme, INK_CONTRAST), factor),
            )
        };
        let option = container(
            text(label)
                .size(14.0)
                .font(semibold())
                .style(iced::theme::Text::Color(ink)),
        )
        .width(Length::Fill)
        .padding(Padding {
            top: VERSION_ROW_HEIGHT,
            bottom: VERSION_ROW_HEIGHT,
            left: VERSION_ROW_SIDE,
            right: VERSION_ROW_SIDE,
        })
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(background)),
            ..container::Appearance::default()
        });
        mouse_area(option)
            .on_enter(Message::hover_with(key, true, VERSION_ROW_HOVER))
            .on_exit(Message::hover_with(key, false, VERSION_ROW_HOVER))
            .on_press(on_press)
            .into()
    }

    /// One sentence of a picker's own body, in the box its options are drawn in.
    ///
    /// Shared by both of the creation dialog's lists -- the game versions and the
    /// loader builds -- because the four states they can be in are the same four
    /// states, and a picker that said *nothing here* for one of them and drew an
    /// empty box for the other would be two answers to one question.
    fn list_note(&self, sentence: &str, ink: Ink) -> Element<'_, Message> {
        let theme = self.theme;
        container(
            text(sentence.to_string())
                .size(14.0)
                .font(medium())
                .style(iced::theme::Text::Color(theme_gen::ink(theme, ink))),
        )
        .width(Length::Fill)
        .padding(Padding {
            top: VERSION_ROW_HEIGHT,
            bottom: VERSION_ROW_HEIGHT,
            left: VERSION_ROW_SIDE,
            right: VERSION_ROW_SIDE,
        })
        .into()
    }

    /// The picker's footer: the reference's own row that adds the snapshots and
    /// old builds to the list, and takes them away again.
    ///
    /// Its hover is `text-secondary hover:text-contrast`, which is an ink that
    /// moves rather than a surface that brightens, so it is drawn from the clock's
    /// hover *fraction* instead of from its brightness factor.
    fn version_toggle(&self, key: &'static str, label: Key, glyph: Glyph) -> Element<'_, Message> {
        let theme = self.theme;
        let (_, fraction) = crate::ui::interaction(key);
        let ink = crate::theme::mix(
            theme_gen::ink(theme, INK_SECONDARY),
            theme_gen::ink(theme, INK_CONTRAST),
            fraction,
        );
        let row = container(
            row![]
                .align_items(Alignment::Center)
                .spacing(6.0)
                .push(icon::icon(glyph, 16.0, ink))
                .push(
                    text(label.message())
                        .size(14.0)
                        .font(semibold())
                        .style(iced::theme::Text::Color(ink)),
                ),
        )
        .width(Length::Fill)
        .center_x()
        .padding(Padding {
            top: VERSION_ROW_HEIGHT,
            bottom: VERSION_ROW_HEIGHT,
            left: 0.0,
            right: 0.0,
        });
        mouse_area(row)
            .on_enter(Message::hover(key, true))
            .on_exit(Message::hover(key, false))
            .on_press(Message::VersionSnapshots(!self.version_snapshots))
            .into()
    }

    /// The import step: what the other launchers on this machine hold, one row
    /// and one button each.
    ///
    /// The list was read when the dialog opened, which is why it is a field: the
    /// scan walks every root this launcher knows how to read, and a walk per frame
    /// would be a walk per frame. What it found is what the reference's own import
    /// step lists -- the launcher each instance came from is `ImportCandidate`'s
    /// own `origin`, not a guess made here.
    fn import_dialog(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let mut body = column![].spacing(12.0);
        if let Some(reason) = &self.import_error {
            body = body.push(
                text(reason.clone())
                    .size(14.0)
                    .font(medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, Ink::Red))),
            );
        }
        if self.import_found.is_empty() {
            body = body.push(
                text(Key::CreationFlowModalImportInstanceNotificationNoInstancesFoundTitle.message())
                    .size(16.0)
                    .font(semibold())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
            );
            body = body.push(
                text(Key::CreationFlowModalImportInstanceNotificationNoInstancesFoundText.message())
                    .size(14.0)
                    .font(medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_DEFAULT))),
            );
        } else {
            for candidate in &self.import_found {
                let key = crate::ui::scoped("shell:import", &candidate.source.to_string_lossy());
                body = body.push(
                    row![]
                        .spacing(ROW_GAP)
                        .align_items(Alignment::Center)
                        .push(
                            column![]
                                .spacing(2.0)
                                .push(
                                    text(candidate.name.clone())
                                        .size(14.0)
                                        .font(semibold())
                                        .style(iced::theme::Text::Color(theme_gen::ink(
                                            theme,
                                            INK_CONTRAST,
                                        ))),
                                )
                                .push(
                                    text(candidate.origin)
                                        .size(13.0)
                                        .font(medium())
                                        .style(iced::theme::Text::Color(theme_gen::ink(
                                            theme,
                                            INK_SECONDARY,
                                        ))),
                                ),
                        )
                        .push(Space::with_width(Length::Fill))
                        .push(crate::ui::button_or(
                            theme,
                            key,
                            Key::CreationFlowModalImportInstanceActionAdd,
                            crate::ui::Kind::Standard,
                            (!self.importing).then_some(Message::Import(candidate.source.clone())),
                        )),
                );
            }
        }
        self.dialog(
            Key::CreationFlowModalImportInstanceLauncherInstancesTitle,
            body.into(),
        )
    }

    /// The scrim and the dialog, over whatever the shell was drawing.
    fn modal_layer(&self) -> Element<'_, Message> {
        let theme = self.theme;
        let dialog = match self.modal {
            Some(Modal::Create) => self.create_dialog(),
            Some(Modal::Import) => self.import_dialog(),
            // The layer is only drawn while a modal is up, and Settings is the one
            // that exists: a `None` here is not reachable from `render`.
            Some(Modal::Settings) | None => self.settings_dialog(),
        };
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

/// A hairline in `--brand-gradient-border`, the reference's `border-l` on the
/// panel and `border-b` under each of its sections.
///
/// iced paints a `Border` on all four sides of a box, so the one edge the
/// reference asks for is a widget: a one-pixel box, down the panel's page edge or
/// across a section's foot.
fn hairline(theme: Gen, vertical: bool) -> Element<'static, Message> {
    let (width, height) = if vertical {
        (Length::Fixed(1.0), Length::Fill)
    } else {
        (Length::Fill, Length::Fixed(1.0))
    };
    container(Space::with_width(Length::Fill))
        .width(width)
        .height(height)
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(theme_gen::ink(theme, Ink::BrandGradientBorder))),
            ..container::Appearance::default()
        })
        .into()
}

/// The account the card's header names: the one in force, or the reference's own
/// sentence for a file whose selection is gone.
///
/// `AccountsCard.vue`'s title slot: `selectedAccount ? selectedAccount.profile.name
/// : messages.selectAccount`. A selection that names an account the file no longer
/// holds is the same case as none at all rather than the wrong name.
fn card_title(accounts: &[AccountEntry], selected: Option<&str>) -> String {
    match selected.and_then(|uuid| accounts.iter().find(|account| account.uuid == uuid)) {
        Some(account) => account.username.clone(),
        None => Key::MinecraftAccountSelectAccount.message().to_string(),
    }
}

/// The accounts card's own frame: `bg-button-bg border-surface-5 rounded-xl`.
///
/// `pad` is the caller's because the card's two states differ: the empty one is
/// `p-3` around its own stack, and the accordion has none of its own -- its header
/// and its body each carry theirs.
fn card_frame<'a, Message: 'a>(
    theme: Gen,
    pad: f32,
    body: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(body)
        .width(Length::Fill)
        .padding(pad)
        .style(move |_theme: &Theme| container::Appearance {
            background: Some(Background::Color(theme_gen::ink(theme, Ink::ButtonBg))),
            border: Border {
                color: theme_gen::ink(theme, Ink::Surface5),
                width: 1.0,
                radius: CARD_FRAME_RADIUS.into(),
            },
            ..container::Appearance::default()
        })
        .into()
}

/// A crossing published by one of the card's own surfaces.
///
/// The kit's controls publish [`Message::Control`] with no hover end, and the
/// shell answers those with the global brightening. The card's header and its rows
/// are `button-base`, whose own rule is `filter: brightness(0.85)`, so the two
/// surfaces that dim name the factor they end at where the crossing is made.
fn card_crossing(key: &'static str, over: bool) -> Message {
    Message::Control { key, over, hover: Some(CARD_PRESS_HOVER) }
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
    /// `--shot PATH`: draw this window, write its pixels to PATH as a PNG, and
    /// close.
    ///
    /// The flag the page gates are run through (`tools/appshot.py`), and a flag
    /// of the launcher rather than a screenshot tool reaching in from outside for
    /// the reason [`write_shot`] gives: only the process that drew the frame can
    /// hand back exactly that frame. A capture states its own size and is born
    /// off the desktop, so taking one neither resizes anything nor interrupts
    /// whoever is at the machine.
    pub shot: Option<std::path::PathBuf>,
}

impl Flags {
    /// Read the shell's own flags out of a command line.
    pub fn from_args(args: impl Iterator<Item = String>) -> Flags {
        let mut flags = Flags::default();
        let mut args = args.peekable();
        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--page" => flags.page = args.next(),
                "--shot" => flags.shot = args.next().map(std::path::PathBuf::from),
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

/// Subscription id of the window's own state, reported by the window procedure.
const WINDOW_STATE_ID: &str = "palantirmc-window-state";
/// Subscription id of the `--shot` capture's settle timer.
const SHOT_ID: &str = "palantirmc-shot";
/// How long a `--shot` run gives the window before taking its picture.
///
/// The number the old shell used, kept because the page gates were measured
/// against it: long enough that a page which reads a file or asks a service has
/// its answer on screen, short enough that a run of captures is not waiting on
/// the timer.
const SHOT_SETTLE: std::time::Duration = std::time::Duration::from_millis(3000);

/// The window's own state, as the window procedure reports it.
///
/// Neither thing it reports can be a widget's message. The maximize control is
/// answered as non-client -- which is what puts Snap Layouts on it -- so no
/// widget ever sees that pointer; and a window maximizes by paths with no button
/// in them at all: Aero Snap, the taskbar, `Win`+`Up`, and the native control
/// itself. iced rebuilds a view only when a message arrives, so both need a
/// message, and both change from outside the app. Polling for them would redraw
/// an idle launcher forever; instead the window procedure -- which is told about
/// the pointer by Windows and about maximizing by the window itself -- reports
/// each change down the pipe this subscription holds open. The cost is zero
/// until something actually changes.
fn window_state() -> Subscription<Message> {
    iced::subscription::channel(WINDOW_STATE_ID, 4, |mut sender| async move {
        let (reports, mut receiver) = futures::channel::mpsc::unbounded();
        crate::native::watch_window_state(reports);
        loop {
            if futures::StreamExt::next(&mut receiver).await.is_none() {
                break;
            }
            if sender.try_send(Message::WindowStateChanged).is_err() {
                break;
            }
        }
        // A channel subscription's future never resolves -- iced ends it by
        // dropping the receiver -- and `Infallible` is how that is said.
        futures::future::pending::<std::convert::Infallible>().await
    })
}

/// The maximize control's rectangle, for the window procedure.
///
/// [`Shell::window_controls`] draws the row from these constants -- `px-1.5`,
/// three `h-9 w-9` buttons, two `gap-2`s, all centred in the head's own height --
/// so the button's rectangle is arithmetic rather than a measurement. A rectangle
/// that cannot go stale does not have to be kept in step by hand, and publishing
/// it is what makes Windows answer the pointer over it as a caption button.
fn caption_target() -> crate::native::CaptionTarget {
    crate::native::CaptionTarget {
        right_inset: CONTROLS_PAD + CONTROLS_BUTTON + CONTROLS_GAP,
        width: CONTROLS_BUTTON,
        top: (BAR - CONTROLS_BUTTON) / 2.0,
        bottom: (BAR + CONTROLS_BUTTON) / 2.0,
    }
}

/// Write a captured frame to `path` as a PNG.
///
/// Written by this process rather than read out of the window by a tool, and
/// that is the whole reason `--shot` is a launcher flag: iced draws the window,
/// so the pixels it hands back are exactly the frame it drew -- no occlusion by
/// whatever else is on the desktop, no scaling by a screen capture, and no
/// dependence on a compositor this build may not even be running on.
fn write_shot(path: &std::path::Path, shot: &iced::window::Screenshot) -> Result<(), String> {
    // `::image`, not `image`: this file imports iced's `image` widget, so the
    // bare name is the widget rather than the crate that encodes the PNG.
    let (width, height) = (shot.size.width, shot.size.height);
    let pixels = shot.bytes.as_ref().clone();
    let image = ::image::RgbaImage::from_raw(width, height, pixels)
        .ok_or_else(|| format!("{} bytes is not {width}x{height} of RGBA", shot.bytes.len()))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    image.save(path).map_err(|error| error.to_string())
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
        // The engine's cache goes in this launcher's own `cache/meta/`, which is
        // where the launcher already keeps metadata it has fetched: a second cache
        // directory would be a second set of stale answers nobody knows about.
        let store = Store::load(&paths).with_engine(Engine::new(&paths));
        // The accounts are the launcher's own file in its data root, read once
        // per window: a launch signs in with whatever is selected, and a launch
        // that renews a session writes it back here.
        let (accounts, accounts_warning) =
            crate::accounts::AccountsStore::load_with_report(&paths.root.join("accounts.json"));
        let mut shell = Shell::new(flags.opening(), theme, &settings)
            .with_store(store)
            .with_prefs(paths.clone(), prefs)
            .with_accounts(accounts, accounts_warning);
        // A capture request is in place before the first frame, which is what
        // makes the settle timer count from the window opening rather than from
        // the first message to arrive after it.
        if let Some(path) = flags.shot.clone() {
            shell.shot = Some(path);
        }
        // The page a window opens on may owe a request before any message has
        // arrived -- `/browse/modpack` owes a search -- and the first frame is the
        // first moment there is anywhere to put the answer, so it is asked for
        // here rather than waited for.
        let command = shell.opening_command();
        (shell, command)
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
        // Four subscriptions and none is owed: the frame clock while something is
        // moving, the launch while a game is being started or is up, the window's
        // own state while Windows can change it, and the settle timer while a
        // capture is waiting. Each says `none` when it is not needed, which is
        // what keeps an idle window -- and a launcher with nothing running -- from
        // waking anything up.
        let frames = if self.animating() { self.frames() } else { Subscription::none() };
        Subscription::batch([frames, self.launching(), window_state(), self.capture()])
    }
}

impl Shell {
    /// One frame of the clock, while anything is moving.
    fn frames(&self) -> Subscription<Message> {
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

    /// The capture a `--shot` run asked for: one settle timer, then the frame.
    ///
    /// The settle is not politeness: a window asked for its pixels on its first
    /// frame would capture whatever had been laid out by then, and a page that
    /// reads a file or asks a service is not finished by its first frame. The
    /// timer is a thread for the same reason the frame clock is one, and it ends
    /// with the subscription: iced drops the receiver once the run has its
    /// answer, and the thread's next send fails.
    fn capture(&self) -> Subscription<Message> {
        if self.shot.is_none() || self.shot_taken {
            return Subscription::none();
        }
        iced::subscription::channel(SHOT_ID, 1, |mut sender| async move {
            let _ = std::thread::spawn(move || {
                std::thread::sleep(SHOT_SETTLE);
                let _ = sender.try_send(Message::ShotDue);
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
    fn a_create_that_cannot_run_keeps_the_dialog_and_says_why() {
        // What a reader must not get is a dialog that closed and nothing else: a
        // store with no launcher behind it is the smallest failure to check that
        // with, and it is the one a test can produce.
        let mut shell = shell_at("/");
        press(&mut shell, Message::Rail(Rail::CreateInstance));
        press(&mut shell, Message::CreateName("Sodium test".to_string()));
        press(&mut shell, Message::Create);
        assert!(shell.creating, "the button is unusable while it is in flight");
        assert!(
            !shell.create_requested,
            "the flag is what `handle` takes to build the command, so it is already spent"
        );
        press(
            &mut shell,
            Message::Created(Err("give the instance a name".to_string())),
        );
        assert_eq!(shell.modal, Some(Modal::Create), "the dialog stays up");
        assert_eq!(shell.create_error.as_deref(), Some("give the instance a name"));
        assert!(!shell.creating, "and the button is usable again");
        // Typing again clears it: the sentence was about the name that changed.
        press(&mut shell, Message::CreateName("Sodium".to_string()));
        assert_eq!(shell.create_error, None);
    }

    #[test]
    fn the_import_step_says_when_this_machine_holds_nothing() {
        // A test's shell has no launcher behind it, so the scan finds nothing and
        // the dialog has to say so: an empty list with a title is the shape that
        // reads as "there is nothing here" rather than as a broken dialog.
        let mut shell = shell_at("/");
        press(
            &mut shell,
            Message::Screen(pages::Message::Home(pages::home::Message::ImportFromLauncher)),
        );
        assert_eq!(shell.modal, Some(Modal::Import));
        assert!(shell.import_found.is_empty());
        drop(shell.render());
    }

    /// A shell with a launcher behind it: a data root, so a launch has somewhere
    /// to run, and no account file, which is the offline session a fresh install
    /// starts with.
    fn shell_with_home(name: &str) -> Shell {
        let root = std::env::temp_dir().join("palantirmc-shell-launch").join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a scratch root");
        let paths = palantir_core::paths::PalantirPaths::at(root);
        Shell::new(Address::at(Route::Home), Gen::Dark, &RailSettings::default())
            .with_store(Store::load(&paths))
            .with_prefs(paths, crate::prefs::Prefs::default())
    }

    /// Ask the instance page on screen to run its instance.
    fn play(shell: &mut Shell) {
        press(
            shell,
            Message::Screen(pages::Message::Instance(pages::instance::Message::Play)),
        );
    }

    #[test]
    fn playing_an_instance_starts_a_run_the_pages_can_see() {
        // The whole of what the Play button does: the page reports it, the shell
        // remembers the run -- which is what the subscription reads -- and the
        // store says so, which is what the header's control is drawn from.
        let mut shell = shell_with_home("play");
        press(&mut shell, Message::Go("/instance/atm10".into()));
        play(&mut shell);
        let run = shell.run.as_ref().expect("a run");
        assert_eq!(run.instance_id, "atm10");
        assert_eq!(run.run_id, 1);
        assert_eq!(run.data_root, shell.home.as_ref().expect("a home").root);
        assert!(
            !run.account.kind.is_online(),
            "no account chosen is the offline session, not a refusal to launch"
        );
        assert_eq!(run.account.username, "Player", "which is the same player every time");
        assert_eq!(shell.store.launch_state("atm10"), store::LaunchState::Starting);
        assert_eq!(shell.store.launch_line("atm10"), Some("preparing 'atm10'"));
        // The same message from another instance's page is not this instance's
        // run: the address is what the page is drawn from, so the page on screen
        // is the one that asked.
        press(&mut shell, Message::Go("/instance/other".into()));
        play(&mut shell);
        assert_eq!(shell.run.as_ref().map(|run| run.instance_id.clone()), Some("atm10".to_string()));
        assert_eq!(shell.runs, 1, "a launch is not started twice behind one game");
    }

    #[test]
    fn a_run_reports_what_it_is_doing_and_the_header_follows() {
        // Every fact a launch sends has exactly one place to land, and the run id
        // is checked first: a `Done` from the run before must not clear the run
        // that is going, which is a stale frame the shell has to survive.
        let mut shell = shell_with_home("events");
        press(&mut shell, Message::Go("/instance/atm10".into()));
        play(&mut shell);
        press(
            &mut shell,
            Message::Launched(launch::LaunchEvent::Progress {
                run_id: 1,
                progress: crate::install::Progress::new("libraries", 3, 12, 0),
            }),
        );
        assert_eq!(shell.store.launch_line("atm10"), Some("libraries 3/12 (25%)"));
        press(
            &mut shell,
            Message::Launched(launch::LaunchEvent::Log {
                run_id: 1,
                lines: vec!["resolving version 1.21.4".to_string()],
            }),
        );
        assert_eq!(shell.store.launch_line("atm10"), Some("resolving version 1.21.4"));
        press(&mut shell, Message::Launched(launch::LaunchEvent::Started { run_id: 1 }));
        assert_eq!(shell.store.launch_state("atm10"), store::LaunchState::Running);
        assert_eq!(
            shell.store.launch_line("atm10"),
            Some("resolving version 1.21.4"),
            "the game coming up changes the control and not the run's own line"
        );
        // And the stale frame: a done from a run this shell has already replaced.
        press(&mut shell, Message::Launched(launch::LaunchEvent::Done {
            run_id: 1,
            note: "process exited (exit status: 0)".to_string(),
        }));
        assert!(shell.run.is_none(), "the run is over");
        assert_eq!(shell.store.launch_state("atm10"), store::LaunchState::Idle);
        assert_eq!(
            shell.store.launch_line("atm10"),
            Some("process exited (exit status: 0)"),
            "and the last thing it said is still what the header shows"
        );
        play(&mut shell);
        assert_eq!(shell.run.as_ref().map(|run| run.run_id), Some(2));
        press(&mut shell, Message::Launched(launch::LaunchEvent::Done {
            run_id: 1,
            note: "stale".to_string(),
        }));
        assert!(shell.run.is_some(), "a stale done must not clear the run that is going");
        press(
            &mut shell,
            Message::Launched(launch::LaunchEvent::Progress {
                run_id: 1,
                progress: crate::install::Progress::new("assets", 1, 2, 0),
            }),
        );
        assert_eq!(
            shell.store.launch_line("atm10"),
            Some("preparing 'atm10'"),
            "and a stale line must not reach the run that replaced it"
        );
    }

    #[test]
    fn stopping_is_a_state_the_header_can_draw_and_a_kill_the_shell_can_send() {
        // Stop is two things at once, and the test asserts both: the store says
        // the run is stopping, which is what the button is drawn from, and the
        // shell has asked the child to die. There is no child in a test -- the
        // slot is empty -- so what is asserted is that the request is harmless
        // and that the state is set before the answer comes back.
        //
        // The game has to be *up* first, and that is the behaviour this test now
        // pins rather than an inconvenience: a stop while the launcher is still
        // preparing has no child to kill, and setting *Stopping* for it would
        // leave the run to start the game under a button that said it was being
        // taken down. Pressing it before the run is up is refused instead, which
        // is why the instance page draws the button unusable in that state.
        let mut shell = shell_with_home("stop");
        press(&mut shell, Message::Go("/instance/atm10".into()));
        play(&mut shell);
        press(
            &mut shell,
            Message::Screen(pages::Message::Instance(pages::instance::Message::Stop)),
        );
        assert_eq!(
            shell.store.launch_state("atm10"),
            store::LaunchState::Starting,
            "nothing to kill yet"
        );
        press(&mut shell, Message::Launched(launch::LaunchEvent::Started { run_id: 1 }));
        assert_eq!(shell.store.launch_state("atm10"), store::LaunchState::Running);
        press(
            &mut shell,
            Message::Screen(pages::Message::Instance(pages::instance::Message::Stop)),
        );
        assert_eq!(shell.store.launch_state("atm10"), store::LaunchState::Stopping);
        assert!(shell.run.is_some(), "the run is not over until the worker says so");
        // A stop for an instance that is not the one running is ignored rather
        // than killing the wrong game, and so is a second one for the same run.
        press(
            &mut shell,
            Message::Screen(pages::Message::Instance(pages::instance::Message::Stop)),
        );
        assert_eq!(shell.store.launch_state("atm10"), store::LaunchState::Stopping);
    }

    #[test]
    fn a_renewed_session_is_written_back_to_the_accounts_file() {
        // A launch renews the Microsoft session inside the worker, because it
        // cannot wait for the window to notice; storing it is the window's,
        // because the file is. The test asserts the file, not the struct: a
        // store that was updated and never saved is the same as one that was
        // never updated.
        let root = std::env::temp_dir().join("palantirmc-shell-launch").join("accounts");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a scratch root");
        let path = root.join("accounts.json");
        let mut accounts = crate::accounts::AccountsStore::load_with_report(&path).0;
        accounts
            .upsert_microsoft(crate::accounts::AccountEntry::microsoft(
                "Notch",
                "069a79f444e94726a5befca90e38aaf5",
                "old-token",
                "old-refresh",
                0,
                Some(true),
            ))
            .expect("an account");
        accounts.select("069a79f444e94726a5befca90e38aaf5").expect("selected");
        accounts.save().expect("written");
        let paths = palantir_core::paths::PalantirPaths::at(root.clone());
        let mut shell = Shell::new(Address::at(Route::Home), Gen::Dark, &RailSettings::default())
            .with_store(Store::load(&paths))
            .with_prefs(paths, crate::prefs::Prefs::default())
            .with_accounts(accounts, None);
        press(&mut shell, Message::Go("/instance/atm10".into()));
        play(&mut shell);
        assert!(
            shell.run.as_ref().expect("a run").account.kind.is_online(),
            "a Microsoft account launches with its own session"
        );
        press(
            &mut shell,
            Message::Launched(launch::LaunchEvent::Tokens(launch::RefreshedTokens {
                uuid: "069a79f444e94726a5befca90e38aaf5".to_string(),
                name: "Notch".to_string(),
                access_token: "new-token".to_string(),
                refresh_token: Some("new-refresh".to_string()),
                expires_at_ms: 42,
            })),
        );
        let written = std::fs::read_to_string(&path).expect("the file");
        assert!(written.contains("new-token"), "the renewal did not reach the disk: {written}");
        assert!(written.contains("new-refresh"), "{written}");
        // And a renewal for an account this launcher does not know is dropped
        // rather than invented.
        press(
            &mut shell,
            Message::Launched(launch::LaunchEvent::Tokens(launch::RefreshedTokens {
                uuid: "nobody".to_string(),
                name: "Nobody".to_string(),
                access_token: "x".to_string(),
                refresh_token: None,
                expires_at_ms: 1,
            })),
        );
        let written = std::fs::read_to_string(&path).expect("the file");
        assert!(!written.contains("\"x\""), "{written}");
    }

    #[test]
    fn a_shell_with_nowhere_to_launch_says_so_rather_than_starting_nothing() {
        // A test's shell has no data root, so there is nowhere for an instance to
        // be: the honest answer is the sentence every other unwired control in
        // this rewrite gives, and the run is not started.
        let mut shell = shell_at("/instance/atm10");
        play(&mut shell);
        assert!(shell.run.is_none());
        assert_eq!(
            shell.store.launch_line("atm10"),
            Some(store::not_implemented("Launching an instance").as_str())
        );
    }

    /// A version list in the shape Mojang publishes: two releases, a snapshot and
    /// an old beta, newest first.
    fn version_list() -> store::VersionList {
        store::VersionList {
            latest_release: "1.21.4".to_string(),
            versions: [("25w02a", false), ("1.21.4", true), ("1.21.3", true), ("b1.7.3", false)]
                .into_iter()
                .map(|(id, release)| store::GameVersion { id: id.to_string(), release })
                .collect(),
        }
    }

    /// The versions the picker would draw, in the order it draws them.
    fn version_ids(shell: &Shell) -> Vec<&str> {
        shell.version_matches().into_iter().map(|version| version.id.as_str()).collect()
    }

    #[test]
    fn the_action_bar_follows_the_run_from_any_page_and_stops_what_is_running() {
        // The surface the plan names: a run used to be watchable only in the
        // header of the instance it belonged to, which meant navigating away from
        // it hid it. The bar is drawn from the store, so the run follows the
        // reader instead -- and the two facts it carries are the two the reference
        // carries: what is running, and how to stop it.
        let mut shell = shell_with_home("action-bar");
        press(&mut shell, Message::Go("/instance/atm10".into()));
        play(&mut shell);
        assert_eq!(shell.store.launch_state("atm10"), store::LaunchState::Starting);
        press(&mut shell, Message::Go("/browse/modpack".into()));
        assert_eq!(
            shell.store.launch_state("atm10"),
            store::LaunchState::Starting,
            "the run is the launcher's, not the page's"
        );
        assert_eq!(state_label(store::LaunchState::Starting), Some("Starting..."));
        assert_eq!(state_label(store::LaunchState::Stopping), Some("Stopping..."));
        assert_eq!(state_label(store::LaunchState::Running), None, "nothing to say");
        assert_eq!(state_label(store::LaunchState::Idle), None, "and nothing running");
        // The bar's own stop, while the launcher is still *preparing*: there is no
        // child process to kill yet, so the control is not drawn and this message
        // is not one it can send. What a press would do otherwise is change a word
        // and leave the run to start the game anyway.
        press(&mut shell, Message::StopRun);
        assert_eq!(
            shell.store.launch_state("atm10"),
            store::LaunchState::Starting,
            "nothing to stop yet"
        );
        press(&mut shell, Message::Launched(launch::LaunchEvent::Started { run_id: 1 }));
        assert_eq!(shell.store.launch_state("atm10"), store::LaunchState::Running);
        press(&mut shell, Message::StopRun);
        assert_eq!(shell.store.launch_state("atm10"), store::LaunchState::Stopping);
        // And the name the chip draws: the library's own when it has been read,
        // the id when it has not -- which is the arm an instance removed while it
        // was running lands in.
        assert_eq!(
            instance_name(&shell.store, "atm10"),
            "atm10",
            "the id, for an instance the library has not read"
        );
        drop(shell.render());
    }

    #[test]
    fn the_download_panel_is_the_launch_s_own_level_and_it_goes_when_the_fetch_does() {
        // The panel is drawn from the level the run reported, which is the reason
        // `LaunchEvent::Progress` is kept as numbers rather than only as the
        // sentence the instance header shows.
        let mut shell = shell_with_home("downloads");
        press(&mut shell, Message::Go("/instance/atm10".into()));
        play(&mut shell);
        press(&mut shell, Message::ToggleDownloads);
        assert!(shell.downloads, "the chip is the panel's own toggle");
        assert!(shell.progress.is_none(), "and there is nothing to show yet");
        drop(shell.render());
        press(
            &mut shell,
            Message::Launched(launch::LaunchEvent::Progress {
                run_id: 1,
                progress: crate::install::Progress::new("files", 3, 12, 6 * 1024 * 1024),
            }),
        );
        let progress = shell.progress.clone().expect("the level");
        assert_eq!(progress.fraction(), 0.25);
        assert_eq!(progress_line(&progress), "files: 3 of 12 · 6.0 MB");
        // An indeterminate level says so in words rather than inventing a
        // fraction: the launcher's own "loading" states have no countable total.
        assert_eq!(
            progress_line(&crate::install::Progress::starting("installing Java")),
            "installing Java…"
        );
        drop(shell.render());
        // The game coming up is the end of the fetch: the level goes with it, and
        // so does the panel a bar would have been left in.
        press(&mut shell, Message::Launched(launch::LaunchEvent::Started { run_id: 1 }));
        assert!(shell.progress.is_none());
        assert!(!shell.downloads);
        // A run that ends clears both too, for the same reason.
        press(&mut shell, Message::ToggleDownloads);
        press(
            &mut shell,
            Message::Launched(launch::LaunchEvent::Progress {
                run_id: 1,
                progress: crate::install::Progress::starting("natives"),
            }),
        );
        press(&mut shell, Message::Launched(launch::LaunchEvent::Done {
            run_id: 1,
            note: "process exited (exit status: 0)".to_string(),
        }));
        assert!(shell.progress.is_none());
        assert!(!shell.downloads);
        assert!(shell.run.is_none());
    }

    #[test]
    fn the_creation_dialog_asks_mojang_for_the_versions_it_offers() {
        // The dialog cannot offer a version it has not been told about, and the
        // list is a request rather than a field read -- so opening the dialog sets
        // the waiting state and the flag that carries the request out of `act`,
        // which `handle` spends on the way to building it.
        let mut shell = shell_at("/");
        let _ = shell.act(Message::Rail(Rail::CreateInstance));
        assert_eq!(shell.modal, Some(Modal::Create));
        assert!(matches!(shell.versions, Load::Loading), "the dialog waits on its own request");
        assert!(shell.versions_requested, "and the request has not left `act` yet");
        press(&mut shell, Message::Versions(Ok(version_list())));
        assert_eq!(shell.versions.ready(), Some(&version_list()));
        assert!(
            !shell.versions_requested,
            "the flag is what `handle` takes to build the command, so it is already spent"
        );
        // And the picker opens on Mojang's own answer rather than on the first row
        // of the list, which is a snapshot.
        assert_eq!(shell.chosen_version().as_deref(), Some("1.21.4"));
        drop(shell.render());
    }

    /// A loader's builds as a service answers them: two stable builds, a
    /// pre-release, then an older one -- newest first, which is the order every
    /// one of these services publishes in.
    fn loader_build_list() -> Vec<store::LoaderBuild> {
        [("0.19.5", true), ("0.19.3-beta.1", false), ("0.19.2", true)]
            .into_iter()
            .map(|(version, stable)| store::LoaderBuild { version: version.to_string(), stable })
            .collect()
    }

    /// The loader and game version a request was made for, as `act` records it.
    fn asked_for(shell: &Shell) -> Option<(crate::catalog::LoaderKind, String)> {
        shell.loader_builds_for.clone()
    }

    #[test]
    fn the_creation_dialog_asks_the_loader_it_opens_on_which_builds_it_has() {
        // The chips are wired to the store's own request, and the order matters:
        // the dialog opens on Fabric, but the game version is what a loader's
        // builds are listed *for*, so the request waits for Mojang's answer rather
        // than being made against a version list that has not arrived.
        let mut shell = shell_at("/");
        press(&mut shell, Message::Rail(Rail::CreateInstance));
        assert_eq!(shell.create_loader, crate::catalog::LoaderKind::Fabric);
        assert_eq!(asked_for(&shell), None, "nothing to ask about yet");
        // Through `act` rather than `press`, for the flag's own reason: `handle`
        // spends it on the way to building the command, and what is being checked
        // here is that `act` raised it.
        let _ = shell.act(Message::Versions(Ok(version_list())));
        assert!(matches!(shell.loader_builds, Load::Loading));
        assert!(shell.loader_builds_requested, "and the request has not left `act` yet");
        assert_eq!(
            asked_for(&shell),
            Some((crate::catalog::LoaderKind::Fabric, "1.21.4".to_string())),
            "asked for the version the picker opened on"
        );
        press(
            &mut shell,
            Message::LoaderBuilds {
                loader: crate::catalog::LoaderKind::Fabric,
                game: "1.21.4".to_string(),
                builds: Ok(loader_build_list()),
            },
        );
        assert!(shell.loader_builds.ready().is_some(), "the list arrived");
        assert!(
            !shell.loader_builds_requested,
            "the flag is what `handle` takes to build the command, so it is already spent"
        );
        // *Stable* is the newest build the loader itself calls stable, which is not
        // the newest build: that is what *Latest* is for.
        assert_eq!(shell.chosen_build().as_deref(), Some("0.19.5"));
        assert!(shell.create_usable());
        drop(shell.render());
    }

    #[test]
    fn a_build_list_about_a_choice_the_dialog_has_left_is_dropped() {
        // The answer carries the question it answers, which is the whole reason
        // `LoaderBuilds` is not a bare `Result`: a Fabric list that arrived after
        // the Quilt chip was pressed would otherwise be drawn under the Quilt chip.
        let mut shell = shell_at("/");
        press(&mut shell, Message::Rail(Rail::CreateInstance));
        press(&mut shell, Message::Versions(Ok(version_list())));
        press(&mut shell, Message::LoaderChoice(crate::catalog::LoaderKind::Quilt));
        assert_eq!(asked_for(&shell).map(|(loader, _)| loader), Some(crate::catalog::LoaderKind::Quilt));
        press(
            &mut shell,
            Message::LoaderBuilds {
                loader: crate::catalog::LoaderKind::Fabric,
                game: "1.21.4".to_string(),
                builds: Ok(loader_build_list()),
            },
        );
        assert!(
            matches!(shell.loader_builds, Load::Loading),
            "the Fabric list is not an answer to the Quilt question"
        );
        // And the loader's own answer, once it is the one in force, is what the
        // chips read.
        press(
            &mut shell,
            Message::LoaderBuilds {
                loader: crate::catalog::LoaderKind::Quilt,
                game: "1.21.4".to_string(),
                builds: Ok(vec![store::LoaderBuild {
                    version: "0.30.1".to_string(),
                    stable: true,
                }]),
            },
        );
        assert_eq!(shell.chosen_build().as_deref(), Some("0.30.1"));
        // *Latest* is the newest of all, which is what a pre-release is when it is
        // the newest -- and `Other` is a build the user picks, or nothing at all.
        press(&mut shell, Message::BuildChoice(BuildChoice::Latest));
        assert_eq!(shell.chosen_build().as_deref(), Some("0.30.1"));
        press(&mut shell, Message::BuildChoice(BuildChoice::Other));
        assert_eq!(shell.chosen_build(), None, "nothing has been picked yet");
        press(&mut shell, Message::BuildPicked("0.29.0".to_string()));
        assert_eq!(
            shell.chosen_build(),
            None,
            "a build the list does not carry is not one this dialog can install"
        );
        press(&mut shell, Message::BuildPicked("0.30.1".to_string()));
        assert_eq!(shell.chosen_build().as_deref(), Some("0.30.1"));
        drop(shell.render());
    }

    #[test]
    fn the_create_button_waits_for_the_loader_s_own_answer() {
        // What the button's own rule is: a create is in flight, or a loader is
        // chosen whose list has not answered. Both are states the dialog can be
        // left from rather than a dead end -- the chips go back to Vanilla, which
        // has no list to wait for.
        let mut shell = shell_at("/");
        press(&mut shell, Message::Rail(Rail::CreateInstance));
        assert!(!shell.create_usable(), "the list has not even been asked for");
        press(&mut shell, Message::Versions(Ok(version_list())));
        assert!(!shell.create_usable(), "and it is still coming");
        press(
            &mut shell,
            Message::LoaderBuilds {
                loader: crate::catalog::LoaderKind::Fabric,
                game: "1.21.4".to_string(),
                builds: Ok(loader_build_list()),
            },
        );
        assert!(shell.create_usable());
        // Vanilla is not a service: its chips drop the build row and the list with
        // it, and there is nothing to wait for.
        press(&mut shell, Message::LoaderChoice(crate::catalog::LoaderKind::Vanilla));
        assert!(matches!(shell.loader_builds, Load::Idle), "the Fabric list is gone");
        assert_eq!(shell.chosen_build(), None, "vanilla installs no build");
        assert!(shell.create_usable());
        drop(shell.render());
        // A loader whose list *failed* is a sentence on the row and a button the
        // reader can still press: the alternative is a dialog with no way out of
        // it, and the failure is already said where it happened.
        press(&mut shell, Message::LoaderChoice(crate::catalog::LoaderKind::Forge));
        press(
            &mut shell,
            Message::LoaderBuilds {
                loader: crate::catalog::LoaderKind::Forge,
                game: "1.21.4".to_string(),
                builds: Err("Forge's build list could not be read: no route".to_string()),
            },
        );
        assert!(matches!(shell.loader_builds, Load::Failed(_)));
        assert!(shell.chosen_build().is_none());
        assert!(shell.create_usable());
        drop(shell.render());
        // The *Stable* chip's own rule: a loader that published nothing stable for
        // this game version leaves the chip in place and dim, which is the
        // reference's `disabledItems` -- and while the list is still coming,
        // nothing is known to be missing.
        let mut shell = shell_at("/");
        press(&mut shell, Message::Rail(Rail::CreateInstance));
        assert!(shell.stable_offered(), "not asked yet");
        press(&mut shell, Message::Versions(Ok(version_list())));
        press(
            &mut shell,
            Message::LoaderBuilds {
                loader: crate::catalog::LoaderKind::Fabric,
                game: "1.21.4".to_string(),
                builds: Ok(vec![store::LoaderBuild {
                    version: "0.19.3-beta.1".to_string(),
                    stable: false,
                }]),
            },
        );
        assert!(!shell.stable_offered(), "nothing stable to offer");
        // And the rule that chip stands for falls back to the newest build there
        // is, which is the one build this loader published: a row that offered
        // *Stable* and nothing to press would be a dead end of its own.
        assert_eq!(shell.chosen_build().as_deref(), Some("0.19.3-beta.1"));
        drop(shell.render());
    }

    #[test]
    fn the_picker_shows_the_releases_and_its_footer_adds_the_rest() {
        // The reference's own arrangement: the list is the releases until the
        // dropdown's footer asks for everything, and the search narrows whichever
        // list is showing.
        let mut shell = shell_at("/");
        press(&mut shell, Message::Rail(Rail::CreateInstance));
        press(&mut shell, Message::Versions(Ok(version_list())));
        assert_eq!(version_ids(&shell), vec!["1.21.4", "1.21.3"], "releases until asked for more");
        press(&mut shell, Message::VersionSnapshots(true));
        assert_eq!(version_ids(&shell), vec!["25w02a", "1.21.4", "1.21.3", "b1.7.3"]);
        // A version is found by the part of it a user remembers rather than only
        // by its beginning: `21.3` is the end of `1.21.3`.
        press(&mut shell, Message::VersionQuery("21.3".to_string()));
        assert_eq!(version_ids(&shell), vec!["1.21.3"]);
        press(&mut shell, Message::VersionQuery("nothing publishes this".to_string()));
        assert!(version_ids(&shell).is_empty());
        // A picker that found nothing is a sentence, and the sentence is drawn by
        // the same code path as the list: this is the render that proves it.
        drop(shell.render());
        // Choosing is what the heading and the create both read, and a choice
        // outranks Mojang's own answer.
        press(&mut shell, Message::VersionSnapshots(false));
        press(&mut shell, Message::VersionQuery(String::new()));
        press(&mut shell, Message::VersionChoice("1.21.3".to_string()));
        assert_eq!(shell.chosen_version().as_deref(), Some("1.21.3"));
        drop(shell.render());
    }

    #[test]
    fn a_version_list_that_did_not_arrive_is_a_sentence_and_not_an_empty_picker() {
        // The failure path, and the one that would look like a working picker if
        // the list were drawn as an empty list: the reason is what is drawn, and
        // the create still leaves with nothing chosen -- which is the store's own
        // question to Mojang rather than a version this dialog invented.
        let mut shell = shell_at("/");
        press(&mut shell, Message::Rail(Rail::CreateInstance));
        press(&mut shell, Message::Versions(Err(store::not_implemented("Minecraft's version list"))));
        assert!(matches!(shell.versions, Load::Failed(_)));
        assert_eq!(shell.chosen_version(), None);
        drop(shell.render());
        press(&mut shell, Message::Create);
        assert!(shell.creating);
        // And a list that came back with nothing in it is the other arm of the
        // same sentence: `Empty` rather than a list of nothing.
        press(
            &mut shell,
            Message::Versions(Ok(store::VersionList {
                latest_release: "1.21.4".to_string(),
                versions: Vec::new(),
            })),
        );
        assert_eq!(shell.versions, Load::Empty);
        drop(shell.render());
    }

    #[test]
    fn the_settings_modal_offers_the_themes_and_takes_one() {
        // The pane the modal has been waiting for since stage 2: the colour
        // themes `color_theme.rs` has carried all along, offered by the
        // reference's own rule and taken by the window that shows them.
        let mut shell = shell_at("/");
        press(&mut shell, Message::Rail(Rail::Settings));
        let offered = ColorTheme::options(false, shell.prefs.theme());
        assert!(offered.contains(&ColorTheme::Dark));
        assert!(offered.contains(&ColorTheme::Light));
        assert!(
            !offered.contains(&ColorTheme::Retro),
            "retro is behind dev mode until it is the theme already in force"
        );
        // Every option is a control the kit builds, which is what `render`
        // proves: a button the kit cannot build is a modal that panics.
        drop(shell.render());
        press(&mut shell, Message::ColorTheme(ColorTheme::Light));
        assert_eq!(shell.theme, Gen::Light);
        assert_eq!(shell.prefs.theme(), ColorTheme::Light);
        // And once retro *is* the theme in force, the filter keeps offering it:
        // the reference's rule read from the other side.
        press(&mut shell, Message::ColorTheme(ColorTheme::Retro));
        assert_eq!(shell.theme, Gen::Retro);
        assert!(ColorTheme::options(false, shell.prefs.theme()).contains(&ColorTheme::Retro));
        // A test has no home, so nothing was written to anyone's preferences.
        assert!(shell.home.is_none());
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

    /// A shell whose accounts come from a file of its own, which is what the
    /// panel draws from.
    fn shell_with_accounts(path: &std::path::Path) -> Shell {
        let (accounts, warning) = AccountsStore::load_with_report(path);
        shell_at("/").with_accounts(accounts, warning)
    }

    #[test]
    fn the_card_names_the_account_a_launch_would_sign_in_as() {
        let steve = AccountEntry::offline("Steve");
        let uuid = steve.uuid.clone();
        // The account in force is the one the file says.
        assert_eq!(card_title(std::slice::from_ref(&steve), Some(&uuid)), "Steve");
        // Nothing selected, or a selection for an account that is gone: the
        // reference's own sentence rather than the wrong account.
        assert_eq!(
            card_title(std::slice::from_ref(&steve), None),
            Key::MinecraftAccountSelectAccount.message()
        );
        assert_eq!(
            card_title(&[steve], Some("99999999999999999999999999999999")),
            Key::MinecraftAccountSelectAccount.message()
        );
        assert_eq!(card_title(&[], None), Key::MinecraftAccountSelectAccount.message());
        // The uuid is the profile id and an offline account's is derived, so the
        // card names the same player Prism does.
        assert_eq!(uuid, crate::accounts::offline_uuid("Steve"));
    }

    #[test]
    fn the_panel_draws_its_section_and_its_card_in_every_theme() {
        // The panel is not a wash any more: the section, the card's two branches
        // and the note all draw. In every theme, because the inks and the frame
        // come from the generated tables and one of the four could be missing a
        // token the others have.
        let dir = tempfile::tempdir().expect("a temp dir");
        let path = dir.path().join("accounts.json");
        let mut shell = shell_with_accounts(&path);
        assert!(!shell.accounts_open, "the reference's accordion starts closed");
        for theme in Gen::ALL {
            shell.theme = *theme;
            let _ = shell.panel();
        }
        // With accounts, the card draws the accordion instead, open and closed.
        {
            let accounts = shell.accounts.as_mut().expect("a store");
            accounts.add("Steve").expect("a name");
            accounts.save().expect("a written file");
        }
        press(&mut shell, Message::ToggleAccounts);
        assert!(shell.accounts_open);
        press(&mut shell, Message::SignIn);
        for theme in Gen::ALL {
            shell.theme = *theme;
            let _ = shell.panel();
        }
        press(&mut shell, Message::ToggleAccounts);
        assert!(!shell.accounts_open);
    }

    #[test]
    fn a_shell_with_no_accounts_file_is_the_card_s_own_empty_state() {
        // The shell the application builds always has a store; a shell with none
        // is what the tests build, and it draws the same card a store with no
        // accounts does.
        let mut shell = shell_at("/");
        assert!(shell.accounts.is_none());
        let _ = shell.panel();
        assert!(shell.accounts_note.is_none());
        // Nothing is written and nothing panics: there is no file to write to.
        press(&mut shell, Message::SelectAccount("nobody".into()));
        press(&mut shell, Message::RemoveAccount("nobody".into()));
        assert!(shell.accounts_note.is_none());
    }

    #[test]
    fn choosing_an_account_writes_the_file_the_other_launcher_reads() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let path = dir.path().join("accounts.json");
        let mut shell = shell_with_accounts(&path);
        let (steve, alex) = {
            let accounts = shell.accounts.as_mut().expect("a store");
            accounts.add("Steve").expect("a name");
            accounts.add("Alex").expect("a name");
            accounts.save().expect("a written file");
            let list = accounts.list();
            (list[0].uuid.clone(), list[1].uuid.clone())
        };
        assert_eq!(
            shell.accounts.as_ref().expect("a store").selected_uuid(),
            Some(steve.as_str()),
            "the first account added is the one in force"
        );

        press(&mut shell, Message::SelectAccount(alex.clone()));
        assert!(shell.accounts_note.is_none(), "{:?}", shell.accounts_note);
        let (back, warning) = AccountsStore::load_with_report(&path);
        assert!(warning.is_none(), "{warning:?}");
        assert_eq!(back.selected_uuid(), Some(alex.as_str()));

        // Taking one away is written the same way, and the file the other
        // launcher reads is the one without it.
        press(&mut shell, Message::RemoveAccount(alex));
        let (back, _) = AccountsStore::load_with_report(&path);
        assert_eq!(back.list().len(), 1);
        assert_eq!(back.list()[0].username, "Steve");
        assert!(back.selected_uuid().is_none(), "the removed account was in force");

        // A uuid that is not in the file is the store's own sentence rather than
        // a silent nothing.
        press(&mut shell, Message::SelectAccount("nope".into()));
        let note = shell.accounts_note.clone().expect("a sentence");
        assert!(note.contains("no account with id"), "{note}");
    }

    #[test]
    fn a_press_that_needs_the_sign_in_flow_says_so_in_the_panel() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let path = dir.path().join("accounts.json");
        let mut shell = shell_with_accounts(&path);
        assert!(shell.accounts_note.is_none());
        press(&mut shell, Message::SignIn);
        assert_eq!(
            shell.accounts_note.as_deref(),
            Some(store::not_implemented("Signing in to Minecraft").as_str())
        );
        // The control that reads it is the one that clears it, and a change to
        // the accounts file clears it too: what it said is no longer the news.
        press(&mut shell, Message::DismissAccountsNote);
        assert!(shell.accounts_note.is_none());
        press(&mut shell, Message::SignIn);
        assert!(shell.accounts_note.is_some());
        // One that does write the file clears it: what the sentence said is no
        // longer the news.
        let uuid = {
            let accounts = shell.accounts.as_mut().expect("a store");
            accounts.add("Steve").expect("a name");
            accounts.list()[0].uuid.clone()
        };
        press(&mut shell, Message::SelectAccount(uuid));
        assert!(shell.accounts_note.is_none());
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

    #[test]
    fn a_page_control_s_hover_is_a_frame_subscription_the_shell_owes() {
        // The join between the two clocks: the pages' controls tween on the
        // interaction clock, and the shell is what asks for the frames -- so a
        // crossing has to show up in `animating()` or the tween would paint one
        // frame and sit there (see `crate::hover`).
        // The key is this test's own, so what is asserted below is about this
        // page's control rather than about a clock the other tests share.
        let key = "shell:test:hover";
        anim::clock().lock().expect("the clock").clear();
        let mut shell = shell_at("/browse/modpack");
        // The rail's plate is the shell's own tween and a fresh shell starts one;
        // let it arrive, so what is asked below is about the page's control.
        shell.advance(Duration::from_secs(1));
        let drawn = |key: &str| anim::clock().lock().expect("the clock").drawn(key);
        assert_eq!(drawn(key), (1.0, 0.0), "nothing has crossed this control");
        press(
            &mut shell,
            Message::Screen(pages::Message::Discover(discover::Message::Hover {
                key,
                over: true,
                hover: None,
            })),
        );
        assert!(
            shell.animating(),
            "a hover in the air is the shell's frames to ask for"
        );
        // The deadline ends it, and the window goes quiet without another
        // message: a tween that settles must not hold the frames open.
        anim::clock()
            .lock()
            .expect("the clock")
            .tick(std::time::Instant::now() + crate::anim::INTERACTION_DURATION);
        assert_eq!(drawn(key), (crate::theme::hover_brightness(), 1.0));
        assert!(!shell.animating(), "the rail settled too, so nothing is moving");
    }

    #[test]
    fn a_navigation_forgets_where_the_pointer_was() {
        // The page that is being left does not draw its controls on the one that
        // arrives, so the crossing that lit one goes with it. Without this a
        // control the new page happens to name the same way would arrive lit.
        let key = "shell:test:left";
        anim::clock().lock().expect("the clock").clear();
        let mut shell = shell_at("/browse/modpack");
        shell.advance(Duration::from_secs(1));
        press(
            &mut shell,
            Message::Screen(pages::Message::Discover(discover::Message::Hover {
                key,
                over: true,
                hover: None,
            })),
        );
        assert!(shell.animating(), "the hover is what is moving now");
        press(&mut shell, Message::Go("/skins".to_string()));
        // The control is not drawn any more, so it is not hovered any more --
        // asserted on the key rather than on the clock's quiet, which every test
        // sharing a process would have a say in.
        let drawn = anim::clock().lock().expect("the clock").drawn(key);
        assert_eq!(
            drawn,
            (1.0, 0.0),
            "the crossing belonged to the page that is gone"
        );
    }
}

