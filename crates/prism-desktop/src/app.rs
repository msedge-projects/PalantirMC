//! Shared Prism desktop application core.
//!
//! [`PrismApp`] owns every piece of GUI state plus the synchronous update
//! logic and the `iced` view tree. Two thin shells reuse it (see `main.rs`):
//!
//! * `State` implements `iced::Sandbox` (the original API, kept working and
//!   fully synchronous: instances load during construction).
//! * `App` implements `iced::Application` (same iced 0.12 crate) so the
//!   launch log can stream through [`Subscription`] — something the
//!   `Sandbox` blanket impl cannot do (it hardcodes
//!   `Subscription::none()`). `App` starts from an instant placeholder
//!   ([`PrismApp::pending`]) and fills the instance grid from a background
//!   thread, so the window appears immediately even on slow disks.
//!
//! Layout mirrors real Prism Launcher 10.x: a compact top toolbar (Add
//! Instance, Folders, Settings, Help, Update + account chip), collapsible
//! group sections with a wrapped grid of instance icon tiles in the main
//! area, a right sidebar with the big icon tile and the vertical action
//! list, and a status bar. Icons are the real carved Prism art, embedded via
//! `include_bytes!` (see `crate::icons`): grid tiles and the sidebar paint
//! `iced::widget::Image` pixels, never letter tiles, with zero runtime IO.
//!
//! Background launching: [`PrismApp::update`] only records an [`ActiveRun`];
//! [`PrismApp::subscription`] exposes an
//! `iced::subscription::channel` keyed by that run id whose task spawns the
//! worker thread from `crate::launch` (the thread owns the futures sender
//! and `try_send`s `Vec<String>` batches through it). Instance loading uses
//! a second channel (fixed id [`LOAD_SUBSCRIPTION_ID`]) with the same
//! spawn-thread + `try_send` pattern.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use iced::widget::{
    button, checkbox, column, container, horizontal_rule, horizontal_space, pick_list, row,
    scrollable, text, text_input, Button, Image,
};
use iced::widget::image::Handle;
use iced::{Background, Border, Color, Element, Font, Length, Subscription, Theme};
use prism_core::{
    instance::{groups::Groups, Instance},
    pack::PackProfile,
    paths::PrismPaths,
    settings::{defaults, Settings},
};
use prism_gui::{InstanceEntry, InstanceListModel, SettingsModel};

use crate::accounts::AccountsStore;
use crate::icons::{instance_handle, ui_handle};
use crate::launch::{
    self, open_in_file_manager, prepare_launch, run_launch_worker, AccountRef, ActiveRunData, ChildSlot,
    LaunchParams,
};
use crate::mods::{list_content_names, list_mods, set_mod_enabled, ModEntry};

/// Maximum console lines kept in memory.
pub const CONSOLE_LINE_CAP: usize = 20_000;

/// How many of the newest console lines the view renders (perf bound; the
/// buffer itself keeps [`CONSOLE_LINE_CAP`]).
pub const CONSOLE_VIEW_LINES: usize = 500;

/// Fixed height of the embedded console pane (it lives inside the main
/// scrollable, so it must be bounded for `snap_to` autoscroll to work).
pub const CONSOLE_PANE_HEIGHT: f32 = 280.0;

/// Instance tile geometry: icon side and total tile width.
pub const TILE_BOX: f32 = 56.0;
/// Total tile width (box + padding + button chrome).
pub const TILE_WIDTH: f32 = 96.0;
/// Tiles per grid row (the "wrapped row" grid is emulated by chunking:
/// iced 0.12 has no flow layout, so rows of [`TILES_PER_ROW`] tiles wrap the
/// same way on any window wide enough for the main area).
pub const TILES_PER_ROW: usize = 4;
/// Big sidebar tile side.
pub const BIG_TILE: f32 = 96.0;
/// Right sidebar width.
pub const SIDEBAR_WIDTH: f32 = 210.0;

/// Label used for the ungrouped section and the pick-list entry.
pub const UNGROUPED_LABEL: &str = "Ungrouped";

/// Subscription id for the one-shot background instance load.
pub const LOAD_SUBSCRIPTION_ID: &str = "prism-desktop-instance-load";

/// Main-area pages (Console is the default landing page).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    /// Scrollable launch/game log.
    #[default]
    Console,
    /// `<game_root>/mods` management.
    Mods,
    /// `resourcepacks/` listing.
    ResourcePacks,
    /// `shaderpacks/` listing.
    ShaderPacks,
    /// `saves/` listing.
    Worlds,
    /// `mmc-pack.json` components.
    Version,
    /// Per-instance settings editor.
    Settings,
    /// Offline account management.
    Accounts,
    /// Version + paths.
    About,
}

impl Page {
    /// Human-readable title.
    pub fn title(&self) -> &'static str {
        match self {
            Page::Console => "Console",
            Page::Mods => "Mods",
            Page::ResourcePacks => "Resource Packs",
            Page::ShaderPacks => "Shader Packs",
            Page::Worlds => "Worlds",
            Page::Version => "Version",
            Page::Settings => "Settings",
            Page::Accounts => "Accounts",
            Page::About => "About",
        }
    }

    /// Tab label in the main area (`About` is reached through the `Help`
    /// toolbar button, like in real Prism).
    pub fn tab_label(&self) -> &'static str {
        match self {
            Page::About => "Help",
            other => other.title(),
        }
    }

    /// All pages in toolbar/sidebar order.
    pub fn all() -> [Page; 9] {
        [
            Page::Console,
            Page::Mods,
            Page::ResourcePacks,
            Page::ShaderPacks,
            Page::Worlds,
            Page::Version,
            Page::Settings,
            Page::Accounts,
            Page::About,
        ]
    }
}

/// Every interaction the GUI can produce.
#[derive(Debug, Clone)]
pub enum Message {
    /// Select an instance by folder id.
    SelectInstance(String),
    /// Sidebar search text.
    SearchChanged(String),
    /// Reload instances + selection caches.
    Refresh,
    /// Switch the main-area page.
    PageSelected(Page),
    /// Start launching the selected instance.
    LaunchPressed,
    /// Terminate the running child, if any.
    KillPressed,
    /// Show the inline Add Instance panel.
    AddInstancePressed,
    /// Add-panel name input.
    AddInstanceNameChanged(String),
    /// Add-panel Minecraft version input.
    AddInstanceVersionChanged(String),
    /// Create the instance from the add panel.
    AddInstanceCreate,
    /// Hide the add panel.
    AddInstanceCancel,
    /// Open the instance editor (Settings page).
    EditPressed,
    /// Reveal the selected instance (or data root) in the file manager.
    OpenFolderPressed,
    /// Delete the selected instance (two clicks to confirm).
    DeletePressed,
    /// Go to the Settings page.
    SettingsPressed,
    /// Go to the Accounts page.
    AccountsPressed,
    /// Go to the About page.
    AboutPressed,
    /// Clear the console buffer.
    ConsoleClear,
    /// Toggle console autoscroll.
    ConsoleAutoscrollToggled(bool),
    /// One streamed batch from the launch worker.
    LaunchLog {
        /// Run this batch belongs to (stale runs are ignored).
        run_id: u64,
        /// Log lines.
        lines: Vec<String>,
    },
    /// The launch worker finished.
    LaunchDone {
        /// Run that finished.
        run_id: u64,
        /// Final outcome line.
        note: String,
    },
    /// Background instance load finished.
    InstancesLoaded(LoadedData),
    /// Collapse/expand a group section (`""` = ungrouped).
    GroupToggled(String),
    /// Move the selected instance to a group (`Ungrouped` clears it).
    ChangeGroupSelected(String),
    /// Duplicate the selected instance folder under a unique name.
    CopyPressed,
    /// Export the selected instance (honestly unimplemented: zip export
    /// would need a new archiving dependency for a one-click feature).
    ExportPressed,
    /// Write a `.url` shortcut for the selected instance on the Desktop.
    ShortcutPressed,
    /// Check for updates (honest: updates ship via the installer).
    UpdatePressed,
    /// Reveal the instances folder in the file manager.
    FoldersPressed,
    /// Enable/disable a mod file.
    ModToggled(String, bool),
    /// Reveal the selected instance's mods folder.
    OpenModsFolderPressed,
    /// Settings form inputs.
    SetName(String),
    /// Settings form inputs.
    SetMinMem(String),
    /// Settings form inputs.
    SetMaxMem(String),
    /// Settings form inputs.
    SetOverrideMemory(bool),
    /// Settings form inputs.
    SetJavaPath(String),
    /// Settings form inputs.
    SetOverrideJava(bool),
    /// Settings form inputs.
    SetWinWidth(String),
    /// Settings form inputs.
    SetWinHeight(String),
    /// Settings form inputs.
    SetOverrideWindow(bool),
    /// Settings form inputs.
    SetJoinServer(bool),
    /// Settings form inputs.
    SetServerAddress(String),
    /// Persist the settings form to `instance.cfg`.
    SettingsSave,
    /// Accounts page username input.
    AccountNameChanged(String),
    /// Add the typed offline account.
    AccountAdd,
    /// Select an account by uuid.
    AccountSelect(String),
    /// Remove an account by uuid.
    AccountRemove(String),
    /// Microsoft login button (honestly unimplemented).
    MicrosoftPressed,
}

/// Result of the background instance scan, delivered as
/// [`Message::InstancesLoaded`]. Everything the grid needs, computed off the
/// GUI thread: the entry list, the group index (incl. collapsed flags), the
/// resolved instances dir and the pre-selected instance, if any.
#[derive(Debug, Clone)]
pub struct LoadedData {
    /// Discovered instances in display order.
    pub list: InstanceListModel,
    /// Group membership + collapsed flags.
    pub groups: Groups,
    /// Pre-selected instance id (from `SelectedInstance`, when it matches).
    pub selected: Option<String>,
    /// Resolved instances dir (`InstanceDir` override applied).
    pub instances_dir: PathBuf,
    /// One-line outcome for the status bar.
    pub status: String,
}

/// One grouped sidebar section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupSection {
    /// Group name (`None` = ungrouped).
    pub name: Option<String>,
    /// Entries in display order.
    pub entries: Vec<InstanceEntry>,
}

