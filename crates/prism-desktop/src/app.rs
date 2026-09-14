//! PalantirMC application core: state, updates and the whole `iced` view tree.
//!
//! The shell mirrors the Modrinth launcher: a custom title bar (the window is
//! undecorated, so the bar both paints and drags), a 68px icon rail on the
//! left, the active page in the middle and a contextual sidebar on the right
//! holding "Getting started", the signed-in account, the selected instance and
//! the run state.
//!
//! Nothing here is a mock-up. Every control has a real effect:
//!
//! * **Create instance** is a two-step dialog ([`CreateStep`]) whose loader
//!   chips, game-version list and loader builds come from the live metadata
//!   catalog ([`crate::catalog`]); submitting writes a real instance,
//!   installing the chosen loader through `prism-loader`.
//! * **Play / Kill** reuse [`crate::launch`] and stream the child's output
//!   through an `iced` subscription into the Logs page.
//! * **Browse** searches Modrinth and installs the newest matching file into
//!   the selected instance's `mods/` folder ([`crate::browse`]).
//! * **Import** scans other launchers on disk and copies what it finds.
//! * **Accounts** are offline or Microsoft, persisted to `accounts.json`.
//!   Microsoft sign-in runs the real device-code flow in a subscription
//!   ([`microsoft_sign_in`]) and its refresh token is renewed before a launch,
//!   so being signed in is a state the launcher keeps rather than one it
//!   announces.
//!
//! Background work follows one pattern: a flag in this struct makes
//! [`PrismApp::subscription`] expose a channel subscription whose id includes a
//! sequence number; the worker thread sends exactly one
//! [`Message::TaskDone`] and the update applies it (stale sequences are
//! dropped). The GUI thread therefore never touches the network or a slow
//! disk.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use iced::widget::{
    button, checkbox, column, container, horizontal_rule, horizontal_space, pick_list, row,
    scrollable, text, text_input, tooltip, vertical_rule, Button, Image,
};
use iced::widget::image::Handle;
use iced::{window, Command, Element, Length, Padding, Point, Subscription, Theme};
use prism_core::instance::groups::Groups;
use prism_core::instance::Instance;
use prism_core::paths::PrismPaths;
use prism_core::settings::{defaults, Settings};

use crate::accounts::AccountEntry;
use crate::brand;
use crate::browse::{self, ContentType, Hit, ImportedPack};
use crate::catalog::{self, LoaderKind, VersionCatalog};
use crate::glyphs::glyph;
use crate::icons::instance_handle;
use crate::instances::{self, InstanceCard, LoadedInstances, NewInstance};
use crate::launch::{
    self, open_in_file_manager, ActiveRunData, AccountRef, ChildSlot, LaunchParams,
};
use crate::mods::{list_content_names, list_mods, set_mod_enabled, ModEntry};
use crate::anim;
use crate::native::{self, ResizeEdge};
use crate::prefs::{self, Prefs};
use crate::screenshots;
use crate::scroll;
use crate::settings;
use crate::theme::{self, ColorTheme};

/// Maximum console lines kept in memory.
pub const CONSOLE_LINE_CAP: usize = 20_000;

/// How many of the newest console lines the view renders.
pub const CONSOLE_VIEW_LINES: usize = 500;

/// Icon-rail width, as the eye measures it.
///
/// 64 is the reference's `--left-bar-width` (`4rem`), which is 8px of padding
/// above and below a 48px button and 5px beside it. Ours was 68 with 10px all
/// round, so the rail was 4px too wide *and* its icons sat further from the
/// edge than the reference's do.
///
/// This is the drawn strip, not the container: the window is undecorated, so
/// the shell draws its own resize bands and the west one is painted in the
/// rail's colour. The container is [`RAIL_CONTAINER_WIDTH`] and the band makes
/// up the difference, which is why the strip measures 64 rather than 58.
pub const RAIL_WIDTH: f32 = 64.0;

/// The width the rail's own container is laid out at.
pub const RAIL_CONTAINER_WIDTH: f32 = RAIL_WIDTH - native::RESIZE_BAND;

/// Side of a rail entry.
///
/// The reference's rail buttons are `w-12 h-12` -- 48px square -- with a 24px
/// icon centred in them (`text-2xl`).
pub const RAIL_BUTTON: f32 = 48.0;

/// Right panel width, measured the same way as [`RAIL_WIDTH`].
///
/// 300 is the reference's `--right-bar-width`. Ours was 304, which nobody would
/// have seen; it changed only because the number is now a transcription of the
/// reference rather than a value that merely looked about right.
pub const SIDEBAR_WIDTH: f32 = 300.0;

/// The width the right panel's own container is laid out at.
pub const SIDEBAR_CONTAINER_WIDTH: f32 = SIDEBAR_WIDTH - native::RESIZE_BAND;

/// Title-bar height.
///
/// 48 is the reference's `--top-bar-height` (`3rem`), measured off its window,
/// which is also where the 1px rule under the bar was measured.
pub const TITLE_BAR_HEIGHT: f32 = 48.0;

/// Padding above and below the title bar's contents.
pub const TITLE_BAR_PAD: f32 = 6.0;

/// The height the title bar's controls and grab patches occupy, between the
/// padding.
pub const TITLE_BAR_CONTENT_HEIGHT: f32 = TITLE_BAR_HEIGHT - 2.0 * TITLE_BAR_PAD;

/// Padding around the title bar's contents.
///
/// The grabbable patches size themselves from this rather than guessing, so
/// the strip you can drag covers the bar from its top edge to its bottom edge.
/// A patch that stopped short would leave a lip along the top and bottom of the
/// bar that looks draggable and swallows the press.
fn title_bar_padding() -> Padding {
    Padding {
        top: TITLE_BAR_PAD,
        right: 10.0,
        bottom: TITLE_BAR_PAD,
        left: 10.0,
    }
}

/// Side of a caption control's glyph.
///
/// Shared with [`caption_target`] rather than written twice: the non-client
/// region Windows is told about has to be the button the user can see, and two
/// numbers that merely happen to agree do not stay equal through a redesign.
const CAPTION_GLYPH: f32 = 14.0;

/// A caption control's padding — vertical first, then horizontal.
const CAPTION_PAD: [f32; 2] = [5.0, 9.0];

/// A caption control's width: its glyph plus its horizontal padding.
const CAPTION_BUTTON_WIDTH: f32 = CAPTION_GLYPH + 2.0 * CAPTION_PAD[1];

/// A caption control's height: its glyph plus its vertical padding.
const CAPTION_BUTTON_HEIGHT: f32 = CAPTION_GLYPH + 2.0 * CAPTION_PAD[0];

/// Gap between the title bar's controls.
const TITLE_BAR_SPACING: f32 = 6.0;

/// Where the title bar's maximize control sits, for the window's hit test.
///
/// The button is answered as *non-client* — that is what makes Windows 11
/// offer Snap Layouts when the pointer rests on it — which means Windows, not
/// iced, sees the click. So Windows has to be told where the button is, before
/// the pointer gets anywhere near it.
///
/// Every quantity is derived from the constants the bar lays itself out with,
/// so the region cannot drift away from the button it belongs to. Read it as
/// the path the layout takes from the client's top-right corner: in over the
/// frame band, over the bar's padding, and past the close control and one gap.
///
/// The frame band counts on *both* axes, and forgetting it on the horizontal
/// one is not a rounding error: the shell is inset by the band on each side, so
/// a region measured from the client's edge lands six pixels to the right of
/// the button it is meant to cover — close enough to look right in a diagram,
/// and wrong.
fn caption_target() -> native::CaptionTarget {
    // The bar sits below the frame band, and the buttons centre themselves in
    // the bar's content height.
    let button_top = native::RESIZE_BAND
        + TITLE_BAR_PAD
        + (TITLE_BAR_CONTENT_HEIGHT - CAPTION_BUTTON_HEIGHT) / 2.0;
    native::CaptionTarget {
        // The side frame band, then the bar's padding, then the close control —
        // the row's last item — and one gap back to the maximize control.
        right_inset: native::RESIZE_BAND
            + title_bar_padding().right
            + CAPTION_BUTTON_WIDTH
            + TITLE_BAR_SPACING,
        width: CAPTION_BUTTON_WIDTH,
        top: button_top,
        bottom: button_top + CAPTION_BUTTON_HEIGHT,
    }
}

/// How long two presses on the title bar may be apart and still count as a
/// double-click. Matches Windows' own 500ms default closely enough that the
/// gesture feels the same as it does on a decorated window.
pub const DOUBLE_CLICK: Duration = Duration::from_millis(500);

/// How far the pointer must travel after a press on the title bar before the
/// window starts following it, in logical pixels.
///
/// The same job as Windows' own `SM_CXDRAG`/`SM_CYDRAG` (4px by default), and
/// not a cosmetic detail: the drag is started by handing the mouse to Windows'
/// modal move loop, and a move loop swallows the second press of a
/// double-click. Starting it on the press therefore made double-click to
/// maximize and restore unreliable. Waiting for real movement keeps an
/// ordinary click — which is a press, a pixel or two of jitter, and a release —
/// from ever entering that loop.
pub const BAR_DRAG_THRESHOLD: f32 = 4.0;

/// Instance cards per grid row (iced 0.12 has no flow layout, so rows are
/// chunked; two wide cards read best next to the sidebar).
pub const CARDS_PER_ROW: usize = 2;

/// Instance icon side inside a card.
pub const CARD_ICON: f32 = 52.0;

/// Instance icon side inside a compact card, proportionally smaller so the
/// card's own padding is still what reads as the tighter one.
pub const COMPACT_CARD_ICON: f32 = 40.0;

/// Subscription id of the one-shot instance scan.
pub const LOAD_ID: &str = "palantirmc-instances";
/// Subscription id of the one-shot metadata catalog load.
pub const CATALOG_ID: &str = "palantirmc-catalog";
/// Subscription id of a Browse page search.
pub const BROWSE_ID: &str = "palantirmc-browse";
/// Subscription id of a Create-dialog search.
pub const CREATE_SEARCH_ID: &str = "palantirmc-create-search";
/// Subscription id of a Modrinth download.
pub const INSTALL_ID: &str = "palantirmc-install";
/// Subscription id of a launcher scan.
pub const IMPORT_ID: &str = "palantirmc-import";
/// Subscription id of the screenshot scan and thumbnail pass.
pub const SHOTS_ID: &str = "palantirmc-screenshots";
/// Subscription id of the page's scroll frames.
pub const FRAME_ID: &str = "palantirmc-frame";
/// Subscription id of the Microsoft device-code sign-in flow.
pub const MICROSOFT_ID: &str = "palantirmc-microsoft";

/// Screenshot tiles per row in the grid.
const SHOTS_PER_ROW: usize = 3;

/// Width of one screenshot tile, in pixels.
const SHOT_TILE_WIDTH: f32 = 268.0;

/// Height of a tile's picture box. Screenshots are captured at the game's
/// aspect ratio, and 16:9 is what a modern instance produces; anything else is
/// letterboxed inside the box rather than stretched.
const SHOT_TILE_HEIGHT: f32 = SHOT_TILE_WIDTH * 9.0 / 16.0;

/// Height of a Create-dialog body, per step (the dialog sits in a scrollable
/// so long version lists and search results cannot push the footer offscreen).
const CREATE_BODY_CHOOSE: f32 = 396.0;
const CREATE_BODY_CONFIGURE: f32 = 440.0;

/// Dialog width and the ceiling on its total height.
///
/// The body scrolls, so the dialog never needs to be taller than this; keeping
/// it comfortably under the window's 640px minimum (title bar and status bar
/// included) is what lets it stay centred instead of being clipped at the top.
const DIALOG_WIDTH: f32 = 600.0;
const DIALOG_MAX_HEIGHT: f32 = 560.0;

// The Settings dialog's own measurements — its width, the section list's width,
// the pane's height and the color-theme card — live in [`crate::settings`],
// beside the panes that use them.

/// Label of the ungrouped group.
pub const UNGROUPED_LABEL: &str = instances::UNGROUPED_LABEL;

/// Main-area pages, in rail order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    /// Instance grid / welcome hero.
    #[default]
    Home,
    /// Modrinth search + install.
    Browse,
    /// Per-instance mod management.
    Mods,
    /// Per-instance worlds.
    Worlds,
    /// Screenshots taken in game, newest first.
    Screenshots,
    /// Launch log.
    Logs,
    /// Per-instance settings.
    Settings,
    /// Accounts.
    Accounts,
    /// Version, paths, shortcuts, credits.
    About,
}

impl Page {
    /// Human-readable title (also used as the page heading).
    pub fn title(self) -> &'static str {
        match self {
            Page::Home => "Home",
            Page::Browse => "Browse",
            Page::Mods => "Mods",
            Page::Worlds => "Worlds",
            Page::Screenshots => "Screenshots",
            Page::Logs => "Logs",
            Page::Settings => "Settings",
            Page::Accounts => "Accounts",
            Page::About => "About",
        }
    }

    /// What the rail entry says when the pointer rests on it.
    ///
    /// Separate from [`Page::title`] on purpose: the page heading stays a noun
    /// (`Browse`) while the hover label can describe the destination the way the
    /// reference does (`Discover content`).
    pub fn tooltip(self) -> &'static str {
        match self {
            Page::Home => "Home",
            Page::Browse => "Discover content",
            Page::Mods => "Mods",
            Page::Worlds => "Worlds",
            Page::Screenshots => "Screenshots",
            Page::Logs => "Logs",
            Page::Settings => "Settings",
            Page::Accounts => "Accounts",
            Page::About => "About",
        }
    }

    /// Rail glyph name (`crate::glyphs` keys).
    pub fn icon(self) -> &'static str {
        match self {
            Page::Home => "play",
            Page::Browse => "compass",
            Page::Mods => "cube",
            Page::Worlds => "globe",
            Page::Screenshots => "image",
            Page::Logs => "terminal",
            Page::Settings => "gear",
            Page::Accounts => "person",
            Page::About => "info",
        }
    }

    /// Pages that get a rail entry, in order.
    pub fn rail() -> [Page; 6] {
        [
            Page::Home,
            Page::Browse,
            Page::Mods,
            Page::Worlds,
            Page::Screenshots,
            Page::Logs,
        ]
    }

    /// Every page.
    pub fn all() -> [Page; 9] {
        [
            Page::Home,
            Page::Browse,
            Page::Mods,
            Page::Worlds,
            Page::Screenshots,
            Page::Logs,
            Page::Settings,
            Page::Accounts,
            Page::About,
        ]
    }
}

/// Which loader build the dialog should use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BuildChoice {
    /// The build the loader marks recommended (else the newest).
    #[default]
    Stable,
    /// The newest published build.
    Latest,
    /// Pick from a dropdown.
    Other,
}

impl BuildChoice {
    /// Label on the chip.
    pub fn label(self) -> &'static str {
        match self {
            BuildChoice::Stable => "Stable",
            BuildChoice::Latest => "Latest",
            BuildChoice::Other => "Other",
        }
    }
}

/// Which half of the Create dialog is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CreateStep {
    /// "Already know what you want to play?" + the four instance types.
    #[default]
    Choose,
    /// Icon, name, loader, versions.
    Configure,
}

/// The modal dialog, if any.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Modal {
    /// No dialog.
    #[default]
    None,
    /// Create instance.
    Create,
    /// Import from another launcher.
    Import,
    /// The launcher's own settings (Appearance and what it can change).
    Settings,
    /// Delete confirmation (carries the instance id).
    ConfirmDelete(String),
    /// Microsoft device-code sign-in (the code and its progress live in
    /// [`PrismApp::microsoft`]).
    Microsoft,
}

impl Modal {
    /// Whether a dialog is open (the content area is replaced while it is).
    pub fn is_open(&self) -> bool {
        !matches!(self, Modal::None)
    }
}

/// The Create dialog's form state.
#[derive(Debug, Clone)]
pub struct CreateForm {
    /// Which step is showing.
    pub step: CreateStep,
    /// Step-1 search text.
    pub query: String,
    /// Search in flight.
    pub searching: bool,
    /// Search results.
    pub results: Vec<Hit>,
    /// Search failure, if any.
    pub search_error: Option<String>,
    /// Search sequence (stale results are dropped).
    pub seq: u64,
    /// Instance name.
    pub name: String,
    /// Whether the user typed a name (stops the auto-generated one).
    pub name_edited: bool,
    /// Chosen loader.
    pub loader: LoaderKind,
    /// Chosen game version.
    pub game: String,
    /// Offer snapshots in the game-version pick list.
    pub show_snapshots: bool,
    /// Stable / Latest / Other.
    pub build_choice: BuildChoice,
    /// The concrete loader build.
    pub build: String,
    /// `iconKey` for a built-in icon.
    pub icon_key: String,
    /// A dropped/uploaded PNG to install as a custom icon.
    pub icon_source: Option<PathBuf>,
    /// The built-in icon picker is expanded.
    pub customize_open: bool,
    /// Waiting for a dropped PNG.
    pub awaiting_upload: bool,
    /// Project to install into the new instance right after creating it.
    pub install_after: Option<(String, String)>,
    /// Validation/submit failure to show inside the dialog.
    pub error: Option<String>,
}

impl Default for CreateForm {
    fn default() -> Self {
        CreateForm {
            step: CreateStep::Choose,
            query: String::new(),
            searching: false,
            results: Vec::new(),
            search_error: None,
            seq: 0,
            name: String::new(),
            name_edited: false,
            loader: LoaderKind::default(),
            game: String::new(),
            show_snapshots: false,
            build_choice: BuildChoice::default(),
            build: String::new(),
            icon_key: String::new(),
            icon_source: None,
            customize_open: false,
            awaiting_upload: false,
            install_after: None,
            error: None,
        }
    }
}

impl CreateForm {
    /// The name to use, falling back to "<Loader> <game>".
    pub fn effective_name(&self) -> String {
        let typed = self.name.trim();
        if !typed.is_empty() {
            return typed.to_string();
        }
        if self.loader == LoaderKind::Vanilla {
            self.game.trim().to_string()
        } else {
            format!("{} {}", self.loader.label(), self.game.trim())
        }
    }
}

/// Metadata-catalog load state.
#[derive(Debug, Clone, Default)]
pub struct CatalogState {
    /// Load in flight.
    pub loading: bool,
    /// A load has finished (successfully or not).
    pub loaded_once: bool,
    /// The catalog itself.
    pub catalog: VersionCatalog,
    /// Failure summary (empty catalogs report here too).
    pub error: Option<String>,
}

/// Browse page state.
#[derive(Debug, Clone, Default)]
pub struct BrowseState {
    /// Search box contents.
    pub query: String,
    /// Search in flight.
    pub loading: bool,
    /// Modrinth project type currently being browsed.
    pub content_type: ContentType,
    /// Request sequence.
    pub seq: u64,
    /// Results.
    pub hits: Vec<Hit>,
    /// Failure, if any.
    pub error: Option<String>,
    /// Project currently being installed (+ its title).
    pub installing: Option<(String, String)>,
    /// Outcome of the last install.
    pub last_result: Option<String>,
}

/// Import dialog state.
#[derive(Debug, Clone, Default)]
pub struct ImportState {
    /// Scan in flight.
    pub loading: bool,
    /// A scan has run.
    pub scanned: bool,
    /// Discovered instances.
    pub candidates: Vec<instances::ImportCandidate>,
    /// Failure, if any.
    pub error: Option<String>,
}

/// State of the Microsoft device-code sign-in dialog.
///
/// The flow runs in a subscription rather than a one-shot task: polling
/// continues for as long as the user takes to type the code, and the result is a
/// code, a progress line and an outcome. Those are three separate messages, so
/// the state they land in lives here.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MicrosoftState {
    /// A flow is running (this is what the subscription watches).
    pub pending: bool,
    /// Short code the user types into the browser.
    pub user_code: Option<String>,
    /// Where they type it.
    pub verification_uri: Option<String>,
    /// Latest progress line.
    pub status: String,
    /// Terminal failure, if the flow did not sign in.
    pub error: Option<String>,
}

impl MicrosoftState {
    /// Begin a flow, clearing anything the last one left behind.
    pub fn start(&mut self) {
        *self = MicrosoftState {
            pending: true,
            status: "Asking Microsoft for a code…".to_string(),
            ..MicrosoftState::default()
        };
    }

    /// Whether a code has arrived and is waiting to be typed.
    pub fn has_code(&self) -> bool {
        self.user_code.is_some()
    }
}

/// Result of a background job, delivered as [`Message::TaskDone`].
#[derive(Debug, Clone)]
pub enum Task {
    /// Instance scan finished.
    Instances(Box<LoadedInstances>),
    /// Metadata catalog finished.
    Catalog(Box<VersionCatalog>),
    /// Browse search finished (sequence, result).
    BrowseSearch {
        /// Request sequence.
        seq: u64,
        /// Hits or a failure message.
        result: Result<Vec<Hit>, String>,
    },
    /// Create-dialog search finished.
    CreateSearch {
        /// Request sequence.
        seq: u64,
        /// Hits or a failure message.
        result: Result<Vec<Hit>, String>,
    },
    /// Mod download finished.
    Installed {
        /// Request sequence.
        seq: u64,
        /// Project title.
        title: String,
        /// Outcome line or failure.
        result: Result<String, String>,
    },
    /// Launcher scan finished.
    ImportScan(Vec<instances::ImportCandidate>),
    /// Screenshot scan and thumbnail pass finished.
    ShotsLoaded(Vec<ShotTile>),
}

/// Which patch of the title bar a pointer event landed on.
///
/// The patches are separate widgets with separate local coordinate frames, so
/// a drag has to remember which one it started in: a move reported for a
/// different patch is measured against the wrong origin and would look like a
/// jump.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarArea {
    /// Logo, product name and version.
    Brand,
    /// The empty stretch between the search box and the run chip.
    Middle,
    /// The "running instance" chip.
    RunChip,
}

