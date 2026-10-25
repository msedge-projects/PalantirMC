//! The app settings modal: the scrim, the plate, the tab rail, and the selected tab's body.
//!
//! **Sources.** The modal is the Modrinth App's `app-settings-modal`: its tab list and order
//! come from `app-frontend/src/features/settings/app-settings-modal/index.vue` (the `tabs`
//! array, lines 88-205), the plate from `packages/ui/src/components/modal/TabbedModal.vue`
//! and `NewModal.vue` under it, and the tab contents from
//! `app-frontend/src/features/settings/**`. Note where those live in this tree: the copy at
//! `vendor/modrinth-app/app-frontend` predates the `features/` reorganisation and has the
//! settings under `src/components/ui/settings/`, so the citations below point at the 0.22
//! checkout at `.scratch/modrinth-0.22/modrinth-code-51e57af/apps/app-frontend`, which is
//! the tree that carries `app-settings-modal/index.vue` at all.
//!
//! **The modal's shape is arithmetic, not a screenshot.** `index.vue:380-381` asks for
//! `width="928px"` and `max-width="928px"`; `NewModal.vue:743` caps that at
//! `calc(100% - 2 * var(--gap-lg))`, i.e. 16 points off each edge; the body is a two-column
//! grid `grid-cols-[250px_minmax(0,1fr)]` at `h-[min(65vh,640px)]` (`TabbedModal.vue:148`),
//! under a header of `p-6` around a `text-2xl` title (`NewModal.vue:50-54`). So the plate is
//! 928 wide and `80 + min(65% of the screen, 640)` tall, centred on the screen.
//!
//! **The rail is 14 rows, not 12.** The source declares Appearance, Features, Behavior,
//! Language, Feature flags (developer only), Profile, Social, Privacy, Java and memory,
//! Window, Launch hooks, Syncing, Resources and Troubleshooting -- 14 -- grouped as Display,
//! Account, Instance defaults and System. Each row is `rounded-[14px] px-4 py-2.5` around a
//! `text-base leading-5` label and a `size-5` icon (`TabbedModal.vue:189-192`), which is
//! 10 + 20 + 10 = 40 points tall, on the scroll column's `gap-1` (line 173): a 44-point
//! pitch. A category heading is `text-sm leading-5` with `pb-1.5`, and `mt-3` when it is not
//! the first child (line 179-180).
//!
//! **14 rows do not fit.** The rail's own height is the pane's 585 points at a 900-tall
//! screen, and the rows need 14 * 44 + 3 * 38 = 730. The reference scrolls the rail and
//! fades both ends (`TabbedModal.vue:156-224`); this pass clips it to the rail rect and
//! leaves the wheel to the slice that adds scrolling. Rows whose centre is outside the rail
//! are not interactive for exactly that reason: a click that lands on a clipped-away row
//! would change the tab from nowhere.
//!
//! **The scrim is the one number not in the theme.** There is no `--overlay` or `bg-overlay`
//! token in `packages/assets/styles/variables.scss`: the only overlay variable is
//! `--splash-overlay` (`:165`, `:385`), which is the splash screen's tint. The modal's own
//! scrim is `.modal-overlay.standard`, a gradient from `rgba(29, 48, 43, 0.52)` to
//! `rgba(14, 21, 26, 0.95)` plus `backdrop-filter: blur(5px)` (`NewModal.vue:679`, `:698`).
//! A flat `from_black_alpha(140)` is 0.549 alpha -- the top stop's -- and stands in for that
//! gradient, so the number came from the CSS and the theme is not the place it lives.
//!
//! **Three tabs carry content, eleven carry their heading.** Appearance and Features are
//! ported in the source's own order, with the labels and descriptions copied verbatim from
//! the catalogues (`app-frontend/src/locales/en-US/index.json`, plus the `@modrinth/ui`
//! catalogue for the keys the app's own file does not carry -- the Appearance tab's strings
//! ship in `packages/ui/src/locales/en-US/index.json`). Privacy carries the three rows whose
//! strings are in the app's catalogue. The remaining eleven tabs draw their heading and an
//! `owed:` note naming the rows that are still to come.
//!
//! **Controls are static.** Nothing here writes a setting: the switch, the select and the
//! button are drawn at a fixed state with no store behind them, which is why there is no
//! hover, no press animation and no `request_repaint`. The rail and the close button are the
//! only things that move, and they move `State`.

use eframe::egui::{self, pos2, vec2, Align2, Color32, Rect, Sense, Stroke, Ui, Vec2};

use crate::fonts;
use crate::icons::{self, Glyph};
use crate::theme;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Whether the modal is up, and which tab it is showing.
#[derive(Debug, Default)]
pub struct State {
    /// The shell keeps drawing while this is true, and stops when it is false.
    pub open: bool,
    /// The selected tab. The source opens on the first one (`index.vue:88-96`).
    pub tab: Tab,
}

/// One row of the source's rail, in the source's order (`index.vue:88-205`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// The first tab the source declares, and the one it opens on.
    #[default]
    Appearance,
    /// Feature toggles: instance tabs, sidebar buttons, the Play page.
    Features,
    /// Startup, navigation, landing page and confirmations.
    Behavior,
    /// The interface language.
    Language,
    /// Developer mode only: one switch per feature flag.
    FeatureFlags,
    /// The signed-in Modrinth account.
    Profile,
    /// Connected social accounts.
    Social,
    /// Telemetry, Discord Rich Presence and ad consent.
    Privacy,
    /// The default memory allocation, Java arguments and environment for new instances.
    JavaAndMemory,
    /// The default game window size and fullscreen flag.
    Window,
    /// The hooks run around a launch.
    LaunchHooks,
    /// What is synced across instances and devices.
    Syncing,
    /// The app directory, download limits and stored content.
    Resources,
    /// Debug info, ad reports and the app cache.
    Troubleshooting,
}

impl Tab {
    /// The rail's order, which is the order `index.vue:88-205` declares.
    pub const ALL: [Tab; 14] = [
        Tab::Appearance,
        Tab::Features,
        Tab::Behavior,
        Tab::Language,
        Tab::FeatureFlags,
        Tab::Profile,
        Tab::Social,
        Tab::Privacy,
        Tab::JavaAndMemory,
        Tab::Window,
        Tab::LaunchHooks,
        Tab::Syncing,
        Tab::Resources,
        Tab::Troubleshooting,
    ];

