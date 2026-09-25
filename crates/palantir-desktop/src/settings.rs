//! The launcher's own settings: the section list, the panes, and the vocabulary
//! that names every switch.
//!
//! The shape is the reference client's, read off its own source rather than
//! guessed from a screenshot: a two-column dialog whose left column is a
//! scrollable list grouped under Display / Account / Instances, and whose right
//! column is one pane drawn at `min(65vh, 600px)` with its own scrollbar. A tab
//! is `rounded-xl px-4 py-2` — a 20px radius and 16px horizontal padding — with
//! a 16px icon, and it is `--color-button-bg-selected` (the accent at 25%) when
//! it is the open one.
//!
//! Two rules this module holds to, because they are the difference between a
//! settings surface and a screenshot of one:
//!
//! * **A control that is drawn is a control that works.** Every switch here
//!   writes [`Prefs`] and every write reaches the disk, and the panes read the
//!   settings they display. Where a reference control has nothing behind it in
//!   this launcher, the row is drawn *disabled* with the reason printed under
//!   it, rather than as a switch that silently forgets — the same treatment the
//!   previous one-pane dialog already gave "sync theme across devices".
//! * **A number that was measured is not re-derived on the way past.** The
//!   switch geometry is the reference's own markup (`h-6 w-[48px] p-1` with a
//!   `w-4 h-4` knob) and [`switch_geometry`] is unit tested against it, so the
//!   knob cannot quietly stop landing on the end of its own track.

use std::collections::BTreeMap;
use std::path::Path;

use iced::widget::{
    button, column, container, row, scrollable, text, text_input, Column, MouseArea, Space,
};
use iced::{Alignment, Border, Color, Element, Length, Padding, Theme};

use crate::accounts::AccountEntry;
use crate::anim::SwitchAnim;
use crate::app::{hover_button, Message};
use crate::brand;
use crate::glyphs::glyph;
use crate::native;
use crate::color_theme::ColorTheme;
use crate::prefs::Prefs;
use crate::scroll;
use crate::theme;

// ---- The vocabulary ------------------------------------------------------

/// Every boolean setting the shell offers.
///
/// One enum rather than one `Message` variant per switch, so a new switch is a
/// row here plus its field in [`Prefs`] — and so the thing that persists it is a
/// single `update` arm that cannot be forgotten for one switch out of twenty.
/// [`Flag::id`] is also the key a slide animation is stored under, which is why
/// it is a stable string rather than the enum's index: reordering the list must
/// not move somebody's switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flag {
    SyncThemeAcrossDevices,
    AdvancedRendering,
    NativeDecorations,
    ExternalLinksNewTab,
    ShowWorldsTab,
    ShowFilesTab,
    ShowScreenshotsTab,
    ShowAllScreenshotsInSidebar,
    ShowSkinSelectorInSidebar,
    QuickInstancesInSidebar,
    ShowJumpInSection,
    MinimizeOnLaunch,
    HideRightSidebar,
    CompactInstanceCards,
    ShowPlayTime,
    WarnUnknownModpacks,
    SkipNonEssentialWarnings,
    Telemetry,
    DiscordRpc,
    AlwaysShowCopyDetails,
}

impl Flag {
    /// The switch's stable id: its animation key, and what the pointer messages
    /// carry across the message boundary.
    pub const fn id(self) -> &'static str {
        match self {
            Flag::SyncThemeAcrossDevices => "appearance.sync_theme_across_devices",
            Flag::AdvancedRendering => "appearance.advanced_rendering",
            Flag::NativeDecorations => "appearance.native_decorations",
            Flag::ExternalLinksNewTab => "appearance.external_links_new_tab",
            Flag::ShowWorldsTab => "features.show_worlds_tab",
            Flag::ShowFilesTab => "features.show_files_tab",
            Flag::ShowScreenshotsTab => "features.show_screenshots_tab",
            Flag::ShowAllScreenshotsInSidebar => "features.show_all_screenshots_in_sidebar",
            Flag::ShowSkinSelectorInSidebar => "features.show_skin_selector_in_sidebar",
            Flag::QuickInstancesInSidebar => "features.quick_instances_in_sidebar",
            Flag::ShowJumpInSection => "features.show_jump_in_section",
            Flag::MinimizeOnLaunch => "behavior.minimize_on_launch",
            Flag::HideRightSidebar => "behavior.hide_right_sidebar",
            Flag::CompactInstanceCards => "behavior.compact_instance_cards",
            Flag::ShowPlayTime => "behavior.show_play_time",
            Flag::WarnUnknownModpacks => "behavior.warn_unknown_modpacks",
            Flag::SkipNonEssentialWarnings => "behavior.skip_non_essential_warnings",
            Flag::Telemetry => "privacy.telemetry",
            Flag::DiscordRpc => "privacy.discord_rpc",
            Flag::AlwaysShowCopyDetails => "resource.always_show_copy_details",
        }
    }

    /// The name of the [`Prefs`] field this switch stores into.
    ///
    /// Kept as a string so that the tests can hold the mapping to account in
    /// both directions: that it names a real key in the prefs file, and that a
    /// flag claiming to be wired is read under that name by something outside
    /// this module. Without it, `wired` would be a predicate nothing could
    /// check — which is how it came to claim four flags the shell never read.
    ///
    /// Test-only, and deliberately so: nothing at runtime needs the field's
    /// name, and a `pub fn` the shell never calls is the next piece of dead
    /// weight this dialog would carry.
    #[cfg(test)]
    pub const fn field(self) -> &'static str {
        match self {
            Flag::SyncThemeAcrossDevices => "sync_theme_across_devices",
            Flag::AdvancedRendering => "advanced_rendering",
            Flag::NativeDecorations => "native_decorations",
            Flag::ExternalLinksNewTab => "external_links_new_tab",
            Flag::ShowWorldsTab => "show_worlds_tab",
            Flag::ShowFilesTab => "show_files_tab",
            Flag::ShowScreenshotsTab => "show_screenshots_tab",
            Flag::ShowAllScreenshotsInSidebar => "show_all_screenshots_in_sidebar",
            Flag::ShowSkinSelectorInSidebar => "show_skin_selector_in_sidebar",
            Flag::QuickInstancesInSidebar => "quick_instances_in_sidebar",
            Flag::ShowJumpInSection => "show_jump_in_section",
            Flag::MinimizeOnLaunch => "minimize_on_launch",
            Flag::HideRightSidebar => "hide_right_sidebar",
            Flag::CompactInstanceCards => "compact_instance_cards",
            Flag::ShowPlayTime => "show_play_time",
            Flag::WarnUnknownModpacks => "warn_unknown_modpacks",
            Flag::SkipNonEssentialWarnings => "skip_non_essential_warnings",
            Flag::Telemetry => "telemetry",
            Flag::DiscordRpc => "discord_rpc",
            Flag::AlwaysShowCopyDetails => "always_show_copy_details",
        }
    }

    /// The value in a prefs file.
    pub fn get(self, prefs: &Prefs) -> bool {
        match self {
            Flag::SyncThemeAcrossDevices => prefs.sync_theme_across_devices,
            Flag::AdvancedRendering => prefs.advanced_rendering,
            Flag::NativeDecorations => prefs.native_decorations,
            Flag::ExternalLinksNewTab => prefs.external_links_new_tab,
            Flag::ShowWorldsTab => prefs.show_worlds_tab,
            Flag::ShowFilesTab => prefs.show_files_tab,
            Flag::ShowScreenshotsTab => prefs.show_screenshots_tab,
            Flag::ShowAllScreenshotsInSidebar => prefs.show_all_screenshots_in_sidebar,
            Flag::ShowSkinSelectorInSidebar => prefs.show_skin_selector_in_sidebar,
            Flag::QuickInstancesInSidebar => prefs.quick_instances_in_sidebar,
            Flag::ShowJumpInSection => prefs.show_jump_in_section,
            Flag::MinimizeOnLaunch => prefs.minimize_on_launch,
            Flag::HideRightSidebar => prefs.hide_right_sidebar,
            Flag::CompactInstanceCards => prefs.compact_instance_cards,
            Flag::ShowPlayTime => prefs.show_play_time,
            Flag::WarnUnknownModpacks => prefs.warn_unknown_modpacks,
            Flag::SkipNonEssentialWarnings => prefs.skip_non_essential_warnings,
            Flag::Telemetry => prefs.telemetry,
            Flag::DiscordRpc => prefs.discord_rpc,
            Flag::AlwaysShowCopyDetails => prefs.always_show_copy_details,
        }
    }

    /// Write the value into a prefs file.
    pub fn set(self, prefs: &mut Prefs, value: bool) {
        match self {
            Flag::SyncThemeAcrossDevices => prefs.sync_theme_across_devices = value,
            Flag::AdvancedRendering => prefs.advanced_rendering = value,
            Flag::NativeDecorations => prefs.native_decorations = value,
            Flag::ExternalLinksNewTab => prefs.external_links_new_tab = value,
            Flag::ShowWorldsTab => prefs.show_worlds_tab = value,
            Flag::ShowFilesTab => prefs.show_files_tab = value,
            Flag::ShowScreenshotsTab => prefs.show_screenshots_tab = value,
            Flag::ShowAllScreenshotsInSidebar => prefs.show_all_screenshots_in_sidebar = value,
            Flag::ShowSkinSelectorInSidebar => prefs.show_skin_selector_in_sidebar = value,
            Flag::QuickInstancesInSidebar => prefs.quick_instances_in_sidebar = value,
            Flag::ShowJumpInSection => prefs.show_jump_in_section = value,
            Flag::MinimizeOnLaunch => prefs.minimize_on_launch = value,
            Flag::HideRightSidebar => prefs.hide_right_sidebar = value,
            Flag::CompactInstanceCards => prefs.compact_instance_cards = value,
            Flag::ShowPlayTime => prefs.show_play_time = value,
            Flag::WarnUnknownModpacks => prefs.warn_unknown_modpacks = value,
            Flag::SkipNonEssentialWarnings => prefs.skip_non_essential_warnings = value,
            Flag::Telemetry => prefs.telemetry = value,
            Flag::DiscordRpc => prefs.discord_rpc = value,
            Flag::AlwaysShowCopyDetails => prefs.always_show_copy_details = value,
        }
    }

    /// Whether the shell actually reads this setting.
    ///
    /// This is the one place that answers "does the launcher behave differently
    /// when this changes". A switch stores a value that survives a restart
    /// either way — so a switch whose value nothing reads looks exactly like one
    /// that works, and only its effect is missing. [`toggle_row`] therefore
    /// draws the switch *disabled* with [`Flag::unwired_reason`] under it when this
    /// is false. That is a visible gap, which is the point: a gap someone can
    /// see is a to-do, and a switch that silently forgets is a bug report.
    ///
    /// Adding a `true` here is a claim that the flag is read somewhere; the test
    /// below pins the set so that claim is a deliberate edit rather than a
    /// default.
    pub const fn wired(self) -> bool {
        // Read by the views: two rail entries drop out, an instance card loses
        // its chip row, the playtime chip is not drawn, the right panel is not
        // drawn, and launching minimizes the window.
        matches!(
            self,
            Flag::ShowWorldsTab
                | Flag::ShowScreenshotsTab
                | Flag::MinimizeOnLaunch
                | Flag::HideRightSidebar
                | Flag::CompactInstanceCards
                | Flag::ShowPlayTime
        )
    }

    /// What the window does differently while this setting is on.
    pub const fn effect(self) -> &'static str {
        match self {
            Flag::ShowWorldsTab => "The rail carries a Worlds entry; off, the page is unreachable.",
            Flag::ShowScreenshotsTab => {
                "The rail carries a Screenshots entry; off, the page is unreachable."
            }
            Flag::HideRightSidebar => "The right panel is not drawn at all.",
            Flag::CompactInstanceCards => {
                "Instance cards drop their chip row and tighten up, fitting more per row."
            }
            Flag::ShowPlayTime => "Each instance card shows how long it has been played.",
            Flag::MinimizeOnLaunch => "The window is minimized as the game starts.",
            _ => "Stored, not yet read: the choice is remembered and the screen it describes has \
                  not been given the switch yet.",
        }
    }

    /// Resolve the id a pointer message carries back to the flag that owns it.
    ///
    /// The hover and press messages travel keyed by id to keep [`Message`] free
    /// of a twenty-arm enum per interaction, so this is what turns one back into
    /// a switch. An id that no longer belongs to anything — a pane that has been
    /// left, a switch removed in a later build — resolves to `None` and the
    /// pointer message is ignored rather than moving the wrong switch.
    pub fn from_id(id: &str) -> Option<Flag> {
        ALL_FLAGS.into_iter().find(|flag| flag.id() == id)
    }
}