/// Group the entries of `model` (filtered by `query` like
/// [`InstanceListModel::filter`]) into named sections first (alphabetical)
/// and the ungrouped tail last.
pub fn grouped_instances(model: &InstanceListModel, query: &str) -> Vec<GroupSection> {
    let mut groups: BTreeMap<Option<String>, Vec<InstanceEntry>> = BTreeMap::new();
    for entry in model.filter(query) {
        groups.entry(entry.group.clone()).or_default().push(entry.clone());
    }
    let mut named: Vec<(String, Vec<InstanceEntry>)> = Vec::new();
    let mut ungrouped: Vec<InstanceEntry> = Vec::new();
    for (key, list) in groups {
        match key {
            Some(group) => named.push((group, list)),
            None => ungrouped = list,
        }
    }
    named.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()).then_with(|| a.0.cmp(&b.0)));
    let mut out: Vec<GroupSection> = Vec::new();
    for (group, list) in named {
        out.push(GroupSection { name: Some(group), entries: list });
    }
    if !ungrouped.is_empty() {
        out.push(GroupSection { name: None, entries: ungrouped });
    }
    out
}

/// Deterministic pastel palette for instance tiles (RGB triples).
pub const TILE_PALETTE: [(u8, u8, u8); 12] = [
    (0xF4, 0xB8, 0xC1),
    (0xF9, 0xD5, 0xA7),
    (0xF7, 0xE8, 0xA0),
    (0xB5, 0xE3, 0xB5),
    (0xA8, 0xD8, 0xF0),
    (0xC3, 0xB2, 0xE8),
    (0xE8, 0xB4, 0xD8),
    (0xB8, 0xE0, 0xD2),
    (0xF5, 0xC9, 0x9B),
    (0xC9, 0xD6, 0xF2),
    (0xD8, 0xE6, 0xA0),
    (0xEF, 0xC3, 0xA0),
];

/// Tile background for an instance id: FNV-1a hash into [`TILE_PALETTE`].
/// Pure and deterministic (same id, same color, every run).
pub fn tile_color(id: &str) -> Color {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in id.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let (red, green, blue) = TILE_PALETTE[(hash as usize) % TILE_PALETTE.len()];
    Color::from_rgb(
        f32::from(red) / 255.0,
        f32::from(green) / 255.0,
        f32::from(blue) / 255.0,
    )
}

/// Selected-tile background, Prism's instance-selection green (`#7CB342`).
pub fn selected_tile_color() -> Color {
    Color::from_rgb(0x7Cu8 as f32 / 255.0, 0xB3u8 as f32 / 255.0, 0x42u8 as f32 / 255.0)
}

/// Tile name label, truncated to two lines' worth of characters (char-safe,
/// never splits a code point).
pub fn short_name(name: &str) -> String {
    const LIMIT: usize = 30;
    let chars: Vec<char> = name.chars().collect();
    if chars.len() <= LIMIT {
        name.to_string()
    } else {
        let kept: String = chars[..LIMIT].iter().collect();
        format!("{kept}…")
    }
}

/// Recursively copy a directory tree (files only; symlinks and special
/// files are skipped, Prism never writes them inside an instance).
pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    let entries =
        std::fs::read_dir(src).map_err(|e| format!("reading '{}': {e}", src.display()))?;
    std::fs::create_dir_all(dst).map_err(|e| format!("creating '{}': {e}", dst.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("listing '{}': {e}", src.display()))?;
        let kind = entry
            .file_type()
            .map_err(|e| format!("stating '{}': {e}", entry.path().display()))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if kind.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else if kind.is_file() {
            std::fs::copy(&from, &to)
                .map_err(|e| format!("copying '{}': {e}", from.display()))?;
        }
    }
    Ok(())
}

/// Contents of a Windows `.url` shortcut pointing at `target`.
///
/// A `.url` file is plain INI: Explorer opens `file:///` URLs in the file
/// manager, so the shortcut reveals the instance folder. Backslashes are
/// normalized to forward slashes (URLs never contain `\`).
pub fn url_shortcut_text(target: &Path) -> String {
    let normalized = target.display().to_string().replace('\\', "/");
    let trimmed = normalized.trim_start_matches('/').to_string();
    format!("[InternetShortcut]\nURL=file:///{trimmed}\n")
}

/// Write a `.url` shortcut named `<file_stem>.url` into `dir`.
pub fn write_url_shortcut(dir: &Path, file_stem: &str, target: &Path) -> Result<PathBuf, String> {
    if file_stem.contains('/') || file_stem.contains('\\') {
        return Err(format!("refusing path-like shortcut name '{file_stem}'"));
    }
    let stem = prism_core::util::sanitize_dir_name(file_stem);
    let path = dir.join(format!("{stem}.url"));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("creating '{}': {e}", parent.display()))?;
    }
    std::fs::write(&path, url_shortcut_text(target))
        .map_err(|e| format!("writing '{}': {e}", path.display()))?;
    Ok(path)
}

/// The user's Desktop folder (`%USERPROFILE%\Desktop` on Windows,
/// `$HOME/Desktop` elsewhere). Returns `None` when no home is known.
pub fn desktop_dir() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())?;
    Some(home.join("Desktop"))
}

/// Pre-selection for startup: `SelectedInstance` from `prismlauncher.cfg`
/// when it matches a discovered id (stale values are ignored).
pub fn resolve_selected_id(paths: &PrismPaths, list: &InstanceListModel) -> Option<String> {
    let want = paths.selected_instance_id()?;
    if want.trim().is_empty() {
        return None;
    }
    if list.entries().iter().any(|entry| entry.id == want) {
        Some(want)
    } else {
        None
    }
}

/// Scan + parse every instance off the GUI thread (see [`LoadedData`]).
/// Failures degrade to an empty list plus a status message, never a crash.
pub fn load_instances_blocking(data_root: &Path) -> LoadedData {
    let paths = PrismPaths::at(data_root);
    let instances_dir = paths.configured_instances_dir();
    let groups = Groups::load(&paths);
    let list = InstanceListModel::load(&paths).unwrap_or_default();
    let selected = resolve_selected_id(&paths, &list);
    let status = if list.is_empty() {
        format!("No instances found in {}.", instances_dir.display())
    } else {
        format!("loaded {} instance(s) from {}", list.len(), instances_dir.display())
    };
    LoadedData { list, groups, selected, instances_dir, status }
}

/// Editable snapshot of the per-instance settings form (all text so empty
/// inputs are representable; parsed on save with fallbacks).
#[derive(Debug, Clone, Default)]
pub struct SettingsForm {
    /// Display name.
    pub name: String,
    /// `MinMemAlloc` text.
    pub min_mem: String,
    /// `MaxMemAlloc` text.
    pub max_mem: String,
    /// `OverrideMemory` gate.
    pub override_memory: bool,
    /// `JavaPath` text.
    pub java_path: String,
    /// `OverrideJavaLocation` gate.
    pub override_java: bool,
    /// `MinecraftWinWidth` text.
    pub win_width: String,
    /// `MinecraftWinHeight` text.
    pub win_height: String,
    /// `OverrideWindow` gate.
    pub override_window: bool,
    /// `JoinServerOnLaunch` (instance-only, no global gate in Prism).
    pub join_server: bool,
    /// `JoinServerOnLaunchAddress` text.
    pub server_address: String,
}

/// Read the form from instance settings.
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

/// Parse a memory/size field, keeping `fallback` for blank/invalid input.
pub fn parse_mem(text: &str, fallback: i64) -> i64 {
    match text.trim().parse::<i64>() {
        Ok(value) => value,
        Err(_) => fallback,
    }
}

/// How many of `total` buffered lines the view renders.
pub fn console_shown_lines(total: usize) -> usize {
    if total > CONSOLE_VIEW_LINES {
        CONSOLE_VIEW_LINES
    } else {
        total
    }
}

/// Identifier of the console scrollable (for autoscroll snap commands).
pub fn console_scroll_id() -> scrollable::Id {
    scrollable::Id::new("prism-console")
}

/// The full application state (runtime-agnostic).
pub struct PrismApp {
    paths: PrismPaths,
    /// Resolved instances dir, cached at (re)load so `view()` does no IO.
    instances_dir: PathBuf,
    list: InstanceListModel,
    groups: Groups,
    /// True while the background load is in flight (placeholder shown).
    loading: bool,
    search: String,
    selected: Option<String>,
    page: Page,
    console: VecDeque<String>,
    autoscroll: bool,
    status: String,
    show_add: bool,
    add_name: String,
    add_version: String,
    delete_armed: bool,
    form: SettingsForm,
    accounts: AccountsStore,
    account_input: String,
    run_seq: u64,
    active_run: Option<ActiveRunData>,
    child: ChildSlot,
    mods: Vec<ModEntry>,
    resource_packs: Vec<String>,
    shader_packs: Vec<String>,
    worlds: Vec<String>,
    components: Vec<(String, String)>,
}

impl PrismApp {
    /// Build against an explicit data root (used by `new` and tests).
    /// Synchronous: loads instances, groups and the `SelectedInstance`
    /// pre-selection before returning.
    pub(crate) fn with_paths(paths: PrismPaths) -> Self {
        let (accounts, accounts_warn) = AccountsStore::load_with_report(&paths.accounts_file());
        let mut app = PrismApp {
            instances_dir: paths.configured_instances_dir(),
            paths,
            list: InstanceListModel::default(),
            groups: Groups::default(),
            loading: false,
            search: String::new(),
            selected: None,
            page: Page::default(),
            console: VecDeque::new(),
            autoscroll: true,
            status: String::new(),
            show_add: false,
            add_name: String::new(),
            add_version: "1.21.1".to_string(),
            delete_armed: false,
            form: SettingsForm::default(),
            accounts,
            account_input: String::new(),
            run_seq: 0,
            active_run: None,
            child: Arc::new(Mutex::new(None)),
            mods: Vec::new(),
            resource_packs: Vec::new(),
            shader_packs: Vec::new(),
            worlds: Vec::new(),
            components: Vec::new(),
        };
        app.reload_instances();
        if app.selected.is_none() {
            app.selected = resolve_selected_id(&app.paths, &app.list);
            if app.selected.is_some() {
                app.refresh_selection_caches();
            }
        }
        if let Some(warn) = accounts_warn {
            app.status = warn.clone();
            app.push_console(vec![warn]);
        }
        app.push_console(vec![
            "Prism Launcher (Rust) console — launch output appears here.".to_string(),
        ]);
        app
    }