    /// The tab's label: the catalogue's own string for the key named in the constant.
    pub fn label(self) -> &'static str {
        match self {
            // `app.settings.tabs.appearance` / `app.settings.tabs.features` / ...
            Tab::Appearance => TAB_APPEARANCE,
            Tab::Features => TAB_FEATURES,
            Tab::Behavior => TAB_BEHAVIOR,
            Tab::Language => TAB_LANGUAGE,
            // `settings.feature-flags.title`, `settings.profile.navigation-title` and
            // `settings.social.title` come from `commonSettingsMessages`
            // (`packages/ui/src/utils/common-messages.ts:1108-1170`), whose catalogue is the
            // ui package's, not the app's.
            Tab::FeatureFlags => TAB_FEATURE_FLAGS,
            Tab::Profile => TAB_PROFILE,
            Tab::Social => TAB_SOCIAL,
            Tab::Privacy => TAB_PRIVACY,
            Tab::JavaAndMemory => TAB_JAVA_AND_MEMORY,
            Tab::Window => TAB_WINDOW,
            Tab::LaunchHooks => TAB_LAUNCH_HOOKS,
            Tab::Syncing => TAB_SYNCHING,
            Tab::Resources => TAB_RESOURCES,
            Tab::Troubleshooting => TAB_TROUBLESHOOTING,
        }
    }

    /// The category the rail groups this tab under (`index.vue:75-86`, `:126`, `:145`).
    fn category(self) -> &'static str {
        match self {
            Tab::Appearance
            | Tab::Features
            | Tab::Behavior
            | Tab::Language
            | Tab::FeatureFlags => CATEGORY_DISPLAY,
            Tab::Profile | Tab::Social | Tab::Privacy => CATEGORY_ACCOUNT,
            Tab::JavaAndMemory | Tab::Window | Tab::LaunchHooks | Tab::Syncing => {
                CATEGORY_INSTANCE_DEFAULTS
            }
            Tab::Resources | Tab::Troubleshooting => CATEGORY_SYSTEM,
        }
    }

    /// The row's leading glyph, the icon `index.vue` names for the tab.
    fn glyph(self) -> Glyph {
        match self {
            Tab::Appearance => Glyph::Paintbrush,
            Tab::Features => Glyph::LightBulb,
            Tab::Behavior => Glyph::Settings2,
            Tab::Language => Glyph::Languages,
            Tab::FeatureFlags => Glyph::ToggleRight,
            Tab::Profile => Glyph::User,
            Tab::Social => Glyph::HeartHandshake,
            Tab::Privacy => Glyph::EyeOff,
            Tab::JavaAndMemory => Glyph::Coffee,
            Tab::Window => Glyph::Window,
            Tab::LaunchHooks => Glyph::Code,
            Tab::Syncing => Glyph::RefreshCw,
            Tab::Resources => Glyph::Gauge,
            Tab::Troubleshooting => Glyph::Wrench,
        }
    }
}

// ---------------------------------------------------------------------------
// Catalogue strings
//
// Every string below is copied verbatim. The key beside it is the one the source passes to
// `defineMessage` / `defineMessages`; the file named after the key is the catalogue that
// resolves it.
// ---------------------------------------------------------------------------

/// The modal's title: `label.settings` (`common-messages.ts:331-333`), rendered by
/// `index.vue:387` at `text-2xl font-semibold text-contrast`.
const DIALOG_TITLE: &str = "Settings";

// --- rail labels: app-frontend/src/locales/en-US/index.json ---
const TAB_APPEARANCE: &str = "Appearance"; // app.settings.tabs.appearance (:3125)
const TAB_FEATURES: &str = "Features"; // app.settings.tabs.features (:3131)
const TAB_BEHAVIOR: &str = "Behavior"; // app.settings.tabs.behavior (:3128)
const TAB_LANGUAGE: &str = "Language"; // app.settings.tabs.language (:3140)
const TAB_FEATURE_FLAGS: &str = "Feature flags"; // settings.feature-flags.title (ui :6317)
const TAB_PROFILE: &str = "Profile"; // settings.profile.navigation-title (ui :6377)
const TAB_SOCIAL: &str = "Social"; // settings.social.title (ui :6494)
const TAB_PRIVACY: &str = "Privacy"; // app.settings.tabs.privacy (:3146)
const TAB_JAVA_AND_MEMORY: &str = "Java and memory"; // app.settings.tabs.java-and-memory (:3134)
const TAB_WINDOW: &str = "Window"; // app.settings.tabs.window (:3158)
const TAB_LAUNCH_HOOKS: &str = "Launch hooks"; // app.settings.tabs.launch-hooks (:3143)
const TAB_SYNCHING: &str = "Syncing"; // app.settings.tabs.syncing (:3152)
const TAB_RESOURCES: &str = "Resources"; // app.settings.tabs.resources (:3149)
const TAB_TROUBLESHOOTING: &str = "Troubleshooting"; // app.settings.tabs.troubleshooting (:3155)

// --- rail category labels ---
const CATEGORY_DISPLAY: &str = "Display"; // settings.sidebar.label.display (:4649)
const CATEGORY_ACCOUNT: &str = "Account"; // settings.sidebar.label.account (:4646)
const CATEGORY_INSTANCE_DEFAULTS: &str = "Instance defaults"; // app.settings.sidebar.label.instance-defaults (:2897)
const CATEGORY_SYSTEM: &str = "System"; // app.settings.sidebar.label.system (:2900)

// --- Appearance (settings/display/appearance-settings.vue -> ui appearance-settings layout)
const COLOR_THEME_TITLE: &str = "Color theme"; // settings.display.theme.title (ui :6314)
const COLOR_THEME_DESCRIPTION: &str = "Select your preferred color theme across Modrinth."; // settings.display.theme.description (ui :6284)
const COLOR_THEME_VALUE: &str = "Dark"; // settings.display.theme.dark (ui :6281), the default from use-theme.ts:38
const THEME_SYNC_LABEL: &str = "Sync theme across devices"; // settings.display.theme.sync-across-devices (ui :6302)
const THEME_SYNC_DESCRIPTION: &str = "Use this theme everywhere you're signed in. Turn this off to keep a separate theme on this device."; // settings.display.theme.sync-across-devices.description (ui :6305)
const ADVANCED_RENDERING_LABEL: &str = "Advanced rendering"; // settings.display.sidebar.advanced-rendering.title (ui :6260)
const ADVANCED_RENDERING_DESCRIPTION: &str = "Enables advanced rendering such as blur effects that may cause performance issues without hardware-accelerated rendering."; // settings.display.sidebar.advanced-rendering.description (ui :6257)
const WINDOW_FRAME_LABEL: &str = "System window frame"; // app.appearance-settings.native-decorations.title (ui :41)
const WINDOW_FRAME_DESCRIPTION: &str = "Use your operating system's title bar and window controls. Requires an app restart."; // app.appearance-settings.native-decorations.description (ui :38)

// --- Features (settings/display/features-settings.vue) ---
const FEATURES_SYNC_LABEL: &str = "Sync features across devices"; // app.features-settings.sync-across-devices.title (:479)
const FEATURES_SYNC_DESCRIPTION: &str = "Use these feature settings everywhere you're signed in. Turn this off to keep separate settings on this device."; // app.features-settings.sync-across-devices.description (:473)
const INSTANCES_HEADING: &str = "Instances"; // app.features-settings.instance-pages.title (:431)
const WORLDS_TAB_LABEL: &str = "Show Worlds tab in instances"; // app.features-settings.show-worlds-tab.title (:467)
const FILES_TAB_LABEL: &str = "Show Files tab in instances"; // app.features-settings.show-files-tab.title (:449)
const SCREENSHOTS_TAB_LABEL: &str = "Show Screenshots tab in instances"; // app.features-settings.show-screenshots-tab.title (:458)
const SIDEBAR_HEADING: &str = "Sidebar"; // app.features-settings.sidebar.title (:470)
const ALL_SCREENSHOTS_LABEL: &str = "Show all screenshots in sidebar"; // app.features-settings.show-all-screenshots.title (:446)
const ALL_SCREENSHOTS_DESCRIPTION: &str =
    "Show a button in the left sidebar to view screenshots from all your instances."; // app.features-settings.show-all-screenshots.description (:443)
const SKIN_SELECTOR_LABEL: &str = "Show skin selector in sidebar"; // app.features-settings.show-skin-selector.title (:464)
const SKIN_SELECTOR_DESCRIPTION: &str = "Show a button in the left sidebar to open the skin selector."; // app.features-settings.show-skin-selector.description (:461)
const QUICK_INSTANCES_LABEL: &str = "Quick instances in sidebar"; // app.features-settings.quick-instances.title (:440)
const QUICK_INSTANCES_DESCRIPTION: &str = "Choose the maximum number of recent instances shown in the left sidebar. You can also adjust this by dragging the divider."; // app.features-settings.quick-instances.description (:437)
const QUICK_INSTANCES_VALUE: &str = "20"; // QUICK_INSTANCE_LIMIT_MAX, use-quick-instance-limit.ts:5
const PLAY_PAGE_HEADING: &str = "Play page"; // app.features-settings.play-page.title (:434)
const JUMP_IN_LABEL: &str = "Show Jump in section"; // app.features-settings.show-jump-in.title (:455)
const JUMP_IN_DESCRIPTION: &str = "Show recently played worlds and instances at the top of the Play page."; // app.features-settings.show-jump-in.description (:452)