/// Every interaction the GUI can produce.
#[derive(Debug, Clone)]
pub enum Message {
    /// Switch pages.
    PageSelected(Page),
    /// Re-scan instances.
    Refresh,
    /// Look for screenshots again.
    RefreshScreenshots,
    /// A wheel event over the page's content, with the geometry it happened in.
    ///
    /// Published by `scroll::guard`, which takes the wheel away from iced's
    /// scrollable so the offset can be eased toward a target instead of jumped.
    PageWheel(scroll::Wheel),
    /// One frame of the page's scroll tween.
    PageScrollTick,
    /// The page's viewport moved by something other than the wheel: a dragged
    /// scrollbar, a touch, a key.
    ///
    /// Carries the numbers rather than iced's `Viewport`, so the rule for
    /// adopting a foreign scroll can be tested without a window.
    PageScrolled {
        /// The offset the content is now drawn at.
        offset: f32,
        /// Full height of the content.
        content_height: f32,
        /// Height of the visible part.
        view_height: f32,
    },
    // ---- the Settings dialog ----
    /// Show a different pane of the Settings dialog.
    OpenSettingsTab(settings::Tab),
    /// A settings switch was released. The value is flipped from the settings
    /// file rather than carried in the message, so a click and a keyboard
    /// activation cannot disagree about what the switch was.
    ToggleFlag(settings::Flag),
    /// A settings text field was typed into.
    SettingsDraft(settings::Field, String),
    /// The pointer arrived on a switch.
    SwitchHover(&'static str),
    /// The pointer left a switch.
    SwitchLeft(&'static str),
    /// A switch was pressed, which shrinks its knob until the release.
    SwitchDown(&'static str),
    /// The Feature flags search box changed.
    FlagFilterChanged(String),
    /// The version in the Settings footer was clicked. Six of these toggle
    /// developer mode, which is how the reference reveals its hidden tab.
    SettingsFooterPressed,
    /// One frame of the dialog pane's own scroll tween.
    SettingsWheel(scroll::Wheel),
    /// The dialog pane's viewport moved by something other than the wheel.
    SettingsScrolled {
        offset: f32,
        content_height: f32,
        view_height: f32,
    },
    /// Reveal the data root in the file manager.
    OpenDataRoot,
    /// Delete this launcher's cached component metadata.
    PurgeCache,
    /// Global search box.
    SearchChanged(String),
    /// Select an instance by id.
    SelectInstance(String),
    /// Select and launch an instance.
    PlayInstance(String),
    /// Kill the running child.
    KillPressed,
    /// Reveal an instance folder.
    OpenFolder(String),
    /// Duplicate an instance folder.
    DuplicateInstance(String),
    /// Ask to delete an instance (opens the confirmation dialog).
    AskDelete(String),
    /// Confirm the pending delete.
    ConfirmDelete,
    /// Open the instance settings page for an instance.
    EditInstance(String),
    /// Move the selection to a group (`Ungrouped` clears it).
    GroupSelected(String),
    /// Reveal the instances folder.
    OpenInstancesFolder,

    // ---- window controls (undecorated window) ----
    /// Minimize.
    WindowMinimize,
    /// Toggle maximize.
    WindowMaximize,
    /// Close.
    WindowClose,
    /// Grab an edge of the window frame and start a native resize loop.
    ResizeStart(ResizeEdge),
    /// A non-interactive patch of the title bar was pressed. This arms a drag
    /// and detects a double-click, but deliberately does not move the window
    /// yet — see [`Message::BarCursorMoved`].
    BarPressed(BarArea),
    /// The pointer moved over a grabbable patch of the title bar.
    ///
    /// A drag begins here rather than on the press, once the pointer has moved
    /// [`BAR_DRAG_THRESHOLD`]. Starting it on the press would hand the mouse to
    /// Windows' modal move loop before the button came back up, and that loop
    /// eats the second press of a double-click.
    BarCursorMoved(BarArea, Point),
    /// The left button came back up over a grabbable patch: stand the drag down.
    BarReleased(BarArea),
    /// Right-click on the title bar: show the native window menu.
    BarRightClick,
    /// The window reported whether it is maximized (drives the caption glyph).
    MaximizedChanged(bool),
    /// The window's own state changed — the maximize control's hover, or
    /// whether the window is maximized. See [`PrismApp::window_state`].
    WindowStateChanged,

    // ---- dialogs ----
    /// Open the Create dialog.
    OpenCreate,
    /// Open the Import dialog (and scan).
    OpenImport,
    /// Open the launcher's own settings dialog.
    OpenSettings,
    /// Pick a color theme (applies immediately and is remembered).
    SetColorTheme(ColorTheme),
    /// Close whatever dialog is open.
    CloseModal,
    /// Create step 1 → step 2 (custom setup).
    CreateCustomSetup,
    /// Create dialog search text.
    CreateSearchChanged(String),
    /// Jump straight to the search results.
    CreateFocusSearch,
    /// A search hit was chosen (project ref, title).
    CreateProjectPicked(String, String),
    /// Back to step 1.
    CreateBack,
    /// Name text.
    CreateNameChanged(String),
    /// Loader chip.
    CreateLoaderPicked(LoaderKind),
    /// Game version picked.
    CreateGamePicked(String),
    /// Snapshots toggle.
    CreateSnapshotsToggled(bool),
    /// Stable/Latest/Other chip.
    CreateBuildChoicePicked(BuildChoice),
    /// A specific build from the "Other" dropdown.
    CreateBuildPicked(String),
    /// Pick a random built-in icon.
    CreateIconRandomize,
    /// Toggle the built-in icon picker.
    CreateIconCustomize,
    /// Choose a built-in icon.
    CreateIconPicked(String),
    /// Wait for a dropped PNG to use as the icon.
    CreateIconUpload,
    /// Wait for a dropped `.mrpack`/`.zip` to build the instance from.
    CreateAwaitPack,
    /// Submit the dialog.
    CreateSubmit,
    /// Import one candidate (index into the scan list).
    ImportPicked(usize),
    /// Re-fetch the version metadata (dialog retry, Ctrl+R companion).
    ReloadCatalog,

    // ---- content ----
    /// Enable/disable a mod file.
    ModToggled(String, bool),
    /// Reveal the instance's mods folder.
    OpenModsFolder,
    /// Reveal the instance's saves folder.
    OpenWorldsFolder,
    /// Go to Browse and search the given text.
    SearchModrinth(String),

    // ---- browse ----
    /// Browse search box.
    BrowseQueryChanged(String),
    /// Switch between Modrinth mods/resource packs/data packs/shaders/modpacks.
    BrowseTypePicked(ContentType),
    /// Run the browse search.
    BrowseSubmitted,
    /// Install a project into the selected instance (project ref, title).
    BrowseInstall(String, String),

    // ---- accounts ----
    /// New offline account name.
    AccountNameChanged(String),
    /// Add the typed account.
    AccountAdd,
    /// Select an account.
    AccountSelect(String),
    /// Remove an account.
    AccountRemove(String),
    /// Start the Microsoft device-code sign-in.
    MicrosoftPressed,
    /// The device-code flow produced the code the user has to type.
    MicrosoftCode {
        /// Short code shown to the user.
        user_code: String,
        /// URL they enter it at.
        verification_uri: String,
    },
    /// A progress line from the device-code flow.
    MicrosoftStatus(String),
    /// The device-code flow finished (signed in, or with the reason it did not).
    MicrosoftDone(Box<Result<AccountEntry, String>>),
    /// Copy the shown code to the clipboard.
    MicrosoftCopyCode,
    /// Open the verification URL in the browser.
    MicrosoftOpenUrl,
    /// Stop waiting for the device-code flow.
    MicrosoftCancel,
    /// Tokens a launch renewal produced, to be stored.
    AccountTokens {
        /// Profile uuid the tokens belong to.
        uuid: String,
        /// Profile name.
        name: String,
        /// Fresh game access token.
        access_token: String,
        /// Fresh Microsoft refresh token.
        refresh_token: Option<String>,
        /// Unix milliseconds at which the access token expires.
        expires_at_ms: i64,
    },

    // ---- logs ----
    /// Clear the log buffer.
    ConsoleClear,
    /// Toggle log autoscroll.
    ConsoleAutoscrollToggled(bool),

    // ---- launch streaming ----
    /// One streamed batch.
    LaunchLog {
        /// Run id.
        run_id: u64,
        /// Lines.
        lines: Vec<String>,
    },
    /// The run finished.
    LaunchDone {
        /// Run id.
        run_id: u64,
        /// Final line.
        note: String,
    },

    // ---- settings form ----
    /// Display name field.
    SetName(String),
    /// `MinMemAlloc` field.
    SetMinMem(String),
    /// `MaxMemAlloc` field.
    SetMaxMem(String),
    /// `OverrideMemory` gate.
    SetOverrideMemory(bool),
    /// `JavaPath` field.
    SetJavaPath(String),
    /// `OverrideJavaLocation` gate.
    SetOverrideJava(bool),
    /// `MinecraftWinWidth` field.
    SetWinWidth(String),
    /// `MinecraftWinHeight` field.
    SetWinHeight(String),
    /// `OverrideWindow` gate.
    SetOverrideWindow(bool),
    /// `JoinServerOnLaunch` gate.
    SetJoinServer(bool),
    /// `JoinServerOnLaunchAddress` field.
    SetServerAddress(String),
    /// Persist the form.
    SettingsSave,

    // ---- background results ----
    /// A worker finished.
    TaskDone(Box<Task>),
    /// A file was dropped on the window.
    FileDropped(PathBuf),
}

/// Editable snapshot of the per-instance settings form.
#[derive(Debug, Clone, Default)]
pub struct SettingsForm {
    /// Display name.
    pub name: String,
    /// `MinMemAlloc`.
    pub min_mem: String,
    /// `MaxMemAlloc`.
    pub max_mem: String,
    /// `OverrideMemory`.
    pub override_memory: bool,
    /// `JavaPath`.
    pub java_path: String,
    /// `OverrideJavaLocation`.
    pub override_java: bool,
    /// `MinecraftWinWidth`.
    pub win_width: String,
    /// `MinecraftWinHeight`.
    pub win_height: String,
    /// `OverrideWindow`.
    pub override_window: bool,
    /// `JoinServerOnLaunch`.
    pub join_server: bool,
    /// `JoinServerOnLaunchAddress`.
    pub server_address: String,
}

/// Read the settings form from an instance's settings.
pub fn load_form(settings: &Settings) -> SettingsForm {
    SettingsForm {
        name: settings.get_str("name", defaults::INSTANCE_NAME),
        min_mem: settings.get_i64("MinMemAlloc", defaults::MIN_MEM_ALLOC).to_string(),
        max_mem: settings.get_i64("MaxMemAlloc", defaults::MAX_MEM_ALLOC).to_string(),
        override_memory: settings.get_bool("OverrideMemory", false),
        java_path: settings.get_str("JavaPath", ""),
        override_java: settings.get_bool("OverrideJavaLocation", false),
        win_width: settings.get_i64("MinecraftWinWidth", defaults::MC_WIN_WIDTH).to_string(),
        win_height: settings.get_i64("MinecraftWinHeight", defaults::MC_WIN_HEIGHT).to_string(),
        override_window: settings.get_bool("OverrideWindow", false),
        join_server: settings.get_bool("JoinServerOnLaunch", false),
        server_address: settings.get_str("JoinServerOnLaunchAddress", ""),
    }
}

/// Parse a numeric field, keeping `fallback` for blank/invalid input.
pub fn parse_mem(text: &str, fallback: i64) -> i64 {
    text.trim().parse::<i64>().unwrap_or(fallback)
}

/// How many buffered log lines the view renders.
pub fn console_shown_lines(total: usize) -> usize {
    scroll::visible_log_lines(total).min(CONSOLE_VIEW_LINES)
}


/// Identifier of the log scrollable (for autoscroll snaps).
pub fn console_scroll_id() -> scrollable::Id {
    scrollable::Id::new("palantirmc-console")
}

/// Identifier of the page scrollable.
///
/// One id for every page, because only one page is mounted at a time: the
/// tween drives "the page", and no two pages can be on screen to confuse it.
/// The log page is the exception — see `view_logs`.
pub fn page_scroll_id() -> scrollable::Id {
    scrollable::Id::new("palantirmc-page")
}

/// Built-in instance icons offered by "Randomize" and "Customize".
pub const ICON_CHOICES: [&str; 10] = [
    "grass", "dirt", "creeper", "steve", "tnt", "gear", "chicken_legacy", "enderman_legacy",
    "enderpearl_legacy", "default",
];

/// One screenshot, decoded and shrunk so the renderer uploads it once.
///
/// Holds pixels rather than a path: the whole point of the thumbnail pass is
/// that the original file — 8 MB of pixels for a 1080p capture — is never handed
/// to the renderer at all.
#[derive(Debug, Clone)]
pub struct ShotTile {
    /// Which file it came from, and which instance owns it.
    pub entry: screenshots::Entry,
    /// Thumbnail pixels, already at thumbnail size.
    pub handle: Handle,
    /// Thumbnail width in pixels.
    pub width: u32,
    /// Thumbnail height in pixels.
    pub height: u32,
}

impl ShotTile {
    /// Caption under the tile: the file name and the instance it came from.
    pub fn caption(&self) -> String {
        format!("{} · {}", self.entry.name, self.entry.instance)
    }
}    /// Screenshots page state.
    #[derive(Default)]
    pub struct ShotState {
    /// Whether the background scan is running.
    pub loading: bool,
    /// Loaded tiles, newest first.
    pub tiles: Vec<ShotTile>,
    /// Set once a scan has finished, so the page can tell "none yet" from
    /// "still looking".
    pub scanned: bool,
}

/// The application state (runtime-agnostic; both shells in `main.rs` use it).
pub struct PrismApp {
    paths: PrismPaths,
    instances_dir: PathBuf,
    cards: Vec<InstanceCard>,
    groups: Groups,
    loading: bool,
    search: String,
    selected: Option<String>,
    page: Page,
    /// Where the current page is scrolled, and where the wheel is taking it.
    ///
    /// The shell owns this rather than iced's scrollable, so a notch can be
    /// eased into place instead of applied as an instant 60-pixel jump.
    page_scroll: scroll::ScrollAnim,
    console: VecDeque<String>,
    autoscroll: bool,
    status: String,
    status_is_error: bool,
    modal: Modal,
    create: CreateForm,
    catalog: CatalogState,
    browse: BrowseState,
    import: ImportState,
    accounts: crate::accounts::AccountsStore,
    account_input: String,
    /// State of the Microsoft device-code sign-in dialog.
    microsoft: MicrosoftState,
    form: SettingsForm,
    mods: Vec<ModEntry>,
    worlds: Vec<String>,
    shots: ShotState,
    run_seq: u64,
    install_seq: u64,
    active_run: Option<ActiveRunData>,
    child: ChildSlot,
    /// Whether the window is maximized, so the caption button can offer
    /// Maximize or Restore. Kept in sync by [`Message::MaximizedChanged`] and
    /// by our own toggles.
    maximized: bool,
    /// When the title bar was last pressed, for detecting a double-click.
    last_bar_press: Option<Instant>,
    /// The title-bar patch the current press landed on, while that press is
    /// still waiting to become a window drag.
    bar_armed: Option<BarArea>,
    /// Where the pointer was when the armed patch first saw it move, so the
    /// drag waits for real movement instead of a click's jitter.
    bar_origin: Option<Point>,
    /// This launcher's own settings, cached.
    ///
    /// Read from disk once and written back on every change, rather than read
    /// per frame: the Settings dialog draws twenty switches and several panes,
    /// and a file read behind each of them would be a disk hit per repaint.
    prefs: Prefs,
    /// Which pane the Settings dialog is showing.
    settings_tab: settings::Tab,
    /// Where the dialog's own pane is scrolled.
    ///
    /// Separate from `page_scroll`, because the pane sits *over* a page that is
    /// still there: sharing one offset would open the pane wherever the library
    /// had been left, and leave the library wherever the pane ended up.
    settings_scroll: scroll::ScrollAnim,
    /// Text the user is typing into a settings field, before it parses.
    settings_drafts: std::collections::BTreeMap<settings::Field, String>,
    /// Which switch the pointer is over, and which is held down.
    switch_pointer: settings::Pointer,
    /// Every switch's knob position, and the slides carrying them there.
    switches: anim::SwitchAnim,
    /// Filter text for the Feature flags pane.
    flag_filter: String,
    /// Clicks on the Settings footer's version, of which the sixth toggles
    /// developer mode (the reference counts `> 5`).
    footer_presses: u8,
}

impl PrismApp {
    /// Build against an explicit data root, loading instances synchronously.
    pub(crate) fn with_paths(paths: PrismPaths) -> Self {
        let (accounts, accounts_warn) = crate::accounts::AccountsStore::load_with_report(&paths.accounts_file());
        // Read once and cached: the Settings dialog draws twenty switches and
        // several panes, and a file read behind each of them would be a disk hit
        // per repaint. Putting the stored theme in force is the entry point's
        // job, not a constructor's — see `main`.
        let prefs = prefs::load(&paths);
        let mut app = PrismApp {
            instances_dir: paths.configured_instances_dir(),
            paths,
            cards: Vec::new(),
            groups: Groups::default(),
            loading: false,
            search: String::new(),
            selected: None,
            page: Page::default(),
            shots: ShotState::default(),
            page_scroll: scroll::ScrollAnim::default(),
            console: VecDeque::new(),
            autoscroll: true,
            status: String::new(),
            status_is_error: false,
            modal: Modal::None,
            create: CreateForm::default(),
            catalog: CatalogState::default(),
            browse: BrowseState::default(),
            import: ImportState::default(),
            accounts,
            account_input: String::new(),
            microsoft: MicrosoftState::default(),
            form: SettingsForm::default(),
            mods: Vec::new(),
            worlds: Vec::new(),
            run_seq: 0,
            install_seq: 0,
            active_run: None,
            child: Arc::new(Mutex::new(None)),
            maximized: false,
            last_bar_press: None,
            bar_armed: None,
            bar_origin: None,
            prefs,
            settings_tab: settings::Tab::Appearance,
            settings_scroll: scroll::ScrollAnim::default(),
            settings_drafts: std::collections::BTreeMap::new(),
            switch_pointer: settings::Pointer::default(),
            switches: anim::SwitchAnim::default(),
            flag_filter: String::new(),
            footer_presses: 0,
        };
        if let Some(warn) = accounts_warn {
            app.set_error(warn.clone());
            app.push_console(vec![warn]);
        }
        app.reload_instances();
        app.push_console(vec![format!("{} console — launch output appears here.", brand::APP_NAME)]);
        app
    }

    /// Instant placeholder for the `Application` shell: no disk IO beyond
    /// resolving the data root, so the window paints immediately.
    pub(crate) fn pending(paths: PrismPaths) -> Self {
        let (accounts, _) = crate::accounts::AccountsStore::load_with_report(&paths.accounts_file());
        let prefs = prefs::load(&paths);
        PrismApp {
            instances_dir: paths.configured_instances_dir(),
            paths,
            cards: Vec::new(),
            groups: Groups::default(),
            loading: true,
            search: String::new(),
            selected: None,
            page: Page::default(),
            shots: ShotState::default(),
            page_scroll: scroll::ScrollAnim::default(),
            console: VecDeque::from([format!("Starting {}…", brand::APP_NAME)]),
            autoscroll: true,
            status: "Loading instances…".to_string(),
            status_is_error: false,
            modal: Modal::None,
            create: CreateForm::default(),
            catalog: CatalogState::default(),
            browse: BrowseState::default(),
            import: ImportState::default(),
            accounts,
            account_input: String::new(),
            microsoft: MicrosoftState::default(),
            form: SettingsForm::default(),
            mods: Vec::new(),
            worlds: Vec::new(),
            run_seq: 0,
            install_seq: 0,
            active_run: None,
            child: Arc::new(Mutex::new(None)),
            maximized: false,
            last_bar_press: None,
            bar_armed: None,
            bar_origin: None,
            prefs,
            settings_tab: settings::Tab::Appearance,
            settings_scroll: scroll::ScrollAnim::default(),
            settings_drafts: std::collections::BTreeMap::new(),
            switch_pointer: settings::Pointer::default(),
            switches: anim::SwitchAnim::default(),
            flag_filter: String::new(),
            footer_presses: 0,
        }
    }

    /// Detect the data root like Prism does.
    pub fn new() -> Self {
        PrismApp::with_paths(PrismPaths::detect())
    }

    /// Window title.
    pub fn title(&self) -> String {
        let selected = self.selected_name();
        if self.selected.is_none() {
            brand::window_title()
        } else {
            format!("{} — {}", brand::window_title(), selected)
        }
    }

    // ---- small accessors used by the views and in tests --------------------

    /// Whether console autoscroll is on.
    pub fn autoscroll_enabled(&self) -> bool {
        self.autoscroll
    }

    /// The page's scroll position.
    ///
    /// Test-only: the view reads it directly out of `self`, and nothing outside
    /// this module has a reason to ask.
    #[cfg(test)]
    fn scroll_state(&self) -> scroll::ScrollAnim {
        self.page_scroll
    }

    /// The selected instance's card.
    pub fn selected_card(&self) -> Option<&InstanceCard> {
        let id = self.selected.as_deref()?;
        self.cards.iter().find(|card| card.id == id)
    }

    /// The open dialog.
    pub fn modal(&self) -> &Modal {
        &self.modal
    }

    /// Which renderer this process is drawing with.
    ///
    /// Reported rather than described, and read from the same probe that chose it
    /// (see [`crate::gpu`]), so it is evidence about *this* machine rather than a
    /// claim about GPUs in general. Two rounds of this shell's renderer bug were
    /// diagnosed wrongly by inferring the adapter from a module list; this is
    /// what makes the real answer visible to whoever hits the next one.
    pub fn graphics_summary(&self) -> String {
        crate::gpu::active_backend().to_string()
    }

    /// Which graphics adapter the probe found, and whether it was used.
    pub fn adapter_summary(&self) -> String {
        match crate::gpu::detected() {
            Some(report) => report.summary(),
            None => "not probed".to_string(),
        }
    }

    /// Display name of the selection (id fallback, `none` when empty).
    pub fn selected_name(&self) -> String {
        match self.selected_card() {
            Some(card) => card.name.clone(),
            None => match self.selected.as_deref() {
                Some(id) => id.to_string(),
                None => "none".to_string(),
            },
        }
    }

    /// Label of the account chip / "Playing as" card.
    pub fn account_label(&self) -> String {
        match self.accounts.selected_account() {
            Some(account) => account.username.clone(),
            None => "No account".to_string(),
        }
    }

    /// Sub-label of the account card.
    pub fn account_detail(&self) -> String {
        match self.accounts.selected_account() {
            Some(account) => account.detail(),
            None => "Sign in to play on servers".to_string(),
        }
    }

    /// Group options for the pick list (`Ungrouped` first).
    pub fn group_options(&self) -> Vec<String> {
        let mut options = vec![UNGROUPED_LABEL.to_string()];
        for name in self.groups.names() {
            if name != UNGROUPED_LABEL {
                options.push(name.to_string());
            }
        }
        options
    }

    /// Current group label of the selection.
    pub fn selected_group_label(&self) -> String {
        self.selected_card()
            .and_then(|card| card.group.clone())
            .unwrap_or_else(|| UNGROUPED_LABEL.to_string())
    }

    // ---- status/console plumbing ------------------------------------------

    /// Append lines, enforcing [`CONSOLE_LINE_CAP`].
    pub fn push_console(&mut self, lines: Vec<String>) {
        for line in lines {
            let clean = line.strip_suffix('\r').unwrap_or(&line).to_string();
            self.console.push_back(clean);
        }
        while self.console.len() > CONSOLE_LINE_CAP {
            self.console.pop_front();
        }
    }

    fn set_status(&mut self, message: impl Into<String>) {
        self.status = message.into();
        self.status_is_error = false;
    }

    fn set_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.status = message.clone();
        self.status_is_error = true;
        self.push_console(vec![message]);
    }

    /// Take the active run (the `Sandbox` shell drains it synchronously).
    pub(crate) fn take_active_run(&mut self) -> Option<ActiveRunData> {
        self.active_run.take()
    }

    /// Synchronous dry run for runtimes without subscriptions.
    ///
    /// This shell cannot stream, so it runs the same preparation with an offline
    /// metadata store and an empty fetcher: everything *cached and installed* is
    /// checked for real, and anything that would have to be downloaded is
    /// reported as exactly that instead of being waited for. The live path
    /// (which fetches) is the `Application` shell's worker.
    pub(crate) fn sandbox_drain_launch(&mut self) {
        let run = match self.take_active_run() {
            Some(run) => run,
            None => return,
        };
        let paths = PrismPaths::at(&run.data_root);
        let mut all: Vec<String> = Vec::new();
        let readiness = {
            let mut log = |line: String| all.push(line);
            let auth = prism_net::MicrosoftAuth::with_prism_client_id();
            match launch::prepare_auth(&run.account, &auth, &mut log) {
                Ok(prepared) => {
                    let (mut store, fetcher) = launch::offline_backend(&paths);
                    launch::prepare_launch(
                        &paths,
                        &run.instance_id,
                        &prepared.session,
                        &mut store,
                        &fetcher,
                        &mut log,
                    )
                }
                Err(error) => {
                    log(format!("sign-in failed: {error} — not launching"));
                    launch::LaunchReadiness::Blocked
                }
            }
        };
        let tail = match readiness {
            launch::LaunchReadiness::Ready(_) => {
                "dry run: launch looks runnable (live streaming needs the Application entrypoint)"
                    .to_string()
            }
            launch::LaunchReadiness::Blocked => "dry run: launch blocked (see the log)".to_string(),
        };
        self.set_status(tail.clone());
        all.push(tail);
        self.push_console(all);
    }

    // ---- loading ----------------------------------------------------------

    /// Rescan instances on this thread (startup + user-triggered refresh).
    fn reload_instances(&mut self) {
        let loaded = instances::load(&self.paths);
        self.apply_loaded(loaded);
    }

    /// The `(instance name, folder)` pairs the screenshot scan should look in.
    ///
    /// One entry per instance in the launcher, not just the selected one: the
    /// reference page is a single library view, and a screenshot is something
    /// you want to find again rather than something you file under the pack you
    /// happened to be playing.
    fn screenshot_dirs(&self) -> Vec<(String, PathBuf)> {
        self.cards
            .iter()
            .map(|card| (card.name.clone(), self.instances_dir.join(&card.id)))
            .collect()
    }

    /// Ask for a screenshot scan.
    ///
    /// Existing tiles stay on screen while the pass runs, so re-opening the page
    /// does not flash empty and then fill in.
    fn start_screenshot_scan(&mut self) {
        if self.shots.loading {
            return;
        }
        self.shots.loading = true;
    }

    fn apply_loaded(&mut self, loaded: LoadedInstances) {
        self.loading = false;
        self.cards = loaded.cards;
        self.groups = loaded.groups;
        self.instances_dir = loaded.instances_dir;
        let keep = self
            .selected
            .as_deref()
            .map(|id| self.cards.iter().any(|card| card.id == id))
            .unwrap_or(false);
        if !keep {
            self.selected = loaded.selected;
        }
        self.set_status(loaded.status);
        self.refresh_selection_caches();
    }

    /// Reload everything derived from the selection.
    fn refresh_selection_caches(&mut self) {
        self.mods.clear();
        self.worlds.clear();
        let Some(card) = self.selected_card().cloned() else {
            self.form = SettingsForm::default();
            return;
        };
        let Ok(instance) = Instance::open(&self.instances_dir.join(&card.id)) else {
            self.form = SettingsForm::default();
            return;
        };
        self.mods = list_mods(&instance.mods_dir());
        self.worlds = list_content_names(&instance.game_root().join("saves"));
        self.form = load_form(instance.settings());
    }

    /// Reload just the settings form.
    fn reload_form(&mut self) {
        let Some(id) = self.selected.clone() else {
            self.form = SettingsForm::default();
            return;
        };
        match Instance::open(&self.instances_dir.join(&id)) {
            Ok(instance) => self.form = load_form(instance.settings()),
            Err(_) => self.form = SettingsForm::default(),
        }
    }

    // ---- launching -------------------------------------------------------

    /// Account identity for the next launch (selection or anonymous).
    ///
    /// The whole entry travels, not just the name: a Microsoft account launches
    /// with its tokens, and renewing them is the worker's first step.
    fn launch_account(&self) -> AccountRef {
        match self.accounts.selected_account() {
            Some(account) => AccountRef::from_entry(account),
            None => AccountRef::anonymous(),
        }
    }

    /// Record a run; the subscription spawns the worker.
    fn start_launch(&mut self, id: Option<String>) {
        // An empty id is "nothing selected" — treating it as a real instance
        // would launch `""` and wipe the current selection on the way in.
        if let Some(id) = id.filter(|id| !id.trim().is_empty()) {
            self.selected = Some(id);
            self.refresh_selection_caches();
        }
        let Some(id) = self.selected.clone() else {
            self.set_error("Pick an instance first — press N to create one.");
            return;
        };
        if self.active_run.is_some() {
            self.set_error("A launch is already running — kill it first.");
            return;
        }
        if let Ok(mut guard) = self.child.lock() {
            if let Some(child) = guard.as_mut() {
                let _ = child.kill();
            }
            *guard = None;
        }
        self.run_seq += 1;
        let account = self.launch_account();
        self.active_run = Some(ActiveRunData {
            run_id: self.run_seq,
            instance_id: id.clone(),
            data_root: self.paths.root.clone(),
            account,
        });
        self.page = Page::Logs;
        self.set_status(format!("Starting '{id}'…"));
        self.push_console(vec![format!("launch requested for '{id}'")]);
    }

    /// Minimize the window for a launch that has actually started.
    ///
    /// The check is on `active_run` rather than on the preference alone, so
    /// that a press which could *not* start — no instance selected, a run
    /// already going — does not hide the window the user is still reading. The
    /// message that explains why is on screen behind it.
    fn minimize_for_launch(&self) -> Command<Message> {
        if self.active_run.is_some() && self.prefs.minimize_on_launch {
            window::minimize(window::Id::MAIN, true)
        } else {
            Command::none()
        }
    }

    fn kill_running(&mut self) {
        // The outcome is decided while the child slot is locked and applied
        // afterwards: holding the lock across a `self` mutation would not
        // borrow-check.
        let outcome = match self.child.lock() {
            Ok(mut guard) => match guard.as_mut() {
                Some(child) => match child.kill() {
                    Ok(()) => Ok("Kill requested.".to_string()),
                    Err(error) => Err(format!("kill failed: {error}")),
                },
                None => Ok("Nothing is running.".to_string()),
            },
            Err(_) => Err("internal lock error".to_string()),
        };
        match outcome {
            Ok(message) => self.set_status(message),
            Err(error) => self.set_error(error),
        }
    }

    // ---- instance actions ------------------------------------------------

    fn open_folder(&mut self, id: &str) {
        let path = if id.is_empty() {
            self.instances_dir.clone()
        } else {
            self.instances_dir.join(id)
        };
        match open_in_file_manager(&path) {
            Ok(()) => self.set_status(format!("Opened {}", path.display())),
            Err(error) => self.set_error(error),
        }
    }

    fn duplicate_instance(&mut self, id: &str) {
        let src = self.instances_dir.join(id);
        let new_id = match prism_core::util::unique_dir_name(&self.instances_dir, &format!("{id} Copy"))
        {
            Ok(name) => name,
            Err(error) => return self.set_error(format!("Duplicating '{id}' failed: {error}")),
        };
        let dst = self.instances_dir.join(&new_id);
        if let Err(error) = instances::copy_dir_recursive(&src, &dst) {
            return self.set_error(format!("Duplicating '{id}' failed: {error}"));
        }
        if let Ok(mut copy) = Instance::open(&dst) {
            let renamed = format!("{} Copy", copy.name());
            copy.set_name(&renamed);
            if let Err(error) = copy.save() {
                self.set_error(format!("Copy saved but renaming failed: {error}"));
            }
        }
        // A copy belongs where the original did: leaving it ungrouped would
        // make duplicating an organised instance look like it had been filed
        // away by itself.
        if let Some(group) = self.groups.group_of(id).map(str::to_string) {
            self.groups.set_group(&new_id, Some(&group));
            if let Err(error) = self.groups.save(&self.paths) {
                self.set_error(format!("copied, but saving groups failed: {error}"));
            }
        }
        self.reload_instances();
        self.selected = Some(new_id.clone());
        self.refresh_selection_caches();
        self.set_status(format!("Duplicated '{id}' as '{new_id}'."));
    }

    fn delete_instance(&mut self, id: &str) {
        // The group index is a separate file, so deleting the folder alone
        // leaves a ghost entry behind that reappears the moment an instance is
        // ever created under the same id again.
        let group = self.groups.group_of(id).map(str::to_string);
        match Instance::delete(&self.instances_dir, id) {
            Ok(()) => {
                let mut note = format!("Deleted '{id}'.");
                if group.is_some() {
                    self.groups.set_group(id, None);
                    if let Err(error) = self.groups.save(&self.paths) {
                        note.push_str(&format!(
                            " Its entry in instgroups.json could not be removed: {error}"
                        ));
                    }
                }
                if self.selected.as_deref() == Some(id) {
                    self.selected = None;
                }
                self.modal = Modal::None;
                self.reload_instances();
                self.set_status(note);
            }
            Err(error) => self.set_error(format!("Deleting '{id}' failed: {error}")),
        }
    }

    fn change_group(&mut self, label: &str) {
        let Some(id) = self.selected.clone() else {
            return self.set_error("Pick an instance first.");
        };
        if label == UNGROUPED_LABEL {
            self.groups.set_group(&id, None);
        } else if label.trim().is_empty() {
            return self.set_error("ignoring a blank group name");
        } else {
            self.groups.set_group(&id, Some(label));
        }
        if let Err(error) = self.groups.save(&self.paths) {
            return self.set_error(format!("saving groups failed: {error}"));
        }
        self.reload_instances();
        self.set_status(format!("Moved '{id}' to '{label}'."));
    }

    fn toggle_mod(&mut self, file: &str, enabled: bool) {
        let Some(id) = self.selected.clone() else {
            return self.set_error("Pick an instance first.");
        };
        let dir = match Instance::open(&self.instances_dir.join(&id)) {
            Ok(instance) => instance.mods_dir(),
            Err(error) => return self.set_error(format!("cannot open '{id}': {error}")),
        };
        match set_mod_enabled(&dir, file, enabled) {
            Ok(()) => {
                self.mods = list_mods(&dir);
                let state = if enabled { "Enabled" } else { "Disabled" };
                self.set_status(format!("{state} '{file}'."));
            }
            Err(error) => self.set_error(error),
        }
    }

    fn open_mods_folder(&mut self) {
        let Some(id) = self.selected.clone() else {
            return self.set_error("Pick an instance first.");
        };
        match Instance::open(&self.instances_dir.join(&id)) {
            Ok(instance) => {
                let dir = instance.mods_dir();
                if let Err(error) = prism_core::util::ensure_dir(&dir) {
                    return self.set_error(format!("cannot create {}: {error}", dir.display()));
                }
                match open_in_file_manager(&dir) {
                    Ok(()) => self.set_status(format!("Opened {}", dir.display())),
                    Err(error) => self.set_error(error),
                }
            }
            Err(error) => self.set_error(format!("cannot open '{id}': {error}")),
        }
    }

    fn open_worlds_folder(&mut self) {
        let Some(id) = self.selected.clone() else {
            return self.set_error("Pick an instance first.");
        };
        match Instance::open(&self.instances_dir.join(&id)) {
            Ok(instance) => {
                let dir = instance.game_root().join("saves");
                if let Err(error) = prism_core::util::ensure_dir(&dir) {
                    return self.set_error(format!("cannot create {}: {error}", dir.display()));
                }
                match open_in_file_manager(&dir) {
                    Ok(()) => self.set_status(format!("Opened {}", dir.display())),
                    Err(error) => self.set_error(error),
                }
            }
            Err(error) => self.set_error(format!("cannot open '{id}': {error}")),
        }
    }

    fn save_settings(&mut self) {
        let Some(id) = self.selected.clone() else {
            return self.set_error("Pick an instance first.");
        };
        let mut instance = match Instance::open(&self.instances_dir.join(&id)) {
            Ok(instance) => instance,
            Err(error) => return self.set_error(format!("cannot open '{id}': {error}")),
        };
        if !self.form.name.trim().is_empty() {
            let name = self.form.name.trim().to_string();
            instance.set_name(&name);
        }
        let settings = instance.settings_mut();
        let min_mem = parse_mem(&self.form.min_mem, settings.get_i64("MinMemAlloc", defaults::MIN_MEM_ALLOC));
        let max_mem = parse_mem(&self.form.max_mem, settings.get_i64("MaxMemAlloc", defaults::MAX_MEM_ALLOC));
        settings.set_bool("OverrideMemory", self.form.override_memory);
        settings.set_i64("MinMemAlloc", min_mem);
        settings.set_i64("MaxMemAlloc", max_mem);
        settings.set_str("JavaPath", self.form.java_path.trim());
        settings.set_bool("OverrideJavaLocation", self.form.override_java);
        let width = parse_mem(&self.form.win_width, settings.get_i64("MinecraftWinWidth", defaults::MC_WIN_WIDTH));
        let height = parse_mem(&self.form.win_height, settings.get_i64("MinecraftWinHeight", defaults::MC_WIN_HEIGHT));
        settings.set_bool("OverrideWindow", self.form.override_window);
        settings.set_i64("MinecraftWinWidth", width);
        settings.set_i64("MinecraftWinHeight", height);
        settings.set_bool("JoinServerOnLaunch", self.form.join_server);
        settings.set_str("JoinServerOnLaunchAddress", self.form.server_address.trim());
        if let Err(error) = instance.save() {
            return self.set_error(format!("saving settings failed: {error}"));
        }
        // Prism renames the instance *folder* when its display name changes, and
        // the group index has to follow: leaving the folder behind means two
        // names for one instance, and leaving the group behind means the renamed
        // instance silently falls out of its group. Both are done here so a
        // rename is one atomic-looking action from the user's side.
        let wanted = prism_core::util::sanitize_dir_name(&self.form.name);
        let mut final_id = id.clone();
        if !self.form.name.trim().is_empty() && wanted != id {
            match instance.rename(&self.form.name) {
                Ok(()) => {
                    let new_id = instance.id();
                    let group = self.groups.group_of(&id).map(str::to_string);
                    self.groups.set_group(&id, None);
                    if let Some(group) = group {
                        self.groups.set_group(&new_id, Some(&group));
                    }
                    if let Err(error) = self.groups.save(&self.paths) {
                        self.set_error(format!("renamed, but saving groups failed: {error}"));
                    }
                    final_id = new_id;
                }
                Err(error) => {
                    // The settings are saved; only the folder rename failed.
                    self.set_error(format!("settings saved, but renaming failed: {error}"));
                }
            }
        }
        self.reload_instances();
        self.selected = Some(final_id.clone());
        self.refresh_selection_caches();
        self.set_status(format!("Saved settings for '{final_id}'."));
    }

    // ---- accounts --------------------------------------------------------

    fn add_account(&mut self) {
        let name = self.account_input.trim().to_string();
        match self.accounts.add(&name).and_then(|()| self.accounts.save()) {
            Ok(()) => {
                self.account_input.clear();
                self.set_status(format!("Added offline account '{name}'."));
            }
            Err(error) => self.set_error(error),
        }
    }

    fn select_account(&mut self, uuid: &str) {
        match self.accounts.select(uuid).and_then(|()| self.accounts.save()) {
            Ok(()) => self.set_status(format!("Now playing as {}.", self.account_label())),
            Err(error) => self.set_error(error),
        }
    }

    fn remove_account(&mut self, uuid: &str) {
        if self.accounts.remove(uuid) {
            match self.accounts.save() {
                Ok(()) => self.set_status("Account removed."),
                Err(error) => self.set_error(error),
            }
        } else {
            self.set_error("account not found");
        }
    }

    // ---- launcher settings ------------------------------------------------

    /// Apply a color theme and remember it.
    ///
    /// The theme is process-wide ([`theme::set_color_theme`]) because the widget
    /// styles read it while painting, and the write to the preferences file is
    /// what makes the choice survive a restart. A write that fails is reported
    /// but does not undo the choice: the window should still look the way it was
    /// just asked to.
    fn set_color_theme(&mut self, theme: ColorTheme) {
        theme::set_color_theme(theme);
        // The cached copy is what carries every other preference, so the theme
        // is written through *it* rather than to the file behind its back — a
        // separate write of one key would be undone by the next switch, which
        // saves the whole struct.
        self.prefs.color_theme = theme.id().to_string();
        match prefs::save(&self.paths, &self.prefs) {
            Ok(()) => self.set_status(format!("Color theme: {}", theme.label())),
            Err(error) => self.set_error(format!(
                "Color theme: {} (could not be saved: {error})",
                theme.label()
            )),
        }
    }

    /// Write the preferences, reporting a failure without undoing the change.
    ///
    /// A settings file that cannot be written is worth saying out loud — the
    /// switch would appear to have taken and then be gone at the next launch —
    /// but it is not a reason to refuse the change that was just made.
    fn save_prefs(&mut self) {
        if let Err(error) = prefs::save(&self.paths, &self.prefs) {
            self.set_error(format!("Could not save settings: {error}"));
        }
    }

    // ---- create dialog ---------------------------------------------------

    /// Open the dialog and make sure the catalog is on its way.
    fn open_create(&mut self) {
        self.create = CreateForm::default();
        self.modal = Modal::Create;
        self.ensure_catalog();
    }

    fn ensure_catalog(&mut self) {
        if !self.catalog.loading && !self.catalog.loaded_once {
            self.reload_catalog();
        }
    }

    /// Fetch the version metadata again; the subscription spawns the worker.
    fn reload_catalog(&mut self) {
        if self.catalog.loading {
            return;
        }
        self.catalog.loading = true;
        self.catalog.error = None;
        self.set_status("Loading Minecraft versions…");
    }

    /// Apply catalog-dependent defaults once versions are known.
    ///
    /// With an empty catalog the version stays unset on purpose: inventing a
    /// version the metadata service never confirmed would create an instance
    /// that cannot be installed. The dialog says so and offers a retry instead.
    fn prime_create_defaults(&mut self) {
        if self.create.game.trim().is_empty() {
            if let Some(default) = self.catalog.catalog.default_game_version() {
                self.create.game = default;
            }
        }
        self.recompute_build();
    }

    /// Recompute the concrete loader build from loader/game/build-choice.
    fn recompute_build(&mut self) {
        if self.create.loader == LoaderKind::Vanilla {
            self.create.build.clear();
            return;
        }
        let game = self.create.game.clone();
        let loader = self.create.loader;
        let builds = self.catalog.catalog.loader_builds(loader, &game);
        let builds_total = builds.len();
        let available = builds.iter().any(|entry| entry.version == self.create.build);
        let choice = self.create.build_choice;
        let picked = match choice {
            BuildChoice::Stable => self.catalog.catalog.stable_build(loader, &game),
            BuildChoice::Latest => self.catalog.catalog.latest_build(loader, &game),
            BuildChoice::Other => {
                if available {
                    Some(self.create.build.clone())
                } else {
                    builds.first().map(|entry| entry.version.clone())
                }
            }
        };
        self.create.build = picked.unwrap_or_default();
        if matches!(choice, BuildChoice::Other) && self.create.build.is_empty() {
            self.create.build = self
                .catalog
                .catalog
                .other_builds(loader, &game)
                .first()
                .cloned()
                .unwrap_or_default();
        }
        if builds_total == 0 && self.catalog.loaded_once {
            self.create.error = Some(format!(
                "{} has no builds for {} yet — pick another game version or loader.",
                loader.label(),
                if game.is_empty() { "that version" } else { &game }
            ));
        } else if self.create.error.as_deref().map(|e| e.contains("has no builds")).unwrap_or(false) {
            self.create.error = None;
        }
    }

    /// Step-1 → step-2 with the current choices turned into an instance.
    fn create_custom_setup(&mut self) {
        self.create.step = CreateStep::Configure;
        self.prime_create_defaults();
    }

    fn submit_create(&mut self) {
        let name = self.create.effective_name();
        let game = self.create.game.clone();
        let spec = NewInstance {
            loader: self.create.loader,
            loader_build: if self.create.loader.loads_mods() {
                Some(self.create.build.clone())
            } else {
                None
            },
            icon_key: if self.create.icon_source.is_none() && !self.create.icon_key.is_empty() {
                Some(self.create.icon_key.clone())
            } else {
                None
            },
            icon_source: self.create.icon_source.clone(),
            ..NewInstance::vanilla(&name, &game)
        };
        if spec.name.trim().is_empty() {
            self.create.error = Some("Give the instance a name.".to_string());
            return;
        }
        if spec.game.trim().is_empty() {
            self.create.error = Some("Pick a game version.".to_string());
            return;
        }
        match instances::create(&self.paths, &spec) {
            Ok(created) => {
                let mut warnings = created.warnings.clone();
                if let Some((project, title)) = self.create.install_after.clone() {
                    self.start_install(project, title, Some(created.id.clone()));
                }
                self.modal = Modal::None;
                self.create = CreateForm::default();
                self.search.clear();
                self.reload_instances();
                self.selected = Some(created.id.clone());
                self.refresh_selection_caches();
                self.page = Page::Home;
                let mut line = format!("Created '{}'.", created.id);
                if !warnings.is_empty() {
                    line.push_str(&format!(" ({})", warnings.join("; ")));
                }
                self.set_status(line);
                self.push_console(std::mem::take(&mut warnings));
            }
            Err(error) => self.create.error = Some(error),
        }
    }

    fn pick_random_icon(&mut self) {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as usize)
            .unwrap_or(0);
        self.create.icon_key = ICON_CHOICES[seed % ICON_CHOICES.len()].to_string();
        self.create.icon_source = None;
        self.create.awaiting_upload = false;
        self.set_status(format!("Icon set to '{}'.", self.create.icon_key));
    }

    // ---- browse ----------------------------------------------------------

    fn start_browse_search(&mut self) {
        let query = self.browse.query.trim().to_string();
        if query.is_empty() {
            self.browse.hits.clear();
            self.browse.loading = false;
            return;
        }
        self.browse.seq += 1;
        self.browse.loading = true;
        self.browse.error = None;
        self.set_status(format!(
            "Searching Modrinth {} for '{query}'…",
            self.browse.content_type.label()
        ));
    }

    fn start_create_search(&mut self) {
        let query = self.create.query.trim().to_string();
        if query.is_empty() {
            self.create.results.clear();
            self.create.searching = false;
            return;
        }
        self.create.seq += 1;
        self.create.searching = true;
        self.create.search_error = None;
    }

    /// Resolve + download the right file for the selection (or a new instance).
    fn start_install(&mut self, project: String, title: String, for_instance: Option<String>) {
        let target = for_instance.or_else(|| self.selected.clone());
        let Some(id) = target else {
            self.set_error("Pick an instance to install into first.");
            return;
        };
        let card = self.cards.iter().find(|card| card.id == id).cloned();
        let (game, loader) = match card {
            Some(card) => (card.mc_version.clone(), card.loader),
            None => {
                // Freshly created instance: read it back from disk.
                match Instance::open(&self.instances_dir.join(&id)) {
                    Ok(instance) => {
                        let card = instances::summarize(&self.instances_dir, &entry_for(&instance));
                        (card.mc_version, card.loader)
                    }
                    Err(error) => {
                        self.set_error(format!("cannot inspect '{id}': {error}"));
                        return;
                    }
                }
            }
        };
        if self.browse.content_type.needs_loader() && !loader.loads_mods() {
            self.set_error(format!(
                "'{}' is vanilla — install a loader (Fabric/NeoForge/Forge/Quilt) before adding mods.",
                id
            ));
            return;
        }
        if game.trim().is_empty() {
            self.set_error(format!("'{id}' has no Minecraft version recorded; open its settings first."));
            return;
        }
        self.install_seq += 1;
        self.browse.installing = Some((project.clone(), title.clone()));
        self.browse.last_result = None;
        self.set_status(format!("Installing '{title}' into '{id}'…"));
    }

    // ---- import ----------------------------------------------------------

    fn start_import_scan(&mut self) {
        self.import.loading = true;
        self.import.scanned = false;
        self.import.error = None;
        self.import.candidates.clear();
    }

    fn import_candidate(&mut self, index: usize) {
        let Some(candidate) = self.import.candidates.get(index).cloned() else {
            return;
        };
        match instances::import_instance(&self.paths, &candidate.source) {
            Ok(id) => {
                self.import.candidates.remove(index);
                self.modal = Modal::None;
                self.reload_instances();
                self.selected = Some(id.clone());
                self.refresh_selection_caches();
                self.set_status(format!("Imported '{}' from {}.", id, candidate.origin));
            }
            Err(error) => self.import.error = Some(error),
        }
    }

    /// Handle a dropped file: an icon for the open dialog, a pack to import, or
    /// a custom icon for the selected instance.
    fn handle_drop(&mut self, path: PathBuf) {
        let extension = path
            .extension()
            .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        match (self.modal.clone(), extension.as_str()) {
            (Modal::Create, "png") => {
                self.create.icon_source = Some(path.clone());
                self.create.awaiting_upload = false;
                self.set_status(format!("Using '{}' as the instance icon.", path.display()));
            }
            (Modal::Create, "mrpack" | "zip") => match browse::import_pack(&self.paths, &path) {
                Ok(ImportedPack { id, format }) => {
                    self.modal = Modal::None;
                    self.reload_instances();
                    self.selected = Some(id.clone());
                    self.refresh_selection_caches();
                    self.set_status(format!(
                        "Imported '{id}' from a {format}; remote pack files still need downloading."
                    ));
                }
                Err(error) => self.create.error = Some(error),
            },
            (_, "mrpack" | "zip") => match browse::import_pack(&self.paths, &path) {
                Ok(ImportedPack { id, format }) => {
                    self.reload_instances();
                    self.selected = Some(id.clone());
                    self.refresh_selection_caches();
                    self.set_status(format!("Imported '{id}' from a {format}."));
                }
                Err(error) => self.set_error(error),
            },
            (_, "png") => {
                let Some(id) = self.selected.clone() else {
                    return self.set_error("Pick an instance first, then drop a PNG to use it as its icon.");
                };
                match instances::set_instance_icon(&self.paths, &id, &path) {
                    Ok(key) => {
                        self.reload_instances();
                        self.selected = Some(id.clone());
                        self.refresh_selection_caches();
                        self.set_status(format!("Icon '{}' applied to '{id}'.", key));
                    }
                    Err(error) => self.set_error(error),
                }
            }
            (_, other) => self.set_error(format!(
                "Dropped '.{other}' files are not used — drop a PNG (icon) or a .mrpack/.zip (pack)."
            )),
        }
    }

    // ---- background results ---------------------------------------------

    fn apply_task(&mut self, task: Task) {
        match task {
            Task::Instances(loaded) => self.apply_loaded(*loaded),
            Task::Catalog(catalog) => {
                self.catalog.loading = false;
                self.catalog.loaded_once = true;
                let warnings = catalog.warnings.clone();
                let empty = catalog.is_empty();
                self.catalog.error = if empty {
                    Some(
                        "No version metadata could be loaded — check your connection, then retry."
                            .to_string(),
                    )
                } else if !warnings.is_empty() {
                    Some(format!("Some version lists were unavailable: {}", warnings.join("; ")))
                } else if catalog.offline {
                    Some("Using cached version metadata (offline).".to_string())
                } else {
                    None
                };
                let count = catalog.game_version_count();
                let offline = catalog.offline;
                self.catalog.catalog = *catalog;
                if self.modal == Modal::Create {
                    self.prime_create_defaults();
                }
                if empty {
                    self.set_error("Loading Minecraft versions failed.");
                } else if offline {
                    self.set_status(format!("Loaded {count} Minecraft versions from cache."));
                } else {
                    self.set_status(format!("Loaded {count} Minecraft versions."));
                }
            }
            Task::BrowseSearch { seq, result } => {
                if seq != self.browse.seq {
                    return;
                }
                self.browse.loading = false;
                match result {
                    Ok(hits) => {
                        let count = hits.len();
                        self.browse.hits = hits;
                        self.set_status(format!("{count} result(s) from Modrinth."));
                    }
                    Err(error) => {
                        self.browse.hits.clear();
                        self.browse.error = Some(error.clone());
                        self.set_error(error);
                    }
                }
            }
            Task::CreateSearch { seq, result } => {
                if seq != self.create.seq {
                    return;
                }
                self.create.searching = false;
                match result {
                    Ok(hits) => self.create.results = hits,
                    Err(error) => {
                        self.create.results.clear();
                        self.create.search_error = Some(error);
                    }
                }
            }
            Task::Installed { seq, title, result } => {
                if seq != self.install_seq {
                    return;
                }
                self.browse.installing = None;
                match result {
                    Ok(line) => {
                        self.browse.last_result = Some(line.clone());
                        self.set_status(line.clone());
                        self.push_console(vec![line]);
                        self.refresh_selection_caches();
                    }
                    Err(error) => {
                        self.browse.last_result = Some(format!("Failed: {error}"));
                        self.set_error(format!("Installing '{title}' failed: {error}"));
                    }
                }
            }
            Task::ShotsLoaded(tiles) => {
                self.shots.loading = false;
                self.shots.scanned = true;
                self.shots.tiles = tiles;
            }
            Task::ImportScan(candidates) => {
                self.import.loading = false;
                self.import.scanned = true;
                if candidates.is_empty() {
                    self.import.error =
                        Some("No other launcher instances found on this machine.".to_string());
                }
                self.import.candidates = candidates;
            }
        }
    }

    /// Synchronous update. Returns the commands the runtime should run.
    pub fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::PageSelected(page) => {
                self.page = page;
                // Every page starts at its own top: a tween left over from the
                // page you came from would scroll the new one by itself. What
                // `restart` deliberately keeps is the measured frame cost --
                // that belongs to the machine, not the page.
                self.page_scroll.restart();
                if page == Page::Screenshots {
                    self.start_screenshot_scan();
                }
                if page == Page::Settings {
                    self.reload_form();
                }
                if page == Page::Browse && self.browse.hits.is_empty() && !self.browse.query.is_empty()
                {
                    self.start_browse_search();
                }
                Command::none()
            }
            Message::PageWheel(wheel) => {
                self.page_scroll.wheel(wheel, Instant::now());
                Command::none()
            }
            Message::PageScrollTick => {
                // One frame for everything that eases: the page, the Settings
                // pane, and any switch knob still travelling. Each `tick` lands
                // exactly on its target, so the last frame leaves nothing moving
                // and the subscription stands down.
                let now = Instant::now();
                self.switches.tick(now);
                self.page_scroll.tick(now);
                let mut commands = vec![scrollable::scroll_to(
                    page_scroll_id(),
                    scrollable::AbsoluteOffset {
                        x: 0.0,
                        y: self.page_scroll.offset,
                    },
                )];
                if self.settings_scroll.animating() {
                    self.settings_scroll.tick(now);
                    commands.push(scrollable::scroll_to(
                        settings::scroll_id(),
                        scrollable::AbsoluteOffset {
                            x: 0.0,
                            y: self.settings_scroll.offset,
                        },
                    ));
                }
                Command::batch(commands)
            }
            Message::PageScrolled { offset, content_height, view_height } => {
                // A scroll nobody eased. Adopting it — rather than re-applying
                // the tween's own offset — is what keeps the scrollbar drag
                // usable; re-measuring here keeps the clamp honest when a
                // search result list grows or shrinks.
                self.page_scroll.observe(content_height, view_height);
                self.page_scroll.resync(offset);
                Command::none()
            }
            // ---- the Settings dialog -------------------------------------
            Message::OpenSettingsTab(tab) => {
                if self.settings_tab != tab {
                    self.settings_tab = tab;
                    // The pane's scroll belongs to the pane: opening a new one
                    // starts at the top, which is what the reference's
                    // `forceCheck` does after a tab change.
                    self.settings_scroll.restart();
                    // A half-typed field belongs to the pane that was open.
                    // Carrying the draft over would show it again the next time
                    // that pane came up.
                    self.settings_drafts.clear();
                }
                Command::none()
            }
            Message::ToggleFlag(flag) => {
                // The value is flipped from the settings file rather than
                // carried in the message, so what the switch does cannot
                // disagree with what it was showing.
                let value = !flag.get(&self.prefs);
                flag.set(&mut self.prefs, value);
                // Both ends travel: the first click on a switch has no stored
                // position, and the value it just left is what says where the
                // knob was drawn.
                self.switches.set(flag.id(), !value, value, Instant::now());
                self.switch_pointer.pressed = None;
                // A switch can take away the screen it was pressed from: with
                // the Worlds entry hidden, being *on* Worlds would leave a page
                // whose rail button no longer exists. Landing on Home is the
                // one page every configuration keeps.
                if !self.rail_shows(self.page) {
                    self.page = Page::Home;
                }
                self.save_prefs();
                Command::none()
            }
            Message::SettingsDraft(field, text) => {
                // The draft is what is on screen; the setting is what has been
                // written. They differ while a field is empty or half-typed,
                // which is exactly why the field cannot be drawn from the
                // setting: clearing "6" to retype it would put the 6 back under
                // the caret.
                field.commit(&mut self.prefs, &text);
                self.settings_drafts.insert(field, text);
                self.save_prefs();
                Command::none()
            }
            Message::SwitchHover(id) => {
                self.switch_pointer.hovered = settings::Flag::from_id(id);
                Command::none()
            }
            Message::SwitchLeft(id) => {
                let flag = settings::Flag::from_id(id);
                // Only if it is still *this* switch: a leave for one switch can
                // arrive after an enter for another, and clearing blindly would
                // unlight the one the pointer is on.
                if self.switch_pointer.hovered == flag {
                    self.switch_pointer.hovered = None;
                }
                if self.switch_pointer.pressed == flag {
                    self.switch_pointer.pressed = None;
                }
                Command::none()
            }
            Message::SwitchDown(id) => {
                self.switch_pointer.pressed = settings::Flag::from_id(id);
                Command::none()
            }
            Message::FlagFilterChanged(text) => {
                self.flag_filter = text;
                Command::none()
            }
            Message::SettingsFooterPressed => {
                // `> 5`, as the reference counts it. The version is a click
                // target rather than a button, and five clicks is short enough
                // to find by accident, which is why it takes six.
                self.footer_presses = self.footer_presses.saturating_add(1);
                if self.footer_presses > 5 {
                    self.footer_presses = 0;
                    self.prefs.developer_mode = !self.prefs.developer_mode;
                    let enabled = self.prefs.developer_mode;
                    // Hiding the tab while it is the open pane would leave a
                    // pane behind a tab that is no longer in the list.
                    if !enabled && self.settings_tab == settings::Tab::FeatureFlags {
                        self.settings_tab = settings::Tab::Appearance;
                    }
                    self.save_prefs();
                    self.set_status(if enabled {
                        "Developer mode enabled."
                    } else {
                        "Developer mode disabled."
                    });
                }
                Command::none()
            }
            Message::SettingsWheel(wheel) => {
                self.settings_scroll.wheel(wheel, Instant::now());
                Command::none()
            }
            Message::SettingsScrolled { offset, content_height, view_height } => {
                self.settings_scroll.observe(content_height, view_height);
                self.settings_scroll.resync(offset);
                Command::none()
            }
            Message::OpenDataRoot => {
                let dir = match self.prefs.app_directory.as_deref() {
                    Some(moved) if !moved.trim().is_empty() => PathBuf::from(moved),
                    _ => self.paths.root.clone(),
                };
                match launch::open_in_file_manager(&dir) {
                    Ok(()) => Command::none(),
                    Err(error) => {
                        self.set_error(format!("Could not open {}: {error}", dir.display()));
                        Command::none()
                    }
                }
            }
            Message::PurgeCache => {
                // This launcher's own cache, which is what `cache_dir` is in
                // this tree — **not** Prism's `meta/`, which Prism itself reads
                // and which deleting would break for the other launcher.
                let cache = self.paths.meta_dir();
                let result = std::fs::remove_dir_all(&cache);
                match result {
                    Ok(()) => {
                        self.set_status(format!("Cache cleared: {}", cache.display()));
                    }
                    // A cache that was never written is not a failure, and
                    // saying so beats reporting an error for a no-op.
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        self.set_status("Cache was already empty.".to_string());
                    }
                    Err(error) => {
                        self.set_error(format!("Could not clear the cache: {error}"));
                    }
                }
                Command::none()
            }
            Message::Refresh => {
                self.reload_instances();
                Command::none()
            }
            Message::RefreshScreenshots => {
                self.start_screenshot_scan();
                Command::none()
            }
            Message::ReloadCatalog => {
                self.reload_catalog();
                Command::none()
            }
            Message::SearchChanged(query) => {
                self.search = query;
                Command::none()
            }
            Message::SelectInstance(id) => {
                self.selected = Some(id);
                self.refresh_selection_caches();
                Command::none()
            }
            Message::PlayInstance(id) => {
                self.start_launch(Some(id));
                self.minimize_for_launch()
            }
            Message::KillPressed => {
                self.kill_running();
                Command::none()
            }
            Message::OpenFolder(id) => {
                self.open_folder(&id);
                Command::none()
            }
            Message::DuplicateInstance(id) => {
                self.duplicate_instance(&id);
                Command::none()
            }
            Message::AskDelete(id) => {
                self.modal = Modal::ConfirmDelete(id);
                Command::none()
            }
            Message::ConfirmDelete => {
                if let Modal::ConfirmDelete(id) = self.modal.clone() {
                    self.delete_instance(&id);
                }
                Command::none()
            }
            Message::EditInstance(id) => {
                self.selected = Some(id);
                self.refresh_selection_caches();
                self.page = Page::Settings;
                self.reload_form();
                Command::none()
            }
            Message::GroupSelected(label) => {
                self.change_group(&label);
                Command::none()
            }
            Message::OpenInstancesFolder => {
                self.open_folder("");
                Command::none()
            }
            Message::WindowMinimize => window::minimize(window::Id::MAIN, true),
            Message::WindowMaximize => self.toggle_maximized(),
            Message::WindowClose => window::close(window::Id::MAIN),
            Message::ResizeStart(edge) => {
                native::start_resize(edge);
                Command::none()
            }
            Message::BarPressed(area) => {
                // A second press inside the system double-click window means
                // "maximize", not "start dragging again".
                let now = Instant::now();
                let doubled = self
                    .last_bar_press
                    .map(|then| now.duration_since(then) < DOUBLE_CLICK)
                    .unwrap_or(false);
                self.last_bar_press = if doubled { None } else { Some(now) };
                if doubled {
                    // The double-click wins outright, and nothing is left armed,
                    // so the pointer drifting during the second press cannot
                    // turn it into a drag instead.
                    self.disarm_bar();
                    self.toggle_maximized()
                } else {
                    self.bar_armed = Some(area);
                    self.bar_origin = None;
                    Command::none()
                }
            }
            Message::BarCursorMoved(area, position) => {
                if self.bar_armed != Some(area) {
                    // Either no press is waiting, or the pointer is over a
                    // different patch whose coordinates do not compare.
                    Command::none()
                } else {
                    let origin = *self.bar_origin.get_or_insert(position);
                    if origin.distance(position) >= BAR_DRAG_THRESHOLD {
                        self.disarm_bar();
                        window::drag(window::Id::MAIN)
                    } else {
                        Command::none()
                    }
                }
            }
            Message::BarReleased(area) => {
                if self.bar_armed == Some(area) {
                    self.disarm_bar();
                }
                Command::none()
            }
            Message::BarRightClick => window::show_system_menu(window::Id::MAIN),
            Message::MaximizedChanged(maximized) => {
                self.maximized = maximized;
                Command::none()
            }
            Message::WindowStateChanged => {
                // The tracked flag is refreshed, but the caption reads the
                // window itself (see `window_is_maximized`), so this arm's real
                // job is to be a message at all: it is what makes iced rebuild
                // the view, and the rebuild is what shows the hover and the
                // maximize/restore glyph.
                self.maximized = native::window_maximized().unwrap_or(self.maximized);
                Command::none()
            }
            Message::OpenCreate => {
                self.open_create();
                Command::none()
            }
            Message::OpenImport => {
                self.modal = Modal::Import;
                self.start_import_scan();
                Command::none()
            }
            Message::OpenSettings => {
                self.modal = Modal::Settings;
                // The pane's scroll belongs to the pane, so opening the dialog
                // starts it at the top rather than wherever it was left.
                self.settings_scroll.restart();
                Command::none()
            }
            Message::SetColorTheme(theme) => {
                self.set_color_theme(theme);
                Command::none()
            }
            Message::CloseModal => {
                // Closing the sign-in dialog stops the flow with it: `pending`
                // is what keeps the subscription alive, and dropping it is what
                // tells the polling thread to stop.
                if self.microsoft.pending {
                    self.microsoft = MicrosoftState::default();
                }
                self.modal = Modal::None;
                Command::none()
            }
            Message::CreateCustomSetup => {
                self.create_custom_setup();
                Command::none()
            }
            Message::CreateSearchChanged(query) => {
                self.create.query = query;
                self.start_create_search();
                Command::none()
            }
            Message::CreateFocusSearch => {
                if self.create.query.trim().is_empty() {
                    self.create.error = Some("Type what you are looking for first.".to_string());
                }
                Command::none()
            }
            Message::CreateProjectPicked(project, title) => {
                self.create.install_after = Some((project, title.clone()));
                if self.create.name.trim().is_empty() || !self.create.name_edited {
                    self.create.name = title.clone();
                }
                self.create.step = CreateStep::Configure;
                self.prime_create_defaults();
                self.set_status(format!("'{title}' will be installed right after the instance is created."));
                Command::none()
            }
            Message::CreateBack => {
                self.create.step = CreateStep::Choose;
                self.create.error = None;
                Command::none()
            }
            Message::CreateNameChanged(name) => {
                self.create.name = name;
                self.create.name_edited = true;
                Command::none()
            }
            Message::CreateLoaderPicked(loader) => {
                self.create.loader = loader;
                self.create.build_choice = BuildChoice::Stable;
                self.create.error = None;
                self.recompute_build();
                Command::none()
            }
            Message::CreateGamePicked(game) => {
                self.create.game = game;
                self.recompute_build();
                Command::none()
            }
            Message::CreateSnapshotsToggled(on) => {
                self.create.show_snapshots = on;
                Command::none()
            }
            Message::CreateBuildChoicePicked(choice) => {
                self.create.build_choice = choice;
                self.recompute_build();
                Command::none()
            }
            Message::CreateBuildPicked(build) => {
                self.create.build = build;
                Command::none()
            }
            Message::CreateIconRandomize => {
                self.pick_random_icon();
                Command::none()
            }
            Message::CreateIconCustomize => {
                self.create.customize_open = !self.create.customize_open;
                self.create.awaiting_upload = false;
                Command::none()
            }
            Message::CreateIconPicked(key) => {
                self.create.icon_key = key.clone();
                self.create.icon_source = None;
                self.create.customize_open = false;
                self.set_status(format!("Icon set to '{key}'."));
                Command::none()
            }
            Message::CreateIconUpload => {
                self.create.awaiting_upload = true;
                self.create.icon_source = None;
                self.set_status("Drop a PNG anywhere on the window to use it as the icon.");
                Command::none()
            }
            Message::CreateAwaitPack => {
                // Nothing to toggle: the drop handler already routes a
                // `.mrpack`/`.zip` dropped on the open Create dialog into
                // `browse::import_pack`. All that was missing was telling the
                // user that, instead of opening the *icon* picker.
                self.create.awaiting_upload = false;
                self.set_status(
                    "Drop a .mrpack or CurseForge .zip anywhere on this window to create the instance from it.",
                );
                Command::none()
            }
            Message::CreateSubmit => {
                self.submit_create();
                Command::none()
            }
            Message::ImportPicked(index) => {
                self.import_candidate(index);
                Command::none()
            }
            Message::ModToggled(file, enabled) => {
                self.toggle_mod(&file, enabled);
                Command::none()
            }
            Message::OpenModsFolder => {
                self.open_mods_folder();
                Command::none()
            }
            Message::OpenWorldsFolder => {
                self.open_worlds_folder();
                Command::none()
            }
            Message::SearchModrinth(query) => {
                self.page = Page::Browse;
                self.browse.query = query;
                self.start_browse_search();
                Command::none()
            }
            Message::BrowseQueryChanged(query) => {
                self.browse.query = query;
                self.browse.seq += 1;
                self.browse.loading = !self.browse.query.trim().is_empty();
                self.browse.error = None;
                Command::none()
            }
            Message::BrowseTypePicked(content_type) => {
                self.browse.content_type = content_type;
                self.browse.hits.clear();
                self.browse.error = None;
                if !self.browse.query.trim().is_empty() {
                    self.start_browse_search();
                }
                Command::none()
            }
            Message::BrowseSubmitted => {
                self.start_browse_search();
                Command::none()
            }
            Message::BrowseInstall(project, title) => {
                self.start_install(project, title, None);
                Command::none()
            }
            Message::AccountNameChanged(value) => {
                self.account_input = value;
                Command::none()
            }
            Message::AccountAdd => {
                self.add_account();
                Command::none()
            }
            Message::AccountSelect(uuid) => {
                self.select_account(&uuid);
                Command::none()
            }
            Message::AccountRemove(uuid) => {
                self.remove_account(&uuid);
                Command::none()
            }
            Message::MicrosoftPressed => {
                self.microsoft.start();
                self.modal = Modal::Microsoft;
                self.set_status("Signing in to Microsoft…");
                Command::none()
            }
            Message::MicrosoftCode { user_code, verification_uri } => {
                self.microsoft.user_code = Some(user_code);
                self.microsoft.verification_uri = Some(verification_uri);
                self.microsoft.status =
                    "Waiting for you to finish signing in…".to_string();
                Command::none()
            }
            Message::MicrosoftStatus(line) => {
                self.microsoft.status = line;
                Command::none()
            }
            Message::MicrosoftDone(result) => {
                self.microsoft.pending = false;
                match *result {
                    Ok(entry) => {
                        let label = entry.username.clone();
                        match self.accounts.upsert_microsoft(entry).and_then(|()| self.accounts.save())
                        {
                            Ok(()) => {
                                self.microsoft = MicrosoftState::default();
                                self.modal = Modal::None;
                                self.set_status(format!("Signed in as {label}."));
                                self.push_console(vec![format!(
                                    "Microsoft sign-in succeeded for '{label}'"
                                )]);
                            }
                            Err(error) => self.microsoft.error = Some(error),
                        }
                    }
                    Err(error) => {
                        // The dialog stays open so the reason is readable next
                        // to the code that produced it.
                        self.microsoft.status.clear();
                        self.microsoft.error = Some(error.clone());
                        self.set_error(error);
                    }
                }
                Command::none()
            }
            Message::MicrosoftCopyCode => match self.microsoft.user_code.clone() {
                Some(code) => {
                    self.set_status(format!("Copied code {code}."));
                    iced::clipboard::write(code)
                }
                None => Command::none(),
            },
            Message::MicrosoftOpenUrl => match self.microsoft.verification_uri.clone() {
                Some(url) => {
                    match launch::open_url(&url) {
                        Ok(()) => self.set_status(format!("Opened {url} in your browser.")),
                        Err(error) => self.set_error(error),
                    }
                    Command::none()
                }
                None => Command::none(),
            },
            Message::MicrosoftCancel => {
                self.microsoft.pending = false;
                self.microsoft.user_code = None;
                self.set_status("Microsoft sign-in cancelled.");
                Command::none()
            }
            Message::AccountTokens {
                uuid,
                name,
                access_token,
                refresh_token,
                expires_at_ms,
            } => {
                // A launch signed in again; keep the store in step so the next
                // launch reuses the token it just paid for.
                match self.accounts.update_tokens(
                    &uuid,
                    &access_token,
                    refresh_token.as_deref(),
                    expires_at_ms,
                ) {
                    Ok(()) => {
                        if let Err(error) = self.accounts.save() {
                            self.set_error(format!("saving the renewed session failed: {error}"));
                        } else {
                            self.push_console(vec![format!(
                                "renewed the Microsoft session for '{name}'"
                            )]);
                        }
                    }
                    Err(error) => self.push_console(vec![format!(
                        "renewed session for '{name}' could not be stored: {error}"
                    )]),
                }
                Command::none()
            }
            Message::ConsoleClear => {
                self.console.clear();
                self.set_status("Log cleared.");
                Command::none()
            }
            Message::ConsoleAutoscrollToggled(on) => {
                self.autoscroll = on;
                Command::none()
            }
            Message::LaunchLog { run_id, lines } => {
                let current = self.active_run.as_ref().map(|run| run.run_id).unwrap_or(0);
                if current == run_id && current != 0 {
                    if let Some(last) = lines.last() {
                        self.set_status(last.clone());
                    }
                    self.push_console(lines);
                }
                Command::none()
            }
            Message::LaunchDone { run_id, note } => {
                let current = self.active_run.as_ref().map(|run| run.run_id).unwrap_or(0);
                if current == run_id && current != 0 {
                    self.active_run = None;
                    self.set_status(note.clone());
                    self.push_console(vec![note]);
                }
                Command::none()
            }
            Message::SetName(value) => {
                self.form.name = value;
                Command::none()
            }
            Message::SetMinMem(value) => {
                self.form.min_mem = value;
                Command::none()
            }
            Message::SetMaxMem(value) => {
                self.form.max_mem = value;
                Command::none()
            }
            Message::SetOverrideMemory(on) => {
                self.form.override_memory = on;
                Command::none()
            }
            Message::SetJavaPath(value) => {
                self.form.java_path = value;
                Command::none()
            }
            Message::SetOverrideJava(on) => {
                self.form.override_java = on;
                Command::none()
            }
            Message::SetWinWidth(value) => {
                self.form.win_width = value;
                Command::none()
            }
            Message::SetWinHeight(value) => {
                self.form.win_height = value;
                Command::none()
            }
            Message::SetOverrideWindow(on) => {
                self.form.override_window = on;
                Command::none()
            }
            Message::SetJoinServer(on) => {
                self.form.join_server = on;
                Command::none()
            }
            Message::SetServerAddress(value) => {
                self.form.server_address = value;
                Command::none()
            }
            Message::SettingsSave => {
                self.save_settings();
                Command::none()
            }
            Message::TaskDone(task) => {
                self.apply_task(*task);
                Command::none()
            }
            Message::FileDropped(path) => {
                self.handle_drop(path);
                Command::none()
            }
        }
    }

    // ---- subscriptions ---------------------------------------------------

    /// Background subscriptions: instance scan, metadata, searches, downloads,
    /// imports and the launch stream.
    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs: Vec<Subscription<Message>> = Vec::new();
        if self.loading {
            let root = self.paths.root.clone();
            subs.push(one_shot(LOAD_ID, 8, move |mut sender| {
                let loaded = instances::load(&PrismPaths::at(root));
                let _ = sender.try_send(Message::TaskDone(Box::new(Task::Instances(Box::new(loaded)))));
            }));
        }
        if self.catalog.loading {
            let meta = self.paths.meta_dir();
            subs.push(one_shot(CATALOG_ID, 8, move |mut sender| {
                let fetched = catalog::fetch(&meta);
                let _ = sender.try_send(Message::TaskDone(Box::new(Task::Catalog(Box::new(fetched)))));
            }));
        }
        if self.browse.loading {
            let (seq, query, content_type) = (
                self.browse.seq,
                self.browse.query.trim().to_string(),
                self.browse.content_type,
            );
            subs.push(one_shot((BROWSE_ID, seq, content_type), 8, move |mut sender| {
                let result = browse::client().and_then(|client| browse::search_typed(&client, &query, content_type));
                let _ = sender.try_send(Message::TaskDone(Box::new(Task::BrowseSearch { seq, result })));
            }));
        }
        if self.create.searching {
            let (seq, query) = (self.create.seq, self.create.query.trim().to_string());
            subs.push(one_shot((CREATE_SEARCH_ID, seq), 8, move |mut sender| {
                let result = browse::client().and_then(|client| browse::search_typed(&client, &query, ContentType::Mods));
                let _ = sender.try_send(Message::TaskDone(Box::new(Task::CreateSearch { seq, result })));
            }));
        }
        if let Some((project, title)) = self.browse.installing.clone() {
            let seq = self.install_seq;
            let paths = self.paths.clone();
            let cards = self.cards.clone();
            let selected = self.selected.clone();
            let content_type = self.browse.content_type;
            subs.push(one_shot((INSTALL_ID, seq, content_type), 8, move |mut sender| {
                let result = install_into(
                    &paths,
                    &cards,
                    selected.as_deref(),
                    &project,
                    &title,
                    content_type,
                );
                let _ = sender
                    .try_send(Message::TaskDone(Box::new(Task::Installed { seq, title, result })));
            }));
        }
        if self.import.loading {
            let paths = self.paths.clone();
            subs.push(one_shot(IMPORT_ID, 8, move |mut sender| {
                let found = instances::find_importable(&paths);
                let _ = sender.try_send(Message::TaskDone(Box::new(Task::ImportScan(found))));
            }));
        }
        if self.shots.loading {
            // A buffer of 4: exactly one message is ever sent, and a failed
            // send has to be non-blocking rather than wedging the worker.
            let dirs = self.screenshot_dirs();
            // Sized from the tile the grid actually draws and the display it is
            // being drawn on, rather than a fixed budget every machine paid.
            // Read here, on the UI thread, because it is a Win32 query.
            let side = screenshots::thumbnail_side(SHOT_TILE_WIDTH, native::system_scale_factor());
            subs.push(one_shot(SHOTS_ID, 4, move |mut sender| {
                let found = screenshots::scan(&dirs);
                let tiles = screenshots::thumbnails(&found, side)
                    .into_iter()
                    .map(|(entry, thumb)| ShotTile {
                        entry,
                        handle: Handle::from_pixels(thumb.width, thumb.height, thumb.pixels),
                        width: thumb.width,
                        height: thumb.height,
                    })
                    .collect();
                let _ = sender.try_send(Message::TaskDone(Box::new(Task::ShotsLoaded(tiles))));
            }));
        }
        if self.page_scroll.animating()
            || self.settings_scroll.animating()
            || self.switches.animating()
        {
            // Only while something is moving. A page that has settled, a pane
            // that has stopped and a switch that has arrived all ask for no
            // frames at all — the reason this is a subscription rather than a
            // timer the app owns, and the reason a switch is animated at all:
            // its slide costs frames for 200 ms after a click and nothing while
            // the dialog sits still.
            subs.push(frame_ticks());
        }
        if self.microsoft.pending {
            // The sign-in flow lives and dies with the dialog: it is asked for
            // only while the dialog is open, and dropping the subscription is
            // what tells the worker thread to stop polling.
            let client_id = prefs::load(&self.paths).microsoft_client_id();
            subs.push(microsoft_sign_in(client_id));
        }
        subs.push(Self::window_state());
        if let Some(run) = self.active_run.clone() {
            let slot = self.child.clone();
            subs.push(iced::subscription::channel(run.run_id, 128, move |sender| async move {
                let params = LaunchParams {
                    data_root: run.data_root.clone(),
                    instance_id: run.instance_id.clone(),
                    account: run.account.clone(),
                    run_id: run.run_id,
                };
                let _ = std::thread::spawn(move || {
                    launch::run_launch_worker(params, slot, sender);
                });
                loop {
                    futures::future::pending::<()>().await;
                }
            }));
        }
        Subscription::batch(subs)
    }

    /// Keyboard shortcuts subscription (`N`, `Esc`, `Ctrl+R`).
    pub fn keyboard(&self) -> Subscription<Message> {
        iced::keyboard::on_key_press(shortcut)
    }

    // ---- views -----------------------------------------------------------

    /// Whole window: title bar, rail, content (or dialog), sidebar, status.
    /// Maximize or restore, keeping [`PrismApp::maximized`] in step so the
    /// caption button can swap between the Maximize and Restore glyphs.
    fn toggle_maximized(&mut self) -> Command<Message> {
        self.maximized = !self.maximized;
        window::toggle_maximize(window::Id::MAIN)
    }

    /// Whether the window is maximized, preferring what the window itself says.
    ///
    /// The caption's maximize control is a non-client region now, so Windows
    /// runs that click and the app never receives a message for it. Maximizing
    /// also arrives by ways the app cannot see at all — Aero Snap, `Win`+`Up`,
    /// the taskbar — and a bar that shows Maximize on a maximized window is
    /// worse than one cheap query per frame. The tracked flag answers only when
    /// there is no window to ask, which is the case in tests and on the first
    /// frames before the shim has found one.
    fn window_is_maximized(&self) -> bool {
        native::window_maximized().unwrap_or(self.maximized)
    }

    /// Watch the window's own state: the maximize control's hover, and whether
    /// the window is maximized.
    ///
    /// Neither can be a widget of ours. The maximize control is answered as
    /// non-client — that is what puts Windows 11's Snap Layouts on it — so no
    /// widget ever sees that pointer; and a window maximizes by paths with no
    /// button in them at all: Aero Snap, the taskbar, `Win`+`Up`, and the
    /// native control itself.
    ///
    /// iced rebuilds a view only when a message arrives, so both need a message
    /// to reach the app, and both change from outside it. Polling for them
    /// would redraw an idle launcher forever; instead the window procedure —
    /// which is told about the pointer by Windows and about maximizing by the
    /// window itself — reports each change down the pipe this subscription
    /// holds open. The cost is zero until something actually changes.
    ///
    /// Kept as the *only* `iced::subscription::run` in the shell on purpose.
    /// iced identifies a `run` recipe by hashing the message type and the id —
    /// and `run` uses `()` for the id — so every `run` subscription in a program
    /// hashes the same, and a second one would be treated as this one rather
    /// than started alongside it. A stream that needs its own identity goes
    /// through `run_with_id`.
    pub fn window_state() -> Subscription<Message> {
        iced::subscription::run(|| {
            let (sender, receiver) = futures::channel::mpsc::unbounded();
            native::watch_window_state(sender);
            // Every report down the pipe is the same message: the state itself
            // is read from the window when the view is built, so there is
            // nothing to carry across but the fact that something changed.
            futures::StreamExt::map(receiver, |()| Message::WindowStateChanged)
        })
    }

    /// Forget a title-bar press that has not (yet) become a window drag.
    fn disarm_bar(&mut self) {
        self.bar_armed = None;
        self.bar_origin = None;
    }

    pub fn view(&self) -> Element<'_, Message> {
        // Keep the window's hit test pointed at the maximize control. The
        // target is derived from constants rather than from the window's size,
        // so this cannot go stale, and re-publishing it is a store behind a
        // lock — cheap enough to repeat on every frame rather than to keep in
        // step by hand.
        native::set_caption_target(caption_target());

        let mut body = row![
            self.view_rail(),
            // The reference's 1px hairline between the rail and the page.
            rail_hairline(),
            self.view_content(),
        ];
        // The panel is *not drawn* rather than drawn at zero width: an empty
        // 320px column would move the page's right edge back and leave the
        // window looking like it had lost its content, which is the opposite of
        // what "hide the sidebar" asks for.
        if !self.prefs.hide_right_sidebar {
            body = body.push(self.view_sidebar());
        }
        let body = body.height(Length::Fill);

        let shell = column![
            self.view_title_bar(),
            // ...and the one under the bar. Both are the separators between the
            // raised chrome and the content, drawn rather than set as borders
            // because iced paints a container's border on every edge.
            hairline(),
            body,
            self.view_status_bar(),
        ]
        .width(Length::Fill)
        .height(Length::Fill);

        // An undecorated window has no frame of its own, so the shell draws one
        // itself: a band of resize grips on every edge and corner, painted in
        // the colour of whatever region borders them so the frame is invisible.
        //
        // These are now the *fallback*. Once the window's hit test answers the
        // edges (see `crate::native`), Windows owns them and a press here never
        // reaches iced at all; the bands are what keeps the window resizable if
        // that shim could not be installed — and they keep the drawn layout
        // exactly as verified, since the region Windows owns is the same strip.
        column![
            row![
                grip(ResizeEdge::NorthWest, Length::Fixed(native::RESIZE_BAND), theme::app_bg),
                grip(ResizeEdge::North, Length::Fill, theme::app_bg),
                grip(ResizeEdge::NorthEast, Length::Fixed(native::RESIZE_BAND), theme::app_bg),
            ]
            .height(Length::Fixed(native::RESIZE_BAND)),
            row![
                // The side bands match the rail and sidebar, which are the same
                // colour as each other, so the seam is invisible.
                grip(ResizeEdge::West, Length::Fixed(native::RESIZE_BAND), theme::rail),
                container(shell).width(Length::Fill).height(Length::Fill),
                grip(ResizeEdge::East, Length::Fixed(native::RESIZE_BAND), theme::rail),
            ]
            .height(Length::Fill),
            row![
                grip(ResizeEdge::SouthWest, Length::Fixed(native::RESIZE_BAND), theme::app_bg),
                grip(ResizeEdge::South, Length::Fill, theme::app_bg),
                grip(ResizeEdge::SouthEast, Length::Fixed(native::RESIZE_BAND), theme::app_bg),
            ]
            .height(Length::Fixed(native::RESIZE_BAND)),
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    /// Undecorated title bar: logo, product name, global search, run chip and
    /// the window controls. The empty area starts an OS window drag.
    fn view_title_bar(&self) -> Element<'_, Message> {
        // Everything in the bar that is not a control is a place to grab the
        // window: the brand cluster, the empty middle, and the run chip. Wrapping
        // each in a mouse area is what turns a title bar you had to aim at into
        // one you can drag from anywhere, and a right-click on any of them opens
        // the native window menu.
        let dragging_area = grabbable(
            BarArea::Brand,
            Length::Fixed(190.0),
            self.bar_armed == Some(BarArea::Brand),
            row![
                Image::new(brand::logo_handle())
                    .width(Length::Fixed(22.0))
                    .height(Length::Fixed(22.0)),
                text(brand::APP_NAME).size(15).font(theme::bold()),
                text(format!("v{}", brand::version())).size(11),
            ]
            .spacing(8)
            .align_items(iced::Alignment::Center),
        );

        let search = container(
            text_input("Search your instances…", &self.search)
                .on_input(Message::SearchChanged)
                .style(theme::Field)
                .padding([8, 10])
                .width(Length::Fill),
        )
        .width(Length::Fixed(300.0));

        // The maximize control is answered as non-client — that is exactly what
        // makes Windows 11 put Snap Layouts on it — so iced never sees the
        // pointer arrive and cannot work out a hover of its own. The window's
        // hit test reports the hover instead, and the button paints the look it
        // would have painted from iced's, unchanged.
        let maximize_hovered = native::maximize_button_hovered();
        let maximized = self.window_is_maximized();

        let bar = row![
            dragging_area,
            search,
            grabbable(
                BarArea::Middle,
                Length::Fill,
                self.bar_armed == Some(BarArea::Middle),
                horizontal_space(),
            ),
            grabbable(
                BarArea::RunChip,
                Length::Shrink,
                self.bar_armed == Some(BarArea::RunChip),
                self.view_run_chip(),
            ),
            button(glyph("refresh", 14.0, theme::text_dim()))
                .on_press(Message::Refresh)
                .style(theme::ghost())
                .padding([5, 9]),
            button(glyph("minimize", CAPTION_GLYPH, theme::text_dim()))
                .on_press(Message::WindowMinimize)
                .style(theme::window_button())
                .padding(CAPTION_PAD),
            button(glyph(
                if maximized { "restore" } else { "maximize" },
                CAPTION_GLYPH,
                if maximize_hovered { theme::text() } else { theme::text_dim() },
            ))
            .on_press(Message::WindowMaximize)
            .style(theme::caption_button(maximize_hovered))
            .padding(CAPTION_PAD),
            // The close glyph stays legible on the red hover fill, where the
            // dimmer idle tint would sink into the background.
            button(glyph("close", CAPTION_GLYPH, theme::text_muted()))
                .on_press(Message::WindowClose)
                .style(theme::close_button())
                .padding(CAPTION_PAD),
        ]
        .spacing(TITLE_BAR_SPACING)
        .padding(title_bar_padding())
        .align_items(iced::Alignment::Center)
        .height(Length::Fixed(TITLE_BAR_HEIGHT));

        // The bar is the raised surface in the reference, not the page's own
        // colour: the bar, the rail and the right panel are one chrome and the
        // content sits inside it. Without this the bar was page-coloured, so the
        // chrome read as three unrelated strips with the page showing through.
        container(bar)
            .style(theme::rail)
            .width(Length::Fill)
            .height(Length::Fixed(TITLE_BAR_HEIGHT))
            .into()
    }

    /// Whether the rail carries an entry for `page`.
    ///
    /// The reference hides its instance tabs (Worlds, Files, Screenshots)
    /// behind Display → Features, and the rail is where this shell keeps that
    /// same set — so the switch has to reach the list the rail draws, not only
    /// the page a click opens. Anything that hides an entry must also make sure
    /// the page is not the one on screen, or the shell would vanish into a
    /// panel it no longer has a way back to; [`Message::ToggleFlag`] handles
    /// that side.
    fn rail_shows(&self, page: Page) -> bool {
        match page {
            Page::Worlds => self.prefs.show_worlds_tab,
            Page::Screenshots => self.prefs.show_screenshots_tab,
            _ => true,
        }
    }

    /// "No instances running" / "Running <name>" chip.
    fn view_run_chip(&self) -> Element<'_, Message> {
        let (color, label) = match self.active_run.as_ref() {
            Some(run) => (theme::accent(), format!("Running {}", run.instance_id)),
            None => (theme::text_dim(), "No instances running".to_string()),
        };
        container(
            row![
                container(text(""))
                    .style(theme::pill(color))
                    .width(Length::Fixed(8.0))
                    .height(Length::Fixed(8.0)),
                text(label).size(12),
            ]
            .spacing(6)
            .align_items(iced::Alignment::Center),
        )
        .style(theme::token_pill)
        .padding([5, 10])
        .into()
    }

    /// Icon rail: pages, then the create button, settings and the account.
    fn view_rail(&self) -> Element<'_, Message> {
        // The reference's rail is `p-[0.5rem] pt-0 gap-[0.25rem]`: 4px between
        // entries and nothing above the first one, so the stack meets the bar and
        // reads as attached to it. The horizontal padding is what is left of the
        // 48px button inside the 58px container.
        let mut rail =
            column![].spacing(4).padding([0, 5]).align_items(iced::Alignment::Center);
        for page in Page::rail().into_iter().filter(|page| self.rail_shows(*page)) {
            rail = rail.push(rail_icon(
                page.icon(),
                page.tooltip(),
                self.page == page,
                Message::PageSelected(page),
            ));
        }
        rail = rail.push(horizontal_rule(1u16));
        rail = rail.push(rail_icon("folder", "Instances folder", false, Message::OpenInstancesFolder));
        rail = rail.push(iced::widget::Space::with_height(Length::Fill));
        rail = rail.push(rail_icon("plus", "Create instance (N)", false, Message::OpenCreate));
        // The gear opens the *launcher's* settings, the way the reference
        // client's does. Per-instance settings stay where they belong: on the
        // instance's own card, through "Edit".
        rail = rail.push(rail_icon(
            "gear",
            "Settings",
            self.modal == Modal::Settings,
            Message::OpenSettings,
        ));
        // The account entry is a drawn glyph too: the player head belongs on
        // the account card, where there is room to read it as an avatar.
        rail = rail.push(rail_icon(
            "person",
            &format!("Playing as {}", self.account_label()),
            self.page == Page::Accounts,
            Message::PageSelected(Page::Accounts),
        ));
        container(rail)
            .style(theme::rail)
            .width(Length::Fixed(RAIL_CONTAINER_WIDTH))
            .height(Length::Fill)
            .into()
    }

    /// Content area: the active page, or the dialog that replaced it.
    fn view_content(&self) -> Element<'_, Message> {
        if self.modal().is_open() {
            return self.view_modal();
        }
        // Two containers, not one, and the outer one is the reason the corner
        // reads at all: iced cuts a radius out of the background the container
        // itself painted, so a panel rounded over a bare window would show the
        // window -- which is the same colour as the panel -- and the notch would
        // vanish. Painting the chrome underneath first is what makes the cut
        // reveal chrome, exactly as `.app-contents` does over the rail.
        container(
            container(self.view_page())
                .style(theme::pane)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(18),
        )
        .style(theme::rail)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn view_page(&self) -> Element<'_, Message> {
        match self.page {
            Page::Home => self.view_home(),
            Page::Browse => self.view_browse(),
            Page::Mods => self.view_mods(),
            Page::Worlds => self.view_worlds(),
            Page::Screenshots => self.view_screenshots(),
            Page::Logs => self.view_logs(),
            Page::Settings => self.view_settings(),
            Page::Accounts => self.view_accounts(),
            Page::About => self.view_about(),
        }
    }

    /// Screenshots: the reference's empty state, or the newest grid.
    ///
    /// The empty state is the whole reason this page exists on a fresh install,
    /// so it says what will appear here rather than showing an empty box — and
    /// it distinguishes "still looking" from "none yet", which otherwise look
    /// identical for the second it takes to scan.
    fn view_screenshots(&self) -> Element<'_, Message> {
        let header = column![
            row![
                text("Screenshots").size(24).font(theme::semibold()),
                if self.shots.tiles.is_empty() {
                    text("").size(12)
                } else {
                    text(format!("{}", self.shots.tiles.len())).size(13)
                },
                horizontal_space(),
                button(
                    row![glyph("refresh", 14.0, theme::text_muted()), text("Refresh").size(12)]
                        .spacing(6),
                )
                .on_press(Message::RefreshScreenshots)
                .style(theme::secondary())
                .padding([6, 12]),
            ]
            .spacing(10)
            .align_items(iced::Alignment::Center),
            horizontal_rule(1u16),
        ]
        .spacing(14);

        if self.shots.tiles.is_empty() {
            let (title, detail) = if self.shots.loading {
                (
                    "Looking for screenshots…",
                    "Reading the screenshots folders of your instances.",
                )
            } else {
                (
                    "No screenshots yet",
                    "Screenshots you take in-game will appear here.",
                )
            };
            let empty = column![
                glyph("image", 70.0, theme::text_dim()),
                text(title).size(17).font(theme::bold()),
                text(detail).size(12).style(iced::theme::Text::Color(theme::text_muted())),
            ]
            .spacing(12)
            .align_items(iced::Alignment::Center);

            return column![
                header,
                container(empty)
                    .style(theme::inset)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x()
                    .center_y(),
            ]
            .spacing(14)
            .into();
        }

        let mut grid = column![].spacing(12);
        for chunk in self.shots.tiles.chunks(SHOTS_PER_ROW) {
            let mut line = row![].spacing(12);
            for tile in chunk {
                line = line.push(shot_tile(tile));
            }
            if chunk.len() < SHOTS_PER_ROW {
                line = line.push(iced::widget::Space::with_width(Length::Fill));
            }
            grid = grid.push(line);
        }

        column![header, page_scroller(grid)].spacing(14).into()
    }

    /// Home: the welcome hero while there are no instances, else the grid.
    fn view_home(&self) -> Element<'_, Message> {
        if self.loading {
            return centered_note("Loading your instances…");
        }
        if self.cards.is_empty() {
            return self.view_hero();
        }
        let needle = self.search.to_lowercase();
        let visible: Vec<&InstanceCard> = self
            .cards
            .iter()
            .filter(|card| {
                needle.is_empty()
                    || card.name.to_lowercase().contains(&needle)
                    || card.id.to_lowercase().contains(&needle)
                    || card.subtitle().to_lowercase().contains(&needle)
            })
            .collect();
        let mut grid = column![
            row![
                text("Instances").size(24).font(theme::semibold()),
                text(format!("{}", self.cards.len())).size(13),
                horizontal_space(),
                button(row![glyph("refresh", 14.0, theme::text_muted()), text("Refresh").size(12)].spacing(6))
                    .on_press(Message::Refresh)
                    .style(theme::secondary())
                    .padding([6, 12]),
                button(text("+ New instance").size(12))
                    .on_press(Message::OpenCreate)
                    .style(theme::primary())
                    .padding([6, 12]),
            ]
            .spacing(10)
            .align_items(iced::Alignment::Center),
            horizontal_rule(1u16),
        ]
        .spacing(14);
        if visible.is_empty() {
            grid = grid.push(centered_note(format!("Nothing matches '{}'.", self.search)));
        }
        for chunk in visible.chunks(CARDS_PER_ROW) {
            let mut line = row![].spacing(12);
            for card in chunk {
                line = line.push(instance_card(
                    card,
                    self.selected.as_deref() == Some(card.id.as_str()),
                    CardChrome::from_prefs(&self.prefs),
                ));
            }
            if chunk.len() < CARDS_PER_ROW {
                line = line.push(iced::widget::Space::with_width(Length::Fill));
                if chunk.len() == 1 {
                    line = line.push(iced::widget::Space::with_width(Length::Fill));
                }
            }
            grid = grid.push(line);
        }
        page_scroller(grid)
    }

    /// Welcome hero (no instances yet).
    fn view_hero(&self) -> Element<'_, Message> {
        let hero = column![
            container(
                Image::new(brand::logo_handle())
                    .width(Length::Fixed(112.0))
                    .height(Length::Fixed(112.0)),
            )
            .style(theme::hero_tile)
            .padding(14),
            text(format!("Welcome to {}", brand::APP_NAME)).size(30).font(theme::bold()),
            text("Ready to start playing?").size(15),
            container(
                button(
                    row![
                        text("+").size(16),
                        text("Create an instance").size(14),
                    ]
                    .spacing(8)
                    .align_items(iced::Alignment::Center),
                )
                .on_press(Message::OpenCreate)
                .style(theme::primary())
                .padding([11, 22]),
            )
            .padding([6, 0]),
            text("Press N to quickly create an instance").size(12),
            container(horizontal_rule(1u16)).padding([14, 0]).width(Length::Fixed(280.0)),
            text("Escaping another launcher?").size(13),
            button(
                row![
                    glyph("folder", 14.0, theme::text_muted()),
                    text("Import from Prism Launcher").size(13),
                ]
                .spacing(8)
                .align_items(iced::Alignment::Center),
            )
            .on_press(Message::OpenImport)
            .style(theme::secondary())
            .padding([9, 16]),
        ]
        .spacing(10)
        .align_items(iced::Alignment::Center);

        container(container(hero).center_x().style(theme::card).padding(28))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
    }

    /// Browse: Modrinth search + one-click install.
    fn view_browse(&self) -> Element<'_, Message> {
        let target = match self.selected_card() {
            Some(card) if self.browse.content_type.needs_loader() && card.has_loader() => {
                format!("{} ({})", card.name, card.subtitle())
            }
            Some(card) if self.browse.content_type.needs_loader() => {
                format!("{} — vanilla, install a loader first", card.name)
            }
            Some(card) => format!("{} ({})", card.name, card.subtitle()),
            None => "no instance selected".to_string(),
        };
        let mut body = column![
            row![
                text(format!("Browse {}", self.browse.content_type.label())).size(24).font(theme::semibold()),
                horizontal_space(),
                text(format!("Installing into: {target}")).size(12),
            ]
            .spacing(10)
            .align_items(iced::Alignment::Center),
            row![
                browse_type_tabs(self.browse.content_type),
            ],
            row![
                text_input("Search Modrinth (e.g. sodium, jei, xaero)", &self.browse.query)
                    .on_input(Message::BrowseQueryChanged)
                    .on_submit(Message::BrowseSubmitted)
                    .style(theme::Field)
                    .padding([10, 12])
                    .width(Length::Fill),
                button(text("Search").size(13))
                    .on_press(Message::BrowseSubmitted)
                    .style(theme::primary())
                    .padding([10, 18]),
            ]
            .spacing(8),
        ]
        .spacing(14);

        if self.browse.loading {
            body = body.push(centered_note("Asking Modrinth…"));
        }
        if let Some(error) = &self.browse.error {
            body = body.push(text(format!("Search failed: {error}")).size(13).style(iced::theme::Text::Color(theme::danger())));
        }
        if let Some(line) = &self.browse.last_result {
            body = body.push(text(line.clone()).size(12));
        }
        if !self.browse.loading && self.browse.hits.is_empty() && self.browse.error.is_none() {
            body = body.push(centered_note(
                "Search Modrinth for content that matches the selected instance's game version.",
            ));
        }
        for hit in &self.browse.hits {
            body = body.push(self.browse_row(hit));
        }
        page_scroller(body)
    }

    fn browse_row(&self, hit: &Hit) -> Element<'static, Message> {
        let installable = self.selected_card().is_some()
            && (!self.browse.content_type.needs_loader()
                || self.selected_card().map(InstanceCard::has_loader).unwrap_or(false));
        let installing = self
            .browse
            .installing
            .as_ref()
            .map(|(project, _)| project == hit.project_ref())
            .unwrap_or(false);
        let project = hit.project_ref().to_string();
        let title = hit.title.clone();
        let mut install_label = "Install".to_string();
        if installing {
            install_label = "Installing…".to_string();
        }
        let install: Button<'static, Message> = if installable && !installing {
            button(text(install_label).size(12))
                .on_press(Message::BrowseInstall(project.clone(), title.clone()))
                .style(theme::primary())
                .padding([8, 16])
        } else {
            button(text(install_label).size(12)).style(theme::secondary()).padding([8, 16])
        };
        container(
            row![
                column![
                    row![
                        text(hit.title.clone()).size(15).font(theme::bold()),
                        chip(hit.project_type.clone(), theme::chip_neutral),
                    ]
                    .spacing(8)
                    .align_items(iced::Alignment::Center),
                    text(hit.byline()).size(11),
                    text(hit.description.clone()).size(12),
                ]
                .spacing(4)
                .width(Length::Fill),
                install,
            ]
            .spacing(12)
            .align_items(iced::Alignment::Center),
        )
        .style(theme::card)
        .padding(12)
        .width(Length::Fill)
        .into()
    }

    /// Mods of the selected instance, with enable/disable toggles.
    fn view_mods(&self) -> Element<'_, Message> {
        let (enabled, total) = self.mods.iter().fold((0, 0), |(enabled, total), entry| {
            (enabled + usize::from(entry.enabled), total + 1)
        });
        let mut body = column![
            row![
                text("Mods").size(24).font(theme::semibold()),
                chip(format!("{enabled}/{total} enabled"), theme::chip),
                horizontal_space(),
                button(text("Find more mods").size(12))
                    .on_press(Message::PageSelected(Page::Browse))
                    .style(theme::secondary())
                    .padding([6, 12]),
                button(text("Open folder").size(12))
                    .on_press(Message::OpenModsFolder)
                    .style(theme::secondary())
                    .padding([6, 12]),
            ]
            .spacing(10)
            .align_items(iced::Alignment::Center),
            horizontal_rule(1u16),
        ]
        .spacing(12);
        if self.selected.is_none() {
            body = body.push(centered_note("Pick an instance to manage its mods."));
        } else if self.mods.is_empty() {
            body = body.push(centered_note(
                "No mods yet — use Browse to install one, or drop a .jar into the mods folder.",
            ));
        }
        for entry in &self.mods {
            let file = entry.file_name.clone();
            body = body.push(
                container(
                    row![
                        container(text(if entry.enabled { "✓" } else { "•" }).size(13))
                            .style(if entry.enabled { theme::chip } else { theme::chip_neutral })
                            .padding([3, 9]),
                        text(entry.file_name.clone()).size(12).width(Length::Fill),
                        checkbox("", entry.enabled)
                            .on_toggle(move |on| Message::ModToggled(file.clone(), on))
                            .style(theme::Tick)
                            .size(18),
                    ]
                    .spacing(10)
                    .align_items(iced::Alignment::Center),
                )
                .style(theme::card)
                .padding(10)
                .width(Length::Fill),
            );
        }
        page_scroller(body)
    }

    /// Worlds of the selected instance.
    fn view_worlds(&self) -> Element<'_, Message> {
        let mut body = column![
            row![
                text("Worlds").size(24).font(theme::semibold()),
                chip(format!("{}", self.worlds.len()), theme::chip_neutral),
                horizontal_space(),
                button(text("Open saves folder").size(12))
                    .on_press(Message::OpenWorldsFolder)
                    .style(theme::secondary())
                    .padding([6, 12]),
            ]
            .spacing(10)
            .align_items(iced::Alignment::Center),
            horizontal_rule(1u16),
        ]
        .spacing(12);
        if self.selected.is_none() {
            body = body.push(centered_note("Pick an instance to see its worlds."));
        } else if self.worlds.is_empty() {
            body = body.push(centered_note("No worlds in this instance yet."));
        }
        for world in &self.worlds {
            body = body.push(
                container(text(world.clone()).size(13))
                    .style(theme::card)
                    .padding(10)
                    .width(Length::Fill),
            );
        }
        page_scroller(body)
    }

    /// Logs: the streamed launch output.
    fn view_logs(&self) -> Element<'_, Message> {
        let total = self.console.len();
        let shown = console_shown_lines(total);
        let skip = total - shown;
        let mut lines = column![].spacing(0);
        for (index, line) in self.console.iter().enumerate() {
            if index >= skip {
                lines = lines.push(text(line.clone()).size(12).font(iced::Font::MONOSPACE));
            }
        }
        // The one scrolling area that keeps iced's own wheel handling: this
        // offset is not the reader's to command — autoscroll snaps it to the
        // end on every line of output — so a tween would spend its frames
        // fighting that snap for the same pixels. The render is capped instead
        // (`scroll::LOG_RENDER_CAP`), which is what makes the wheel cheap here.
        column![
            row![
                text("Logs").size(24).font(theme::semibold()),
                chip(format!("{total} line(s)"), theme::chip_neutral),
                horizontal_space(),
                checkbox("Autoscroll", self.autoscroll)
                    .on_toggle(Message::ConsoleAutoscrollToggled)
                    .style(theme::Tick),
                button(text("Clear").size(12))
                    .on_press(Message::ConsoleClear)
                    .style(theme::secondary())
                    .padding([6, 12]),
                if self.active_run.is_some() {
                    button(text("Kill").size(12))
                        .on_press(Message::KillPressed)
                        .style(theme::destructive())
                        .padding([6, 12])
                } else {
                    button(text("Kill").size(12)).style(theme::secondary()).padding([6, 12])
                },
            ]
            .spacing(10)
            .align_items(iced::Alignment::Center),
            container(
                scrollable(lines)
                    .id(console_scroll_id())
                    .style(iced::theme::Scrollable::custom(theme::Thin))
                    .height(Length::Fill),
            )
            .style(theme::inset)
            .padding(10)
            .width(Length::Fill)
            .height(Length::Fill),
        ]
        .spacing(12)
        .into()
    }

    /// Per-instance settings form.
    fn view_settings(&self) -> Element<'_, Message> {
        if self.selected.is_none() {
            return centered_note("Pick an instance to edit its settings.");
        }
        let card = self.selected_card().cloned();
        let mut body = column![
            row![
                text("Instance settings").size(24).font(theme::semibold()),
                if let Some(card) = &card {
                    chip(card.subtitle(), theme::chip)
                } else {
                    chip("".to_string(), theme::chip_neutral)
                },
                horizontal_space(),
            ]
            .spacing(10)
            .align_items(iced::Alignment::Center),
            horizontal_rule(1u16),
        ]
        .spacing(12);

        body = body.push(section("Display name"));
        body = body.push(
            text_input("Name", &self.form.name)
                .on_input(Message::SetName)
                .style(theme::Field)
                .padding([8, 10])
                .width(Length::Fill),
        );

        body = body.push(section("Memory"));
        body = body.push(
            checkbox(
                "Override memory (use this instance's Min/MaxMemAlloc)",
                self.form.override_memory,
            )
            .style(theme::Tick)
            .on_toggle(Message::SetOverrideMemory),
        );
        body = body.push(
            row![
                text_input("Min MiB", &self.form.min_mem)
                    .on_input(Message::SetMinMem)
                    .style(theme::Field)
                    .padding([8, 10])
                    .width(Length::Fixed(180.0)),
                text_input("Max MiB", &self.form.max_mem)
                    .on_input(Message::SetMaxMem)
                    .style(theme::Field)
                    .padding([8, 10])
                    .width(Length::Fixed(180.0)),
            ]
            .spacing(10),
        );

        body = body.push(section("Java"));
        body = body.push(
            checkbox("Override Java location (use this instance's JavaPath)", self.form.override_java)
                .style(theme::Tick)
                .on_toggle(Message::SetOverrideJava),
        );
        body = body.push(
            text_input("Java path (empty = look on PATH)", &self.form.java_path)
                .on_input(Message::SetJavaPath)
                .style(theme::Field)
                .padding([8, 10])
                .width(Length::Fill),
        );

        body = body.push(section("Window"));
        body = body.push(
            checkbox("Override window size", self.form.override_window)
                .style(theme::Tick)
                .on_toggle(Message::SetOverrideWindow),
        );
        body = body.push(
            row![
                text_input("Width", &self.form.win_width)
                    .on_input(Message::SetWinWidth)
                    .style(theme::Field)
                    .padding([8, 10])
                    .width(Length::Fixed(180.0)),
                text_input("Height", &self.form.win_height)
                    .on_input(Message::SetWinHeight)
                    .style(theme::Field)
                    .padding([8, 10])
                    .width(Length::Fixed(180.0)),
            ]
            .spacing(10),
        );

        body = body.push(section("On launch"));
        body = body.push(
            checkbox("Join a server automatically", self.form.join_server)
                .style(theme::Tick)
                .on_toggle(Message::SetJoinServer),
        );
        body = body.push(
            text_input("host[:port]", &self.form.server_address)
                .on_input(Message::SetServerAddress)
                .style(theme::Field)
                .padding([8, 10])
                .width(Length::Fill),
        );

        body = body.push(
            row![
                button(text("Save").size(13))
                    .on_press(Message::SettingsSave)
                    .style(theme::primary())
                    .padding([9, 20]),
                button(text("Open folder").size(13))
                    .on_press(Message::OpenFolder(self.selected.clone().unwrap_or_default()))
                    .style(theme::secondary())
                    .padding([9, 16]),
                button(text("Duplicate").size(13))
                    .on_press(Message::DuplicateInstance(self.selected.clone().unwrap_or_default()))
                    .style(theme::secondary())
                    .padding([9, 16]),
                button(text("Delete…").size(13))
                    .on_press(Message::AskDelete(self.selected.clone().unwrap_or_default()))
                    .style(theme::destructive())
                    .padding([9, 16]),
            ]
            .spacing(10),
        );
        body = body.push(text(
            "Values apply only while their override gate is on; otherwise the global launcher settings win (Prism semantics).",
        )
        .size(11));

        page_scroller(body)
    }

    /// Accounts page.
    fn view_accounts(&self) -> Element<'_, Message> {
        let signed_in = self.accounts.selected_account().is_some();
        let mut body = column![
            text("Accounts").size(24).font(theme::semibold()),
            text("A Microsoft account is what lets you join online servers; offline accounts play single-player and offline servers. Both are stored in accounts.json next to your instances.")
                .size(12),
            horizontal_rule(1u16),
            self.microsoft_card(),
            text("Offline accounts").size(15).font(theme::bold()),
            row![
                text_input("Username", &self.account_input)
                    .on_input(Message::AccountNameChanged)
                    .on_submit(Message::AccountAdd)
                    .style(theme::Field)
                    .padding([8, 10])
                    .width(Length::Fill),
                button(text("Add account").size(13))
                    .on_press(Message::AccountAdd)
                    .style(theme::primary())
                    .padding([8, 18]),
            ]
            .spacing(8),
        ]
        .spacing(12);
        for account in self.accounts.list() {
            let selected = self.accounts.selected_uuid() == Some(account.uuid.as_str());
            let stale = account.is_microsoft()
                && account.needs_refresh(prism_core::util::now_millis());
            body = body.push(
                container(
                    row![
                        icon_tile("steve", 36.0, false),
                        column![
                            row![
                                text(account.username.clone()).size(15).font(theme::bold()),
                                chip(
                                    account.kind.label().to_string(),
                                    if account.kind.is_online() {
                                        theme::chip
                                    } else {
                                        theme::chip_neutral
                                    },
                                ),
                            ]
                            .spacing(8)
                            .align_items(iced::Alignment::Center),
                            text(account.detail()).size(11),
                            if stale {
                                text("Session expired — it renews on the next launch, or sign in again.")
                                    .size(11)
                            } else {
                                text("").size(11)
                            },
                        ]
                        .spacing(2)
                        .width(Length::Fill),
                        if selected {
                            chip("Playing as".to_string(), theme::chip)
                        } else {
                            chip(String::new(), theme::chip_neutral)
                        },
                        button(text(if stale { "Sign in again" } else { "Use" }).size(12))
                            .on_press(if stale {
                                Message::MicrosoftPressed
                            } else {
                                Message::AccountSelect(account.uuid.clone())
                            })
                            .style(theme::secondary())
                            .padding([6, 12]),
                        button(text("Remove").size(12))
                            .on_press(Message::AccountRemove(account.uuid.clone()))
                            .style(theme::destructive())
                            .padding([6, 12]),
                    ]
                    .spacing(10)
                    .align_items(iced::Alignment::Center),
                )
                .style(theme::card)
                .padding(12)
                .width(Length::Fill),
            );
        }
        if self.accounts.list().is_empty() {
            body = body.push(centered_note(
                if signed_in {
                    "No accounts yet."
                } else {
                    "No accounts yet — sign in with Microsoft above, or add an offline name."
                },
            ));
        }
        page_scroller(body)
    }

    /// The Microsoft card at the top of the Accounts page: what the sign-in is,
    /// what it is doing right now, and the two ways in.
    fn microsoft_card(&self) -> Element<'_, Message> {
        let account = self
            .accounts
            .list()
            .iter()
            .find(|account| account.is_microsoft())
            .cloned();
        let mut content = column![
            text("Microsoft account").size(15).font(theme::bold()),
            text("Signing in uses the Microsoft device-code flow: this launcher shows a code, you type it at microsoft.com/link on any device, and the launcher polls until you are done. The tokens it receives are stored next to your instances and renewed automatically before a launch.")
                .size(12),
        ]
        .spacing(8);
        if let Some(account) = &account {
            content = content.push(text(match account.entitled {
                Some(true) => format!("Signed in as {} — this account owns Minecraft.", account.username),
                Some(false) => format!("Signed in as {}, but no game entitlement was found.", account.username),
                None => format!("Signed in as {}.", account.username),
            })
            .size(12));
            // Said here rather than only next to the account row: the card is
            // where a user looks when a launch is about to renew a token, and
            // "expired" is a state that resolves itself or needs one click.
            if account.needs_refresh(prism_core::util::now_millis()) {
                content = content.push(
                    text("The stored session has expired; the next launch renews it automatically.")
                        .size(11),
                );
            }
        }
        if let Some(error) = &self.microsoft.error {
            content = content.push(text(format!("Sign-in failed: {error}")).size(12));
        } else if self.microsoft.pending {
            content = content.push(text(self.microsoft.status.clone()).size(12));
        }
        content = content.push(
            row![
                button(text(if account.is_some() { "Sign in again" } else { "Sign in with Microsoft" }).size(12))
                    .on_press(Message::MicrosoftPressed)
                    .style(theme::primary())
                    .padding([7, 16]),
                text("An offline account is enough for single-player; servers need a Microsoft account.")
                    .size(11),
            ]
            .spacing(12)
            .align_items(iced::Alignment::Center),
        );
        container(content)
            .style(theme::card)
            .padding(14)
            .width(Length::Fill)
            .into()
    }

    /// The device-code dialog: the code, where to type it, and what is happening.
    fn view_microsoft_dialog(&self) -> Element<'_, Message> {
        let code = self
            .microsoft
            .user_code
            .clone()
            .unwrap_or_else(|| "··········".to_string());
        let url = self
            .microsoft
            .verification_uri
            .clone()
            .unwrap_or_else(|| "https://microsoft.com/link".to_string());
        let mut body = column![
            text("Sign in with Microsoft").size(16).font(theme::bold()),
            text("Open the page below and enter this code.")
                .size(12),
            container(text(code.clone()).size(28).font(theme::bold()))
                .style(theme::card)
                .padding([14, 20])
                .width(Length::Fill)
                .center_x(),
            text(url.clone()).size(12),
            row![
                button(text("Copy code").size(12))
                    .on_press(Message::MicrosoftCopyCode)
                    .style(theme::secondary())
                    .padding([7, 14]),
                button(text("Open browser").size(12))
                    .on_press(Message::MicrosoftOpenUrl)
                    .style(theme::primary())
                    .padding([7, 14]),
            ]
            .spacing(10),
        ]
        .spacing(12);
        if let Some(error) = &self.microsoft.error {
            body = body.push(text(format!("Sign-in failed: {error}")).size(12));
        } else if self.microsoft.pending {
            body = body.push(text(self.microsoft.status.clone()).size(12));
        }
        body = body.push(text("The code expires in about fifteen minutes. This dialog stops polling the moment it closes.")
            .size(11));
        let footer: Element<'_, Message> = row![
            horizontal_space(),
            button(text("Cancel").size(12))
                .on_press(Message::MicrosoftCancel)
                .style(theme::secondary())
                .padding([7, 16]),
            button(text("Close").size(12))
                .on_press(Message::CloseModal)
                .style(theme::secondary())
                .padding([7, 16]),
        ]
        .spacing(10)
        .into();
        modal_shell("Microsoft sign-in", body.into(), footer, 520.0)
    }

    /// About / help page.
    fn view_about(&self) -> Element<'_, Message> {
        let rows: Vec<(String, String)> = vec![
            ("Version".into(), brand::full_name()),
            ("Graphics".into(), self.graphics_summary()),
            ("Adapter".into(), self.adapter_summary()),
            ("Data root".into(), self.paths.root.display().to_string()),
            ("Instances".into(), self.instances_dir.display().to_string()),
            ("Metadata cache".into(), self.paths.meta_dir().display().to_string()),
            ("Accounts".into(), self.paths.accounts_file().display().to_string()),
            ("Global config".into(), self.paths.global_config().display().to_string()),
        ];
        let mut body = column![
            row![
                container(
                    Image::new(brand::logo_handle())
                        .width(Length::Fixed(64.0))
                        .height(Length::Fixed(64.0)),
                )
                .style(theme::hero_tile)
                .padding(8),
                column![
                    text(brand::APP_NAME).size(24).font(theme::bold()),
                    text("A Prism-compatible Minecraft launcher with a Modrinth-style shell.").size(12),
                ]
                .spacing(4),
            ]
            .spacing(14)
            .align_items(iced::Alignment::Center),
            horizontal_rule(1u16),
            text("Shortcuts").size(15).font(theme::bold()),
            text("N — create an instance     ·     Esc — close a dialog     ·     Ctrl+R — rescan instances").size(12),
            text("Drag & drop — drop a PNG for a custom instance icon, or a .mrpack/.zip to import a pack").size(12),
            horizontal_rule(1u16),
            text("Paths").size(15).font(theme::bold()),
        ]
        .spacing(10);
        for (label, value) in rows {
            body = body.push(
                row![
                    text(label).size(12).width(Length::Fixed(120.0)),
                    text(value).size(12).font(iced::Font::MONOSPACE),
                ]
                .spacing(10),
            );
        }
        body = body.push(horizontal_rule(1u16));
        body = body.push(text("Honest status").size(15).font(theme::bold()));
        body = body.push(text("• Launching fetches the version metadata, libraries, the client jar, the asset index and its objects, extracts the natives, probes Java and streams the game's output. A file that cannot be downloaded blocks the launch and says which one it was.").size(12));
        body = body.push(text("• Microsoft sign-in uses the device-code flow; the refresh token is stored in accounts.json and the session is renewed automatically before a launch.").size(12));
        body = body.push(text("• Offline account UUIDs are derived from the name the way Java does, so the same name is the same player in every launcher.").size(12));
        body = body.push(text("• Modpack import copies the pack's overrides offline; the remote files it lists are not fetched.").size(12));
        body = body.push(text("• Instance icons are the Prism Launcher art (GPL-3.0-only), embedded at build time.").size(12));
        page_scroller(body)
    }

    /// The launcher's own settings: a section list beside the active pane.
    ///
    /// Every tab in the list is a real button onto a real pane. The previous
    /// version drew ten of the eleven as plain labels, on the argument that a
    /// button leading to an empty pane is a lie — which was true, and is why the
    /// panes now exist rather than the labels disappearing.
    fn view_settings_dialog(&self) -> Element<'_, Message> {
        let view = settings::View {
            prefs: &self.prefs,
            anim: &self.switches,
            pointer: self.switch_pointer,
            drafts: &self.settings_drafts,
            account: self.accounts.selected_account(),
            flag_filter: &self.flag_filter,
        };
        let body = row![
            container(settings::nav(self.settings_tab, self.prefs.developer_mode))
                .width(Length::Fixed(settings::NAV_WIDTH)),
            vertical_rule(1u16),
            container(settings::pane(self.settings_tab, &view))
                .padding([0, 18])
                .width(Length::Fill),
        ];

        container(column![
            modal_header("Settings"),
            body,
            container(settings::footer(self.prefs.developer_mode)).padding([12, 22]),
        ])
        .style(theme::modal)
        .width(Length::Fixed(settings::DIALOG_WIDTH))
        .max_height(DIALOG_MAX_HEIGHT)
        .into()
    }

    /// The dialog that replaced the content area.
    fn view_modal(&self) -> Element<'_, Message> {
        let dialog: Element<'_, Message> = match &self.modal {
            Modal::None => return self.view_page(),
            Modal::Create => self.view_create_dialog(),
            Modal::Import => self.view_import_dialog(),
            Modal::Settings => self.view_settings_dialog(),
            Modal::Microsoft => self.view_microsoft_dialog(),
            Modal::ConfirmDelete(id) => view_confirm_delete(id),
        };
        // One container, centred on both axes. (A wrapper around the dialog
        // cannot centre it: a shrink-wrapped container has nothing to centre
        // *within*, so the dialog used to land in the pane's top-left corner.)
        container(dialog)
            .style(theme::backdrop)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
    }

    /// Create instance, step 1 (choose a path) or step 2 (configure).
    fn view_create_dialog(&self) -> Element<'_, Message> {
        let header = modal_header("Create instance");
        let (body, footer) = match self.create.step {
            CreateStep::Choose => {
                let mut body = column![
                    text("Already know what you want to play?").size(14).font(theme::bold()),
                    text_input("Search mods, modpacks, and more…", &self.create.query)
                        .on_input(Message::CreateSearchChanged)
                        .on_submit(Message::CreateFocusSearch)
                        .style(theme::Field)
                        .padding([10, 12]),
                ]
                .spacing(10);
                if self.create.searching {
                    body = body.push(text("Searching Modrinth…").size(12));
                }
                if let Some(error) = &self.create.search_error {
                    body = body.push(text(format!("Search failed: {error}")).size(12));
                }
                for hit in &self.create.results {
                let project = hit.project_ref().to_string();
                    let title = hit.title.clone();
                    body = body.push(
                        button(
                            row![
                                column![
                                    text(hit.title.clone()).size(13).font(theme::bold()),
                                    text(hit.description.clone()).size(11),
                                ]
                                .spacing(2)
                                .width(Length::Fill),
                                chip(hit.downloads_label(), theme::chip_neutral),
                            ]
                            .spacing(10)
                            .align_items(iced::Alignment::Center),
                        )
                        .on_press(Message::CreateProjectPicked(project, title))
                        .style(theme::secondary())
                        .padding(10)
                        .width(Length::Fill),
                    );
                }
                body = body.push(divider_label("or"));
                body = body.push(text("Choose instance type").size(14).font(theme::bold()));
                body = body.push(option_row(
                    "cube",
                    "Custom setup",
                    "Start from scratch by picking a loader and game version.",
                    Message::CreateCustomSetup,
                ));
                body = body.push(option_row(
                    "search",
                    "Start from a mod or modpack",
                    "Search Modrinth above and we will install it right after creating.",
                    Message::CreateFocusSearch,
                ));
                body = body.push(option_row(
                    "upload",
                    "Upload a modpack",
                    "Drop a .mrpack or CurseForge .zip anywhere on this window.",
                    Message::CreateAwaitPack,
                ));
                body = body.push(option_row(
                    "download",
                    "Import instance",
                    "Copy an instance from Prism Launcher on this machine.",
                    Message::OpenImport,
                ));
                let footer = row![
                    horizontal_space(),
                    button(text("Cancel").size(13))
                        .on_press(Message::CloseModal)
                        .style(theme::ghost())
                        .padding([9, 16]),
                ]
                .spacing(8);
                (body, footer)
            }
            CreateStep::Configure => {
                let catalog = &self.catalog.catalog;
                let builds = catalog.loader_builds(self.create.loader, &self.create.game);
                let other = catalog.other_builds(self.create.loader, &self.create.game);
                let games = catalog.game_versions(self.create.show_snapshots);
                let no_games = games.is_empty();

                let mut body = column![
                    row![
                        self.view_icon_preview(),
                        column![
                            button(text("Upload").size(12))
                                .on_press(Message::CreateIconUpload)
                                .style(theme::secondary())
                                .padding([7, 14])
                                .width(Length::Fill),
                            button(text("Randomize").size(12))
                                .on_press(Message::CreateIconRandomize)
                                .style(theme::secondary())
                                .padding([7, 14])
                                .width(Length::Fill),
                            button(text("Customize").size(12))
                                .on_press(Message::CreateIconCustomize)
                                .style(if self.create.customize_open { theme::primary() } else { theme::secondary() })
                                .padding([7, 14])
                                .width(Length::Fill),
                        ]
                        .spacing(6)
                        .width(Length::Fill),
                    ]
                    .spacing(12)
                    .align_items(iced::Alignment::Start),
                ]
                .spacing(10);

                if self.create.awaiting_upload {
                    body = body.push(text("Drop a PNG anywhere on the window…").size(12));
                }
                if let Some(source) = &self.create.icon_source {
                    body = body.push(text(format!("Icon: {}", source.display())).size(11));
                }
                if self.create.customize_open {
                    let mut icons = row![].spacing(8);
                    for key in ICON_CHOICES {
                        let active = self.create.icon_key == key;
                        icons = icons.push(
                            button(icon_tile(key, 34.0, active))
                                .on_press(Message::CreateIconPicked(key.to_string()))
                                .style(theme::ghost())
                                .padding(2),
                        );
                    }
                    body = body.push(icons);
                }

                body = body.push(section("Name"));
                body = body.push(text_input("My instance", &self.create.name)
                    .on_input(Message::CreateNameChanged)
                    .style(theme::Field)
                    .padding([9, 11]));

                body = body.push(section("Loader"));
                let mut chips = row![].spacing(6);
                for loader in LoaderKind::all() {
                    chips = chips.push(
                        button(text(loader.label()).size(12))
                            .on_press(Message::CreateLoaderPicked(loader))
                            .style(theme::chip_button(self.create.loader == loader))
                            .padding([7, 13]),
                    );
                }
                body = body.push(chips);

                body = body.push(section("Game version"));
                let game_selected = if self.create.game.is_empty() {
                    None
                } else {
                    Some(self.create.game.clone())
                };
                body = body.push(
                    row![
                        pick_list(games, game_selected, Message::CreateGamePicked)
                            .style(theme::Dropdown)
                            .padding([8, 10])
                            .width(Length::Fill),
                        checkbox("Snapshots", self.create.show_snapshots)
                            .on_toggle(Message::CreateSnapshotsToggled)
                            .style(theme::Tick),
                    ]
                    .spacing(10)
                    .align_items(iced::Alignment::Center),
                );

                if no_games {
                    // No list means no version to pick, and no version means
                    // nothing to create: say why, and offer the way out.
                    let mut hint = row![text(if self.catalog.loading {
                        "Loading Minecraft versions…"
                    } else {
                        "No Minecraft versions yet — retry once you are back online."
                    })
                    .size(11)]
                    .spacing(8)
                    .align_items(iced::Alignment::Center);
                    if !self.catalog.loading {
                        hint = hint.push(
                            button(text("Retry").size(12))
                                .on_press(Message::ReloadCatalog)
                                .style(theme::secondary())
                                .padding([6, 12]),
                        );
                    }
                    body = body.push(hint);
                }

                if self.create.loader.loads_mods() {
                    body = body.push(section("Loader version"));
                    let mut choices = row![].spacing(6);
                    for choice in [BuildChoice::Stable, BuildChoice::Latest, BuildChoice::Other] {
                        choices = choices.push(
                            button(text(choice.label()).size(12))
                                .on_press(Message::CreateBuildChoicePicked(choice))
                                .style(theme::chip_button(self.create.build_choice == choice))
                                .padding([7, 13]),
                        );
                    }
                    body = body.push(choices);
                    if self.create.build_choice == BuildChoice::Other {
                        let selected = if self.create.build.is_empty() {
                            None
                        } else {
                            Some(self.create.build.clone())
                        };
                        body = body.push(
                            pick_list(other, selected, Message::CreateBuildPicked)
                                .style(theme::Dropdown)
                                .padding([8, 10])
                                .width(Length::Fill),
                        );
                    }
                    let summary = if self.catalog.loading {
                        "Loading builds…".to_string()
                    } else if builds.is_empty() {
                        "No builds for this game version.".to_string()
                    } else {
                        format!(
                            "{} {} · {} build(s) available for {}",
                            self.create.loader.label(),
                            if self.create.build.is_empty() { "?" } else { &self.create.build },
                            builds.len(),
                            self.create.game
                        )
                    };
                    body = body.push(text(summary).size(11));
                } else {
                    body = body.push(text("Vanilla: no mod loader will be installed.").size(11));
                }

                if let Some(error) = &self.create.error {
                    body = body.push(text(error.clone()).size(12).style(iced::theme::Text::Color(theme::danger())));
                }
                if let Some(project) = &self.create.install_after {
                    body = body.push(text(format!("Then install: {}", project.1)).size(11));
                }
                if let Some(warning) = &self.catalog.error {
                    body = body.push(text(warning.clone()).size(11));
                }

                let can_submit = !self.create.effective_name().trim().is_empty()
                    && !self.create.game.trim().is_empty()
                    && (!self.create.loader.loads_mods()
                        || (!self.create.build.is_empty() && !builds.is_empty()));
                let create_button: Button<'_, Message> = if can_submit {
                    button(text("+ Create instance").size(13))
                        .on_press(Message::CreateSubmit)
                        .style(theme::primary())
                        .padding([9, 18])
                } else {
                    button(text("+ Create instance").size(13))
                        .style(theme::primary())
                        .padding([9, 18])
                };
                let footer = row![
                    button(text("← Back").size(13))
                        .on_press(Message::CreateBack)
                        .style(theme::secondary())
                        .padding([9, 16]),
                    horizontal_space(),
                    create_button,
                ]
                .spacing(8);
                (body, footer)
            }
        };

        let height = match self.create.step {
            CreateStep::Choose => CREATE_BODY_CHOOSE,
            CreateStep::Configure => CREATE_BODY_CONFIGURE,
        };
        container(column![
            header,
            container(
                scrollable(body)
                    .style(iced::theme::Scrollable::custom(theme::Thin))
                    .height(Length::Fixed(height)),
            )
            .padding([4, 22]),
            container(footer).padding([14, 22]),
        ])
        .style(theme::modal)
        .width(Length::Fixed(DIALOG_WIDTH))
        .max_height(DIALOG_MAX_HEIGHT)
        .into()
    }

    /// The icon the new instance would get: a dropped PNG, a chosen built-in
    /// icon, or the default grass block.
    fn view_icon_preview(&self) -> Element<'_, Message> {
        match &self.create.icon_source {
            Some(path) => container(
                Image::new(Handle::from_path(path.clone()))
                    .width(Length::Fixed(58.0))
                    .height(Length::Fixed(58.0)),
            )
            .style(theme::icon_tile(theme::surface_input()))
            .width(Length::Fixed(72.0))
            .height(Length::Fixed(72.0))
            .center_x()
            .center_y()
            .into(),
            None => {
                let key = if self.create.icon_key.is_empty() {
                    "grass"
                } else {
                    self.create.icon_key.as_str()
                };
                icon_tile(key, 72.0, true)
            }
        }
    }

    /// Import dialog.
    fn view_import_dialog(&self) -> Element<'_, Message> {
        let mut body = column![
            text("Copy an instance from another launcher. Prism Launcher's on-disk format is the one this launcher reads, so imports are exact copies.")
                .size(12),
            horizontal_rule(1u16),
        ]
        .spacing(10);
        if self.import.loading {
            body = body.push(centered_note("Looking for other launchers…"));
        }
        if let Some(error) = &self.import.error {
            body = body.push(text(error.clone()).size(12));
        }
        for (index, candidate) in self.import.candidates.iter().enumerate() {
            body = body.push(
                container(
                    row![
                        column![
                            text(candidate.name.clone()).size(14).font(theme::bold()),
                            text(candidate.source.display().to_string()).size(11),
                        ]
                        .spacing(2)
                        .width(Length::Fill),
                        chip(candidate.origin.to_string(), theme::chip_neutral),
                        button(text("Import").size(12))
                            .on_press(Message::ImportPicked(index))
                            .style(theme::primary())
                            .padding([7, 14]),
                    ]
                    .spacing(10)
                    .align_items(iced::Alignment::Center),
                )
                .style(theme::card)
                .padding(12)
                .width(Length::Fill),
            );
        }
        container(column![
            modal_header("Import instance"),
            container(
                scrollable(body)
                    .style(iced::theme::Scrollable::custom(theme::Thin))
                    .height(Length::Fixed(CREATE_BODY_CHOOSE)),
            )
            .padding([4, 22]),
            container(
                row![
                    horizontal_space(),
                    button(text("Close").size(13))
                        .on_press(Message::CloseModal)
                        .style(theme::secondary())
                        .padding([9, 16]),
                ]
                .spacing(8),
            )
            .padding([14, 22]),
        ])
        .style(theme::modal)
        .width(Length::Fixed(620.0))
        .into()
    }

    /// Right sidebar: getting started, account, selection, run state, about.
    fn view_sidebar(&self) -> Element<'_, Message> {
        // `p-4` between the panel's edge and its first card, and between
        // sections, is what the reference uses for every section of this column.
        let mut side = column![].spacing(12).padding(16);
        side = side.push(self.card_getting_started());
        side = side.push(self.card_account());
        if let Some(card) = self.selected_card() {
            side = side.push(self.card_instance(card));
        }
        if let Some(run) = &self.active_run {
            side = side.push(self.card_running(&run.instance_id));
        }
        side = side.push(self.card_version());
        container(scrollable(side).style(iced::theme::Scrollable::custom(theme::Thin)).height(Length::Fill))
            .style(theme::sidebar)
            .width(Length::Fixed(SIDEBAR_CONTAINER_WIDTH))
            .height(Length::Fill)
            .into()
    }

    fn card_getting_started(&self) -> Element<'_, Message> {
        let steps = [
            ("Add an account", self.accounts.selected_account().is_some()),
            ("Create an instance", !self.cards.is_empty()),
            ("Press Play", self.cards.iter().any(|card| card.playtime_secs > 0)),
        ];
        // The reference's step row is `h-10 px-4 gap-2 rounded-xl`, i.e. exactly
        // 40px tall with 16px of side padding, and its marker is the one place
        // the two states differ completely: an 18px filled brand disc holding a
        // 12px check once done, and a 20px hollow ring while not. The label
        // dims to secondary and strikes through when complete rather than the
        // row disappearing, so the panel does not change height underneath the
        // pointer the moment a step is finished.
        let mut rows = column![].spacing(8);
        for (label, done) in steps {
            let marker: Element<'_, Message> = if done {
                container(glyph("check", 11.0, theme::on_accent()))
                    .style(theme::circle(theme::accent()))
                    .width(Length::Fixed(18.0))
                    .height(Length::Fixed(18.0))
                    .center_x()
                    .center_y()
                    .into()
            } else {
                container(text(""))
                    .style(theme::step_ring)
                    .width(Length::Fixed(20.0))
                    .height(Length::Fixed(20.0))
                    .into()
            };
            rows = rows.push(
                container(
                    row![
                        marker,
                        text(label)
                            .size(16)
                            .font(theme::medium())
                            .width(Length::Fill)
                            .style(iced::theme::Text::Color(if done {
                                theme::text_muted()
                            } else {
                                theme::text()
                            })),
                    ]
                    .spacing(8)
                    .align_items(iced::Alignment::Center),
                )
                .style(theme::sidebar_step)
                .padding([0, 16])
                .height(Length::Fixed(40.0))
                .width(Length::Fill),
            );
        }
        container(
            column![
                row![
                    Image::new(brand::logo_handle())
                        .width(Length::Fixed(20.0))
                        .height(Length::Fixed(20.0)),
                    text("Getting started").size(16).font(theme::semibold()),
                ]
                .spacing(8)
                .align_items(iced::Alignment::Center),
                rows,
                button(text("+ Create an instance").size(12))
                    .on_press(Message::OpenCreate)
                    .style(theme::primary())
                    .padding([8, 14])
                    .width(Length::Fill),
            ]
            .spacing(12),
        )
        .style(theme::sidebar_card)
        .padding(14)
        .width(Length::Fill)
        .into()
    }

    fn card_account(&self) -> Element<'_, Message> {
        let signed_in = self.accounts.selected_account().is_some();
        container(
            column![
                text("Playing as").size(16).font(theme::semibold()),
                row![
                    icon_tile("steve", 40.0, signed_in),
                    column![
                        text(self.account_label()).size(14).font(theme::bold()),
                        text(self.account_detail()).size(11),
                    ]
                    .spacing(2)
                    .width(Length::Fill),
                ]
                .spacing(10)
                .align_items(iced::Alignment::Center),
                if signed_in {
                    button(text("Manage accounts").size(12))
                        .on_press(Message::PageSelected(Page::Accounts))
                        .style(theme::secondary())
                        .padding([7, 12])
                        .width(Length::Fill)
                } else {
                    button(text("Add an account").size(12))
                        .on_press(Message::PageSelected(Page::Accounts))
                        .style(theme::primary())
                        .padding([7, 12])
                        .width(Length::Fill)
                },
            ]
            .spacing(10),
        )
        .style(theme::sidebar_card)
        .padding(14)
        .width(Length::Fill)
        .into()
    }

    fn card_instance(&self, card: &InstanceCard) -> Element<'_, Message> {
        let running = self
            .active_run
            .as_ref()
            .map(|run| run.instance_id == card.id)
            .unwrap_or(false);
        let mut body = column![
            row![
                icon_tile(&card.icon, CARD_ICON, true),
                column![
                    text(card.name.clone()).size(16).font(theme::semibold()),
                    text(card.subtitle()).size(11),
                ]
                .spacing(3)
                .width(Length::Fill),
            ]
            .spacing(10)
            .align_items(iced::Alignment::Center),
            {
                let mut chips = row![chip(
                    format!("{} mods", card.mods_total),
                    theme::chip_neutral
                )]
                .spacing(6);
                if self.prefs.show_play_time {
                    chips = chips.push(chip(card.playtime_label(), theme::chip_neutral));
                }
                chips.push(chip(format!("{} MiB", card.max_mem_mb), theme::chip_neutral))
            },
        ]
        .spacing(10);
        if let Some(problem) = &card.problem {
            body = body.push(text(problem.clone()).size(11).style(iced::theme::Text::Color(theme::danger())));
        }
        let play: Element<'_, Message> = if running {
            button(text("Kill").size(13))
                .on_press(Message::KillPressed)
                .style(theme::destructive())
                .padding([9, 16])
                .width(Length::Fill)
                .into()
        } else {
            button(
                row![
                    glyph("play", 14.0, theme::on_accent()),
                    text("Play").size(13),
                ]
                .spacing(8)
                .align_items(iced::Alignment::Center),
            )
            .on_press(Message::PlayInstance(card.id.clone()))
            .style(theme::primary())
            .padding([9, 16])
            .width(Length::Fill)
            .into()
        };
        body = body.push(play);
        body = body.push(
            row![
                button(text("Edit").size(12))
                    .on_press(Message::EditInstance(card.id.clone()))
                    .style(theme::secondary())
                    .padding([6, 10])
                    .width(Length::Fill),
                button(text("Folder").size(12))
                    .on_press(Message::OpenFolder(card.id.clone()))
                    .style(theme::secondary())
                    .padding([6, 10])
                    .width(Length::Fill),
                button(text("Copy").size(12))
                    .on_press(Message::DuplicateInstance(card.id.clone()))
                    .style(theme::secondary())
                    .padding([6, 10])
                    .width(Length::Fill),
            ]
            .spacing(6),
        );
        body = body.push(
            row![
                text("Group").size(11),
                pick_list(
                    self.group_options(),
                    Some(self.selected_group_label()),
                    Message::GroupSelected,
                )
                .style(theme::Dropdown)
                .padding([4, 8])
                .width(Length::Fill),
            ]
            .spacing(8)
            .align_items(iced::Alignment::Center),
        );
        body = body.push(
            button(text("Delete…").size(12))
                .on_press(Message::AskDelete(card.id.clone()))
                .style(theme::destructive())
                .padding([6, 10])
                .width(Length::Fill),
        );
        container(body)
            .style(theme::sidebar_card)
            .padding(14)
            .width(Length::Fill)
            .into()
    }

    fn card_running(&self, id: &str) -> Element<'_, Message> {
        container(
            column![                    row![
                    container(text("")).style(theme::pill(theme::accent())).width(Length::Fixed(8.0)).height(Length::Fixed(8.0)),
                    text("Running").size(16).font(theme::semibold()),
                ]
                .spacing(8)
                .align_items(iced::Alignment::Center),
                text(id.to_string()).size(13),
                text("Open the Logs page to watch the output.").size(11),
                button(text("Kill process").size(12))
                    .on_press(Message::KillPressed)
                    .style(theme::destructive())
                    .padding([7, 12])
                    .width(Length::Fill),
            ]
            .spacing(8),
        )
        .style(theme::sidebar_card)
        .padding(14)
        .width(Length::Fill)
        .into()
    }

    fn card_version(&self) -> Element<'_, Message> {
        container(
            column![
                text(format!("{} v{}", brand::APP_NAME, brand::version()))
                    .size(16)
                    .font(theme::semibold()),
                text(format!("Data root: {}", self.paths.root.display())).size(10),
                row![
                    button(text("About").size(12))
                        .on_press(Message::PageSelected(Page::About))
                        .style(theme::secondary())
                        .padding([6, 12])
                        .width(Length::Fill),
                    button(text("Instances folder").size(12))
                        .on_press(Message::OpenInstancesFolder)
                        .style(theme::secondary())
                        .padding([6, 12])
                        .width(Length::Fill),
                ]
                .spacing(6),
            ]
            .spacing(8),
        )
        .style(theme::sidebar_card)
        .padding(14)
        .width(Length::Fill)
        .into()
    }

    /// Slim status strip at the bottom.
    fn view_status_bar(&self) -> Element<'_, Message> {
        let color = if self.status_is_error { theme::danger() } else { theme::text_muted() };
        container(
            row![
                text(format!("{} instance(s)", self.cards.len())).size(11),
                text("·").size(11),
                text(self.status.clone()).size(11).style(iced::theme::Text::Color(color)),
                horizontal_space(),
                text(format!(
                    "selected: {}",
                    self.selected.as_deref().unwrap_or("none")
                ))
                .size(11),
            ]
            .spacing(8)
            .align_items(iced::Alignment::Center),
        )
        .style(theme::toast)
        .padding([5, 12])
        .width(Length::Fill)
        .into()
    }
}