    /// Instant placeholder for the `Application` shell: no disk IO beyond
    /// resolving the data root, so the window can paint immediately. The
    /// background loader (see [`PrismApp::subscription`]) fills the grid.
    pub(crate) fn pending(paths: PrismPaths) -> Self {
        let (accounts, _) = AccountsStore::load_with_report(&paths.accounts_file());
        PrismApp {
            instances_dir: paths.configured_instances_dir(),
            paths,
            list: InstanceListModel::default(),
            groups: Groups::default(),
            loading: true,
            search: String::new(),
            selected: None,
            page: Page::default(),
            console: VecDeque::from(["Loading instances…".to_string()]),
            autoscroll: true,
            status: "Loading instances…".to_string(),
            show_add: false,
            add_name: String::new(),
            add_version: "1.21.1".to_string(),
            delete_armed: false,
            form: SettingsForm::default(),
            accounts,
            account_input: String::new(),
            run_seq: 0,
            active_run: None,
            child: Arc::new(Mutex::new(None)),
            mods: Vec::new(),
            resource_packs: Vec::new(),
            shader_packs: Vec::new(),
            worlds: Vec::new(),
            components: Vec::new(),
        }
    }

    /// Detect the data root like Prism does.
    pub fn new() -> Self {
        PrismApp::with_paths(PrismPaths::detect())
    }

    /// Window title.
    pub fn title(&self) -> String {
        "Prism Launcher (Rust)".to_string()
    }

    /// Whether console autoscroll is on.
    pub fn autoscroll_enabled(&self) -> bool {
        self.autoscroll
    }

    /// Display name of the selection (folder id fallback).
    pub fn selected_name(&self) -> String {
        match self.selected.as_deref() {
            Some(id) => match self.list.entries().iter().find(|e| e.id == id) {
                Some(entry) => entry.name.clone(),
                None => id.to_string(),
            },
            None => "none".to_string(),
        }
    }

    /// Account chip label for the toolbar (`👤 <name>` or the empty state).
    pub fn account_chip_label(&self) -> String {
        match self.accounts.selected_account() {
            Some(account) => format!("👤 {}", account.username),
            None => "👤 No account".to_string(),
        }
    }

    /// Current group of the selection (`Ungrouped` when there is none).
    pub fn selected_group_label(&self) -> String {
        match self.selected.as_deref() {
            Some(id) => match self.list.entries().iter().find(|e| e.id == id) {
                Some(entry) => entry.group.clone().unwrap_or_else(|| UNGROUPED_LABEL.to_string()),
                None => UNGROUPED_LABEL.to_string(),
            },
            None => UNGROUPED_LABEL.to_string(),
        }
    }

    /// Group options for the Change Group pick-list (`Ungrouped` first).
    pub fn group_options(&self) -> Vec<String> {
        let mut options = vec![UNGROUPED_LABEL.to_string()];
        for name in self.groups.names() {
            if name != UNGROUPED_LABEL {
                options.push(name.to_string());
            }
        }
        options
    }

    /// Append lines, enforcing [`CONSOLE_LINE_CAP`].
    pub fn push_console(&mut self, lines: Vec<String>) {
        for line in lines {
            let clean = match line.strip_suffix('\r') {
                Some(stripped) => stripped.to_string(),
                None => line,
            };
            self.console.push_back(clean);
        }
        while self.console.len() > CONSOLE_LINE_CAP {
            self.console.pop_front();
        }
    }

    /// Take the active run (used by the `Sandbox` shell for its honest
    /// synchronous dry run; the `Application` shell never calls this).
    pub(crate) fn take_active_run(&mut self) -> Option<ActiveRunData> {
        self.active_run.take()
    }

    /// Synchronous dry run for runtimes without subscriptions: resolve and
    /// report, never spawn.
    pub(crate) fn sandbox_drain_launch(&mut self) {
        let run = match self.take_active_run() {
            Some(run) => run,
            None => return,
        };
        let paths = PrismPaths::at(&run.data_root);
        let (lines, readiness) = prepare_launch(&paths, &run.instance_id, &run.account);
        let tail = match readiness {
            launch::LaunchReadiness::Ready(_) => {
                "dry run: launch looks runnable (live streaming needs the Application entrypoint)".to_string()
            }
            launch::LaunchReadiness::Blocked => "dry run: launch blocked (see console)".to_string(),
        };
        self.status = tail.clone();
        let mut all = lines;
        all.push(tail);
        self.push_console(all);
    }