// --- Privacy (settings/account/privacy-settings.vue) ---
const PRIVACY_ADS_HEADING: &str = "Your privacy and how ads support Modrinth"; // app.ads-consent.title (:146)
const PRIVACY_ADS_INTRO: &str = "Ads make Modrinth possible and fund creator payouts. Our partners may store or access cookies in the app to personalize ads and measure performance. You can opt out or manage your preferences below."; // app.settings.privacy.ads-consent.intro (:2756)
const PRIVACY_MANAGE_LABEL: &str = "Manage preferences"; // app.ads-consent.manage (:140)
const TELEMETRY_LABEL: &str = "Telemetry"; // app.settings.privacy.telemetry.title (:2768)
const TELEMETRY_DESCRIPTION: &str = "Modrinth collects anonymized analytics and usage data to improve our user experience and customize your experience. By disabling this option, you opt out and your data will no longer be collected."; // app.settings.privacy.telemetry.description (:2765)
const DISCORD_LABEL: &str = "Discord Rich Presence"; // app.settings.privacy.discord-rich-presence.title (:2762)
const DISCORD_DESCRIPTION: &str = "Show Modrinth App as your current activity on Discord. This does not affect Rich Presence added to instances by mods. Requires an app restart."; // app.settings.privacy.discord-rich-presence.description (:2759)

// ---------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------

/// The plate's width: `width="928px"` (`app-settings-modal/index.vue:380`).
const DIALOG_WIDTH: f32 = 928.0;

/// How much of the screen's height the two-column body takes: `h-[min(65vh,640px)]`
/// (`TabbedModal.vue:148`), here as the 65vh fraction.
const DIALOG_VIEWPORT: f32 = 0.65;

/// The body's cap, the `640px` of the same class (`TabbedModal.vue:148`).
const DIALOG_MAX_HEIGHT: f32 = 640.0;

/// The screen inset the plate may not cross: `calc(100% - 2 * var(--gap-lg))`
/// (`NewModal.vue:743`), where `--gap-lg` is 1rem (`variables.scss:198`).
const DIALOG_MARGIN: f32 = 16.0;

/// The plate's corner: `!rounded-[20px]` (`TabbedModal.vue:142`), which is `--radius-xl`
/// (`variables.scss:205`) -- the theme's own ladder, `theme::RADIUS_XL`.
const PLATE_RADIUS: u8 = theme::RADIUS_XL;

/// The content pane shares the plate's bottom-right rounding and is square on the other
/// three corners, because it meets the header above it and the rail to its left.
const PANE_RADIUS: egui::CornerRadius = egui::CornerRadius {
    nw: 0,
    ne: 0,
    sw: 0,
    se: theme::RADIUS_XL,
};

/// The header's `p-6` (`NewModal.vue:50`), and the title's left inset with it.
const HEADER_PADDING: f32 = 24.0;

/// The header's title size: `text-2xl` (`index.vue:387`) is 1.5rem = 24 points.
const TITLE_SIZE: f32 = 24.0;

/// The header's line box: `text-2xl` is 1.5rem on a 2rem leading, so the title is 32 tall
/// (`index.vue:387`, the same pair `library.rs` states as `HEADING_LINE`).
const HEADING_LINE: f32 = 32.0;

/// The header's total height: `24 + 32 + 24` -- `p-6` above and below the title's line
/// (`NewModal.vue:50-54`).
const HEADER_HEIGHT: f32 = HEADER_PADDING + HEADING_LINE + HEADER_PADDING;

/// A 1-point rule: the header's `border-b` (`NewModal.vue:50`) and the rail's `border-r`
/// (`TabbedModal.vue:152`), i.e. `theme::STROKE_WIDTH`.
const RULE_STROKE: f32 = theme::STROKE_WIDTH;

/// The rail's column: `grid-cols-[250px_minmax(0,1fr)]` (`TabbedModal.vue:148`).
const RAIL_WIDTH: f32 = 250.0;

/// The rail's padding: `p-4` (`TabbedModal.vue:149`).
const RAIL_PADDING: f32 = 16.0;

/// The gap between rail children: `gap-1` on the scroll column (`TabbedModal.vue:173`).
const RAIL_ROW_GAP: f32 = 4.0;

/// A rail row's height: `py-2.5` (10 above and below) around a `leading-5` label
/// (`TabbedModal.vue:189`), i.e. `10 + 20 + 10`.
const RAIL_ROW_HEIGHT: f32 = 40.0;

/// A rail row's corner: `rounded-[14px]` (`TabbedModal.vue:189`). 14 is not on the theme's
/// ladder (4, 8, 12, 16, 20), so it is stated here rather than taken from `theme::RADIUS_*`.
const RAIL_ROW_RADIUS: u8 = 14;

/// A rail row's horizontal padding: `px-4` (`TabbedModal.vue:189`).
const RAIL_ROW_PADDING: f32 = 16.0;

/// The icon at the start of a rail row: `size-5` (`TabbedModal.vue:192`).
const RAIL_ICON: f32 = 20.0;

/// The gap between that icon and the label: `gap-2` (`TabbedModal.vue:189`).
const RAIL_ICON_GAP: f32 = 8.0;

/// A rail row's label size: `text-base` (`TabbedModal.vue:189`).
const RAIL_TEXT_SIZE: f32 = 16.0;

/// A category heading's line: `text-sm leading-5` (`TabbedModal.vue:179`), so a 20-point
/// line box; the drawn size follows the label's own 14-point step.
const CATEGORY_LINE: f32 = 20.0;

/// A category heading's drawn size: `text-sm` (`TabbedModal.vue:179`).
const CATEGORY_TEXT_SIZE: f32 = 14.0;

/// The space under a category heading: `pb-1.5` (`TabbedModal.vue:179`).
const CATEGORY_PADDING: f32 = 6.0;

/// The space above a category heading that is not the first child: `mt-3`
/// (`TabbedModal.vue:180`).
const CATEGORY_TOP_GAP: f32 = 12.0;

/// The body's inset: `px-6 pt-6` on the scroll content (`TabbedModal.vue:249`).
const CONTENT_PADDING: f32 = 24.0;

/// The space a section rule puts above itself: `mt-8` (`features-settings.vue:325`).
const CONTENT_SECTION_GAP: f32 = 32.0;

/// The space a section rule puts below itself: `pt-6` (`features-settings.vue:325`).
const CONTENT_SECTION_PADDING: f32 = 24.0;

/// A section heading's drawn size: `text-xl` (`features-settings.vue:280`, `layout.vue:36`).
const GROUP_HEADING_SIZE: f32 = 20.0;

/// A group heading's drawn size: `text-lg` (`features-settings.vue:255`,
/// `AppearanceSettingRow.vue:25`).
const SECTION_HEADING_SIZE: f32 = 18.0;

/// A row's label size. The source's row titles are `text-lg` (18) and its descriptions
/// inherit the 16-point base; this pass draws the 14/12 pair the brief asks for, so the
/// wrapped line count of a long description is the one thing that will move when the rows
/// are raised to the source's sizes.
const ROW_LABEL_SIZE: f32 = 14.0;

/// A row's description size: `text-secondary` with no size class of its own
/// (`AppearanceSettingRow.vue:29`), drawn at the brief's 12 here.
const ROW_DESCRIPTION_SIZE: f32 = 12.0;