/// Observation accessors: the tests drive the whole shell through them, and
/// they are the hooks a future remote-debug surface would use. They are gated
/// on `cfg(test)` so the shipped binary carries no unreachable API surface.
#[cfg(test)]
impl PrismApp {
    /// Current status line.
    pub fn status(&self) -> &str {
        &self.status
    }

    /// Whether the status line reports a failure.
    pub fn status_is_error(&self) -> bool {
        self.status_is_error
    }

    /// Currently selected instance id.
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// Whether a launch is streaming.
    pub fn is_running(&self) -> bool {
        self.active_run.is_some()
    }

    /// The page currently shown.
    pub fn page(&self) -> Page {
        self.page
    }

    /// The create dialog's form.
    pub fn create_form(&self) -> &CreateForm {
        &self.create
    }

    /// The metadata-catalog state.
    pub fn catalog_state(&self) -> &CatalogState {
        &self.catalog
    }

    /// The browse state.
    pub fn browse_state(&self) -> &BrowseState {
        &self.browse
    }

    /// The import state.
    pub fn import_state(&self) -> &ImportState {
        &self.import
    }

    /// Number of instances.
    pub fn instance_count(&self) -> usize {
        self.cards.len()
    }

    /// The instance cards.
    pub fn cards(&self) -> &[InstanceCard] {
        &self.cards
    }