    /// Subscriptions for the background instance load (while [`PrismApp::loading`])
    /// and the active launch run (keyed by run id), or none.
    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs: Vec<Subscription<Message>> = Vec::new();
        if self.loading {
            let root = self.paths.root.clone();
            subs.push(iced::subscription::channel(
                LOAD_SUBSCRIPTION_ID,
                16,
                move |sender| async move {
                    let _ = std::thread::spawn(move || {
                        run_instance_loader(root, sender);
                    });
                    loop {
                        futures::future::pending::<()>().await;
                    }
                },
            ));
        }
        if let Some(run) = self.active_run.clone() {
            let slot = self.child.clone();
            subs.push(iced::subscription::channel(run.run_id, 100, move |sender| async move {
                let params = LaunchParams {
                    data_root: run.data_root.clone(),
                    instance_id: run.instance_id.clone(),
                    account: run.account.clone(),
                    run_id: run.run_id,
                };
                let _ = std::thread::spawn(move || {
                    run_launch_worker(params, slot, sender);
                });
                loop {
                    futures::future::pending::<()>().await;
                }
            }));
        }
        Subscription::batch(subs)
    }

    /// Apply the background load result (selection caches follow).
    fn apply_loaded(&mut self, data: LoadedData) {
        self.loading = false;
        self.list = data.list;
        self.groups = data.groups;
        self.instances_dir = data.instances_dir;
        self.selected = data.selected;
        self.status = data.status;
        self.refresh_selection_caches();
    }

    /// Reload the instance list, pruning a vanished selection. Also refreshes
    /// the cached resolved instances dir and the group index.
    fn reload_instances(&mut self) {
        self.instances_dir = self.paths.configured_instances_dir();
        self.groups = Groups::load(&self.paths);
        match InstanceListModel::load(&self.paths) {
            Ok(list) => {
                let count = list.len();
                self.list = list;
                let keep = match self.selected.as_deref() {
                    Some(id) => self.list.entries().iter().any(|e| e.id == id),
                    None => false,
                };
                if !keep {
                    self.selected = None;
                }
                if count == 0 {
                    self.status = format!("No instances found in {}.", self.instances_dir.display());
                } else {
                    self.status = format!("refreshed ({count} instance(s))");
                }
            }
            Err(e) => {
                self.list = InstanceListModel::default();
                self.selected = None;
                self.status = format!("listing instances failed: {e}");
            }
        }
    }

    /// Reload everything derived from the selection (mods, content lists,
    /// version components, settings form).
    fn refresh_selection_caches(&mut self) {
        self.mods.clear();
        self.resource_packs.clear();
        self.shader_packs.clear();
        self.worlds.clear();
        self.components.clear();
        let id = match self.selected.clone() {
            Some(id) => id,
            None => {
                self.form = SettingsForm::default();
                return;
            }
        };
        let instance = match Instance::open(&self.instances_dir.join(&id)) {
            Ok(instance) => instance,
            Err(_) => {
                self.form = SettingsForm::default();
                return;
            }
        };
        let game_root = instance.game_root();
        self.mods = list_mods(&instance.mods_dir());
        self.resource_packs = list_content_names(&game_root.join("resourcepacks"));
        self.shader_packs = list_content_names(&game_root.join("shaderpacks"));
        self.worlds = list_content_names(&game_root.join("saves"));
        self.components = match PackProfile::load(&instance.mmc_pack_path()) {
            Ok(profile) => profile
                .components()
                .iter()
                .map(|c| (c.uid.clone(), c.version.clone()))
                .collect(),
            Err(_) => Vec::new(),
        };
        self.form = load_form(instance.settings());
    }

    /// Reload just the settings form for the selection.
    fn reload_form(&mut self) {
        let id = match self.selected.clone() {
            Some(id) => id,
            None => {
                self.form = SettingsForm::default();
                return;
            }
        };
        match Instance::open(&self.instances_dir.join(&id)) {
            Ok(instance) => self.form = load_form(instance.settings()),
            Err(_) => self.form = SettingsForm::default(),
        }
    }

    /// Account identity for the next launch (selection or anonymous).
    fn launch_account(&self) -> AccountRef {
        match self.accounts.selected_account() {
            Some(account) => AccountRef {
                username: account.username.clone(),
                uuid: account.uuid.clone(),
            },
            None => AccountRef {
                username: "Player".to_string(),
                uuid: uuid::Uuid::new_v4().simple().to_string(),
            },
        }
    }

    /// Handle `LaunchPressed`: record the run (the subscription spawns the
    /// worker) or refuse with an honest reason.
    fn start_launch(&mut self) {
        self.delete_armed = false;
        let id = match self.selected.clone() {
            Some(id) => id,
            None => {
                self.status = "select an instance first".to_string();
                return;
            }
        };
        if self.active_run.is_some() {
            self.status = "a launch is already running (Kill it first)".to_string();
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
        self.page = Page::Console;
        self.status = format!("starting '{id}'...");
        self.push_console(vec![format!("launch requested for '{id}'")]);
    }

    /// Handle `KillPressed`.
    fn kill_running(&mut self) {
        match self.child.lock() {
            Ok(mut guard) => match guard.as_mut() {
                Some(child) => match child.kill() {
                    Ok(()) => self.status = "kill requested".to_string(),
                    Err(e) => self.status = format!("kill failed: {e}"),
                },
                None => self.status = "no running process".to_string(),
            },
            Err(_) => self.status = "internal lock error".to_string(),
        }
    }

    /// Handle `AddInstanceCreate`.
    fn create_instance(&mut self) {
        let name = self.add_name.trim().to_string();
        let version = self.add_version.trim().to_string();
        if name.is_empty() {
            self.status = "instance name is empty".to_string();
            return;
        }
        if version.is_empty() {
            self.status = "minecraft version is empty".to_string();
            return;
        }
        match Instance::create(&self.instances_dir, &name, &version) {
            Ok(instance) => {
                let id = instance.id();
                self.show_add = false;
                self.add_name.clear();
                self.add_version.clear();
                self.reload_instances();
                self.selected = Some(id.clone());
                self.refresh_selection_caches();
                self.status = format!("created instance '{id}'");
            }
            Err(e) => self.status = format!("creating instance failed: {e}"),
        }
    }

    /// Handle `DeletePressed` (first click arms, second click deletes).
    fn delete_selected(&mut self) {
        let id = match self.selected.clone() {
            Some(id) => id,
            None => {
                self.status = "select an instance first".to_string();
                return;
            }
        };
        if !self.delete_armed {
            self.delete_armed = true;
            self.status = format!("click Delete again to confirm deleting '{id}'");
            return;
        }
        self.delete_armed = false;
        match Instance::delete(&self.instances_dir, &id) {
            Ok(()) => {
                self.selected = None;
                self.reload_instances();
                self.refresh_selection_caches();
                self.status = format!("deleted instance '{id}'");
            }
            Err(e) => self.status = format!("deleting '{id}' failed: {e}"),
        }
    }

    /// Handle `CopyPressed`: duplicate the folder under a unique name.
    fn copy_selected(&mut self) {
        self.delete_armed = false;
        let id = match self.selected.clone() {
            Some(id) => id,
            None => {
                self.status = "select an instance first".to_string();
                return;
            }
        };
        let src = self.instances_dir.join(&id);
        let new_id = match prism_core::util::unique_dir_name(&self.instances_dir, &format!("{id} Copy")) {
            Ok(name) => name,
            Err(e) => {
                self.status = format!("copying '{id}' failed: {e}");
                return;
            }
        };
        let dst = self.instances_dir.join(&new_id);
        if let Err(e) = copy_dir_recursive(&src, &dst) {
            self.status = format!("copying '{id}' failed: {e}");
            return;
        }
        if let Ok(mut copied) = Instance::open(&dst) {
            let renamed = format!("{} Copy", copied.name());
            copied.set_name(&renamed);
            if copied.save().is_err() {
                self.status = format!("copied '{id}' to '{new_id}' but renaming the copy failed");
                self.reload_instances();
                self.selected = Some(new_id);
                self.refresh_selection_caches();
                return;
            }
        }
        self.reload_instances();
        self.selected = Some(new_id.clone());
        self.refresh_selection_caches();
        self.status = format!("copied '{id}' to '{new_id}'");
    }

    /// Handle `ExportPressed`: honest stub (see the `Message` docs).
    fn export_selected(&mut self) {
        let id = match self.selected.clone() {
            Some(id) => id,
            None => {
                self.status = "select an instance first".to_string();
                return;
            }
        };
        let path = self.instances_dir.join(&id);
        self.status = format!(
            "Export is not implemented yet — copy the instance folder manually: {}",
            path.display()
        );
        self.push_console(vec![self.status.clone()]);
    }

    /// Handle `ShortcutPressed`: write a `.url` shortcut on the Desktop.
    fn create_shortcut(&mut self) {
        let id = match self.selected.clone() {
            Some(id) => id,
            None => {
                self.status = "select an instance first".to_string();
                return;
            }
        };
        let target = self.instances_dir.join(&id);
        let stem = self.selected_name();
        match desktop_dir() {
            Some(desktop) => match write_url_shortcut(&desktop, &stem, &target) {
                Ok(path) => {
                    self.status = format!(
                        "wrote shortcut {} (a .url file; on Windows it opens the instance folder)",
                        path.display()
                    );
                }
                Err(e) => self.status = format!("creating shortcut failed: {e}"),
            },
            None => {
                self.status = "cannot locate your Desktop folder — shortcut not created".to_string();
            }
        }
    }

    /// Handle `UpdatePressed`: honest stub (updates ship via the installer).
    fn check_updates(&mut self) {
        self.status =
            "Updates are delivered via the installer — this build does not update itself.".to_string();
        self.push_console(vec![self.status.clone()]);
    }

    /// Handle `OpenFolderPressed`.
    fn open_selected_folder(&mut self) {
        let path = match self.selected.clone() {
            Some(id) => self.instances_dir.join(id),
            None => self.paths.root.clone(),
        };
        match open_in_file_manager(&path) {
            Ok(()) => self.status = format!("opened {}", path.display()),
            Err(e) => self.status = e,
        }
    }

    /// Handle `FoldersPressed`: reveal the resolved instances folder.
    fn open_folders(&mut self) {
        let dir = self.instances_dir.clone();
        match open_in_file_manager(&dir) {
            Ok(()) => self.status = format!("opened {}", dir.display()),
            Err(e) => self.status = e,
        }
    }

    /// Handle `GroupToggled`: collapse/expand a section (persisted, like
    /// Prism's `hidden` flag in `instgroups.json`).
    fn toggle_group(&mut self, key: &str) {
        let collapsed = self.groups.is_collapsed(key);
        self.groups.set_collapsed(key, !collapsed);
        if let Err(e) = self.groups.save(&self.paths) {
            self.status = format!("saving groups failed: {e}");
        }
    }

    /// Handle `ChangeGroupSelected`: move the selection between groups.
    fn change_group(&mut self, label: &str) {
        let id = match self.selected.clone() {
            Some(id) => id,
            None => {
                self.status = "select an instance first".to_string();
                return;
            }
        };
        if label == UNGROUPED_LABEL {
            self.groups.set_group(&id, None);
        } else if label.trim().is_empty() {
            self.status = "ignoring blank group name".to_string();
            return;
        } else {
            self.groups.set_group(&id, Some(label));
        }
        if let Err(e) = self.groups.save(&self.paths) {
            self.status = format!("saving groups failed: {e}");
            return;
        }
        self.reload_instances();
        if label == UNGROUPED_LABEL {
            self.status = format!("moved '{id}' to Ungrouped");
        } else {
            self.status = format!("moved '{id}' to group '{label}'");
        }
    }

    /// Handle `OpenModsFolderPressed` (creating the folder like Prism does
    /// on launch so there is always something to reveal).
    fn open_mods_folder(&mut self) {
        let id = match self.selected.clone() {
            Some(id) => id,
            None => {
                self.status = "select an instance first".to_string();
                return;
            }
        };
        let dir = match Instance::open(&self.instances_dir.join(&id)) {
            Ok(instance) => instance.mods_dir(),
            Err(e) => {
                self.status = format!("cannot open '{id}': {e}");
                return;
            }
        };
        if std::fs::create_dir_all(&dir).is_err() {
            self.status = format!("cannot create {}", dir.display());
            return;
        }
        match open_in_file_manager(&dir) {
            Ok(()) => self.status = format!("opened {}", dir.display()),
            Err(e) => self.status = e,
        }
    }

    /// Handle `ModToggled`.
    fn toggle_mod(&mut self, file: &str, enabled: bool) {
        let id = match self.selected.clone() {
            Some(id) => id,
            None => {
                self.status = "select an instance first".to_string();
                return;
            }
        };
        let dir = match Instance::open(&self.instances_dir.join(&id)) {
            Ok(instance) => instance.mods_dir(),
            Err(e) => {
                self.status = format!("cannot open '{id}': {e}");
                return;
            }
        };
        match set_mod_enabled(&dir, file, enabled) {
            Ok(()) => {
                self.mods = list_mods(&dir);
                if enabled {
                    self.status = format!("enabled mod '{file}'");
                } else {
                    self.status = format!("disabled mod '{file}'");
                }
            }
            Err(e) => self.status = e,
        }
    }

    /// Handle `SettingsSave`: parse the form (falling back per field) and
    /// persist `instance.cfg` through the `Settings` setters.
    fn save_settings(&mut self) {
        let id = match self.selected.clone() {
            Some(id) => id,
            None => {
                self.status = "select an instance first".to_string();
                return;
            }
        };
        let mut instance = match Instance::open(&self.instances_dir.join(&id)) {
            Ok(instance) => instance,
            Err(e) => {
                self.status = format!("cannot open '{id}': {e}");
                return;
            }
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
        let width = parse_mem(
            &self.form.win_width,
            settings.get_i64("MinecraftWinWidth", defaults::MC_WIN_WIDTH),
        );
        let height = parse_mem(
            &self.form.win_height,
            settings.get_i64("MinecraftWinHeight", defaults::MC_WIN_HEIGHT),
        );
        settings.set_bool("OverrideWindow", self.form.override_window);
        settings.set_i64("MinecraftWinWidth", width);
        settings.set_i64("MinecraftWinHeight", height);
        settings.set_bool("JoinServerOnLaunch", self.form.join_server);
        settings.set_str("JoinServerOnLaunchAddress", self.form.server_address.trim());
        match instance.save() {
            Ok(()) => {
                self.reload_instances();
                self.refresh_selection_caches();
                self.status = format!("saved settings for '{id}'");
            }
            Err(e) => {
                self.status = format!("saving settings failed: {e}");
            }
        }
    }

    /// Handle `AccountAdd`.
    fn add_account(&mut self) {
        let name = self.account_input.trim().to_string();
        match self.accounts.add(&name) {
            Ok(()) => match self.accounts.save() {
                Ok(()) => {
                    self.account_input.clear();
                    self.status = format!("added account '{name}'");
                }
                Err(e) => self.status = e,
            },
            Err(e) => self.status = e,
        }
    }

    /// Handle `AccountSelect`.
    fn select_account(&mut self, uuid: &str) {
        match self.accounts.select(uuid) {
            Ok(()) => match self.accounts.save() {
                Ok(()) => self.status = "account selected".to_string(),
                Err(e) => self.status = e,
            },
            Err(e) => self.status = e,
        }
    }

    /// Handle `AccountRemove`.
    fn remove_account(&mut self, uuid: &str) {
        if self.accounts.remove(uuid) {
            match self.accounts.save() {
                Ok(()) => self.status = "account removed".to_string(),
                Err(e) => self.status = e,
            }
        } else {
            self.status = "account not found".to_string();
        }
    }

    /// Gate-aware effective memory line for the Settings page.
    fn effective_memory_line(&self) -> Option<String> {
        let id = self.selected.as_ref()?;
        let instance = Instance::open(&self.instances_dir.join(id)).ok()?;
        let global = match Settings::load(&self.paths.global_config()) {
            Ok(settings) => settings,
            Err(_) => Settings::empty(self.paths.global_config()),
        };
        let model = SettingsModel::with_instance(global, instance.settings().clone());
        let (low, high) = model.effective_memory();
        Some(format!("Effective memory (OverrideMemory gate): {low} / {high} MiB"))
    }

    /// Synchronous update shared by both shells.
    pub fn update(&mut self, message: Message) {
        match message {
            Message::SelectInstance(id) => {
                self.delete_armed = false;
                self.selected = Some(id);
                self.refresh_selection_caches();
                self.status = format!("selected {}", self.selected_name());
            }
            Message::SearchChanged(query) => self.search = query,
            Message::Refresh => {
                self.delete_armed = false;
                self.reload_instances();
                self.refresh_selection_caches();
            }
            Message::PageSelected(page) => {
                self.delete_armed = false;
                self.page = page;
                if page == Page::Settings {
                    self.reload_form();
                }
            }
            Message::LaunchPressed => self.start_launch(),
            Message::KillPressed => self.kill_running(),
            Message::AddInstancePressed => self.show_add = true,
            Message::AddInstanceNameChanged(value) => self.add_name = value,
            Message::AddInstanceVersionChanged(value) => self.add_version = value,
            Message::AddInstanceCreate => self.create_instance(),
            Message::AddInstanceCancel => {
                self.show_add = false;
                self.add_name.clear();
                self.add_version.clear();
            }
            Message::EditPressed => {
                self.page = Page::Settings;
                self.reload_form();
                self.status = "editing instance settings".to_string();
            }
            Message::OpenFolderPressed => self.open_selected_folder(),
            Message::DeletePressed => self.delete_selected(),
            Message::SettingsPressed => {
                self.page = Page::Settings;
                self.reload_form();
            }
            Message::AccountsPressed => self.page = Page::Accounts,
            Message::AboutPressed => self.page = Page::About,
            Message::ConsoleClear => {
                self.console.clear();
                self.status = "console cleared".to_string();
            }
            Message::ConsoleAutoscrollToggled(on) => self.autoscroll = on,
            Message::LaunchLog { run_id, lines } => {
                let current = match self.active_run.as_ref() {
                    Some(run) => run.run_id,
                    None => 0,
                };
                if current == run_id && current != 0 {
                    if let Some(last) = lines.last() {
                        self.status = last.clone();
                    }
                    self.push_console(lines);
                }
            }
            Message::LaunchDone { run_id, note } => {
                let current = match self.active_run.as_ref() {
                    Some(run) => run.run_id,
                    None => 0,
                };
                if current == run_id && current != 0 {
                    self.active_run = None;
                    self.status = note.clone();
                    self.push_console(vec![note]);
                }
            }
            Message::InstancesLoaded(data) => self.apply_loaded(data),
            Message::GroupToggled(key) => self.toggle_group(&key),
            Message::ChangeGroupSelected(label) => self.change_group(&label),
            Message::CopyPressed => self.copy_selected(),
            Message::ExportPressed => self.export_selected(),
            Message::ShortcutPressed => self.create_shortcut(),
            Message::UpdatePressed => self.check_updates(),
            Message::FoldersPressed => self.open_folders(),
            Message::ModToggled(file, enabled) => self.toggle_mod(&file, enabled),
            Message::OpenModsFolderPressed => self.open_mods_folder(),
            Message::SetName(value) => self.form.name = value,
            Message::SetMinMem(value) => self.form.min_mem = value,
            Message::SetMaxMem(value) => self.form.max_mem = value,
            Message::SetOverrideMemory(on) => self.form.override_memory = on,
            Message::SetJavaPath(value) => self.form.java_path = value,
            Message::SetOverrideJava(on) => self.form.override_java = on,
            Message::SetWinWidth(value) => self.form.win_width = value,
            Message::SetWinHeight(value) => self.form.win_height = value,
            Message::SetOverrideWindow(on) => self.form.override_window = on,
            Message::SetJoinServer(on) => self.form.join_server = on,
            Message::SetServerAddress(value) => self.form.server_address = value,
            Message::SettingsSave => self.save_settings(),
            Message::AccountNameChanged(value) => self.account_input = value,
            Message::AccountAdd => self.add_account(),
            Message::AccountSelect(uuid) => self.select_account(&uuid),
            Message::AccountRemove(uuid) => self.remove_account(&uuid),
            Message::MicrosoftPressed => {
                self.status = "Microsoft login is not implemented yet".to_string();
                self.push_console(vec![
                    "Microsoft login is not implemented yet — offline accounts only.".to_string(),
                ]);
            }
        }
    }

    /// View tree shared by both shells. Pure reads over cached state: no IO
    /// happens here, so per-frame work stays minimal.
    pub fn view(&self) -> Element<'_, Message> {
        container(
            column![
                self.view_toolbar(),
                row![self.view_main(), self.view_sidebar()].spacing(12).height(Length::Fill),
                self.view_status(),
            ]
            .spacing(8),
        )
        .padding(10)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    /// Prism-order top toolbar with the account chip right-aligned. Every
    /// button pairs a 16px embedded icon (see `crate::icons::ui_icon`) with
    /// its text label; the account chip uses the steve head as the player
    /// avatar. Zero runtime IO: all art is `include_bytes!`'d.
    fn view_toolbar(&self) -> Element<'_, Message> {
        let chip = self.account_chip_label();
        row![
            toolbar_icon_button(ui_handle("check"), "＋ Add Instance", Message::AddInstancePressed),
            toolbar_icon_button(ui_handle("folder"), "Folders", Message::FoldersPressed),
            toolbar_icon_button(ui_handle("gear"), "Settings", Message::SettingsPressed),
            toolbar_icon_button(ui_handle("help"), "Help", Message::AboutPressed),
            toolbar_icon_button(ui_handle("update"), "Update", Message::UpdatePressed),
            horizontal_space(),
            toolbar_icon_button(instance_handle("steve"), &chip, Message::AccountsPressed),
        ]
        .spacing(4)
        .into()
    }

    /// Left/main area: search, collapsible group grid, page tabs, page.
    fn view_main(&self) -> Element<'_, Message> {
        let mut main = column![
            row![
                text_input("Search instances...", &self.search)
                    .on_input(Message::SearchChanged)
                    .width(Length::Fill),
                button(
                    row![
                        Image::new(ui_handle("refresh"))
                            .width(Length::Fixed(16.0))
                            .height(Length::Fixed(16.0)),
                        text("Refresh").size(13),
                    ]
                    .spacing(4),
                )
                .on_press(Message::Refresh),
            ]
            .spacing(6),
        ]
        .spacing(8);
        if self.show_add {
            main = main.push(self.view_add_panel());
        }
        let mut body = column![].spacing(10);
        if self.loading {
            body = body.push(text("Loading instances…").size(14));
        } else {
            let sections = grouped_instances(&self.list, &self.search);
            if sections.is_empty() {
                if self.list.is_empty() {
                    body = body.push(
                        text(format!("No instances found in {}.", self.instances_dir.display())).size(13),
                    );
                } else {
                    body = body.push(text("No instances match the search.").size(13));
                }
            }
            for section in &sections {
                body = body.push(self.view_section(section));
            }
        }
        body = body.push(horizontal_rule(1u16));
        body = body.push(self.view_tabs());
        body = body.push(self.view_page());
        main = main.push(scrollable(body).height(Length::Fill));
        container(main).width(Length::Fill).height(Length::Fill).padding(6).into()
    }

    /// One collapsible group section with its wrapped tile grid.
    fn view_section(&self, section: &GroupSection) -> Element<'_, Message> {
        let (key, display) = match section.name.as_deref() {
            Some(group) => (group.to_string(), group.to_string()),
            None => (String::new(), UNGROUPED_LABEL.to_string()),
        };
        let arrow = if self.groups.is_collapsed(&key) { "▸" } else { "▾" };
        let mut content = column![
            button(
                row![
                    text(format!("{arrow} {display} ({})", section.entries.len())).size(14),
                    horizontal_space(),
                ]
            )
            .on_press(Message::GroupToggled(key))
            .width(Length::Fill),
        ]
        .spacing(6);
        if !self.groups.is_collapsed(
            section.name.as_deref().unwrap_or(""),
        ) {
            for chunk in section.entries.chunks(TILES_PER_ROW) {
                let mut grid_row = row![].spacing(8);
                for entry in chunk {
                    let is_selected = self.selected.as_deref() == Some(entry.id.as_str());
                    grid_row = grid_row.push(instance_tile(entry, is_selected));
                }
                content = content.push(grid_row);
            }
        }
        content.into()
    }

    /// Page tabs (the About page is labeled Help, like Prism's Help menu).
    fn view_tabs(&self) -> Element<'_, Message> {
        let mut tabs = row![].spacing(4);
        for page in Page::all() {
            tabs = tabs.push(toolbar_button(page.tab_label(), Message::PageSelected(page)));
        }
        tabs.into()
    }

    /// Right sidebar: big tile, name, divider, vertical action list.
    fn view_sidebar(&self) -> Element<'_, Message> {
        let mut side = column![].spacing(8);
        let selected_entry = self
            .selected
            .as_deref()
            .and_then(|id| self.list.entries().iter().find(|entry| entry.id == id));
        match selected_entry {
            Some(entry) => {
                side = side.push(big_tile(entry, true));
                side = side.push(text(entry.name.clone()).size(15));
            }
            None => {
                side = side.push(big_tile_placeholder());
                side = side.push(text("No instance selected").size(13));
            }
        }
        side = side.push(horizontal_rule(1u16));
        let has_selection = self.selected.is_some();
        let running = self.active_run.is_some();
        side = side.push(sidebar_button("▶ Launch", has_selection, Message::LaunchPressed));
        side = side.push(sidebar_button("✖ Kill", running, Message::KillPressed));
        side = side.push(sidebar_button("Edit", has_selection, Message::EditPressed));
        if has_selection {
            side = side.push(
                row![
                    text("Group:").size(13),
                    pick_list(
                        self.group_options(),
                        Some(self.selected_group_label()),
                        Message::ChangeGroupSelected,
                    ),
                ]
                .spacing(6),
            );
        }
        side = side.push(sidebar_button("Folder", has_selection, Message::OpenFolderPressed));
        side = side.push(sidebar_button("Export", has_selection, Message::ExportPressed));
        side = side.push(sidebar_button("Copy", has_selection, Message::CopyPressed));
        let delete_label = if self.delete_armed { "Confirm delete?" } else { "Delete" };
        side = side.push(sidebar_button(delete_label, has_selection, Message::DeletePressed));
        side = side.push(sidebar_button("Create Shortcut", has_selection, Message::ShortcutPressed));
        container(side).width(Length::Fixed(SIDEBAR_WIDTH)).height(Length::Fill).padding(8).into()
    }

    fn view_add_panel(&self) -> Element<'_, Message> {
        container(
            column![
                text("Add instance").size(16),
                text_input("Name", &self.add_name).on_input(Message::AddInstanceNameChanged),
                text_input("Minecraft version (e.g. 1.21.1)", &self.add_version)
                    .on_input(Message::AddInstanceVersionChanged),
                row![
                    button(text("Create").size(13)).on_press(Message::AddInstanceCreate),
                    button(text("Cancel").size(13)).on_press(Message::AddInstanceCancel),
                ]
                .spacing(6),
            ]
            .spacing(6),
        )
        .padding(8)
        .into()
    }

    fn view_page(&self) -> Element<'_, Message> {
        match self.page {
            Page::Console => self.view_console(),
            Page::Mods => self.view_mods(),
            Page::ResourcePacks => self.view_name_list("Resource packs", &self.resource_packs, "resourcepacks/"),
            Page::ShaderPacks => self.view_name_list("Shader packs", &self.shader_packs, "shaderpacks/"),
            Page::Worlds => self.view_name_list("Worlds", &self.worlds, "saves/"),
            Page::Version => self.view_version(),
            Page::Settings => self.view_settings(),
            Page::Accounts => self.view_accounts(),
            Page::About => self.view_about(),
        }
    }

    fn view_console(&self) -> Element<'_, Message> {
        let total = self.console.len();
        let shown = console_shown_lines(total);
        let skip = total - shown;
        let mut main = column![
            row![
                button(text("Clear").size(13)).on_press(Message::ConsoleClear),
                checkbox("Autoscroll", self.autoscroll).on_toggle(Message::ConsoleAutoscrollToggled),
                text(format!("{total} line(s), showing last {shown}")).size(12),
            ]
            .spacing(8),
        ]
        .spacing(6);
        let mut lines = column![].spacing(0);
        let mut index = 0usize;
        for line in self.console.iter() {
            if index >= skip {
                lines = lines.push(text(line.clone()).size(12).font(Font::MONOSPACE));
            }
            index += 1;
        }
        main = main.push(
            scrollable(lines)
                .id(console_scroll_id())
                .height(Length::Fixed(CONSOLE_PANE_HEIGHT)),
        );
        main.into()
    }

    fn view_mods(&self) -> Element<'_, Message> {
        let mut main = column![
            row![
                button(text("Open Mods Folder").size(13)).on_press(Message::OpenModsFolderPressed),
                text(format!("{} mod(s)", self.mods.len())).size(13),
            ]
            .spacing(8),
        ]
        .spacing(6);
        if self.selected.is_none() {
            main = main.push(text("Select an instance to manage mods.").size(13));
        }
        for entry in &self.mods {
            let file = entry.file_name.clone();
            main = main.push(
                row![
                    text(entry.display_name.clone()).size(13).font(Font::MONOSPACE).width(Length::Fill),
                    checkbox("Enabled", entry.enabled)
                        .on_toggle(move |on| Message::ModToggled(file.clone(), on)),
                ]
                .spacing(8),
            );
        }
        main.into()
    }

    fn view_name_list(&self, title: &str, names: &[String], folder: &str) -> Element<'_, Message> {
        let mut main = column![
            text(format!("{title} ({folder})")).size(16),
            text(format!("{} item(s)", names.len())).size(12),
        ]
        .spacing(6);
        if self.selected.is_none() {
            main = main.push(text("Select an instance to browse.").size(13));
        }
        for name in names {
            main = main.push(text(name.clone()).size(13).font(Font::MONOSPACE));
        }
        main.into()
    }

    fn view_version(&self) -> Element<'_, Message> {
        let mut main = column![
            text("Version (mmc-pack components)").size(16),
            text(format!("{} component(s)", self.components.len())).size(12),
        ]
        .spacing(6);
        if self.selected.is_none() {
            main = main.push(text("Select an instance to inspect.").size(13));
        }
        for (uid, version) in &self.components {
            main = main.push(text(format!("{uid}  {version}")).size(13).font(Font::MONOSPACE));
        }
        main.into()
    }

    fn view_settings(&self) -> Element<'_, Message> {
        let mut main = column![text("Instance settings").size(16)].spacing(6);
        if self.selected.is_none() {
            main = main.push(text("Select an instance to edit settings.").size(13));
            return main.into();
        }
        main = main.push(text("Display name").size(12));
        main = main.push(text_input("Name", &self.form.name).on_input(Message::SetName));
        main = main.push(
            checkbox("Override memory (use instance Min/MaxMemAlloc)", self.form.override_memory)
                .on_toggle(Message::SetOverrideMemory),
        );
        main = main.push(
            row![
                text("Min MiB").size(13),
                text_input("128", &self.form.min_mem)
                    .on_input(Message::SetMinMem)
                    .width(Length::Fixed(110.0)),
                text("Max MiB").size(13),
                text_input("4096", &self.form.max_mem)
                    .on_input(Message::SetMaxMem)
                    .width(Length::Fixed(110.0)),
            ]
            .spacing(6),
        );
        main = main.push(
            checkbox("Override Java location (use instance JavaPath)", self.form.override_java)
                .on_toggle(Message::SetOverrideJava),
        );
        main = main.push(
            text_input("Java path (empty = PATH lookup)", &self.form.java_path)
                .on_input(Message::SetJavaPath),
        );
        main = main.push(
            checkbox("Override window size", self.form.override_window).on_toggle(Message::SetOverrideWindow),
        );
        main = main.push(
            row![
                text("Width").size(13),
                text_input("854", &self.form.win_width)
                    .on_input(Message::SetWinWidth)
                    .width(Length::Fixed(110.0)),
                text("Height").size(13),
                text_input("480", &self.form.win_height)
                    .on_input(Message::SetWinHeight)
                    .width(Length::Fixed(110.0)),
            ]
            .spacing(6),
        );
        main = main.push(
            checkbox("Join server on launch", self.form.join_server).on_toggle(Message::SetJoinServer),
        );
        main = main.push(
            text_input("Server address (host[:port])", &self.form.server_address)
                .on_input(Message::SetServerAddress),
        );
        main = main.push(button(text("Save").size(13)).on_press(Message::SettingsSave));
        if let Some(line) = self.effective_memory_line() {
            main = main.push(text(line).size(12));
        }
        main = main.push(text(
            "Gates follow SettingsModel semantics: an instance value only takes effect while its \
              Override gate is on; otherwise the global prismlauncher.cfg value wins. Join-server \
              settings are instance-only in Prism (no global gate).",
        ).size(12));
        main.into()
    }

    fn view_accounts(&self) -> Element<'_, Message> {
        let mut main = column![text("Accounts (offline)").size(16)].spacing(6);
        main = main.push(
            row![
                text_input("username", &self.account_input)
                    .on_input(Message::AccountNameChanged)
                    .width(Length::Fill),
                button(text("Add").size(13)).on_press(Message::AccountAdd),
            ]
            .spacing(6),
        );
        for account in self.accounts.list() {
            let select_id = account.uuid.clone();
            let remove_id = account.uuid.clone();
            let marker = if self.accounts.selected_uuid() == Some(account.uuid.as_str()) {
                "*"
            } else {
                " "
            };
            main = main.push(
                row![
                    text(format!("{marker} {} ({})", account.username, account.uuid))
                        .size(13)
                        .font(Font::MONOSPACE)
                        .width(Length::Fill),
                    button(text("Select").size(12)).on_press(Message::AccountSelect(select_id)),
                    button(text("Remove").size(12)).on_press(Message::AccountRemove(remove_id)),
                ]
                .spacing(6),
            );
        }
        main = main.push(button(text("Microsoft login").size(13)).on_press(Message::MicrosoftPressed));
        main = main.push(
            text("Microsoft login is not implemented yet — offline accounts only.").size(12),
        );
        main.into()
    }

    /// Help page (the old About content lives here, reached via Help).
    fn view_about(&self) -> Element<'_, Message> {
        column![
            text("Help").size(16),
            text("Single-click a tile to select it, then use the sidebar actions (Launch opens the Console page).").size(13),
            text("Group headers collapse/expand; the Change Group list moves the selected instance.").size(13),
            text(format!("prism-desktop {}", env!("CARGO_PKG_VERSION"))).size(16),
            text(format!("data root: {}", self.paths.root.display())).size(13),
            text(format!("instances: {}", self.instances_dir.display())).size(13),
            text(format!("meta cache: {}", self.paths.meta_dir().display())).size(13),
            text(format!("global config: {}", self.paths.global_config().display())).size(13),
            text(format!("accounts: {}", self.paths.accounts_file().display())).size(13),
            text("Notes: Export is a stub (copy the folder manually); Create Shortcut writes a .url file on the Desktop; software rendering is preferred (see main.rs).").size(12),
        ]
        .spacing(6)
        .into()
    }

    fn view_status(&self) -> Element<'_, Message> {
        let selected = match self.selected.as_deref() {
            Some(id) => id.to_string(),
            None => "none".to_string(),
        };
        container(
            text(format!("selected: {selected} | instances: {} | {}", self.list.len(), self.status)).size(12),
        )
        .padding(4)
        .into()
    }
}