/// The gap between a row's label and its description: `mt-1` (`AppearanceSettingRow.vue:29`).
const ROW_DESCRIPTION_GAP: f32 = 4.0;

/// The gap between two rows: `gap-6` (`features-settings.vue:283`, `layout.vue:101`).
const ROW_GAP: f32 = 24.0;

/// The gap between a heading and the rows under it: `mt-4` (`features-settings.vue:283`).
const HEADING_TO_ROWS: f32 = 16.0;

/// The gap between a row's text and its control: `gap-4` on the row's flex line
/// (`AppearanceSettingRow.vue:20`).
const ROW_CONTROL_GAP: f32 = 16.0;

/// A label column narrower than this would push a description to one word a line. The
/// source's `min-w-0` lets the text column shrink to nothing (`AppearanceSettingRow.vue:21`);
/// a floor is the one thing this pass adds.
const MIN_TEXT_WIDTH: f32 = 120.0; // guess: re-measure

/// A control's height: `h-9` on `ButtonFrame`'s `md` size (`ButtonFrame.vue:31`), which the
/// selects and buttons both use.
const CONTROL_HEIGHT: f32 = 36.0;

/// A control's corner: `rounded-xl` (`ButtonFrame.vue:31`), i.e. `theme::RADIUS_MD`.
const CONTROL_RADIUS: u8 = theme::RADIUS_MD;

/// A control's horizontal padding: `px-2.5` (`ButtonFrame.vue:31`).
const CONTROL_PADDING: f32 = 10.0;

/// A control's text size: `text-base` (`ButtonFrame.vue:31`), semibold.
const CONTROL_TEXT_SIZE: f32 = 16.0;

/// The chevron that marks a select: `[&>svg]:size-5` (`ButtonFrame.vue:31`), drawn at 16
/// here as `library.rs` draws its own selects' chevrons.
const SELECT_CHEVRON: f32 = 16.0;

/// A button's leading glyph: `[&>svg]:size-5` (`ButtonFrame.vue:31`).
const CONTROL_GLYPH: f32 = 20.0;

/// The gap between a button's glyph and its label: `gap-1.5` (`ButtonFrame.vue:31`).
const CONTROL_GLYPH_GAP: f32 = 6.0;

/// The switch's track: `h-6 !w-[48px]` (`Toggle.vue:20`).
const SWITCH_WIDTH: f32 = 48.0;

/// The switch's track height: the same `h-6` (`Toggle.vue:20`).
const SWITCH_HEIGHT: f32 = 24.0;

/// The switch's track corner: `rounded-full` (`Toggle.vue:19`), which is the theme's
/// maximum radius -- `theme::RADIUS_MAX` -- rather than an arbitrary number.
const SWITCH_RADIUS: u8 = theme::RADIUS_MAX;

/// The knob's diameter: `w-4 h-4` at the default size (`Toggle.vue:26`).
const SWITCH_KNOB: f32 = 16.0;

/// The track's inner padding, which is where the knob rests: `p-1` (`Toggle.vue:19`).
const SWITCH_INSET: f32 = 4.0;

/// The close button's box: `IconButton` at its default `md` is `w-9 h-9` with `!px-0`
/// (`IconButton.vue:27`, `ButtonFrame.vue:39`).
const CLOSE_SIZE: f32 = 36.0;

/// The close button's glyph: `[&>svg]:size-5` (`ButtonFrame.vue:31`).
const CLOSE_GLYPH: f32 = 20.0;

/// The scrim's alpha. The reference has no `--overlay` token; its scrim is
/// `.modal-overlay.standard`'s gradient, `rgba(29, 48, 43, 0.52)` to `rgba(14, 21, 26, 0.95)`
/// (`NewModal.vue:679`). 140/255 is 0.549, the top stop's alpha.
const SCRIM_ALPHA: u8 = 140;

// ---------------------------------------------------------------------------
// The body's blocks
// ---------------------------------------------------------------------------

/// The control a row offers. All three are drawn at rest; none of them writes anything.
#[derive(Debug, Clone, Copy)]
enum Control {
    /// A `Toggle` (`Toggle.vue`): a brand track with the knob to the right when on.
    Switch {
        /// Which way the switch is drawn. Nothing stores it yet.
        on: bool,
    },
    /// A select's trigger (`ButtonFrame.vue:31`): a plate, the value, and a chevron.
    Select {
        /// The value the trigger shows.
        value: &'static str,
    },
    /// A button (`ButtonFrame.vue:45`, the `base` colour): a plate and a label.
    Button {
        /// The button's label.
        label: &'static str,
        /// The glyph before the label, when the source's button has one.
        glyph: Option<Glyph>,
    },
}

/// One block of a tab's body, in the order the source lays it out.
#[derive(Debug, Clone, Copy)]
enum Block {
    /// A section or group heading, with the description that sits under it and, when the
    /// source puts one on the same line, its control (`features-settings.vue:255-270`).
    Heading {
        /// The heading's text.
        text: &'static str,
        /// The muted line under it.
        description: Option<&'static str>,
        /// `true` for `text-xl` (20), `false` for `text-lg` (18).
        big: bool,
        /// The control on the heading's right, if any.
        control: Option<Control>,
    },
    /// A labelled setting: label, optional description, control on the right.
    Row {
        /// The setting's name.
        label: &'static str,
        /// The muted line under it.
        description: Option<&'static str>,
        /// The control on the right.
        control: Control,
    },
    /// A control on a line of its own, left-aligned. The theme selector is one: the source
    /// puts a row of theme cards under the heading, with no label of its own
    /// (`layout.vue:44-51`).
    Standalone(Control),
    /// A `border-t` rule between sections (`features-settings.vue:325`).
    Rule,
}

/// The Appearance tab, in the source's order.
///
/// `layout.vue:36-51` is the `Color theme` heading and description with the theme selector
/// under them; `:54-67` is the sync row; `:100-101` is the `mt-8 border-t` rule and the
/// `gap-6` group that holds advanced rendering, the external-link rows, the sidebar rows and
/// the window frame.
///
/// `// owed:` "Project list layouts" (`layout.vue:80-98`) and the external-link and
/// sidebar-preference rows (`:117`, `:134`) are absent in the app because
/// `appearance-settings.vue:150-168` provides only `theme`, `advancedRendering` and
/// `nativeDecorations`; the selector itself is drawn as a select rather than the card row of
/// `appearance-settings-theme-selector.vue`.
const APPEARANCE_BLOCKS: &[Block] = &[
    Block::Heading {
        text: COLOR_THEME_TITLE,
        description: Some(COLOR_THEME_DESCRIPTION),
        big: true,
        control: None,
    },
    Block::Standalone(Control::Select {
        value: COLOR_THEME_VALUE,
    }),
    Block::Row {
        label: THEME_SYNC_LABEL,
        description: Some(THEME_SYNC_DESCRIPTION),
        // Signed out, the source draws this off and disabled (`layout.vue:60-75`).
        control: Control::Switch { on: false },
    },
    Block::Rule,
    Block::Row {
        label: ADVANCED_RENDERING_LABEL,
        description: Some(ADVANCED_RENDERING_DESCRIPTION),
        // `use-theme.ts:53` starts advanced rendering on.
        control: Control::Switch { on: true },
    },
    Block::Row {
        label: WINDOW_FRAME_LABEL,
        description: Some(WINDOW_FRAME_DESCRIPTION),
        // Drawn on to show the lit track; it is Windows-only in the source
        // (`appearance-settings.vue:170-176` skips it on macOS).
        control: Control::Switch { on: true },
    },
];

