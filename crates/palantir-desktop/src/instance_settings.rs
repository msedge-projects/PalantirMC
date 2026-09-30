//! An instance's own settings, as a form: read back into controls and written
//! on save. Two tabs, because the two writes are different kinds of thing.
//!
//! Why this exists at all: [`crate::model`] inherited a settings *reader* from
//! `palantir-gui` -- the override-gate semantics Prism keeps (`OverrideMemory`
//! and its neighbours) -- and the note it left behind says the write side
//! belongs to the page that never arrived. This is that page, as a modal,
//! because a modal is where the reference keeps it: `InstanceSettingsModal`.
//!
//! **The two halves, and where the reference keeps each.** The reference's modal
//! has tabs for general, installation, sync overrides and sharing. Its
//! `java-settings.vue` -- the heap, the Java path and the JVM arguments, each
//! behind a `Toggle` -- is drawn *inside* the sync-overrides tab, beside the
//! data-pack and command-history switches; those switches are Modrinth's synced
//! settings, which this launcher does not have (G118), so the Java settings get a
//! tab of their own and its label is this module's word. Its `installation` tab is
//! `components/settings-modal/installation-settings.vue`: the platform, the game
//! version and the loader's build, all three read from the instance's own
//! `mmc-pack.json` and written back to it. That is the second tab here.
//!
//! **Where the installation tab is drawn differently, and why.** The reference
//! draws each of its three choices as a combobox (a `Select` inside
//! `InstallationSettingsLayout`) whose options it filters itself: the game
//! versions by what the loader's own manifest lists for it, and the loader builds
//! by the game version chosen. This kit has no combobox of that shape, so each
//! choice is the chips-plus-search-and-list the creation dialog already draws --
//! and the *filter* is the service's rather than a second document's: the store
//! asks the loader for the builds of the chosen game version
//! ([`crate::store::Store::loader_builds`]) and an empty answer is drawn as the
//! reference's own "no versions available" sentence. What that costs is one
//! visible difference worth knowing: a game version the loader never published
//! for is not hidden from the list up front, it answers with that sentence.
//!
//! **The linked pack, and what of it is still owed.** An instance installed from
//! a Modrinth pack keeps the project and version it came from
//! ([`crate::store::InstanceLink`], a file of its own beside `mmc-pack.json`), and
//! this tab draws the reference's own panel over it: the *Installed modpack* card,
//! named from the service the way the reference names it, *Unlink modpack*, which
//! forgets the link and nothing else, and *Repair instance*, which re-installs the
//! instance's own files with every one already on disk hashed against the digest
//! its metadata publishes ([`crate::launch::repair_instance`]). Two of the
//! reference's four actions are still owed -- *Change version* (its *Swap*) and
//! *Re-install modpack* -- because both re-apply the *pack's* files rather than the
//! launcher's dependencies, and putting a pack into an instance that already exists
//! is a different path from the one that made it. The general and sharing tabs are
//! not built by decision: sharing is
//! `shared-instances.modrinth.com` (G118), and general is name, icon and update
//! channel, which the instance cards' own flows own today.
//!
//! The card is also drawn without the project's picture, which the reference puts
//! beside its title: an icon is a fetch and a decode this kit has no path for on a
//! modal (the user page's avatar is the one place that does it), and a card that
//! names the pack answers the question the card is asked.
//!
//! **What the forms hold and what the files hold.** The controls are strings
//! while they are being typed ([`InstanceSettings`] and [`InstanceInstallation`]
//! are the parsed values), because "2048x" is a keystroke rather than a heap and
//! a form that refused a keystroke would be a form nobody could type in. Save
//! parses; a buffer that is not a number is a sentence beside the fields rather
//! than a written line, and both halves' floors are the store's own refusals
//! because that is where the files are.
//!
//! **What this slice does not promise about a version change.** Changing the game
//! version or the loader writes the profile; the *install* -- the libraries,
//! assets and the loader's own profile -- is what a launch does next, resolving
//! the same components it always resolves. The reference re-runs its install from
//! the modal's own Save (`afterSave`); this one lets the next run pay for it, and
//! says so where the reader is about to press Save.

use iced::widget::{column, container, mouse_area, row, scrollable, text, text_input, Space};
use iced::{mouse::Interaction, Alignment, Background, Border, Element, Length, Padding};

use crate::catalog::LoaderKind;
use crate::icon;
use crate::icons_gen::Glyph;
use crate::page::Load;
use crate::store::{
    GameVersion, InstanceInstallation, InstanceLink, InstanceSettings, LinkedModpack, LoaderBuild,
};
use crate::style::{medium, semibold, INK_CONTRAST, INK_SECONDARY};
use crate::text_gen::Key;
use crate::theme_gen::{self, Ink, Theme as Gen};
use crate::ui::{self, Hovered};

/// A control's stable name on the hover clock: the tab strip's two.
const JAVA_TAB_KEY: &str = "instance-settings:tab:java";
const INSTALLATION_TAB_KEY: &str = "instance-settings:tab:installation";
/// The repeating rows' names, scoped by the value they carry.
const VERSION_ROW: &str = "instance-settings:version";
const BUILD_ROW: &str = "instance-settings:build";
/// The card's own control: the reference's *Unlink modpack*.
const UNLINK_KEY: &str = "instance-settings:unlink";
/// The repair button, which has two names rather than one for the snapshot
/// toggle's reason: its word changes with the state it reports (*Repairing…*
/// while the check runs), and a hover must not carry across a control whose label
/// changed under the pointer.
const REPAIR_KEY: &str = "instance-settings:repair";
const REPAIRING_KEY: &str = "instance-settings:repairing";
/// The game-version list's footer toggle, which has two names rather than one:
/// its word changes with the state it flips, and a hover must not carry across a
/// control whose label changed under the pointer. The creation dialog's picker
/// splits the same row the same way.
const SNAPSHOTS_SHOW_ALL: &str = "instance-settings:snapshots-show-all";
const SNAPSHOTS_HIDE: &str = "instance-settings:snapshots-hide";
/// The toggle row's own vertical padding. It is the creation dialog's row height
/// (`VERSION_ROW_HEIGHT`), so the two version pickers' footers are the same
/// height even though the lists above them are not.
const TOGGLE_PAD: f32 = 12.0;
/// How tall a picker's list is allowed to be: the reference's own options list is
/// bounded the same way (`max-h-*` on a scroll area), so the modal's Save stays
/// on screen with a hundred versions behind it.
const LIST_HEIGHT: f32 = 200.0;

/// Which half of the modal is on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    /// The heap, the Java path and the JVM arguments ([`InstanceSettings`]).
    #[default]
    Java,
    /// The platform, the game version and the loader's build
    /// ([`InstanceInstallation`]).
    Installation,
}