/// One instance tile: the real carved icon (`iconKey` resolved via
/// [`crate::icons::instance_icon`], 56px) on a rounded box plus the name
/// label below. The selected tile keeps Prism's green (`#7CB342`)
/// background behind the icon; unselected tiles keep their deterministic
/// pastel ([`tile_color`]). Single click selects (the sidebar Launch acts on
/// it). Fully owned (`'static`), so grids built from locals can be returned.
/// Unknown `iconKey`s show the default grass block — never a letter tile.
fn instance_tile(entry: &InstanceEntry, selected: bool) -> Element<'static, Message> {
    let background = if selected { selected_tile_color() } else { tile_color(&entry.id) };
    let tile = container(
        Image::new(instance_handle(&entry.icon))
            .width(Length::Fixed(TILE_BOX))
            .height(Length::Fixed(TILE_BOX)),
    )
    .width(Length::Fixed(TILE_BOX))
    .height(Length::Fixed(TILE_BOX))
    .center_x()
    .center_y()
    .style(move |_: &Theme| container::Appearance {
        background: Some(Background::Color(background)),
        border: Border { radius: 10.0.into(), ..Default::default() },
        ..Default::default()
    });
    let id = entry.id.clone();
    button(
        column![
            tile,
            container(text(short_name(&entry.name)).size(11))
                .width(Length::Fixed(TILE_WIDTH - 8.0))
                .center_x(),
        ]
        .spacing(4),
    )
    .on_press(Message::SelectInstance(id))
    .width(Length::Fixed(TILE_WIDTH))
    .into()
}