/// The Features tab, in the source's order: the sync row (`features-settings.vue:252-278`),
/// then `Instances` with its three switches (`:279-322`), `Sidebar` with two switches and the
/// quick-instance count (`:325-376`), and `Play page` with the Jump in switch (`:379-...`).
///
/// `// owed:` the quick-instance count is a `Slider` (`:364`), which is not one of the three
/// controls this pass draws, so the row shows its value in a select; the signed-out tooltip
/// on the sync switch (`app.features-settings.sync-across-devices.signed-out-tooltip`) is
/// also still owed.
const FEATURES_BLOCKS: &[Block] = &[
    Block::Heading {
        text: FEATURES_SYNC_LABEL,
        description: Some(FEATURES_SYNC_DESCRIPTION),
        big: false,
        control: Some(Control::Switch { on: false }),
    },
    Block::Heading {
        text: INSTANCES_HEADING,
        description: None,
        big: true,
        control: None,
    },
    Block::Row {
        label: WORLDS_TAB_LABEL,
        description: None,
        control: Control::Switch { on: true },
    },
    Block::Row {
        label: FILES_TAB_LABEL,
        description: None,
        control: Control::Switch { on: true },
    },
    Block::Row {
        label: SCREENSHOTS_TAB_LABEL,
        description: None,
        control: Control::Switch { on: true },
    },
    Block::Rule,
    Block::Heading {
        text: SIDEBAR_HEADING,
        description: None,
        big: true,
        control: None,
    },
    Block::Row {
        label: ALL_SCREENSHOTS_LABEL,
        description: Some(ALL_SCREENSHOTS_DESCRIPTION),
        control: Control::Switch { on: true },
    },
    Block::Row {
        label: SKIN_SELECTOR_LABEL,
        description: Some(SKIN_SELECTOR_DESCRIPTION),
        control: Control::Switch { on: true },
    },
    Block::Row {
        label: QUICK_INSTANCES_LABEL,
        description: Some(QUICK_INSTANCES_DESCRIPTION),
        // The source's `Slider` (`:364`), drawn as the value it holds at its cap.
        control: Control::Select {
            value: QUICK_INSTANCES_VALUE,
        },
    },
    Block::Rule,
    Block::Heading {
        text: PLAY_PAGE_HEADING,
        description: None,
        big: true,
        control: None,
    },
    Block::Row {
        label: JUMP_IN_LABEL,
        description: Some(JUMP_IN_DESCRIPTION),
        control: Control::Switch { on: true },
    },
];

/// The Privacy tab's rows (`privacy-settings.vue:77-113`): the ad-consent heading with its
/// intro and the `Manage preferences` button, then telemetry and Discord Rich Presence.
///
/// `// owed:` the button's `Settings2Icon` is drawn, its click is not: the source opens the
/// ad-consent window (`privacy-settings.vue:60-62`).
const PRIVACY_BLOCKS: &[Block] = &[
    Block::Heading {
        text: PRIVACY_ADS_HEADING,
        description: Some(PRIVACY_ADS_INTRO),
        big: false,
        control: None,
    },
    Block::Standalone(Control::Button {
        label: PRIVACY_MANAGE_LABEL,
        glyph: Some(Glyph::Settings2),
    }),
    Block::Row {
        label: TELEMETRY_LABEL,
        description: Some(TELEMETRY_DESCRIPTION),
        control: Control::Switch { on: true },
    },
    Block::Row {
        label: DISCORD_LABEL,
        description: Some(DISCORD_DESCRIPTION),
        control: Control::Switch { on: true },
    },
];

/// The blocks for one tab. The eleven tabs that return an empty slice draw their heading
/// only, with their owed rows named in the match below.
fn blocks(tab: Tab) -> &'static [Block] {
    match tab {
        Tab::Appearance => APPEARANCE_BLOCKS,
        Tab::Features => FEATURES_BLOCKS,
        Tab::Privacy => PRIVACY_BLOCKS,

        // owed: the remaining tabs are heading-only this pass. Their rows, in the source's
        // order:
        //
        // Behavior -- `display/behavior-settings.vue`: "Sync behavior across devices",
        //   "Startup and navigation" ("Minimize app", "Refocus app when Minecraft closes"),
        //   "Home and content" (default landing page Home / Library), "Confirmations"
        //   ("Skip non-essential warnings").
        Tab::Behavior => &[],
        // Language -- `display/language-settings.vue`: the language list, one row per
        //   locale, with the interface's own name for each.
        Tab::Language => &[],
        // Feature flags -- `display/feature-flag-settings.vue`: one switch per flag, the
        //   flag's name capitalised as the heading (`:77`).
        Tab::FeatureFlags => &[],
        // Profile -- `account/profile-settings.vue`: the profile form (avatar, username,
        //   bio) in the shared `ProfileSettingsLayout`.
        Tab::Profile => &[],
        // Social -- `account/social-settings.vue`: the connections list in the shared
        //   `SocialSettingsLayout`.
        Tab::Social => &[],
        // Java and memory -- `instance-defaults/java-and-memory-settings/index.vue`:
        //   "Java installations", "Memory allocation", "Java arguments", "Environment
        //   variables".
        Tab::JavaAndMemory => &[],
        // Window -- `instance-defaults/window-settings.vue`: "Fullscreen", "Width",
        //   "Height".
        Tab::Window => &[],
        // Launch hooks -- `instance-defaults/launch-hooks-settings.vue`: "Pre-launch hook",
        //   "Wrapper hook", "Post-exit hook", and the hook-variables description.
        Tab::LaunchHooks => &[],
        // Syncing -- `syncing/syncing-settings.vue`: "Sync resource packs", "Sync data
        //   packs", "Sync game options", "Edit game settings", the synced servers and the
        //   command history.
        Tab::Syncing => &[],
        // Resources -- `system/resource-management-settings/index.vue`: "App directory",
        //   "Maximum concurrent downloads", "Maximum concurrent writes"; and
        //   `content-storage.vue`'s per-category storage rows.
        Tab::Resources => &[],
        // Troubleshooting -- `system/troubleshooting-settings.vue`: "Debug info" /
        //   "Export debug info", "Report an ad" / "Copy ad details", "App cache" / "Clear
        //   cache".
        Tab::Troubleshooting => &[],
    }
}

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

/// Draw the settings modal over `screen`, and say whether it is still open.
///
/// Returns `state.open`: the shell keeps drawing while it is `true`, and stops when a click
/// on the close button or a press of Escape has set it false.
pub fn draw(ui: &mut egui::Ui, screen: egui::Rect, theme: &crate::theme::Theme, state: &mut State) -> bool {
    if !state.open {
        return false;
    }

    let dialog = dialog_rect(screen);
    {
        let painter = ui.painter();
        painter.rect_filled(screen, 0, Color32::from_black_alpha(SCRIM_ALPHA));
        painter.rect_filled(dialog, PLATE_RADIUS, theme.RAISED_BG);
        painter.rect_stroke(
            dialog,
            PLATE_RADIUS,
            Stroke::new(RULE_STROKE, theme.DIVIDER),
            egui::StrokeKind::Inside,
        );
        draw_header(painter, dialog, theme);
        draw_rail(painter, dialog, theme, state.tab);
        draw_body(painter, dialog, theme, state.tab);
    }

    // The rail's rows are the only navigation. A row clipped away by the rail's overflow is
    // not a target: its centre has to be inside the rail for the click to count.
    let rail = rail_rect(dialog);
    for (index, entry) in rail_rows(dialog).into_iter().enumerate() {
        if entry.row.center().y <= rail.top() || entry.row.center().y >= rail.bottom() {
            continue;
        }
        let id = egui::Id::new(("sift-settings-tab", index));
        if ui.interact(entry.row, id, Sense::click()).clicked() {
            state.tab = entry.tab;
        }
    }

    if ui
        .interact(close_rect(dialog), egui::Id::new("sift-settings-close"), Sense::click())
        .clicked()
    {
        state.open = false;
    }

    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        state.open = false;
    }

    state.open
}