    /// The log buffer.
    pub fn console(&self) -> &VecDeque<String> {
        &self.console
    }

    /// Accounts handle.
    pub fn accounts(&self) -> &crate::accounts::AccountsStore {
        &self.accounts
    }

    /// Mods of the selection.
    pub fn mods(&self) -> &[ModEntry] {
        &self.mods
    }
}

// ---- free widget helpers ------------------------------------------------

/// A rail entry: a drawn glyph that takes the rail's ink colour.
///
/// The colour is why the icons are vectors — a bitmap cannot be recoloured, so
/// an embedded icon looked identical whether its page was the active one or
/// not.
/// One icon-rail entry, with the label it shows on hover.
///
/// The label used to be dropped on the floor (`_label`), so six rail buttons
/// looked identical whether you were pointing at one or not. The tooltip is the
/// reference UI's answer to that: a single icon per destination, named on hover.
fn rail_icon(icon: &str, label: &str, active: bool, message: Message) -> Element<'static, Message> {
    let color = if active { theme::accent() } else { theme::text_muted() };
    let tile = button(container(glyph(icon, 24.0, color)).center_x().center_y())
        .on_press(message)
        .style(theme::rail_button(active))
        .padding([0, 0])
        .width(Length::Fixed(RAIL_BUTTON))
        .height(Length::Fixed(RAIL_BUTTON));
    tooltip(
        tile,
        container(text(label.to_string()).size(12))
            .style(theme::Tooltip)
            .padding([6, 10]),
        tooltip::Position::Right,
    )
    .gap(6)
    .padding(0)
    .into()
}