/// Every flag, so [`Flag::from_id`] has one list to search.
pub const ALL_FLAGS: [Flag; 20] = [
    Flag::SyncThemeAcrossDevices,
    Flag::AdvancedRendering,
    Flag::NativeDecorations,
    Flag::ExternalLinksNewTab,
    Flag::ShowWorldsTab,
    Flag::ShowFilesTab,
    Flag::ShowScreenshotsTab,
    Flag::ShowAllScreenshotsInSidebar,
    Flag::ShowSkinSelectorInSidebar,
    Flag::QuickInstancesInSidebar,
    Flag::ShowJumpInSection,
    Flag::MinimizeOnLaunch,
    Flag::HideRightSidebar,
    Flag::CompactInstanceCards,
    Flag::ShowPlayTime,
    Flag::WarnUnknownModpacks,
    Flag::SkipNonEssentialWarnings,
    Flag::Telemetry,
    Flag::DiscordRpc,
    Flag::AlwaysShowCopyDetails,
];

/// A settings text field.
///
/// A field whose value is rendered straight from the parsed setting cannot be
/// *edited*: clearing it in order to retype leaves nothing to parse, and
/// re-rendering the parsed value puts the old number back under the caret. The
/// shell therefore keeps what the user is typing and writes the setting when it
/// parses, and this type is the key that draft is stored under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Field {
    MinMemory,
    MaxMemory,
    DefaultJavaPath,
    AppDirectory,
    ConcurrentDownloads,
    ConcurrentWrites,
    /// One Java major's binary, keyed by the major ("25", "21", "17", "8").
    JavaPath(&'static str),
}

impl Field {
    /// What the field shows before anything has been typed into it.
    pub fn initial(self, prefs: &Prefs) -> String {
        match self {
            Field::MinMemory => prefs.min_mem_mib().to_string(),
            Field::MaxMemory => prefs.max_mem_mib().to_string(),
            Field::DefaultJavaPath => prefs.default_java_path.clone().unwrap_or_default(),
            Field::AppDirectory => prefs.app_directory.clone().unwrap_or_default(),
            Field::ConcurrentDownloads => prefs.concurrent_downloads().to_string(),
            Field::ConcurrentWrites => prefs.concurrent_writes().to_string(),
            Field::JavaPath(major) => prefs.java_path(major).unwrap_or_default().to_string(),
        }
    }

    /// Write a typed value into a prefs file.
    ///
    /// A number that does not parse — including the empty string on the way
    /// through, while it is being retyped — leaves the setting alone rather than
    /// writing a zero, and a number *of* zero is refused for the same reason:
    /// six concurrent downloads replaced by none is not a preference, it is a
    /// broken launcher. A path field clears rather than storing a blank, because
    /// an empty path is not a path.
    pub fn commit(self, prefs: &mut Prefs, text: &str) {
        let trimmed = text.trim();
        let positive = || trimmed.parse::<u32>().ok().filter(|value| *value > 0);
        match self {
            Field::MinMemory => prefs.default_min_mem_mib = positive(),
            Field::MaxMemory => prefs.default_max_mem_mib = positive(),
            Field::ConcurrentDownloads => prefs.max_concurrent_downloads = positive(),
            Field::ConcurrentWrites => prefs.max_concurrent_writes = positive(),
            Field::DefaultJavaPath => {
                prefs.default_java_path = Some(trimmed.to_string()).filter(|v| !v.is_empty());
            }
            Field::AppDirectory => {
                prefs.app_directory = Some(trimmed.to_string()).filter(|v| !v.is_empty());
            }
            Field::JavaPath(major) => {
                if trimmed.is_empty() {
                    prefs.java_paths.remove(major);
                } else {
                    prefs.java_paths.insert(major.to_string(), trimmed.to_string());
                }
            }
        }
    }
}

// ---- The section list ----------------------------------------------------

/// One pane of the Settings dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Appearance,
    Features,
    Behavior,
    Language,
    FeatureFlags,
    Profile,
    Social,
    Privacy,
    SyncedSettings,
    Java,
    Resource,
}

impl Tab {
    /// Every tab, in the order the reference lists them.
    pub const ALL: [Tab; 11] = [
        Tab::Appearance,
        Tab::Features,
        Tab::Behavior,
        Tab::Language,
        Tab::FeatureFlags,
        Tab::Profile,
        Tab::Social,
        Tab::Privacy,
        Tab::SyncedSettings,
        Tab::Java,
        Tab::Resource,
    ];

    /// The pane's name in the section list.
    pub const fn label(self) -> &'static str {
        match self {
            Tab::Appearance => "Appearance",
            Tab::Features => "Features",
            Tab::Behavior => "Behavior",
            Tab::Language => "Language",
            Tab::FeatureFlags => "Feature flags",
            Tab::Profile => "Profile",
            Tab::Social => "Social",
            Tab::Privacy => "Privacy",
            Tab::SyncedSettings => "Synced settings",
            Tab::Java => "Java installations",
            Tab::Resource => "Resource management",
        }
    }

    /// The group heading this tab sits under. Neighbouring tabs that share a
    /// group print one heading between them, which is how the reference's
    /// `startsCategory` works.
    pub const fn category(self) -> &'static str {
        match self {
            Tab::Appearance | Tab::Features | Tab::Behavior | Tab::Language | Tab::FeatureFlags => {
                "Display"
            }
            Tab::Profile | Tab::Social | Tab::Privacy => "Account",
            Tab::SyncedSettings | Tab::Java | Tab::Resource => "Instances",
        }
    }

    /// The glyph key for the row's icon.
    pub const fn icon(self) -> &'static str {
        match self {
            Tab::Appearance => "paintbrush",
            Tab::Features => "bulb",
            Tab::Behavior => "options",
            Tab::Language => "globe",
            Tab::FeatureFlags => "switch",
            Tab::Profile => "person",
            Tab::Social => "heart",
            Tab::Privacy => "shield",
            Tab::SyncedSettings => "refresh",
            Tab::Java => "coffee",
            Tab::Resource => "gauge",
        }
    }

    /// Whether the tab only appears once developer mode is on, as the
    /// reference's own `developerOnly` flag does.
    pub const fn developer_only(self) -> bool {
        matches!(self, Tab::FeatureFlags)
    }
}