/// The plate: the source's width, centred on the screen, capped by the screen's margin, and
/// as tall as the header plus the two-column body.
fn dialog_rect(screen: Rect) -> Rect {
    let width = DIALOG_WIDTH.min((screen.width() - 2.0 * DIALOG_MARGIN).max(DIALOG_MARGIN));
    let body = (screen.height() * DIALOG_VIEWPORT).min(DIALOG_MAX_HEIGHT);
    Rect::from_center_size(screen.center(), vec2(width, HEADER_HEIGHT + body))
}

/// The header strip across the top of the plate.
fn header_rect(dialog: Rect) -> Rect {
    Rect::from_min_size(dialog.min, vec2(dialog.width(), HEADER_HEIGHT))
}

/// The rail's column, below the header.
fn rail_rect(dialog: Rect) -> Rect {
    Rect::from_min_max(
        pos2(dialog.left(), dialog.top() + HEADER_HEIGHT),
        pos2(dialog.left() + RAIL_WIDTH, dialog.bottom()),
    )
}

/// The body's column, right of the rail and below the header.
fn content_rect(dialog: Rect) -> Rect {
    Rect::from_min_max(
        pos2(dialog.left() + RAIL_WIDTH, dialog.top() + HEADER_HEIGHT),
        dialog.max,
    )
}

/// The close button's box: the header's `IconButton`, sitting in the right cell of the
/// header's grid, vertically centred on the title's line (`NewModal.vue:50-70`).
fn close_rect(dialog: Rect) -> Rect {
    let slack = (CLOSE_SIZE - HEADING_LINE) / 2.0;
    Rect::from_min_size(
        pos2(
            dialog.right() - HEADER_PADDING - CLOSE_SIZE,
            dialog.top() + HEADER_PADDING - slack,
        ),
        Vec2::splat(CLOSE_SIZE),
    )
}

/// One rail row, and the category heading that may sit above it.
#[derive(Debug, Clone, Copy)]
struct RailRow {
    /// The tab this row selects.
    tab: Tab,
    /// The category heading, on the first row of each category.
    heading: Option<Rect>,
    /// The row's own box.
    row: Rect,
}

/// How many rows the rail has: one per tab.
const TAB_COUNT: usize = Tab::ALL.len();

/// Lay the rail out: category headings first, then a 40-point row and a 4-point gap for
/// every tab.
fn rail_rows(dialog: Rect) -> [RailRow; TAB_COUNT] {
    let rail = rail_rect(dialog);
    let width = rail.width() - 2.0 * RAIL_PADDING;
    let mut rows = [RailRow {
        tab: Tab::Appearance,
        heading: None,
        row: Rect::NOTHING,
    }; TAB_COUNT];

    let mut y = rail.top() + RAIL_PADDING;
    let mut previous: Option<Tab> = None;
    for (index, tab) in Tab::ALL.into_iter().enumerate() {
        let starts_category = previous.map_or(true, |last| last.category() != tab.category());
        let heading = if starts_category {
            // `mt-3` above a category that is not the first child (`TabbedModal.vue:180`).
            if index > 0 {
                y += CATEGORY_TOP_GAP;
            }
            let rect = Rect::from_min_size(
                pos2(rail.left() + RAIL_PADDING, y),
                vec2(width, CATEGORY_LINE + CATEGORY_PADDING),
            );
            y = rect.bottom();
            Some(rect)
        } else {
            None
        };

        let row = Rect::from_min_size(
            pos2(rail.left() + RAIL_PADDING, y),
            vec2(width, RAIL_ROW_HEIGHT),
        );
        rows[index] = RailRow { tab, heading, row };
        y = row.bottom() + RAIL_ROW_GAP;
        previous = Some(tab);
    }

    rows
}

/// The title, the rule under it, and the close button.
fn draw_header(painter: &egui::Painter, dialog: Rect, theme: &theme::Theme) {
    let header = header_rect(dialog);
    painter.text(
        pos2(header.left() + HEADER_PADDING, header.top() + HEADER_PADDING),
        Align2::LEFT_TOP,
        DIALOG_TITLE,
        fonts::semibold(TITLE_SIZE),
        theme.CONTRAST,
    );

    // The header's `border-b`, inset by the plate's radius at both ends so it does not cut
    // across the rounded corners the modal clips it to (`TabbedModal.vue:142`).
    let inset = PLATE_RADIUS as f32;
    painter.hline(
        (header.left() + inset)..=(header.right() - inset),
        header.bottom(),
        Stroke::new(RULE_STROKE, theme.DIVIDER),
    );

    let close = close_rect(dialog);
    painter.rect_filled(close, SWITCH_RADIUS, theme.SUPER_RAISED_BG);
    icons::draw(
        painter,
        Glyph::X,
        Rect::from_center_size(close.center(), Vec2::splat(CLOSE_GLYPH)),
        theme.CONTRAST,
    );
}

/// The rail: its divider, a category heading per group, and one row per tab.
fn draw_rail(painter: &egui::Painter, dialog: Rect, theme: &theme::Theme, selected: Tab) {
    let rail = rail_rect(dialog);
    let clipped = painter.with_clip_rect(rail);

    // The rail's `border-r` (`TabbedModal.vue:152`).
    clipped.vline(
        rail.right(),
        rail.top()..=rail.bottom(),
        Stroke::new(RULE_STROKE, theme.DIVIDER),
    );

    for entry in rail_rows(dialog) {
        if let Some(heading) = entry.heading {
            clipped.text(
                pos2(heading.left(), heading.top()),
                Align2::LEFT_TOP,
                entry.tab.category(),
                fonts::semibold(CATEGORY_TEXT_SIZE),
                theme.SECONDARY,
            );
        }

        let is_selected = entry.tab == selected;
        if is_selected {
            // `bg-[color-mix(in_srgb,var(--color-brand)_30%,transparent)]`
            // (`TabbedModal.vue:189`); the theme's brand wash is `BUTTON_BG_SELECTED`.
            clipped.rect_filled(entry.row, RAIL_ROW_RADIUS, theme.BUTTON_BG_SELECTED);
        }
        let ink = if is_selected {
            theme.BUTTON_TEXT_SELECTED
        } else {
            theme.BASE
        };

        let icon_centre = pos2(
            entry.row.left() + RAIL_ROW_PADDING + RAIL_ICON / 2.0,
            entry.row.center().y,
        );
        icons::draw(
            &clipped,
            entry.tab.glyph(),
            Rect::from_center_size(icon_centre, Vec2::splat(RAIL_ICON)),
            ink,
        );
        clipped.text(
            pos2(
                entry.row.left() + RAIL_ROW_PADDING + RAIL_ICON + RAIL_ICON_GAP,
                entry.row.center().y,
            ),
            Align2::LEFT_CENTER,
            entry.tab.label(),
            fonts::semibold(RAIL_TEXT_SIZE),
            ink,
        );
    }
}