/// What the form can be told.
#[derive(Debug, Clone)]
pub enum Message {
    /// The reader moved to the other half.
    Tab(Tab),
    /// The instance's own Java was switched on or off.
    OverrideJava(bool),
    /// The Java path field changed.
    JavaPath(String),
    /// The instance's own heap was switched on or off.
    OverrideMemory(bool),
    /// The heap floor field changed, in MiB.
    MemoryMin(String),
    /// The heap ceiling field changed, in MiB.
    MemoryMax(String),
    /// The instance's own JVM arguments were switched on or off.
    OverrideJavaArgs(bool),
    /// The JVM arguments field changed.
    JvmArgs(String),
    /// The Java half was asked to save.
    ///
    /// Handled by the shell rather than by [`State::update`], because saving is a
    /// write to a file the modal does not own: the shell is where the store is.
    Save,
    /// A different platform was chosen: vanilla, or one of the four loaders.
    Platform(LoaderKind),
    /// A game version was chosen.
    GameVersion(String),
    /// A loader build was chosen.
    LoaderBuild(String),
    /// The game-version search field changed.
    GameQuery(String),
    /// The loader-build search field changed.
    BuildQuery(String),
    /// Snapshots were shown or hidden in the game-version list.
    ShowSnapshots(bool),
    /// The installation half was asked to save, and is the shell's for
    /// [`Message::Save`]'s reason -- a second file, a second refusal.
    SaveInstallation,
    /// The reader asked to forget the project this instance was installed from.
    ///
    /// The shell's, for [`Message::Save`]'s reason twice over: forgetting a link
    /// is a file the modal does not own, and it is a file the *launcher* owns
    /// rather than the instance.
    Unlink,
    /// The reader asked for the instance's own files to be installed again, with
    /// every one already on disk checked against its published digest.
    ///
    /// The shell's, for [`Message::Save`]'s reason and a larger one: this is the
    /// half of a launch that fetches files ([`crate::launch::repair_instance`]),
    /// which is the longest thing this launcher does that is not a launch, and it
    /// cannot run on the frame thread.
    Repair,
    /// The pointer entered or left one of the form's controls, for the clock
    /// that carries a hover's 150 ms (see [`crate::ui`]).
    ///
    /// The form is not a page, but its controls are drawn by [`ui::button`],
    /// [`ui::tabs`] and this module's own rows, which ask for a crossing like
    /// every other control in the kit -- so the form carries one, and
    /// [`State::update`] hands it to the clock.
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

/// The form: which instance, which tab, and the two halves' values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    /// Which instance, which is what a save writes to.
    pub id: String,
    /// Its name, for the dialog's title.
    pub name: String,
    /// Whether the form has values under it.
    ///
    /// False only for a read that failed: the modal is then the sentence alone,
    /// because a form of zeroes would be this launcher inventing settings for an
    /// instance it could not open.
    pub loaded: bool,
    /// Which half is on screen.
    pub tab: Tab,
    /// Whether the instance's own Java is what a launch uses.
    pub override_java: bool,
    /// The Java path buffer.
    pub java_path: String,
    /// Whether the instance's own heap is what a launch uses.
    pub override_memory: bool,
    /// The heap floor buffer, in MiB.
    pub memory_min: String,
    /// The heap ceiling buffer, in MiB.
    pub memory_max: String,
    /// Whether the instance's own JVM arguments are what a launch uses.
    pub override_java_args: bool,
    /// The JVM arguments buffer.
    pub jvm_args: String,
    /// The platform in force, or the one the reader has chosen instead.
    pub platform: LoaderKind,
    /// The game version in force, or the one chosen instead.
    pub game_version: String,
    /// The loader build in force, or the one chosen instead.
    pub loader_build: String,
    /// Whether snapshot versions are offered in the game-version list.
    pub show_snapshots: bool,
    /// What the reader has typed into the game-version search field.
    pub game_query: String,
    /// The same for the loader-build list.
    pub build_query: String,
    /// Mojang's version list, once the shell has read it.
    pub versions: Load<Vec<GameVersion>>,
    /// The chosen platform's builds for the chosen game version.
    pub builds: Load<Vec<LoaderBuild>>,
    /// The `(platform, game)` pair [`State::builds`] belongs to.
    ///
    /// Kept so that an answer which arrives after the reader moved on -- a slower
    /// game version, or the other platform -- is dropped rather than drawn under
    /// the wrong heading. `None` while nothing has been asked for.
    pub builds_for: Option<(LoaderKind, String)>,
    /// The project this instance was installed from, or the reason the file that
    /// says so could not be read.
    ///
    /// Read by the shell when the modal opens, because it is a disk read and the
    /// modal owns no files: `Idle` is "this instance did not come from a pack",
    /// which is the answer for every instance made or imported by hand.
    pub link: Load<InstanceLink>,
    /// That project and version, named by the service, for the card to draw.
    pub modpack: Load<LinkedModpack>,
    /// What a repair came to, or the reason it could not finish.
    ///
    /// `Loading` *is* the busy state: it draws the button disabled under the
    /// reference's own word for the wait, which is the whole of what a reader is
    /// told while a machine hashes their install. A [`Load`] rather than a flag
    /// beside a sentence because the three states it can be in are this enum's
    /// three, and the section draws each of them differently.
    pub repair: Load<String>,
    /// The last refusal, in the reader's words, or `None` while nothing failed.
    pub error: Option<String>,
}

impl State {
    /// A form filled from what the store read.
    ///
    /// The values are already the ones in force -- the store resolves an
    /// instance that overrides nothing to this launcher's own numbers -- so a
    /// switch drawn off still shows the reader what a launch would use.
    pub fn new(
        id: String,
        name: String,
        loaded: &InstanceSettings,
        installation: &InstanceInstallation,
        link: Load<InstanceLink>,
    ) -> State {
        State {
            id,
            name,
            loaded: true,
            tab: Tab::Java,
            override_java: loaded.override_java,
            java_path: loaded.java_path.clone(),
            override_memory: loaded.override_memory,
            memory_min: loaded.memory_min.to_string(),
            memory_max: loaded.memory_max.to_string(),
            override_java_args: loaded.override_java_args,
            jvm_args: loaded.jvm_args.clone(),
            platform: installation.platform,
            game_version: installation.game_version.clone(),
            loader_build: installation.loader_build.clone(),
            show_snapshots: false,
            game_query: String::new(),
            build_query: String::new(),
            versions: Load::Idle,
            builds: Load::Idle,
            builds_for: None,
            link,
            modpack: Load::Idle,
            repair: Load::Idle,
            error: None,
        }
    }