/// One screenshot tile: the picture, then what it is.
fn shot_tile(tile: &ShotTile) -> Element<'static, Message> {
    let picture = Image::new(tile.handle.clone())
        .width(Length::Fixed(SHOT_TILE_WIDTH))
        .height(Length::Fixed(SHOT_TILE_HEIGHT));
    container(
        column![
            // The dark square behind the picture stands in for the letterboxing
            // bars on a screenshot that is not the expected aspect ratio.
            container(picture)
                .style(theme::icon_tile(theme::bg()))
                .padding(2),
            text(tile.caption())
                .size(11)
                .style(iced::theme::Text::Color(theme::text_muted())),
        ]
        .spacing(7),
    )
    .style(theme::card)
    .padding(8)
    .into()
}

/// A rounded icon tile (embedded instance art).
fn icon_tile(key: &str, side: f32, accent: bool) -> Element<'static, Message> {
    let background = if accent { theme::alpha(theme::accent(), 0.18) } else { theme::surface_input() };
    container(
        Image::new(instance_handle(key))
            .width(Length::Fixed(side - 14.0))
            .height(Length::Fixed(side - 14.0)),
    )
    .style(theme::icon_tile(background))
    .width(Length::Fixed(side))
    .height(Length::Fixed(side))
    .center_x()
    .center_y()
    .into()
}

