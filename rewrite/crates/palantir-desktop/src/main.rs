#![deny(clippy::unwrap_used, clippy::expect_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod addon;
mod backend;
mod bedrock;
mod controls;
mod design;
mod files;
mod flows;
mod gallery;
mod local;
mod locale;
mod modal;
mod model;
mod surfaces;

use design::input;
use design::{Button, label, muted, subtitle, title};
use iced::widget::{Space, button, column, container, row, scrollable, text_input};
use iced::{Application, Command, Element, Length, Settings, Subscription, Theme, executor};
use locale::Key;
use model::{
    Accent, Edition, Instance, Loader, Preferences, Project, ProjectDetail, ProjectType, Scale,
    Sort,
};
use std::collections::{BTreeMap, VecDeque};
use std::sync::mpsc;
use std::time::Duration;

fn main() -> iced::Result {
    App::run(Settings {
        window: iced::window::Settings {
            size: if std::env::args().any(|a| a == "--ui-gallery")
                && std::env::args().any(|a| a == "--ui-small")
            {
                iced::Size::new(840.0, 600.0)
            } else {
                iced::Size::new(1280.0, 820.0)
            },
            min_size: Some(iced::Size::new(840.0, 600.0)),
            ..Default::default()
        },
        default_text_size: iced::Pixels(14.0),
        ..Default::default()
    })
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Home,
    Library,
    Discover,
    Settings,
    About,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstanceTab {
    Overview,
    Content,
    Worlds,
    Logs,
    Manage,
}
impl std::fmt::Display for InstanceTab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}
impl InstanceTab {
    const ALL: [Self; 5] = [
        Self::Overview,
        Self::Content,
        Self::Worlds,
        Self::Logs,
        Self::Manage,
    ];
    fn name(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Content => "Content",
            Self::Worlds => "Worlds",
            Self::Logs => "Logs",
            Self::Manage => "Manage",
        }
    }
    /// The one predicate behind every appearance of a tab: the strip, the
    /// narrow picker, and the selection guard all ask it, so a hidden tab
    /// cannot surface through any of them.
    fn shown(self, features: &model::Features) -> bool {
        match self {
            Self::Overview => features.show_overview,
            Self::Content => features.show_content,
            Self::Worlds => features.show_worlds,
            Self::Logs => features.show_logs,
            Self::Manage => features.show_manage,
        }
    }
    fn visible(features: &model::Features) -> Vec<Self> {
        Self::ALL
            .into_iter()
            .filter(|tab| tab.shown(features))
            .collect()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    Appearance,
    Language,
    Java,
    Window,
    Resources,
    Privacy,
    Behaviour,
    Features,
}
impl SettingsTab {
    const ALL: [Self; 8] = [
        Self::Appearance,
        Self::Language,
        Self::Java,
        Self::Window,
        Self::Resources,
        Self::Privacy,
        Self::Behaviour,
        Self::Features,
    ];
    fn name(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Language => "Language",
            Self::Java => "Java & player",
            Self::Window => "Window",
            Self::Resources => "Resources",
            Self::Privacy => "Privacy",
            Self::Behaviour => "Behaviour",
            Self::Features => "Features",
        }
    }
}
#[derive(Debug, Clone)]
enum Dialog {
    Create,
    Import,
    Delete(String),
    Rename(String),
    Duplicate(String),
    InstanceActions(String),
    JavaAdd,
    InstanceWindow(String),
    InstanceRuntime(String),
    Install,
    Account,
    SignIn,
    Versions,
    Browser,
    ImportReview,
    Preferences,
    Credits,
    PackSetup,
    RemoveContent(String, String, String),
    InstallReview(String),
    Surface(surfaces::Panel),
}
#[derive(Debug, Clone)]
enum Message {
    Navigate(Page),
    Edition(Edition),
    BedrockRefresh,
    BedrockLaunch(palantir_bedrock::Channel),
    BedrockInspect(palantir_bedrock::Channel),
    BedrockFolder(palantir_bedrock::Channel, String),
    Tick,
    Event(iced::Event),
    CloseDialog,
    Open(Dialog),
    Name(String),
    Game(String),
    ImportPath(String),
    Loader(Loader),
    Create,
    Import,
    Rename(String),
    Delete(String),
    Select(String),
    /// Open a collection on one tab -- the actions menu's export entry lands
    /// on the world export surface rather than a menu of its own.
    SelectTab(String, InstanceTab),
    InstanceTab(InstanceTab),
    Launch(String),
    Stop(String),
    OpenFolder(Option<String>, String),
    ToggleMod(String, String),
    Query(String),
    FilterGame(String),
    Kind(ProjectType),
    Sort(Sort),
    Search,
    Next,
    Previous,
    Project(Project),
    CloseProject,
    InstallContent(String),
    ReviewContent(String),
    Draft(surfaces::Edit),
    ReviewLocal,
    ApplyLocal,
    LocalClosed(bool),
    ActivationWorld(String),
    GalleryScene(gallery::Scene),
    BedrockTab(bedrock::Tab),
    BedrockQuery(String),
    SettingsTab(SettingsTab),
    EditPlayer,
    AccountName(String),
    AccountAdd,
    AccountSelect(String),
    AccountRemove(String),
    Player(String),
    Java(String),
    Memory(String),
    Accent(Accent),
    Scale(Scale),
    Activity(bool),
    SaveSettings,
    WindowWidth(String),
    WindowHeight(String),
    InstanceWindowOpen(String),
    InstanceWindowWidth(String),
    InstanceWindowHeight(String),
    InstanceWindowSave(String),
    InstanceWindowClear(String),
    InstanceRuntimeOpen(String),
    InstanceRuntimeMemory(String),
    InstanceRuntimeJava(String),
    InstanceRuntimeArgs(String),
    InstanceRuntimeEnv(String),
    InstanceRuntimeHookPre(String),
    InstanceRuntimeHookWrapper(String),
    InstanceRuntimeHookPost(String),
    InstanceRuntimeSave(String),
    InstanceRuntimeClear(String),
    DuplicateOpen(String),
    DuplicateSave(String),
    JavaAddOpen,
    JavaAddPath(String),
    JavaAddSave,
    JavaUse(String),
    JavaRemove(String),
    FeatureOverview(bool),
    FeatureContent(bool),
    FeatureWorlds(bool),
    FeatureLogs(bool),
    FeatureManage(bool),
    ClearLogs,
    CopyLogs,
    Dismiss,
    ShowActivity,
    LibraryQuery(String),
    LoadVersions,
    ChooseGame(String),
    IncludeSnapshots(bool),
    VersionsQuery(String),
    ReleaseFilter(String),
    MoreReleases,
    LoaderPin(String),
    InspectImport,
    FetchPack,
    BrowsePath(String),
    BrowseFolder,
    BrowsePathAndRead(String),
    ShowReleases,
    ReleaseChangelog(String),
    ChooseSource(String),
    OptionalFile(String, bool),
    ProjectTab(ProjectTab),
    ChooseRelease(String),
    RetryProject,
    ResetFilters,
    OpenUrl(String),
    ResetSettings,
    RevertSettings,
    RememberPage(bool),
    CopyDiagnostics,
    AuthStart,
    AuthCancel,
    CopyDeviceCode,
    ClearHistory,
    RefreshInventory,
    RemoveContent(String, String, String),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectTab {
    Description,
    Releases,
    Changelog,
}
impl ProjectTab {
    const ALL: [Self; 3] = [Self::Description, Self::Releases, Self::Changelog];
    fn name(self) -> &'static str {
        match self {
            Self::Description => "Description",
            Self::Releases => "Releases",
            Self::Changelog => "Changelog",
        }
    }
}
struct App {
    page: Page,
    edition: Edition,
    bedrock: bedrock::State,
    instances: Vec<Instance>,
    preferences: Preferences,
    worker: Option<mpsc::Sender<backend::Request>>,
    events: mpsc::Receiver<backend::Event>,
    dialog: Option<Dialog>,
    name: String,
    // A draft name for the offline-account list, kept apart from the collection
    // name draft so an unfinished dialog cannot overwrite either one.
    account_name: String,
    game: String,
    loader: Loader,
    import_path: String,
    selected: Option<String>,
    instance_tab: InstanceTab,
    inventory: Vec<files::Entry>,
    inventory_loading: bool,
    query: String,
    filter_game: String,
    kind: ProjectType,
    sort: Sort,
    results: Vec<Project>,
    total: usize,
    offset: usize,
    search_id: u64,
    searching: bool,
    /// Live browsing: typing marks the catalogue query dirty and the ticker
    /// fires the search once typing settles, so results follow the query.
    search_dirty: bool,
    search_due: std::time::Instant,
    project: Option<Project>,
    detail: Option<ProjectDetail>,
    detail_loading: bool,
    settings_tab: SettingsTab,
    memory: String,
    /// Settings drafts. The window fields show blank for "unset" and only
    /// become numbers on save, so they live apart from `preferences`.
    window_width: String,
    window_height: String,
    /// The per-collection window dialog's own drafts, kept apart so an
    /// unfinished dialog cannot overwrite the Preferences drafts.
    instance_window_width: String,
    instance_window_height: String,
    /// The per-collection launch-settings dialog's own drafts.
    instance_memory: String,
    instance_java: String,
    instance_args: String,
    instance_env: String,
    instance_hook_pre: String,
    instance_hook_wrapper: String,
    instance_hook_post: String,
    /// The add-a-runtime dialog's candidate path.
    java_add_path: String,
    root: String,
    width: f32,
    pending: BTreeMap<String, String>,
    running: BTreeMap<String, String>,
    logs: BTreeMap<String, VecDeque<String>>,
    notice: Option<String>,
    history: VecDeque<String>,
    saved_preferences: Preferences,
    pending_preferences: Option<Preferences>,
    /// The receipt an in-flight save must leave on screen: an immediate
    /// roster/account save names its own action, and the generic
    /// "Preferences saved." must not bury it when the write lands.
    pending_note: Option<String>,
    /// The visit a dialog was opened over (Preferences, Account): closing or
    /// completing the child returns here instead of dropping the user on the
    /// page behind. Empty for dialogs opened from the page.
    dialog_stack: Vec<Dialog>,
    /// A deletion in flight and its receipt, rendered inside the open
    /// confirmation dialog (the reviewed-operation pattern).
    delete_pending: bool,
    delete_outcome: Option<Result<String, String>>,
    library_query: String,
    game_choices: Vec<model::GameChoice>,
    versions_loading: bool,
    include_snapshots: bool,
    versions_query: String,
    release_filter: String,
    visible_releases: usize,
    loader_versions: Vec<String>,
    loader_pin: String,
    loader_loading: bool,
    import_id: u64,
    import_review: Option<model::ImportReview>,
    import_loading: bool,
    pack_loading: bool,
    optional_files: Vec<String>,
    browser_path: String,
    directory: Vec<model::DirectoryEntry>,
    browse_loading: bool,
    project_tab: ProjectTab,
    releases: Vec<model::Release>,
    releases_loading: bool,
    release_id: Option<String>,
    content_review_id: u64,
    content_plan: Option<model::ContentPlan>,
    content_review_loading: bool,
    content_review_error: Option<String>,
    auth_error: bool,
    authentication: model::Authentication,
    draft: surfaces::Draft,
    local: local::State,
    gallery: bool,
}
impl Application for App {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();
    fn new(_: ()) -> (Self, Command<Message>) {
        let gallery = std::env::args().any(|argument| argument == "--ui-gallery");
        let (worker, events) = if gallery {
            let (worker, _requests) = mpsc::channel();
            let (_sender, events) = mpsc::channel();
            (worker, events)
        } else {
            backend::start()
        };
        let mut app = Self::with_worker(worker, events);
        if gallery {
            gallery::populate(&mut app);
            if std::env::args().any(|a| a == "--ui-small") {
                app.width = 840.0;
            }
            if std::env::args().any(|a| a == "--ui-large-density") {
                app.preferences.scale = Scale::Large;
            }
            if let Some(name) = std::env::args()
                .find_map(|argument| argument.strip_prefix("--ui-scene=").map(str::to_string))
            {
                if let Some(scene) = gallery::Scene::ALL
                    .iter()
                    .find(|scene| scene.slug() == name)
                {
                    gallery::scene(&mut app, *scene);
                }
            }
        }
        (app, Command::none())
    }
    fn title(&self) -> String {
        "PalantirMC · your next world".into()
    }
    fn theme(&self) -> Theme {
        design::theme(self.preferences.accent)
    }
    fn scale_factor(&self) -> f64 {
        f64::from(self.preferences.scale.factor())
    }
    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced::time::every(Duration::from_millis(100)).map(|_| Message::Tick),
            iced::event::listen_with(|event, status| {
                // Dialogs capture input, but global escape/focus keys must still reach the app.
                let shortcut = matches!(&event, iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Character(key), modifiers, ..
                }) if modifiers.control() && matches!(key.as_str(), "1" | "2" | "3" | ","));
                if status == iced::event::Status::Ignored
                    || shortcut
                    || matches!(
                        &event,
                        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                            key: iced::keyboard::Key::Named(
                                iced::keyboard::key::Named::Escape
                                    | iced::keyboard::key::Named::Tab
                                    | iced::keyboard::key::Named::F1
                            ),
                            ..
                        })
                    )
                {
                    Some(Message::Event(event))
                } else {
                    None
                }
            }),
        ])
    }
    fn update(&mut self, message: Message) -> Command<Message> {
        let reset_scroll = matches!(
            &message,
            Message::Navigate(_)
                | Message::Edition(_)
                | Message::Select(_)
                | Message::InstanceTab(_)
                | Message::SettingsTab(_)
                | Message::Project(_)
                | Message::CloseProject
                | Message::ProjectTab(_)
                | Message::ReleaseChangelog(_)
                | Message::Search
                | Message::Next
                | Message::Previous
        );
        match message {
            Message::GalleryScene(scene) => {
                if self.gallery {
                    gallery::scene(self, scene);
                    return scrollable::snap_to(
                        scrollable::Id::new("page"),
                        scrollable::RelativeOffset::START,
                    );
                }
            }
            Message::Draft(edit) => {
                self.draft.edit(edit);
                self.local.invalidate();
            }
            Message::LocalClosed(value) => self.local.closed_game = value,
            Message::ActivationWorld(folder) => {
                self.local.activation_world = if folder == "Do not activate" {
                    None
                } else {
                    Some(folder)
                };
                self.local.invalidate();
            }
            Message::ReviewLocal => match self.local_input() {
                Ok(input) => {
                    let key = input.target().key();
                    if self.running.contains_key(&key) || self.pending.contains_key(&key) {
                        self.note("Finish the collection's session or task first.".into());
                    } else {
                        self.local.invalidate();
                        self.local.loading = true;
                        self.send(backend::Request::ReviewLocal(self.local.revision, input));
                    }
                }
                Err(error) => self.note(error),
            },
            Message::ApplyLocal => {
                if self.local.closed_game && !self.local.loading {
                    if let Some(plan) = self.local.plan.clone() {
                        let key = plan.input.target().key();
                        if self.local_input().is_ok_and(|input| input == plan.input)
                            && !self.running.contains_key(&key)
                            && !self.pending.contains_key(&key)
                            && !(matches!(plan.input.target(), local::Target::Bedrock(_))
                                && self.pending.contains_key("bedrock-launch"))
                        {
                            self.pending
                                .insert(key, "Writing reviewed local files…".into());
                            self.local.invalidate();
                            self.send(backend::Request::ApplyLocal(plan));
                        }
                    }
                }
            }
            Message::BedrockTab(tab) => {
                self.bedrock.tab = tab;
                return scrollable::snap_to(
                    scrollable::Id::new("page"),
                    scrollable::RelativeOffset::START,
                );
            }
            Message::BedrockQuery(value) => self.bedrock.query = value,
            Message::Edition(edition) => {
                if self.edition != edition {
                    if matches!(self.dialog, Some(Dialog::Preferences)) {
                        self.preferences = self.settings_baseline();
                        self.sync_settings_drafts();
                    }
                    self.edition = edition;
                    self.page = Page::Home;
                    self.dialog = None;
                    self.dialog_stack.clear();
                    self.local.invalidate();
                    self.selected = None;
                    self.notice = None;
                    if edition == Edition::Bedrock && !self.bedrock.checked {
                        let _ = self.update(Message::BedrockRefresh);
                    }
                }
            }
            Message::BedrockRefresh => {
                if !self.bedrock.loading {
                    self.local.invalidate();
                    self.bedrock.loading = true;
                    self.bedrock.error = None;
                    self.bedrock.selected = None;
                    self.bedrock.entries.clear();
                    self.bedrock.worlds.clear();
                    self.bedrock.files_error = None;
                    self.bedrock.reading = false;
                    self.send(backend::Request::BedrockDetect);
                }
            }
            Message::BedrockLaunch(channel) => {
                if self.edition == Edition::Bedrock
                    && self
                        .bedrock
                        .installs
                        .iter()
                        .any(|copy| copy.channel == channel)
                    && !self.pending.contains_key("bedrock-launch")
                    && !self.pending.contains_key("bedrock-local")
                {
                    self.pending
                        .insert("bedrock-launch".into(), "Opening Bedrock…".into());
                    self.send(backend::Request::BedrockLaunch(channel));
                }
            }
            Message::BedrockInspect(channel) => {
                if self.edition == Edition::Bedrock
                    && self
                        .bedrock
                        .installs
                        .iter()
                        .any(|copy| copy.channel == channel)
                {
                    self.local.invalidate();
                    self.bedrock.selected = Some(channel);
                    self.bedrock.reading = true;
                    self.bedrock.entries.clear();
                    self.bedrock.worlds.clear();
                    self.bedrock.files_error = None;
                    self.send(backend::Request::BedrockInventory(channel));
                }
            }
            Message::BedrockFolder(channel, folder) => {
                if self.edition == Edition::Bedrock
                    && self
                        .bedrock
                        .installs
                        .iter()
                        .any(|copy| copy.channel == channel)
                {
                    self.send(backend::Request::BedrockFolder(channel, folder));
                }
            }
            Message::Navigate(Page::Settings) => {
                self.notice = None;
                self.dialog_stack.clear();
                self.dialog = Some(Dialog::Preferences);
            }
            Message::Navigate(page) => {
                self.page = page;
                self.selected = None;
                self.dialog = None;
                self.dialog_stack.clear();
                if self.edition == Edition::Java
                    && page == Page::Discover
                    && self.results.is_empty()
                    && !self.searching
                {
                    self.search();
                }
            }
            Message::Event(event) => match event {
                iced::Event::Window(_, iced::window::Event::Resized { width, .. }) => {
                    self.width = width as f32
                }
                iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key, modifiers, ..
                }) => {
                    use iced::keyboard::{Key, key::Named};
                    match key {
                        Key::Named(Named::Escape) => {
                            if self.dialog.is_some() {
                                return self.update(Message::CloseDialog);
                            } else if self.project.is_some() {
                                self.project = None;
                            } else {
                                self.selected = None;
                            }
                        }
                        Key::Named(Named::F1) => {
                            return self
                                .update(Message::Open(Dialog::Surface(surfaces::Panel::Help)));
                        }
                        Key::Named(Named::Tab) => {
                            return if modifiers.shift() {
                                iced::widget::focus_previous()
                            } else {
                                iced::widget::focus_next()
                            };
                        }
                        Key::Character(ref key)
                            if modifiers.control() && key == "1" && self.dialog.is_none() =>
                        {
                            return self.update(Message::Navigate(Page::Home));
                        }
                        Key::Character(ref key)
                            if modifiers.control() && key == "2" && self.dialog.is_none() =>
                        {
                            return self.update(Message::Navigate(Page::Library));
                        }
                        Key::Character(ref key)
                            if modifiers.control() && key == "3" && self.dialog.is_none() =>
                        {
                            return self.update(Message::Navigate(Page::Discover));
                        }
                        Key::Character(ref key)
                            if modifiers.control() && key == "," && self.dialog.is_none() =>
                        {
                            return self.update(Message::Navigate(Page::Settings));
                        }
                        _ => {}
                    }
                }
                _ => {}
            },
            Message::Tick => {
                if self.gallery {
                    return Command::none();
                }
                let mut count = 0;
                while count < 250 {
                    match self.events.try_recv() {
                        Ok(event) => {
                            count += 1;
                            self.receive(event);
                        }
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            self.worker = None;
                            self.local.invalidate();
                            self.local.error = Some(
                                "The background service stopped. Restart the launcher.".into(),
                            );
                            self.pending.clear();
                            break;
                        }
                    }
                }
                if self.search_dirty && std::time::Instant::now() >= self.search_due {
                    self.search_dirty = false;
                    self.offset = 0;
                    self.project = None;
                    self.search();
                }
            }
            Message::CloseDialog => {
                // A dialog opened over a visit closes back into it:
                // completing or cancelling a step must not end the visit it
                // is part of (found by the UI smoke, not a test).
                if let Some(parent) = self.dialog_stack.pop() {
                    self.dialog = Some(parent);
                    return Command::none();
                }
                if matches!(self.dialog, Some(Dialog::Preferences)) {
                    self.preferences = self.settings_baseline();
                    self.sync_settings_drafts();
                }
                self.dialog = None;
                self.local.invalidate();
                self.invalidate_content_review();
                self.import_id += 1;
                self.import_loading = false;
                self.pack_loading = false;
            }
            Message::Open(dialog) => {
                self.notice = None;
                self.local.invalidate();
                // A dialog opened over a visit dialog remembers it, so the
                // child closes back into the visit; anything opened over the
                // page (or over a step of another flow) starts fresh.
                match self.dialog.clone() {
                    Some(parent) if matches!(parent, Dialog::Preferences | Dialog::Account) => {
                        self.dialog_stack.push(parent)
                    }
                    _ => self.dialog_stack.clear(),
                }
                if matches!(dialog, Dialog::Delete(_)) {
                    self.delete_pending = false;
                    self.delete_outcome = None;
                }
                if let Dialog::Surface(panel) = &dialog {
                    match panel {
                        surfaces::Panel::Launch(_) => {
                            self.draft.launch_mode = surfaces::LaunchMode::Offline
                        }
                        surfaces::Panel::Addition(_) => {
                            self.draft.addition_kind = if self.edition == Edition::Bedrock {
                                surfaces::AdditionKind::BedrockPack
                            } else {
                                surfaces::AdditionKind::Mod
                            }
                        }
                        _ => {}
                    }
                }
                if let Dialog::Rename(id) = &dialog {
                    self.name = self
                        .instances
                        .iter()
                        .find(|i| &i.id == id)
                        .map(|i| i.name.clone())
                        .unwrap_or_default();
                } else if (matches!(dialog, Dialog::Import)
                    && !matches!(self.dialog, Some(Dialog::Browser | Dialog::ImportReview)))
                    || (matches!(dialog, Dialog::Create)
                        && !matches!(self.dialog, Some(Dialog::Versions)))
                {
                    self.name.clear();
                }
                if matches!(dialog, Dialog::Create)
                    && self.game_choices.is_empty()
                    && !self.versions_loading
                {
                    self.versions_loading = true;
                    self.send(backend::Request::GameVersions);
                }
                if matches!(dialog, Dialog::Browser) {
                    self.browse_loading = true;
                    self.send(backend::Request::Browse(self.browser_path.clone()));
                }
                if matches!(dialog, Dialog::SignIn) {
                    self.auth_error = false;
                }
                let review_target = if let Dialog::InstallReview(id) = &dialog {
                    Some(id.clone())
                } else {
                    None
                };
                self.invalidate_content_review();
                let focus = matches!(dialog, Dialog::Create | Dialog::Import | Dialog::Rename(_));
                self.dialog = Some(dialog);
                if let Some(id) = review_target {
                    let _ = self.update(Message::ReviewContent(id));
                }
                let scroll = scrollable::snap_to(
                    scrollable::Id::new("dialog-body"),
                    scrollable::RelativeOffset::START,
                );
                return if focus {
                    Command::batch([
                        scroll,
                        text_input::focus(text_input::Id::new("dialog-first")),
                    ])
                } else {
                    scroll
                };
            }
            Message::Name(value) => self.name = value,
            Message::Game(value) => {
                self.game = value;
                self.loader_pin.clear();
                self.loader_versions.clear();
            }
            Message::ImportPath(value) => {
                self.import_path = value;
                self.import_review = None;
                self.import_id += 1;
                self.import_loading = false;
            }
            Message::Loader(value) => {
                self.loader = value;
                self.loader_pin.clear();
                self.loader_versions.clear();
                // Every loader but vanilla publishes releases to pin: Fabric and
                // Quilt through their meta services, the Forge family through
                // the vendor Maven's own metadata.
                if value != Loader::Vanilla {
                    self.loader_loading = true;
                    self.send(backend::Request::LoaderVersions(self.game.clone(), value));
                }
            }
            Message::Create => {
                if self.name.trim().is_empty() || model::safe_component(self.game.trim()).is_err() {
                    self.note("Enter a collection name and a valid game version.".into());
                    return Command::none();
                }
                if self.pending.contains_key("create") {
                    return Command::none();
                }
                self.pending
                    .insert("create".into(), "Creating a collection…".into());
                self.send(backend::Request::Create(
                    self.name.clone(),
                    self.game.clone(),
                    self.loader,
                    self.loader_pin.clone(),
                ));
            }
            Message::Import => {
                if !self
                    .import_review
                    .as_ref()
                    .is_some_and(|r| r.source == self.import_path)
                {
                    self.note("Inspect and review this source before importing it.".into());
                    return Command::none();
                }
                if self.pending.contains_key("import") {
                    return Command::none();
                }
                let archive = self.import_path.to_ascii_lowercase().ends_with(".mrpack");
                // A reviewed archive names its own game and loader, so those win
                // over whatever the setup fields happen to hold; a directory
                // copy carries no such metadata and uses them.
                let (game, loader) = match &self.import_review {
                    Some(review) if !review.game.is_empty() => (review.game.clone(), review.loader),
                    _ => (self.game.clone(), self.loader),
                };
                // The optional client files are the ones ticked on the review. An
                // explicit decision -- even an empty one -- is what lets an
                // archive carrying optional files install at all: the worker
                // refuses a pack whose optional files were never decided.
                let selected = Some(self.optional_files.clone());
                self.pending.insert(
                    "import".into(),
                    if archive {
                        "Installing the pack archive…".into()
                    } else {
                        "Copying a local collection…".into()
                    },
                );
                self.send(backend::Request::Import(
                    self.import_path.clone(),
                    self.name.clone(),
                    game,
                    loader,
                    selected,
                    self.import_review
                        .as_ref()
                        .and_then(|review| review.archive_sha1.clone()),
                ));
            }
            Message::Rename(id) => {
                self.send(backend::Request::Rename(id, self.name.clone()));
            }
            Message::Delete(id) => {
                // The receipt belongs in this dialog: it stays open and shows
                // the outcome, the way reviewed file operations show theirs --
                // never only in a banner behind the scrim.
                self.delete_pending = true;
                self.delete_outcome = None;
                self.send(backend::Request::Delete(id));
            }
            Message::Select(id) => {
                // "Open collection" also comes from the actions menu; choosing
                // it ends that menu visit.
                self.dialog = None;
                self.edition = Edition::Java;
                self.selected = Some(id.clone());
                self.page = Page::Library;
                self.instance_tab = InstanceTab::Overview;
                self.inventory.clear();
                self.inventory_loading = true;
                self.send(backend::Request::Inventory(id));
            }
            Message::SelectTab(id, tab) => {
                // Land on one tab of the collection -- a hidden tab falls back
                // to the first visible one like any other selection.
                let command = self.update(Message::Select(id));
                self.instance_tab = tab;
                self.normalize_instance_tab();
                return command;
            }
            Message::InstanceTab(tab) => {
                // A hidden tab is not selectable, including by a stale
                // selection or any keyboard path.
                if tab.shown(&self.preferences.features) {
                    self.instance_tab = tab;
                    if let Some(id) = self.selected.clone() {
                        self.inventory_loading = true;
                        self.send(backend::Request::Inventory(id));
                    }
                }
            }
            Message::Launch(id) => {
                // Starting the game ends the review panel, but not the visit
                // it may have been opened over: play from the Account
                // dialog's "Review play mode" returns to Account.
                self.dialog = self.dialog_stack.pop();
                self.pending
                    .insert(id.clone(), "Preparing game files…".into());
                // The identity the review screen chose travels with the request:
                // the worker must never decide on its own that an offline
                // session is good enough when a Microsoft one was asked for.
                self.send(backend::Request::Launch(
                    id,
                    self.preferences.clone(),
                    self.draft.launch_mode,
                ));
            }
            Message::Stop(id) => self.send(backend::Request::Stop(id)),
            Message::OpenFolder(id, folder) => self.send(backend::Request::OpenFolder(id, folder)),
            Message::ToggleMod(id, name) => self.send(backend::Request::ToggleMod(id, name)),
            Message::Query(value) => {
                self.query = value;
                self.schedule_search();
            }
            Message::FilterGame(value) => {
                self.filter_game = value;
                self.schedule_search();
            }
            Message::Kind(value) => {
                self.kind = value;
                self.offset = 0;
                self.project = None;
                self.search();
            }
            Message::Sort(value) => {
                self.sort = value;
                self.offset = 0;
                self.search();
            }
            Message::Search => {
                self.search_dirty = false;
                self.offset = 0;
                self.project = None;
                self.search();
            }
            Message::Next => {
                self.offset += 12;
                self.search();
            }
            Message::Previous => {
                self.offset = self.offset.saturating_sub(12);
                self.search();
            }
            Message::Project(project) => {
                self.detail = None;
                self.detail_loading = true;
                self.send(backend::Request::Project(project.project_id.clone()));
                self.send(backend::Request::Releases(project.project_id.clone()));
                self.releases.clear();
                self.releases_loading = true;
                self.release_id = None;
                self.project_tab = ProjectTab::Description;
                self.release_filter.clear();
                self.visible_releases = 30;
                self.project = Some(project);
            }
            Message::CloseProject => {
                self.project = None;
                self.detail = None;
            }
            Message::ReviewContent(id) => {
                self.invalidate_content_review();
                if self.edition == Edition::Java
                    && matches!(&self.dialog, Some(Dialog::InstallReview(target)) if target == &id)
                    && !self.pending.contains_key(&id)
                    && !self.running.contains_key(&id)
                {
                    if let (Some(project), Some(release)) = (&self.project, self.selected_release())
                    {
                        if project.project_type != "modpack"
                            && release.project_id == project.project_id
                        {
                            let request = backend::Request::ReviewContent(
                                self.content_review_id,
                                id,
                                project.clone(),
                                release.id.clone(),
                            );
                            self.content_review_loading = true;
                            self.send(request);
                        }
                    }
                }
            }
            Message::InstallContent(id) => {
                if self.running.contains_key(&id) || self.pending.contains_key(&id) {
                    self.note(
                        "Stop the session and wait for its active task before installing.".into(),
                    );
                    return Command::none();
                }
                if let Some(project) = self.project.clone() {
                    let eligible = self
                        .content_plan
                        .as_ref()
                        .is_some_and(|plan| self.content_plan_matches(&id, plan))
                        && !self.content_review_loading
                        && self.edition == Edition::Java
                        && matches!(&self.dialog, Some(Dialog::InstallReview(target)) if target == &id);
                    if !eligible || project.project_type == "modpack" {
                        self.note("Resolve and review the exact files for this collection before installing.".into());
                        return Command::none();
                    }
                    self.pending
                        .insert(id.clone(), "Resolving a compatible release…".into());
                    let Some(plan) = self.content_plan.take() else {
                        return Command::none();
                    };
                    self.send(backend::Request::Content(id, project, plan));
                    self.dialog = None;
                }
            }
            Message::SettingsTab(tab) => {
                self.settings_tab = tab;
                return scrollable::snap_to(
                    scrollable::Id::new("dialog-body"),
                    scrollable::RelativeOffset::START,
                );
            }
            Message::EditPlayer => {
                self.settings_tab = SettingsTab::Java;
                return self.update(Message::Navigate(Page::Settings));
            }
            Message::AccountName(value) => self.account_name = value,
            Message::AccountAdd => match self.preferences.add_offline_account(&self.account_name) {
                Ok(()) => {
                    self.account_name.clear();
                    // Save immediately: an identity the interface already shows as
                    // active must not disappear on the next start.
                    self.save_preferences_now("Offline account added and selected.".into());
                }
                Err(error) => self.note(error),
            },
            Message::AccountSelect(id) => {
                if let Err(error) = self.preferences.select_offline_account(&id) {
                    self.note(error);
                } else {
                    self.save_preferences_now("Preferences saved.".into());
                }
            }
            Message::AccountRemove(id) => {
                // The removal was confirmed inside the Account visit; its
                // receipt lands back there.
                self.dialog = self.dialog_stack.pop().or(Some(Dialog::Account));
                if let Err(error) = self.preferences.remove_offline_account(&id) {
                    self.note(error);
                } else {
                    self.save_preferences_now("Offline account removed.".into());
                }
            }
            Message::Player(value) => self.preferences.player_name = value,
            Message::Java(value) => self.preferences.java = value,
            Message::Memory(value) => self.memory = value,
            Message::WindowWidth(value) => self.window_width = value,
            Message::WindowHeight(value) => self.window_height = value,
            Message::JavaAddOpen => {
                self.java_add_path.clear();
                // The add dialog is opened from the roster in Preferences;
                // remember the visit so closing or completing returns there.
                match self.dialog.clone() {
                    Some(parent) if matches!(parent, Dialog::Preferences | Dialog::Account) => {
                        self.dialog_stack.push(parent)
                    }
                    _ => self.dialog_stack.clear(),
                }
                self.dialog = Some(Dialog::JavaAdd);
                return Command::batch([
                    scrollable::snap_to(
                        scrollable::Id::new("dialog-body"),
                        scrollable::RelativeOffset::START,
                    ),
                    text_input::focus(text_input::Id::new("dialog-first")),
                ]);
            }
            Message::JavaAddPath(value) => self.java_add_path = value,
            Message::JavaAddSave => {
                let path = self.java_add_path.trim().to_string();
                if path.is_empty() || path.chars().any(char::is_control) {
                    self.note("Enter a plain executable path.".to_string());
                } else if self
                    .preferences
                    .java_installs
                    .iter()
                    .any(|install| install.path == path)
                {
                    self.note("That runtime is already in the list.".to_string());
                } else {
                    self.send(backend::Request::JavaProbe(path));
                }
            }
            Message::JavaUse(path) => {
                self.preferences.java = path;
                self.save_preferences_now("Java runtime selected for launches.".into());
            }
            Message::JavaRemove(path) => {
                self.preferences
                    .java_installs
                    .retain(|install| install.path != path);
                if self.preferences.java == path {
                    self.preferences.java.clear();
                }
                self.save_preferences_now("Java runtime removed from the list.".to_string());
            }
            Message::DuplicateOpen(id) => {
                if let Some(item) = self.instances.iter().find(|item| item.id == id) {
                    self.name = format!("Copy of {}", item.name);
                }
                self.dialog = Some(Dialog::Duplicate(id));
                return Command::batch([
                    scrollable::snap_to(
                        scrollable::Id::new("dialog-body"),
                        scrollable::RelativeOffset::START,
                    ),
                    text_input::focus(text_input::Id::new("dialog-first")),
                ]);
            }
            Message::DuplicateSave(id) => {
                let name = self.name.trim();
                if name.is_empty()
                    || name.chars().count() > 80
                    || name.chars().any(char::is_control)
                {
                    self.note("Choose a name between 1 and 80 characters.".to_string());
                } else {
                    self.send(backend::Request::Duplicate(id, self.name.clone()));
                }
            }
            Message::InstanceWindowOpen(id) => {
                let window = self
                    .instances
                    .iter()
                    .find(|item| item.id == id)
                    .and_then(|item| item.window);
                self.instance_window_width =
                    window.map_or_else(String::new, |w| draft_side(w.width));
                self.instance_window_height =
                    window.map_or_else(String::new, |w| draft_side(w.height));
                self.dialog = Some(Dialog::InstanceWindow(id));
                // The same first-field focus the create/rename dialogs get:
                // a dialog whose fields cannot be typed into is not a dialog.
                return Command::batch([
                    scrollable::snap_to(
                        scrollable::Id::new("dialog-body"),
                        scrollable::RelativeOffset::START,
                    ),
                    text_input::focus(text_input::Id::new("dialog-first")),
                ]);
            }
            Message::InstanceWindowWidth(value) => self.instance_window_width = value,
            Message::InstanceWindowHeight(value) => self.instance_window_height = value,
            Message::InstanceWindowSave(id) => {
                match model::parse_window_size(
                    &self.instance_window_width,
                    &self.instance_window_height,
                ) {
                    // A blank pair means "follow the launcher": no override stored.
                    Ok((width, height)) => {
                        let window = (width > 0).then_some(model::WindowOverride { width, height });
                        self.send(backend::Request::Window(id, window));
                    }
                    Err(error) => self.note(error),
                }
            }
            Message::InstanceWindowClear(id) => self.send(backend::Request::Window(id, None)),
            Message::InstanceRuntimeOpen(id) => {
                let settings = self
                    .instances
                    .iter()
                    .find(|item| item.id == id)
                    .map(|item| (item.memory_mb, item.java.clone(), item.custom.clone()));
                if let Some((memory_mb, java, custom)) = settings {
                    self.instance_memory = memory_mb.map_or_else(String::new, |m| m.to_string());
                    self.instance_java = java.unwrap_or_default();
                    let (args, env, pre, wrapper, post) = model::launch_custom_text(&custom);
                    self.instance_args = args;
                    self.instance_env = env;
                    self.instance_hook_pre = pre;
                    self.instance_hook_wrapper = wrapper;
                    self.instance_hook_post = post;
                }
                self.dialog = Some(Dialog::InstanceRuntime(id));
                return Command::batch([
                    scrollable::snap_to(
                        scrollable::Id::new("dialog-body"),
                        scrollable::RelativeOffset::START,
                    ),
                    text_input::focus(text_input::Id::new("dialog-first")),
                ]);
            }
            Message::InstanceRuntimeMemory(value) => self.instance_memory = value,
            Message::InstanceRuntimeJava(value) => self.instance_java = value,
            Message::InstanceRuntimeArgs(value) => self.instance_args = value,
            Message::InstanceRuntimeEnv(value) => self.instance_env = value,
            Message::InstanceRuntimeHookPre(value) => self.instance_hook_pre = value,
            Message::InstanceRuntimeHookWrapper(value) => self.instance_hook_wrapper = value,
            Message::InstanceRuntimeHookPost(value) => self.instance_hook_post = value,
            Message::InstanceRuntimeSave(id) => {
                let saved =
                    model::parse_runtime_overrides(&self.instance_memory, &self.instance_java)
                        .and_then(|(memory_mb, java)| {
                            let custom = model::parse_launch_custom(
                                &self.instance_args,
                                &self.instance_env,
                                &self.instance_hook_pre,
                                &self.instance_hook_wrapper,
                                &self.instance_hook_post,
                            )?;
                            Ok((memory_mb, java, custom))
                        });
                match saved {
                    // Blank fields mean "follow the launcher" or "none":
                    // nothing is stored that the player did not type.
                    Ok((memory_mb, java, custom)) => {
                        self.send(backend::Request::Runtime(id, memory_mb, java, custom))
                    }
                    Err(error) => self.note(error),
                }
            }
            Message::InstanceRuntimeClear(id) => self.send(backend::Request::Runtime(
                id,
                None,
                None,
                palantir_loader::hooks::LaunchCustom::default(),
            )),
            Message::Accent(value) => self.preferences.accent = value,
            Message::Scale(value) => self.preferences.scale = value,
            Message::Activity(value) => self.preferences.show_activity = value,
            Message::FeatureOverview(value) => {
                self.preferences.features.show_overview = value;
                self.normalize_instance_tab();
            }
            Message::FeatureContent(value) => {
                self.preferences.features.show_content = value;
                self.normalize_instance_tab();
            }
            Message::FeatureWorlds(value) => {
                self.preferences.features.show_worlds = value;
                self.normalize_instance_tab();
            }
            Message::FeatureLogs(value) => {
                self.preferences.features.show_logs = value;
                self.normalize_instance_tab();
            }
            Message::FeatureManage(value) => {
                self.preferences.features.show_manage = value;
                self.normalize_instance_tab();
            }
            // Every save carries the whole settings snapshot and the worker
            // writes them in order, so a save while another is in flight is
            // safe -- and swallowing it would drop the click entirely.
            Message::SaveSettings => {
                let window = model::parse_window_size(&self.window_width, &self.window_height);
                match (self.memory.parse::<u32>(), window) {
                    (Ok(memory), Ok((width, height))) if (512..=32768).contains(&memory) => {
                        self.preferences.memory_mb = memory;
                        self.preferences.window_width = width;
                        self.preferences.window_height = height;
                        self.preferences.last_page = match self.page {
                            Page::Library => "library",
                            Page::Discover => "discover",
                            Page::About => "about",
                            _ => "home",
                        }
                        .into();
                        if let Err(error) = self.preferences.validate() {
                            self.note(error);
                        } else {
                            self.save_preferences_now("Preferences saved.".into());
                        }
                    }
                    // The window pair is checked first: a half-filled pair is a
                    // specific mistake and deserves its own sentence.
                    (_, Err(error)) => self.note(error),
                    _ => self.notice = Some("Use 512–32768 MiB for the memory budget.".into()),
                }
            }
            Message::ClearLogs => {
                if let Some(id) = &self.selected {
                    self.logs.remove(id);
                }
            }
            Message::CopyLogs => {
                if let Some(id) = &self.selected {
                    let text = self
                        .logs
                        .get(id)
                        .map(|lines| lines.iter().cloned().collect::<Vec<_>>().join("\n"))
                        .unwrap_or_default();
                    return iced::clipboard::write(text);
                }
            }
            Message::Dismiss => self.notice = None,
            Message::ShowActivity => {
                self.preferences.show_activity = !self.preferences.show_activity
            }
            Message::LibraryQuery(value) => self.library_query = value,
            Message::LoadVersions => {
                self.versions_loading = true;
                self.send(backend::Request::GameVersions);
            }
            Message::ChooseGame(game) => {
                self.game = game;
                self.dialog = Some(Dialog::Create);
                self.loader_pin.clear();
                if self.loader != Loader::Vanilla {
                    self.loader_loading = true;
                    self.send(backend::Request::LoaderVersions(
                        self.game.clone(),
                        self.loader,
                    ));
                }
            }
            Message::IncludeSnapshots(value) => self.include_snapshots = value,
            Message::VersionsQuery(value) => self.versions_query = value,
            Message::ReleaseFilter(value) => {
                self.release_filter = value;
                self.visible_releases = 30;
            }
            Message::MoreReleases => self.visible_releases += 30,
            Message::LoaderPin(value) => {
                self.loader_pin = if value == "Automatic" {
                    String::new()
                } else {
                    value
                }
            }
            Message::FetchPack => {
                if !self.pack_loading
                    && self.edition == Edition::Java
                    && matches!(self.dialog, Some(Dialog::PackSetup))
                {
                    if let (Some(project), Some(release)) =
                        (self.project.clone(), self.selected_release().cloned())
                    {
                        if project.project_type == "modpack"
                            && release.project_id == project.project_id
                        {
                            self.import_id += 1;
                            self.import_review = None;
                            self.import_loading = false;
                            self.pack_loading = true;
                            self.send(backend::Request::FetchPack(
                                self.import_id,
                                project,
                                release,
                            ));
                        }
                    }
                }
            }
            Message::InspectImport => {
                self.import_id += 1;
                self.import_loading = true;
                self.import_review = None;
                self.send(backend::Request::InspectSource(
                    self.import_id,
                    self.import_path.clone(),
                ));
            }
            Message::BrowsePath(path) => self.browser_path = path,
            Message::BrowsePathAndRead(path) => {
                self.browser_path = path;
                return self.update(Message::BrowseFolder);
            }
            Message::ShowReleases => {
                self.dialog = None;
                self.project_tab = ProjectTab::Releases;
            }
            Message::ReleaseChangelog(id) => {
                self.release_id = Some(id);
                self.project_tab = ProjectTab::Changelog;
            }
            Message::BrowseFolder => {
                self.browse_loading = true;
                self.send(backend::Request::Browse(self.browser_path.clone()));
            }
            Message::ChooseSource(path) => {
                self.import_path = path;
                self.dialog = Some(Dialog::Import);
                return self.update(Message::InspectImport);
            }
            Message::OptionalFile(path, include) => {
                self.optional_files.retain(|p| p != &path);
                if include {
                    self.optional_files.push(path);
                }
            }
            Message::ProjectTab(tab) => self.project_tab = tab,
            Message::ChooseRelease(id) => {
                self.release_id = Some(id);
                self.dialog = Some(Dialog::Install);
            }
            Message::RetryProject => {
                if let Some(project) = self.project.clone() {
                    return self.update(Message::Project(project));
                }
            }
            Message::ResetFilters => {
                self.search_dirty = false;
                self.query.clear();
                self.filter_game.clear();
                self.offset = 0;
                self.sort = Sort::Relevance;
                self.search();
            }
            Message::OpenUrl(url) => self.send(backend::Request::OpenUrl(url)),
            Message::ResetSettings => {
                self.preferences = Preferences::default();
                self.sync_settings_drafts();
            }
            Message::RevertSettings => {
                self.preferences = self.settings_baseline();
                self.sync_settings_drafts();
                self.notice = None;
            }
            Message::RememberPage(value) => self.preferences.remember_page = value,
            Message::CopyDiagnostics => {
                return iced::clipboard::write(format!(
                    "PalantirMC {}\nOS: {}\nEdition: {}\nData: {}\nJava collections: {}\nWorker: {}\n{}",
                    env!("CARGO_PKG_VERSION"),
                    std::env::consts::OS,
                    self.edition.name(),
                    self.root,
                    self.instances.len(),
                    self.worker.is_some(),
                    self.notice.as_deref().unwrap_or("No current error")
                ));
            }
            Message::AuthStart => {
                self.authentication = model::Authentication::Requesting;
                self.send(backend::Request::Authenticate);
            }
            Message::AuthCancel => self.send(backend::Request::CancelAuthentication),
            Message::CopyDeviceCode => {
                if let model::Authentication::DeviceCode { code, .. } = &self.authentication {
                    return iced::clipboard::write(code.clone());
                }
            }
            Message::ClearHistory => self.history.clear(),
            Message::RefreshInventory => {
                if let Some(id) = self.selected.clone() {
                    self.inventory_loading = true;
                    self.send(backend::Request::Inventory(id));
                }
            }
            Message::RemoveContent(id, folder, name) => {
                self.dialog = None;
                self.send(backend::Request::RemoveContent(id, folder, name));
            }
        }
        if reset_scroll {
            // Reusing the page scroll state must not hide a new page's header
            // after reading long project notes or the inline activity notebook.
            scrollable::snap_to(
                scrollable::Id::new("page"),
                scrollable::RelativeOffset::START,
            )
        } else {
            Command::none()
        }
    }
    fn view(&self) -> Element<'_, Message> {
        let compact = self.width / self.preferences.scale.factor() < 1100.0;
        // At the compact sidebar width the wordmark shares its line with the
        // mark, so it takes the tighter of the two sizes: the lockup stays on
        // one line of tagline instead of wrapping mid-phrase.
        let (mark, gap, tagline) = if compact {
            (30.0, 8.0, 9.0)
        } else {
            (38.0, 10.0, 10.0)
        };
        let mut nav = column![
            row![
                design::mark(mark),
                column![
                    subtitle("PALANTIR"),
                    muted("FIELD NOTES / WORLDS").size(tagline)
                ]
                .spacing(3)
            ]
            .spacing(gap),
            Space::with_height(22)
        ]
        .spacing(8);
        nav = nav.push(self.edition_switch()).push(Space::with_height(10));
        for (page, name, icon) in [
            (Page::Home, Key::Home, "home"),
            (Page::Library, Key::Library, "library"),
            (Page::Discover, Key::Discover, "discover"),
            (Page::Settings, Key::Settings, "settings"),
            (Page::About, Key::About, "about"),
        ] {
            if self.edition == Edition::Bedrock && page == Page::Discover {
                continue;
            }
            let message = Message::Navigate(page);
            nav = nav.push(controls::content(
                button(
                    row![
                        design::icon(icon, self.page == page),
                        label(
                            if self.edition == Edition::Bedrock && page == Page::Library {
                                "Installed copies"
                            } else {
                                locale::message(name, &self.preferences.language)
                            }
                        )
                    ]
                    .spacing(12)
                    .align_items(iced::Alignment::Center),
                )
                .padding([12, 14])
                .width(Length::Fill)
                .style(if self.page == page {
                    Button::Selected
                } else {
                    Button::Quiet
                })
                .on_press(message.clone())
                .into(),
                message,
            ));
        }
        nav = nav
            .push(Space::with_height(18))
            .push(action(
                &if self.edition == Edition::Bedrock {
                    "Windows / Xbox account".into()
                } else {
                    format!("{} · offline", self.preferences.active_identity().1)
                },
                Message::Open(Dialog::Account),
                Button::Quiet,
            ))
            .push(action(
                "Help / F1",
                Message::Open(Dialog::Surface(surfaces::Panel::Help)),
                Button::Quiet,
            ))
            .push(muted("Ctrl 1 / 2 / 3 · Esc to return").size(10));
        // The nav column carries the sidebar's padding inside the scrollable
        // so the navigation bar is drawn over that padding rather than over
        // the wordmark and the buttons' right edges.
        let sidebar = container(scrollable(nav.padding(18)).id(scrollable::Id::new("navigation")))
            .width(if compact { 190 } else { 220 })
            .height(Length::Fill)
            .style(design::chrome);
        let mut content = column![
            row![
                muted(match self.page {
                    Page::Home => "FIELD NOTES / HOME",
                    Page::Library if self.edition == Edition::Bedrock =>
                        "FIELD NOTES / INSTALLED COPIES",
                    Page::Library => "FIELD NOTES / COLLECTIONS",
                    Page::Discover => "FIELD NOTES / EXPLORE",
                    Page::Settings => "FIELD NOTES / PREFERENCES",
                    Page::About => "FIELD NOTES / ABOUT",
                })
                .size(11),
                Space::with_width(Length::Fill),
                action(
                    if self.preferences.show_activity {
                        "Hide activity"
                    } else {
                        "Activity"
                    },
                    Message::ShowActivity,
                    Button::Quiet
                )
            ]
            .spacing(8)
            .align_items(iced::Alignment::Center)
        ]
        .spacing(18);
        if self.worker.is_none() {
            content = content.push(card(
                column![
                    subtitle("Background service unavailable"),
                    muted("Restart the launcher to reconnect. Your saved collection is unchanged.")
                ]
                .spacing(8),
            ));
        }
        if let Some(notice) = &self.notice {
            content = content.push(
                container(
                    row![
                        label(notice).width(Length::Fill),
                        action("Copy details", Message::CopyDiagnostics, Button::Quiet),
                        action("Dismiss", Message::Dismiss, Button::Quiet)
                    ]
                    .spacing(10),
                )
                .padding(14)
                .style(design::well),
            );
        }
        if self.gallery {
            content = content.push(card(column![label("UI GALLERY / sample state, not live data"), muted("Engine actions are disabled. Use navigation and F1 to inspect the interface; no sample account or task represents a real session."), self.gallery_links()].spacing(8)));
        }
        content = content.push(match self.page {
            Page::Home | Page::Library | Page::Discover if self.edition == Edition::Bedrock => {
                self.bedrock_view()
            }
            Page::Home => self.home(),
            Page::Library => self.library(),
            Page::Discover => self.discover(),
            Page::Settings => self.settings(),
            Page::About => self.about(),
        });
        if compact && self.preferences.show_activity {
            content = content.push(self.activity());
        }
        let mut shell = row![
            sidebar,
            container(
                scrollable(content.padding(if compact { 20 } else { 28 }))
                    .id(scrollable::Id::new("page")),
            )
            .width(Length::Fill)
            .height(Length::Fill)
        ];
        if !compact && self.preferences.show_activity {
            shell = shell.push(
                container(scrollable(self.activity()))
                    .padding(18)
                    .width(250)
                    .height(Length::Fill)
                    .style(design::chrome),
            );
        }
        let base: Element<'_, Message> = shell.into();
        if let Some(dialog) = &self.dialog {
            modal::modal(base, self.dialog_view(dialog), Message::CloseDialog)
        } else {
            base
        }
    }
}
impl App {
    fn with_worker(
        worker: mpsc::Sender<backend::Request>,
        events: mpsc::Receiver<backend::Event>,
    ) -> Self {
        Self {
            page: Page::Home,
            edition: Edition::Java,
            bedrock: bedrock::State::default(),
            instances: vec![],
            preferences: Preferences::default(),
            worker: Some(worker),
            events,
            dialog: None,
            name: String::new(),
            account_name: String::new(),
            game: "1.20.1".into(),
            loader: Loader::Vanilla,
            import_path: String::new(),
            selected: None,
            instance_tab: InstanceTab::Overview,
            inventory: vec![],
            inventory_loading: false,
            query: String::new(),
            filter_game: String::new(),
            kind: ProjectType::Mod,
            sort: Sort::Relevance,
            results: vec![],
            total: 0,
            offset: 0,
            search_id: 0,
            searching: false,
            search_dirty: false,
            search_due: std::time::Instant::now(),
            project: None,
            detail: None,
            detail_loading: false,
            settings_tab: SettingsTab::Appearance,
            memory: "4096".into(),
            window_width: String::new(),
            window_height: String::new(),
            instance_window_width: String::new(),
            instance_window_height: String::new(),
            instance_memory: String::new(),
            instance_java: String::new(),
            instance_args: String::new(),
            instance_env: String::new(),
            instance_hook_pre: String::new(),
            instance_hook_wrapper: String::new(),
            instance_hook_post: String::new(),
            java_add_path: String::new(),
            root: String::new(),
            width: 1280.0,
            pending: BTreeMap::new(),
            running: BTreeMap::new(),
            logs: BTreeMap::new(),
            notice: None,
            history: VecDeque::new(),
            saved_preferences: Preferences::default(),
            pending_preferences: None,
            pending_note: None,
            dialog_stack: Vec::new(),
            delete_pending: false,
            delete_outcome: None,
            library_query: String::new(),
            game_choices: Vec::new(),
            versions_loading: false,
            include_snapshots: false,
            versions_query: String::new(),
            release_filter: String::new(),
            visible_releases: 30,
            loader_versions: Vec::new(),
            loader_pin: String::new(),
            loader_loading: false,
            import_id: 0,
            import_review: None,
            import_loading: false,
            pack_loading: false,
            optional_files: Vec::new(),
            browser_path: std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .unwrap_or_else(|_| "/".into()),
            directory: Vec::new(),
            browse_loading: false,
            project_tab: ProjectTab::Description,
            releases: Vec::new(),
            releases_loading: false,
            release_id: None,
            content_review_id: 0,
            content_plan: None,
            content_review_loading: false,
            content_review_error: None,
            auth_error: false,
            authentication: model::Authentication::Offline,
            draft: surfaces::Draft::default(),
            local: local::State::default(),
            gallery: false,
        }
    }
    fn receive(&mut self, event: backend::Event) {
        match event {
            backend::Event::LocalReview(revision, result) => {
                if revision == self.local.revision && self.local.loading {
                    self.local.loading = false;
                    match result {
                        Ok(plan) if self.local_input().is_ok_and(|input| input == plan.input) => {
                            self.local.plan = Some(plan)
                        }
                        Ok(_) => self.local.error = Some("Selection changed. Review again.".into()),
                        Err(error) => self.local.error = Some(error),
                    }
                }
            }
            backend::Event::ContentReview(review_id, result) => {
                if review_id == self.content_review_id
                    && matches!(self.dialog, Some(Dialog::InstallReview(_)))
                {
                    self.content_review_loading = false;
                    match result {
                        Ok(plan) if self.content_plan_matches(&plan.instance_id, &plan) => {
                            self.content_plan = Some(plan)
                        }
                        Ok(_) => {
                            self.content_review_error =
                                Some("The selection changed. Resolve the files again.".into())
                        }
                        Err(error) => self.content_review_error = Some(error),
                    }
                }
            }
            backend::Event::BedrockInstalls(result) => {
                self.bedrock.loading = false;
                self.bedrock.checked = true;
                self.bedrock.installs.clear();
                self.bedrock.selected = None;
                self.bedrock.reading = false;
                self.bedrock.entries.clear();
                self.bedrock.worlds.clear();
                self.local.invalidate();
                match result {
                    Ok(installs) => {
                        self.bedrock.installs = installs;
                        self.bedrock.error = None;
                    }
                    Err(error) => self.bedrock.error = Some(error),
                }
            }
            backend::Event::BedrockInventory(channel, result) => {
                if self.bedrock.selected == Some(channel) {
                    self.bedrock.reading = false;
                    match result {
                        Ok(inventory) => {
                            self.bedrock.entries = inventory.entries;
                            self.bedrock.worlds = inventory.worlds;
                            self.bedrock.files_error = None;
                        }
                        Err(error) => {
                            self.bedrock.entries.clear();
                            self.bedrock.worlds.clear();
                            self.bedrock.files_error = Some(error);
                        }
                    }
                }
            }
            backend::Event::Loaded {
                instances,
                preferences,
                root,
            } => {
                self.instances = instances;
                self.memory = preferences.memory_mb.to_string();
                self.window_width = draft_side(preferences.window_width);
                self.window_height = draft_side(preferences.window_height);
                self.saved_preferences = preferences.clone();
                if preferences.remember_page {
                    self.page = match preferences.last_page.as_str() {
                        "library" => Page::Library,
                        "discover" => Page::Discover,
                        "about" => Page::About,
                        _ => Page::Home,
                    };
                }
                self.preferences = preferences;
                self.root = root;
            }
            backend::Event::GameVersions(result) => {
                self.versions_loading = false;
                match result {
                    Ok(items) => self.game_choices = items,
                    Err(error) => self.note(format!(
                        "Game versions unavailable: {error}. Enter a version manually or retry."
                    )),
                }
            }
            backend::Event::LoaderVersions(game, loader, result) => {
                if self.game == game && self.loader == loader {
                    self.loader_loading = false;
                    match result { Ok(items) => self.loader_versions = items, Err(error) => self.note(format!("Loader versions unavailable: {error}. Automatic selection remains available.")) }
                }
            }
            backend::Event::FetchedPack(revision, project_id, release_id, result) => {
                if revision == self.import_id && self.pack_loading {
                    self.pack_loading = false;
                    if self.edition == Edition::Java
                        && matches!(self.dialog, Some(Dialog::PackSetup))
                        && self
                            .project
                            .as_ref()
                            .is_some_and(|p| p.project_id == project_id)
                        && self
                            .selected_release()
                            .is_some_and(|r| r.id == release_id && r.project_id == project_id)
                    {
                        self.receive(backend::Event::ImportReview(revision, result));
                    }
                }
            }
            backend::Event::ImportReview(id, result) => {
                if id == self.import_id {
                    self.import_loading = false;
                    match result {
                        Ok(review) => {
                            if self.name.trim().is_empty() {
                                self.name = review.name.clone();
                            }
                            if !review.game.is_empty() {
                                self.game = review.game.clone();
                            }
                            self.loader = review.loader;
                            self.import_path = review.source.clone();
                            self.optional_files.clear();
                            self.import_review = Some(review);
                            self.dialog = Some(Dialog::ImportReview);
                        }
                        Err(error) => self.note(format!("Source review failed: {error}")),
                    }
                }
            }
            backend::Event::Directory(path, result) => {
                if path == self.browser_path {
                    self.browse_loading = false;
                    match result {
                        Ok(entries) => self.directory = entries,
                        Err(error) => self.note(error),
                    }
                }
            }
            backend::Event::Releases(id, result) => {
                if self.project.as_ref().is_some_and(|p| p.project_id == id) {
                    self.releases_loading = false;
                    match result {
                        Ok(releases) => {
                            self.release_id = releases.first().map(|r| r.id.clone());
                            self.releases = releases;
                        }
                        Err(error) => self.note(format!(
                            "Releases unavailable: {error}. Retry project information."
                        )),
                    }
                }
            }
            backend::Event::Instances(items) => {
                self.instances = items;
                if self
                    .selected
                    .as_ref()
                    .is_some_and(|id| !self.instances.iter().any(|i| &i.id == id))
                {
                    self.selected = None;
                }
            }
            backend::Event::Inventory(id, result) => {
                if self.selected.as_ref() == Some(&id) {
                    self.inventory_loading = false;
                    match result {
                        Ok(items) => self.inventory = items,
                        Err(error) => self.notice = Some(error),
                    }
                }
            }
            backend::Event::JavaProbed(path, result) => {
                // A probe's answer belongs to the dialog that asked for it;
                // one that arrives after closing is not applied. The add
                // dialog is opened from the roster in Preferences, so its
                // receipt lands back there -- completing an add must not end
                // the Preferences visit (found by the UI smoke, not a test).
                if matches!(self.dialog, Some(Dialog::JavaAdd)) {
                    self.dialog = self.dialog_stack.pop().or(Some(Dialog::Preferences));
                    self.settings_tab = SettingsTab::Java;
                    match result {
                        Ok(label) => {
                            let label = if label.is_empty() {
                                path.clone()
                            } else {
                                label
                            };
                            self.preferences
                                .java_installs
                                .push(model::JavaInstall { label, path });
                            self.save_preferences_now(
                                "Java runtime added to the list.".to_string(),
                            );
                        }
                        Err(error) => self.note(format!("Action could not finish: {error}")),
                    }
                }
            }
            backend::Event::Status(id, status) => {
                self.pending.insert(id, status);
            }
            backend::Event::Finished(id, result) => {
                self.pending.remove(&id);
                match result {
                    Ok(note) => {
                        if id == "bedrock-local" {
                            if let Some(channel) = self.bedrock.selected {
                                self.bedrock.reading = true;
                                self.send(backend::Request::BedrockInventory(channel));
                            }
                        }
                        // A deletion's receipt renders inside its own open
                        // dialog (the reviewed-operation pattern), and the
                        // collection's menu visit is done with it.
                        if self.delete_pending
                            && matches!(&self.dialog, Some(Dialog::Delete(target)) if *target == id)
                        {
                            self.delete_pending = false;
                            self.delete_outcome = Some(Ok(note.clone()));
                            self.dialog_stack.clear();
                        }
                        if id == "preferences" {
                            if let Some(saved) = self.pending_preferences.take() {
                                self.saved_preferences = saved;
                            }
                            // An immediate save keeps its own receipt on
                            // screen; the generic confirmation is only the
                            // fallback for a plain dialog save.
                            let receipt = self.pending_note.take().unwrap_or(note);
                            self.note(receipt);
                        } else {
                            self.note(note);
                        }
                        let completed_dialog = match &self.dialog {
                            Some(Dialog::Create) => id == "create",
                            Some(Dialog::Import | Dialog::ImportReview) => id == "import",
                            Some(Dialog::Rename(instance)) => id == format!("rename:{instance}"),
                            Some(Dialog::Duplicate(instance)) => {
                                id == format!("duplicate:{instance}")
                            }
                            Some(Dialog::InstanceWindow(instance)) => {
                                id == format!("window:{instance}")
                            }
                            Some(Dialog::InstanceRuntime(instance)) => {
                                id == format!("runtime:{instance}")
                            }
                            _ => false,
                        };
                        if completed_dialog {
                            self.dialog = self.dialog_stack.pop();
                        }
                    }
                    Err(error) => {
                        if id == "preferences" {
                            self.pending_preferences = None;
                            self.pending_note = None;
                        }
                        if self.delete_pending
                            && matches!(&self.dialog, Some(Dialog::Delete(target)) if *target == id)
                        {
                            self.delete_pending = false;
                            self.delete_outcome = Some(Err(error.clone()));
                        }
                        self.note(format!("Action could not finish: {error}"));
                    }
                }
            }
            backend::Event::LocalApplied(id, result) => {
                self.pending.remove(&id);
                if id == "bedrock-local" {
                    if let Some(channel) = self.bedrock.selected {
                        self.bedrock.reading = true;
                        self.send(backend::Request::BedrockInventory(channel));
                    }
                }
                match result {
                    Ok(note) => {
                        self.local.outcome = Some(Ok(note.clone()));
                        self.note(note);
                    }
                    Err(error) => {
                        self.local.outcome = Some(Err(error.clone()));
                        self.note(format!("Action could not finish: {error}"));
                    }
                }
            }
            backend::Event::Running(id, name) => {
                self.pending.remove(&id);
                self.running.insert(id, name);
            }
            backend::Event::Exited(id, status) => {
                self.running.remove(&id);
                self.note(status);
            }
            backend::Event::Log(id, line) => {
                let lines = self.logs.entry(id).or_default();
                lines.push_back(line.chars().take(8192).collect());
                if lines.len() > 800 {
                    lines.pop_front();
                }
            }
            backend::Event::Search(id, result) => {
                if id == self.search_id {
                    self.searching = false;
                    match result {
                        Ok((hits, total)) => {
                            self.results = hits;
                            self.total = total;
                        }
                        Err(error) => {
                            self.results.clear();
                            self.total = 0;
                            self.note(format!("Explore could not load: {error}. Try again."));
                        }
                    }
                }
            }
            backend::Event::Project(id, result) => {
                if self.project.as_ref().is_some_and(|p| p.project_id == id) {
                    self.detail_loading = false;
                    match result {
                        Ok(detail) => self.detail = Some(detail),
                        Err(error) => {
                            self.note(format!("Project information unavailable: {error}"))
                        }
                    }
                }
            }
            backend::Event::Authentication(state) => {
                if let model::Authentication::Failed(error) = &state {
                    self.auth_error = true;
                    self.note(error.clone());
                }
                self.authentication = state;
            }
            backend::Event::Error(error) => self.note(error),
        }
    }
    fn local_input(&self) -> model::Result<local::Input> {
        let panel = match &self.dialog {
            Some(Dialog::Surface(panel)) => panel,
            _ => return Err("Open a world or local-content review first.".into()),
        };
        let target = |id: &str| -> model::Result<local::Target> {
            if id.is_empty() && self.edition == Edition::Bedrock {
                self.bedrock
                    .selected
                    .map(local::Target::Bedrock)
                    .ok_or_else(|| "Inspect an installed Bedrock copy first.".into())
            } else if self.edition == Edition::Java && self.instances.iter().any(|i| i.id == id) {
                Ok(local::Target::Java(id.into()))
            } else {
                Err("Select an existing collection in the correct edition.".into())
            }
        };
        match panel {
            surfaces::Panel::WorldReview(id, folder) => Ok(local::Input::World {
                target: target(id)?,
                folder: folder.clone(),
                output: self.draft.output.clone(),
            }),
            surfaces::Panel::AdditionReview(id) => Ok(local::Input::Addition {
                target: target(id)?,
                kind: self.draft.addition_kind,
                source: self.draft.source.clone(),
                world: self.local.activation_world.clone(),
            }),
            _ => Err("Open the final review first.".into()),
        }
    }
    fn invalidate_content_review(&mut self) {
        self.content_review_id += 1;
        self.content_plan = None;
        self.content_review_loading = false;
        self.content_review_error = None;
    }
    fn content_plan_matches(&self, id: &str, plan: &model::ContentPlan) -> bool {
        plan.instance_id == id
            && !plan.files.is_empty()
            && self
                .project
                .as_ref()
                .is_some_and(|project| project.project_id == plan.project_id)
            && self.selected_release().is_some_and(|release| {
                release.id == plan.release_id && release.project_id == plan.project_id
            })
            && self
                .instances
                .iter()
                .find(|item| item.id == id)
                .is_some_and(|item| {
                    item.game_version == plan.game_version
                        && item.loader == plan.loader
                        && item.name == plan.instance_name
                })
    }
    fn note(&mut self, note: String) {
        self.notice = Some(note.clone());
        // The same sentence twice in a row is one event said twice: an
        // immediate save notes its receipt on the spot and the write's
        // confirmation says it again, and Recent notes is a history of
        // events, not of echoes.
        if self.history.front() != Some(&note) {
            self.history.push_front(note);
        }
        if self.history.len() > 8 {
            self.history.pop_back();
        }
    }
    /// Write the settings now, keeping both the receipt and the saved
    /// baseline honest. `receipt` is what stays on screen when the write
    /// lands -- an action's own confirmation must not be buried by the
    /// generic save confirmation -- and the exact snapshot sent becomes
    /// `saved_preferences` when the write lands (a failed write stays
    /// visible as unsaved). Roster and account changes save immediately, so
    /// closing Preferences must not roll them back to an older baseline.
    fn save_preferences_now(&mut self, receipt: String) {
        self.note(receipt.clone());
        self.pending_note = Some(receipt);
        self.pending_preferences = Some(self.preferences.clone());
        self.send(backend::Request::Save(self.preferences.clone()));
    }
    /// What a revert rolls back to: the snapshot on its way to disk, or the
    /// last one written -- never an older baseline that would discard an
    /// immediate save before its receipt lands.
    fn settings_baseline(&self) -> Preferences {
        self.pending_preferences
            .clone()
            .unwrap_or_else(|| self.saved_preferences.clone())
    }
    fn send(&mut self, request: backend::Request) {
        if self.gallery {
            gallery::preview_request(self, &request);
            self.note("UI GALLERY / sample state only · engine actions are disabled".into());
            return;
        }
        if self
            .worker
            .as_ref()
            .is_none_or(|worker| worker.send(request).is_err())
        {
            self.local.invalidate();
            self.local.error = Some("The background service stopped. Restart the launcher.".into());
            self.content_review_loading = false;
            self.content_plan = None;
            self.content_review_error =
                Some("The background service stopped. Restart the launcher.".into());
            self.searching = false;
            self.inventory_loading = false;
            self.versions_loading = false;
            self.loader_loading = false;
            self.import_loading = false;
            self.pack_loading = false;
            self.browse_loading = false;
            self.releases_loading = false;
            self.detail_loading = false;
            self.pending_preferences = None;
            self.pending_note = None;
            self.pending.clear();
            self.worker = None;
            self.note("The background service stopped. Restart the launcher.".into());
        }
    }
    /// Live browsing: the query changed; let typing settle before searching.
    fn schedule_search(&mut self) {
        self.search_dirty = true;
        self.search_due = std::time::Instant::now() + Duration::from_millis(350);
    }
    fn search(&mut self) {
        self.search_id += 1;
        self.searching = true;
        self.send(backend::Request::Search(
            self.search_id,
            self.query.clone(),
            self.kind,
            self.sort,
            self.offset,
            self.filter_game.clone(),
        ));
    }
    fn activity(&self) -> Element<'_, Message> {
        let mut panel = column![
            subtitle("On your desk"),
            muted(format!(
                "{} collections · {} running",
                self.instances.len(),
                self.running.len()
            ))
            .size(12)
        ]
        .spacing(14);
        if self.pending.is_empty() && self.running.is_empty() {
            panel = panel.push(muted(
                "Nothing in flight. Your next journey can wait for you.",
            ));
        }
        for (id, status) in &self.pending {
            let name = self
                .instances
                .iter()
                .find(|i| &i.id == id)
                .map(|i| i.name.as_str())
                .unwrap_or(match id.as_str() {
                    "create" => "New collection",
                    "import" => "Import",
                    _ => "Background task",
                });
            panel = panel.push(
                container(
                    column![
                        label(name),
                        muted(status).size(12),
                        action(
                            "Task details",
                            Message::Open(Dialog::Surface(surfaces::Panel::Task(id.clone()))),
                            Button::Quiet
                        )
                    ]
                    .spacing(6),
                )
                .padding(12)
                .style(design::well),
            );
        }
        for (id, name) in &self.running {
            panel = panel.push(
                column![
                    label(name),
                    row![
                        action("Inspect", Message::Select(id.clone()), Button::Quiet),
                        action("Stop", Message::Stop(id.clone()), Button::Danger)
                    ]
                    .spacing(6)
                ]
                .spacing(8),
            );
        }
        panel = panel.push(
            row![
                subtitle("Recent notes"),
                action("Clear", Message::ClearHistory, Button::Quiet)
            ]
            .spacing(8),
        );
        for note in &self.history {
            panel = panel.push(muted(note).size(12));
        }
        panel.into()
    }
    fn home(&self) -> Element<'_, Message> {
        let hero = card(
            column![
                design::landscape(),
                title("Somewhere you haven't been."),
                muted("A collection is a small promise: room for the world you want to make."),
                self.actions(vec![
                    action(
                        "Start a collection",
                        Message::Open(Dialog::Create),
                        Button::Primary
                    ),
                    action(
                        "Explore additions",
                        Message::Navigate(Page::Discover),
                        Button::Quiet
                    )
                ])
            ]
            .spacing(16),
        );
        let mut page = column![
            hero,
            row![
                subtitle("Pick up a thread"),
                Space::with_width(Length::Fill),
                action(
                    "All collections",
                    Message::Navigate(Page::Library),
                    Button::Quiet
                )
            ]
            .align_items(iced::Alignment::Center)
        ]
        .spacing(20);
        if self.instances.is_empty() {
            page=page.push(self.empty("An open shelf", "Create a collection or bring one from another launcher. Each keeps its own worlds and content."));
        }
        for item in self.instances.iter().take(3) {
            page = page.push(self.instance_card(item));
        }
        page.into()
    }
    fn empty(&self, heading: &str, detail: &str) -> Element<'_, Message> {
        card(
            column![
                design::collection(),
                subtitle(heading),
                muted(detail),
                row![
                    action("Create", Message::Open(Dialog::Create), Button::Primary),
                    action("Import", Message::Open(Dialog::Import), Button::Quiet)
                ]
                .spacing(10)
            ]
            .spacing(14)
            .align_items(iced::Alignment::Center),
        )
    }
    fn instance_card(&self, item: &Instance) -> Element<'_, Message> {
        let status = self.pending.get(&item.id).map(String::as_str).unwrap_or(
            if self.running.contains_key(&item.id) {
                "In session"
            } else if item.installed_version.is_some() {
                "Installed"
            } else {
                "Installs on first play"
            },
        );
        design::project_card(
            design::icon_tile(design::mark(28.0)),
            &item.name,
            format!("{} / Minecraft {}", item.loader, item.game_version),
            vec![status.to_string()],
            // The narrow layout stacks the cluster so the name column keeps
            // its width; a single row of pills squeezes long names to five
            // lines at the minimum window size.
            self.actions(vec![
                action("Inspect", Message::Select(item.id.clone()), Button::Quiet),
                self.play_button(item),
                action(
                    "Actions",
                    Message::Open(Dialog::InstanceActions(item.id.clone())),
                    Button::Quiet,
                ),
            ]),
        )
    }
    fn play_button(&self, item: &Instance) -> Element<'_, Message> {
        if self.pending.contains_key(&item.id) {
            button(label("Working…"))
                .padding([10, 16])
                .style(Button::Quiet)
                .into()
        } else if self.running.contains_key(&item.id) {
            action(
                "Stop session",
                Message::Stop(item.id.clone()),
                Button::Danger,
            )
        } else {
            action(
                "Play offline",
                Message::Open(Dialog::Surface(surfaces::Panel::Launch(item.id.clone()))),
                Button::Primary,
            )
        }
    }
    fn library(&self) -> Element<'_, Message> {
        if let Some(item) = self
            .selected
            .as_ref()
            .and_then(|id| self.instances.iter().find(|i| &i.id == id))
        {
            return self.instance_detail(item);
        }
        let mut page = column![
            title("A shelf of possibilities."),
            self.actions(vec![
                action(
                    "New collection",
                    Message::Open(Dialog::Create),
                    Button::Primary
                ),
                action(
                    "Bring a collection",
                    Message::Open(Dialog::Import),
                    Button::Quiet
                )
            ])
        ]
        .spacing(18);
        if self.instances.is_empty() {
            page = page.push(self.empty(
                "Nothing here yet",
                "Choose a game and loader, or inspect an existing collection to bring it here.",
            ));
        }
        page = page.push(
            input(
                "Filter collections by name, game, or loader",
                &self.library_query,
            )
            .on_input(Message::LibraryQuery)
            .padding(12),
        );
        let query = self.library_query.to_lowercase();
        let mut matched = 0;
        for item in &self.instances {
            if !format!("{} {} {}", item.name, item.game_version, item.loader)
                .to_lowercase()
                .contains(&query)
            {
                continue;
            }
            matched += 1;
            page = page.push(self.instance_card(item));
        }
        if matched == 0 && !self.instances.is_empty() {
            page = page.push(muted(
                "No collections match. Clear the filter to see your shelf.",
            ));
        }
        page.into()
    }
    fn instance_detail(&self, item: &Instance) -> Element<'_, Message> {
        let visible = InstanceTab::visible(&self.preferences.features);
        if visible.is_empty() {
            // Every tab is hidden by Features: the tabs themselves must not
            // appear anywhere, and the page says where the switches live.
            return column![
                action(
                    "← Collections",
                    Message::Navigate(Page::Library),
                    Button::Quiet
                ),
                title(&item.name),
                card(
                    column![
                        subtitle("Every collection tab is hidden"),
                        muted("The Features section in Preferences hides this page's tabs. Turn one back on to see this collection's content."),
                        action("Open Preferences", Message::Navigate(Page::Settings), Button::Primary),
                    ]
                    .spacing(12)
                ),
            ]
            .spacing(18)
            .into();
        }
        let mut tabs = row![].spacing(6);
        for tab in visible.iter().copied() {
            tabs = tabs.push(action(
                tab.name(),
                Message::InstanceTab(tab),
                if tab == self.instance_tab {
                    Button::Selected
                } else {
                    Button::Quiet
                },
            ));
        }
        let mut page = column![
            action(
                "← Collections",
                Message::Navigate(Page::Library),
                Button::Quiet
            ),
            row![
                title(&item.name).width(Length::Fill),
                self.actions(vec![
                    action(
                        "Actions",
                        Message::Open(Dialog::InstanceActions(item.id.clone())),
                        Button::Quiet,
                    ),
                    self.play_button(item)
                ])
            ]
            .spacing(12),
            muted(format!("Minecraft {} · {}", item.game_version, item.loader)),
            if self.narrow() {
                controls::picker(
                    visible.iter().copied(),
                    Some(self.instance_tab),
                    Message::InstanceTab,
                )
            } else {
                tabs.into()
            }
        ]
        .spacing(18);
        let body:Element<'_,Message>=match self.instance_tab{
            InstanceTab::Overview=>column![card(column![subtitle("The essentials"),label(format!("Game version: {}",item.game_version)),label(format!("Loader: {}",item.loader)),label(format!("Installation: {}",item.installed_version.as_deref().unwrap_or("Not installed yet"))),muted("First play downloads the game, libraries, and runtime. Shared files are reused across collections.")].spacing(12)),card(column![subtitle("Your game directory"),muted(format!("{}/instances/{}/game",self.root,item.id)),action("Open game folder",Message::OpenFolder(Some(item.id.clone()),String::new()),Button::Quiet)].spacing(12))].spacing(16).into(),
            InstanceTab::Content=>{
                let mut body=column![subtitle("Additions"),row![action("Find content",Message::Navigate(Page::Discover),Button::Primary),action("Refresh",Message::RefreshInventory,Button::Quiet)].spacing(8),row![action("Mods folder",Message::OpenFolder(Some(item.id.clone()),"mods".into()),Button::Quiet),action("Resource packs",Message::OpenFolder(Some(item.id.clone()),"resourcepacks".into()),Button::Quiet)].spacing(8),action("Shaders folder",Message::OpenFolder(Some(item.id.clone()),"shaderpacks".into()),Button::Quiet),action("Import local content…", Message::Open(Dialog::Surface(surfaces::Panel::Addition(item.id.clone()))), Button::Quiet),muted("A .disabled suffix keeps a mod on disk without loading it.")].spacing(14);
                if self.inventory_loading{body=body.push(muted("Reading this collection…"));}
                let entries:Vec<_>=self.inventory.iter().filter(|e|["mods","resourcepacks","shaderpacks"].contains(&e.folder.as_str())).collect();
                if entries.is_empty()&&!self.inventory_loading{body=body.push(card(column![design::collection(),subtitle("Room for additions"),muted("Browse Explore, or place your own files in the game folders.")].spacing(12)));}
                for entry in entries{let mut line=row![column![label(&entry.name),muted(format!("{} · {}",entry.folder,byte_size(entry.bytes))).size(12)].spacing(4).width(Length::Fill)];if entry.folder=="mods"&&!entry.directory&&!self.running.contains_key(&item.id)&&!self.pending.contains_key(&item.id){line=line.push(action(if entry.name.ends_with(".disabled"){"Enable"}else{"Disable"},Message::ToggleMod(item.id.clone(),entry.name.clone()),Button::Quiet));}
                if !entry.directory&&!self.running.contains_key(&item.id)&&!self.pending.contains_key(&item.id){line=line.push(action("Remove…",Message::Open(Dialog::RemoveContent(item.id.clone(),entry.folder.clone(),entry.name.clone())),Button::Danger));}body=body.push(card(line.spacing(10)));}body.into()
            },
            InstanceTab::Worlds=>{let mut body=column![design::emblem("worlds"),self.actions(vec![action("Open saves",Message::OpenFolder(Some(item.id.clone()),"saves".into()),Button::Quiet),action("Refresh worlds",Message::RefreshInventory,Button::Quiet)]),muted("Back up a world before changing game versions or content. The launcher never edits world data.")].spacing(14);let mut found=false;for entry in self.inventory.iter().filter(|e|e.folder=="saves"&&e.directory){found=true;body=body.push(card(column![row![design::icon("worlds",false),label(&entry.name)].spacing(12),action("Backup / export…",Message::Open(Dialog::Surface(surfaces::Panel::World(item.id.clone(),entry.name.clone()))),Button::Quiet)].spacing(10)));}if !found{body=body.push(muted(if self.inventory_loading { "Reading saved worlds…" } else { "No saved worlds in this collection yet. New worlds appear after you save them in-game." }));}body.into()},
            InstanceTab::Logs=>{let output=self.logs.get(&item.id).map(|lines|lines.iter().cloned().collect::<Vec<_>>().join("\n")).unwrap_or_else(||"No session output yet. Start the game to watch its output here.".into());column![row![subtitle("Session journal").width(Length::Fill),action("Copy",Message::CopyLogs,Button::Quiet),action("Clear view",Message::ClearLogs,Button::Quiet)].spacing(8),container(scrollable(label(output).font(iced::Font::MONOSPACE).size(12)).height(330)).padding(16).style(design::well),muted("The last 800 lines are held in the view. Minecraft's saved logs remain in the game folder."),action("Open saved logs",Message::OpenFolder(Some(item.id.clone()),"logs".into()),Button::Quiet)].spacing(14).into()},
            InstanceTab::Manage=>card(column![subtitle("Collection details"),action("Rename collection",Message::Open(Dialog::Rename(item.id.clone())),Button::Quiet),action("Duplicate collection…",Message::DuplicateOpen(item.id.clone()),Button::Quiet),muted("Game version and loader are immutable for an existing collection. Create a separate collection to protect its worlds."),subtitle("Game window"),if let Some(window)=item.window{muted(format!("This collection launches at {}×{}. The launcher preference is ignored.",window.width,window.height))}else{muted("Follows the launcher window size in Preferences.")},self.actions({let mut operations=vec![action("Set a window size…",Message::InstanceWindowOpen(item.id.clone()),Button::Quiet)];if item.window.is_some(){operations.push(action("Use launcher default",Message::InstanceWindowClear(item.id.clone()),Button::Quiet));}operations}),subtitle("Launch settings"),if item.memory_mb.is_none()&&item.java.is_none()&&item.custom.is_empty(){muted("Follows the launcher's memory budget and runtime from Preferences.")}else{muted(format!("This collection overrides: {}.",{let mut parts=vec![];if let Some(memory_mb)=item.memory_mb{parts.push(format!("{memory_mb} MiB memory"));}
if let Some(java)=&item.java{parts.push(format!("runtime {java}"));}
if !item.custom.extra_args.is_empty(){parts.push(format!("{} extra argument(s)",item.custom.extra_args.len()));}
if !item.custom.env.is_empty(){parts.push(format!("{} environment entry(s)",item.custom.env.len()));}
if !item.custom.is_empty(){let hooks=[item.custom.hooks.pre_launch.as_ref().map(|_|"pre-launch"),item.custom.hooks.wrapper.as_ref().map(|_|"wrapper"),item.custom.hooks.post_exit.as_ref().map(|_|"post-exit")];let named:Vec<&str>=hooks.into_iter().flatten().collect();if !named.is_empty(){parts.push(format!("{} hook",named.join("/")));}}parts.join(" · ")}))},self.actions({let mut operations=vec![action("Set memory & runtime…",Message::InstanceRuntimeOpen(item.id.clone()),Button::Quiet)];if item.memory_mb.is_some()||item.java.is_some()||!item.custom.is_empty(){operations.push(action("Use launcher defaults",Message::InstanceRuntimeClear(item.id.clone()),Button::Quiet));}operations}),subtitle("Danger zone"),muted("Deleting removes this collection's saves and additions. It does not delete shared downloads."),if self.running.contains_key(&item.id)||self.pending.contains_key(&item.id){button(label("Stop current work before deleting")).padding(12).style(Button::Quiet).into()}else{action("Delete collection…",Message::Open(Dialog::Delete(item.id.clone())),Button::Danger)}].spacing(14)),
        };
        page = page.push(body);
        page.into()
    }
    fn discover(&self) -> Element<'_, Message> {
        if let Some(project) = &self.project {
            return self.project_view(project);
        }
        let filters = column![
            row![
                input("Name, idea, or something unexpected…", &self.query)
                    .id(text_input::Id::new("search"))
                    .on_input(Message::Query)
                    .on_submit(Message::Search)
                    .padding(12),
                action("Find additions", Message::Search, Button::Primary)
            ]
            .spacing(10),
            self.actions(
                ProjectType::ALL
                    .iter()
                    .map(|kind| {
                        controls::chip(kind.to_string(), *kind == self.kind, Message::Kind(*kind))
                    })
                    .collect(),
            ),
            self.actions(vec![
                controls::picker(Sort::ALL, Some(self.sort), Message::Sort),
                input("Minecraft version (optional)", &self.filter_game)
                    .on_input(Message::FilterGame)
                    .on_submit(Message::Search)
                    .padding(10)
                    .into(),
                action("Reset", Message::ResetFilters, Button::Quiet)
            ])
        ]
        .spacing(12);
        let mut page = column![
            title("Follow your curiosity."),
            card(filters),
            muted(if self.searching {
                "Looking through the catalogue…".into()
            } else {
                format!("{} results · public content catalogue", self.total)
            })
        ]
        .spacing(18);
        if self.results.is_empty() {
            page = page.push(card(
                column![
                    design::collection(),
                    subtitle(if self.searching {
                        "Looking for a thread"
                    } else {
                        "No matching additions"
                    }),
                    muted(if self.searching {
                        "Live results are on their way. You can keep navigating."
                    } else {
                        "Try a shorter search or remove the version filter."
                    })
                ]
                .spacing(12),
            ));
        }
        for project in &self.results {
            page = page.push(design::project_card(
                design::icon_tile(design::icon("discover", false)),
                &project.title,
                &project.description,
                vec![
                    project.project_type.to_string(),
                    format!("by {}", project.author),
                    format!("{} downloads", project.downloads),
                ],
                action(
                    "Read more",
                    Message::Project(project.clone()),
                    Button::Quiet,
                ),
            ));
        }
        let mut pagination = row![
            muted(format!(
                "Showing {}–{}",
                if self.results.is_empty() {
                    0
                } else {
                    self.offset + 1
                },
                self.offset + self.results.len()
            ))
            .width(Length::Fill)
        ]
        .spacing(8);
        if self.offset > 0 && !self.searching {
            pagination = pagination.push(action("Previous", Message::Previous, Button::Quiet));
        }
        if self.offset + 12 < self.total && !self.searching {
            pagination = pagination.push(action("Next", Message::Next, Button::Quiet));
        }
        page.push(pagination).into()
    }
    fn project_view(&self, project: &Project) -> Element<'_, Message> {
        let mut page = column![
            action("← Explore", Message::CloseProject, Button::Quiet),
            title(&project.title),
            muted(format!("by {} / {}", project.author, project.project_type)),
            card(
                column![
                    label(&project.description),
                    row![
                        label(format!("{} downloads", project.downloads)).width(Length::Fill),
                        action(
                            "Add to a collection…",
                            Message::Open(Dialog::Install),
                            Button::Primary
                        )
                    ]
                    .spacing(10)
                ]
                .spacing(18)
            )
        ]
        .spacing(18);
        if self.detail_loading {
            page = page.push(muted("Loading the author's project notes…"));
        } else if self.detail.is_none() {
            page = page
                .push(action(
                    "Retry project information",
                    Message::RetryProject,
                    Button::Quiet,
                ))
                .push(self.project_tabs());
        }
        if let Some(detail) = &self.detail {
            page = page.push(card(
                column![
                    subtitle(&detail.title),
                    muted(&detail.description),
                    muted(format!(
                        "{} downloads · {}",
                        detail.downloads,
                        detail.categories.join(" / ")
                    )),
                    label(format!(
                        "Licence: {} · {} game versions · Loaders: {}",
                        detail.license.id,
                        detail.game_versions.len(),
                        detail.loaders.join(", ")
                    )),
                    self.project_links(project, detail),
                    self.project_tabs()
                ]
                .spacing(14),
            ));
        }
        page.push(muted("Project descriptions belong to their authors and are retrieved as live catalogue content. They are not launcher assets or interface copy.").size(11)).into()
    }
    fn settings(&self) -> Element<'_, Message> {
        let mut tabs = column![].spacing(6);
        for tab in SettingsTab::ALL {
            tabs = tabs.push(action(
                tab.name(),
                Message::SettingsTab(tab),
                if self.settings_tab == tab {
                    Button::Selected
                } else {
                    Button::Quiet
                },
            ));
        }
        let pane:Element<'_,Message>=match self.settings_tab{
            SettingsTab::Appearance=>card(column![subtitle("A tone of your own"),muted("Choose an accent. Changes appear immediately; save to keep them."),label("Accent"),controls::picker(Accent::ALL,Some(self.preferences.accent),Message::Accent),label("Interface density"),controls::picker(Scale::ALL,Some(self.preferences.scale),Message::Scale),controls::toggle("Show the activity notebook",self.preferences.show_activity,Message::Activity),muted("Transitions are intentionally instantaneous in this build. No motion setting is needed to turn them off.")].spacing(16)),
            SettingsTab::Language=>card(column![subtitle("Words that belong here"),label("English"),muted("English is the only authored language catalogue. We do not ship another launcher's translations."),muted("Additional language catalogues need original translations and complete coverage before they can be selected."),muted("System fonts and advanced shaping are used for non-Latin project names.")].spacing(16)),
            SettingsTab::Java=>{
                let mut pane=column![subtitle("Runtime & player"),label("Offline player name (fallback)"),input("Player",&self.preferences.player_name).on_input(Message::Player).padding(12),muted("The offline account list under Account status is the primary way to change identity. This name is used only while no account is selected."),label("Java executable"),input("Leave blank for automatic management",&self.preferences.java).on_input(Message::Java).padding(12),muted("Automatic management uses the runtime named by the game's metadata. A custom path is executed directly, never as a shell command."),subtitle("Java installations"),muted("Runtimes you have added. Choose one to use it for launches; removing it from the list never deletes files on disk.")].spacing(14);
                if self.preferences.java_installs.is_empty(){pane=pane.push(muted("No runtimes added yet. Automatic management covers every collection without an override."));}
                for install in &self.preferences.java_installs{
                    let active=self.preferences.java==install.path;
                    pane=pane.push(card(column![label(&install.label),muted(&install.path).size(12),self.actions({let mut operations:Vec<Element<'_,Message>>=vec![if active{surfaces::disabled("In use")}else{action("Use for launches",Message::JavaUse(install.path.clone()),Button::Quiet)}];operations.push(action("Remove from list",Message::JavaRemove(install.path.clone()),Button::Quiet));operations})].spacing(10)));
                }
                pane=pane.push(action("Add a Java executable…",Message::JavaAddOpen,Button::Quiet));
                pane=pane.push(action("Account status",Message::Open(Dialog::Account),Button::Quiet));
                card(pane)
            },
            SettingsTab::Window=>card(column![subtitle("Where the game shows up"),label("Game window width (pixels)"),input("Unset – the game chooses",&self.window_width).on_input(Message::WindowWidth).padding(12),label("Game window height (pixels)"),input("Unset – the game chooses",&self.window_height).on_input(Message::WindowHeight).padding(12),muted("Leave both blank to let the game open at its own default size. Set both sides or neither; 320–16384 pixels per side."),muted("A collection with its own window size in Manage overrides this. The size applies at the next launch.")].spacing(14)),
            SettingsTab::Resources=>card(column![subtitle("Give the game room"),label("Maximum Java memory (MiB)"),input("4096",&self.memory).on_input(Message::Memory).padding(12),muted("Allowed: 512–32768 MiB. Leave room for the operating system."),label("Shared storage"),muted(&self.root),action("Open data directory",Message::OpenFolder(None,String::new()),Button::Quiet),muted("Downloads share one content store. Deleting a collection does not remove files used by another collection.")].spacing(16)),
            SettingsTab::Privacy=>card(column![subtitle("A small network footprint"),label("No telemetry or background analytics"),muted("Explore talks to the public content API. Installation talks to the game/loader metadata and download services. Their operators can see ordinary request metadata such as your IP address."),muted("Project descriptions are displayed as text; no remote script or embedded browser runs them."),muted("Your preferences and collection descriptors stay in the data directory. This build does not store account access tokens.")].spacing(16)),
            SettingsTab::Behaviour=>card(column![controls::toggle("Remember the last launcher page",self.preferences.remember_page,Message::RememberPage),subtitle("Before anything disappears"),label("Deletion always requires explicit confirmation"),muted("A collection includes saved worlds. That is why the safety dialog cannot be disabled."),controls::toggle("Show activity alongside large windows",self.preferences.show_activity,Message::Activity),muted("Closing the launcher stops games it supervises. Pending installations do not start a game after the window closes."),subtitle("Keyboard guide"),label("Tab / Shift+Tab: move focus\nCtrl+1 / 2 / 3: Home / Collections / Explore\nCtrl+,: Preferences\nEscape: close a dialog or return")].spacing(16)),
            SettingsTab::Features=>card(column![subtitle("Which collection tabs you see"),muted("Turning a tab off removes it from every collection page, including the tab picker at small window sizes. Changes apply immediately; save to keep them."),controls::toggle("Overview tab",self.preferences.features.show_overview,Message::FeatureOverview),controls::toggle("Content tab",self.preferences.features.show_content,Message::FeatureContent),controls::toggle("Worlds tab",self.preferences.features.show_worlds,Message::FeatureWorlds),controls::toggle("Logs tab",self.preferences.features.show_logs,Message::FeatureLogs),controls::toggle("Manage tab",self.preferences.features.show_manage,Message::FeatureManage),muted("Tabs for files and screenshots join this list when those surfaces exist — no switch here promises a tab that is not built.")].spacing(16)),
        };
        let compact = self.width / self.preferences.scale.factor() < 1000.0;
        let layout: Element<'_, Message> = if compact {
            let mut compact_tabs = column![].spacing(6);
            for pair in SettingsTab::ALL.chunks(2) {
                compact_tabs = compact_tabs.push(
                    row(pair.iter().map(|tab| {
                        action(
                            tab.name(),
                            Message::SettingsTab(*tab),
                            if *tab == self.settings_tab {
                                Button::Selected
                            } else {
                                Button::Quiet
                            },
                        )
                    }))
                    .spacing(6),
                );
            }
            column![compact_tabs, pane].spacing(14).into()
        } else {
            row![
                container(tabs).width(150),
                container(pane).width(Length::Fill)
            ]
            .spacing(16)
            .into()
        };
        column![
            subtitle("Settle in."),
            layout,
            muted(
                if self.preferences == self.saved_preferences
                    && self.memory == self.saved_preferences.memory_mb.to_string()
                    && model::parse_window_size(&self.window_width, &self.window_height).ok()
                        == Some((
                            self.saved_preferences.window_width,
                            self.saved_preferences.window_height,
                        ))
                {
                    "Saved preferences"
                } else {
                    "Unsaved changes · save or revert"
                }
            ),
            row![
                action("Revert", Message::RevertSettings, Button::Quiet),
                action("Reset defaults", Message::ResetSettings, Button::Quiet)
            ]
            .spacing(8),
            action("Save preferences", Message::SaveSettings, Button::Primary)
        ]
        .spacing(14)
        .into()
    }
    /// A hidden tab is never the selected one: fall back to the first tab the
    /// Features settings still show.
    fn normalize_instance_tab(&mut self) {
        if !self.instance_tab.shown(&self.preferences.features) {
            if let Some(tab) = InstanceTab::visible(&self.preferences.features).first() {
                self.instance_tab = *tab;
            }
        }
    }
    /// Mirror the stored values into the settings drafts, so the fields show
    /// exactly what a save would write.
    fn sync_settings_drafts(&mut self) {
        self.memory = self.preferences.memory_mb.to_string();
        self.window_width = draft_side(self.preferences.window_width);
        self.window_height = draft_side(self.preferences.window_height);
    }
    fn about(&self) -> Element<'_, Message> {
        column![action("Notices & diagnostics",Message::Open(Dialog::Credits),Button::Quiet),card(column![design::mark(84.0),title("PalantirMC"),subtitle("A notebook for your next world."),muted(format!("Version {} · native desktop",env!("CARGO_PKG_VERSION"))),label("An independently authored interface around a Rust game-launching engine. Asymmetric wayfinder artwork, original terrain illustrations, native system typography, and no borrowed launcher branding.")].spacing(18)),card(column![subtitle("Provenance & credits"),label("Launcher artwork and UI copy are original work recorded in THIRD_PARTY_NOTICES.md. iced is MIT; serde, serde_json, reqwest and sha1 offer MIT / Apache-2.0. zip is MIT."),muted("This launcher is not affiliated with the game or catalogue operators. Their product and project names identify compatible services, not our branding."),muted("Original does not mean legally unprotected. Artwork may itself be copyrightable; similarity and trademark questions require legal review."),muted("Product licence: UNLICENSED placeholder until the owner chooses. No legal-clearance guarantee is made.")].spacing(14))].spacing(18).into()
    }
    fn dialog_view(&self, dialog: &Dialog) -> Element<'_, Message> {
        if let Dialog::Surface(panel) = dialog {
            return self.outer_dialog(panel);
        }
        if let Some(view) = self.extended_dialog(dialog) {
            return view;
        }
        let(header,body):(&str,Element<'_,Message>)=match dialog{
            Dialog::Rename(id)=>("Rename this collection",column![input("Collection name",&self.name).id(text_input::Id::new("dialog-first")).on_input(Message::Name).on_submit(Message::Rename(id.clone())).padding(12),muted("Only the display name changes. Files remain in the same place."),action("Save name",Message::Rename(id.clone()),Button::Primary)].spacing(14).into()),
            Dialog::Delete(id)=>{
                // The confirmed operation's receipt belongs in this dialog,
                // not behind the scrim: the outcome is rendered here, the way
                // reviewed file operations render theirs.
                let body:Element<'_,Message>=match &self.delete_outcome{
                    Some(Ok(receipt))=>column![label(receipt),action("Close",Message::CloseDialog,Button::Primary)].spacing(16).into(),
                    Some(Err(error))=>column![muted(format!("The collection was not deleted: {error}")),action("Close",Message::CloseDialog,Button::Primary)].spacing(16).into(),
                    None if self.delete_pending=>column![muted("Removing this collection…"),surfaces::disabled("Deleting…")].spacing(16).into(),
                    None=>column![subtitle(self.instances.iter().find(|i|&i.id==id).map(|i|i.name.as_str()).unwrap_or("Collection")),muted("Its saved worlds, settings, and additions will be deleted. Shared engine downloads stay. This cannot be undone by the launcher."),action("Delete permanently",Message::Delete(id.clone()),Button::Danger)].spacing(16).into(),
                };
                let header=match &self.delete_outcome{Some(Ok(_))=>"Collection deleted",Some(Err(_))=>"Deletion could not finish",None=>"This removes a whole collection"};
                (header,body)
            },
            Dialog::Install=>{let mut choices=column![muted("Choose a stopped collection. Compatibility and hashes are checked before files are installed.")].spacing(12); let mut eligible=0;if let Some(project)=&self.project{choices=choices.push(subtitle(&project.title));if project.project_type=="modpack"{choices=choices.push(muted("A modpack needs its own collection: choose Configure new pack collection. A pack's overrides must not land on top of a collection's existing worlds."));}else{for item in &self.instances{if !self.running.contains_key(&item.id)&&!self.pending.contains_key(&item.id) && (project.project_type != "mod" || item.loader != Loader::Vanilla) {eligible+=1;choices=choices.push(action(&format!("{} · {} / {}",item.name,item.game_version,item.loader),Message::InstallContent(item.id.clone()),Button::Primary));}}}}
                if eligible == 0 && self.project.as_ref().is_none_or(|p| p.project_type != "modpack") { choices=choices.push(muted("No eligible stopped collection. Mods need Fabric or Quilt; stop active sessions before installing.")); choices=choices.push(action("Create a collection",Message::Open(Dialog::Create),Button::Quiet)); }("Add to a collection",choices.into())},
            Dialog::JavaAdd=>("Add a Java executable",column![label("Executable path"),input("/usr/lib/jvm/…/bin/java",&self.java_add_path).id(text_input::Id::new("dialog-first")).on_input(Message::JavaAddPath).on_submit(Message::JavaAddSave).padding(12),muted("The executable is asked for its version before it joins the list; nothing is installed or deleted here."),action("Check and add",Message::JavaAddSave,Button::Primary)].spacing(14).into()),
            Dialog::Duplicate(id)=>("Duplicate this collection",column![input("Collection name",&self.name).id(text_input::Id::new("dialog-first")).on_input(Message::Name).on_submit(Message::DuplicateSave(id.clone())).padding(12),muted("The copy carries its worlds, content, and settings byte for byte. Shared engine downloads stay shared."),if self.instances.iter().any(|item| item.id == *id){action("Create the copy",Message::DuplicateSave(id.clone()),Button::Primary)}else{surfaces::disabled("Collection no longer exists")}].spacing(14).into()),
            Dialog::InstanceWindow(id)=>("Set this collection's window size",column![label("Width (pixels)"),input("Unset – the game chooses",&self.instance_window_width).id(text_input::Id::new("dialog-first")).on_input(Message::InstanceWindowWidth).padding(12),label("Height (pixels)"),input("Unset – the game chooses",&self.instance_window_height).on_input(Message::InstanceWindowHeight).padding(12),muted("Leave both blank to follow the launcher window size. The size applies at the next launch."),if self.instances.iter().any(|item| item.id == *id){action("Save window size",Message::InstanceWindowSave(id.clone()),Button::Primary)}else{surfaces::disabled("Collection no longer exists")}].spacing(14).into()),
            Dialog::InstanceRuntime(id)=>("Set this collection's launch settings",column![label("Maximum Java memory (MiB)"),input("Launcher default",&self.instance_memory).id(text_input::Id::new("dialog-first")).on_input(Message::InstanceRuntimeMemory).padding(12),muted("512–32768 MiB. Leave blank to follow the launcher's budget."),label("Java executable"),input("Launcher default",&self.instance_java).on_input(Message::InstanceRuntimeJava).padding(12),muted("Blank follows the launcher setting; with nothing set anywhere, the runtime named by the game's metadata is used. A path is executed directly, never as a shell command."),label("Extra launch arguments"),input("--example \"two words\"",&self.instance_args).on_input(Message::InstanceRuntimeArgs).padding(12),muted("Shell-quoted words appended after the game's own arguments."),label("Environment entries"),input("FOO=1 BAR=\"two words\"",&self.instance_env).on_input(Message::InstanceRuntimeEnv).padding(12),muted("Shell-quoted KEY=VALUE pairs applied in order. Only the first = separates, so a value may contain one."),label("Pre-launch hook"),input("Runs before the game starts",&self.instance_hook_pre).on_input(Message::InstanceRuntimeHookPre).padding(12),muted("Runs to completion before the game spawns; a failing hook stops the launch."),label("Wrapper hook"),input("Runs around the game command",&self.instance_hook_wrapper).on_input(Message::InstanceRuntimeHookWrapper).padding(12),muted("Its words become the program and its prefix arguments, then the game follows — like gamemoderun java …"),label("Post-exit hook"),input("Runs after the game closes",&self.instance_hook_post).on_input(Message::InstanceRuntimeHookPost).padding(12),muted("Runs after the game's process ends, however it ended. Commands are parsed with shell quoting but executed directly — no shell."),if self.instances.iter().any(|item| item.id == *id){action("Save launch settings",Message::InstanceRuntimeSave(id.clone()),Button::Primary)}else{surfaces::disabled("Collection no longer exists")}].spacing(14).into()),
            Dialog::Account=>{
                let (_, active_name, _) = self.preferences.active_identity();
                let mut body=column![design::emblem("account"),subtitle(&active_name),label("Offline identity · active"),muted("Offline play does not verify ownership or provide authenticated multiplayer. Play launches with the identity selected here.")].spacing(16);
                body=body.push(subtitle("Offline accounts"));
                if self.preferences.offline_accounts.is_empty(){body=body.push(muted("No saved accounts yet. Play uses the fallback name in Preferences, and the first account added becomes the active one."));}
                for account in &self.preferences.offline_accounts {
                    let active=self.preferences.active_account.as_deref()==Some(account.id.as_str());
                    let detail=if active{"Active · used by Play".to_string()}else{format!("Offline · {}",account.uuid())};
                    let select = if active{surfaces::disabled("Selected")}else{action("Use this account",Message::AccountSelect(account.id.clone()),Button::Quiet)};
                    body=body.push(card(column![label(&account.name),muted(detail).size(12), self.actions(vec![select,action("Remove…",Message::Open(Dialog::Surface(surfaces::Panel::RemoveAccount(account.id.clone()))),Button::Danger)])].spacing(10)));
                }
                body=body.push(input("New offline name",&self.account_name).id(text_input::Id::new("dialog-first")).on_input(Message::AccountName).on_submit(Message::AccountAdd).padding(12));
                body=body.push(if model::validate_player_name(&self.account_name).is_ok() { action("Add account",Message::AccountAdd,Button::Primary) } else { surfaces::disabled("Enter a valid offline name") });
                body=body.push(muted("1–16 letters, digits, or underscores, and each name only once. Adding an account selects it and saves the list."));
                body=body.push(action("Edit the fallback name",Message::EditPlayer,Button::Quiet));
                body=body.push(subtitle("Microsoft account"));
                if let model::Authentication::Connected { player } = &self.authentication { body=body.push(label(format!("Verified Java profile: {player}"))); }
                body=body.push(action("Review play mode", Message::Open(Dialog::Surface(surfaces::Panel::Launch(self.instances.first().map(|i|i.id.clone()).unwrap_or_default()))), Button::Quiet));
                body=body.push(muted("Device-code sign-in requires the registered authentication backend. The setup screen explains its availability before requesting any code."));
                body=body.push(action("Sign in with Microsoft…",Message::Open(Dialog::SignIn),Button::Primary));
                ("Your player identity",body.into())
            },
            Dialog::InstanceActions(id)=>match self.instances.iter().find(|i|&i.id==id){
                // The complete action set for one collection. Every entry is a
                // plain button in a dialog, so every action has a keyboard
                // path -- none of them lives behind a right-click only.
                Some(item)=>{let busy=self.running.contains_key(id)||self.pending.contains_key(id);("Collection actions",column![
                    muted("Everything this collection can do. Each action also lives in the collection itself."),
                    action("Open collection",Message::Select(id.clone()),Button::Quiet),
                    self.play_button(item),
                    action("Rename collection…",Message::Open(Dialog::Rename(id.clone())),Button::Quiet),
                    action("Duplicate collection…",Message::DuplicateOpen(id.clone()),Button::Quiet),
                    action("Back up or export a world…",Message::SelectTab(id.clone(),InstanceTab::Worlds),Button::Quiet),
                    muted("Worlds back up and export from the Worlds tab. A whole-collection export is not built yet."),
                    action("Open game folder",Message::OpenFolder(Some(id.clone()),String::new()),Button::Quiet),
                    if busy{surfaces::disabled("Stop current work before deleting")}else{action("Delete collection…",Message::Open(Dialog::Delete(id.clone())),Button::Danger)},
                ].spacing(12).into())},
                None=>("Collection actions",column![muted("That collection no longer exists."),action("Close",Message::CloseDialog,Button::Quiet)].spacing(12).into()),
            },
            _ => ("Launcher", column![muted("Return to the launcher."), action("Close", Message::CloseDialog, Button::Quiet)].into()),
        };
        self.panel_shell(header, body, false)
    }
}
/// The window drafts show an unset side as blank rather than a zero.
fn draft_side(value: u32) -> String {
    if value == 0 {
        String::new()
    } else {
        value.to_string()
    }
}
impl Drop for App {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _closed = worker.send(backend::Request::Shutdown);
        }
    }
}
fn action<'a>(caption: &str, message: Message, style: Button) -> Element<'a, Message> {
    controls::action(caption, message, style)
}
fn card<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content)
        .padding(20)
        .width(Length::Fill)
        .style(design::panel)
        .into()
}
fn byte_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{} KiB", bytes / 1024)
    }
}
fn markdown(body: &str) -> Element<'_, Message> {
    // A text-only native reading surface: headings, lists, and code, never remote HTML/scripts.
    let mut content = column![].spacing(9);
    let mut code = false;
    for line in body.lines().take(1500) {
        if line.trim_start().starts_with("```") {
            code = !code;
            continue;
        }
        if code {
            content = content.push(
                container(label(line).font(iced::Font::MONOSPACE).size(12))
                    .padding(8)
                    .style(design::well),
            );
        } else if let Some(text) = line.strip_prefix("# ") {
            content = content.push(subtitle(text));
        } else if let Some(text) = line
            .strip_prefix("## ")
            .or_else(|| line.strip_prefix("### "))
        {
            content = content.push(label(text).size(17));
        } else if let Some(text) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            content = content.push(label(format!("• {text}")));
        } else if !line.trim().is_empty() {
            content = content.push(label(line));
        }
    }
    content.into()
}
#[cfg(test)]
mod ui_tests {
    use super::*;
    #[test]
    fn all_required_settings_and_instance_surfaces_are_named() {
        assert_eq!(SettingsTab::ALL.len(), 8);
        assert_eq!(InstanceTab::ALL.len(), 5);
        for tab in SettingsTab::ALL {
            assert!(!tab.name().is_empty());
        }
    }
    fn isolated_app() -> (App, mpsc::Receiver<backend::Request>) {
        let (worker, requests) = mpsc::channel();
        let (_sender, events) = mpsc::channel();
        (App::with_worker(worker, events), requests)
    }
    fn project(id: &str) -> Project {
        Project {
            project_id: id.into(),
            title: "Test addition".into(),
            description: "An author's description".into(),
            author: "Author".into(),
            downloads: 42,
            project_type: "mod".into(),
        }
    }
    #[test]
    fn create_is_submitted_once_and_only_its_completion_closes_the_dialog() {
        let (mut app, requests) = isolated_app();
        let _ = app.update(Message::Open(Dialog::Create));
        assert!(matches!(
            requests.try_recv().unwrap(),
            backend::Request::GameVersions
        ));
        let _ = app.update(Message::Name("Valley".into()));
        let _ = app.update(Message::Create);
        let _ = app.update(Message::Create);
        assert!(
            matches!(requests.try_recv().unwrap(), backend::Request::Create(name, _, _, _) if name == "Valley")
        );
        assert!(requests.try_recv().is_err());
        app.receive(backend::Event::Finished(
            "other".into(),
            Ok("Installed".into()),
        ));
        assert!(matches!(app.dialog, Some(Dialog::Create)));
        app.receive(backend::Event::Finished(
            "create".into(),
            Err("Invalid version".into()),
        ));
        assert!(matches!(app.dialog, Some(Dialog::Create)));
        assert!(app.notice.as_ref().unwrap().contains("Invalid version"));
        let _ = app.dialog_view(app.dialog.as_ref().unwrap());
        app.receive(backend::Event::Finished(
            "create".into(),
            Ok("Created".into()),
        ));
        assert!(app.dialog.is_none());
        assert!(!app.pending.contains_key("create"));
    }
    #[test]
    fn typing_in_the_catalogue_searches_once_typing_settles() {
        // Not isolated_app: the ticker treats a dropped event channel as a
        // stopped worker, so this test keeps its sender alive.
        let (worker, requests) = mpsc::channel();
        let (_event_sender, events) = mpsc::channel();
        let mut app = App::with_worker(worker, events);
        let _ = app.update(Message::Query("shaders".into()));
        assert!(app.search_dirty);
        // Typing keeps the search pending; no request leaves early.
        let _ = app.update(Message::Tick);
        let early = requests.try_recv();
        assert!(
            matches!(early, Err(std::sync::mpsc::TryRecvError::Empty)),
            "early request: {early:?}"
        );
        app.search_due = std::time::Instant::now();
        let _ = app.update(Message::Tick);
        assert!(!app.search_dirty);
        assert!(app.searching);
        match requests.try_recv().unwrap() {
            backend::Request::Search(_, query, _, _, 0, _) => assert_eq!(query, "shaders"),
            other => panic!("expected a catalogue search, got {other:?}"),
        }
        // An explicit submit replaces the pending debounce immediately.
        let _ = app.update(Message::Query("lanterns".into()));
        let _ = app.update(Message::Search);
        assert!(!app.search_dirty);
        match requests.try_recv().unwrap() {
            backend::Request::Search(_, query, _, _, 0, _) => assert_eq!(query, "lanterns"),
            other => panic!("expected a catalogue search, got {other:?}"),
        }
    }
    #[test]
    fn stale_catalogue_responses_never_replace_a_newer_search_or_project() {
        let (mut app, requests) = isolated_app();
        let _ = app.update(Message::Search);
        let first = app.search_id;
        let _ = app.update(Message::Query("second".into()));
        let _ = app.update(Message::Search);
        app.receive(backend::Event::Search(first, Ok((vec![project("old")], 1))));
        assert!(app.searching);
        assert!(app.results.is_empty());
        app.receive(backend::Event::Search(
            app.search_id,
            Ok((vec![project("new")], 1)),
        ));
        assert!(!app.searching);
        assert_eq!(app.results[0].project_id, "new");
        let _ = app.update(Message::Project(project("new")));
        app.receive(backend::Event::Project(
            "old".into(),
            Err("stale failure".into()),
        ));
        assert!(app.detail_loading);
        assert!(app.notice.is_none());
        assert!(matches!(
            requests.try_recv().unwrap(),
            backend::Request::Search(_, _, _, _, _, _)
        ));
    }
    #[test]
    fn logs_and_activity_history_are_bounded() {
        let (mut app, _requests) = isolated_app();
        for number in 0..1000 {
            app.receive(backend::Event::Log("test".into(), number.to_string()));
        }
        assert_eq!(app.logs["test"].len(), 800);
        assert_eq!(app.logs["test"].front().unwrap(), "200");
        for number in 0..20 {
            app.note(number.to_string());
        }
        assert_eq!(app.history.len(), 8);
    }
    #[test]
    fn every_page_pane_tab_and_dialog_builds_in_both_layouts() {
        let (mut app, _requests) = isolated_app();
        app.instances.push(Instance {
            id: "test".into(),
            name: "A collection".into(),
            game_version: "1.20.1".into(),
            loader: Loader::Fabric,
            loader_version: None,
            window: None,
            memory_mb: None,
            java: None,
            custom: Default::default(),
            installed_version: None,
        });
        for width in [840.0, 1280.0] {
            app.width = width;
            for page in [
                Page::Home,
                Page::Library,
                Page::Discover,
                Page::Settings,
                Page::About,
            ] {
                app.page = page;
                let _ = app.view();
            }
            app.page = Page::Settings;
            for tab in SettingsTab::ALL {
                app.settings_tab = tab;
                let _ = app.view();
            }
            app.page = Page::Library;
            app.selected = Some("test".into());
            for tab in InstanceTab::ALL {
                app.instance_tab = tab;
                let _ = app.view();
            }
            for dialog in [
                Dialog::Create,
                Dialog::Import,
                Dialog::Rename("test".into()),
                Dialog::Duplicate("test".into()),
                Dialog::InstanceWindow("test".into()),
                Dialog::InstanceRuntime("test".into()),
                Dialog::Delete("test".into()),
                Dialog::InstanceActions("test".into()),
                Dialog::Account,
                Dialog::Install,
            ] {
                app.dialog = Some(dialog);
                let _ = app.view();
            }
            app.dialog = None;
            app.selected = None;
        }
    }
    #[test]
    fn window_sizes_pair_up_before_persistence() {
        let (mut app, requests) = isolated_app();
        // A half-filled pair is refused and nothing is sent to persistence.
        let _ = app.update(Message::WindowWidth("1280".into()));
        let _ = app.update(Message::SaveSettings);
        assert!(requests.try_recv().is_err());
        assert!(app.notice.is_some());
        let _ = app.update(Message::WindowHeight("720".into()));
        let _ = app.update(Message::SaveSettings);
        match requests.try_recv().unwrap() {
            backend::Request::Save(prefs) => {
                assert_eq!((prefs.window_width, prefs.window_height), (1280, 720));
            }
            other => panic!("expected a preferences save, got {other:?}"),
        }
        // The next save waits for the in-flight one's receipt, exactly as the
        // launcher enforces it.
        app.receive(backend::Event::Finished(
            "preferences".into(),
            Ok("Saved".into()),
        ));
        // Both blank means the game's own default, not a zero-sized window.
        let _ = app.update(Message::WindowWidth(String::new()));
        let _ = app.update(Message::WindowHeight(String::new()));
        let _ = app.update(Message::SaveSettings);
        match requests.try_recv().unwrap() {
            backend::Request::Save(prefs) => {
                assert_eq!((prefs.window_width, prefs.window_height), (0, 0));
            }
            other => panic!("expected a preferences save, got {other:?}"),
        }
    }
    #[test]
    fn collection_window_dialog_sends_an_override_and_a_clear() {
        let (mut app, requests) = isolated_app();
        app.instances.push(Instance {
            id: "world-test".into(),
            name: "Test".into(),
            game_version: "1.20.1".into(),
            loader: Loader::Vanilla,
            installed_version: None,
            loader_version: None,
            window: None,
            memory_mb: None,
            java: None,
            custom: Default::default(),
        });
        let _ = app.update(Message::InstanceWindowOpen("world-test".into()));
        assert!(matches!(app.dialog, Some(Dialog::InstanceWindow(ref id)) if id == "world-test"));
        let _ = app.update(Message::InstanceWindowWidth("800".into()));
        let _ = app.update(Message::InstanceWindowHeight("600".into()));
        let _ = app.update(Message::InstanceWindowSave("world-test".into()));
        match requests.try_recv().unwrap() {
            backend::Request::Window(id, window) => {
                assert_eq!(id, "world-test");
                assert_eq!(
                    window,
                    Some(model::WindowOverride {
                        width: 800,
                        height: 600
                    })
                );
            }
            other => panic!("expected a window override, got {other:?}"),
        }
        // The worker's refresh carries the stored override back into the
        // list; reopening the dialog prefills from it.
        app.receive(backend::Event::Instances(vec![Instance {
            window: Some(model::WindowOverride {
                width: 800,
                height: 600,
            }),
            ..app.instances[0].clone()
        }]));
        let _ = app.update(Message::InstanceWindowOpen("world-test".into()));
        assert_eq!(app.instance_window_width, "800");
        assert_eq!(app.instance_window_height, "600");
        // Blank drafts save no override at all: the launcher preference rules.
        let _ = app.update(Message::InstanceWindowWidth(String::new()));
        let _ = app.update(Message::InstanceWindowHeight(String::new()));
        let _ = app.update(Message::InstanceWindowSave("world-test".into()));
        assert!(matches!(
            requests.try_recv().unwrap(),
            backend::Request::Window(_, None)
        ));
        let _ = app.update(Message::InstanceWindowClear("world-test".into()));
        assert!(matches!(
            requests.try_recv().unwrap(),
            backend::Request::Window(_, None)
        ));
    }
    #[test]
    fn java_roster_probes_a_runtime_and_removes_it_again() {
        let (mut app, requests) = isolated_app();
        let _ = app.update(Message::JavaAddOpen);
        assert!(matches!(app.dialog, Some(Dialog::JavaAdd)));
        // An empty or hostile path is refused before anything runs.
        let _ = app.update(Message::JavaAddSave);
        assert!(requests.try_recv().is_err());
        let _ = app.update(Message::JavaAddPath("/opt/jdk/bin/java".into()));
        let _ = app.update(Message::JavaAddSave);
        assert!(matches!(
            requests.try_recv().unwrap(),
            backend::Request::JavaProbe(path) if path == "/opt/jdk/bin/java"
        ));
        // The probe's answer becomes the roster label and is persisted, and
        // the receipt lands back in Preferences -- the add dialog is part of
        // that visit, not a way out of it.
        app.receive(backend::Event::JavaProbed(
            "/opt/jdk/bin/java".into(),
            Ok("openjdk 21.0.3".into()),
        ));
        assert!(matches!(app.dialog, Some(Dialog::Preferences)));
        assert_eq!(app.settings_tab, SettingsTab::Java);
        assert_eq!(app.preferences.java_installs.len(), 1);
        assert_eq!(app.preferences.java_installs[0].label, "openjdk 21.0.3");
        assert!(matches!(
            requests.try_recv().unwrap(),
            backend::Request::Save(p) if p.java_installs.len() == 1
        ));
        // The write's snapshot is the new baseline: a revert cannot undo an
        // add whose save is already on its way to disk.
        assert_eq!(app.settings_baseline().java_installs.len(), 1);
        // The save's own receipt must not bury the action's: the note still
        // names what happened after the write lands.
        app.receive(backend::Event::Finished(
            "preferences".into(),
            Ok("Preferences saved.".into()),
        ));
        assert!(
            app.notice
                .as_deref()
                .unwrap_or_default()
                .contains("added to the list")
        );
        // Selecting it drives launches; removing it clears the selection too.
        let _ = app.update(Message::JavaUse("/opt/jdk/bin/java".into()));
        assert_eq!(app.preferences.java, "/opt/jdk/bin/java");
        assert!(matches!(
            requests.try_recv().unwrap(),
            backend::Request::Save(p) if p.java == "/opt/jdk/bin/java"
        ));
        let _ = app.update(Message::JavaRemove("/opt/jdk/bin/java".into()));
        assert!(app.preferences.java_installs.is_empty());
        assert_eq!(app.preferences.java, "");
        assert!(matches!(
            requests.try_recv().unwrap(),
            backend::Request::Save(p) if p.java_installs.is_empty()
        ));
        app.receive(backend::Event::Finished(
            "preferences".into(),
            Ok("Preferences saved.".into()),
        ));
        assert!(
            app.notice
                .as_deref()
                .unwrap_or_default()
                .contains("removed from the list")
        );
    }
    #[test]
    fn cancelling_the_add_dialog_returns_to_the_roster() {
        let (mut app, _requests) = isolated_app();
        let _ = app.update(Message::Navigate(Page::Settings));
        let _ = app.update(Message::SettingsTab(SettingsTab::Java));
        let _ = app.update(Message::JavaAddOpen);
        assert!(matches!(app.dialog, Some(Dialog::JavaAdd)));
        let _ = app.update(Message::CloseDialog);
        assert!(matches!(app.dialog, Some(Dialog::Preferences)));
        assert_eq!(app.settings_tab, SettingsTab::Java);
    }
    #[test]
    fn closing_preferences_cannot_roll_back_an_immediate_save() {
        let (mut app, requests) = isolated_app();
        let _ = app.update(Message::Navigate(Page::Settings));
        app.preferences.java_installs.push(model::JavaInstall {
            label: "openjdk 21".into(),
            path: "/opt/jdk/bin/java".into(),
        });
        app.save_preferences_now("Preferences saved.".into());
        assert!(matches!(
            requests.try_recv().unwrap(),
            backend::Request::Save(p) if p.java_installs.len() == 1
        ));
        // Closing the dialog reverts drafts -- to the snapshot on its way to
        // disk, not to a baseline that predates the add.
        let _ = app.update(Message::CloseDialog);
        assert_eq!(app.preferences.java_installs.len(), 1);
        // The receipt then makes that snapshot the confirmed baseline.
        app.receive(backend::Event::Finished(
            "preferences".into(),
            Ok("Preferences saved.".into()),
        ));
        assert_eq!(app.saved_preferences.java_installs.len(), 1);
    }
    #[test]
    fn a_probe_answer_for_a_closed_dialog_is_ignored() {
        let (mut app, _requests) = isolated_app();
        app.receive(backend::Event::JavaProbed(
            "/opt/jdk/bin/java".into(),
            Ok("openjdk 21".into()),
        ));
        assert!(app.preferences.java_installs.is_empty());
    }
    #[test]
    fn the_actions_menu_is_the_complete_keyboard_path_for_one_collection() {
        let (mut app, requests) = isolated_app();
        app.instances.push(Instance {
            id: "test".into(),
            name: "Test".into(),
            game_version: "1.20.1".into(),
            loader: Loader::Vanilla,
            installed_version: None,
            loader_version: None,
            window: None,
            memory_mb: None,
            java: None,
            custom: Default::default(),
        });
        // The menu opens from the card's and the header's Actions button and
        // every entry in it is a plain button -- the keyboard path.
        let _ = app.update(Message::Open(Dialog::InstanceActions("test".into())));
        assert!(matches!(app.dialog, Some(Dialog::InstanceActions(ref id)) if id == "test"));
        // "Open collection" ends the menu visit and lands on the collection.
        let _ = app.update(Message::Select("test".into()));
        assert!(app.dialog.is_none());
        assert_eq!(app.selected.as_deref(), Some("test"));
        // The export entry lands on the world export surface: the collection
        // opens on its Worlds tab.
        let _ = app.update(Message::Open(Dialog::InstanceActions("test".into())));
        let _ = app.update(Message::SelectTab("test".into(), InstanceTab::Worlds));
        assert!(app.dialog.is_none());
        assert_eq!(app.instance_tab, InstanceTab::Worlds);
        // Delete goes through its confirmation first, and nothing is removed
        // until that dialog's own danger action is chosen.
        let _ = app.update(Message::Open(Dialog::InstanceActions("test".into())));
        let _ = app.update(Message::Open(Dialog::Delete("test".into())));
        assert!(matches!(app.dialog, Some(Dialog::Delete(ref id)) if id == "test"));
        let _ = app.update(Message::Delete("test".into()));
        // The confirmation stays open for its receipt.
        assert!(matches!(app.dialog, Some(Dialog::Delete(ref id)) if id == "test"));
        assert!(app.delete_pending);
        let sent: Vec<_> = requests.try_iter().collect();
        assert_eq!(sent.len(), 3); // two inventories and the delete, in order
        assert!(matches!(sent[0], backend::Request::Inventory(ref id) if id == "test"));
        assert!(matches!(sent[2], backend::Request::Delete(ref id) if id == "test"));
    }
    #[test]
    fn the_deletion_receipt_renders_inside_the_open_dialog() {
        let (mut app, requests) = isolated_app();
        app.instances.push(Instance {
            id: "test".into(),
            name: "Test".into(),
            game_version: "1.20.1".into(),
            loader: Loader::Vanilla,
            installed_version: None,
            loader_version: None,
            window: None,
            memory_mb: None,
            java: None,
            custom: Default::default(),
        });
        let _ = app.update(Message::Open(Dialog::Delete("test".into())));
        let _ = app.update(Message::Delete("test".into()));
        assert!(matches!(app.dialog, Some(Dialog::Delete(ref id)) if id == "test"));
        assert!(app.delete_pending);
        assert!(matches!(
            requests.try_recv().unwrap(),
            backend::Request::Delete(id) if id == "test"
        ));
        // The receipt lands in the dialog state itself -- not only in a
        // banner the scrim would hide.
        app.receive(backend::Event::Finished(
            "test".into(),
            Ok("Instance deleted. Shared downloads were kept.".into()),
        ));
        assert!(matches!(&app.delete_outcome, Some(Ok(note)) if note.contains("Instance deleted")));
        assert!(!app.delete_pending);
        assert!(
            matches!(app.dialog, Some(Dialog::Delete(_))),
            "the receipt lands in the open dialog"
        );
        let _ = app.update(Message::CloseDialog);
        assert!(app.dialog.is_none());
    }
    #[test]
    fn a_refused_deletion_reports_inside_the_dialog_too() {
        let (mut app, _requests) = isolated_app();
        app.instances.push(Instance {
            id: "test".into(),
            name: "Test".into(),
            game_version: "1.20.1".into(),
            loader: Loader::Vanilla,
            installed_version: None,
            loader_version: None,
            window: None,
            memory_mb: None,
            java: None,
            custom: Default::default(),
        });
        let _ = app.update(Message::Open(Dialog::Delete("test".into())));
        let _ = app.update(Message::Delete("test".into()));
        app.receive(backend::Event::Finished(
            "test".into(),
            Err("Stop the instance and wait first.".into()),
        ));
        assert!(
            matches!(&app.delete_outcome, Some(Err(error)) if error.contains("Stop the instance"))
        );
        assert!(matches!(app.dialog, Some(Dialog::Delete(_))));
    }
    #[test]
    fn a_child_dialog_closes_back_into_the_visit_it_belongs_to() {
        let (mut app, _requests) = isolated_app();
        // Preferences → Account → sign-in: every close goes one level up.
        let _ = app.update(Message::Navigate(Page::Settings));
        let _ = app.update(Message::Open(Dialog::Account));
        let _ = app.update(Message::Open(Dialog::SignIn));
        let _ = app.update(Message::CloseDialog);
        assert!(matches!(app.dialog, Some(Dialog::Account)));
        let _ = app.update(Message::CloseDialog);
        assert!(matches!(app.dialog, Some(Dialog::Preferences)));
        // Closing Preferences itself ends the visit.
        let _ = app.update(Message::CloseDialog);
        assert!(app.dialog.is_none());
        // Navigation wipes the visit memory: no stale parent may resurrect
        // under a later dialog.
        let _ = app.update(Message::Navigate(Page::Settings));
        let _ = app.update(Message::Open(Dialog::Account));
        let _ = app.update(Message::Navigate(Page::Home));
        let _ = app.update(Message::Open(Dialog::SignIn));
        let _ = app.update(Message::CloseDialog);
        assert!(app.dialog.is_none());
    }
    #[test]
    fn play_from_the_account_review_returns_to_the_account_visit() {
        let (mut app, requests) = isolated_app();
        app.instances.push(Instance {
            id: "test".into(),
            name: "Test".into(),
            game_version: "1.20.1".into(),
            loader: Loader::Vanilla,
            installed_version: None,
            loader_version: None,
            window: None,
            memory_mb: None,
            java: None,
            custom: Default::default(),
        });
        let _ = app.update(Message::Navigate(Page::Settings));
        let _ = app.update(Message::Open(Dialog::Account));
        let _ = app.update(Message::Open(Dialog::Surface(surfaces::Panel::Launch(
            "test".into(),
        ))));
        let _ = app.update(Message::Launch("test".into()));
        assert!(
            matches!(app.dialog, Some(Dialog::Account)),
            "starting the game ends the review, not the visit it came from"
        );
        assert!(matches!(
            requests.try_recv().unwrap(),
            backend::Request::Launch(id, _, _) if id == "test"
        ));
    }
    #[test]
    fn play_from_a_card_ends_on_the_page_as_it_always_has() {
        let (mut app, _requests) = isolated_app();
        app.instances.push(Instance {
            id: "test".into(),
            name: "Test".into(),
            game_version: "1.20.1".into(),
            loader: Loader::Vanilla,
            installed_version: None,
            loader_version: None,
            window: None,
            memory_mb: None,
            java: None,
            custom: Default::default(),
        });
        let _ = app.update(Message::Open(Dialog::Surface(surfaces::Panel::Launch(
            "test".into(),
        ))));
        let _ = app.update(Message::Launch("test".into()));
        assert!(app.dialog.is_none());
    }
    #[test]
    fn an_immediate_save_says_its_receipt_once() {
        let (mut app, requests) = isolated_app();
        app.preferences.java_installs.push(model::JavaInstall {
            label: "openjdk 21".into(),
            path: "/opt/jdk/bin/java".into(),
        });
        let _ = app.update(Message::JavaRemove("/opt/jdk/bin/java".into()));
        assert!(matches!(
            requests.try_recv().unwrap(),
            backend::Request::Save(_)
        ));
        // The write's confirmation repeats the action's own sentence;
        // Recent notes records the event once, not twice.
        app.receive(backend::Event::Finished(
            "preferences".into(),
            Ok("Preferences saved.".into()),
        ));
        assert_eq!(
            app.history
                .iter()
                .filter(|note| note.contains("removed from the list"))
                .count(),
            1
        );
        assert!(
            app.notice
                .as_deref()
                .unwrap_or_default()
                .contains("removed from the list")
        );
    }
    #[test]
    fn duplicating_a_collection_carries_a_name_and_closes_on_its_receipt() {
        let (mut app, requests) = isolated_app();
        app.instances.push(Instance {
            id: "test".into(),
            name: "Test".into(),
            game_version: "1.20.1".into(),
            loader: Loader::Vanilla,
            installed_version: None,
            loader_version: None,
            window: None,
            memory_mb: None,
            java: None,
            custom: Default::default(),
        });
        let _ = app.update(Message::DuplicateOpen("test".into()));
        assert!(matches!(app.dialog, Some(Dialog::Duplicate(ref id)) if id == "test"));
        assert_eq!(app.name, "Copy of Test");
        // A nameless copy is refused before anything is sent.
        let _ = app.update(Message::Name("  ".into()));
        let _ = app.update(Message::DuplicateSave("test".into()));
        assert!(requests.try_recv().is_err());
        let _ = app.update(Message::Name("Copy of Test".into()));
        let _ = app.update(Message::DuplicateSave("test".into()));
        assert!(matches!(
            requests.try_recv().unwrap(),
            backend::Request::Duplicate(id, name) if id == "test" && name == "Copy of Test"
        ));
        // The receipt closes exactly this dialog.
        app.receive(backend::Event::Finished(
            "duplicate:test".into(),
            Ok("Duplicated".into()),
        ));
        assert!(app.dialog.is_none());
    }
    #[test]
    fn collection_runtime_dialog_saves_overrides_and_clears() {
        let (mut app, requests) = isolated_app();
        app.instances.push(Instance {
            id: "world-test".into(),
            name: "Test".into(),
            game_version: "1.20.1".into(),
            loader: Loader::Vanilla,
            installed_version: None,
            loader_version: None,
            window: None,
            memory_mb: None,
            java: None,
            custom: Default::default(),
        });
        let _ = app.update(Message::InstanceRuntimeOpen("world-test".into()));
        assert!(matches!(app.dialog, Some(Dialog::InstanceRuntime(ref id)) if id == "world-test"));
        // An impossible budget is refused before anything is sent.
        let _ = app.update(Message::InstanceRuntimeMemory("12".into()));
        let _ = app.update(Message::InstanceRuntimeSave("world-test".into()));
        assert!(requests.try_recv().is_err());
        assert!(app.notice.is_some());
        let _ = app.update(Message::InstanceRuntimeMemory("2048".into()));
        let _ = app.update(Message::InstanceRuntimeJava("/opt/java/bin/java".into()));
        let _ = app.update(Message::InstanceRuntimeArgs(
            r#"--quickPlaySingleplayer "My World""#.into(),
        ));
        let _ = app.update(Message::InstanceRuntimeEnv("FOO=a=b".into()));
        let _ = app.update(Message::InstanceRuntimeHookPre(
            r#""/path with space/hook.sh""#.into(),
        ));
        let _ = app.update(Message::InstanceRuntimeSave("world-test".into()));
        match requests.try_recv().unwrap() {
            backend::Request::Runtime(id, memory_mb, java, custom) => {
                assert_eq!(id, "world-test");
                assert_eq!(memory_mb, Some(2048));
                assert_eq!(java.as_deref(), Some("/opt/java/bin/java"));
                // Quoted words arrive whole and in order; an environment
                // value containing = survives intact; a hook is stored as
                // typed.
                assert_eq!(
                    custom.extra_args,
                    vec![
                        "--quickPlaySingleplayer".to_string(),
                        "My World".to_string()
                    ]
                );
                assert_eq!(custom.env, vec![("FOO".to_string(), "a=b".to_string())]);
                assert_eq!(
                    custom.hooks.pre_launch.as_deref(),
                    Some(r#""/path with space/hook.sh""#)
                );
            }
            other => panic!("expected launch-settings overrides, got {other:?}"),
        }
        // The worker's refresh carries the overrides back; reopening prefills.
        app.receive(backend::Event::Instances(vec![Instance {
            memory_mb: Some(2048),
            java: Some("/opt/java/bin/java".into()),
            custom: palantir_loader::hooks::LaunchCustom {
                extra_args: vec!["--quickPlaySingleplayer".into(), "My World".into()],
                env: vec![("FOO".into(), "a=b".into())],
                hooks: palantir_loader::hooks::Hooks {
                    pre_launch: Some(r#""/path with space/hook.sh""#.into()),
                    wrapper: None,
                    post_exit: None,
                },
            },
            ..app.instances[0].clone()
        }]));
        let _ = app.update(Message::InstanceRuntimeOpen("world-test".into()));
        assert_eq!(app.instance_memory, "2048");
        assert_eq!(app.instance_java, "/opt/java/bin/java");
        // Reopening shows the stored customs back in editable form.
        assert_eq!(app.instance_args, r#"--quickPlaySingleplayer 'My World'"#);
        assert_eq!(app.instance_env, "FOO=a=b");
        assert_eq!(app.instance_hook_pre, r#""/path with space/hook.sh""#);
        // Blank fields clear every override: the launcher rules again.
        let _ = app.update(Message::InstanceRuntimeMemory(String::new()));
        let _ = app.update(Message::InstanceRuntimeJava(String::new()));
        let _ = app.update(Message::InstanceRuntimeArgs(String::new()));
        let _ = app.update(Message::InstanceRuntimeEnv(String::new()));
        let _ = app.update(Message::InstanceRuntimeHookPre(String::new()));
        let _ = app.update(Message::InstanceRuntimeSave("world-test".into()));
        match requests.try_recv().unwrap() {
            backend::Request::Runtime(_, None, None, custom) => assert!(custom.is_empty()),
            other => panic!("expected cleared launch settings, got {other:?}"),
        }
        let _ = app.update(Message::InstanceRuntimeClear("world-test".into()));
        match requests.try_recv().unwrap() {
            backend::Request::Runtime(_, None, None, custom) => assert!(custom.is_empty()),
            other => panic!("expected cleared launch settings, got {other:?}"),
        }
    }
    #[test]
    fn a_hook_that_would_not_run_is_refused_before_persistence() {
        let (mut app, requests) = isolated_app();
        app.instances.push(Instance {
            id: "world-test".into(),
            name: "Test".into(),
            game_version: "1.20.1".into(),
            loader: Loader::Vanilla,
            installed_version: None,
            loader_version: None,
            window: None,
            memory_mb: None,
            java: None,
            custom: Default::default(),
        });
        let _ = app.update(Message::InstanceRuntimeOpen("world-test".into()));
        // An unterminated quote would run half a command; it is refused here
        // instead of at midnight.
        let _ = app.update(Message::InstanceRuntimeHookPre("\"open".into()));
        let _ = app.update(Message::InstanceRuntimeSave("world-test".into()));
        assert!(requests.try_recv().is_err());
        assert!(app.notice.is_some());
        let _ = app.update(Message::InstanceRuntimeHookPre(String::new()));
        let _ = app.update(Message::InstanceRuntimeEnv("NOT_A_PAIR".into()));
        let _ = app.update(Message::InstanceRuntimeSave("world-test".into()));
        assert!(requests.try_recv().is_err());
        assert!(app.notice.is_some());
    }
    #[test]
    fn features_hide_tabs_persist_and_restore() {
        let (mut app, requests) = isolated_app();
        // The default install shows every tab it has.
        assert_eq!(InstanceTab::visible(&app.preferences.features).len(), 5);
        app.instance_tab = InstanceTab::Worlds;
        let _ = app.update(Message::FeatureWorlds(false));
        assert!(!InstanceTab::visible(&app.preferences.features).contains(&InstanceTab::Worlds));
        // A hidden tab is never the selected one and cannot be selected.
        assert_eq!(app.instance_tab, InstanceTab::Overview);
        let _ = app.update(Message::InstanceTab(InstanceTab::Worlds));
        assert_eq!(app.instance_tab, InstanceTab::Overview);
        // The choice travels with the saved preferences.
        let _ = app.update(Message::SaveSettings);
        match requests.try_recv().unwrap() {
            backend::Request::Save(prefs) => assert!(!prefs.features.show_worlds),
            other => panic!("expected a preferences save, got {other:?}"),
        }
        // Turning it back on restores exactly the previous state.
        let _ = app.update(Message::FeatureWorlds(true));
        assert!(InstanceTab::visible(&app.preferences.features).contains(&InstanceTab::Worlds));
    }
    #[test]
    fn hiding_every_tab_says_where_the_switches_live() {
        let (mut app, _requests) = isolated_app();
        for message in [
            Message::FeatureOverview(false),
            Message::FeatureContent(false),
            Message::FeatureWorlds(false),
            Message::FeatureLogs(false),
            Message::FeatureManage(false),
        ] {
            let _ = app.update(message);
        }
        assert!(InstanceTab::visible(&app.preferences.features).is_empty());
        app.instances.push(Instance {
            id: "test".into(),
            name: "Test".into(),
            game_version: "1.20.1".into(),
            loader: Loader::Vanilla,
            installed_version: None,
            loader_version: None,
            window: None,
            memory_mb: None,
            java: None,
            custom: Default::default(),
        });
        app.selected = Some("test".into());
        app.page = Page::Library;
        // The detail page still builds with every tab hidden.
        let _ = app.view();
    }
    #[test]
    fn invalid_memory_is_not_sent_to_persistence() {
        let (mut app, requests) = isolated_app();
        app.memory = "12".into();
        let _ = app.update(Message::SaveSettings);
        assert!(requests.try_recv().is_err());
        assert!(app.notice.is_some());
        app.memory = "2048".into();
        let _ = app.update(Message::SaveSettings);
        assert!(
            matches!(requests.try_recv().unwrap(), backend::Request::Save(prefs) if prefs.memory_mb == 2048)
        );
    }
    #[test]
    fn extended_flows_preserve_form_and_reject_stale_source_reviews() {
        let (mut app, _requests) = isolated_app();
        app.name = "Keep this name".into();
        app.dialog = Some(Dialog::Create);
        let _ = app.update(Message::Open(Dialog::Versions));
        let _ = app.update(Message::ChooseGame("1.21.1".into()));
        assert_eq!(app.name, "Keep this name");
        assert_eq!(app.game, "1.21.1");
        let _ = app.update(Message::InspectImport);
        let id = app.import_id;
        let _ = app.update(Message::CloseDialog);
        app.receive(backend::Event::ImportReview(
            id,
            Ok(model::ImportReview {
                source: "/tmp/source".into(),
                name: "Late".into(),
                game: "1.20.1".into(),
                loader: Loader::Vanilla,
                files: 1,
                bytes: 1,
                optional: vec![],
                notes: vec![],
                copy_supported: true,
                archive_sha1: None,
            }),
        ));
        assert!(app.dialog.is_none());
        assert!(app.import_review.is_none());
    }
    #[test]
    fn offline_accounts_persist_and_reach_the_launch_request() {
        let (mut app, requests) = isolated_app();
        let _ = app.update(Message::AccountName("Quiet".into()));
        let _ = app.update(Message::AccountAdd);
        assert_eq!(app.preferences.offline_accounts.len(), 1);
        let id = app.preferences.offline_accounts[0].id.clone();
        assert_eq!(app.preferences.active_account.as_deref(), Some(id.as_str()));
        // An identity the interface shows as active must survive a restart.
        assert!(matches!(
            requests.try_recv().unwrap(),
            backend::Request::Save(_)
        ));
        // A duplicate name would derive one id twice, so it is refused and saves nothing.
        let _ = app.update(Message::AccountName("Quiet".into()));
        let _ = app.update(Message::AccountAdd);
        assert_eq!(app.preferences.offline_accounts.len(), 1);
        let _ = app.update(Message::Launch("world-test".into()));
        match requests.try_recv().unwrap() {
            backend::Request::Launch(_, preferences, mode) => {
                assert_eq!(preferences.active_identity().1, "Quiet");
                // The chosen identity travels with the request rather than being
                // re-read from the worker's default.
                assert_eq!(mode, model::LaunchMode::Offline);
            }
            other => panic!("expected a launch carrying the active account, got {other:?}"),
        }
        let _ = app.update(Message::AccountRemove(id));
        assert!(app.preferences.offline_accounts.is_empty());
        assert_eq!(app.preferences.active_account, None);
        assert_eq!(
            app.preferences.active_identity().1,
            app.preferences.player_name
        );
    }
    #[test]
    fn preferences_saved_receipt_does_not_mark_later_edits_as_saved() {
        let (mut app, requests) = isolated_app();
        let _ = app.update(Message::Player("First".into()));
        let _ = app.update(Message::SaveSettings);
        assert!(
            matches!(requests.try_recv().unwrap(),backend::Request::Save(p) if p.player_name=="First")
        );
        let _ = app.update(Message::Player("Later".into()));
        app.receive(backend::Event::Finished(
            "preferences".into(),
            Ok("Saved".into()),
        ));
        assert_eq!(app.saved_preferences.player_name, "First");
        assert_eq!(app.preferences.player_name, "Later");
        let _ = app.update(Message::RevertSettings);
        assert_eq!(app.preferences.player_name, "First");
    }
    #[test]
    fn account_and_preferences_frontends_never_invent_authenticated_state() {
        let (mut app, requests) = isolated_app();
        let _ = app.update(Message::Navigate(Page::Settings));
        assert!(matches!(app.dialog, Some(Dialog::Preferences)));
        assert_eq!(app.page, Page::Home);
        let _ = app.update(Message::Open(Dialog::SignIn));
        let _ = app.update(Message::AuthStart);
        assert!(matches!(
            app.authentication,
            model::Authentication::Requesting
        ));
        assert!(matches!(
            requests.try_recv().unwrap(),
            backend::Request::Authenticate
        ));
        app.receive(backend::Event::Authentication(
            model::Authentication::Failed("No device code was issued".into()),
        ));
        assert!(app.auth_error);
        assert!(app.notice.as_ref().unwrap().contains("No device code"));
        for dialog in [
            Dialog::Versions,
            Dialog::Browser,
            Dialog::ImportReview,
            Dialog::Preferences,
            Dialog::Credits,
            Dialog::Surface(surfaces::Panel::Loader(String::new())),
            Dialog::SignIn,
            Dialog::InstallReview("test".into()),
        ] {
            let _ = app.dialog_view(&dialog);
        }
    }
    #[test]
    fn every_authentication_state_is_backend_driven_and_renderable() {
        let (mut app, requests) = isolated_app();
        for json in [
            r#"{"state":"requesting"}"#,
            r#"{"state":"device_code","code":"TEST-ONLY","url":"https://example.com/verify","expires_in":900}"#,
            r#"{"state":"verifying"}"#,
            r#"{"state":"connected","player":"TestPlayer"}"#,
            r#"{"state":"expired"}"#,
            r#"{"state":"cancelled"}"#,
        ] {
            // Test-only backend events cover states the current live backend cannot emit yet.
            let state: model::Authentication = serde_json::from_str(json).unwrap();
            app.receive(backend::Event::Authentication(state));
            let _ = app.dialog_view(&Dialog::SignIn);
        }
        app.receive(backend::Event::Authentication(
            model::Authentication::Failed("Unavailable".into()),
        ));
        let _ = app.dialog_view(&Dialog::SignIn);
        let _ = app.update(Message::AuthCancel);
        assert!(matches!(
            requests.try_recv().unwrap(),
            backend::Request::CancelAuthentication
        ));
    }
    #[test]
    fn install_review_requires_resolved_files_and_refuses_running_targets() {
        let (mut app, requests) = isolated_app();
        app.instances.push(Instance {
            id: "target".into(),
            name: "Target".into(),
            game_version: "1.20.1".into(),
            loader: Loader::Fabric,
            installed_version: None,
            loader_version: None,
            window: None,
            memory_mb: None,
            java: None,
            custom: Default::default(),
        });
        app.project = Some(project("project"));
        let release:model::Release=serde_json::from_str(r#"{"id":"release","project_id":"project","name":"Test","version_number":"1","version_type":"release","game_versions":["1.20.1"],"loaders":["fabric"],"files":[],"dependencies":[{"dependency_type":"required","project_id":"api"}]}"#).unwrap();
        app.release_id = Some(release.id.clone());
        app.releases.push(release);
        let _ = app.dialog_view(&Dialog::InstallReview("target".into()));
        let _ = app.update(Message::InstallContent("target".into()));
        assert!(requests.try_recv().is_err());
        let _ = app.update(Message::Open(Dialog::InstallReview("target".into())));
        let review_id = app.content_review_id;
        assert!(matches!(
            requests.try_recv().unwrap(),
            backend::Request::ReviewContent(_, _, _, _)
        ));
        let plan = model::ContentPlan {
            instance_id: "target".into(),
            instance_name: "Target".into(),
            project_id: "project".into(),
            release_id: "release".into(),
            game_version: "1.20.1".into(),
            loader: Loader::Fabric,
            files: vec![model::ContentFile {
                url: "https://cdn.modrinth.com/a.jar".into(),
                file_name: "a.jar".into(),
                sha1: "a".repeat(40),
                size: Some(42),
                folder: "mods".into(),
                already_present: false,
            }],
        };
        app.receive(backend::Event::ContentReview(review_id, Ok(plan.clone())));
        app.running.insert("target".into(), "Target".into());
        let _ = app.update(Message::InstallContent("target".into()));
        assert!(requests.try_recv().is_err());
        app.running.clear();
        let _ = app.update(Message::InstallContent("target".into()));
        assert!(
            matches!(requests.try_recv().unwrap(),backend::Request::Content(id,_,reviewed) if id=="target"&&reviewed==plan)
        );
    }
    #[test]
    fn stale_file_reviews_never_enable_confirmation_or_reopen_a_closed_dialog() {
        let (mut app, requests) = isolated_app();
        app.instances.push(Instance {
            id: "target".into(),
            name: "Target".into(),
            game_version: "1.20.1".into(),
            loader: Loader::Fabric,
            installed_version: None,
            loader_version: None,
            window: None,
            memory_mb: None,
            java: None,
            custom: Default::default(),
        });
        app.project = Some(project("project"));
        app.releases.push(serde_json::from_str(r#"{"id":"release","project_id":"project","name":"Test","version_number":"1","version_type":"release","game_versions":["1.20.1"],"loaders":["fabric"],"files":[]}"#).unwrap());
        app.release_id = Some("release".into());
        let _ = app.update(Message::Open(Dialog::InstallReview("target".into())));
        let old = app.content_review_id;
        let _ = app.update(Message::ReviewContent("target".into()));
        let current = app.content_review_id;
        app.receive(backend::Event::ContentReview(
            old,
            Err("stale error".into()),
        ));
        assert!(app.content_review_loading);
        assert!(app.content_review_error.is_none());
        app.receive(backend::Event::ContentReview(
            current,
            Err("No compatible dependency".into()),
        ));
        assert!(!app.content_review_loading);
        assert_eq!(
            app.content_review_error.as_deref(),
            Some("No compatible dependency")
        );
        let _ = app.dialog_view(&Dialog::InstallReview("target".into()));
        let _ = app.update(Message::CloseDialog);
        app.receive(backend::Event::ContentReview(
            current,
            Err("late result".into()),
        ));
        assert!(app.dialog.is_none());
        assert!(app.content_review_error.is_none());
        assert!(
            requests
                .try_iter()
                .all(|r| matches!(r, backend::Request::ReviewContent(_, _, _, _)))
        );
    }
    fn bedrock_copy(channel: palantir_bedrock::Channel) -> palantir_bedrock::BedrockInstall {
        palantir_bedrock::BedrockInstall {
            channel,
            package_name: channel.package_prefix().into(),
            version: "1.21.120".into(),
            install_path: None,
            data_path: None,
        }
    }
    #[test]
    fn edition_switch_keeps_java_collections_and_does_not_call_java_services() {
        let (mut app, requests) = isolated_app();
        app.preferences.add_offline_account("Quiet").unwrap();
        let original = app.preferences.clone();
        app.instances.push(Instance {
            id: "java".into(),
            name: "Java collection".into(),
            game_version: "1.20.1".into(),
            loader: Loader::Fabric,
            loader_version: None,
            window: None,
            memory_mb: None,
            java: None,
            custom: Default::default(),
            installed_version: None,
        });
        let _ = app.update(Message::Edition(Edition::Bedrock));
        assert!(matches!(
            requests.try_recv().unwrap(),
            backend::Request::BedrockDetect
        ));
        let _ = app.update(Message::BedrockRefresh);
        let _ = app.update(Message::Navigate(Page::Discover));
        let _ = app.dialog_view(&Dialog::Account);
        let _ = app.dialog_view(&Dialog::SignIn);
        assert!(requests.try_recv().is_err());
        assert_eq!(app.preferences, original);
        assert_eq!(app.instances[0].id, "java");
        app.receive(backend::Event::BedrockInstalls(Ok(vec![])));
        let _ = app.update(Message::Edition(Edition::Java));
        assert_eq!(app.page, Page::Home);
        assert_eq!(app.edition, Edition::Java);
        assert_eq!(app.instances.len(), 1);
        assert!(requests.try_recv().is_err());
    }
    #[test]
    fn bedrock_actions_require_detected_copies_and_never_claim_a_running_session() {
        use palantir_bedrock::Channel;
        let (mut app, requests) = isolated_app();
        app.edition = Edition::Bedrock;
        let _ = app.update(Message::BedrockLaunch(Channel::Release));
        assert!(requests.try_recv().is_err());
        app.receive(backend::Event::BedrockInstalls(Ok(vec![
            bedrock_copy(Channel::Release),
            bedrock_copy(Channel::Preview),
        ])));
        let _ = app.update(Message::BedrockLaunch(Channel::Release));
        let _ = app.update(Message::BedrockLaunch(Channel::Release));
        assert!(matches!(
            requests.try_recv().unwrap(),
            backend::Request::BedrockLaunch(Channel::Release)
        ));
        assert!(requests.try_recv().is_err());
        app.receive(backend::Event::Finished(
            "bedrock-launch".into(),
            Ok("Windows was asked to open Bedrock".into()),
        ));
        assert!(app.running.is_empty());
        assert!(!app.pending.contains_key("bedrock-launch"));
        let _ = app.update(Message::BedrockInspect(Channel::Release));
        let _ = app.update(Message::BedrockInspect(Channel::Preview));
        app.receive(backend::Event::BedrockInventory(
            Channel::Release,
            Err("stale result".into()),
        ));
        assert!(app.bedrock.reading);
        assert!(app.bedrock.files_error.is_none());
        app.receive(backend::Event::BedrockInventory(
            Channel::Preview,
            Err("No data detected".into()),
        ));
        assert!(!app.bedrock.reading);
        assert_eq!(app.bedrock.files_error.as_deref(), Some("No data detected"));
        app.receive(backend::Event::BedrockInstalls(Err(
            "Windows unavailable".into()
        )));
        assert!(app.bedrock.installs.is_empty());
        assert!(app.bedrock.selected.is_none());
    }
    #[test]
    fn bedrock_surfaces_render_in_both_layouts_and_all_densities() {
        use palantir_bedrock::Channel;
        let (mut app, _requests) = isolated_app();
        app.edition = Edition::Bedrock;
        for width in [840.0, 1280.0] {
            app.width = width;
            for scale in Scale::ALL {
                app.preferences.scale = scale;
                for result in [
                    Ok(vec![]),
                    Err("Windows only".into()),
                    Ok(vec![
                        bedrock_copy(Channel::Release),
                        bedrock_copy(Channel::Preview),
                    ]),
                ] {
                    app.receive(backend::Event::BedrockInstalls(result));
                    let _ = app.view();
                    app.bedrock.selected = Some(Channel::Release);
                    app.bedrock.entries = vec![bedrock::Entry {
                        name: "world-id".into(),
                        folder: "minecraftWorlds".into(),
                    }];
                    let _ = app.view();
                    for dialog in [Dialog::Account, Dialog::SignIn, Dialog::Preferences] {
                        app.dialog = Some(dialog);
                        let _ = app.view();
                    }
                    app.dialog = None;
                }
            }
        }
    }
    #[test]
    fn switching_editions_reverts_unsaved_preferences() {
        let (mut app, _requests) = isolated_app();
        app.dialog = Some(Dialog::Preferences);
        app.preferences.player_name = "Draft".into();
        let _ = app.update(Message::Edition(Edition::Bedrock));
        assert_eq!(app.preferences, app.saved_preferences);
        assert!(app.dialog.is_none());
    }
    #[test]
    fn local_review_rejects_stale_results_and_requires_closed_game_confirmation() {
        let (mut app, requests) = isolated_app();
        app.instances.push(Instance {
            id: "local".into(),
            name: "Local".into(),
            game_version: "1.20.1".into(),
            loader: Loader::Vanilla,
            loader_version: None,
            window: None,
            memory_mb: None,
            java: None,
            custom: Default::default(),
            installed_version: None,
        });
        app.draft.source = "/test/file.zip".into();
        let _ = app.update(Message::Open(Dialog::Surface(
            surfaces::Panel::AdditionReview("local".into()),
        )));
        let _ = app.update(Message::ReviewLocal);
        let (revision, input) = match requests.try_recv().unwrap() {
            backend::Request::ReviewLocal(revision, input) => (revision, input),
            _ => panic!("Expected local review"),
        };
        let plan = local::Plan {
            input,
            root: "/test".into(),
            source: "/test/file.zip".into(),
            destination: "/test/mods/file.zip".into(),
            label: "Test".into(),
            files: vec![],
            bytes: 0,
            expanded_files: vec![],
            pack: None,
            bundle: None,
            activation: None,
        };
        app.receive(backend::Event::LocalReview(revision, Ok(plan.clone())));
        let _ = app.update(Message::ApplyLocal);
        assert!(requests.try_recv().is_err());
        let _ = app.update(Message::LocalClosed(true));
        let _ = app.update(Message::ApplyLocal);
        let _ = app.update(Message::ApplyLocal);
        assert!(matches!(
            requests.try_recv().unwrap(),
            backend::Request::ApplyLocal(_)
        ));
        assert!(requests.try_recv().is_err());
        app.pending.clear();
        let _ = app.update(Message::ReviewLocal);
        let stale_revision = app.local.revision;
        let _ = app.update(Message::Draft(surfaces::Edit::Source(
            "/changed.zip".into(),
        )));
        app.receive(backend::Event::LocalReview(
            stale_revision,
            Ok(plan.clone()),
        ));
        assert!(app.local.plan.is_none());
        assert!(!app.local.closed_game);
        let _ = app.update(Message::CloseDialog);
        app.receive(backend::Event::LocalReview(stale_revision, Ok(plan)));
        assert!(app.dialog.is_none());
        assert!(app.local.plan.is_none());
    }
    #[test]
    fn reviewed_operation_receipt_is_shown_in_the_open_review_dialog() {
        let (mut app, requests) = isolated_app();
        app.instances.push(Instance {
            id: "local".into(),
            name: "Local".into(),
            game_version: "1.20.1".into(),
            loader: Loader::Vanilla,
            loader_version: None,
            window: None,
            memory_mb: None,
            java: None,
            custom: Default::default(),
            installed_version: None,
        });
        app.draft.source = "/test/file.zip".into();
        let _ = app.update(Message::Open(Dialog::Surface(
            surfaces::Panel::AdditionReview("local".into()),
        )));
        let _ = app.update(Message::ReviewLocal);
        let (revision, input) = match requests.try_recv().unwrap() {
            backend::Request::ReviewLocal(revision, input) => (revision, input),
            _ => panic!("Expected local review"),
        };
        let plan = local::Plan {
            input,
            root: "/test".into(),
            source: "/test/file.zip".into(),
            destination: "/test/mods/file.zip".into(),
            label: "Test".into(),
            files: vec![],
            bytes: 0,
            expanded_files: vec![],
            pack: None,
            bundle: None,
            activation: None,
        };
        app.receive(backend::Event::LocalReview(revision, Ok(plan)));
        let _ = app.update(Message::LocalClosed(true));
        let _ = app.update(Message::ApplyLocal);
        assert!(matches!(
            requests.try_recv().unwrap(),
            backend::Request::ApplyLocal(_)
        ));
        app.receive(backend::Event::LocalApplied(
            "local".into(),
            Ok("Saved 1 files to /test/out.zip".into()),
        ));
        assert!(
            matches!(&app.local.outcome, Some(Ok(note)) if note.contains("Saved 1 files")),
            "the receipt stays in the review state: {:?}",
            app.local.outcome
        );
        assert!(app.notice.as_deref().unwrap().contains("Saved 1 files"));
        assert!(
            matches!(
                app.dialog,
                Some(Dialog::Surface(surfaces::Panel::AdditionReview(_)))
            ),
            "the review dialog stays open on the receipt"
        );
        // A new review replaces the receipt instead of leaving it stale.
        let _ = app.update(Message::ReviewLocal);
        assert!(app.local.outcome.is_none());
        assert!(matches!(
            requests.try_recv().unwrap(),
            backend::Request::ReviewLocal(_, _)
        ));
    }
    #[test]
    fn catalogue_pack_results_need_the_same_release_and_an_open_review() {
        let (mut app, requests) = isolated_app();
        app.project = Some(model::Project {
            project_type: "modpack".into(),
            ..project("pack-project")
        });
        app.releases.push(serde_json::from_value(serde_json::json!({"id":"pack-release","project_id":"pack-project","name":"Pack","version_number":"1","version_type":"release","game_versions":["1.20.1"],"loaders":["fabric"],"files":[]})).unwrap());
        app.release_id = Some("pack-release".into());
        let _ = app.update(Message::Open(Dialog::PackSetup));
        let _ = app.update(Message::FetchPack);
        let _ = app.update(Message::FetchPack);
        let revision = app.import_id;
        assert!(matches!(
            requests.try_recv().unwrap(),
            backend::Request::FetchPack(_, _, _)
        ));
        assert!(requests.try_recv().is_err());
        let review = model::ImportReview {
            source: "/cache/archive.mrpack".into(),
            name: "Pack".into(),
            game: "1.20.1".into(),
            loader: Loader::Fabric,
            files: 1,
            bytes: 4,
            optional: vec!["mods/optional.jar".into()],
            notes: vec![],
            copy_supported: false,
            archive_sha1: Some("a".repeat(40)),
        };
        app.receive(backend::Event::FetchedPack(
            revision,
            "pack-project".into(),
            "pack-release".into(),
            Ok(review.clone()),
        ));
        assert!(matches!(app.dialog, Some(Dialog::ImportReview)));
        assert!(app.optional_files.is_empty());
        assert_eq!(app.import_path, review.source);
        let _ = app.update(Message::Import);
        assert!(
            matches!(requests.try_recv().unwrap(), backend::Request::Import(_, _, _, _, Some(choice), Some(hash)) if choice.is_empty() && hash == "a".repeat(40))
        );
        let _ = app.update(Message::Open(Dialog::PackSetup));
        let _ = app.update(Message::FetchPack);
        let revision = app.import_id;
        let _ = app.update(Message::CloseDialog);
        app.receive(backend::Event::FetchedPack(
            revision,
            "pack-project".into(),
            "pack-release".into(),
            Ok(review),
        ));
        assert!(app.dialog.is_none());
    }
    #[test]
    fn authored_assets_are_embedded() {
        assert!(include_str!("../assets/portal.svg").contains("wayfinder"));
        assert!(include_str!("../assets/collection.svg").contains("original artwork"));
    }
}