    /// The form for an instance that could not be read at all.
    ///
    /// The one state that draws no controls: the shell has a sentence and no
    /// values, and a form filled with defaults would be the launcher claiming an
    /// instance has a heap it never read.
    pub fn failed(id: String, name: String, problem: String) -> State {
        State {
            id,
            name,
            loaded: false,
            tab: Tab::Java,
            override_java: false,
            java_path: String::new(),
            override_memory: false,
            memory_min: String::new(),
            memory_max: String::new(),
            override_java_args: false,
            jvm_args: String::new(),
            platform: LoaderKind::Vanilla,
            game_version: String::new(),
            loader_build: String::new(),
            show_snapshots: false,
            game_query: String::new(),
            build_query: String::new(),
            versions: Load::Idle,
            builds: Load::Idle,
            builds_for: None,
            link: Load::Idle,
            modpack: Load::Idle,
            repair: Load::Idle,
            error: Some(problem),
        }
    }

    /// The message applied.
    pub fn update(&mut self, message: Message) {
        match message {
            Message::Tab(tab) => self.tab = tab,
            Message::OverrideJava(on) => self.override_java = on,
            Message::JavaPath(value) => self.java_path = value,
            Message::OverrideMemory(on) => self.override_memory = on,
            Message::MemoryMin(value) => self.memory_min = value,
            Message::MemoryMax(value) => self.memory_max = value,
            Message::OverrideJavaArgs(on) => self.override_java_args = on,
            Message::JvmArgs(value) => self.jvm_args = value,
            // A platform change drops the builds that belong to the old one, so
            // the list cannot draw the previous platform's versions under the new
            // heading while the shell reads the new pair -- and it drops the
            // *chosen build* too, because a build number belongs to one loader:
            // keeping Fabric's `0.16.9` while the form says Quilt would let Save
            // write a version that does not exist, and the store's own refusal
            // ("pick a Quilt build") is a better sentence than a broken profile.
            Message::Platform(platform) => {
                if platform != self.platform {
                    self.loader_build.clear();
                }
                self.platform = platform;
                self.builds = Load::Idle;
                self.builds_for = None;
            }
            // The same on the game version, for the same reason: a build is
            // published *for* a game version, and the pair is what the list and
            // the choice both belong to.
            Message::GameVersion(version) => {
                if version != self.game_version {
                    self.loader_build.clear();
                }
                self.game_version = version;
                self.builds = Load::Idle;
                self.builds_for = None;
            }
            Message::LoaderBuild(build) => self.loader_build = build,
            Message::GameQuery(value) => self.game_query = value,
            Message::BuildQuery(value) => self.build_query = value,
            Message::ShowSnapshots(on) => self.show_snapshots = on,
            // The crossing is only the clock's: it is recorded here because
            // this is the form's `update`, and the control that reports it is
            // drawn on the next frame.
            Message::Hover { key, over, hover } => crate::ui::pointer_with(
                key,
                over,
                hover.unwrap_or_else(crate::theme::hover_brightness),
            ),
            // See `Message::Save`, `Message::SaveInstallation`, `Message::Unlink`
            // and `Message::Repair`: the shell is the one that acts on them.
            Message::Save | Message::SaveInstallation | Message::Unlink | Message::Repair => {}
        }
    }

    /// The value a Java save would write, or the sentence that stops it.
    ///
    /// The number parse is here because this is where the buffers are; the
    /// store's own refusals -- a heap under the game's floor, or upside down --
    /// are its own, because that is where the file is.
    pub fn edit(&self) -> Result<InstanceSettings, String> {
        fn number(buffer: &str, what: &str) -> Result<i64, String> {
            buffer
                .trim()
                .parse::<i64>()
                .map_err(|_| format!("the {what} has to be a number of MiB; '{buffer}' is not one"))
        }
        let (memory_min, memory_max) = if self.override_memory {
            (
                number(&self.memory_min, "minimum heap")?,
                number(&self.memory_max, "maximum heap")?,
            )
        } else {
            // Nothing is written while the gate is off, so a buffer that no
            // longer parses cannot stand between the reader and their other two
            // settings.
            (0, 0)
        };
        Ok(InstanceSettings {
            java_path: self.java_path.clone(),
            override_java: self.override_java,
            memory_min,
            memory_max,
            override_memory: self.override_memory,
            jvm_args: self.jvm_args.clone(),
            override_java_args: self.override_java_args,
        })
    }

    /// The value an installation save would write.
    ///
    /// No parse to do: every choice the tab offers is already one of the values
    /// the profile holds, and a buffer the reader typed is not among the controls
    /// -- the store's refusals are about what the profile can carry.
    pub fn installation(&self) -> InstanceInstallation {
        InstanceInstallation {
            platform: self.platform,
            game_version: self.game_version.clone(),
            loader_build: self.loader_build.clone(),
        }
    }

    /// Whether the shell should read Mojang's version list for this form.
    ///
    /// Only the installation tab needs it, and only once: [`Load::settled`] is
    /// what stops a second request per frame, and the shell answers by asking the
    /// store once and handing the answer back through its own message.
    pub fn needs_versions(&self) -> bool {
        self.loaded && self.tab == Tab::Installation && self.versions == Load::Idle
    }

    /// The `(platform, game)` pair whose builds the shell should read, if the
    /// list on screen does not belong to the pair the reader is on.
    ///
    /// Vanilla has nothing to read -- it has no builds -- so a vanilla form
    /// answers `None` rather than asking a service that would answer nothing.
    pub fn needs_builds(&self) -> Option<(LoaderKind, String)> {
        if !self.loaded || self.tab != Tab::Installation || !self.platform.loads_mods() {
            return None;
        }
        let game = self.game_version.trim();
        if game.is_empty() {
            return None;
        }
        let pair = (self.platform, game.to_string());
        if self.builds_for.as_ref() == Some(&pair) {
            return None;
        }
        Some(pair)
    }

    /// The project whose name the shell should read, when the card has none.
    ///
    /// Only on the installation tab, and only once: [`Load::settled`] is what
    /// stops a second request per frame, exactly as it does for the two lists.
    /// The Java half draws no card, so a modal opened on it asks nobody anything.
    pub fn needs_modpack(&self) -> Option<String> {
        if !self.loaded || self.tab != Tab::Installation || self.modpack != Load::Idle {
            return None;
        }
        match &self.link {
            Load::Ready(link) => Some(link.project_id.clone()),
            _ => None,
        }
    }

    /// The versions the game-version list draws: releases, snapshots only when
    /// the reader asked for them, and only those whose id matches the search.
    ///
    /// A pure function of the state so the filter can be tested without a window,
    /// which is the same split [`crate::scroll`] uses for its window.
    pub fn shown_versions(&self) -> Vec<GameVersion> {
        let query = self.game_query.trim().to_ascii_lowercase();
        let Some(versions) = self.versions.ready() else {
            return Vec::new();
        };
        versions
            .iter()
            .filter(|version| self.show_snapshots || version.release)
            .filter(|version| query.is_empty() || version.id.to_ascii_lowercase().contains(&query))
            .cloned()
            .collect()
    }