/// Secondary ink: `--color-secondary` in the reference.
fn muted() -> iced::theme::Text {
    iced::theme::Text::Color(theme::text_muted())
}

/// Title ink: `--color-contrast`.
fn strong() -> iced::theme::Text {
    iced::theme::Text::Color(theme::text())
}

/// Tertiary ink, one rung dimmer than [`muted`].
fn faint() -> iced::theme::Text {
    iced::theme::Text::Color(theme::text_dim())
}

/// Which switch the pointer is over, and which is held down.
///
/// [`Flag`]s rather than raw ids, so a message that outlived its pane cannot
/// address whatever moved into that slot.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pointer {
    pub hovered: Option<Flag>,
    pub pressed: Option<Flag>,
}

/// Everything a pane needs in order to draw itself.
///
/// Developer mode is deliberately *not* here: it decides which tabs the section
/// list contains, and that is [`nav`]'s parameter. A pane that could read it
/// would eventually draw a control for a tab that is not on screen.
pub struct View<'a> {
    pub prefs: &'a Prefs,
    pub anim: &'a SwitchAnim,
    pub pointer: Pointer,
    /// Text the user has typed into a field, where it differs from the setting.
    pub drafts: &'a BTreeMap<Field, String>,
    /// The account the Profile pane describes.
    pub account: Option<&'a AccountEntry>,
    /// Filter text for the Feature flags pane's search box.
    pub flag_filter: &'a str,
    /// This launcher's own directory: where these settings are written.
    pub home: &'a Path,
    /// The data root in use: where the instances, libraries and assets are.
    ///
    /// Read from the shell rather than from the setting, because the two differ
    /// exactly while a typed path is waiting to be applied — and the pane must
    /// describe the folder that is really holding the instances.
    pub data_root: &'a Path,
    /// A data root that has been recorded but could not be applied.
    pub data_root_pending: Option<String>,
}

// ---- The dialog ----------------------------------------------------------

/// Width of the Settings dialog: the reference's `min(928px, 95vw - 10rem)`,
/// resolved against the shell's own 980x640 minimum window.
pub const DIALOG_WIDTH: f32 = 928.0;
/// Width of the section list: the reference's `minmax(12.5rem, 18rem)` track.
pub const NAV_WIDTH: f32 = 236.0;
/// Height of the pane: the reference's `min-h-[min(65vh,600px)]`, resolved
/// against the minimum window so the dialog never grows past the screen.
pub const PANE_HEIGHT: f32 = 404.0;
/// Width of one color-theme card, so the four cards make a 2x2 grid that fills
/// the pane.
pub const THEME_CARD_WIDTH: f32 = 272.0;

/// The id of the pane's own scrollable.
///
/// The pane scrolls independently of the page behind the dialog, and the shell
/// keeps its position separately for the same reason: otherwise opening
/// Settings over a library scrolled halfway down would open the pane halfway
/// down too, and closing it would leave the library where the pane had got to.
pub fn scroll_id() -> scrollable::Id {
    scrollable::Id::new("settings-pane")
}

/// The pane's scrolling container, eased the way the pages are.
pub fn scroller<'a>(content: impl Into<Element<'a, Message>> + 'a) -> Element<'a, Message> {
    scrollable(scroll::guard(content, Message::SettingsWheel))
        .id(scroll_id())
        .on_scroll(|viewport: scrollable::Viewport| Message::SettingsScrolled {
            offset: viewport.absolute_offset().y,
            content_height: viewport.content_bounds().height,
            view_height: viewport.bounds().height,
        })
        .style(iced::theme::Scrollable::custom(theme::Thin))
        .width(Length::Fill)
        .height(Length::Fixed(PANE_HEIGHT))
        .into()
}

/// The section list down the left of the dialog.
///
/// Its own scrollable with its own id: the reference's sidebar scrolls
/// independently of the pane, and it must not share the pane's identity or a
/// wheel over one would move the other.
pub fn nav<'a>(open: Tab, developer_mode: bool) -> Element<'a, Message> {
    let mut list: Column<'a, Message> = column![].spacing(4);
    let mut last_category = "";
    for tab in Tab::ALL {
        if tab.developer_only() && !developer_mode {
            continue;
        }
        if tab.category() != last_category {
            last_category = tab.category();
            list = list.push(
                // `text-xs font-bold uppercase tracking-wide`, which the
                // reference draws in `--color-secondary`.
                container(
                    text(tab.category().to_uppercase())
                        .size(12)
                        .font(theme::bold())
                        .style(muted()),
                )
                .padding(Padding { top: 8.0, right: 16.0, bottom: 4.0, left: 16.0 }),
            );
        }
        list = list.push(nav_item(tab, tab == open));
    }
    scrollable(container(list).padding(Padding { top: 0.0, right: 16.0, bottom: 12.0, left: 0.0 }))
        .id(scrollable::Id::new("settings-nav"))
        .style(iced::theme::Scrollable::custom(theme::Thin))
        .width(Length::Fill)
        .height(Length::Fixed(PANE_HEIGHT))
        .into()
}

/// One row of the section list. A real button, unlike the labels it replaced.
fn nav_item<'a>(tab: Tab, active: bool) -> Element<'a, Message> {
    let ink = if active { theme::accent() } else { theme::text_muted() };
    hover_button(
        "nav_item:1",
        theme::nav_item(active),
        button(
            row![
                glyph(tab.icon(), 16.0, ink),
                text(tab.label()).size(14).font(theme::semibold()),
            ]
            .spacing(8)
            .align_items(Alignment::Center),
        )
        .on_press(Message::OpenSettingsTab(tab))
        .padding([8, 16])
        .width(Length::Fill),
    )
}

/// The version and platform line along the bottom of the dialog.
///
/// The product mark is a click target, not a button: six presses on it toggle
/// developer mode, which is how the reference reveals its hidden tab. It is
/// kept because a hidden tab with no way to reveal it is a tab nobody can reach,
/// and the version beside it is what a user is asked for when something goes
/// wrong — so it is read from the product and the OS rather than typed into the
/// source.
pub fn footer<'a>(developer_mode: bool) -> Element<'a, Message> {
    let platform = native::windows_version().unwrap_or_else(|| std::env::consts::OS.to_string());
    let mark = if developer_mode { theme::accent() } else { theme::text_muted() };
    let mut block: Column<'a, Message> = column![].spacing(6);
    if developer_mode {
        block = block.push(
            text("Developer mode enabled.")
                .size(13)
                .font(theme::semibold())
                .style(iced::theme::Text::Color(theme::accent())),
        );
    }
    block
        .push(
            row![
                hover_button(
                    "ghost:5",
                    theme::ghost(),
                    button(glyph("gear", 24.0, mark))
                            .on_press(Message::SettingsFooterPressed)
                            .padding([4, 6]),
                ),
                column![
                    text(format!("{} {}", brand::APP_NAME, brand::version()))
                        .size(13)
                        .style(muted()),
                    text(platform).size(13).style(muted()),
                ]
                .spacing(1),
                Space::with_width(Length::Fill),
            ]
            .spacing(12)
            .align_items(Alignment::Center),
        )
        .into()
}

/// The pane for a tab.
pub fn pane<'a>(tab: Tab, view: &View<'a>) -> Element<'a, Message> {
    let body = match tab {
        Tab::Appearance => appearance(view),
        Tab::Features => features(view),
        Tab::Behavior => behavior(view),
        Tab::Language => language(view),
        Tab::FeatureFlags => feature_flags(view),
        Tab::Profile => profile(view),
        Tab::Social => social(view),
        Tab::Privacy => privacy(view),
        Tab::SyncedSettings => synced_settings(view),
        Tab::Java => java(view),
        Tab::Resource => resource(view),
    };
    scroller(
        container(body)
            .padding(Padding { top: 2.0, right: 6.0, bottom: 12.0, left: 6.0 }),
    )
}

// ---- Row builders --------------------------------------------------------

/// A pane section: a 20px heading, its description, and the rows below it.
///
/// Between groups this draws the reference's `mt-8 border-t border-divider` —
/// 1px of `surface-5` — because a settings pane that runs its groups together
/// with only whitespace reads as one long list of unrelated switches.
fn section<'a>(
    heading: impl ToString,
    description: Option<String>,
    first: bool,
) -> Element<'a, Message> {
    // Annotated because a section can be built from nothing but text, which
    // leaves the column's message type to be inferred from the return type
    // alone. Naming it here means a section cannot fail to compile depending on
    // whether the row that follows happens to mention a message.
    let mut block: Column<'a, Message> = column![].spacing(4);
    if !first {
        block = block.push(
            container(text(""))
                .width(Length::Fill)
                .height(Length::Fixed(1.0))
                .style(theme::separator),
        );
        block = block.push(Space::with_height(Length::Fixed(20.0)));
    }
    block = block.push(text(heading).size(20).font(theme::semibold()).style(strong()));
    if let Some(description) = description {
        block = block.push(text(description).size(14).style(muted()));
    }
    block.into()
}