/// The selected tab's body: its pane, its heading, and its rows.
fn draw_body(painter: &egui::Painter, dialog: Rect, theme: &theme::Theme, tab: Tab) {
    let content = content_rect(dialog);
    painter.rect_filled(content, PANE_RADIUS, theme.SURFACE_2);
    let body = painter.with_clip_rect(content);

    let left = content.left() + CONTENT_PADDING;
    let right = content.right() - CONTENT_PADDING;
    let mut y = content.top() + CONTENT_PADDING;

    let blocks = blocks(tab);
    if blocks.is_empty() {
        // Nothing but the heading: the tab is in the rail and its rows are owed.
        draw_labelled(
            &body,
            theme,
            left,
            right,
            y,
            tab.label(),
            None,
            None,
            GROUP_HEADING_SIZE,
        );
        return;
    }

    // The gap owed by the block above; a rule brings its own margins.
    let mut gap = 0.0;
    for block in blocks {
        if let Block::Rule = block {
            y += CONTENT_SECTION_GAP;
            body.hline(
                left..=right,
                y,
                Stroke::new(RULE_STROKE, theme.DIVIDER),
            );
            y += CONTENT_SECTION_PADDING;
            gap = 0.0;
            continue;
        }

        y += gap;
        gap = match *block {
            Block::Heading {
                text,
                description,
                big,
                control,
            } => {
                let size = if big {
                    GROUP_HEADING_SIZE
                } else {
                    SECTION_HEADING_SIZE
                };
                draw_labelled(
                    &body,
                    theme,
                    left,
                    right,
                    y,
                    text,
                    description,
                    control,
                    size,
                ) + HEADING_TO_ROWS
            }
            Block::Row {
                label,
                description,
                control,
            } => draw_labelled(
                &body,
                theme,
                left,
                right,
                y,
                label,
                description,
                Some(control),
                ROW_LABEL_SIZE,
            ) + ROW_GAP,
            Block::Standalone(control) => {
                // Left-aligned: the control's right edge is its own width past the inset.
                let width = control.width(&body);
                control.draw(&body, theme, left + width, y + control.height() / 2.0);
                control.height() + ROW_GAP
            }
            // Handled above.
            Block::Rule => 0.0,
        };
    }
}

/// Draw one labelled block -- a heading or a row -- and return its height.
///
/// The text takes whatever the control does not: the source's rows are
/// `flex items-center justify-between gap-4` with the text column `min-w-0`
/// (`AppearanceSettingRow.vue:20-21`), so the control keeps its own width and the label
/// wraps in what is left.
#[allow(clippy::too_many_arguments)]
fn draw_labelled(
    painter: &egui::Painter,
    theme: &theme::Theme,
    left: f32,
    right: f32,
    top: f32,
    label: &str,
    description: Option<&str>,
    control: Option<Control>,
    label_size: f32,
) -> f32 {
    let control_width = control.map_or(0.0, |control| control.width(painter));
    let text_width = (right - left - control_width - ROW_CONTROL_GAP).max(MIN_TEXT_WIDTH);

    let mut y = top;
    y += text(
        painter,
        left,
        y,
        label,
        fonts::semibold(label_size),
        theme.CONTRAST,
        text_width,
    );
    if let Some(description) = description {
        y += ROW_DESCRIPTION_GAP;
        y += text(
            painter,
            left,
            y,
            description,
            fonts::body(ROW_DESCRIPTION_SIZE),
            theme.SECONDARY,
            text_width,
        );
    }

    let text_height = y - top;
    let control_height = control.map_or(0.0, |control| control.height());
    if let Some(control) = control {
        control.draw(
            painter,
            theme,
            right,
            top + text_height.max(control_height) / 2.0,
        );
    }
    text_height.max(control_height)
}

/// Draw wrapped text at `(x, y)` and return the height it took.
fn text(
    painter: &egui::Painter,
    x: f32,
    y: f32,
    body: &str,
    font: egui::FontId,
    colour: Color32,
    wrap: f32,
) -> f32 {
    let galley = painter.layout(body.to_owned(), font, colour, wrap);
    let height = galley.size().y;
    painter.galley(pos2(x, y), galley, colour);
    height
}

impl Control {
    /// The width the control keeps on the row.
    fn width(self, painter: &egui::Painter) -> f32 {
        match self {
            Control::Switch { .. } => SWITCH_WIDTH,
            Control::Select { value } => {
                let value = painter.layout_no_wrap(
                    value.to_owned(),
                    fonts::semibold(CONTROL_TEXT_SIZE),
                    Color32::PLACEHOLDER,
                );
                2.0 * CONTROL_PADDING + value.size().x + CONTROL_GLYPH_GAP + SELECT_CHEVRON
            }
            Control::Button { label, glyph } => {
                let label = painter.layout_no_wrap(
                    label.to_owned(),
                    fonts::semibold(CONTROL_TEXT_SIZE),
                    Color32::PLACEHOLDER,
                );
                let leading = if glyph.is_some() {
                    CONTROL_GLYPH + CONTROL_GLYPH_GAP
                } else {
                    0.0
                };
                2.0 * CONTROL_PADDING + leading + label.size().x
            }
        }
    }

    /// The height the control needs.
    fn height(self) -> f32 {
        match self {
            Control::Switch { .. } => SWITCH_HEIGHT,
            Control::Select { .. } | Control::Button { .. } => CONTROL_HEIGHT,
        }
    }

    /// Draw the control with its right edge at `right` and its centre at `centre_y`.
    fn draw(self, painter: &egui::Painter, theme: &theme::Theme, right: f32, centre_y: f32) {
        let height = self.height();
        let width = self.width(painter);
        match self {
            Control::Switch { on } => {
                let track = Rect::from_min_size(
                    pos2(right - SWITCH_WIDTH, centre_y - SWITCH_HEIGHT / 2.0),
                    vec2(SWITCH_WIDTH, SWITCH_HEIGHT),
                );
                switch(painter, track, theme, on);
            }
            Control::Select { value } => {
                let rect = Rect::from_min_size(
                    pos2(right - width, centre_y - height / 2.0),
                    vec2(width, height),
                );
                painter.rect_filled(rect, CONTROL_RADIUS, theme.BUTTON_BG);
                painter.text(
                    pos2(rect.left() + CONTROL_PADDING, rect.center().y),
                    Align2::LEFT_CENTER,
                    value,
                    fonts::semibold(CONTROL_TEXT_SIZE),
                    theme.CONTRAST,
                );
                icons::draw(
                    painter,
                    Glyph::ChevronDown,
                    Rect::from_center_size(
                        pos2(
                            rect.right() - CONTROL_PADDING - SELECT_CHEVRON / 2.0,
                            rect.center().y,
                        ),
                        Vec2::splat(SELECT_CHEVRON),
                    ),
                    theme.SECONDARY,
                );
            }
            Control::Button { label, glyph } => {
                let rect = Rect::from_min_size(
                    pos2(right - width, centre_y - height / 2.0),
                    vec2(width, height),
                );
                // `ButtonFrame`'s `base` colour: `bg-surface-4 text-contrast`
                // (`ButtonFrame.vue:45`), with `[&>svg]:text-primary` for the glyph.
                painter.rect_filled(rect, CONTROL_RADIUS, theme.SUPER_RAISED_BG);
                let mut x = rect.left() + CONTROL_PADDING;
                if let Some(glyph) = glyph {
                    icons::draw(
                        painter,
                        glyph,
                        Rect::from_center_size(
                            pos2(x + CONTROL_GLYPH / 2.0, rect.center().y),
                            Vec2::splat(CONTROL_GLYPH),
                        ),
                        theme.BASE,
                    );
                    x += CONTROL_GLYPH + CONTROL_GLYPH_GAP;
                }
                painter.text(
                    pos2(x, rect.center().y),
                    Align2::LEFT_CENTER,
                    label,
                    fonts::semibold(CONTROL_TEXT_SIZE),
                    theme.CONTRAST,
                );
            }
        }
    }
}