/// A whole instance card: clickable body + inline play button.
/// A 1px hairline between the raised chrome and the page.
///
/// A container rather than `horizontal_rule`, because that widget takes its
/// colour from the theme's own rule style while this line has a measured value:
/// `#42444a`, the reference's `surface-5`, along the bar's bottom edge and the
/// rail's right edge. A container's border is drawn on every edge, so a line
/// that exists on exactly one edge has to be its own widget.
fn hairline() -> Element<'static, Message> {
    container(text(""))
        .style(theme::separator)
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .into()
}

/// The vertical twin of [`hairline`], for the rail's right edge.
fn rail_hairline() -> Element<'static, Message> {
    container(text(""))
        .style(theme::separator)
        .width(Length::Fixed(1.0))
        .height(Length::Fill)
        .into()
}

/// What a library card draws beyond the instance itself.
///
/// Two switches reach the same card, so they travel together as one value
/// rather than as two positional `bool`s — three booleans in a row at a call
/// site is a place where the wrong one eventually gets passed and nothing says
/// so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CardChrome {
    /// Draw the playtime chip.
    show_play_time: bool,
    /// Drop the metadata chips and tighten the card up.
    compact: bool,
}

impl CardChrome {
    fn from_prefs(prefs: &Prefs) -> CardChrome {
        CardChrome { show_play_time: prefs.show_play_time, compact: prefs.compact_instance_cards }
    }

    /// Whether a given chip is drawn on the card.
    ///
    /// A pure function of the two switches rather than four `if`s spread
    /// through the layout, so what "compact" *removes* is a testable claim
    /// instead of something only a screenshot could settle.
    fn shows(self, chip: CardChip) -> bool {
        match chip {
            CardChip::PlayTime => self.show_play_time,
            CardChip::Mods | CardChip::Loader | CardChip::Group => !self.compact,
        }
    }
}

/// One of the metadata chips a library card can carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CardChip {
    PlayTime,
    Mods,
    Loader,
    Group,
}

fn instance_card(
    card: &InstanceCard,
    selected: bool,
    chrome: CardChrome,
) -> Element<'static, Message> {
    let body_card = card.clone();
    let name = card.name.clone();
    let subtitle = card.subtitle();
    let playtime = card.playtime_label();
    let id = card.id.clone();
    let play_id = card.id.clone();
    let group_chip = card.group.clone();
    let loader_chip = if card.has_loader() {
        Some(format!("{} {}", card.loader.label(), card.loader_version))
    } else {
        None
    };

    // Collected before they are drawn for two reasons: the row is omitted
    // entirely when nothing lands in it — an empty row still costs its own
    // height and the gap above it — and because which chips are present is
    // then a list a test can read.
    let mut chips: Vec<(String, ChipStyle)> = Vec::new();
    if chrome.shows(CardChip::PlayTime) {
        chips.push((playtime, theme::chip_neutral));
    }
    if chrome.shows(CardChip::Mods) {
        chips.push((format!("{} mods", body_card.mods_total), theme::chip_neutral));
    }
    if chrome.shows(CardChip::Loader) {
        let label = loader_chip.unwrap_or_else(|| "Vanilla".to_string());
        chips.push((label, theme::chip));
    }
    if chrome.shows(CardChip::Group) {
        if let Some(group) = group_chip {
            chips.push((group, theme::chip_neutral));
        }
    }

    let mut info = column![
        text(name).size(if chrome.compact { 14 } else { 16 }).font(theme::semibold()),
        text(subtitle).size(if chrome.compact { 12 } else { 14 }),
    ]
    .spacing(5)
    .width(Length::Fill);
    if !chips.is_empty() {
        let mut line = row![].spacing(6);
        for (label, style) in chips {
            line = line.push(chip(label, style));
        }
        info = info.push(line);
    }

    let icon = if chrome.compact { COMPACT_CARD_ICON } else { CARD_ICON };
    let body = button(
        row![icon_tile(&body_card.icon, icon, selected), info]
            .spacing(if chrome.compact { 9 } else { 12 })
            .align_items(iced::Alignment::Center),
    )
    .on_press(Message::SelectInstance(id))
    .style(theme::card_area(selected))
    .padding(if chrome.compact { 7 } else { 10 })
    .width(Length::Fill);

    let play = button(glyph("play", 16.0, theme::on_accent()))
        .on_press(Message::PlayInstance(play_id))
        .style(theme::primary())
        .padding(if chrome.compact { 9 } else { 12 });

    container(row![body, play].spacing(8).align_items(iced::Alignment::Center))
        .style(if selected { theme::card_selected } else { theme::card })
        .padding(if chrome.compact { 6 } else { 8 })
        .width(Length::Fill)
        .into()
}

/// How a chip paints itself, named so that a card's list of chips is a type
/// rather than a paragraph.
type ChipStyle = fn(&Theme) -> container::Appearance;

/// A labelled small pill; `label` may be empty (renders nothing).
fn chip(label: String, style: ChipStyle) -> Element<'static, Message> {
    let content: Element<'static, Message> = if label.is_empty() {
        Element::from(text("").size(11))
    } else {
        Element::from(text(label).size(11))
    };
    container(content).style(style).padding([3, 8]).into()
}

/// A section label inside a form.
fn section(label: &str) -> Element<'static, Message> {
    column![text(label.to_string()).size(13).font(theme::bold())].spacing(2).into()
}

/// The "or" separator used in the create dialog.
fn divider_label(label: &str) -> Element<'static, Message> {
    row![
        container(horizontal_rule(1u16)).width(Length::Fill),
        text(label.to_string()).size(11),
        container(horizontal_rule(1u16)).width(Length::Fill),
    ]
    .spacing(10)
    .align_items(iced::Alignment::Center)
    .into()
}

/// One of the four Create-dialog type rows.
fn option_row(icon: &str, title: &str, description: &str, message: Message) -> Element<'static, Message> {
    container(
        button(
            row![
                container(glyph(icon, 18.0, theme::text_muted()))
                    .style(theme::chip_neutral)
                    .padding([8, 10]),
                column![
                    text(title.to_string()).size(14).font(theme::bold()),
                    text(description.to_string()).size(11),
                ]
                .spacing(2)
                .width(Length::Fill),
                text("→").size(14),
            ]
            .spacing(12)
            .align_items(iced::Alignment::Center),
        )
        .on_press(message)
        .style(theme::ghost())
        .padding(10)
        .width(Length::Fill),
    )
    .style(theme::option_row)
    .padding(4)
    .width(Length::Fill)
    .into()
}

/// Make a non-interactive patch of chrome grabbable: press and drag moves the
/// window, right-click opens the native window menu.
///
/// iced has no notion of a "drag region", so every inert part of the title bar
/// has to opt in. Missing one leaves a dead patch the user cannot drag by,
/// which is what made moving the window feel broken.
/// The patch is given an explicit width and an explicit height spanning the
/// bar between its padding.
///
/// That is not cosmetic. A child that sizes to its content — a
/// `horizontal_space()`, say — has no height of its own inside a row whose
/// items are centred, and the mouse area built around it then inherits a rect
/// with zero height. A zero-height rect can never be *hovered*, and iced gates
/// every press on that test, so the widest stretch of the title bar ends up a
/// dead strip that ignores the pointer entirely.
///
/// `armed` is the drag cost control, and it matters more than it looks. In
/// iced 0.12 a *published message* is what makes the shell rebuild: `iced_winit`'s
/// event loop (`application.rs`, the `AboutToWait` arm) rebuilds only when event
/// dispatch produced a message or reported the interface `Outdated`, and each
/// message then runs the app's `update` and `view` — twice over for a command
/// that carries a widget operation. A pointer move that publishes nothing costs
/// a repaint of the widget tree the shell already holds, which `iced_tiny_skia`
/// then skips entirely when the primitives compare equal to last frame's. So an
/// always-attached `on_move` turned the title bar into the most expensive strip
/// in the window: sliding the pointer along it republished the whole view once
/// per move event — around a hundred times a second on Windows — for a handler
/// that could not do anything until a press had armed a drag. Attaching it only
/// between the press and the release keeps the drag behaviour identical and
/// stops the move storm.
fn grabbable<'a>(
    area: BarArea,
    width: Length,
    armed: bool,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    let mouse_area = iced::widget::MouseArea::new(
        container(content)
            .width(width)
            .height(Length::Fixed(TITLE_BAR_CONTENT_HEIGHT))
            .center_y(),
    )
    .on_press(Message::BarPressed(area))
    .on_release(Message::BarReleased(area))
    .on_right_press(Message::BarRightClick);

    let mouse_area = if armed {
        mouse_area.on_move(move |position| Message::BarCursorMoved(area, position))
    } else {
        mouse_area
    };

    mouse_area.into()
}