/// A setting row: title and description on the left, the control on the right.
///
/// `items-center` with `gap-4`, as the reference's own `AppearanceSettingRow`.
fn setting_row<'a>(
    title: impl ToString,
    description: impl ToString,
    control: Element<'a, Message>,
) -> Element<'a, Message> {
    row![
        column![
            text(title).size(18).font(theme::semibold()).style(strong()),
            text(description).size(14).style(muted()),
        ]
        .spacing(4)
        .width(Length::Fill),
        container(control).width(Length::Shrink),
    ]
    .spacing(16)
    .align_items(Alignment::Center)
    .into()
}

/// A row whose control cannot be offered, with the reason printed beneath it.
///
/// This is the shell's standing answer to a reference control it cannot honour:
/// the row is drawn, the switch is drawn *disabled*, and the text under it says
/// why the switch does not move. A switch that silently forgets would be worse
/// than no switch, and a row that vanished would be indistinguishable from a
/// bug.
fn unavailable<'a>(
    title: impl ToString,
    description: impl ToString,
    reason: impl ToString,
) -> Element<'a, Message> {
    row![
        column![
            text(title).size(18).font(theme::semibold()).style(strong()),
            text(description).size(14).style(muted()),
            text(reason).size(12).style(faint()),
        ]
        .spacing(4)
        .width(Length::Fill),
        switch_parts(0.0, 0.0, false, true),
    ]
    .spacing(16)
    .align_items(Alignment::Center)
    .into()
}

/// A short note under a control, in the dimmest ink.
fn note<'a>(value: impl ToString) -> Element<'a, Message> {
    text(value).size(12).style(faint()).into()
}

/// A row holding the switch for `flag`.
///
/// A flag the shell does not read yet is drawn the same way as one it can never
/// honour: a disabled switch with the reason under it. The alternative — a live
/// switch whose value nothing reads — is the one shape this dialog must not
/// contain, because it is indistinguishable from a working control until the
/// user notices nothing happened.
fn toggle_row<'a>(
    view: &View<'a>,
    title: impl ToString,
    description: impl ToString,
    flag: Flag,
) -> Element<'a, Message> {
    if !flag.wired() {
        return unavailable(title, description, flag.effect());
    }
    setting_row(title, description, switch(view, flag))
}

/// A setting row whose control is a text field bound to `field`.
fn field_row<'a>(
    view: &View<'a>,
    title: impl ToString,
    description: impl ToString,
    field: Field,
    wide: bool,
) -> Element<'a, Message> {
    let value = view
        .drafts
        .get(&field)
        .cloned()
        .unwrap_or_else(|| field.initial(view.prefs));
    setting_row(
        title,
        description,
        text_input("", &value)
            .on_input(move |text| Message::SettingsDraft(field, text))
            .style(theme::Field)
            .padding([10, 12])
            .width(Length::Fixed(if wide { 260.0 } else { 110.0 }))
            .into(),
    )
}

// ---- The switch ----------------------------------------------------------

/// The track's width, from the reference's `w-[48px]`.
pub const TRACK_WIDTH: f32 = 48.0;
/// The track's height, from `h-6`.
pub const TRACK_HEIGHT: f32 = 24.0;
/// The knob's diameter, from `w-4 h-4`.
pub const KNOB: f32 = 16.0;
/// The knob's diameter while the pointer is over the switch, from
/// `group-hover:w-[18px] group-hover:h-[18px]`.
pub const KNOB_HOVER: f32 = 18.0;
/// The knob's diameter while it is held down, from `group-active:w-[14px]`.
pub const KNOB_PRESSED: f32 = 14.0;
/// The gap between the track's edge and the knob, from `p-1`.
pub const INSET: f32 = 4.0;

/// Where the knob sits and how wide it is, for a given progress.
///
/// Kept apart from the widgets so that the one thing about a switch which has to
/// be exactly right — that it lands on both ends of its own track — is a pure
/// function with a test, rather than something only a slow-motion video could
/// check.
///
/// The knob *swells in place*, which is what `m-[-1px]` does in the reference:
/// growing it from 16 to 18 moves its leading edge one pixel back, so the centre
/// stays put and the switch does not appear to lurch sideways as the pointer
/// arrives.
pub fn switch_geometry(progress: f32, diameter: f32) -> (f32, f32) {
    // A NaN would propagate into a layout size, which iced asserts against, so
    // it is folded to zero here rather than at the call site.
    let progress = if progress.is_nan() { 0.0 } else { progress.clamp(0.0, 1.0) };
    let (off, on) = (INSET + KNOB / 2.0, TRACK_WIDTH - INSET - KNOB / 2.0);
    let centre = off + (on - off) * progress;
    (centre - diameter / 2.0, diameter)
}

/// The two colours a switch is painted with, at a given progress.
///
/// Off is `--color-button-bg` (`surface-4`) and on is `--color-brand`, so the
/// transition is the track recolouring as the knob crosses it — which is what
/// `transition-all duration-200` is doing in the reference, and why a switch
/// that only moved its knob would be a different component.
pub fn switch_colors(progress: f32) -> (Color, Color) {
    let progress = if progress.is_nan() { 0.0 } else { progress.clamp(0.0, 1.0) };
    let track = theme::mix(theme::surface_input(), theme::accent(), progress);
    // `bg-secondary` off, `bg-black/90` on. The on colour is opaque here on
    // purpose: the track beneath it is already the brand green, so compositing
    // 90% black over it would give a dark green rather than the near-black the
    // reference shows.
    let knob = theme::mix(theme::text_dim(), Color::from_rgba(0.0, 0.0, 0.0, 1.0), progress);
    (track, knob)
}

/// The switch for `flag`, drawn at whatever point its slide has reached.
fn switch<'a>(view: &View<'a>, flag: Flag) -> Element<'a, Message> {
    let on = flag.get(view.prefs);
    let progress = view.anim.progress(flag.id(), on);
    let hovered = view.pointer.hovered == Some(flag);
    let pressed = view.pointer.pressed == Some(flag);
    // The knob's growth is a tween like every other interaction in the shell:
    // the fraction comes from the interaction clock that `SwitchHover` and
    // `SwitchLeft` start, so the knob swells over 150 ms instead of appearing
    // two pixels wider on the frame the pointer arrives. A clock that cannot be
    // read is the end state, which is what this control showed before the
    // growth was tweened at all.
    let grow = match crate::anim::clock().lock() {
        Ok(clock) => clock.hover_progress(flag.id(), hovered, pressed),
        Err(_) => {
            if hovered {
                1.0
            } else {
                0.0
            }
        }
    };
    let track = switch_parts(progress, grow, pressed, false);
    with_pointer(track, flag)
}

/// The track and knob of a switch, at a given progress and pointer state.
fn switch_parts<'a>(
    progress: f32,
    grow: f32,
    pressed: bool,
    disabled: bool,
) -> Element<'a, Message> {
    // Hover grows the knob and a press shrinks it — `group-hover` and
    // `group-active` in the reference, which is most of what makes the control
    // feel alive under the hand. `grow` is the *tweened* fraction of that
    // growth rather than the pointer state, so the knob swells into its hover
    // size instead of jumping to it.
    let diameter = if pressed {
        KNOB_PRESSED
    } else {
        KNOB + (KNOB_HOVER - KNOB) * grow.clamp(0.0, 1.0)
    };
    let (leading, diameter) = switch_geometry(progress, diameter);
    let trailing = TRACK_WIDTH - leading - diameter;

    let (track, knob) = switch_colors(progress);
    let (track, knob) = if disabled {
        (theme::alpha(track, 0.5), theme::alpha(knob, 0.5))
    } else {
        (track, knob)
    };

    let radius = TRACK_HEIGHT / 2.0;
    let knob = container(text(""))
        .width(Length::Fixed(diameter))
        .height(Length::Fixed(diameter))
        .style(theme::plate(knob, radius));

    container(
        row![
            Space::with_width(Length::Fixed(leading)),
            knob,
            Space::with_width(Length::Fixed(trailing)),
        ]
        .align_items(Alignment::Center),
    )
    .width(Length::Fixed(TRACK_WIDTH))
    .height(Length::Fixed(TRACK_HEIGHT))
    .style(move |_: &Theme| container::Appearance {
        background: Some(track.into()),
        border: Border { radius: radius.into(), width: 1.0, color: theme::border_strong() },
        ..Default::default()
    })
    .into()
}

/// Make a switch clickable: press, release, and the hover that grows the knob.
fn with_pointer<'a>(widget: Element<'a, Message>, flag: Flag) -> Element<'a, Message> {
    MouseArea::new(widget)
        .on_enter(Message::SwitchHover(flag.id()))
        .on_exit(Message::SwitchLeft(flag.id()))
        .on_press(Message::SwitchDown(flag.id()))
        // The toggle lands on *release*, which is what a button does: a press
        // that is dragged off the switch and let go must change nothing.
        .on_release(Message::ToggleFlag(flag))
        .interaction(iced::mouse::Interaction::Pointer)
        .into()
}

// ---- Panes ---------------------------------------------------------------

