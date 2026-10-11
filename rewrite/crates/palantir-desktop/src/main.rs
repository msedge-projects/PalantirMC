#![deny(clippy::unwrap_used, clippy::expect_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod backend;
mod controls;
mod design;
mod files;
mod flows;
mod locale;
mod modal;
mod model;

use design::input;
use design::{Button, label, muted, subtitle, title};
use iced::widget::{Space, button, column, container, pick_list, row, scrollable, text_input};
use iced::{Application, Command, Element, Length, Settings, Subscription, Theme, executor};
use locale::Key;
use model::{
    Accent, Instance, Loader, Preferences, Project, ProjectDetail, ProjectType, Scale, Sort,
};
use std::collections::{BTreeMap, VecDeque};
use std::sync::mpsc;
use std::time::Duration;

fn main() -> iced::Result {
    App::run(Settings {
        window: iced::window::Settings {
            size: iced::Size::new(1280.0, 820.0),
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
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    Appearance,
    Language,
    Java,
    Resources,
    Privacy,
    Behaviour,
}
impl SettingsTab {
    const ALL: [Self; 6] = [
        Self::Appearance,
        Self::Language,
        Self::Java,
        Self::Resources,
        Self::Privacy,
        Self::Behaviour,
    ];
    fn name(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Language => "Language",
            Self::Java => "Java & player",
            Self::Resources => "Resources",
            Self::Privacy => "Privacy",
            Self::Behaviour => "Behaviour",
        }
    }
}
#[derive(Debug, Clone)]
enum Dialog {
    Create,
    Import,
    Delete(String),
    Rename(String),
    Install,
    Account,
    SignIn,
    Versions,
    Browser,
    ImportReview,
    Preferences,
    Credits,
    LoaderStatus,
    PackSetup,
    RemoveContent(String, String, String),
    InstallReview(String),
}
#[derive(Debug, Clone)]
enum Message {
    Navigate(Page),
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
    project: Option<Project>,
    detail: Option<ProjectDetail>,
    detail_loading: bool,
    settings_tab: SettingsTab,
    memory: String,
    root: String,
    width: f32,
    pending: BTreeMap<String, String>,
    running: BTreeMap<String, String>,
    logs: BTreeMap<String, VecDeque<String>>,
    notice: Option<String>,
    history: VecDeque<String>,
    saved_preferences: Preferences,
    pending_preferences: Option<Preferences>,
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
    optional_files: Vec<String>,
    browser_path: String,
    directory: Vec<model::DirectoryEntry>,
    browse_loading: bool,
    project_tab: ProjectTab,
    releases: Vec<model::Release>,
    releases_loading: bool,
    release_id: Option<String>,
    auth_error: bool,
    authentication: model::Authentication,
}
impl Application for App {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();
    fn new(_: ()) -> (Self, Command<Message>) {
        let (worker, events) = backend::start();
        (Self::with_worker(worker, events), Command::none())
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
            Message::Navigate(Page::Settings) => {
                self.notice = None;
                self.dialog = Some(Dialog::Preferences);
            }
            Message::Navigate(page) => {
                self.page = page;
                self.selected = None;
                self.dialog = None;
                if page == Page::Discover && self.results.is_empty() && !self.searching {
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
                            break;
                        }
                    }
                }
            }
            Message::CloseDialog => {
                if matches!(self.dialog, Some(Dialog::Preferences)) {
                    self.preferences = self.saved_preferences.clone();
                    self.memory = self.preferences.memory_mb.to_string();
                }
                self.dialog = None;
                self.import_id += 1;
                self.import_loading = false;
            }
            Message::Open(dialog) => {
                self.notice = None;
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
                let focus = matches!(dialog, Dialog::Create | Dialog::Import | Dialog::Rename(_));
                self.dialog = Some(dialog);
                if focus {
                    return text_input::focus(text_input::Id::new("dialog-first"));
                }
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
                if matches!(value, Loader::Fabric | Loader::Quilt) {
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
                    .is_some_and(|r| r.source == self.import_path && r.copy_supported)
                {
                    self.note("Inspect and review this source before copying it.".into());
                    return Command::none();
                }
                if self.pending.contains_key("import") {
                    return Command::none();
                }
                self.pending
                    .insert("import".into(), "Copying a local collection…".into());
                self.send(backend::Request::Import(
                    self.import_path.clone(),
                    self.name.clone(),
                    self.game.clone(),
                    self.loader,
                ));
            }
            Message::Rename(id) => {
                self.send(backend::Request::Rename(id, self.name.clone()));
            }
            Message::Delete(id) => {
                self.dialog = None;
                self.send(backend::Request::Delete(id));
            }
            Message::Select(id) => {
                self.selected = Some(id.clone());
                self.page = Page::Library;
                self.instance_tab = InstanceTab::Overview;
                self.inventory.clear();
                self.inventory_loading = true;
                self.send(backend::Request::Inventory(id));
            }
            Message::InstanceTab(tab) => {
                self.instance_tab = tab;
                if let Some(id) = self.selected.clone() {
                    self.inventory_loading = true;
                    self.send(backend::Request::Inventory(id));
                }
            }
            Message::Launch(id) => {
                self.pending
                    .insert(id.clone(), "Preparing game files…".into());
                self.send(backend::Request::Launch(id, self.preferences.clone()));
            }
            Message::Stop(id) => self.send(backend::Request::Stop(id)),
            Message::OpenFolder(id, folder) => self.send(backend::Request::OpenFolder(id, folder)),
            Message::ToggleMod(id, name) => self.send(backend::Request::ToggleMod(id, name)),
            Message::Query(value) => self.query = value,
            Message::FilterGame(value) => self.filter_game = value,
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
            Message::InstallContent(id) => {
                if self.running.contains_key(&id) || self.pending.contains_key(&id) {
                    self.note(
                        "Stop the session and wait for its active task before installing.".into(),
                    );
                    return Command::none();
                }
                if let Some(project) = self.project.clone() {
                    let eligible = self.selected_release().is_some_and(|r| {
                        r.project_id == project.project_id
                            && !r.required_dependencies()
                            && self
                                .instances
                                .iter()
                                .find(|i| i.id == id)
                                .is_some_and(|i| r.compatible(i, &project.project_type))
                    });
                    if !eligible || project.project_type == "modpack" {
                        self.note("Review a compatible release without unresolved backend requirements before installing.".into());
                        return Command::none();
                    }
                    self.pending
                        .insert(id.clone(), "Resolving a compatible release…".into());
                    self.send(backend::Request::Content(
                        id,
                        project,
                        self.release_id.clone(),
                    ));
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
                    self.note("Offline account added and selected.".into());
                    // Save immediately: an identity the interface already shows as
                    // active must not disappear on the next start.
                    self.send(backend::Request::Save(self.preferences.clone()));
                }
                Err(error) => self.note(error),
            },
            Message::AccountSelect(id) => {
                if let Err(error) = self.preferences.select_offline_account(&id) {
                    self.note(error);
                } else {
                    self.send(backend::Request::Save(self.preferences.clone()));
                }
            }
            Message::AccountRemove(id) => {
                if let Err(error) = self.preferences.remove_offline_account(&id) {
                    self.note(error);
                } else {
                    self.note("Offline account removed.".into());
                    self.send(backend::Request::Save(self.preferences.clone()));
                }
            }
            Message::Player(value) => self.preferences.player_name = value,
            Message::Java(value) => self.preferences.java = value,
            Message::Memory(value) => self.memory = value,
            Message::Accent(value) => self.preferences.accent = value,
            Message::Scale(value) => self.preferences.scale = value,
            Message::Activity(value) => self.preferences.show_activity = value,
            Message::SaveSettings if self.pending_preferences.is_some() => {}
            Message::SaveSettings => match self.memory.parse::<u32>() {
                Ok(memory) if (512..=32768).contains(&memory) => {
                    self.preferences.memory_mb = memory;
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
                        self.pending_preferences = Some(self.preferences.clone());
                        self.send(backend::Request::Save(self.preferences.clone()));
                    }
                }
                _ => self.notice = Some("Use 512–32768 MiB for the memory budget.".into()),
            },
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
                if matches!(self.loader, Loader::Fabric | Loader::Quilt) {
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
                self.query.clear();
                self.filter_game.clear();
                self.offset = 0;
                self.sort = Sort::Relevance;
                self.search();
            }
            Message::OpenUrl(url) => self.send(backend::Request::OpenUrl(url)),
            Message::ResetSettings => {
                self.preferences = Preferences::default();
                self.memory = self.preferences.memory_mb.to_string();
            }
            Message::RevertSettings => {
                self.preferences = self.saved_preferences.clone();
                self.memory = self.preferences.memory_mb.to_string();
                self.notice = None;
            }
            Message::RememberPage(value) => self.preferences.remember_page = value,
            Message::CopyDiagnostics => {
                return iced::clipboard::write(format!(
                    "PalantirMC {}\nOS: {}\nData: {}\nCollections: {}\nWorker: {}\n{}",
                    env!("CARGO_PKG_VERSION"),
                    std::env::consts::OS,
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
        let mut nav = column![
            row![
                design::mark(38.0),
                column![subtitle("PALANTIR"), muted("FIELD NOTES / WORLDS").size(10)].spacing(3)
            ]
            .spacing(10),
            Space::with_height(22)
        ]
        .spacing(8);
        for (page, name, icon) in [
            (Page::Home, Key::Home, "home"),
            (Page::Library, Key::Library, "library"),
            (Page::Discover, Key::Discover, "discover"),
            (Page::Settings, Key::Settings, "settings"),
            (Page::About, Key::About, "about"),
        ] {
            let message = Message::Navigate(page);
            nav = nav.push(controls::content(
                button(
                    row![
                        design::icon(icon, self.page == page),
                        label(locale::message(name, &self.preferences.language))
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
            .push(Space::with_height(Length::Fill))
            .push(action(
                &format!("{} · offline", self.preferences.active_identity().1),
                Message::Open(Dialog::Account),
                Button::Quiet,
            ))
            .push(muted("Ctrl 1 / 2 / 3 · Esc to return").size(10));
        let sidebar = container(nav)
            .padding(18)
            .width(if compact { 190 } else { 220 })
            .height(Length::Fill)
            .style(design::chrome);
        let mut content = column![
            row![
                muted(match self.page {
                    Page::Home => "FIELD NOTES / HOME",
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
        content = content.push(match self.page {
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
            project: None,
            detail: None,
            detail_loading: false,
            settings_tab: SettingsTab::Appearance,
            memory: "4096".into(),
            root: String::new(),
            width: 1280.0,
            pending: BTreeMap::new(),
            running: BTreeMap::new(),
            logs: BTreeMap::new(),
            notice: None,
            history: VecDeque::new(),
            saved_preferences: Preferences::default(),
            pending_preferences: None,
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
            auth_error: false,
            authentication: model::Authentication::Offline,
        }
    }
    fn receive(&mut self, event: backend::Event) {
        match event {
            backend::Event::Loaded {
                instances,
                preferences,
                root,
            } => {
                self.instances = instances;
                self.memory = preferences.memory_mb.to_string();
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
            backend::Event::Status(id, status) => {
                self.pending.insert(id, status);
            }
            backend::Event::Finished(id, result) => {
                self.pending.remove(&id);
                match result {
                    Ok(note) => {
                        if id == "preferences" {
                            if let Some(saved) = self.pending_preferences.take() {
                                self.saved_preferences = saved;
                            }
                        }
                        self.note(note);
                        let completed_dialog = match &self.dialog {
                            Some(Dialog::Create) => id == "create",
                            Some(Dialog::Import | Dialog::ImportReview) => id == "import",
                            Some(Dialog::Rename(instance)) => id == format!("rename:{instance}"),
                            _ => false,
                        };
                        if completed_dialog {
                            self.dialog = None;
                        }
                    }
                    Err(error) => {
                        if id == "preferences" {
                            self.pending_preferences = None;
                        }
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
    fn note(&mut self, note: String) {
        self.notice = Some(note.clone());
        self.history.push_front(note);
        if self.history.len() > 8 {
            self.history.pop_back();
        }
    }
    fn send(&mut self, request: backend::Request) {
        if self
            .worker
            .as_ref()
            .is_none_or(|worker| worker.send(request).is_err())
        {
            self.searching = false;
            self.inventory_loading = false;
            self.versions_loading = false;
            self.loader_loading = false;
            self.import_loading = false;
            self.browse_loading = false;
            self.releases_loading = false;
            self.detail_loading = false;
            self.pending_preferences = None;
            self.pending.clear();
            self.worker = None;
            self.note("The background service stopped. Restart the launcher.".into());
        }
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
                container(column![label(name), muted(status).size(12)].spacing(6))
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
                row![
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
                ]
                .spacing(10)
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
        card(
            column![
                row![
                    design::mark(32.0),
                    column![
                        subtitle(&item.name),
                        muted(format!("{} / Minecraft {}", item.loader, item.game_version))
                            .size(12)
                    ]
                    .spacing(4)
                    .width(Length::Fill),
                    action("Inspect", Message::Select(item.id.clone()), Button::Quiet)
                ]
                .spacing(14)
                .align_items(iced::Alignment::Center),
                row![muted(status).width(Length::Fill), self.play_button(item)].spacing(12)
            ]
            .spacing(14),
        )
    }
    fn play_button(&self, item: &Instance) -> Element<'_, Message> {
        if !item.loader.desktop_supported() {
            return action(
                "Loader setup required",
                Message::Open(Dialog::LoaderStatus),
                Button::Quiet,
            );
        }
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
                Message::Launch(item.id.clone()),
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
            row![
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
            ]
            .spacing(10)
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
        let mut tabs = row![].spacing(6);
        for tab in InstanceTab::ALL {
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
                self.play_button(item)
            ]
            .spacing(12),
            muted(format!("Minecraft {} · {}", item.game_version, item.loader)),
            tabs
        ]
        .spacing(18);
        let body:Element<'_,Message>=match self.instance_tab{
            InstanceTab::Overview=>column![card(column![subtitle("The essentials"),label(format!("Game version: {}",item.game_version)),label(format!("Loader: {}",item.loader)),label(format!("Installation: {}",item.installed_version.as_deref().unwrap_or("Not installed yet"))),muted("First play downloads the game, libraries, and runtime. Shared files are reused across collections.")].spacing(12)),card(column![subtitle("Your game directory"),muted(format!("{}/instances/{}/game",self.root,item.id)),action("Open game folder",Message::OpenFolder(Some(item.id.clone()),String::new()),Button::Quiet)].spacing(12))].spacing(16).into(),
            InstanceTab::Content=>{
                let mut body=column![subtitle("Additions"),row![action("Find content",Message::Navigate(Page::Discover),Button::Primary),action("Refresh",Message::RefreshInventory,Button::Quiet)].spacing(8),row![action("Mods folder",Message::OpenFolder(Some(item.id.clone()),"mods".into()),Button::Quiet),action("Resource packs",Message::OpenFolder(Some(item.id.clone()),"resourcepacks".into()),Button::Quiet)].spacing(8),action("Shaders folder",Message::OpenFolder(Some(item.id.clone()),"shaderpacks".into()),Button::Quiet),muted("A .disabled suffix keeps a mod on disk without loading it.")].spacing(14);
                if self.inventory_loading{body=body.push(muted("Reading this collection…"));}
                let entries:Vec<_>=self.inventory.iter().filter(|e|["mods","resourcepacks","shaderpacks"].contains(&e.folder.as_str())).collect();
                if entries.is_empty()&&!self.inventory_loading{body=body.push(card(column![design::collection(),subtitle("Room for additions"),muted("Browse Explore, or place your own files in the game folders.")].spacing(12)));}
                for entry in entries{let mut line=row![column![label(&entry.name),muted(format!("{} · {}",entry.folder,byte_size(entry.bytes))).size(12)].spacing(4).width(Length::Fill)];if entry.folder=="mods"&&!entry.directory&&!self.running.contains_key(&item.id)&&!self.pending.contains_key(&item.id){line=line.push(action(if entry.name.ends_with(".disabled"){"Enable"}else{"Disable"},Message::ToggleMod(item.id.clone(),entry.name.clone()),Button::Quiet));}
                if !entry.directory&&!self.running.contains_key(&item.id)&&!self.pending.contains_key(&item.id){line=line.push(action("Remove…",Message::Open(Dialog::RemoveContent(item.id.clone(),entry.folder.clone(),entry.name.clone())),Button::Danger));}body=body.push(card(line.spacing(10)));}body.into()
            },
            InstanceTab::Worlds=>{let mut body=column![row![subtitle("Worlds").width(Length::Fill),action("Open saves",Message::OpenFolder(Some(item.id.clone()),"saves".into()),Button::Quiet)].spacing(10),muted("Back up a world before changing game versions or content. The launcher never edits world data.")].spacing(14);let mut found=false;for entry in self.inventory.iter().filter(|e|e.folder=="saves"&&e.directory){found=true;body=body.push(card(row![design::icon("home",false),label(&entry.name)].spacing(12)));}if !found{body=body.push(muted("No saved worlds in this collection yet."));}body.into()},
            InstanceTab::Logs=>{let output=self.logs.get(&item.id).map(|lines|lines.iter().cloned().collect::<Vec<_>>().join("\n")).unwrap_or_else(||"No session output yet. Start the game to watch its output here.".into());column![row![subtitle("Session journal").width(Length::Fill),action("Copy",Message::CopyLogs,Button::Quiet),action("Clear view",Message::ClearLogs,Button::Quiet)].spacing(8),container(scrollable(label(output).font(iced::Font::MONOSPACE).size(12)).height(330)).padding(16).style(design::well),muted("The last 800 lines are held in the view. Minecraft's saved logs remain in the game folder."),action("Open saved logs",Message::OpenFolder(Some(item.id.clone()),"logs".into()),Button::Quiet)].spacing(14).into()},
            InstanceTab::Manage=>card(column![subtitle("Collection details"),action("Rename collection",Message::Open(Dialog::Rename(item.id.clone())),Button::Quiet),muted("Game version and loader are immutable for an existing collection. Create a separate collection to protect its worlds."),subtitle("Danger zone"),muted("Deleting removes this collection's saves and additions. It does not delete shared downloads."),if self.running.contains_key(&item.id)||self.pending.contains_key(&item.id){button(label("Stop current work before deleting")).padding(12).style(Button::Quiet).into()}else{action("Delete collection…",Message::Open(Dialog::Delete(item.id.clone())),Button::Danger)}].spacing(14)),
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
            row![
                controls::picker(ProjectType::ALL, Some(self.kind), Message::Kind),
                controls::picker(Sort::ALL, Some(self.sort), Message::Sort)
            ]
            .spacing(10),
            row![
                input("Minecraft version (optional)", &self.filter_game)
                    .on_input(Message::FilterGame)
                    .on_submit(Message::Search)
                    .padding(10)
                    .width(220),
                action("Apply filters", Message::Search, Button::Quiet),
                action("Reset", Message::ResetFilters, Button::Quiet)
            ]
            .spacing(10)
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
            page = page.push(card(
                column![
                    row![
                        subtitle(&project.title).width(Length::Fill),
                        action(
                            "Read more",
                            Message::Project(project.clone()),
                            Button::Quiet
                        )
                    ]
                    .spacing(8),
                    muted(format!(
                        "{} · by {} · {} downloads",
                        project.project_type, project.author, project.downloads
                    ))
                    .size(12),
                    label(&project.description)
                ]
                .spacing(10),
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
            SettingsTab::Java=>card(column![subtitle("Runtime & player"),label("Offline player name (fallback)"),input("Player",&self.preferences.player_name).on_input(Message::Player).padding(12),muted("The offline account list under Account status is the primary way to change identity. This name is used only while no account is selected."),label("Java executable"),input("Leave blank for automatic management",&self.preferences.java).on_input(Message::Java).padding(12),muted("Automatic management uses the runtime named by the game's metadata. A custom path is executed directly, never as a shell command."),action("Account status",Message::Open(Dialog::Account),Button::Quiet)].spacing(14)),
            SettingsTab::Resources=>card(column![subtitle("Give the game room"),label("Maximum Java memory (MiB)"),input("4096",&self.memory).on_input(Message::Memory).padding(12),muted("Allowed: 512–32768 MiB. Leave room for the operating system."),label("Shared storage"),muted(&self.root),action("Open data directory",Message::OpenFolder(None,String::new()),Button::Quiet),muted("Downloads share one content store. Deleting a collection does not remove files used by another collection.")].spacing(16)),
            SettingsTab::Privacy=>card(column![subtitle("A small network footprint"),label("No telemetry or background analytics"),muted("Explore talks to the public content API. Installation talks to the game/loader metadata and download services. Their operators can see ordinary request metadata such as your IP address."),muted("Project descriptions are displayed as text; no remote script or embedded browser runs them."),muted("Your preferences and collection descriptors stay in the data directory. This build does not store account access tokens.")].spacing(16)),
            SettingsTab::Behaviour=>card(column![controls::toggle("Remember the last launcher page",self.preferences.remember_page,Message::RememberPage),subtitle("Before anything disappears"),label("Deletion always requires explicit confirmation"),muted("A collection includes saved worlds. That is why the safety dialog cannot be disabled."),controls::toggle("Show activity alongside large windows",self.preferences.show_activity,Message::Activity),muted("Closing the launcher stops games it supervises. Pending installations do not start a game after the window closes."),subtitle("Keyboard guide"),label("Tab / Shift+Tab: move focus\nCtrl+1 / 2 / 3: Home / Collections / Explore\nCtrl+,: Preferences\nEscape: close a dialog or return")].spacing(16)),
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
    fn about(&self) -> Element<'_, Message> {
        column![action("Notices & diagnostics",Message::Open(Dialog::Credits),Button::Quiet),card(column![design::mark(84.0),title("PalantirMC"),subtitle("A notebook for your next world."),muted(format!("Version {} · native desktop",env!("CARGO_PKG_VERSION"))),label("An independently authored interface around a Rust game-launching engine. Asymmetric wayfinder artwork, original terrain illustrations, native system typography, and no borrowed launcher branding.")].spacing(18)),card(column![subtitle("Provenance & credits"),label("Launcher artwork and UI copy are original work recorded in THIRD_PARTY_NOTICES.md. iced is MIT; serde, serde_json, reqwest and sha1 offer MIT / Apache-2.0. zip is MIT."),muted("This launcher is not affiliated with the game or catalogue operators. Their product and project names identify compatible services, not our branding."),muted("Original does not mean legally unprotected. Artwork may itself be copyrightable; similarity and trademark questions require legal review."),muted("Product licence: UNLICENSED placeholder until the owner chooses. No legal-clearance guarantee is made.")].spacing(14))].spacing(18).into()
    }
    fn dialog_view(&self, dialog: &Dialog) -> Element<'_, Message> {
        if let Some(view) = self.extended_dialog(dialog) {
            return view;
        }
        let(header,body):(&str,Element<'_,Message>)=match dialog{
            Dialog::Create=>("A new collection",column![muted("Keep one game version and its additions together."),label("Name"),input("Quiet valley",&self.name).id(text_input::Id::new("dialog-first")).on_input(Message::Name).on_submit(Message::Create).padding(12),label("Minecraft version"),input("1.20.1",&self.game).on_input(Message::Game).padding(12),label("Loader"),pick_list(Loader::ALL,Some(self.loader),Message::Loader).text_shaping(iced::widget::text::Shaping::Advanced).style(design::selector()).padding(12),muted("Forge and NeoForge installation controls are not connected to the desktop yet. Choose a supported loader here."),if self.pending.contains_key("create"){button(label("Creating…")).padding(12).style(Button::Quiet).into()}else{action("Create collection",Message::Create,Button::Primary)}].spacing(12).into()),
            Dialog::Import=>("Bring a collection",column![muted("Choose a game directory or a Prism/MultiMC instance directory. A new copy is created; the original is never moved or changed."),label("Source directory"),input("Absolute directory path",&self.import_path).id(text_input::Id::new("dialog-first")).on_input(Message::ImportPath).padding(12),label("Collection name (optional for Prism)"),input("Imported collection",&self.name).on_input(Message::Name).padding(12),label("Version / loader for plain game directories"),row![input("1.20.1",&self.game).on_input(Message::Game).padding(12),pick_list(Loader::ALL,Some(self.loader),Message::Loader).text_shaping(iced::widget::text::Shaping::Advanced).style(design::selector()).padding(12)].spacing(8),muted("Worlds, mods and configuration are copied. Accounts and authentication files are excluded. Archives and unsupported loaders need a separate importer."),if self.pending.contains_key("import"){button(label("Copying…")).padding(12).style(Button::Quiet).into()}else{action("Copy into a new collection",Message::Import,Button::Primary)}].spacing(12).into()),
            Dialog::Rename(id)=>("Rename this collection",column![input("Collection name",&self.name).id(text_input::Id::new("dialog-first")).on_input(Message::Name).on_submit(Message::Rename(id.clone())).padding(12),muted("Only the display name changes. Files remain in the same place."),action("Save name",Message::Rename(id.clone()),Button::Primary)].spacing(14).into()),
            Dialog::Delete(id)=>("This removes a whole collection",column![subtitle(self.instances.iter().find(|i|&i.id==id).map(|i|i.name.as_str()).unwrap_or("Collection")),muted("Its saved worlds, settings, and additions will be deleted. Shared engine downloads stay. This cannot be undone by the launcher."),action("Delete permanently",Message::Delete(id.clone()),Button::Danger)].spacing(16).into()),
            Dialog::Install=>{let mut choices=column![muted("Choose a stopped collection. Compatibility and hashes are checked before files are installed.")].spacing(12); let mut eligible=0;if let Some(project)=&self.project{choices=choices.push(subtitle(&project.title));if project.project_type=="modpack"{choices=choices.push(muted("Modpack coordination is not available in this build. Pack installations require a new collection and safe dependency/override handling."));}else{for item in &self.instances{if !self.running.contains_key(&item.id)&&!self.pending.contains_key(&item.id) && (project.project_type != "mod" || item.loader != Loader::Vanilla) {eligible+=1;choices=choices.push(action(&format!("{} · {} / {}",item.name,item.game_version,item.loader),Message::InstallContent(item.id.clone()),Button::Primary));}}}}
                if eligible == 0 && self.project.as_ref().is_none_or(|p| p.project_type != "modpack") { choices=choices.push(muted("No eligible stopped collection. Mods need Fabric or Quilt; stop active sessions before installing.")); choices=choices.push(action("Create a collection",Message::Open(Dialog::Create),Button::Quiet)); }("Add to a collection",choices.into())},
            Dialog::Account=>{
                let (_, active_name, _) = self.preferences.active_identity();
                let mut body=column![design::emblem("account"),subtitle(&active_name),label("Offline identity · active"),muted("Offline play does not verify ownership or provide authenticated multiplayer. Play launches with the identity selected here.")].spacing(16);
                body=body.push(subtitle("Offline accounts"));
                if self.preferences.offline_accounts.is_empty(){body=body.push(muted("No saved accounts yet. Play uses the fallback name in Preferences, and the first account added becomes the active one."));}
                for account in &self.preferences.offline_accounts {
                    let active=self.preferences.active_account.as_deref()==Some(account.id.as_str());
                    let detail=if active{"Active · used by Play".to_string()}else{format!("Offline · {}",account.uuid())};
                    let mut line=row![column![label(&account.name),muted(detail).size(12)].spacing(4).width(Length::Fill)];
                    line=line.push(if active{button(label("Selected")).padding([10,14]).style(Button::Quiet).into()}else{action("Use this account",Message::AccountSelect(account.id.clone()),Button::Quiet)});
                    line=line.push(action("Remove",Message::AccountRemove(account.id.clone()),Button::Danger));
                    body=body.push(card(line.spacing(10)));
                }
                body=body.push(row![input("New offline name",&self.account_name).id(text_input::Id::new("dialog-first")).on_input(Message::AccountName).on_submit(Message::AccountAdd).padding(12),action("Add account",Message::AccountAdd,Button::Primary)].spacing(8));
                body=body.push(muted("1–16 letters, digits, or underscores, and each name only once. Adding an account selects it and saves the list."));
                body=body.push(action("Edit the fallback name",Message::EditPlayer,Button::Quiet));
                body=body.push(subtitle("Microsoft account"));
                body=body.push(muted("Device-code sign-in requires the registered authentication backend. The setup screen explains its availability before requesting any code."));
                body=body.push(action("Sign in with Microsoft…",Message::Open(Dialog::SignIn),Button::Primary));
                ("Your player identity",body.into())
            },
            _ => ("Launcher", column![muted("Return to the launcher."), action("Close", Message::CloseDialog, Button::Quiet)].into()),
        };
        let mut content = column![
            row![
                subtitle(header).width(Length::Fill),
                action("Close", Message::CloseDialog, Button::Quiet)
            ]
            .spacing(10)
        ]
        .spacing(18);
        if let Some(notice) = &self.notice {
            content = content.push(container(label(notice)).padding(12).style(design::well));
        }
        content = content.push(scrollable(body).height(Length::Shrink));
        container(content)
            .padding(24)
            .width(560)
            .max_height(560)
            .style(design::panel)
            .into()
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
    if bytes >= 1024 * 1024 {
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
        assert_eq!(SettingsTab::ALL.len(), 6);
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
                Dialog::Delete("test".into()),
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
            backend::Request::Launch(_, preferences) => {
                assert_eq!(preferences.active_identity().1, "Quiet")
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
            Dialog::LoaderStatus,
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
    fn install_review_never_submits_unresolved_dependencies_or_running_targets() {
        let (mut app, requests) = isolated_app();
        app.instances.push(Instance {
            id: "target".into(),
            name: "Target".into(),
            game_version: "1.20.1".into(),
            loader: Loader::Fabric,
            installed_version: None,
            loader_version: None,
        });
        app.project = Some(project("project"));
        let release:model::Release=serde_json::from_str(r#"{"id":"release","project_id":"project","name":"Test","version_number":"1","version_type":"release","game_versions":["1.20.1"],"loaders":["fabric"],"files":[],"dependencies":[{"dependency_type":"required","project_id":"api"}]}"#).unwrap();
        app.release_id = Some(release.id.clone());
        app.releases.push(release);
        let _ = app.dialog_view(&Dialog::InstallReview("target".into()));
        let _ = app.update(Message::InstallContent("target".into()));
        assert!(requests.try_recv().is_err());
        app.releases[0].dependencies.clear();
        app.running.insert("target".into(), "Target".into());
        let _ = app.update(Message::InstallContent("target".into()));
        assert!(requests.try_recv().is_err());
        app.running.clear();
        let _ = app.update(Message::InstallContent("target".into()));
        assert!(
            matches!(requests.try_recv().unwrap(),backend::Request::Content(id,_,Some(release)) if id=="target"&&release=="release")
        );
    }
    #[test]
    fn authored_assets_are_embedded() {
        assert!(include_str!("../assets/portal.svg").contains("wayfinder"));
        assert!(include_str!("../assets/collection.svg").contains("original artwork"));
    }
}