    /// The builds the loader-build list draws, filtered by their own search.
    pub fn shown_builds(&self) -> Vec<LoaderBuild> {
        let query = self.build_query.trim().to_ascii_lowercase();
        let Some(builds) = self.builds.ready() else {
            return Vec::new();
        };
        builds
            .iter()
            .filter(|build| query.is_empty() || build.version.to_ascii_lowercase().contains(&query))
            .cloned()
            .collect()
    }
}

/// The form's body: the tab strip, then the half it selects.
pub fn view(theme: Gen, state: &State) -> Element<'_, Message> {
    if !state.loaded {
        let sentence = state.error.clone().unwrap_or_default();
        return ui::admonition(theme, ui::Severity::Warning, "instance-settings", &sentence);
    }
    let tabs = ui::tabs(
        theme,
        &[JAVA_TAB_KEY, INSTALLATION_TAB_KEY],
        // The installation label is the reference's own (`instance.settings.tabs
        // .installation`); the Java one is this module's word for the settings
        // the reference draws inside its sync-overrides tab, which is the tab
        // G118's decision left without a home.
        &[
            ("Java".to_string(), state.tab == Tab::Java),
            (
                Key::InstanceSettingsTabsInstallation.message().to_string(),
                state.tab == Tab::Installation,
            ),
        ],
        |index| Message::Tab(if index == 0 { Tab::Java } else { Tab::Installation }),
    );
    let body = match state.tab {
        Tab::Java => java_body(theme, state),
        Tab::Installation => installation_body(theme, state),
    };
    column![tabs, body].spacing(12.0).into()
}

/// The Java half: three sections, each a switch over its own controls.
fn java_body(theme: Gen, state: &State) -> Element<'_, Message> {
    let mut body = column![]
        .spacing(12.0)
        .push(section(
            theme,
            state.override_java,
            Message::OverrideJava(!state.override_java),
            Key::InstanceSettingsTabsJavaCustomJavaInstallation,
            field(
                theme,
                Key::InstanceSettingsTabsJavaJavaPathPlaceholder.message(),
                &state.java_path,
                Message::JavaPath,
            ),
        ))
        .push(memory_section(theme, state))
        .push(section(
            theme,
            state.override_java_args,
            Message::OverrideJavaArgs(!state.override_java_args),
            Key::InstanceSettingsTabsJavaCustomJavaArguments,
            field(
                theme,
                Key::InstanceSettingsTabsJavaEnterJavaArguments.message(),
                &state.jvm_args,
                Message::JvmArgs,
            ),
        ));
    if let Some(error) = &state.error {
        body = body.push(ui::admonition(theme, ui::Severity::Warning, "instance-settings", error));
    }
    body.push(save_row(theme, "instance-settings:save", Message::Save)).into()
}

/// The installation half: the platform, the game version and the loader's build.
///
/// The order is the reference's own rows: what the instance *is* first, then what
/// it runs, then which build of it -- and each picker says what is in force, so a
/// reader who changed a value can see both the choice and the file.
fn installation_body(theme: Gen, state: &State) -> Element<'_, Message> {
    let platform_labels: Vec<(String, bool)> = LoaderKind::all()
        .iter()
        .map(|loader| (loader.label().to_string(), state.platform == *loader))
        .collect();
    let platform_keys: Vec<&'static str> = LoaderKind::all()
        .iter()
        .map(|loader| platform_chip_key(*loader))
        .collect();
    let mut body = column![].spacing(12.0);
    // The linked pack goes first, where the reference draws it: above the platform,
    // the game version and the loader, because it is what the instance *is* rather
    // than what it runs.
    match &state.link {
        Load::Ready(_) => {
            body = body
                .push(modpack_card(theme, state))
                .push(unlink_section(theme))
                .push(repair_section(theme, state));
        }
        // The file that says what this instance came from is there and cannot be
        // read. A sentence where the card would be rather than no card at all:
        // "this instance came from nothing" and "this launcher cannot read the
        // link" are different answers, and only one of the two is anything the
        // reader could act on.
        Load::Failed(reason) => {
            body = body.push(ui::admonition(
                theme,
                ui::Severity::Warning,
                "instance-settings:link",
                reason,
            ));
        }
        // The link read is a file read the shell does before the modal is drawn, so
        // there is no waiting state for it to draw: `Loading` and `Empty` are arms
        // this read cannot produce, kept so the match is total.
        Load::Idle | Load::Loading | Load::Empty => {}
    }
    body = body
        .push(ui::card(
            theme,
            column![]
                .spacing(8.0)
                .push(section_heading(theme, Key::LabelPlatform.message()))
                .push(ui::chips(theme, &platform_keys, &platform_labels, |index| {
                    Some(Message::Platform(LoaderKind::all()[index]))
                })),
        ))
        .push(ui::card(
            theme,
            column![]
                .spacing(8.0)
                .push(section_heading(
                    theme,
                    &format!("{} · {}", Key::LabelGameVersion.message(), state.game_version),
                ))
                .push(ui::search(
                    theme,
                    Key::CreationFlowModalCustomSetupGameVersionSearchPlaceholder.message(),
                    &state.game_query,
                    Message::GameQuery,
                ))
                .push(version_list(theme, state))
                .push(snapshot_toggle(theme, state)),
        ));
    if state.platform.loads_mods() {
        body = body.push(ui::card(
            theme,
            column![]
                .spacing(8.0)
                .push(section_heading(
                    theme,
                    // The reference's own row label, `{loader} version`, filled
                    // with the platform on screen.
                    &loader_version_heading(state),
                ))
                .push(ui::search(
                    theme,
                    Key::CreationFlowModalCustomSetupLoaderVersionSearchPlaceholder.message(),
                    &state.build_query,
                    Message::BuildQuery,
                ))
                .push(build_list(theme, state)),
        ));
    } else {
        // Vanilla's own sentence, in the reference's words for a build row with
        // nothing behind it: the loader row is *drawn* rather than hidden, so the
        // modal does not change height when the platform does.
        body = body.push(ui::card(
            theme,
            column![]
                .spacing(8.0)
                .push(section_heading(
                    theme,
                    Key::CreationFlowModalCustomSetupLoaderVersionLabel.message(),
                ))
                .push(paragraph(
                    theme,
                    Key::CreationFlowModalCustomSetupOptionsNoVersionsAvailable.message(),
                )),
        ));
    }
    body = body.push(paragraph(theme, SAVE_INSTALLS_NOTE));
    if let Some(error) = &state.error {
        body = body.push(ui::admonition(theme, ui::Severity::Warning, "instance-settings", error));
    }
    body.push(save_row(theme, "instance-settings:installation:save", Message::SaveInstallation))
        .into()
}