/// Display > Appearance.
fn appearance<'a>(view: &View<'a>) -> Element<'a, Message> {
    let current = theme::color_theme();
    let mut cards: Column<'a, Message> = column![].spacing(12);
    // [`ColorTheme::PAINTED`] and not [`ColorTheme::ALL`]: this pane paints each
    // card from the hand-written palette, which has no retro, so offering retro
    // here would offer a card that looked like Dark. The five-theme list, with the
    // reference's own dev-mode rule for retro, is the Settings modal's.
    for couple in ColorTheme::PAINTED.chunks(2) {
        let mut line = row![].spacing(12);
        for choice in couple {
            line = line.push(theme_choice(*choice, *choice == current));
        }
        cards = cards.push(line);
    }

    column![
        section(
            "Color theme",
            Some(format!(
                "Select your preferred color theme across {}.",
                brand::APP_NAME
            )),
            true,
        ),
        Space::with_height(Length::Fixed(16.0)),
        cards,
        Space::with_height(Length::Fixed(24.0)),
        unavailable(
            "Sync theme across devices",
            "Use this theme everywhere you're signed in. Turn this off to keep a separate theme on \
             this device.",
            "Not available yet: syncing needs a Modrinth account, and this launcher signs in to \
             Microsoft only.",
        ),
        Space::with_height(Length::Fixed(24.0)),
        section("Rendering", Some("How the shell draws itself on this machine.".to_string()), false),
        Space::with_height(Length::Fixed(16.0)),
        toggle_row(
            view,
            "Advanced rendering",
            "Enables advanced rendering such as blur effects that may cause performance issues \
             without hardware-accelerated rendering.",
            Flag::AdvancedRendering,
        ),
        Space::with_height(Length::Fixed(20.0)),
        toggle_row(
            view,
            "Open external links in new tab",
            "Make links which go outside the launcher open in the default browser instead of \
             inside the shell.",
            Flag::ExternalLinksNewTab,
        ),
        Space::with_height(Length::Fixed(20.0)),
        toggle_row(
            view,
            "System window frame",
            "Use your operating system's title bar and window controls. Requires an app restart.",
            Flag::NativeDecorations,
        ),
    ]
    .spacing(0)
    .into()
}

/// Display > Features.
fn features<'a>(view: &View<'a>) -> Element<'a, Message> {
    column![
        section("Instances", Some("Which tabs an instance shows.".to_string()), true),
        Space::with_height(Length::Fixed(16.0)),
        toggle_row(
            view,
            "Show Worlds tab",
            "List the worlds saved inside an instance.",
            Flag::ShowWorldsTab
        ),
        Space::with_height(Length::Fixed(20.0)),
        toggle_row(
            view,
            "Show Files tab",
            "Browse and open the instance's own folders.",
            Flag::ShowFilesTab
        ),
        Space::with_height(Length::Fixed(20.0)),
        toggle_row(
            view,
            "Show Screenshots tab",
            "Show the screenshots taken in this instance.",
            Flag::ShowScreenshotsTab
        ),
        Space::with_height(Length::Fixed(24.0)),
        section("Sidebar", Some("Entries in the left rail.".to_string()), false),
        Space::with_height(Length::Fixed(16.0)),
        toggle_row(
            view,
            "Show all screenshots",
            "Let the Screenshots page show every instance at once rather than only the selected one.",
            Flag::ShowAllScreenshotsInSidebar
        ),
        Space::with_height(Length::Fixed(20.0)),
        unavailable(
            "Show skin selector in sidebar",
            "Show a button in the left sidebar to open the skin selector.",
            "Not available yet: changing a skin needs a Microsoft account with a purchased \
             profile, which the device-code sign-in does not grant.",
        ),
        Space::with_height(Length::Fixed(20.0)),
        toggle_row(
            view,
            "Quick instances in sidebar",
            "Put your most recently played instances at the foot of the rail.",
            Flag::QuickInstancesInSidebar
        ),
        Space::with_height(Length::Fixed(24.0)),
        section(
            "Play page",
            Some("What the home page shows above the library.".to_string()),
            false
        ),
        Space::with_height(Length::Fixed(16.0)),
        toggle_row(
            view,
            "Show Jump back in section",
            "Show recently played worlds and instances at the top of the Play page.",
            Flag::ShowJumpInSection
        ),
    ]
    .spacing(0)
    .into()
}

/// Display > Behavior.
fn behavior<'a>(view: &View<'a>) -> Element<'a, Message> {
    column![
        section("Startup and navigation", None, true),
        Space::with_height(Length::Fixed(16.0)),
        toggle_row(
            view,
            "Minimize app",
            format!("Minimize {} when Minecraft starts.", brand::APP_NAME),
            Flag::MinimizeOnLaunch
        ),
        Space::with_height(Length::Fixed(20.0)),
        toggle_row(
            view,
            "Hide right sidebar",
            "Hide the right sidebar by default and add a button to show or hide it.",
            Flag::HideRightSidebar
        ),
        Space::with_height(Length::Fixed(24.0)),
        section("Home and content", None, false),
        Space::with_height(Length::Fixed(16.0)),
        toggle_row(
            view,
            "Compact mode",
            "Display library instances in a compact row layout.",
            Flag::CompactInstanceCards
        ),
        Space::with_height(Length::Fixed(20.0)),
        toggle_row(
            view,
            "Show play time",
            "Show how long you have played each instance.",
            Flag::ShowPlayTime
        ),
        Space::with_height(Length::Fixed(24.0)),
        section("Confirmations", None, false),
        Space::with_height(Length::Fixed(16.0)),
        toggle_row(
            view,
            "Warn me before installing unknown modpacks",
            "Ask before a modpack from outside Modrinth is installed, since its contents cannot be \
             checked against anything.",
            Flag::WarnUnknownModpacks
        ),
        Space::with_height(Length::Fixed(20.0)),
        toggle_row(
            view,
            "Skip non-essential warnings",
            "Do not stop for warnings that would not change the outcome of what you asked for.",
            Flag::SkipNonEssentialWarnings
        ),
    ]
    .spacing(0)
    .into()
}

/// Display > Language.
///
/// One language ships, so one row is drawn selected and the rest of the list is
/// absent. A menu of translations that do not exist would be a menu of lies.
fn language<'a>(view: &View<'a>) -> Element<'a, Message> {
    column![
        section(
            "Language",
            Some("Choose the language the interface is drawn in.".to_string()),
            true
        ),
        Space::with_height(Length::Fixed(16.0)),
        container(
            row![
                container(text(""))
                    .width(Length::Fixed(16.0))
                    .height(Length::Fixed(16.0))
                    .style(theme::plate(theme::accent(), 8.0)),
                text("English (United States)")
                    .size(16)
                    .font(theme::medium())
                    .style(strong()),
                Space::with_width(Length::Fill),
                text("en-US").size(14).style(muted()),
            ]
            .spacing(12)
            .align_items(Alignment::Center),
        )
        .style(theme::option_row)
        .padding([12, 14])
        .width(Length::Fill),
        Space::with_height(Length::Fixed(12.0)),
        note(
            "Only this language ships in the current build, so the list is one entry long rather \
             than a menu of translations that are not there.",
        ),
        Space::with_height(Length::Fixed(8.0)),
        note(format!("Stored choice: {}", view.prefs.locale())),
    ]
    .spacing(0)
    .into()
}

/// Display > Feature flags (developer mode only).
///
/// The reference filters a list of runtime flags over a search box. This
/// launcher has no flag system, so the honest pane is that search box over an
/// empty list — which is exactly what the reference draws when a filter matches
/// nothing, so the shape is still right.
fn feature_flags<'a>(view: &View<'a>) -> Element<'a, Message> {
    column![
        section(
            "Feature flags",
            Some("Experimental switches, hidden behind developer mode.".to_string()),
            true
        ),
        Space::with_height(Length::Fixed(16.0)),
        text_input("Search…", view.flag_filter)
            .on_input(Message::FlagFilterChanged)
            .style(theme::Field)
            .padding([10, 12])
            .width(Length::Fill),
        Space::with_height(Length::Fixed(24.0)),
        container(
            text("No feature flags found")
                .size(14)
                .style(muted()),
        )
        .width(Length::Fill)
        .center_x(),
        Space::with_height(Length::Fixed(12.0)),
        note(
            "Developer mode is on, which is why this tab is visible. No experimental flags are \
             registered in this build yet.",
        ),
    ]
    .spacing(0)
    .into()
}