/// Large sidebar tile for the selected instance: the 96px carved icon on
/// the same selected/unselected background as the grid tiles.
fn big_tile(entry: &InstanceEntry, selected: bool) -> Element<'static, Message> {
    let background = if selected { selected_tile_color() } else { tile_color(&entry.id) };
    container(
        Image::new(instance_handle(&entry.icon))
            .width(Length::Fixed(BIG_TILE))
            .height(Length::Fixed(BIG_TILE)),
    )
    .width(Length::Fixed(BIG_TILE))
    .height(Length::Fixed(BIG_TILE))
    .center_x()
    .center_y()
    .style(move |_: &Theme| container::Appearance {
        background: Some(Background::Color(background)),
        border: Border { radius: 14.0.into(), ..Default::default() },
        ..Default::default()
    })
    .into()
}

/// Sidebar tile when nothing is selected: the default grass block on a
/// neutral grey box (the grass fallback, never a letter tile).
fn big_tile_placeholder() -> Element<'static, Message> {
    container(
        Image::new(instance_handle(""))
            .width(Length::Fixed(BIG_TILE))
            .height(Length::Fixed(BIG_TILE)),
    )
    .width(Length::Fixed(BIG_TILE))
    .height(Length::Fixed(BIG_TILE))
    .center_x()
    .center_y()
    .style(|_: &Theme| container::Appearance {
        background: Some(Background::Color(Color::from_rgb(0.45, 0.45, 0.48))),
        border: Border { radius: 14.0.into(), ..Default::default() },
        ..Default::default()
    })
    .into()
}