/// The pack an instance was installed from, named.
///
/// The heading is the reference's own (`label.installed-modpack`), and the card
/// under it is the reference's shape minus its icon: the project's picture is a
/// fetch and a decode this slice does not do, and a card with the pack named in
/// words is a card -- what it would add is the picture, not the answer.
fn modpack_card<'a>(theme: Gen, state: &'a State) -> Element<'a, Message> {
    let mut card = column![
    ]
    .spacing(8.0)
    .push(section_heading(theme, Key::LabelInstalledModpack.message()));
    card = match &state.modpack {
        // Asked, not answered. The reference replaces its whole tab with a spinner
        // while its own linked-pack query is in flight; this is the kit's word for
        // the same wait, drawn where the name will be.
        Load::Idle | Load::Loading => card.push(paragraph(theme, Key::LabelLoading.message())),
        Load::Failed(reason) => card.push(paragraph_ink(theme, reason, Ink::Red)),
        // An answer with nothing in it is not a state this read produces -- a linked
        // instance is linked to a project, so its answer is a title or the failure
        // above. The arm draws the heading alone rather than a name invented for it,
        // which is what keeps the match total without lying about what arrived.
        Load::Empty => card,
        Load::Ready(pack) => card.push(pack_line(theme, pack)),
    };
    ui::card(theme, card)
}

/// One named pack: the title the service gave, and the caption under it.
fn pack_line<'a>(theme: Gen, pack: &LinkedModpack) -> Element<'a, Message> {
    let caption = pack_caption(pack);
    let mut line = column![
    ]
    .spacing(2.0)
    .push(
        text(pack.title.clone())
            .size(14.0)
            .font(semibold())
            .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
    );
    if !caption.is_empty() {
        line = line.push(paragraph(theme, &caption));
    }
    line.into()
}

/// The pack's caption: the author and the version number, with the middot the
/// reference puts between them only when both arrived.
///
/// Either half can be missing and neither is a failure: an author is a caption on
/// a team read that is allowed to fail, and a version the author has deleted is a
/// version no card can name.
fn pack_caption(pack: &LinkedModpack) -> String {
    match (pack.author.is_empty(), pack.version.is_empty()) {
        (false, false) => format!("{} · {}", pack.author, pack.version),
        (false, true) => pack.author.clone(),
        (true, false) => pack.version.clone(),
        (true, true) => String::new(),
    }
}

/// The way to forget the link, in the reference's own words.
///
/// The heading is the reference's `installation-settings.linked-instance.title`
/// filled with its own word for the kind of link (`modpack`), and the button is
/// `button.unlink-modpack`. The kit has no orange, which is the colour the
/// reference gives this button beside the red it gives *Re-install*: both are its
/// "this takes something away" pair, and here both take the one colour this kit
/// has for that. The sentence under the button is the reference's own, and it is
/// the whole of what this button does -- the instance keeps its files and stops
/// being updatable, because the thing that went is the link and not the content.
///
/// `'static` rather than borrowed, because everything in it is owned: the two
/// sentences are the reference's words with its placeholders filled, so nothing
/// here outlives the call.
fn unlink_section(theme: Gen) -> Element<'static, Message> {
    ui::card(
        theme,
        column![]
            .spacing(8.0)
            .push(section_heading(theme, &unlink_title()))
            .push(paragraph(theme, &unlink_sentence()))
            .push(row![ui::button(
                theme,
                UNLINK_KEY,
                Key::ButtonUnlinkModpack,
                ui::Kind::Danger,
                Message::Unlink,
            )]),
    )
}

/// The unlink section's heading: the reference's `Linked {projectType}` filled
/// with its own word for the kind of link.
fn unlink_title() -> String {
    Key::InstallationSettingsLinkedInstanceTitle
        .message()
        .replace("{projectType}", Key::InstallationSettingsLinkedModpack.message())
}

/// The unlink section's sentence, with the reference's two placeholders filled.
///
/// Both words are the reference's own (`instance`, and `modpack` for the thing
/// being unlinked from), and the sentence is the whole of what the button does: a
/// reader who presses it keeps every file and stops receiving updates.
fn unlink_sentence() -> String {
    Key::InstallationSettingsUnlinkDescription
        .message()
        .replace("{type}", Key::InstallationSettingsTypeInstance.message())
        .replace("{projectType}", Key::InstallationSettingsLinkedModpack.message())
}

/// The way to re-install the instance's own files.
///
/// The heading, the button and the sentence under it are the reference's own
/// (`installation-settings.repair.instance-title`, `button.repair`/`button
/// .repairing`, and `installation-settings.repair.instance-description`), and the
/// sentence says what the button is for: the launcher's half of an instance -- the
/// loader, Minecraft's libraries, the client jar and the assets -- checked and
/// put back, with nothing a reader added to the instance touched. The reference
/// draws its description under the button and asks again in a confirmation modal;
/// this kit has no such modal, and the sentence under the button is the whole of
/// what the confirmation would say, so the press is the confirmation.
///
/// What the section draws once the check is over is this launcher's own word for
/// it: the reference answers through a notification, and a modal that is already
/// open has a better place to be told.
fn repair_section<'a>(theme: Gen, state: &'a State) -> Element<'a, Message> {
    let button = if state.repair == Load::Loading {
        // The press is gone while the check runs, for [`crate::ui::button_or`]'s
        // reason: a second press would start a second install over the first, and
        // the reference draws a spinner here for the same one.
        ui::button_or(theme, REPAIRING_KEY, Key::ButtonRepairing, ui::Kind::Standard, None)
    } else {
        ui::button(theme, REPAIR_KEY, Key::ButtonRepair, ui::Kind::Standard, Message::Repair)
    };
    let mut section = column![]
        .spacing(8.0)
        .push(section_heading(theme, Key::InstallationSettingsRepairInstanceTitle.message()))
        .push(row![button])
        .push(paragraph(theme, Key::InstallationSettingsRepairInstanceDescription.message()));
    match &state.repair {
        Load::Ready(line) => section = section.push(paragraph(theme, line)),
        Load::Failed(reason) => section = section.push(paragraph_ink(theme, reason, Ink::Red)),
        // Neither has been asked for, the check is running -- the disabled button
        // above is that state's whole drawing -- or the section is drawn for a form
        // whose repair state was never set. The last arm is not a state the shell
        // produces; it is here so the match is total without inventing a sentence.
        Load::Idle | Load::Loading | Load::Empty => {}
    }
    ui::card(theme, section)
}

/// The sentence under the installation Save: what a version or platform change
/// costs, which is a download at the next launch rather than now.
const SAVE_INSTALLS_NOTE: &str =
    "The next launch installs what a changed version or platform needs; nothing is downloaded \
     until then.";