/// Account > Profile: the account the game launches as.
fn profile<'a>(view: &View<'a>) -> Element<'a, Message> {
    let mut body = column![section(
        "Profile",
        Some("The account the game launches as.".to_string()),
        true
    )]
    .spacing(0)
    .push(Space::with_height(Length::Fixed(16.0)));

    body = match view.account {
        Some(account) => body
            .push(
                container(
                    row![
                        glyph("person", 28.0, theme::accent()),
                        column![
                            text(account.username.clone())
                                .size(18)
                                .font(theme::semibold())
                                .style(strong()),
                            text(format!("{} · {}", account.kind.label(), account.uuid))
                                .size(12)
                                .style(faint()),
                        ]
                        .spacing(2)
                        .width(Length::Fill),
                    ]
                    .spacing(14)
                    .align_items(Alignment::Center),
                )
                .style(theme::card)
                .padding(16)
                .width(Length::Fill),
            )
            .push(Space::with_height(Length::Fixed(20.0)))
            .push(note(
                "An offline account is local only: the UUID above is derived from the name the way \
                 Java derives it, so the same name is the same player in every launcher.",
            )),
        None => body
            .push(
                container(
                    column![
                        text("No account yet").size(18).font(theme::semibold()).style(strong()),
                        text(
                            "Sign in to Microsoft to play online, or add an offline account from the \
                             Accounts page.",
                        )
                        .size(14)
                        .style(muted()),
                    ]
                    .spacing(6),
                )
                .style(theme::inset)
                .padding(16)
                .width(Length::Fill),
            )
            .push(Space::with_height(Length::Fixed(20.0)))
            .push(
                row![
                    Space::with_width(Length::Fill),
                    hover_button(
                        "primary:14",
                        theme::primary(),
                        button(text("Sign in to Microsoft").size(14).font(theme::semibold()))
                                .on_press(Message::MicrosoftPressed)
                                .padding([10, 18]),
                    ),
                ]
                .width(Length::Fill),
            ),
    };
    body.into()
}

/// Account > Social.
fn social<'a>(_view: &View<'a>) -> Element<'a, Message> {
    column![
        section("Social", Some("People you have blocked.".to_string()), true),
        Space::with_height(Length::Fixed(16.0)),
        container(
            column![
                text("No blocked users").size(18).font(theme::semibold()).style(strong()),
                text(
                    "Blocking belongs to a Modrinth account, and this launcher signs in to \
                     Microsoft, so there is nobody here to unblock.",
                )
                .size(14)
                .style(muted()),
            ]
            .spacing(6),
        )
        .style(theme::inset)
        .padding(16)
        .width(Length::Fill),
    ]
    .spacing(0)
    .into()
}

/// Account > Privacy.
fn privacy<'a>(view: &View<'a>) -> Element<'a, Message> {
    column![
        section(
            "Privacy",
            Some("What this launcher does with your data.".to_string()),
            true
        ),
        Space::with_height(Length::Fixed(16.0)),
        unavailable(
            "Telemetry",
            "Send anonymous counts of which features are used.",
            "Off, and not offered: this launcher has no analytics endpoint and sends nothing, so \
             there is no switch to turn.",
        ),
        Space::with_height(Length::Fixed(20.0)),
        unavailable(
            "Discord Rich Presence",
            "Announce the running game and instance over Discord's local socket.",
            "Not implemented yet, so nothing is sent to Discord. The preference is stored, so a \
             build that adds it can honour it.",
        ),
        Space::with_height(Length::Fixed(20.0)),
        toggle_row(
            view,
            "Always show copy details",
            "Keep the full details of a copy on screen rather than behind a hover.",
            Flag::AlwaysShowCopyDetails,
        ),
        Space::with_height(Length::Fixed(24.0)),
        note(
            "Everything the launcher writes lives under the data root named on the Resource \
             management tab. Nothing is uploaded.",
        ),
    ]
    .spacing(0)
    .into()
}

/// Instances > Synced settings: what an instance starts from.
fn synced_settings<'a>(view: &View<'a>) -> Element<'a, Message> {
    column![
        section(
            "Defaults for instances",
            Some(
                "What a new instance starts with, and what a launch uses when the instance \
                 overrides nothing."
                    .to_string(),
            ),
            true
        ),
        Space::with_height(Length::Fixed(16.0)),
        field_row(
            view,
            "Memory floor",
            "Heap floor a launch uses, in MiB, unless the instance overrides memory.",
            Field::MinMemory,
            false
        ),
        Space::with_height(Length::Fixed(20.0)),
        field_row(
            view,
            "Memory ceiling",
            "Heap ceiling a launch uses, in MiB, unless the instance overrides memory.",
            Field::MaxMemory,
            false
        ),
        Space::with_height(Length::Fixed(20.0)),
        field_row(
            view,
            "Java binary",
            "Java a new instance starts with, and what a version that names no Java of its own \
             runs on. Empty means find one on this machine, and, failing that, download the one \
             the version asks for.",
            Field::DefaultJavaPath,
            true
        ),
        Space::with_height(Length::Fixed(24.0)),
        note(
            "These are this launcher's own defaults, kept in palantirmc-desktop.json. On a machine \
             where Prism already has memory settings they start from Prism's numbers — read once, \
             never rewritten — so adopting an install does not change its heap until a value is \
             typed here, and assigning one does not change what Prism itself would do.",
        ),
    ]
    .spacing(0)
    .into()
}

/// Instances > Java installations.
fn java<'a>(view: &View<'a>) -> Element<'a, Message> {
    const MAJORS: [&str; 4] = ["25", "21", "17", "8"];
    let mut body = column![section(
        "Java installations",
        Some(
            "Which Java to run each major version with. Empty means find one when it is needed."
                .to_string(),
        ),
        true
    )]
    .spacing(0);

    for (index, major) in MAJORS.iter().enumerate() {
        body = body.push(Space::with_height(Length::Fixed(if index == 0 { 16.0 } else { 24.0 })));
        body = body.push(
            column![
                text(format!("Java {major} location"))
                    .size(18)
                    .font(theme::semibold())
                    .style(strong()),
                field_row(
                    view,
                    "",
                    "Path to javaw.exe. Empty means search for it on PATH when it is needed.",
                    Field::JavaPath(major),
                    true
                ),
            ]
            .spacing(8)
            .width(Length::Fill),
        );
    }

    body.push(Space::with_height(Length::Fixed(24.0)))
        .push(note(
            "A version left empty is found automatically when an instance that needs it is \
             launched: the Java the instance itself names, then a path set here for one of the \
             majors the version accepts, then the Java binary on the Synced settings tab — which \
             is also the Java a new instance starts with — then the runtimes under the data \
             root's java folder, JAVA_HOME, the standard install locations and PATH, preferring \
             one whose version matches what the instance asks for. A path that is gone, or that \
             names a major this version cannot run on, is looked past rather than used. If \
             nothing on this machine matches, the runtime the version names is downloaded from \
             the metadata service and unpacked under the data root's java folder. A path set here \
             is used for that major and no other.",
        ))
        .into()
}

/// Instances > Resource management.
///
/// Two directories, both named: the data root the instances are read from, and
/// this launcher's own folder where these settings are written. They are
/// separate on purpose — the data root can be an install another launcher
/// created, while the settings have to live somewhere that is ours — and a pane
/// that showed only one of them is how somebody ends up unable to find either.
fn resource<'a>(view: &View<'a>) -> Element<'a, Message> {
    let mut body = column![
        section(
            "App directory",
            Some("Where instances, caches and downloads live.".to_string()),
            true
        ),
        Space::with_height(Length::Fixed(16.0)),
        container(
            row![
                column![
                    text("Current directory").size(18).font(theme::semibold()).style(strong()),
                    text(view.data_root.display().to_string()).size(13).style(muted()),
                ]
        .spacing(4)
        .width(Length::Fill),
        hover_button(
            "secondary:30",
            theme::secondary(),
            button(text("Open folder").size(13).font(theme::semibold()))
                    .on_press(Message::OpenDataRoot)
                    .padding([10, 14]),
        ),
            ]
            .spacing(16)
            .align_items(Alignment::Center),
        )
        .width(Length::Fill),
        Space::with_height(Length::Fixed(14.0)),
        setting_row(
            "Settings directory",
            "This launcher's own preferences, kept beside nothing else: it does not move with \
             the data root, so where the game data lives is remembered independently of it.",
            text(view.home.display().to_string()).size(12).style(muted()).into(),
        ),
        Space::with_height(Length::Fixed(16.0)),
        field_row(
            view,
            "Use a different directory",
            "An absolute path, or empty for this launcher's own folder. It takes effect as soon \
             as it names a directory that exists.",
            Field::AppDirectory,
            true
        ),
        Space::with_height(Length::Fixed(24.0)),
        section("Downloads", Some("How much work the launcher does at once.".to_string()), false),
        Space::with_height(Length::Fixed(16.0)),
        field_row(
            view,
            "Maximum concurrent downloads",
            "How many files may be fetched in parallel. Six is the shipped default.",
            Field::ConcurrentDownloads,
            false
        ),
        Space::with_height(Length::Fixed(20.0)),
        field_row(
            view,
            "Maximum concurrent writes",
            "How many files may be written to disk at once. Six is the shipped default.",
            Field::ConcurrentWrites,
            false
        ),
        Space::with_height(Length::Fixed(24.0)),
        section("Diagnostics", Some("What the log keeps, and how it is shown.".to_string()), false),
        Space::with_height(Length::Fixed(16.0)),
        toggle_row(
            view,
            "Always show copy details",
            "Keep the full details of a copy on screen rather than behind a hover.",
            Flag::AlwaysShowCopyDetails,
        ),
        Space::with_height(Length::Fixed(20.0)),
        setting_row(
            "App cache",
            "Cached component metadata, re-fetched on demand. Cleaning it cannot lose an \
             instance: the files it holds are all downloadable again.",
            Element::from(
                hover_button(
                    "destructive:8",
                    theme::destructive(),
                    button(text("Purge cache").size(13).font(theme::semibold()))
                            .on_press(Message::PurgeCache)
                            .padding([10, 14]),
                ),
            ),
        ),
    ]
    .spacing(0);

    // A path that has been typed but could not be applied is said out loud.
    // The alternative — showing it as the current directory — is a pane that
    // claims instances live somewhere they do not, which is worse than the
    // setting not working yet.
    if let Some(pending) = &view.data_root_pending {
        body = body.push(Space::with_height(Length::Fixed(12.0))).push(note(format!(
            "Recorded but not in use: {pending} is not a directory that is there, so the \
             instances above are still the ones being read. Create it and this launcher picks it \
             up, or clear the field to come back here."
        )));
    }

    body.into()
}