/// One edge or corner of the window frame: an invisible band that hands the
/// pointer to Windows to run a native resize loop.
///
/// `side` is the size across the band (the band's thickness for a corner, or
/// its extent for an edge); the other axis always fills. `paint` matches the
/// colour of the region the band borders so the frame never shows.
fn grip(
    edge: ResizeEdge,
    side: Length,
    paint: fn(&Theme) -> iced::widget::container::Appearance,
) -> Element<'static, Message> {
    let vertical = matches!(edge, ResizeEdge::West | ResizeEdge::East);
    let (width, height) = if vertical {
        (side, Length::Fill)
    } else {
        (Length::Fill, side)
    };

    iced::widget::MouseArea::new(
        container(text(""))
            .style(paint)
            .width(width)
            .height(height),
    )
    .on_press(Message::ResizeStart(edge))
    .on_right_press(Message::BarRightClick)
    .interaction(edge.interaction())
    .into()
}

/// Centered informational note.
fn browse_type_tabs(active: ContentType) -> Element<'static, Message> {
    let mut tabs = row![].spacing(6);
    for content_type in ContentType::all() {
        tabs = tabs.push(
            button(text(content_type.label()).size(12))
                .on_press(Message::BrowseTypePicked(content_type))
                .style(theme::chip_button(content_type == active))
                .padding([6, 10]),
        );
    }
    tabs.into()
}

fn centered_note(message: impl Into<String>) -> Element<'static, Message> {
    container(text(message.into()).size(13))
        .width(Length::Fill)
        .padding(24)
        .center_x()
        .into()
}

/// The delete confirmation dialog.
fn view_confirm_delete(id: &str) -> Element<'static, Message> {
    let body: Element<'static, Message> = column![
        text(format!("Delete '{id}'?")).size(15).font(theme::bold()),
        text("This removes the instance folder from disk. It cannot be undone.").size(12),
    ]
    .spacing(8)
    .into();
    let footer: Element<'static, Message> = row![
        horizontal_space(),
        button(text("Cancel").size(13))
            .on_press(Message::CloseModal)
            .style(theme::secondary())
            .padding([9, 16]),
        button(text("Delete").size(13))
            .on_press(Message::ConfirmDelete)
            .style(theme::destructive())
            .padding([9, 18]),
    ]
    .spacing(8)
    .into();
    modal_shell("Delete instance", body, footer, 440.0)
}

/// A dialog box: header, body, footer, modal styling and a fixed width.
fn modal_shell<'a>(
    title: &str,
    body: Element<'a, Message>,
    footer: Element<'a, Message>,
    width: f32,
) -> Element<'a, Message> {
    container(
        column![
            modal_header(title),
            container(body).padding([8, 22]),
            container(footer).padding([14, 22]),
        ]
        .spacing(0),
    )
    .style(theme::modal)
    .width(Length::Fixed(width))
    .into()
}

/// Dialog header: title plus a close button.
fn modal_header(title: &str) -> Element<'static, Message> {
    container(
        row![
            text(title.to_string()).size(20).font(theme::bold()),
            horizontal_space(),
            button(glyph("close", 13.0, theme::text_muted()))
                .on_press(Message::CloseModal)
                .style(theme::ghost())
                .padding([5, 9]),
        ]
        .spacing(10)
        .align_items(iced::Alignment::Center),
    )
    .padding([18, 22])
    .width(Length::Fill)
    .into()
}