/// Draw a switch: the brand track when on, `--surface-4` when off (`Toggle.vue:20`), and
/// the knob at the end the state puts it.
fn switch(painter: &egui::Painter, track: Rect, theme: &theme::Theme, on: bool) {
    let fill = if on {
        theme.BRAND
    } else {
        // `bg-button-bg` (`Toggle.vue:20`), which is `--surface-4` in every theme
        // (`variables.scss:98`, `:329`) -- the same value the theme exposes as
        // `SUPER_RAISED_BG` and `BUTTON_BG`.
        theme.SUPER_RAISED_BG
    };
    painter.rect_filled(track, SWITCH_RADIUS, fill);

    let fraction = if on { 1.0 } else { 0.0 };
    let left = knob_x(track, fraction);
    let knob = Rect::from_min_size(
        pos2(left, track.center().y - SWITCH_KNOB / 2.0),
        Vec2::splat(SWITCH_KNOB),
    );
    // The source's knob is `bg-black/90` when on and `bg-secondary` when off
    // (`Toggle.vue:24-25`); the brief asks for white on the brand track, which is what is
    // drawn, and the off knob keeps the source's secondary.
    let knob_fill = if on { Color32::WHITE } else { theme.SECONDARY };
    painter.circle_filled(knob.center(), SWITCH_KNOB / 2.0, knob_fill);
}

/// Where the knob's left edge sits on a track, for a state in `0.0..=1.0`.
///
/// The fraction is clamped, and a fraction that is not a number is treated as off, so a
/// caller cannot put the knob outside the track. The travel is what `Toggle.vue:29` states
/// as `translate-x-[24px]`: the track, less its `p-1` inset at both ends, less the knob.
fn knob_x(track: egui::Rect, fraction: f32) -> f32 {
    let fraction = if fraction.is_finite() {
        fraction.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let rest = track.left() + SWITCH_INSET;
    let travel = (track.width() - 2.0 * SWITCH_INSET - SWITCH_KNOB).max(0.0);
    rest + travel * fraction
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1440x900 screen, the reference's own window (`geometry::WINDOW`).
    fn screen() -> Rect {
        Rect::from_min_size(pos2(0.0, 0.0), vec2(1440.0, 900.0))
    }

    /// The plate is centred, so the two gutters are equal and the centre is the screen's.
    #[test]
    fn the_dialog_is_centred_in_the_screen() {
        let screen = screen();
        let dialog = dialog_rect(screen);

        assert!((dialog.center().x - screen.center().x).abs() < 0.001);
        assert!((dialog.center().y - screen.center().y).abs() < 0.001);
        assert!((dialog.left() - screen.left() - (screen.right() - dialog.right())).abs() < 0.001);
        assert!((dialog.top() - screen.top() - (screen.bottom() - dialog.bottom())).abs() < 0.001);

        // 928 wide, and 80 of header plus min(65% of 900, 640) = 585 of body.
        assert!((dialog.width() - DIALOG_WIDTH).abs() < 0.001);
        assert!((dialog.height() - 665.0).abs() < 0.001);
        assert!((header_rect(dialog).height() - 80.0).abs() < 0.001);
    }

    /// The body stops growing at the source's cap, and the plate gives way to the screen's
    /// 16-point margin before it ever overflows it.
    #[test]
    fn a_tall_screen_caps_the_body_and_a_narrow_one_caps_the_width() {
        let tall = dialog_rect(Rect::from_min_size(pos2(0.0, 0.0), vec2(1920.0, 1400.0)));
        assert!((tall.height() - (HEADER_HEIGHT + DIALOG_MAX_HEIGHT)).abs() < 0.001);

        let narrow = dialog_rect(Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0)));
        assert!((narrow.width() - (800.0 - 2.0 * DIALOG_MARGIN)).abs() < 0.001);
        assert!(narrow.left() >= DIALOG_MARGIN - 0.001);
        assert!(narrow.right() <= 800.0 - DIALOG_MARGIN + 0.001);
    }

    /// The rail's rows sit on one pitch inside a category: 40 of row and 4 of `gap-1`. A
    /// category heading is not a row, so the row after one is a heading's height further
    /// down -- 20 of line, 6 of `pb-1.5` and 12 of `mt-3`.
    #[test]
    fn the_rail_rows_sit_on_one_pitch() {
        let dialog = dialog_rect(screen());
        let rows = rail_rows(dialog);
        assert_eq!(rows.len(), TAB_COUNT);

        let first = rows[0];
        // The first row is under the Display heading: the rail's `p-4`, the heading's 20-point
        // line and its `pb-1.5`.
        let first_row_top =
            dialog.top() + HEADER_HEIGHT + RAIL_PADDING + CATEGORY_LINE + CATEGORY_PADDING;
        assert!((first.row.top() - first_row_top).abs() < 0.001);
        assert!((first.row.height() - RAIL_ROW_HEIGHT).abs() < 0.001);
        assert!(first.heading.is_some(), "the first row opens the Display group");

        let mut crossings = 0;
        for pair in rows.windows(2) {
            let pitch = pair[1].row.top() - pair[0].row.top();
            if pair[0].tab.category() == pair[1].tab.category() {
                assert!(
                    (pitch - (RAIL_ROW_HEIGHT + RAIL_ROW_GAP)).abs() < 0.001,
                    "same-category pitch: {pitch}"
                );
                assert!(pair[1].heading.is_none(), "a category is headed once");
            } else {
                crossings += 1;
                assert!(
                    pair[1].heading.is_some(),
                    "a new category starts with its heading"
                );
                let heading_pitch =
                    RAIL_ROW_HEIGHT + RAIL_ROW_GAP + CATEGORY_TOP_GAP + CATEGORY_LINE + CATEGORY_PADDING;
                assert!(
                    (pitch - heading_pitch).abs() < 0.001,
                    "crossing pitch: {pitch} against {heading_pitch}"
                );
            }
        }
        assert_eq!(crossings, 3, "Display, Account, Instance defaults, System");
    }

    /// The knob starts at the `p-1` inset, ends a knob short of the far inset, and moves
    /// half the travel for a half state. Out-of-range and non-finite fractions are clamped.
    #[test]
    fn the_switch_knob_travels_from_one_end_to_the_other() {
        let track = Rect::from_min_size(pos2(100.0, 40.0), vec2(SWITCH_WIDTH, SWITCH_HEIGHT));

        let off = knob_x(track, 0.0);
        let half = knob_x(track, 0.5);
        let on = knob_x(track, 1.0);

        assert!((off - (track.left() + SWITCH_INSET)).abs() < 0.001);
        assert!((on - (track.right() - SWITCH_INSET - SWITCH_KNOB)).abs() < 0.001);
        assert!((half - (off + on) / 2.0).abs() < 0.001);
        // `translate-x-[24px]`: 48 - 2 * 4 - 16.
        assert!((on - off - 24.0).abs() < 0.001);

        assert!((knob_x(track, -3.0) - off).abs() < 0.001);
        assert!((knob_x(track, 7.5) - on).abs() < 0.001);
        assert!((knob_x(track, f32::NAN) - off).abs() < 0.001);
        assert!(knob_x(track, 1.0) + SWITCH_KNOB <= track.right() - SWITCH_INSET + 0.001);
    }

    /// Every tab has its own non-empty label, and there are exactly 14 of them: the source's
    /// list, in its order, with no two rows reading the same.
    #[test]
    fn every_tab_has_its_own_label() {
        assert_eq!(TAB_COUNT, 14);
        assert_eq!(Tab::default(), Tab::Appearance);

        let mut seen: [&'static str; TAB_COUNT] = [""; TAB_COUNT];
        for (index, tab) in Tab::ALL.into_iter().enumerate() {
            let label = tab.label();
            assert!(!label.is_empty(), "tab {index} has a label");
            for earlier in &seen[..index] {
                assert_ne!(*earlier, label, "{label} is one tab's label, not two");
            }
            seen[index] = label;
        }

        // The source's order, by its first and last labels.
        assert_eq!(Tab::ALL[0].label(), "Appearance");
        assert_eq!(Tab::ALL[TAB_COUNT - 1].label(), "Troubleshooting");
    }
}