// ---- The color-theme cards ----------------------------------------------

/// One color-theme card: a miniature of the theme, then its name and a radio.
///
/// The miniature is painted *from that theme's palette*, not from a picture of
/// it, so a card cannot promise something the theme does not deliver — the Light
/// card is white because choosing Light really does paint white, and the OLED
/// card is black because it really is black.
fn theme_choice<'a>(choice: ColorTheme, selected: bool) -> Element<'a, Message> {
    let palette = choice.palette();
    let preview = container(
        row![
            container(text(""))
                .width(Length::Fixed(30.0))
                .height(Length::Fixed(30.0))
                .style(theme::plate(palette.bg_rail, 8.0)),
            column![
                bar(96.0, 9.0, theme::alpha(palette.text, 0.85)),
                bar(62.0, 7.0, theme::alpha(palette.text, 0.45)),
            ]
            .spacing(8),
        ]
        .spacing(12)
        .align_items(Alignment::Center),
    )
    .style(move |_: &Theme| container::Appearance {
        background: Some(palette.bg.into()),
        border: Border { radius: 8.0.into(), width: 1.0, color: palette.border },
        ..Default::default()
    })
    .padding(14)
    .width(Length::Fill)
    .height(Length::Fixed(84.0));

    let radio = container(text(""))
        .width(Length::Fixed(15.0))
        .height(Length::Fixed(15.0))
        .style(move |_: &Theme| container::Appearance {
            background: selected.then(|| theme::accent().into()),
            border: Border {
                radius: 999.0.into(),
                width: 2.0,
                color: if selected { theme::accent() } else { theme::text_dim() },
            },
            ..Default::default()
        });

    let label = row![
        radio,
        text(choice.label()).size(12).style(iced::theme::Text::Color(if selected {
            theme::accent()
        } else {
            theme::text()
        })),
    ]
    .spacing(8)
    .align_items(Alignment::Center);

    hover_button(
        "theme_card:1",
        theme::theme_card(selected),
        button(column![preview, label].spacing(10).width(Length::Fill))
                .on_press(Message::SetColorTheme(choice))
                .padding(10)
                .width(Length::Fixed(THEME_CARD_WIDTH)),
    )}