/// A page's scrolling area: eased wheel scrolling, and iced's own scrollbar.
///
/// The guard is *inside* the scrollable, which is the whole trick: iced gives
/// the event to the content first and stands down when the content claims it, so
/// the shell gets to own the offset while the scrollbar, dragging it, touch and
/// keyboard scrolling all stay iced's.
///
/// The log page deliberately does not use this — see `view_logs`.
fn page_scroller<'a>(content: impl Into<Element<'a, Message>> + 'a) -> Element<'a, Message> {
    scrollable(scroll::guard(content, Message::PageWheel))
        .id(page_scroll_id())
        .on_scroll(|viewport: scrollable::Viewport| Message::PageScrolled {
            offset: viewport.absolute_offset().y,
            content_height: viewport.content_bounds().height,
            view_height: viewport.bounds().height,
        })
        .style(iced::theme::Scrollable::custom(theme::Thin))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Keyboard shortcut mapping.
fn shortcut(key: iced::keyboard::Key, modifiers: iced::keyboard::Modifiers) -> Option<Message> {
    use iced::keyboard::key::Named;
    use iced::keyboard::Key;
    match key.as_ref() {
        Key::Character(character) => {
            let text = character;
            if modifiers.control() && text.eq_ignore_ascii_case("r") {
                return Some(Message::Refresh);
            }
            if !modifiers.control() && text.eq_ignore_ascii_case("n") {
                return Some(Message::OpenCreate);
            }
            None
        }
        Key::Named(Named::Escape) => Some(Message::CloseModal),
        Key::Named(Named::Enter) => None,
        _ => None,
    }
}

/// One-shot background job as a subscription.
///
/// The worker thread runs `job`, which is expected to send exactly one
/// [`Message::TaskDone`]; the future then parks forever (the runtime drops the
/// subscription as soon as the caller stops asking for it).
fn one_shot<H, F>(id: H, capacity: usize, job: F) -> Subscription<Message>
where
    H: std::hash::Hash + 'static,
    F: FnOnce(futures::channel::mpsc::Sender<Message>) + Send + 'static,
{
    iced::subscription::channel(id, capacity, move |sender| async move {
        let _ = std::thread::spawn(move || job(sender));
        loop {
            futures::future::pending::<()>().await;
        }
    })
}

/// The Microsoft device-code sign-in flow, as a subscription.
///
/// Its life is the dialog's. The app asks for this only while
/// [`MicrosoftState::pending`], and when the dialog closes iced drops the
/// subscription — which drops the receiver — so the next send fails and the
/// polling thread ends there. That is the whole cancellation story: no flag to
/// poll, no thread to join, and no chance of two flows running at once (the
/// subscription id is fixed).
///
/// The thread does three things in order: ask for a code, poll until the user
/// finishes (honouring the RFC 8628 `authorization_pending` / `slow_down`
/// rules), then walk the Xbox → XSTS → Minecraft chain for the actual session.
/// Every message it sends can fail, and a failure means the dialog is gone.
fn microsoft_sign_in(client_id: String) -> Subscription<Message> {
    iced::subscription::channel(MICROSOFT_ID, 8, move |mut sender| async move {
        let _ = std::thread::spawn(move || {
            let auth = prism_net::MicrosoftAuth::new(
                prism_net::MicrosoftOAuth::with_default_scope(client_id),
            );
            run_microsoft_sign_in(&auth, &mut sender);
        });
        loop {
            futures::future::pending::<()>().await;
        }
    })
}

/// The body of [`microsoft_sign_in`], split out so it reads as a sequence.
fn run_microsoft_sign_in(
    auth: &prism_net::MicrosoftAuth,
    sender: &mut futures::channel::mpsc::Sender<Message>,
) {
    use prism_net::PollOutcome;

    let code = match auth.request_device_code() {
        Ok(code) => code,
        Err(error) => {
            let _ = send_signed_in(sender, Err(error.to_string()));
            return;
        }
    };
    if !send_message(
        sender,
        Message::MicrosoftCode {
            user_code: code.user_code.clone(),
            verification_uri: code.verification_uri.clone(),
        },
    ) {
        return;
    }
    let deadline = Instant::now() + Duration::from_secs(code.expires_in);
    let mut interval = code.interval;
    loop {
        if Instant::now() >= deadline {
            let _ = send_signed_in(
                sender,
                Err("The code expired before the sign-in finished — try again.".to_string()),
            );
            return;
        }
        std::thread::sleep(Duration::from_secs(interval));
        match auth.poll(&code.device_code, interval) {
            Ok(PollOutcome::Retry { interval: next }) => interval = next,
            Ok(PollOutcome::Authorized(msa)) => {
                if !send_message(sender, Message::MicrosoftStatus("Signing in to Minecraft…".into()))
                {
                    return;
                }
                let outcome = auth.finish(&msa).map(|session| {
                    let expires_at_ms = prism_core::util::now_millis()
                        + session.expires_in.max(0) * 1000;
                    AccountEntry::microsoft(
                        &session.name,
                        &session.uuid,
                        &session.access_token,
                        &msa.refresh_token,
                        expires_at_ms,
                        Some(session.entitled),
                    )
                });
                let _ = send_signed_in(sender, outcome.map_err(|error| error.to_string()));
                return;
            }
            Ok(PollOutcome::Failed(reason)) => {
                let _ = send_signed_in(sender, Err(reason));
                return;
            }
            Err(error) if error.retryable() => {
                // A dropped connection is not a failed sign-in: back off and
                // keep asking, exactly as Prism does.
                interval = (interval * 2).min(MAX_POLL_INTERVAL_SECS);
            }
            Err(error) => {
                let _ = send_signed_in(sender, Err(error.to_string()));
                return;
            }
        }
    }
}

/// Longest a sign-in poll waits between attempts after repeated failures.
const MAX_POLL_INTERVAL_SECS: u64 = 30;

/// Send one message to the GUI, waiting while the channel is full.
///
/// `false` means the dialog is gone; every caller treats that as "stop".
fn send_message(sender: &mut futures::channel::mpsc::Sender<Message>, message: Message) -> bool {
    let mut pending = Some(message);
    loop {
        let message = match pending.take() {
            Some(message) => message,
            None => return true,
        };
        match sender.try_send(message) {
            Ok(()) => return true,
            Err(error) if error.is_full() => {
                pending = Some(error.into_inner());
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => return false,
        }
    }
}

/// Deliver the sign-in outcome.
fn send_signed_in(
    sender: &mut futures::channel::mpsc::Sender<Message>,
    result: Result<AccountEntry, String>,
) -> bool {
    send_message(sender, Message::MicrosoftDone(Box::new(result)))
}

/// Frame ticks for the page's scroll tween, while one is running.
///
/// A thread and a channel rather than `iced::time::every`, which needs a
/// futures-runtime feature this build does not enable — and enabling it would
/// put an async runtime in the process to deliver a 16-millisecond sleep this
/// shell can do with `std::thread`. Every other background job here is driven
/// the same way (see `one_shot`).
///
/// The subscription's life is the animation's: it is only asked for while
/// something is moving, and when the app stops asking iced drops the receiver,
/// the next send reports the channel is gone, and the thread ends with it.
fn frame_ticks() -> Subscription<Message> {
    iced::subscription::channel(FRAME_ID, 4, |mut sender| async move {
        let _ = std::thread::spawn(move || loop {
            std::thread::sleep(scroll::FRAME);
            match sender.try_send(Message::PageScrollTick) {
                Ok(()) => {}
                // Full means the UI is a frame or two behind. Frames are
                // droppable, so the animation simply skips ahead.
                Err(error) if error.is_full() => {}
                // Closed: nothing is animating any more.
                Err(_) => break,
            }
        });
        loop {
            futures::future::pending::<()>().await;
        }
    })
}

/// The instance-list entry for an open instance (used when a freshly created
/// instance is not in the cached card list yet).
pub fn entry_for(instance: &Instance) -> prism_gui::InstanceEntry {
    prism_gui::InstanceEntry {
        id: instance.id(),
        name: instance.name(),
        icon: instance.icon_key(),
        group: None,
        playtime_secs: instance.total_time_played_secs(),
    }
}

/// Resolve and download a project's best file for the target instance.
fn install_into(
    paths: &PrismPaths,
    cards: &[InstanceCard],
    selected: Option<&str>,
    project: &str,
    title: &str,
    content_type: ContentType,
) -> Result<String, String> {
    let id = selected.ok_or_else(|| "no instance selected".to_string())?;
    let (game, loader) = match cards.iter().find(|card| card.id == id) {
        Some(card) => (card.mc_version.clone(), card.loader),
        None => {
            let instance = Instance::open(&paths.configured_instances_dir().join(id))
                .map_err(|error| format!("cannot open '{id}': {error}"))?;
            let card = instances::summarize(&paths.configured_instances_dir(), &entry_for(&instance));
            (card.mc_version, card.loader)
        }
    };
    if content_type.needs_loader() && !loader.loads_mods() {
        return Err(format!(
            "'{id}' is vanilla — install a loader before adding mods"
        ));
    }
    let client = browse::client()?;
    let versions = browse::project_versions(&client, project)?;
    let version = browse::pick_version(&versions, &game, loader).ok_or_else(|| {
        format!("no {title} release matches {} {}", loader.label(), game)
    })?;
    let target_dir = paths
        .configured_instances_dir()
        .join(id)
        .join(content_type.target_folder());
    let installed = browse::install_version(&client, &target_dir, version)?;
    let note = if installed.verified {
        "sha1 verified"
    } else {
        "size checked"
    };
    Ok(format!(
        "Installed {} ({}) into '{id}' — {note}",
        installed.filename,
        installed_bytes(installed.bytes)
    ))
}

/// Human byte size for status lines.
fn installed_bytes(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.0} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prism_core::settings::Settings;

    #[test]
    fn the_title_bar_needs_two_quick_presses_to_maximize() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        assert!(!app.maximized);

        // First press arms a drag and leaves the size alone.
        let _ = app.update(Message::BarPressed(BarArea::Middle));
        assert!(!app.maximized, "one press is a drag, not a maximize");

        // A second press inside the double-click window maximizes.
        let _ = app.update(Message::BarPressed(BarArea::Middle));
        assert!(app.maximized);

        // A third starts a fresh gesture rather than flipping straight back, so
        // a triple-click cannot land the window in the state it started in.
        let _ = app.update(Message::BarPressed(BarArea::Middle));
        assert!(app.maximized, "the third press begins a new drag");
        let _ = app.update(Message::BarPressed(BarArea::Middle));
        assert!(!app.maximized, "the fourth is the second half of a new double-click");
    }

    #[test]
    fn two_slow_presses_are_two_drags() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        let _ = app.update(Message::BarPressed(BarArea::Middle));
        // Backdate the remembered press so the next one falls outside the
        // double-click window, the way a slow double-click does.
        app.last_bar_press = Some(Instant::now() - DOUBLE_CLICK - Duration::from_millis(1));
        let _ = app.update(Message::BarPressed(BarArea::Middle));
        assert!(
            !app.maximized,
            "a slow second press drags again instead of maximizing"
        );
    }

    #[test]
    fn a_press_arms_a_drag_without_starting_one() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        let _ = app.update(Message::BarPressed(BarArea::Middle));
        assert_eq!(app.bar_armed, Some(BarArea::Middle));
        assert_eq!(app.bar_origin, None, "no origin until the pointer moves");
    }

    #[test]
    fn a_click_that_jitters_under_the_threshold_stays_a_click() {
        // The whole reason the threshold exists: a press that wobbles a pixel
        // or two and comes back up must never enter Windows' move loop, because
        // that loop eats the second press of a double-click.
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        let _ = app.update(Message::BarPressed(BarArea::Middle));
        let _ = app.update(Message::BarCursorMoved(BarArea::Middle, Point::new(100.0, 10.0)));
        let _ = app.update(Message::BarCursorMoved(
            BarArea::Middle,
            Point::new(100.0 + BAR_DRAG_THRESHOLD - 0.5, 10.0),
        ));
        assert_eq!(
            app.bar_armed,
            Some(BarArea::Middle),
            "jitter must not be mistaken for a drag"
        );

        let _ = app.update(Message::BarReleased(BarArea::Middle));
        assert_eq!(app.bar_armed, None, "the release stands the arm down");
    }

    #[test]
    fn moving_past_the_threshold_spends_the_arm() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        let _ = app.update(Message::BarPressed(BarArea::Middle));
        let _ = app.update(Message::BarCursorMoved(BarArea::Middle, Point::new(100.0, 10.0)));
        let _ = app.update(Message::BarCursorMoved(
            BarArea::Middle,
            Point::new(100.0 + BAR_DRAG_THRESHOLD, 10.0),
        ));
        assert_eq!(app.bar_armed, None, "a real drag must stand the arm down");
        assert_eq!(app.bar_origin, None);
    }

    #[test]
    fn a_move_reported_for_another_patch_cannot_start_the_drag() {
        // The patches are separate widgets with separate local origins, so a
        // position from one says nothing about how far the pointer has actually
        // travelled since a press in another.
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        let _ = app.update(Message::BarPressed(BarArea::Brand));
        let _ = app.update(Message::BarCursorMoved(BarArea::Middle, Point::new(900.0, 900.0)));
        assert_eq!(
            app.bar_armed,
            Some(BarArea::Brand),
            "a move outside the pressed patch must not start a drag"
        );
    }

    #[test]
    fn the_second_press_of_a_double_click_arms_nothing() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        let _ = app.update(Message::BarPressed(BarArea::Middle));
        let _ = app.update(Message::BarPressed(BarArea::Middle));
        assert!(app.maximized);
        assert_eq!(app.bar_armed, None, "maximizing must not leave a drag armed");
        assert_eq!(app.bar_origin, None);
    }

    #[test]
    fn the_caption_button_follows_the_real_window_state() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        // The window reports its own state at startup, which is what lets the
        // caption button offer Restore on a window that opened maximized.
        let _ = app.update(Message::MaximizedChanged(true));
        assert!(app.maximized);
        let _ = app.update(Message::WindowMaximize);
        assert!(!app.maximized, "clicking the caption toggles back to windowed");
    }

    #[test]
    fn every_resize_edge_is_handled_without_a_window() {
        // All eight grips exist and their messages are processed headlessly. On
        // Windows each call hands off to the OS; elsewhere it is a no-op. What
        // must never happen is a panic from a missing edge.
        assert_eq!(crate::native::ResizeEdge::ALL.len(), 8);
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        for edge in crate::native::ResizeEdge::ALL {
            let _ = app.update(Message::ResizeStart(edge));
        }
        // And the bar's right-click reaches the native menu request.
        let _ = app.update(Message::BarRightClick);
    }

    #[test]
    fn the_published_maximize_region_is_the_button_the_bar_draws() {
        // The shim turns exactly this rectangle into non-client area, so a
        // number that disagrees with the layout is either a maximize button
        // that cannot be clicked or a dead hole in the middle of the bar.
        let target = caption_target();
        assert_eq!(target.width, CAPTION_GLYPH + 2.0 * CAPTION_PAD[1]);
        assert_eq!(target.bottom - target.top, CAPTION_GLYPH + 2.0 * CAPTION_PAD[0]);
        // Spelled out as well, so a change to the constants has to be meant.
        assert_eq!(target.width, 32.0, "the caption button's width");
        assert_eq!(target.bottom - target.top, 24.0, "the caption button's height");

        // It is the second control in from the right: the frame band, the bar's
        // padding, the close control, and one gap.
        assert_eq!(
            target.right_inset,
            native::RESIZE_BAND
                + title_bar_padding().right
                + CAPTION_BUTTON_WIDTH
                + TITLE_BAR_SPACING
        );
        assert_eq!(target.right_inset, 54.0, "six rows of frame plus 48 of bar");
    }

    #[test]
    fn the_maximize_region_sits_inside_the_bars_content_band() {
        // Below the frame band and inside the bar's padding on both sides, so
        // the region covers the button and nothing else in the bar.
        let target = caption_target();
        let content_top = native::RESIZE_BAND + TITLE_BAR_PAD;
        let content_bottom = native::RESIZE_BAND + TITLE_BAR_HEIGHT - TITLE_BAR_PAD;
        assert!(target.top >= content_top, "top {} is above the content band", target.top);
        assert!(target.bottom <= content_bottom, "bottom {} overflows the bar", target.bottom);
        // Centred in it, which is where the bar puts a control of this height.
        assert!((target.top - content_top - (content_bottom - target.bottom)).abs() < 0.01);
    }

    #[test]
    fn the_frame_band_never_wins_over_the_maximize_button() {
        // `native::hit_code` answers the frame edges first, so a button that
        // reached into the band would be swallowed by the frame instead of
        // being answered as a button — and Windows would resize the window
        // instead of offering Snap Layouts.
        let target = caption_target();
        assert!(target.top >= native::RESIZE_BAND, "the button starts inside the frame");
        assert!(
            target.right_inset >= native::RESIZE_BAND,
            "the button ends inside the frame"
        );
    }

    #[test]
    fn the_shim_answers_the_maximize_button_and_leaves_its_neighbours_alone() {
        // The one test that runs the two modules together: what the title bar
        // publishes has to come back out of the window's hit test as the button
        // Windows is looking for.
        let target = caption_target();
        let (width, height) = (1257.0, 707.0);
        let mid_y = (target.top + target.bottom) / 2.0;
        let (left, right) = target.x_span(width);
        let mid_x = (left + right) / 2.0;
        assert_eq!(
            native::hit_code(mid_x, mid_y, width, height, Some(target), false),
            Some(native::HTMAXBUTTON)
        );

        // The close control is one gap further in, and stays the bar's own: it
        // keeps the red hover and the click iced draws for it.
        let close_centre = width
            - native::RESIZE_BAND
            - title_bar_padding().right
            - CAPTION_BUTTON_WIDTH / 2.0;
        assert_eq!(
            native::hit_code(close_centre, mid_y, width, height, Some(target), false),
            Some(native::HTCLIENT),
            "the close control must stay the bar's to handle"
        );
        // As does the bar around the button.
        assert_eq!(
            native::hit_code(left - 20.0, mid_y, width, height, Some(target), false),
            Some(native::HTCLIENT)
        );
    }

    #[test]
    fn the_maximize_region_tracks_the_window_edge_as_it_resizes() {
        // The region is measured from the client's right edge, so it follows the
        // control as the window is resized instead of having to be recomputed.
        let target = caption_target();
        for width in [980.0, 1257.0, 1920.0, 2560.0] {
            let (left, right) = target.x_span(width);
            assert_eq!(width - right, target.right_inset, "at {width} wide");
            assert_eq!(right - left, target.width, "at {width} wide");
            assert!(left > 0.0, "the region fell off the left of a {width}px window");
        }
    }

    #[test]
    fn a_window_state_report_does_not_invent_a_caption_state() {
        // The arm refreshes the tracked flag from the window itself. With no
        // window to ask — a test process, or the frames before the shim has
        // found one — the last known state has to stand rather than being
        // invented, or a maximized window's caption would flip on a report.
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        let _ = app.update(Message::MaximizedChanged(true));
        let _ = app.update(Message::WindowStateChanged);
        assert!(app.maximized, "nothing was there to contradict it");
        assert!(app.window_is_maximized());
    }

    #[test]
    fn the_caption_falls_back_to_the_tracked_state_without_a_window() {
        // Nothing to ask in a test process, which is also the state of the first
        // frames before the shim has found the window: the glyph then follows
        // what the app was last told.
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        assert!(!app.window_is_maximized());
        let _ = app.update(Message::MaximizedChanged(true));
        assert!(
            app.window_is_maximized(),
            "the tracked state answers when there is no window to ask"
        );
    }

    /// Hold the theme lock, surviving a panic in another test.
    fn theme_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::theme::THEME_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn the_gear_opens_the_launcher_settings_dialog() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        // Opening it is a dialog, not a page: the reference client's gear does
        // the same, and per-instance settings live on the instance's card.
        let _ = app.update(Message::OpenSettings);
        assert_eq!(app.modal(), &Modal::Settings);
        assert!(app.modal().is_open());
        {
            // The view is built and dropped inside its own scope: it borrows
            // the app, and the next message needs it back.
            let dialog: Element<'_, Message> = app.view();
            let _ = dialog;
        }
        let _ = app.update(Message::CloseModal);
        assert_eq!(app.modal(), &Modal::None);
    }

    #[test]
    fn choosing_a_color_theme_applies_it_and_records_it() {
        let _guard = theme_lock();
        let original = theme::color_theme();
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths.clone());
        let _ = app.update(Message::OpenSettings);

        for theme in [ColorTheme::Oled, ColorTheme::Light, ColorTheme::System] {
            let _ = app.update(Message::SetColorTheme(theme));
            assert_eq!(theme::color_theme(), theme, "the chosen theme is not in force");
            assert_eq!(prefs::load(&paths).theme(), theme, "the choice was not remembered");
            assert!(
                app.status().contains(theme.label()),
                "the status line should name the theme: {}",
                app.status()
            );
        }

        // The dialog stays open across the choice, so a second one is one click
        // away rather than a reopen.
        assert_eq!(app.modal(), &Modal::Settings);
        // Leave the process as it was found: the palette is global.
        let _ = app.update(Message::SetColorTheme(original));
    }

    #[test]
    fn a_wheel_notch_eases_the_page_instead_of_jumping() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        let _ = app.update(Message::PageWheel(scroll::Wheel {
            notches: -1.0,
            content_height: 3000.0,
            view_height: 700.0,
        }));

        // The target moved by exactly one notch; the offset has not moved at
        // all, which is the difference between a jump and a glide.
        assert_eq!(app.scroll_state().target, scroll::WHEEL_PIXELS_PER_NOTCH);
        assert_eq!(app.scroll_state().offset, 0.0);
        assert!(app.scroll_state().animating());

        // Frames now carry it there, and it lands exactly on the target rather
        // than near it — which is what lets the frame subscription stop.
        //
        // The clock is the test's rather than the wall's, so what is counted
        // here is the policy's frame budget and not how fast this machine
        // happens to be: a tween whose length depended on the test box would be
        // a tween that behaves differently on the reviewer's.
        let mut frames = 0;
        let mut now = Instant::now();
        while app.scroll_state().animating() {
            app.page_scroll.tick(now);
            now += scroll::FRAME;
            frames += 1;
            assert!(frames < 40, "the tween must finish in well under a second");
        }
        assert_eq!(app.scroll_state().offset, scroll::WHEEL_PIXELS_PER_NOTCH);
        assert!(frames >= 5, "a single frame would be the jump we are replacing");
    }

    #[test]
    fn a_foreign_scroll_is_adopted_rather_than_fought() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        let _ = app.update(Message::PageWheel(scroll::Wheel {
            notches: -1.0,
            content_height: 3000.0,
            view_height: 700.0,
        }));
        let _ = app.update(Message::PageScrollTick);

        // The user grabbed the scrollbar and dragged it somewhere else.
        let _ = app.update(Message::PageScrolled {
            offset: 1400.0,
            content_height: 3000.0,
            view_height: 700.0,
        });
        assert_eq!(app.scroll_state().offset, 1400.0);
        assert_eq!(app.scroll_state().target, 1400.0, "the tween must not drag it back");
        assert!(!app.scroll_state().animating(), "nothing left to animate");
    }

    #[test]
    fn switching_pages_forgets_the_previous_scroll() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        let _ = app.update(Message::PageWheel(scroll::Wheel {
            notches: -4.0,
            content_height: 3000.0,
            view_height: 700.0,
        }));
        assert!(app.scroll_state().animating());
        let _ = app.update(Message::PageSelected(Page::Browse));
        assert_eq!(app.scroll_state(), scroll::ScrollAnim::default());
    }

    #[test]
    fn every_theme_can_be_previewed_in_the_appearance_pane() {
        let _guard = theme_lock();
        let original = theme::color_theme();
        // Building the pane is the test: each card paints its own palette, so
        // a theme whose colors were never resolved would fail to construct.
        // The card's own geometry is checked in `settings`, where the pane now
        // lives; this is the app-level check that the dialog reaches it.
        for theme in ColorTheme::ALL {
            theme::set_color_theme(theme);
            let (_dir, paths) = test_paths();
            let mut app = PrismApp::with_paths(paths);
            let _ = app.update(Message::OpenSettings);
            let _ = app.update(Message::OpenSettingsTab(settings::Tab::Appearance));
            let dialog: Element<'_, Message> = app.view_settings_dialog();
            let _ = dialog;
        }
        theme::set_color_theme(original);
    }

    #[test]
    fn every_settings_tab_opens_a_pane_without_a_panic() {
        // The dialog's whole surface, reached through the real messages rather
        // than by calling the pane directly: a tab that is in the list but not
        // in `pane`'s match, or a pane that panics on a fresh profile, is a
        // blank dialog for the user and nothing at all in the type system.
        let _guard = theme_lock();
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        let _ = app.update(Message::OpenSettings);
        for tab in settings::Tab::ALL {
            let _ = app.update(Message::OpenSettingsTab(tab));
            assert_eq!(app.settings_tab, tab, "{} did not open", tab.label());
            let dialog: Element<'_, Message> = app.view_settings_dialog();
            let _ = dialog;
        }
        // And the hidden one is reachable, but only once developer mode is on.
        let _ = app.update(Message::OpenSettingsTab(settings::Tab::FeatureFlags));
        for _ in 0..6 {
            let _ = app.update(Message::SettingsFooterPressed);
        }
        assert!(app.prefs.developer_mode, "six presses should reveal the hidden tab");
        assert_eq!(app.settings_tab, settings::Tab::FeatureFlags);
        // Hiding it again must not leave the hidden pane on screen.
        for _ in 0..6 {
            let _ = app.update(Message::SettingsFooterPressed);
        }
        assert!(!app.prefs.developer_mode);
        assert_eq!(app.settings_tab, settings::Tab::Appearance, "the hidden pane stayed open");
    }

    #[test]
    fn a_switch_flips_its_setting_and_remembers_it() {
        // The whole path a click takes: the message, the cached prefs, the disk,
        // and the animation that draws it. A switch that only moved would pass
        // any check that looked at the view alone.
        let _guard = theme_lock();
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths.clone());
        assert!(app.prefs.show_files_tab, "the shipped default");

        let _ = app.update(Message::ToggleFlag(settings::Flag::ShowFilesTab));
        assert!(!app.prefs.show_files_tab);
        assert!(!prefs::load(&paths).show_files_tab, "the change did not reach the disk");
        assert!(app.switches.animating(), "the switch should be sliding");

        // And it survives a restart, which is the only thing that makes it a
        // preference rather than a temporary lie.
        let reopened = PrismApp::with_paths(paths);
        assert!(!reopened.prefs.show_files_tab);
    }

    #[test]
    fn a_switch_that_hides_a_page_leaves_it_before_it_can_hide_it() {
        // The failure this pins: turning the Worlds entry off *while on*
        // Worlds would leave the shell drawing a page whose only route was the
        // rail button that just went, with no way back to anything else.
        let _guard = theme_lock();
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);

        let _ = app.update(Message::PageSelected(Page::Worlds));
        assert_eq!(app.page, Page::Worlds);
        assert!(app.rail_shows(Page::Worlds));

        let _ = app.update(Message::ToggleFlag(settings::Flag::ShowWorldsTab));
        assert!(!app.rail_shows(Page::Worlds));
        assert_eq!(app.page, Page::Home, "the hidden page stayed on screen");
        // The switch next to it is not this switch's business.
        assert!(app.rail_shows(Page::Screenshots));

        // And the entry comes back with the switch, which is what makes the
        // fallback a detour rather than a one-way door.
        let _ = app.update(Message::ToggleFlag(settings::Flag::ShowWorldsTab));
        assert!(app.rail_shows(Page::Worlds));
    }

    #[test]
    fn the_card_switches_remove_exactly_what_they_say_they_remove() {
        // The switches reach the card through `CardChrome`, so what "compact"
        // takes away is a claim that can be read here instead of only being
        // visible in a screenshot.
        let shipped = CardChrome::from_prefs(&Prefs::default());
        assert!(!shipped.shows(CardChip::PlayTime), "playtime ships hidden");
        assert!(shipped.shows(CardChip::Mods));

        let played = CardChrome { show_play_time: true, compact: false };
        assert!(played.shows(CardChip::PlayTime));
        assert!(played.shows(CardChip::Loader) && played.shows(CardChip::Group));

        let compact = CardChrome { show_play_time: false, compact: true };
        for chip in [CardChip::Mods, CardChip::Loader, CardChip::Group] {
            assert!(!compact.shows(chip), "compact left {chip:?} on the card");
        }
        // Compact is not "everything off": the playtime chip belongs to the
        // other switch and must survive on its own.
        let both = CardChrome { show_play_time: true, compact: true };
        assert!(both.shows(CardChip::PlayTime));
    }

    #[test]
    fn a_settings_field_keeps_what_is_being_typed_and_writes_what_parses() {
        // The draft is what is on screen; the setting is what has been written.
        // Without the draft, clearing a number to retype it would put the old
        // value back under the caret.
        let _guard = theme_lock();
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths.clone());

        let _ = app.update(Message::SettingsDraft(settings::Field::ConcurrentDownloads, String::new()));
        assert_eq!(
            app.settings_drafts.get(&settings::Field::ConcurrentDownloads).map(String::as_str),
            Some(""),
            "an empty field must show as empty, not snap back to the default"
        );
        assert_eq!(
            app.prefs.concurrent_downloads(),
            prefs::DEFAULT_CONCURRENT_DOWNLOADS,
            "an empty field must not have been written"
        );

        let _ = app.update(Message::SettingsDraft(
            settings::Field::ConcurrentDownloads,
            "9".to_string(),
        ));
        assert_eq!(app.prefs.concurrent_downloads(), 9);
        assert_eq!(prefs::load(&paths).concurrent_downloads(), 9, "it did not reach the disk");
    }

    #[test]
    fn the_settings_pane_has_its_own_scroll() {
        // The pane sits over a page that is still there. Sharing one offset
        // would open the pane wherever the library was left and leave the
        // library wherever the pane ended up.
        let _guard = theme_lock();
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);

        let _ = app.update(Message::PageWheel(scroll::Wheel {
            notches: -3.0,
            content_height: 4_000.0,
            view_height: 400.0,
        }));
        assert!(app.scroll_state().animating());

        let _ = app.update(Message::SettingsWheel(scroll::Wheel {
            notches: -1.0,
            content_height: 4_000.0,
            view_height: 400.0,
        }));
        assert!(app.settings_scroll.animating());

        // Opening a tab starts the pane at the top, and leaves the page alone.
        let page_before = app.scroll_state().target;
        let _ = app.update(Message::OpenSettingsTab(settings::Tab::Java));
        assert_eq!(app.settings_scroll.offset, 0.0, "the pane did not start at the top");
        assert_eq!(app.settings_scroll.target, 0.0);
        assert!(!app.settings_scroll.animating());
        assert_eq!(app.scroll_state().target, page_before, "the page scroll moved with the pane");
    }

    fn test_paths() -> (tempfile::TempDir, PrismPaths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = PrismPaths::at(dir.path());
        std::fs::create_dir_all(paths.instances_dir()).unwrap();
        (dir, paths)
    }

    fn catalog_with(game: &[&str], fabric: &[(&str, bool, &str)]) -> VersionCatalog {
        use prism_core::resolve::VersionEntry;
        let mut catalog = VersionCatalog::default();
        catalog.game = game
            .iter()
            .map(|version| VersionEntry {
                uid: catalog::MINECRAFT_UID.to_string(),
                version: version.to_string(),
                type_: "release".to_string(),
                recommended: *version == "26.2",
                release_time: format!("2026-0{}-01T00:00:00+00:00", version.len()),
                ..Default::default()
            })
            .collect();
        catalog.loaders.insert(
            LoaderKind::Fabric,
            fabric
                .iter()
                .map(|(version, recommended, time)| VersionEntry {
                    uid: "net.fabricmc.fabric-loader".to_string(),
                    version: version.to_string(),
                    type_: "release".to_string(),
                    recommended: *recommended,
                    release_time: (*time).to_string(),
                    ..Default::default()
                })
                .collect(),
        );
        catalog
    }

    #[test]
    fn pages_cover_the_rail_and_titles() {
        // Nine pages: the six on the rail, plus the two reached from an
        // instance or a dialog (Settings, Accounts) and About.
        assert_eq!(Page::all().len(), 9);
        assert_eq!(Page::default(), Page::Home);
        assert_eq!(Page::rail().len(), 6);
        assert!(Page::rail().contains(&Page::Screenshots));
        // Each page draws its own symbol: a repeated or unknown key would make
        // two rail entries indistinguishable.
        let mut glyphs = Vec::new();
        for page in Page::all() {
            assert!(!page.title().is_empty());
            let glyph = crate::glyphs::Glyph::from_name(page.icon());
            assert!(!glyphs.contains(&glyph), "page '{}' repeats a glyph", page.title());
            glyphs.push(glyph);
        }
        assert_eq!(Page::Accounts.icon(), "person");
        assert_eq!(Page::Mods.icon(), "cube");
        assert_eq!(Page::Worlds.icon(), "globe");
    }

    #[test]
    fn modal_reports_open_state() {
        assert!(!Modal::None.is_open());
        assert!(Modal::Create.is_open());
        assert!(Modal::Import.is_open());
        assert!(Modal::ConfirmDelete("x".into()).is_open());
    }

    #[test]
    fn navigation_switches_pages_without_a_dialog() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        let _ = app.update(Message::PageSelected(Page::Browse));
        assert_eq!(app.page(), Page::Browse);
        let _ = app.update(Message::OpenCreate);
        assert_eq!(app.modal(), &Modal::Create);
        assert!(app.catalog_state().loading, "opening the dialog starts the catalog");
        let _ = app.update(Message::CloseModal);
        assert_eq!(app.modal(), &Modal::None);
    }

    #[test]
    fn create_button_lifecycle_opens_a_real_instance() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths.clone());
        app.catalog.loading = false;
        app.catalog.loaded_once = true;
        app.catalog.catalog = catalog_with(
            &["26.2", "1.21.1"],
            &[("0.19.5", true, "2026-08-28T11:01:04+00:00"), ("0.18.0", false, "2026-01-01T00:00:00+00:00")],
        );
        let _ = app.update(Message::OpenCreate);
        let _ = app.update(Message::CreateCustomSetup);
        assert_eq!(app.create_form().step, CreateStep::Configure);
        assert_eq!(app.create_form().game, "26.2", "recommended release is the default");
        assert_eq!(app.create_form().loader, LoaderKind::Vanilla);

        let _ = app.update(Message::CreateLoaderPicked(LoaderKind::Fabric));
        assert_eq!(app.create_form().build, "0.19.5", "Stable build comes from the catalog");
        let _ = app.update(Message::CreateBuildChoicePicked(BuildChoice::Latest));
        assert_eq!(app.create_form().build, "0.19.5");
        let _ = app.update(Message::CreateBuildChoicePicked(BuildChoice::Other));
        let _ = app.update(Message::CreateBuildPicked("0.18.0".to_string()));
        let _ = app.update(Message::CreateNameChanged("My Fabric Pack".to_string()));
        let _ = app.update(Message::CreateSubmit);
        assert_eq!(app.modal(), &Modal::None, "submit closes the dialog");
        assert_eq!(app.instance_count(), 1);
        assert_eq!(app.selected(), Some("My Fabric Pack"));

        let instance = Instance::open(&paths.instances_dir().join("My Fabric Pack")).unwrap();
        let profile = prism_core::pack::PackProfile::load(&instance.mmc_pack_path()).unwrap();
        assert_eq!(profile.get("net.fabricmc.fabric-loader").unwrap().version, "0.18.0");
        let card = app.selected_card().unwrap();
        assert_eq!(card.loader, LoaderKind::Fabric);
        assert_eq!(card.mc_version, "26.2");
    }

    #[test]
    fn create_requires_a_name_and_a_game_version() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        app.catalog.loaded_once = true;
        let _ = app.update(Message::OpenCreate);
        let _ = app.update(Message::CreateCustomSetup);
        // No catalog at all: nothing can be submitted.
        let _ = app.update(Message::CreateSubmit);
        assert!(app.create_form().error.is_some());
        assert_eq!(app.instance_count(), 0);
    }

    #[test]
    fn loader_without_builds_reports_it_instead_of_guessing() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        app.catalog.loaded_once = true;
        app.catalog.catalog = catalog_with(&["26.2"], &[]);
        let _ = app.update(Message::OpenCreate);
        let _ = app.update(Message::CreateCustomSetup);
        let _ = app.update(Message::CreateLoaderPicked(LoaderKind::Fabric));
        assert!(app.create_form().build.is_empty());
        assert!(app
            .create_form()
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("no builds"));
    }

    #[test]
    fn picking_a_project_prefills_the_configure_step() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        app.catalog.loaded_once = true;
        app.catalog.catalog = catalog_with(&["26.2"], &[("0.19.5", true, "2026-08-28T11:01:04+00:00")]);
        let _ = app.update(Message::OpenCreate);
        let _ = app.update(Message::CreateProjectPicked(
            "AANobbMI".to_string(),
            "Sodium".to_string(),
        ));
        assert_eq!(app.create_form().step, CreateStep::Configure);
        assert_eq!(app.create_form().name, "Sodium");
        assert_eq!(
            app.create_form().install_after.as_ref().map(|(p, _)| p.as_str()),
            Some("AANobbMI")
        );
    }

    #[test]
    fn create_search_results_are_dropped_when_stale() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        let _ = app.update(Message::OpenCreate);
        let _ = app.update(Message::CreateSearchChanged("sodium".to_string()));
        let seq = app.create_form().seq;
        let hit = Hit {
            project_id: "P".into(),
            slug: "sodium".into(),
            title: "Sodium".into(),
            description: String::new(),
            author: String::new(),
            downloads: 1,
            icon_url: String::new(),
            project_type: "mod".into(),
        };
        let _ = app.update(Message::TaskDone(Box::new(Task::CreateSearch {
            seq: seq + 5,
            result: Ok(vec![hit.clone()]),
        })));
        assert!(app.create_form().results.is_empty(), "stale results are ignored");
        let _ = app.update(Message::TaskDone(Box::new(Task::CreateSearch {
            seq,
            result: Ok(vec![hit]),
        })));
        assert_eq!(app.create_form().results.len(), 1);
        assert!(!app.create_form().searching);
    }

    #[test]
    fn catalog_results_drive_the_browse_and_create_state() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        let _ = app.update(Message::OpenCreate);
        let catalog = catalog_with(&["26.2"], &[("0.19.5", true, "2026-08-28T11:01:04+00:00")]);
        let _ = app.update(Message::TaskDone(Box::new(Task::Catalog(Box::new(catalog.clone())))));
        assert!(!app.catalog_state().loading);
        assert!(app.catalog_state().loaded_once);
        assert!(app.catalog_state().error.is_none());
        assert_eq!(app.create_form().game, "26.2");

        // An empty catalog is reported honestly.
        let _ = app.update(Message::TaskDone(Box::new(Task::Catalog(Box::new(VersionCatalog::default())))));
        assert!(app.catalog_state().error.is_some());
        assert!(app.status_is_error());
    }

    #[test]
    fn browse_search_and_install_are_sequenced() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        let _ = app.update(Message::BrowseQueryChanged("sodium".to_string()));
        assert!(app.browse_state().loading);
        let seq = app.browse_state().seq;
        let hit = Hit {
            project_id: "P".into(),
            slug: "sodium".into(),
            title: "Sodium".into(),
            description: "Fast".into(),
            author: "jellysquid3".into(),
            downloads: 10,
            icon_url: String::new(),
            project_type: "mod".into(),
        };
        let _ = app.update(Message::TaskDone(Box::new(Task::BrowseSearch {
            seq,
            result: Ok(vec![hit]),
        })));
        assert_eq!(app.browse_state().hits.len(), 1);
        assert!(!app.browse_state().loading);

        // Installing without an instance is refused with a reason.
        let _ = app.update(Message::BrowseInstall("P".to_string(), "Sodium".to_string()));
        assert!(app.status().contains("Pick an instance"), "status: {}", app.status());
        assert!(app.browse_state().installing.is_none());
    }

    #[test]
    fn installing_into_a_vanilla_instance_is_refused_before_any_network() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths.clone());
        let created = instances::create(&paths, &NewInstance::vanilla("Plain", "26.2")).unwrap();
        app.reload_instances();
        app.selected = Some(created.id.clone());
        let _ = app.update(Message::BrowseInstall("P".to_string(), "Sodium".to_string()));
        assert!(app.status().contains("vanilla"), "status: {}", app.status());
        assert!(app.browse_state().installing.is_none());
    }

    #[test]
    fn install_results_land_in_the_status_and_browse_state() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths.clone());
        let created = instances::create(
            &paths,
            &NewInstance {
                loader: LoaderKind::Fabric,
                loader_build: Some("0.19.5".to_string()),
                ..NewInstance::vanilla("Modded", "26.2")
            },
        )
        .unwrap();
        app.reload_instances();
        app.selected = Some(created.id.clone());
        let _ = app.update(Message::BrowseInstall("P".to_string(), "Sodium".to_string()));
        assert!(app.browse_state().installing.is_some());
        let seq = app.install_seq;
        let _ = app.update(Message::TaskDone(Box::new(Task::Installed {
            seq,
            title: "Sodium".to_string(),
            result: Ok("Installed sodium.jar into 'Modded' — sha1 verified".to_string()),
        })));
        assert!(app.browse_state().installing.is_none());
        assert!(app.status().contains("sha1 verified"));
        // Stale installs are ignored.
        let _ = app.update(Message::BrowseInstall("P".to_string(), "Sodium".to_string()));
        let _ = app.update(Message::TaskDone(Box::new(Task::Installed {
            seq: seq + 9,
            title: "Sodium".to_string(),
            result: Err("stale".to_string()),
        })));
        assert!(app.browse_state().installing.is_some(), "stale install result must be dropped");
    }

    #[test]
    fn dropping_files_routes_icons_and_packs() {
        let (dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        // With the Create dialog open, a PNG becomes the instance icon.
        let png = dir.path().join("avatar.png");
        std::fs::write(&png, [&[0x89u8, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A][..], b"data"].concat())
            .unwrap();
        let _ = app.update(Message::OpenCreate);
        let _ = app.update(Message::FileDropped(png.clone()));
        assert_eq!(app.create_form().icon_source.as_deref(), Some(png.as_path()));
        // Any other extension is refused with a reason.
        let weird = dir.path().join("notes.txt");
        std::fs::write(&weird, b"x").unwrap();
        let _ = app.update(Message::FileDropped(weird));
        assert!(app.status().contains("not used"), "status: {}", app.status());
    }

    #[test]
    fn random_icons_come_from_the_built_in_set() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        let _ = app.update(Message::OpenCreate);
        let _ = app.update(Message::CreateIconRandomize);
        assert!(ICON_CHOICES.contains(&app.create_form().icon_key.as_str()));
        let _ = app.update(Message::CreateIconPicked("creeper".to_string()));
        assert_eq!(app.create_form().icon_key, "creeper");
        let _ = app.update(Message::CreateIconCustomize);
        assert!(app.create_form().customize_open);
    }

    #[test]
    fn deleting_asks_first_and_then_removes_the_folder() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths.clone());
        let created = instances::create(&paths, &NewInstance::vanilla("Doomed", "26.2")).unwrap();
        app.reload_instances();
        app.selected = Some(created.id.clone());
        let _ = app.update(Message::AskDelete(created.id.clone()));
        assert_eq!(app.modal(), &Modal::ConfirmDelete(created.id.clone()));
        assert!(paths.instances_dir().join(&created.id).is_dir());
        let _ = app.update(Message::CloseModal);
        assert_eq!(app.modal(), &Modal::None);
        let _ = app.update(Message::AskDelete(created.id.clone()));
        let _ = app.update(Message::ConfirmDelete);
        assert!(!paths.instances_dir().join(&created.id).exists());
        assert_eq!(app.instance_count(), 0);
        assert!(app.selected().is_none());
    }

    #[test]
    fn editing_settings_round_trips_through_instance_cfg() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths.clone());
        let created = instances::create(&paths, &NewInstance::vanilla("Cfg", "26.2")).unwrap();
        app.reload_instances();
        let _ = app.update(Message::EditInstance(created.id.clone()));
        assert_eq!(app.page(), Page::Settings);
        let _ = app.update(Message::SetMinMem("256".to_string()));
        let _ = app.update(Message::SetMaxMem("bogus".to_string()));
        let _ = app.update(Message::SetOverrideMemory(true));
        let _ = app.update(Message::SetJavaPath("/opt/java".to_string()));
        let _ = app.update(Message::SetServerAddress("mc.example.com:25570".to_string()));
        let _ = app.update(Message::SetJoinServer(true));
        let _ = app.update(Message::SettingsSave);
        let back = Instance::open(&paths.instances_dir().join(&created.id)).unwrap();
        assert_eq!(back.settings().get_i64("MinMemAlloc", 0), 256);
        assert_eq!(back.settings().get_i64("MaxMemAlloc", 0), 4096, "invalid input keeps the old value");
        assert_eq!(back.settings().get_str("JavaPath", ""), "/opt/java");
        assert!(back.settings().get_bool("JoinServerOnLaunch", false));
        assert!(app.status().contains("Saved"));
    }

    #[test]
    fn accounts_add_select_and_remove() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths.clone());
        assert_eq!(app.account_label(), "No account");
        let _ = app.update(Message::AccountNameChanged("Steve".to_string()));
        let _ = app.update(Message::AccountAdd);
        assert_eq!(app.account_label(), "Steve");
        assert!(app.status().contains("Steve"));
        let uuid = app.accounts().selected_uuid().unwrap().to_string();
        let _ = app.update(Message::AccountRemove(uuid));
        assert_eq!(app.account_label(), "No account");

        // Microsoft sign-in opens the device-code dialog and starts the flow.
        let _ = app.update(Message::MicrosoftPressed);
        assert_eq!(app.modal(), &Modal::Microsoft);
        assert!(app.microsoft.pending, "the flow is running while the dialog is open");
        // Closing the dialog stops it.
        let _ = app.update(Message::CloseModal);
        assert!(!app.microsoft.pending);
        assert_eq!(app.modal(), &Modal::None);
        // The accounts file is real.
        assert!(paths.accounts_file().is_file());
    }

    #[test]
    fn launch_streaming_ignores_stale_runs() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths.clone());
        let created = instances::create(&paths, &NewInstance::vanilla("Runnable", "26.2")).unwrap();
        app.reload_instances();
        app.console.clear();
        let _ = app.update(Message::LaunchLog { run_id: 7, lines: vec!["x".to_string()] });
        assert!(app.console().is_empty());

        let _ = app.update(Message::PlayInstance(created.id.clone()));
        assert!(app.is_running());
        assert_eq!(app.page(), Page::Logs);
        let run_id = app.active_run.as_ref().unwrap().run_id;
        // `start_launch` already logged the request; only the new lines count.
        let before = app.console().len();
        let _ = app.update(Message::LaunchLog {
            run_id,
            lines: vec!["hello".to_string()],
        });
        assert_eq!(app.console().len(), before + 1);
        assert_eq!(app.console().back().unwrap(), "hello");
        let _ = app.update(Message::LaunchDone { run_id: run_id + 1, note: "stale".to_string() });
        assert!(app.is_running());
        let _ = app.update(Message::LaunchDone { run_id, note: "done".to_string() });
        assert!(!app.is_running());
        assert_eq!(app.status(), "done");
    }

    #[test]
    fn launching_without_a_selection_explains_itself() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        // Nothing selected at all: an empty id must not become instance `""`.
        let _ = app.update(Message::PlayInstance(String::new()));
        assert!(app.status().contains("Pick an instance"), "status: {}", app.status());
        assert!(app.status_is_error());
        assert!(!app.is_running());
        assert_eq!(app.selected, None);
    }

    #[test]
    fn an_empty_instance_id_plays_the_current_selection() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths.clone());
        let created = instances::create(&paths, &NewInstance::vanilla("Kept", "26.2")).unwrap();
        app.reload_instances();
        let _ = app.update(Message::SelectInstance(created.id.clone()));
        let _ = app.update(Message::PlayInstance(String::new()));
        assert!(app.is_running());
        assert_eq!(app.selected.as_deref(), Some(created.id.as_str()));
        assert_eq!(app.active_run.as_ref().unwrap().instance_id, created.id);
    }

    #[test]
    fn mods_toggle_on_disk_and_refresh_the_cache() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths.clone());
        let created = instances::create(&paths, &NewInstance::vanilla("Modded", "26.2")).unwrap();
        // Prism keeps the game folder (and therefore `mods/`) under `minecraft/`.
        let mods_dir = paths.instances_dir().join(&created.id).join("minecraft").join("mods");
        std::fs::create_dir_all(&mods_dir).unwrap();
        std::fs::write(mods_dir.join("sodium.jar"), b"jar").unwrap();
        app.reload_instances();
        app.selected = Some(created.id.clone());
        let _ = app.update(Message::SelectInstance(created.id.clone()));
        assert_eq!(app.mods().len(), 1);
        let _ = app.update(Message::ModToggled("sodium.jar".to_string(), false));
        assert!(!app.mods()[0].enabled);
        assert!(mods_dir.join("sodium.jar.disabled").is_file());
        assert!(app.status().contains("Disabled"));
    }

    #[test]
    fn grouping_moves_the_selection_and_persists() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths.clone());
        let created = instances::create(&paths, &NewInstance::vanilla("Movable", "26.2")).unwrap();
        app.reload_instances();
        app.selected = Some(created.id.clone());
        assert_eq!(app.selected_group_label(), UNGROUPED_LABEL);
        let _ = app.update(Message::GroupSelected("Packs".to_string()));
        assert_eq!(app.selected_group_label(), "Packs");
        assert!(app.group_options().contains(&"Packs".to_string()));
        let _ = app.update(Message::GroupSelected(UNGROUPED_LABEL.to_string()));
        assert_eq!(app.selected_group_label(), UNGROUPED_LABEL);
    }

    #[test]
    fn duplicating_copies_the_tree_and_selects_the_copy() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths.clone());
        let created = instances::create(&paths, &NewInstance::vanilla("Original", "26.2")).unwrap();
        std::fs::write(
            paths.instances_dir().join(&created.id).join("marker.txt"),
            b"keep",
        )
        .unwrap();
        app.reload_instances();
        let _ = app.update(Message::DuplicateInstance(created.id.clone()));
        assert_eq!(app.instance_count(), 2);
        let copy = app.selected().unwrap().to_string();
        assert_ne!(copy, created.id);
        assert!(paths.instances_dir().join(&copy).join("marker.txt").is_file());
    }

    #[test]
    fn import_scan_results_are_reported() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        let _ = app.update(Message::OpenImport);
        assert_eq!(app.modal(), &Modal::Import);
        assert!(app.import_state().loading);
        let _ = app.update(Message::TaskDone(Box::new(Task::ImportScan(Vec::new()))));
        assert!(!app.import_state().loading);
        assert!(app.import_state().scanned);
        assert!(app.import_state().error.is_some(), "an empty scan is explained, not hidden");
    }

    #[test]
    fn importing_a_candidate_copies_it_and_closes_the_dialog() {
        let (dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths.clone());
        let source = dir.path().join("Borrowed");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("instance.cfg"), b"[General]\nname=Borrowed\n").unwrap();
        let _ = app.update(Message::OpenImport);
        let _ = app.update(Message::TaskDone(Box::new(Task::ImportScan(vec![
            instances::ImportCandidate {
                name: "Borrowed".to_string(),
                source: source.clone(),
                origin: "Prism Launcher",
            },
        ]))));
        let _ = app.update(Message::ImportPicked(0));
        assert_eq!(app.modal(), &Modal::None);
        assert_eq!(app.selected(), Some("Borrowed"));
        assert!(paths.instances_dir().join("Borrowed").join("instance.cfg").is_file());
    }

    #[test]
    fn console_caps_and_clears() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths);
        app.console.clear();
        let lines: Vec<String> = (0..(CONSOLE_LINE_CAP + 5)).map(|i| format!("line {i}")).collect();
        app.push_console(lines);
        assert_eq!(app.console().len(), CONSOLE_LINE_CAP);
        assert_eq!(app.console().front().map(String::as_str), Some("line 5"));
        assert_eq!(console_shown_lines(1_000_000), CONSOLE_VIEW_LINES);
        let _ = app.update(Message::ConsoleClear);
        assert!(app.console().is_empty());
    }

    #[test]
    fn mem_parser_falls_back() {
        assert_eq!(parse_mem("8192", 4096), 8192);
        assert_eq!(parse_mem(" 2048 ", 0), 2048);
        assert_eq!(parse_mem("", 4096), 4096);
        assert_eq!(parse_mem("lots", 4096), 4096);
    }

    #[test]
    fn form_loads_prism_keys_with_defaults() {
        let settings = Settings::empty("instance.cfg");
        let form = load_form(&settings);
        assert_eq!(form.name, defaults::INSTANCE_NAME);
        assert_eq!(form.min_mem, "128");
        assert_eq!(form.max_mem, "4096");
        assert!(!form.override_memory);
        assert_eq!(form.win_width, "854");
        assert_eq!(form.win_height, "480");
    }

    #[test]
    fn create_form_invents_a_name_only_when_needed() {
        let mut form = CreateForm {
            loader: LoaderKind::Fabric,
            game: "26.2".to_string(),
            ..Default::default()
        };
        assert_eq!(form.effective_name(), "Fabric 26.2");
        form.loader = LoaderKind::Vanilla;
        assert_eq!(form.effective_name(), "26.2");
        form.name = "  Typed  ".to_string();
        assert_eq!(form.effective_name(), "Typed");
    }

    #[test]
    fn refresh_rescans_the_instances_folder() {
        let (_dir, paths) = test_paths();
        let mut app = PrismApp::with_paths(paths.clone());
        instances::create(&paths, &NewInstance::vanilla("Later", "26.2")).unwrap();
        assert_eq!(app.instance_count(), 0);
        let _ = app.update(Message::Refresh);
        assert_eq!(app.instance_count(), 1);
        assert_eq!(app.cards()[0].name, "Later");
        let _ = app.update(Message::SearchChanged("lat".to_string()));
        assert!(app.cards()[0].name.contains("Lat"));
    }

    #[test]
    fn shortcut_map_covers_n_and_escape() {
        use iced::keyboard::{key::Named, Key, Modifiers};
        assert!(matches!(
            shortcut(Key::Character("n".into()), Modifiers::default()),
            Some(Message::OpenCreate)
        ));
        assert!(matches!(
            shortcut(Key::Character("N".into()), Modifiers::default()),
            Some(Message::OpenCreate)
        ));
        assert!(matches!(
            shortcut(Key::Character("r".into()), Modifiers::CTRL),
            Some(Message::Refresh)
        ));
        assert!(matches!(
            shortcut(Key::Named(Named::Escape), Modifiers::default()),
            Some(Message::CloseModal)
        ));
        assert!(shortcut(Key::Named(Named::Enter), Modifiers::default()).is_none());
        assert!(shortcut(Key::Character("q".into()), Modifiers::default()).is_none());
    }

    #[test]
    fn byte_sizes_are_human_readable() {
        assert_eq!(installed_bytes(512), "512 B");
        assert_eq!(installed_bytes(2048), "2 KiB");
        assert_eq!(installed_bytes(3 * 1024 * 1024), "3.0 MiB");
    }
}