/// The game-version list: the versions the search and the snapshot toggle leave,
/// drawn as pressable rows, or the sentence for a list that has nothing.
fn version_list<'a>(theme: Gen, state: &'a State) -> Element<'a, Message> {
    let rows = match &state.versions {
        Load::Idle | Load::Loading => Some((Key::LabelLoading.message().to_string(), INK_SECONDARY)),
        Load::Failed(reason) => Some((reason.clone(), Ink::Red)),
        Load::Empty => Some((
            Key::CreationFlowModalCustomSetupOptionsNoVersionsAvailable.message().to_string(),
            INK_SECONDARY,
        )),
        Load::Ready(_) => None,
    };
    if let Some((sentence, ink)) = rows {
        return paragraph_ink(theme, &sentence, ink);
    }
    let versions = state.shown_versions();
    if versions.is_empty() {
        return paragraph(
            theme,
            Key::CreationFlowModalCustomSetupOptionsNoVersionsAvailable.message(),
        );
    }
    let items: Vec<Element<'_, Message>> = versions
        .iter()
        .map(|version| {
            let key = ui::scoped(VERSION_ROW, &version.id);
            choice_row(
                theme,
                key,
                version.id.clone(),
                version.id == state.game_version,
                Message::GameVersion(version.id.clone()),
            )
        })
        .collect();
    scrollable(column(items).width(Length::Fill))
        .height(Length::Fixed(LIST_HEIGHT))
        .into()
}

/// The game-version list's footer: the reference's own row that adds the
/// snapshots and the old versions to the list, and takes them away again
/// (`ButtonShowAllVersions` / `ButtonHideSnapshots`).
///
/// It is drawn here rather than taken from [`ui`] because the kit has no widget
/// for it: the shape is the one the creation dialog's picker draws from its own
/// `version_toggle`, over the same two words and the same eye glyph, so a later
/// slice that lifts it into [`ui`] has both call sites waiting for it.
fn snapshot_toggle<'a>(theme: Gen, state: &State) -> Element<'a, Message> {
    let (key, label, glyph) = if state.show_snapshots {
        (SNAPSHOTS_HIDE, Key::ButtonHideSnapshots, Glyph::EyeOff)
    } else {
        (SNAPSHOTS_SHOW_ALL, Key::ButtonShowAllVersions, Glyph::Eye)
    };
    let (_, fraction) = ui::interaction(key);
    // The reference's hover is `text-secondary hover:text-contrast` -- an ink
    // that moves rather than a surface that brightens -- so the row is drawn from
    // the crossing's fraction rather than from a brightness factor.
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
                text(label.message()).size(14.0).font(semibold()).style(iced::theme::Text::Color(ink)),
            ),
    )
    .width(Length::Fill)
    .center_x()
    .padding(Padding { top: TOGGLE_PAD, bottom: TOGGLE_PAD, left: 0.0, right: 0.0 });
    mouse_area(row)
        .interaction(Interaction::Pointer)
        .on_enter(Hovered::hover(key, true))
        .on_exit(Hovered::hover(key, false))
        .on_press(Message::ShowSnapshots(!state.show_snapshots))
        .into()
}

/// The loader-build list, the same shape over the platform's own builds.
fn build_list<'a>(theme: Gen, state: &'a State) -> Element<'a, Message> {
    let rows = match &state.builds {
        Load::Idle | Load::Loading => Some((Key::LabelLoading.message().to_string(), INK_SECONDARY)),
        Load::Failed(reason) => Some((reason.clone(), Ink::Red)),
        Load::Empty => Some((
            Key::CreationFlowModalCustomSetupOptionsNoVersionsAvailable.message().to_string(),
            INK_SECONDARY,
        )),
        Load::Ready(_) => None,
    };
    if let Some((sentence, ink)) = rows {
        return paragraph_ink(theme, &sentence, ink);
    }
    let builds = state.shown_builds();
    if builds.is_empty() {
        return paragraph(
            theme,
            Key::CreationFlowModalCustomSetupOptionsNoVersionsAvailable.message(),
        );
    }
    let items: Vec<Element<'_, Message>> = builds
        .iter()
        .map(|build| {
            let key = ui::scoped(BUILD_ROW, &build.version);
            let label = if build.stable {
                build.version.clone()
            } else {
                // A build the loader itself does not call stable says so, which
                // is the reference's own `VersionType` distinction drawn beside
                // the number rather than instead of it.
                format!(
                    "{} · {}",
                    build.version,
                    Key::CreationFlowModalCustomSetupLoaderVersionTypeOther.message()
                )
            };
            choice_row(
                theme,
                key,
                label,
                build.version == state.loader_build,
                Message::LoaderBuild(build.version.clone()),
            )
        })
        .collect();
    scrollable(column(items).width(Length::Fill))
        .height(Length::Fixed(LIST_HEIGHT))
        .into()
}

/// The loader row's heading: the reference's `{loader} version`, filled.
fn loader_version_heading(state: &State) -> String {
    let template = Key::InstanceSettingsTabsInstallationLoaderVersion.message();
    let filled = template.replace("{loader}", state.platform.label());
    format!("{filled} · {}", state.loader_build)
}

/// One pressable row: a version or a build, plated while it is the value in
/// force, labelled with the value itself.
///
/// A row rather than a chip, because a picker's list is long and a chip row would
/// wrap: this is the reference's own dropdown shape (a full-width row per option)
/// drawn with the kit's hover ticket so the crossing has an owner.
fn choice_row<'a>(
    theme: Gen,
    key: &'static str,
    label: String,
    chosen: bool,
    on_press: Message,
) -> Element<'a, Message> {
    let (factor, _) = ui::interaction(key);
    let ink = if chosen { INK_CONTRAST } else { INK_SECONDARY };
    let plate = chosen.then(|| theme_gen::ink(theme, Ink::ButtonBg));
    let row = container(
        text(label)
            .size(14.0)
            .font(medium())
            .style(iced::theme::Text::Color(crate::theme::brightness(
                theme_gen::ink(theme, ink),
                factor,
            ))),
    )
    .width(Length::Fill)
    .padding(Padding { top: 8.0, bottom: 8.0, left: 12.0, right: 12.0 })
    .style(move |_theme: &iced::Theme| container::Appearance {
        background: plate.map(Background::Color),
        border: Border { radius: 8.0.into(), ..Border::default() },
        ..container::Appearance::default()
    });
    mouse_area(row)
        .interaction(Interaction::Pointer)
        .on_enter(Hovered::hover(key, true))
        .on_exit(Hovered::hover(key, false))
        .on_press(on_press)
        .into()
}

/// A pressable row's stable name: one per value, from the value itself.
fn platform_chip_key(loader: LoaderKind) -> &'static str {
    // The chips repeat and the clock wants `&'static str`, which is what
    // `ui::scoped` exists for -- the same reason the loader chips in the creation
    // dialog have theirs.
    ui::scoped("instance-settings:platform", loader.label())
}

/// A section's title, on the dialog's own surface.
fn section_heading<'a>(theme: Gen, title: &str) -> Element<'a, Message> {
    text(title.to_string())
        .size(14.0)
        .font(semibold())
        .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST)))
        .into()
}

/// A sentence at the secondary ink: a note, not a failure.
fn paragraph<'a>(theme: Gen, sentence: &str) -> Element<'a, Message> {
    paragraph_ink(theme, sentence, INK_SECONDARY)
}