/// One line of fake text in a theme miniature.
fn bar<'a>(width: f32, height: f32, color: Color) -> Element<'a, Message> {
    container(text(""))
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .style(theme::plate(color, height / 2.0))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_flag_id_is_unique() {
        // The id is the animation key and the pointer payload, so two flags
        // sharing one would move each other's switches.
        let mut seen = std::collections::BTreeSet::new();
        for flag in ALL_FLAGS {
            assert!(seen.insert(flag.id()), "duplicate id: {}", flag.id());
        }
        assert_eq!(seen.len(), ALL_FLAGS.len());
    }

    #[test]
    fn every_flag_round_trips_through_its_id() {
        for flag in ALL_FLAGS {
            assert_eq!(Flag::from_id(flag.id()), Some(flag), "{}", flag.id());
        }
        assert_eq!(Flag::from_id("no.such.flag"), None);
        assert_eq!(Flag::from_id(""), None);
    }

    #[test]
    fn a_flag_writes_what_it_reads() {
        // The property that makes one message arm safe for twenty switches: if a
        // setter wrote a different field than its getter reads, the switch would
        // appear to move and then spring back on the next repaint.
        for flag in ALL_FLAGS {
            let mut prefs = Prefs::default();
            let before = flag.get(&prefs);
            flag.set(&mut prefs, !before);
            assert_eq!(flag.get(&prefs), !before, "{}", flag.id());
            flag.set(&mut prefs, before);
            assert_eq!(flag.get(&prefs), before, "{}", flag.id());
        }
    }

    #[test]
    fn one_flag_does_not_write_another() {
        let others = || {
            ALL_FLAGS
                .into_iter()
                .filter(|flag| *flag != Flag::ShowFilesTab)
                .map(|flag| flag.get(&Prefs::default()))
        };
        let mut prefs = Prefs::default();
        let before: Vec<bool> = others().collect();
        // Read first, then write: `set` takes the same `prefs` mutably, and the
        // one-line form does not borrow-check.
        let flipped = !Flag::ShowFilesTab.get(&prefs);
        Flag::ShowFilesTab.set(&mut prefs, flipped);
        let after: Vec<bool> = ALL_FLAGS
            .into_iter()
            .filter(|flag| *flag != Flag::ShowFilesTab)
            .map(|flag| flag.get(&prefs))
            .collect();
        assert_eq!(before, after, "a switch moved another switch");
    }

    #[test]
    fn the_knob_lands_on_both_ends_of_its_track() {
        // The measured property: off, the knob's leading edge is the track's own
        // padding; on, its trailing edge is. A switch that stops short of either
        // end reads as broken at a glance, and an off-by-one here is invisible in
        // a still frame but obvious in motion.
        let (off, diameter) = switch_geometry(0.0, KNOB);
        assert_eq!(off, INSET, "the knob must sit on the track's padding when off");
        assert_eq!(off + diameter, INSET + KNOB);

        let (on, diameter) = switch_geometry(1.0, KNOB);
        assert_eq!(on, TRACK_WIDTH - INSET - KNOB);
        assert_eq!(on + diameter, TRACK_WIDTH - INSET, "…and touch the far padding when on");
    }

    #[test]
    fn the_knob_swells_in_place_rather_than_lurching() {
        // `m-[-1px]` in the reference: growing the knob moves its leading edge
        // back by exactly what it grew, so the centre is fixed. Without this the
        // switch appears to jump sideways as the pointer arrives.
        let centre = |leading: f32, diameter: f32| leading + diameter / 2.0;
        for progress in [0.0_f32, 0.25, 0.5, 0.75, 1.0] {
            let (rest, _) = switch_geometry(progress, KNOB);
            let expected = centre(rest, KNOB);
            for (leading, diameter) in [
                switch_geometry(progress, KNOB_HOVER),
                switch_geometry(progress, KNOB_PRESSED),
            ] {
                assert!(
                    (centre(leading, diameter) - expected).abs() < 1e-4,
                    "the centre moved at progress {progress}"
                );
            }
        }
    }

    #[test]
    fn the_geometry_is_bounded_whatever_it_is_handed() {
        // An easing overshoot, a stale value and a NaN all reach here, and a NaN
        // would reach iced as a layout size, which it rejects.
        for progress in [-5.0_f32, -0.001, 1.001, 9.0, f32::NAN] {
            let (leading, diameter) = switch_geometry(progress, KNOB);
            assert!(leading.is_finite(), "a non-finite leading edge at {progress}");
            assert!(leading >= -0.001, "leading edge escaped at {progress}");
            assert!(
                leading + diameter <= TRACK_WIDTH + 0.001,
                "trailing edge escaped at {progress}"
            );
            assert!(
                TRACK_WIDTH - leading - diameter >= -0.001,
                "trailing space went negative at {progress}"
            );
        }
    }

    #[test]
    fn the_track_recolours_as_the_knob_crosses_it() {
        // Off is the button surface and on is the brand green, so the transition
        // is *both* things moving — which is what the reference's
        // `transition-all` does. A switch that only moved its knob would be a
        // different component.
        let (off_track, off_knob) = switch_colors(0.0);
        assert_eq!(off_track, theme::surface_input());
        assert_eq!(off_knob, theme::text_dim());

        let (on_track, on_knob) = switch_colors(1.0);
        assert_eq!(on_track, theme::accent());
        assert!(on_knob.r < 0.01 && on_knob.a > 0.99, "the on knob is `bg-black/90`");

        // Halfway is strictly between, so the blend is a blend rather than a
        // snap from one end to the other.
        let (mid_track, mid_knob) = switch_colors(0.5);
        let between = |value: f32, a: f32, b: f32| value > a.min(b) && value < a.max(b);
        assert!(between(mid_track.g, off_track.g, on_track.g), "track did not blend");
        assert!(between(mid_knob.g, off_knob.g, on_knob.g), "knob did not blend");
    }

    #[test]
    fn a_number_field_refuses_to_write_nonsense() {
        // The draft buffer means the empty string reaches `commit` on the way
        // through, and a zero is one keystroke away from a launcher that cannot
        // download anything.
        let mut prefs = Prefs::default();
        for text in ["", " ", "abc", "0", "-4", "6.5"] {
            Field::ConcurrentDownloads.commit(&mut prefs, text);
            assert_eq!(
                prefs.concurrent_downloads(),
                crate::prefs::DEFAULT_CONCURRENT_DOWNLOADS,
                "`{text}` should not have been written"
            );
        }
        Field::ConcurrentDownloads.commit(&mut prefs, "12");
        assert_eq!(prefs.concurrent_downloads(), 12);
    }

    #[test]
    fn a_path_field_clears_rather_than_storing_a_blank() {
        let mut prefs = Prefs::default();
        Field::DefaultJavaPath.commit(&mut prefs, "C:/jdk/bin/javaw.exe");
        assert_eq!(prefs.default_java_path.as_deref(), Some("C:/jdk/bin/javaw.exe"));
        Field::DefaultJavaPath.commit(&mut prefs, "   ");
        assert_eq!(prefs.default_java_path, None, "a blank path is no path");
    }

    #[test]
    fn a_java_path_is_removed_rather_than_blanked() {
        // The Java pane's fields are per major version, and an empty one has to
        // leave the map rather than put an empty string in it — a blank entry
        // would read back as a chosen path.
        let mut prefs = Prefs::default();
        Field::JavaPath("21").commit(&mut prefs, "C:/jdk21/bin/javaw.exe");
        assert_eq!(prefs.java_path("21"), Some("C:/jdk21/bin/javaw.exe"));
        Field::JavaPath("21").commit(&mut prefs, "");
        assert_eq!(prefs.java_path("21"), None);
        assert!(prefs.java_paths.is_empty(), "an empty field must not leave a key behind");
    }

    #[test]
    fn every_field_has_something_to_show_before_anything_is_typed() {
        let prefs = Prefs::default();
        for field in [
            Field::MinMemory,
            Field::MaxMemory,
            Field::DefaultJavaPath,
            Field::AppDirectory,
            Field::ConcurrentDownloads,
            Field::ConcurrentWrites,
            Field::JavaPath("17"),
        ] {
            // The two path fields legitimately show nothing; the rest must not
            // show an empty box where a default number belongs.
            let shown = field.initial(&prefs);
            let is_path = matches!(
                field,
                Field::DefaultJavaPath | Field::AppDirectory | Field::JavaPath(_)
            );
            assert!(is_path || !shown.is_empty(), "{field:?} showed nothing");
        }
        assert_eq!(Field::ConcurrentDownloads.initial(&prefs), "6");
        assert_eq!(Field::MaxMemory.initial(&prefs), "4096");
    }

    #[test]
    fn every_tab_has_a_label_a_group_and_a_glyph_that_exists() {
        for tab in Tab::ALL {
            assert!(!tab.label().is_empty());
            assert!(!tab.category().is_empty());
            // `from_name` falls back to `Info` for an unknown key, so a typo
            // would silently draw the same dot on three tabs and look like a
            // design choice rather than a mistake.
            assert_ne!(
                crate::glyphs::Glyph::from_name(tab.icon()),
                crate::glyphs::Glyph::Info,
                "unknown glyph key on {}",
                tab.label()
            );
        }
    }

    #[test]
    fn the_set_of_wired_flags_is_a_decision_rather_than_a_default() {
        // Pinned so that wiring a flag is an edit here as well as in the views,
        // and so that a flag cannot be *un*wired by accident — a switch that
        // stops being read is the exact failure this predicate exists to make
        // visible.
        let wired: Vec<&str> =
            ALL_FLAGS.into_iter().filter(|flag| flag.wired()).map(|flag| flag.id()).collect();
        assert_eq!(
            wired,
            vec![
                "features.show_worlds_tab",
                "features.show_screenshots_tab",
                "behavior.minimize_on_launch",
                "behavior.hide_right_sidebar",
                "behavior.compact_instance_cards",
                "behavior.show_play_time",
            ],
            "the wired set changed; wire the flag or amend this list on purpose"
        );
    }

    #[test]
    fn every_flag_names_a_field_that_carries_its_value() {
        // `field` is a string the compiler cannot check, so it is checked the
        // way the prefs file itself would: write the key with the opposite
        // value and read it back through the flag. A typo — or a field renamed
        // and missed — parses as a file with an unknown key, which serde
        // ignores, so the read comes back unchanged and this fails.
        for flag in ALL_FLAGS {
            let current = flag.get(&Prefs::default());
            let json = format!("{{\"{}\": {}}}", flag.field(), !current);
            let parsed: Prefs = serde_json::from_str(&json).expect("a one-key prefs file");
            assert_eq!(
                flag.get(&parsed),
                !current,
                "`{}` does not carry {flag:?}'s value",
                flag.field()
            );
        }
    }

    #[test]
    fn every_wired_flag_is_read_outside_this_module() {
        // This test exists because of a real defect: `wired` claimed four flags
        // that nothing but this file had ever read. The switch slid, the value
        // reached the disk, and the window never changed — the exact shape of
        // bug [`Flag::wired`] is meant to make impossible. `wired` is a claim
        // about other modules, so it is checked against their text.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut sources = String::new();
        for entry in std::fs::read_dir(&dir).expect("the crate's own source tree") {
            let path = entry.expect("a directory entry").path();
            let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
            // The mapping lives in this file, and `Flag::get` reads every field
            // — so counting it here would make every claim true by definition.
            if name == "settings.rs" || path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                continue;
            }
            sources.push_str(&std::fs::read_to_string(&path).expect("a readable source"));
        }
        for flag in ALL_FLAGS.into_iter().filter(|flag| flag.wired()) {
            assert!(
                sources.contains(&format!("prefs.{}", flag.field())),
                "{flag:?} is marked wired, but no module outside this file reads `prefs.{}`",
                flag.field()
            );
        }
    }

    #[test]
    fn every_unwired_flag_says_what_it_would_do_and_why_it_does_not() {
        for flag in ALL_FLAGS {
            let effect = flag.effect();
            assert!(!effect.is_empty(), "{} has no explanation", flag.id());
            if !flag.wired() {
                assert!(
                    effect.starts_with("Stored, not yet read"),
                    "{} is unwired but does not say so: {effect}",
                    flag.id()
                );
            }
        }
    }

    #[test]
    fn the_feature_flags_tab_is_the_only_developer_one() {
        let developer: Vec<Tab> = Tab::ALL.into_iter().filter(|tab| tab.developer_only()).collect();
        assert_eq!(developer, vec![Tab::FeatureFlags]);
    }

    #[test]
    fn the_section_headings_run_in_the_reference_order() {
        // The reference's `startsCategory` prints one heading per run of tabs
        // that share a category, so the run structure is what the list looks
        // like — and it must be Display, Account, Instances in that order.
        let mut categories: Vec<&str> = Vec::new();
        for tab in Tab::ALL {
            if categories.last() != Some(&tab.category()) {
                categories.push(tab.category());
            }
        }
        assert_eq!(categories, vec!["Display", "Account", "Instances"]);
    }

    #[test]
    fn every_pane_builds_under_every_theme() {
        // Building is the test: each pane reads the palette through the widget
        // styles, so a theme whose colours were never resolved would fail to
        // construct. It also proves a pane survives an empty prefs file and a
        // signed-out account.
        let guard = theme::THEME_TEST_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let original = theme::color_theme();
        let prefs = Prefs::default();
        let anim = SwitchAnim::default();
        let drafts = BTreeMap::new();
        // Two directories, named but never touched: the panes read them for
        // display only, and a test that made them real folders would be testing
        // the filesystem rather than the pane.
        let home = Path::new("/palantir-home");
        let data_root = Path::new("/palantir-data");
        for choice in ColorTheme::ALL {
            theme::set_color_theme(choice);
            for developer_mode in [false, true] {
                let view = View {
                    prefs: &prefs,
                    anim: &anim,
                    pointer: Pointer::default(),
                    drafts: &drafts,
                    account: None,
                    flag_filter: "",
                    home,
                    data_root,
                    data_root_pending: None,
                };
                let list: Element<'_, Message> = nav(Tab::Appearance, developer_mode);
                let _ = list;
                for tab in Tab::ALL {
                    let element: Element<'_, Message> = pane(tab, &view);
                    let _ = element;
                }
                // The one pane with a branch of its own: a recorded data root
                // that could not be applied draws an extra line, and a pane
                // that only builds in the easy state is a pane that panics on
                // the state somebody is actually in.
                let pending = View {
                    prefs: &prefs,
                    anim: &anim,
                    pointer: Pointer::default(),
                    drafts: &drafts,
                    account: None,
                    flag_filter: "",
                    home,
                    data_root,
                    data_root_pending: Some("D:/not-there".to_string()),
                };
                let element: Element<'_, Message> = pane(Tab::Resource, &pending);
                let _ = element;
            }
        }
        theme::set_color_theme(original);
        drop(guard);
    }

    #[test]
    fn a_switch_is_drawn_part_way_through_its_own_slide() {
        // The wiring between the animation and the drawing, which nothing else
        // covers: a switch whose value is already on, but whose slide has not
        // finished, must be drawn between the two ends rather than at the end.
        let mut anim = SwitchAnim::default();
        let start = std::time::Instant::now();
        anim.set(Flag::ShowPlayTime.id(), false, true, start);
        let running = anim.tick(start + crate::anim::DURATION / 2);
        assert!(running, "the slide should still be running halfway through");
        let progress = anim.progress(Flag::ShowPlayTime.id(), true);
        assert!(progress > 0.05 && progress < 0.95, "got {progress}");
        // And a switch nobody has touched is simply at its value, so the first
        // paint of a pane does not animate everything on it into place.
        assert_eq!(anim.progress(Flag::ShowFilesTab.id(), false), 0.0);
        assert_eq!(anim.progress(Flag::ShowFilesTab.id(), true), 1.0);
    }
}
