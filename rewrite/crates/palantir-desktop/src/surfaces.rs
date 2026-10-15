//! Outer-layer flows. Local file changes cross a worker-backed review boundary;
//! unsupported operations remain explicit rather than reporting fake success.
use crate::design::{self, Button, label, muted, subtitle};
use crate::model::Edition;
use crate::{App, Dialog, Message, action, card};
use iced::widget::{column, container, row, scrollable};
use iced::{Element, Length};

/// The launch identity picker is defined in the model, not here: the worker
/// has to be told which identity to launch with, and a copy of the enum in the
/// UI layer would be a second source of truth for what those two modes mean.
pub use crate::model::LaunchMode;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldOperation {
    Backup,
    Export,
}
impl WorldOperation {
    pub const ALL: [Self; 2] = [Self::Backup, Self::Export];
}
impl std::fmt::Display for WorldOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Backup => "Backup",
            Self::Export => "Export",
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdditionKind {
    Mod,
    Resource,
    Shader,
    BedrockPack,
}
impl AdditionKind {
    pub const JAVA: [Self; 3] = [Self::Mod, Self::Resource, Self::Shader];
}
impl std::fmt::Display for AdditionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Mod => "Mod",
            Self::Resource => "Resource pack",
            Self::Shader => "Shader",
            Self::BedrockPack => "Bedrock add-on",
        })
    }
}
#[derive(Debug, Clone)]
pub enum Panel {
    Help,
    Launch(String),
    World(String, String),
    WorldReview(String, String),
    Addition(String),
    AdditionReview(String),
    PackReview,
    Loader(String),
    Task(String),
    RemoveAccount(String),
    BedrockAccess,
}
#[derive(Debug, Clone)]
pub enum Edit {
    Source(String),
    Output(String),
    WorldOperation(WorldOperation),
    AdditionKind(AdditionKind),
    LaunchMode(LaunchMode),
}
#[derive(Debug)]
pub struct Draft {
    pub source: String,
    pub output: String,
    pub world_operation: WorldOperation,
    pub addition_kind: AdditionKind,
    pub launch_mode: LaunchMode,
}
impl Default for Draft {
    fn default() -> Self {
        Self {
            source: String::new(),
            output: String::new(),
            world_operation: WorldOperation::Backup,
            addition_kind: AdditionKind::Mod,
            launch_mode: LaunchMode::Offline,
        }
    }
}
impl Draft {
    pub fn edit(&mut self, edit: Edit) {
        match edit {
            Edit::Source(v) => self.source = v,
            Edit::Output(v) => self.output = v,
            Edit::WorldOperation(v) => self.world_operation = v,
            Edit::AdditionKind(v) => self.addition_kind = v,
            Edit::LaunchMode(v) => self.launch_mode = v,
        }
    }
    pub fn path_valid(value: &str) -> bool {
        !value.trim().is_empty() && !value.chars().any(char::is_control)
    }
}

pub fn disabled<'a>(caption: &str) -> Element<'a, Message> {
    iced::widget::button(label(caption))
        .padding([10, 14])
        .style(Button::Quiet)
        .into()
}
pub fn field<'a>(
    caption: &str,
    placeholder: &str,
    value: &str,
    change: fn(String) -> Message,
) -> Element<'a, Message> {
    column![
        label(caption),
        design::input(placeholder, value)
            .on_input(change)
            .padding(12)
    ]
    .spacing(7)
    .into()
}
pub fn unavailable<'a>(operation: &str, reason: &str) -> Element<'a, Message> {
    card(column![label(format!("{operation} · engine connection pending")), muted(reason), muted("Your choices are a session-only draft. No game files, credentials, or system settings are changed by this review.")].spacing(8))
}