/// Full-width sidebar action button; without `enabled` it renders disabled
/// (no `on_press`), which is how Kill stays off while nothing runs.
fn sidebar_button(label: &str, enabled: bool, message: Message) -> Button<'static, Message> {
    let styled = button(text(label).size(13)).width(Length::Fill);
    if enabled {
        styled.on_press(message)
    } else {
        styled
    }
}

fn toolbar_button(label: &str, message: Message) -> Button<'static, Message> {
    button(text(label).size(13)).on_press(message)
}

/// Top-toolbar button: a 16px embedded icon plus the text label. The
/// handle is prebuilt and cached (see `crate::icons`), so this does no IO.
fn toolbar_icon_button(icon: Handle, label: &str, message: Message) -> Button<'static, Message> {
    button(
        row![
            Image::new(icon).width(Length::Fixed(16.0)).height(Length::Fixed(16.0)),
            text(label).size(13),
        ]
        .spacing(4),
    )
    .on_press(message)
}

/// Background instance loader: scan + parse off the GUI thread, then hand
/// the result back over the subscription sender. A full channel just waits
/// briefly; a gone GUI ends the thread.
fn run_instance_loader(data_root: PathBuf, mut sender: futures::channel::mpsc::Sender<Message>) {
    let mut pending: Option<LoadedData> = Some(load_instances_blocking(&data_root));
    loop {
        let data = match pending.take() {
            Some(data) => data,
            None => return,
        };
        match sender.try_send(Message::InstancesLoaded(data)) {
            Ok(()) => return,
            Err(e) => {
                if e.is_full() {
                    match e.into_inner() {
                        Message::InstancesLoaded(data) => {
                            pending = Some(data);
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        _ => return,
                    }
                } else {
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use prism_core::instance::groups::Groups;

    fn test_paths(tag: &str) -> (tempfile::TempDir, PrismPaths) {
        let dir = tempfile::tempdir().unwrap();
        let _ = tag;
        let paths = PrismPaths::at(dir.path());
        std::fs::create_dir_all(paths.instances_dir()).unwrap();
        (dir, paths)
    }

    fn write_cfg(paths: &PrismPaths, text: &str) {
        std::fs::write(paths.global_config(), text).unwrap();
    }

    #[test]
    fn pages_cover_toolbar_order_and_default_console() {
        assert_eq!(Page::all().len(), 9);
        assert_eq!(Page::default(), Page::Console);
        let titles: Vec<&str> = Page::all().iter().map(|p| p.title()).collect();
        assert!(titles.contains(&"Console"));
        assert!(titles.contains(&"Mods"));
        assert!(titles.contains(&"Settings"));
        assert!(titles.contains(&"Accounts"));
        assert!(titles.contains(&"About"));
        assert_eq!(Page::About.tab_label(), "Help");
    }

    #[test]
    fn navigation_updates_page() {
        let (_dir, paths) = test_paths("nav");
        let mut app = PrismApp::with_paths(paths);
        assert_eq!(app.page, Page::Console);
        app.update(Message::PageSelected(Page::Mods));
        assert_eq!(app.page, Page::Mods);
        app.update(Message::SettingsPressed);
        assert_eq!(app.page, Page::Settings);
        app.update(Message::AccountsPressed);
        assert_eq!(app.page, Page::Accounts);
        app.update(Message::AboutPressed);
        assert_eq!(app.page, Page::About);
        app.update(Message::EditPressed);
        assert_eq!(app.page, Page::Settings);
    }

    #[test]
    fn console_caps_at_twenty_thousand_lines() {
        let (_dir, paths) = test_paths("console");
        let mut app = PrismApp::with_paths(paths);
        app.console.clear();
        let lines: Vec<String> = (0..(CONSOLE_LINE_CAP + 5)).map(|i| format!("line {i}")).collect();
        app.push_console(lines);
        assert_eq!(app.console.len(), CONSOLE_LINE_CAP);
        assert_eq!(app.console.front().map(String::as_str), Some("line 5"));
        assert_eq!(console_shown_lines(10), 10);
        assert_eq!(console_shown_lines(1_000_000), CONSOLE_VIEW_LINES);
        app.update(Message::ConsoleClear);
        assert!(app.console.is_empty());
    }

    #[test]
    fn launch_streaming_ignores_stale_runs() {
        let (_dir, paths) = test_paths("stale");
        let mut app = PrismApp::with_paths(paths);
        app.console.clear();
        // No active run: everything ignored.
        app.update(Message::LaunchLog { run_id: 7, lines: vec!["x".to_string()] });
        assert!(app.console.is_empty());
        app.active_run = Some(ActiveRunData {
            run_id: 3,
            instance_id: "a".to_string(),
            data_root: PathBuf::from("/tmp"),
            account: AccountRef { username: "u".to_string(), uuid: "v".to_string() },
        });
        app.update(Message::LaunchLog { run_id: 9, lines: vec!["stale".to_string()] });
        assert!(app.console.is_empty());
        app.update(Message::LaunchLog { run_id: 3, lines: vec!["fresh".to_string()] });
        assert_eq!(app.console.len(), 1);
        app.update(Message::LaunchDone { run_id: 9, note: "stale done".to_string() });
        assert!(app.active_run.is_some());
        app.update(Message::LaunchDone { run_id: 3, note: "done".to_string() });
        assert!(app.active_run.is_none());
        assert_eq!(app.status, "done");
    }

    #[test]
    fn mem_parser_falls_back() {
        assert_eq!(parse_mem("8192", 4096), 8192);
        assert_eq!(parse_mem(" 2048 ", 0), 2048);
        assert_eq!(parse_mem("", 4096), 4096);
        assert_eq!(parse_mem("lots", 4096), 4096);
        assert_eq!(parse_mem("-5", 1), -5);
    }

    #[test]
    fn form_loads_prism_keys_with_defaults() {
        let settings = Settings::empty("instance.cfg");
        let form = load_form(&settings);
        assert_eq!(form.min_mem, "128");
        assert_eq!(form.max_mem, "4096");
        assert!(!form.override_memory);
        assert_eq!(form.win_width, "854");
        assert_eq!(form.win_height, "480");
        assert!(!form.join_server);
    }

    #[test]
    fn grouped_instances_orders_groups_then_ungrouped() {
        let (_dir, paths) = test_paths("groups");
        let first = Instance::create(&paths.instances_dir(), "Zulu", "1.21.1").unwrap();
        Instance::create(&paths.instances_dir(), "alpha", "1.21.1").unwrap();
        Instance::create(&paths.instances_dir(), "Mike", "1.21.1").unwrap();
        let mut groups = Groups::default();
        groups.set_group(&first.id(), Some("Packs"));
        groups.save(&paths).unwrap();
        let model = InstanceListModel::load(&paths).unwrap();
        assert_eq!(model.len(), 3);

        let sections = grouped_instances(&model, "");
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].name.as_deref(), Some("Packs"));
        assert_eq!(sections[0].entries.len(), 1);
        assert_eq!(sections[1].name, None);
        assert_eq!(sections[1].entries.len(), 2);

        let filtered = grouped_instances(&model, "alp");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].entries[0].name, "alpha");

        let none = grouped_instances(&model, "zzz");
        assert!(none.is_empty());
    }

    #[test]
    fn delete_requires_two_clicks() {
        let (_dir, paths) = test_paths("delete");
        let instance = Instance::create(&paths.instances_dir(), "Doomed", "1.21.1").unwrap();
        let mut app = PrismApp::with_paths(paths);
        app.update(Message::SelectInstance(instance.id()));
        assert!(app.selected.is_some());
        app.update(Message::DeletePressed);
        assert!(app.delete_armed);
        assert!(app.selected.is_some());
        app.update(Message::DeletePressed);
        assert!(!app.delete_armed);
        assert!(app.selected.is_none());
        assert_eq!(app.list.len(), 0);
    }

    #[test]
    fn microsoft_button_is_honest() {
        let (_dir, paths) = test_paths("ms");
        let mut app = PrismApp::with_paths(paths);
        app.update(Message::MicrosoftPressed);
        assert!(app.status.contains("not implemented yet"));
    }

    #[test]
    fn update_check_is_honest() {
        let (_dir, paths) = test_paths("update");
        let mut app = PrismApp::with_paths(paths);
        app.update(Message::UpdatePressed);
        assert!(app.status.contains("installer"));
    }

    #[test]
    fn export_stub_names_the_folder() {
        let (_dir, paths) = test_paths("export");
        let instance = Instance::create(&paths.instances_dir(), "Pack", "1.21.1").unwrap();
        let mut app = PrismApp::with_paths(paths.clone());
        app.update(Message::SelectInstance(instance.id()));
        app.update(Message::ExportPressed);
        assert!(app.status.contains("not implemented yet"));
        assert!(app.status.contains("Pack"));
    }

    #[test]
    fn settings_save_round_trips_through_instance_cfg() {
        let (_dir, paths) = test_paths("save");
        let instance = Instance::create(&paths.instances_dir(), "Cfg", "1.21.1").unwrap();
        let mut app = PrismApp::with_paths(paths.clone());
        app.update(Message::SelectInstance(instance.id()));
        app.update(Message::SetMinMem("256".to_string()));
        app.update(Message::SetMaxMem("bogus".to_string()));
        app.update(Message::SetOverrideMemory(true));
        app.update(Message::SetJavaPath("/opt/java".to_string()));
        app.update(Message::SetOverrideJava(true));
        app.update(Message::SetServerAddress("mc.example.com:25570".to_string()));
        app.update(Message::SetJoinServer(true));
        app.update(Message::SettingsSave);
        assert!(app.status.contains("saved"), "status: {}", app.status);

        let back = Instance::open(&paths.instances_dir().join(instance.id())).unwrap();
        assert_eq!(back.settings().get_i64("MinMemAlloc", 0), 256);
        // Invalid input kept the previous value.
        assert_eq!(back.settings().get_i64("MaxMemAlloc", 0), 4096);
        assert!(back.settings().get_bool("OverrideMemory", false));
        assert_eq!(back.settings().get_str("JavaPath", ""), "/opt/java");
        assert!(back.settings().get_bool("JoinServerOnLaunch", false));
        assert_eq!(back.settings().get_str("JoinServerOnLaunchAddress", ""), "mc.example.com:25570");
    }

    #[test]
    fn tile_colors_are_deterministic_and_selected_is_prism_green() {
        let first = tile_color("1.21.1");
        assert_eq!(first, tile_color("1.21.1"));
        // Every palette entry is reachable-ish and in range.
        for id in ["a", "b", "c", "1.21.10", "Some Pack"] {
            let color = tile_color(id);
            for channel in [color.r, color.g, color.b] {
                assert!((0.0..=1.0).contains(&channel));
            }
        }
        let selected = selected_tile_color();
        let to_byte = |v: f32| (v * 255.0).round() as u32;
        assert_eq!((to_byte(selected.r), to_byte(selected.g), to_byte(selected.b)), (0x7C, 0xB3, 0x42));
    }

    #[test]
    fn short_names_truncate_without_splitting_chars() {
        assert_eq!(short_name("Tiny"), "Tiny");
        let exact = "x".repeat(30);
        assert_eq!(short_name(&exact), exact);
        let long = format!("{}tail", "🧱".repeat(30));
        let shortened = short_name(&long);
        assert!(shortened.ends_with('…'));
        assert_eq!(shortened.chars().count(), 31);
        assert!(shortened.is_char_boundary(shortened.len()));
    }

    #[test]
    fn group_headers_toggle_collapsed_state() {
        let (_dir, paths) = test_paths("collapse");
        let instance = Instance::create(&paths.instances_dir(), "Solo", "1.21.1").unwrap();
        let mut app = PrismApp::with_paths(paths.clone());
        // Ungrouped starts expanded.
        assert!(!app.groups.is_collapsed(""));
        app.update(Message::GroupToggled(String::new()));
        assert!(app.groups.is_collapsed(""));
        // Persisted like Prism's `hidden` flag.
        let back = Groups::load(&paths);
        assert!(back.is_collapsed(""));
        app.update(Message::GroupToggled(String::new()));
        assert!(!app.groups.is_collapsed(""));
        let _ = instance;
    }

    #[test]
    fn change_group_moves_instance_and_persists() {
        let (_dir, paths) = test_paths("changegroup");
        let instance = Instance::create(&paths.instances_dir(), "Movable", "1.21.1").unwrap();
        let mut app = PrismApp::with_paths(paths.clone());
        app.update(Message::SelectInstance(instance.id()));
        assert_eq!(app.selected_group_label(), UNGROUPED_LABEL);
        assert!(app.group_options().contains(&UNGROUPED_LABEL.to_string()));

        app.update(Message::ChangeGroupSelected("Packs".to_string()));
        assert!(app.status.contains("Packs"), "status: {}", app.status);
        assert_eq!(app.selected_group_label(), "Packs");
        let back = Groups::load(&paths);
        assert_eq!(back.group_of(&instance.id()), Some("Packs"));

        app.update(Message::ChangeGroupSelected(UNGROUPED_LABEL.to_string()));
        assert_eq!(app.selected_group_label(), UNGROUPED_LABEL);
        let back = Groups::load(&paths);
        assert!(back.group_of(&instance.id()).is_none());
    }

    #[test]
    fn copy_duplicates_the_folder_with_unique_name() {
        let (_dir, paths) = test_paths("copy");
        let instance = Instance::create(&paths.instances_dir(), "Original", "1.21.1").unwrap();
        std::fs::write(paths.instances_dir().join(instance.id()).join("marker.txt"), b"keep").unwrap();
        let mut app = PrismApp::with_paths(paths.clone());
        app.update(Message::SelectInstance(instance.id()));
        app.update(Message::CopyPressed);
        assert!(app.status.contains("copied"), "status: {}", app.status);
        assert_eq!(app.list.len(), 2);
        let new_id = app.selected.clone().unwrap();
        assert_ne!(new_id, instance.id());
        assert!(paths.instances_dir().join(&new_id).join("marker.txt").is_file());
        assert!(paths.instances_dir().join(&new_id).join("instance.cfg").is_file());
    }

    #[test]
    fn copy_dir_recursive_copies_trees_and_skips_nothing_real() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("a.txt"), b"a").unwrap();
        std::fs::write(src.join("sub").join("b.txt"), b"b").unwrap();
        let dst = dir.path().join("dst");
        copy_dir_recursive(&src, &dst).unwrap();
        assert_eq!(std::fs::read(dst.join("a.txt")).unwrap(), b"a");
        assert_eq!(std::fs::read(dst.join("sub").join("b.txt")).unwrap(), b"b");
        assert!(copy_dir_recursive(&dir.path().join("missing"), &dst).is_err());
    }

    #[test]
    fn url_shortcuts_are_plain_ini_files() {
        let dir = tempfile::tempdir().unwrap();
        let target = PathBuf::from("C:/Games/PrismLauncher/instances/1.21.1");
        let text = url_shortcut_text(&target);
        assert!(text.starts_with("[InternetShortcut]\nURL=file:///"));
        assert!(text.contains("C:/Games/PrismLauncher/instances/1.21.1"));
        let written = write_url_shortcut(dir.path(), "My Pack", &target).unwrap();
        assert_eq!(written.extension().and_then(|e| e.to_str()), Some("url"));
        let back = std::fs::read_to_string(&written).unwrap();
        assert_eq!(back, text);
        assert!(write_url_shortcut(dir.path(), "a/b", &target).is_err());
    }

    #[test]
    fn startup_uses_configured_dir_and_preselects_selected_instance() {
        let dir = tempfile::tempdir().unwrap();
        let paths = PrismPaths::at(dir.path());
        let custom = dir.path().join("custom-instances");
        std::fs::create_dir_all(&custom).unwrap();
        let forward = custom.display().to_string().replace('\\', "/");
        write_cfg(
            &paths,
            &format!("[General]\nConfigVersion=1.3\nInstanceDir={forward}\nSelectedInstance=Best\n"),
        );
        let created = Instance::create(&custom, "Best", "1.21.1").unwrap();
        assert_eq!(created.id(), "Best");

        let app = PrismApp::with_paths(paths.clone());
        assert_eq!(app.instances_dir, custom);
        assert_eq!(app.list.len(), 1);
        assert_eq!(app.selected.as_deref(), Some("Best"));

        // The same resolution drives the background loader payload.
        let loaded = load_instances_blocking(dir.path());
        assert_eq!(loaded.instances_dir, custom);
        assert_eq!(loaded.selected.as_deref(), Some("Best"));
        assert_eq!(loaded.list.len(), 1);
    }

    #[test]
    fn stale_selected_instance_is_ignored() {
        let (_dir, paths) = test_paths("stale-sel");
        Instance::create(&paths.instances_dir(), "Real", "1.21.1").unwrap();
        write_cfg(&paths, "[General]\nConfigVersion=1.3\nSelectedInstance=Gone\n");
        let app = PrismApp::with_paths(paths);
        assert!(app.selected.is_none());
        let loaded = load_instances_blocking(&app.paths.root);
        assert!(loaded.selected.is_none());
    }

    #[test]
    fn instances_loaded_message_fills_the_placeholder() {
        let (_dir, paths) = test_paths("async");
        Instance::create(&paths.instances_dir(), "Late", "1.21.1").unwrap();
        let mut app = PrismApp::pending(paths.clone());
        assert!(app.loading);
        assert_eq!(app.list.len(), 0);
        let data = load_instances_blocking(&paths.root);
        app.update(Message::InstancesLoaded(data));
        assert!(!app.loading);
        assert_eq!(app.list.len(), 1);
        assert!(!app.status.is_empty());
    }

    #[test]
    fn account_chip_reflects_selection() {
        let (_dir, paths) = test_paths("chip");
        let mut app = PrismApp::with_paths(paths);
        assert_eq!(app.account_chip_label(), "👤 No account");
        app.update(Message::AccountNameChanged("Steve".to_string()));
        app.update(Message::AccountAdd);
        assert_eq!(app.account_chip_label(), "👤 Steve");
    }
}