/// The same, in a caller's own ink: a failure is drawn in red and everything
/// else in secondary.
fn paragraph_ink<'a>(theme: Gen, sentence: &str, ink: Ink) -> Element<'a, Message> {
    text(sentence.to_string())
        .size(12.0)
        .font(medium())
        .style(iced::theme::Text::Color(theme_gen::ink(theme, ink)))
        .into()
}

/// The Save button at the end of a tab, on its own row.
fn save_row<'a>(theme: Gen, key: &'static str, message: Message) -> Element<'a, Message> {
    row![]
        .align_items(Alignment::Center)
        .push(Space::with_width(Length::Fill))
        .push(ui::button(theme, key, Key::ButtonSave, ui::Kind::Colored, message))
        .into()
}

/// The memory section: the switch, then the two numbers the instance file holds.
///
/// The reference's own memory control is a slider over the machine's RAM, which
/// this kit has none of; the pair of numbers is what the file and Prism's own
/// pane already hold, so the form edits the thing it will write. The two labels
/// are this module's words -- the reference has no label for them because it has
/// no fields -- and they say `MiB` for Prism's sake, whose keys are megabytes by
/// name and mebibytes by value.
fn memory_section(theme: Gen, state: &State) -> Element<'_, Message> {
    let number_field = |label: &'static str, value: &str, on_input: fn(String) -> Message| {
        column![]
            .spacing(4.0)
            .width(Length::Fill)
            .push(
                text(label.to_string())
                    .size(12.0)
                    .font(medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
            )
            .push(field(theme, "1024", value, on_input))
    };
    section(
        theme,
        state.override_memory,
        Message::OverrideMemory(!state.override_memory),
        Key::InstanceSettingsTabsJavaCustomMemoryAllocation,
        row![]
            .spacing(8.0)
            .push(number_field("Minimum (MiB)", &state.memory_min, Message::MemoryMin))
            .push(number_field("Maximum (MiB)", &state.memory_max, Message::MemoryMax))
            .into(),
    )
}

/// One section: its override switch, its title, and the controls under it.
fn section<'a>(
    theme: Gen,
    on: bool,
    on_press: Message,
    title: Key,
    body: Element<'a, Message>,
) -> Element<'a, Message> {
    ui::card(
        theme,
        column![]
            .spacing(8.0)
            .push(
                row![]
                    .align_items(Alignment::Center)
                    .spacing(8.0)
                    .push(ui::switch(theme, on, on_press))
                    .push(section_heading(theme, title.message())),
            )
            .push(body),
    )
}