impl App {
    pub(crate) fn narrow(&self) -> bool {
        self.width / self.preferences.scale.factor() < 1100.0
    }
    pub(crate) fn actions<'a>(&self, items: Vec<Element<'a, Message>>) -> Element<'a, Message> {
        if self.narrow() {
            column(items).spacing(8).into()
        } else {
            row(items).spacing(8).into()
        }
    }
    pub(crate) fn panel_shell<'a>(
        &self,
        heading: &str,
        body: Element<'a, Message>,
        wide: bool,
    ) -> Element<'a, Message> {
        // iced draws a scrollable's bar over its content's right edge and
        // reserves no space for it, so the pane's rows share a 14px gutter
        // (the bar is 10px wide): the bar never covers a card border, and
        // heading, notice and cards keep one right edge with or without it.
        let gutter = [0.0, 14.0, 0.0, 14.0];
        let mut panel = column![
            row![
                subtitle(heading).width(Length::Fill),
                action("Close", Message::CloseDialog, Button::Quiet)
            ]
            .width(Length::Fill)
            .spacing(10)
            .padding(gutter)
        ]
        .spacing(14);
        if let Some(notice) = &self.notice {
            panel = panel.push(
                container(container(label(notice)).padding(12).style(design::well))
                    .width(Length::Fill)
                    .padding(gutter),
            );
        }
        panel = panel.push(
            scrollable(container(body).width(Length::Fill).padding(gutter))
                .id(scrollable::Id::new("dialog-body"))
                .height(Length::Fill),
        );
        container(panel)
            .padding(20)
            .width(if wide { 850 } else { 610 })
            .height(Length::Fill)
            .max_height(if wide { 700 } else { 600 })
            .style(design::panel)
            .into()
    }
    pub(crate) fn local_review_view(&self) -> Element<'_, Message> {
        let mut body = column![].spacing(12);
        if self.worker.is_none() && !self.gallery {
            return muted(
                "The background service stopped. Restart the launcher before reviewing files.",
            )
            .into();
        }
        if self.local.loading {
            return column![
                muted("Reading and hashing files on the worker…"),
                disabled("Review in progress")
            ]
            .spacing(10)
            .into();
        }
        // The confirmed operation's receipt belongs here: the modal scrim
        // hides the banner behind the dialog, so the outcome must be shown
        // where the user confirmed it.
        if let Some(outcome) = &self.local.outcome {
            body = body.push(card(
                column![
                    label(match outcome {
                        Ok(_) => "Operation complete",
                        Err(_) => "Operation could not finish",
                    }),
                    muted(match outcome {
                        Ok(note) => note,
                        Err(error) => error,
                    }),
                ]
                .spacing(8),
            ));
        }
        if let Some(error) = &self.local.error {
            body = body.push(card(
                column![label("Review could not finish"), muted(error)].spacing(8),
            ));
        }
        if let Some(plan) = &self.local.plan {
            body = body
                .push(label(format!(
                    "Destination: {}",
                    plan.destination.display()
                )))
                .push(muted(format!(
                    "{} source files · {}",
                    plan.files.len(),
                    crate::byte_size(plan.bytes)
                )));
            if let Some(pack) = &plan.pack {
                body = body.push(subtitle(&pack.name)).push(muted(format!(
                    "Pack id: {}\nVersion: {}\nManifest destination: {}",
                    pack.uuid,
                    pack.version,
                    pack.destination.unwrap_or("Unavailable")
                )));
            }
            if !plan.expanded_files.is_empty() {
                let total: u64 = plan.expanded_files.iter().map(|(_, bytes)| bytes).sum();
                body = body.push(label(format!(
                    "{} extracted files · {} expanded",
                    plan.expanded_files.len(),
                    crate::byte_size(total)
                )));
                for (name, bytes) in plan.expanded_files.iter().take(100) {
                    body =
                        body.push(muted(format!("{name} · {}", crate::byte_size(*bytes))).size(12));
                }
                if plan.expanded_files.len() > 100 {
                    body = body.push(muted(
                        "First 100 extracted paths shown; all paths are checked.",
                    ));
                }
            }
            if let Some(bundle) = &plan.bundle {
                for pack in &bundle.packs {
                    body = body.push(card(
                        column![
                            subtitle(&pack.name),
                            muted(format!("UUID: {} · version {}", pack.uuid, pack.version)),
                            label(format!("Destination: {}", pack.destination.display()))
                        ]
                        .spacing(6),
                    ));
                }
                for activation in &bundle.activations {
                    body = body.push(muted(format!(
                        "World list: {} · {} existing → {} reviewed entries",
                        activation.path.display(),
                        activation.before.len(),
                        activation.after.len()
                    )));
                }
            }
            if let Some(activation) = &plan.activation {
                body = body
                    .push(label(format!(
                        "World pack list: {}",
                        activation.path.display()
                    )))
                    .push(muted(format!(
                        "{} existing entries → {} reviewed entries",
                        activation.before.len(),
                        activation.after.len()
                    )));
            }
            for entry in plan.files.iter().take(100) {
                body = body.push(
                    muted(format!(
                        "{} · {}\nSHA-1 {}",
                        entry.relative,
                        crate::byte_size(entry.bytes),
                        entry.sha1
                    ))
                    .size(12),
                );
            }
            if plan.files.len() > 100 {
                body = body.push(muted(
                    "First 100 shown; every file in the total is checked.",
                ));
            }
            let key = plan.input.target().key();
            body = body.push(crate::controls::toggle(
                "I closed the game before copying or changing these files",
                self.local.closed_game,
                Message::LocalClosed,
            ));
            body = body.push(
                if self.pending.contains_key(&key)
                    || self.running.contains_key(&key)
                    || (matches!(plan.input.target(), crate::local::Target::Bedrock(_))
                        && self.pending.contains_key("bedrock-launch"))
                {
                    disabled("Finish the current session or task first")
                } else if self.local.closed_game {
                    action(
                        "Confirm reviewed operation",
                        Message::ApplyLocal,
                        Button::Primary,
                    )
                } else {
                    disabled("Close the game and confirm above")
                },
            );
        }
        body.push(action(
            if self.local.plan.is_some() {
                "Recheck files"
            } else {
                "Resolve and review files"
            },
            Message::ReviewLocal,
            Button::Quiet,
        ))
        .into()
    }
    pub(crate) fn outer_dialog(&self, panel: &Panel) -> Element<'_, Message> {
        let (heading, body): (&str, Element<'_, Message>) = match panel {
            Panel::Help => ("Find your way", column![
                design::emblem("help"), subtitle("One launcher, two editions"),
                card(column![label("Java collections"), muted("Each collection keeps its own game version, loader, worlds and additions. Play downloads missing files; Explore adds compatible content after review.")].spacing(8)),
                card(column![label("Bedrock installed copies"), muted("Windows owns the installation and entitlement. The launcher opens detected copies and shows their player folders. Java accounts do not switch the game's Xbox identity.")].spacing(8)),
                subtitle("Keyboard"), label("Tab / Shift+Tab — next / previous control\nEnter / Space — activate a focused action\nArrow keys — change a focused selector\nEscape — close a dialog or return\nCtrl+1 / 2 / 3 — Home / Collections / Explore\nCtrl+, — preferences\nF1 — this guide"),
                subtitle("When something goes wrong"), muted("Read the action's error, copy diagnostics, and retry after checking the connection or destination. An unavailable action means the screen is ready but the engine feature is not connected."),
                self.actions(vec![action("Preferences", Message::Navigate(crate::Page::Settings), Button::Quiet), action("Copy diagnostics", Message::CopyDiagnostics, Button::Quiet), action("Notices", Message::Open(Dialog::Credits), Button::Quiet)]),
            ].spacing(14).into()),
            Panel::Launch(id) => {
                let item = self.instances.iter().find(|item| &item.id == id);
                let mut body = column![design::emblem("account"), subtitle(item.map(|i| i.name.as_str()).unwrap_or("Collection unavailable")), label("Play as"), crate::controls::picker(LaunchMode::ALL, Some(self.draft.launch_mode), |v| Message::Draft(Edit::LaunchMode(v)))].spacing(14);
                if let Some(item) = item {
                    body = body.push(label(format!("Minecraft {} / {}", item.game_version, item.loader)))
                        .push(muted(format!("Offline identity: {}", self.preferences.active_identity().1)))
                        .push(muted(format!("Memory: {} MiB · Runtime: {}", self.preferences.memory_mb, if self.preferences.java.is_empty() { "Managed automatically" } else { &self.preferences.java })));
                    match self.draft.launch_mode {
                        LaunchMode::Offline => { body = body.push(muted("Offline identity does not provide authenticated multiplayer. Game files and Java are prepared before the session starts."));
                            body = body.push(if self.running.contains_key(id) || self.pending.contains_key(id) { disabled("Finish the current session or task first") } else { action("Play with offline identity", Message::Launch(id.clone()), Button::Primary) }); }
                        LaunchMode::Microsoft => { body = body.push(muted("Play uses the Microsoft session stored when you signed in; the worker refreshes it when it has expired. A missing or refused session stops the launch instead of quietly playing offline."));
                            body = body.push(if self.running.contains_key(id) || self.pending.contains_key(id) { disabled("Finish the current session or task first") } else { action("Play with Microsoft account", Message::Launch(id.clone()), Button::Primary) });
                            body = body.push(action("Verify Microsoft profile", Message::Open(Dialog::SignIn), Button::Quiet)); }
                    }
                } else { body = body.push(muted("Return to Collections and select an existing collection.")); }
                ("Review your session", body.into())
            }
            Panel::World(id, name) => {
                let format = if self.edition == Edition::Bedrock { ".mcworld" } else { ".zip" };
                let body = column![design::emblem("worlds"), subtitle(name), label(format!("Source: {}", if id.is_empty() { "Bedrock player data" } else { "Selected Java collection" })), label("Operation"), crate::controls::picker(WorldOperation::ALL, Some(self.draft.world_operation), |v| Message::Draft(Edit::WorldOperation(v))),
                    field("Destination", &format!("Absolute path ending in {format}"), &self.draft.output, |v| Message::Draft(Edit::Output(v))),
                    muted("Close the game before a snapshot. The complete world is copied without changing the original. Existing outputs are always refused; choose a new filename."),
                    if Draft::path_valid(&self.draft.output) { action("Review world operation", Message::Open(Dialog::Surface(Panel::WorldReview(id.clone(), name.clone()))), Button::Primary) } else { disabled("Choose an output destination") },
                ].spacing(14);
                ("Keep a world safe", body.into())
            }
            Panel::WorldReview(id, name) => ("Review world operation", column![
                action("← Change world options", Message::Open(Dialog::Surface(Panel::World(id.clone(), name.clone()))), Button::Quiet),
                design::emblem("worlds"), subtitle(name),
                label(format!("Operation: {}", self.draft.world_operation)),
                label(format!("Output: {}", self.draft.output)),
                muted("Backup and export both copy the complete world, including settings and pack references. No database is opened or interpreted. The output directory must already exist."),
                self.local_review_view(),
            ].spacing(14).into()),
            Panel::Addition(id) => {
                let kinds = if self.edition == Edition::Bedrock { vec![AdditionKind::BedrockPack] } else { AdditionKind::JAVA.to_vec() };
                ("Bring your own content", column![design::emblem("additions"), label("Content type"), crate::controls::picker(kinds, Some(self.draft.addition_kind), |v| Message::Draft(Edit::AdditionKind(v))),
                    field("Source file", if self.edition == Edition::Bedrock { ".mcpack or .mcaddon path" } else { ".jar or .zip path" }, &self.draft.source, |v| Message::Draft(Edit::Source(v))),
                    muted("Select a local file you trust. Review reads it without installation; confirmation copies a Java file or extracts the reviewed Bedrock packs. Nothing is executed by the launcher."),
                    if Draft::path_valid(&self.draft.source) { action("Review content import", Message::Open(Dialog::Surface(Panel::AdditionReview(id.clone()))), Button::Primary) } else { disabled("Enter a source file") },
                ].spacing(14).into())
            }
            Panel::AdditionReview(id) => {
                let mut body = column![design::emblem("additions"),
                    action("← Change content source", Message::Open(Dialog::Surface(Panel::Addition(id.clone()))), Button::Quiet),
                    label(format!("Type: {}", self.draft.addition_kind)), label(format!("Source: {}", self.draft.source)),
                    muted("Existing destinations are refused. Review checks the source hash and all archive paths; confirmation rechecks the plan. Compatibility and trust remain your responsibility."),
                ].spacing(14);
                if id.is_empty() && self.edition == Edition::Bedrock {
                    let mut worlds = vec!["Do not activate".to_string()];
                    worlds.extend(self.bedrock.worlds.iter().map(|world| world.folder.clone()));
                    body = body.push(label("Enable for one world (optional)"))
                        .push(crate::controls::picker(worlds, Some(self.local.activation_world.clone().unwrap_or_else(|| "Do not activate".into())), Message::ActivationWorld))
                        .push(muted("The selected world's resource/behaviour pack list is reviewed before it changes. Other entries are preserved. Without a world selection, enable the installed pack inside the game."));
                }
                ("Review content import", body.push(self.local_review_view()).into())
            },
            Panel::PackReview => {                let mut body = column![
                    action("← Pack setup", Message::Open(Dialog::PackSetup), Button::Quiet),
                    design::emblem("download"),
                    subtitle(if self.name.trim().is_empty() { "New pack collection" } else { &self.name }), label("Destination: a new, independent Java collection")].spacing(14);
                if let Some(release) = self.selected_release() { body = body.push(label(format!("Release: {} / {}", release.name, release.version_number))).push(muted(format!("Game: {}\nLoaders: {}", release.game_versions.join(", "), release.loaders.join(", ")))); }
                if let Some(review) = &self.import_review { body = body.push(label(format!("Archive: {}", review.source))).push(muted(format!("{} indexed files · {}", review.files, crate::byte_size(review.bytes)))); }
                body = body.push(label(format!("Optional files selected: {}", self.optional_files.len())));
                for path in &self.optional_files { body = body.push(muted(path)); }
                body = body.push(muted("Installing a pack reads the archive's own index, reuses files whose hash already matches, refuses a changed, linked or duplicate destination, and applies the pack's overrides last. No existing collection or world is reused or replaced."));
                let reviewed_archive = self
                    .import_review
                    .as_ref()
                    .is_some_and(|review| review.source == self.import_path);
                body = body.push(if reviewed_archive {
                    if self.pending.contains_key("import") {
                        disabled("Installing the pack archive…")
                    } else if self.name.trim().is_empty() {
                        disabled("Enter a new collection name")
                    } else {
                        action("Install the pack archive", Message::Import, Button::Primary)
                    }
                } else {
                    action("Inspect a local .mrpack…", Message::Open(Dialog::Import), Button::Quiet)
                });
                ("Final pack review", body.into())
            }
            Panel::Loader(id) => {
                let item = self.instances.iter().find(|i| &i.id == id);
                let loader = item.map(|i| i.loader).unwrap_or(self.loader);
                let body = column![design::emblem("download"), subtitle(format!("{loader} setup")), label(format!("Game: {}", item.map(|i| i.game_version.as_str()).unwrap_or(&self.game))), label("Loader release"),
                    label(item.and_then(|i| i.loader_version.as_deref()).unwrap_or("Automatic release selection")),
                    card(column![label("1 / Resolve installer"), muted("Read the loader's release metadata and choose the pinned build."), label("2 / Prepare libraries"), muted("Download and verify the files named by that installer."), label("3 / Run processors"), muted("Execute installer processors with the matching managed Java runtime."), label("4 / Verify output"), muted("Only a complete verified version can become launchable.")].spacing(8)),
                    muted("Play runs this pipeline for every loader: resolve the installer, download and verify the files it names, run its installer processors with the managed Java runtime, then launch. The first Play installs and verifies it; the saved loader pin shown above is the release Play uses."),
                ].spacing(14);
                let body = if item.is_some() {
                    body.push(action("Play this collection", Message::Launch(id.clone()), Button::Primary))
                } else {
                    body.push(disabled("Select an existing collection"))
                };
                ("Review loader setup", body.into())
            }
            Panel::Task(id) => {
                let status = self.pending.get(id).map(String::as_str).unwrap_or("No active task with this identifier");
                let name = self.instances.iter().find(|i| &i.id == id).map(|i| i.name.as_str()).unwrap_or("Launcher operation");
                let mut body = column![design::emblem("download"), subtitle(name), label(status), muted("Task status is supplied by the worker. No percentage or time estimate is invented when the worker only reports a stage."), label(format!("Operation id: {id}")), action("Copy diagnostics", Message::CopyDiagnostics, Button::Quiet)].spacing(14);
                if self.instances.iter().any(|i| &i.id == id) { body = body.push(action("Open collection", Message::Select(id.clone()), Button::Quiet)); }
                body = body.push(muted("This installation worker does not expose cancellation yet. Closing this panel leaves the operation running; do not remove its files manually."));
                ("Operation details", body.into())
            }
            Panel::RemoveAccount(id) => ("Remove this offline identity?", column![design::emblem("account"), subtitle(self.preferences.offline_accounts.iter().find(|a| &a.id == id).map(|a| a.name.as_str()).unwrap_or("Identity no longer exists")), muted("This removes the launcher identity, not a Microsoft account or a saved world. Removing the selected identity returns Play to the fallback name."), action("Remove offline identity", Message::AccountRemove(id.clone()), Button::Danger)].spacing(14).into()),
            Panel::BedrockAccess => ("Bedrock access checklist", column![design::emblem("account"), subtitle("Ownership stays with Windows"), label("1 / Microsoft Store"), muted("Use the Microsoft account that purchased Bedrock. Install the owned copy through the Store or official launcher."), label("2 / Xbox identity"), muted("Open Xbox and the game to confirm the intended player identity. Java device-code verification does not switch it."), label("3 / Preview access"), muted("Preview is a separate channel; access and installation are managed by Microsoft's own apps."), label("4 / Refresh here"), muted("Detect the installed package and open it. A detected installation is not a licence receipt."), unavailable("Entitlement pre-check", "This launcher cannot pre-check or change Store/Xbox account state yet."), action("Refresh installed copies", Message::BedrockRefresh, Button::Quiet)].spacing(14).into()),
        };
        self.panel_shell(heading, body, false)
    }
}