/// A text field on the dialog's own surface: the shell's create dialog field,
/// which is where this kit's bordered text style comes from.
fn field<'a>(
    theme: Gen,
    placeholder: &str,
    value: &str,
    on_input: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    text_input(placeholder, value)
        .on_input(on_input)
        .padding(Padding { top: 10.0, bottom: 10.0, left: 12.0, right: 12.0 })
        .size(14.0)
        .font(medium())
        .style(iced::theme::TextInput::Custom(Box::new(ui::Field::bordered(theme))))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loaded() -> InstanceSettings {
        InstanceSettings {
            java_path: "C:/jdk21/bin/javaw.exe".to_string(),
            override_java: true,
            memory_min: 2048,
            memory_max: 8192,
            override_memory: true,
            jvm_args: "-XX:+UseG1GC".to_string(),
            override_java_args: true,
        }
    }

    fn installed() -> InstanceInstallation {
        InstanceInstallation {
            platform: LoaderKind::Fabric,
            game_version: "1.21.4".to_string(),
            loader_build: "0.16.9".to_string(),
        }
    }

    fn state() -> State {
        State::new(
            "atm10".to_string(),
            "All the Mods 10".to_string(),
            &loaded(),
            &installed(),
            // An instance this launcher made by hand: there is no link file, and
            // nothing about its installation tab is owed because of that.
            Load::Idle,
        )
    }

    /// The same form for an instance that was installed from a Modrinth pack.
    fn linked_state() -> State {
        State::new(
            "cobblemon".to_string(),
            "Cobblemon".to_string(),
            &loaded(),
            &installed(),
            Load::Ready(InstanceLink {
                project_id: "cobblemon".to_string(),
                version_id: "pack-1".to_string(),
            }),
        )
    }

    fn named(author: &str, version: &str) -> LinkedModpack {
        LinkedModpack {
            project_id: "cobblemon".to_string(),
            title: "Cobblemon".to_string(),
            author: author.to_string(),
            version: version.to_string(),
        }
    }

    #[test]
    fn a_linked_instance_asks_for_its_pack_and_an_unlinked_one_asks_nothing() {
        // The card's name is the service's, so the shell is asked for it once --
        // and an instance that came from no pack asks nobody anything, which is
        // what keeps a hand-made instance off the network.
        let mut plain = state();
        plain.update(Message::Tab(Tab::Installation));
        assert_eq!(plain.needs_modpack(), None, "this instance came from nothing");

        let mut linked = linked_state();
        assert_eq!(
            linked.needs_modpack(),
            None,
            "the Java half draws no card, so opening on it asks nothing"
        );
        linked.update(Message::Tab(Tab::Installation));
        assert_eq!(linked.needs_modpack(), Some("cobblemon".to_string()));
        // One answer arrives, and the question is not asked again on the next
        // frame: `Load::settled` is the same rule the two lists keep.
        linked.modpack = Load::Ready(named("jellysquid3", "1.6.1"));
        assert_eq!(linked.needs_modpack(), None);

        // A link file that cannot be read is not a question either: there is
        // nothing to name, and the tab says so where the card would be.
        let broken = State::new(
            "cobblemon".to_string(),
            "Cobblemon".to_string(),
            &loaded(),
            &installed(),
            Load::Failed("modrinth-link.json is not a link this launcher wrote".to_string()),
        );
        assert_eq!(broken.needs_modpack(), None);
    }

    #[test]
    fn the_pack_caption_says_what_the_service_gave_and_nothing_more() {
        // The reference puts the author and the version number on one line with a
        // middot between them, and both halves can be missing: a team read that
        // failed, or a version its author has deleted.
        assert_eq!(pack_caption(&named("jellysquid3", "1.6.1")), "jellysquid3 · 1.6.1");
        assert_eq!(pack_caption(&named("jellysquid3", "")), "jellysquid3");
        assert_eq!(
            pack_caption(&named("", "1.6.1")),
            "1.6.1",
            "no middot when there is only one thing to say"
        );
        assert_eq!(pack_caption(&named("", "")), "");
    }

    #[test]
    fn the_unlink_copy_is_the_reference_own_with_its_placeholders_filled() {
        // The two words the reference fills in, and what a reader must never see:
        // an unfilled `{projectType}` left in a sentence.
        let title = unlink_title();
        assert_eq!(title, "Linked modpack");
        let sentence = unlink_sentence();
        assert!(!sentence.contains('{'), "{sentence}");
        assert!(sentence.contains("this instance"), "{sentence}");
        assert!(sentence.contains("modpack"), "{sentence}");
    }

    #[test]
    fn the_repair_section_wears_the_reference_own_words() {
        // The heading, the button in both of its states and the sentence under it
        // are the reference's own, and they are the whole of what the section
        // says: the two sentences of the installation tab that a reader must not
        // see paraphrased are this one and *Unlink*'s.
        assert_eq!(
            Key::InstallationSettingsRepairInstanceTitle.message(),
            "Repair instance"
        );
        assert_eq!(Key::ButtonRepair.message(), "Repair");
        assert_eq!(Key::ButtonRepairing.message(), "Repairing...");
        let description = Key::InstallationSettingsRepairInstanceDescription.message();
        assert!(description.contains("checks for corruption"), "{description}");
        assert!(
            description.contains("Minecraft dependencies"),
            "the sentence names what is re-installed: {description}"
        );
    }

    #[test]
    fn the_form_does_not_start_a_repair_itself() {
        // `Message::Repair` is the shell's -- the check is a minute of blocking
        // work and a window that ran it would stop drawing -- so the form's own
        // update leaves the state where it is. It is the shell that moves the
        // form to `Load::Loading` when it raises the request, and the reply that
        // moves it on from there; a form that started the work itself would be
        // the frame thread hashing an install.
        let mut state = linked_state();
        state.update(Message::Repair);
        assert_eq!(state.repair, Load::Idle, "nothing was asked of the shell here");
        assert_eq!(state.error, None, "and no sentence appeared under the button");
        assert_eq!(
            state.link,
            Load::Ready(InstanceLink {
                project_id: "cobblemon".to_string(),
                version_id: "pack-1".to_string(),
            }),
            "the link is still what the card is drawn from"
        );
    }

    #[test]
    fn the_form_starts_where_the_file_is() {
        let state = state();
        assert_eq!(state.java_path, "C:/jdk21/bin/javaw.exe");
        assert_eq!(state.memory_min, "2048");
        assert_eq!(state.memory_max, "8192");
        assert_eq!(state.jvm_args, "-XX:+UseG1GC");
        assert!(state.override_memory && state.override_java && state.override_java_args);
        assert!(state.error.is_none());
        // A form that is opened and saved without a keystroke writes back what
        // it read: the round trip is the modal's whole promise.
        assert_eq!(state.edit().expect("an edit"), loaded());

        // The installation half's round trip is the same promise, over the other
        // file: what the profile said is what a save without a press writes.
        assert_eq!(state.tab, Tab::Java, "the modal opens on the Java half");
        assert_eq!(state.installation(), installed());
        assert_eq!(state.link, Load::Idle, "and it came from no pack");
    }

    #[test]
    fn a_field_that_is_not_a_number_is_a_sentence_rather_than_a_written_line() {
        let mut state = state();
        state.update(Message::MemoryMin("2048x".to_string()));
        let refused = state.edit().expect_err("not a number");
        assert!(refused.contains("minimum heap"), "{refused}");
        assert!(refused.contains("2048x"), "the sentence names what was typed: {refused}");

        // The other direction: a field that is off does not stand between the
        // reader and the settings that are on.
        state.update(Message::OverrideMemory(false));
        let edit = state.edit().expect("the gates that are on still save");
        assert!(!edit.override_memory);
        assert!(edit.override_java && edit.override_java_args);
    }

    #[test]
    fn the_switches_are_flipped_by_their_own_messages() {
        let mut state = state();
        state.update(Message::OverrideMemory(false));
        state.update(Message::OverrideJava(false));
        state.update(Message::OverrideJavaArgs(false));
        assert!(!state.override_memory && !state.override_java && !state.override_java_args);
        state.update(Message::OverrideJava(true));
        assert!(state.override_java && !state.override_memory);
        // Flipping a switch is not a save: the buffers stay where they were, so
        // a reader who changes their mind finds their numbers again.
        assert_eq!(state.memory_max, "8192");
    }

    #[test]
    fn a_platform_change_asks_for_the_new_build_list_and_a_vanilla_choice_stops_asking() {
        // The list on screen belongs to a pair of values; changing either one
        // makes it stale, and the shell is told by `needs_builds` rather than by
        // a second copy of the rule.
        let mut state = state();
        state.update(Message::Tab(Tab::Installation));
        assert_eq!(
            state.needs_builds(),
            Some((LoaderKind::Fabric, "1.21.4".to_string())),
            "the tab asks for the builds in force"
        );
        // One answer arrives: the pair is remembered, so the same question is not
        // asked again on the next frame.
        state.builds_for = state.needs_builds();
        state.builds = Load::Ready(vec![LoaderBuild { version: "0.16.9".to_string(), stable: true }]);
        assert_eq!(state.needs_builds(), None);

        state.update(Message::GameVersion("1.21.1".to_string()));
        assert_eq!(state.builds, Load::Idle, "the old list goes with the old pair");
        assert_eq!(
            state.loader_build, "",
            "and the build chosen for the old game version goes with it"
        );
        assert_eq!(state.needs_builds(), Some((LoaderKind::Fabric, "1.21.1".to_string())));

        // Vanilla has no builds to ask anyone for, and the build it cannot have
        // is taken out of the value a save would write.
        state.update(Message::Platform(LoaderKind::Vanilla));
        assert_eq!(state.needs_builds(), None);
        assert_eq!(state.loader_build, "");
        assert_eq!(state.installation().platform, LoaderKind::Vanilla);
    }

    #[test]
    fn the_lists_draw_what_the_search_and_the_snapshot_toggle_leave() {
        let mut state = state();
        state.versions = Load::Ready(vec![
            GameVersion { id: "1.21.4".to_string(), release: true },
            GameVersion { id: "1.21.1".to_string(), release: true },
            GameVersion { id: "24w45a".to_string(), release: false },
        ]);
        let ids = |state: &State| {
            state.shown_versions().into_iter().map(|version| version.id).collect::<Vec<_>>()
        };
        assert_eq!(ids(&state), ["1.21.4", "1.21.1"], "releases by default");
        state.update(Message::ShowSnapshots(true));
        assert_eq!(ids(&state), ["1.21.4", "1.21.1", "24w45a"]);
        state.update(Message::GameQuery("21.1".to_string()));
        assert_eq!(ids(&state), ["1.21.1"], "the search narrows the same list");

        state.builds = Load::Ready(vec![
            LoaderBuild { version: "0.16.9".to_string(), stable: true },
            LoaderBuild { version: "0.17.0-beta.1".to_string(), stable: false },
        ]);
        assert_eq!(state.shown_builds().len(), 2);
        state.update(Message::BuildQuery("beta".to_string()));
        assert_eq!(state.shown_builds().len(), 1);
        assert_eq!(state.shown_builds()[0].version, "0.17.0-beta.1");
    }
}
