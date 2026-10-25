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
//! **All fourteen tabs carry content.** Each tab's rows are ported in the source's own
//! order, with the labels and descriptions copied verbatim from the catalogues
//! (`apps/app-frontend/src/locales/en-US/index.json`, cited as `(:NNNN)`, plus the
//! `@modrinth/ui` catalogue for the keys the app's own file does not carry -- the Appearance
//! tab's strings and the shared Profile and Social tabs ship in
//! `packages/ui/src/locales/en-US/index.json`, cited as `(ui :NNNN)`). Two kinds of row are
//! not a straight copy: an ICU string is resolved exactly as the source resolves it
//! (`{platform}` is `app`, `{version}` is each version the source iterates), and the few
//! strings with no catalogue key at all -- the feature-flag names, a template literal, a
//! slider's own labels -- name the `.vue` line they come from instead.
//!
//! **The reference's field controls have no block here.** A text field, a slider, a chip
//! group and a table all draw as the closest thing this renderer has -- a select plate -- so
//! each tab's table says in its `// owed:` note which control the reference really uses, what
//! state it does not draw, and which rows it leaves out entirely.
//!
//! **The filled tabs overflow the pane.** The body is `min(65% of the screen, 640)` tall -- 585
//! points at a 900-tall screen -- and most of the fourteen tabs are longer than that now.
//! `draw_body` clips; the source scrolls the content column (`TabbedModal.vue:244-246`), so
//! scrolling it is the next thing this page owes.
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
    /// How far the selected tab's body is scrolled, in points.
    ///
    /// Every ported tab's body is taller than the pane, so without this the last rows of
    /// Behavior, Resources and Troubleshooting are unreachable -- the body clips, it does not
    /// scroll (`TabbedModal.vue:244-246` is the reference's own scroller). Kept as an offset
    /// rather than a `ScrollArea` because the body is drawn through a painter, not a `Ui`.
    pub scroll: f32,
    /// The height the body reported last frame, so the clamp knows where the end is.
    pub last_body_height: f32,
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
    /// Sync, startup and navigation, home and content, and confirmations.
    Behavior,
    /// The interface language.
    Language,
    /// Developer mode only: one switch per feature flag.
    FeatureFlags,
    /// The signed-in Modrinth account.
    Profile,
    /// Friend requests, instance invites, hosting access and blocked users.
    Social,
    /// Telemetry, Discord Rich Presence and ad consent.
    Privacy,
    /// The Java installations, memory allocation, Java arguments and environment for new
    /// instances.
    JavaAndMemory,
    /// The default game window size and fullscreen flag.
    Window,
    /// The hooks run around a launch.
    LaunchHooks,
    /// What is synced across instances and devices.
    Syncing,
    /// The app directory, download limits and stored content.
    Resources,
    /// Debug info, ad reports, the app cache and the database backups.
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

// --- Behavior (settings/display/behavior-settings.vue) ---
const BEHAVIOR_SYNC_LABEL: &str = "Sync behavior across devices"; // app.behavior-settings.sync-across-devices.title (:224)
const BEHAVIOR_SYNC_DESCRIPTION: &str = "Use these behavior settings everywhere you're signed in. Turn this off to keep separate settings on this device."; // app.behavior-settings.sync-across-devices.description (:218)
const BEHAVIOR_STARTUP_HEADING: &str = "Startup and navigation"; // app.behavior-settings.startup-and-navigation.title (:215)
const MINIMIZE_APP_LABEL: &str = "Minimize app"; // app.appearance-settings.minimize-launcher.title (:170)
const MINIMIZE_APP_DESCRIPTION: &str = "Minimize Modrinth App when Minecraft starts."; // app.appearance-settings.minimize-launcher.description (:167)
const REFOCUS_LABEL: &str = "Refocus app when Minecraft closes"; // app.behavior-settings.refocus-on-game-close.title (:212)
const REFOCUS_DESCRIPTION: &str = "Bring Modrinth App to the foreground when Minecraft exits."; // app.behavior-settings.refocus-on-game-close.description (:209)
const HIDE_SIDEBAR_LABEL: &str = "Hide right sidebar"; // app.appearance-settings.toggle-sidebar.title (:188)
const HIDE_SIDEBAR_DESCRIPTION: &str = "Hide the right sidebar by default and add a button to show or hide it."; // app.appearance-settings.toggle-sidebar.description (:185)
const BEHAVIOR_CONTENT_HEADING: &str = "Home and content"; // app.behavior-settings.content.title (:206)
const COMPACT_MODE_LABEL: &str = "Compact mode"; // app.appearance-settings.compact-mode.title (:152)
const COMPACT_MODE_DESCRIPTION: &str = "Display library instances in a compact row layout."; // app.appearance-settings.compact-mode.description (:149)
const PLAY_TIME_LABEL: &str = "Show play time"; // app.appearance-settings.show-play-time.title (:176)
const PLAY_TIME_DESCRIPTION: &str = "Show how long you've played each instance."; // app.appearance-settings.show-play-time.description (:173)
const HIDE_NAMETAG_LABEL: &str = "Hide nametag"; // app.appearance-settings.hide-nametag.title (:164)
const HIDE_NAMETAG_DESCRIPTION: &str = "Hide your username above the player preview on the Skin selector page."; // app.appearance-settings.hide-nametag.description (:161)
const BEHAVIOR_CONFIRMATIONS_HEADING: &str = "Confirmations"; // app.behavior-settings.confirmations.title (:203)
const UNKNOWN_PACK_LABEL: &str = "Warn me before installing unknown modpacks"; // app.appearance-settings.unknown-pack-warning.title (:194)
const UNKNOWN_PACK_DESCRIPTION: &str = "Show a safety warning before installing a Modrinth Pack (.mrpack) that isn't hosted on Modrinth."; // app.appearance-settings.unknown-pack-warning.description (:191)
const SKIP_WARNINGS_LABEL: &str = "Skip non-essential warnings"; // app.appearance-settings.skip-non-essential-warnings.title (:182)
const SKIP_WARNINGS_DESCRIPTION: &str = "Skip confirmations for low-risk actions such as duplicate installs, normal content deletion, bulk updates, unlinking, and repairs. Warnings for dangerous actions are always shown."; // app.appearance-settings.skip-non-essential-warnings.description (:179)

// --- Language (packages/ui/src/layouts/wrapped/settings/language-settings/index.vue) ---
const LANGUAGE_TITLE: &str = "Language"; // settings.language.title (ui :6356)
// The two long lines are the catalogue's, with `{platform}` resolved to `app`
// (`settings.language.platform.app`, ui :6350) exactly as `index.vue:54-60` resolves it.
const LANGUAGE_WARNING: &str = "Changing the app language may cause some content to appear in English if a translation is not available. The app is not yet fully translated, so some content may remain in English for certain languages."; // settings.language.warning (ui :6359)
const LANGUAGE_DESCRIPTION: &str = "Choose your preferred language for the app. Translations are contributed by volunteers <crowdin-link>on Crowdin</crowdin-link>."; // settings.language.description (ui :6338); the `<crowdin-link>` span is a link to translate.modrinth.com (`index.vue:16-20`)
const LANGUAGE_CATEGORY: &str = "Standard languages"; // settings.language.categories.default (ui :6323)
const LANGUAGE_CURRENT: &str = "English (United States)"; // locale.en-US (ui :2826), the name the list checks; the en-US locale's own name is the same string in `packages/ui/src/locales/en-US/meta.json:4`
const LANGUAGE_SEARCH_PLACEHOLDER: &str = "Search for a language..."; // settings.language.languages.search-field.placeholder (ui :6341)

// --- Feature flags (settings/display/feature-flag-settings.vue) ---
//
// These rows have no catalogue keys. The source lists `DEFAULT_FEATURE_FLAGS`'s keys and
// draws each with its underscores turned into spaces (`feature-flag-settings.vue:59`) under
// CSS `capitalize` (`:58`), so the text below is that transformation of the flag's name, not
// a translated string. The flag ids are in `use-app-settings.ts:4-13`, in this order.
const FLAGS_SEARCH_PLACEHOLDER: &str = "Search flags..."; // the search field's placeholder, feature-flag-settings.vue:53 -- a literal in the template, not a key
const FLAG_PROJECT_BACKGROUND: &str = "Project Background"; // `project_background`, use-app-settings.ts:4, off by default
const FLAG_PAGE_PATH: &str = "Page Path"; // `page_path`, use-app-settings.ts:5, off
const FLAG_SERVER_PROJECT_QA: &str = "Server Project Qa"; // `server_project_qa`, use-app-settings.ts:6, off
const FLAG_VERSION_ENVIRONMENT: &str = "Show Version Environment Column"; // `show_version_environment_column`, use-app-settings.ts:7, off
const FLAG_SERVER_RAM_BYTES: &str = "Server Ram As Bytes Always On"; // `server_ram_as_bytes_always_on`, use-app-settings.ts:8, off
const FLAG_APP_CONTROLS: &str = "Always Show App Controls"; // `always_show_app_controls`, use-app-settings.ts:9, off
const FLAG_SYNC_UPDATE_MODAL: &str = "Show Sync Instances Update Modal"; // `show_sync_instances_update_modal`, use-app-settings.ts:10, off
const FLAG_PRIDE_FUNDRAISER: &str = "Pride Fundraiser"; // `pride_fundraiser`, use-app-settings.ts:11 -- the one flag that starts on
const FLAG_I18N_DEBUG: &str = "I18n Debug"; // `i18n_debug`, use-app-settings.ts:12, off
const FLAG_LOCALHOST_SIGN_IN: &str = "Localhost Sign In"; // `localhost_sign_in`, use-app-settings.ts:13, off

// --- Profile (packages/ui/src/layouts/wrapped/settings/AccountProfileSettings.vue) ---
const PROFILE_PICTURE_LABEL: &str = "Profile picture"; // settings.profile.profile-picture.title (ui :6380)
const PROFILE_DESCRIPTION: &str = "Your profile information is publicly <profile-link>viewable on Modrinth</profile-link> and through the <docs-link>Modrinth API</docs-link>."; // settings.profile.public-information.description (ui :6383); both spans are links (AccountProfileSettings.vue:27-40)
const PROFILE_UPLOAD_LABEL: &str = "Upload image"; // button.upload-image (ui :338), the avatar `FileButton`'s prompt (AccountProfileSettings.vue:59)
const PROFILE_REMOVE_LABEL: &str = "Remove image"; // button.remove-image (ui :272), drawn while an avatar is set (AccountProfileSettings.vue:66-74)
const PROFILE_USERNAME_LABEL: &str = "Username"; // label.username (ui :2789), the field's heading (AccountProfileSettings.vue:89-91)
const PROFILE_USERNAME_DESCRIPTION: &str = "A unique case-insensitive name to identify your profile."; // settings.profile.username.description (ui :6398)
const PROFILE_BIO_LABEL: &str = "Bio"; // settings.profile.bio.title (ui :6374)
const PROFILE_BIO_DESCRIPTION: &str = "A short description to tell everyone a little bit about you."; // settings.profile.bio.description (ui :6371)

// --- Social (packages/ui/src/layouts/wrapped/settings/AccountSocialSettings.vue) ---
const SOCIAL_FRIEND_REQUESTS_LABEL: &str = "Friend requests"; // settings.social.friend-requests.title (ui :6452)
const SOCIAL_FRIEND_REQUESTS_DESCRIPTION: &str = "Control who can send you friend requests on Modrinth."; // settings.social.friend-requests.description (ui :6449)
const SOCIAL_SHARED_INSTANCES_LABEL: &str = "Shared instance invites"; // settings.social.shared-instance-invites.title (ui :6485)
const SOCIAL_SHARED_INSTANCES_DESCRIPTION: &str = "Control who can send you invites to shared instances."; // settings.social.shared-instance-invites.description (ui :6482)
const SOCIAL_HOSTING_LABEL: &str = "Hosting access invites"; // settings.social.hosting-access.title (ui :6458)
const SOCIAL_HOSTING_DESCRIPTION: &str = "Control who can invite you to manage a Modrinth Hosting server."; // settings.social.hosting-access.description (ui :6455)
const SOCIAL_BLOCKED_LABEL: &str = "Blocked users"; // settings.social.blocked-users.title (ui :6431)
const SOCIAL_BLOCKED_DESCRIPTION: &str = "These are the users you have blocked on Modrinth. They cannot:"; // settings.social.blocked-users.description (ui :6410)
const SOCIAL_EVERYONE: &str = "Everyone"; // settings.social.interaction-source.everyone (ui :6461), the start for all three settings (AccountSocialSettings.vue:254-256)

// --- Privacy (settings/account/privacy-settings.vue) ---
const PRIVACY_ADS_HEADING: &str = "Your privacy and how ads support Modrinth"; // app.ads-consent.title (:146)
const PRIVACY_ADS_INTRO: &str = "Ads make Modrinth possible and fund creator payouts. Our partners may store or access cookies in the app to personalize ads and measure performance. You can opt out or manage your preferences below."; // app.settings.privacy.ads-consent.intro (:2756)
const PRIVACY_MANAGE_LABEL: &str = "Manage preferences"; // app.ads-consent.manage (:140)
const TELEMETRY_LABEL: &str = "Telemetry"; // app.settings.privacy.telemetry.title (:2768)
const TELEMETRY_DESCRIPTION: &str = "Modrinth collects anonymized analytics and usage data to improve our user experience and customize your experience. By disabling this option, you opt out and your data will no longer be collected."; // app.settings.privacy.telemetry.description (:2765)
const DISCORD_LABEL: &str = "Discord Rich Presence"; // app.settings.privacy.discord-rich-presence.title (:2762)
const DISCORD_DESCRIPTION: &str = "Show Modrinth App as your current activity on Discord. This does not affect Rich Presence added to instances by mods. Requires an app restart."; // app.settings.privacy.discord-rich-presence.description (:2759)

// --- Java and memory (settings/instance-defaults/java-and-memory-settings/index.vue) ---
const JAVA_INSTALLATIONS_HEADING: &str = "Java installations"; // app.settings.tabs.java-installations (:3137), the section's own `text-xl` heading (:69-71)
// The per-version labels are one catalogue string with `{version}` filled in from the
// `[25, 21, 17, 8]` list `java-installations.vue:34` iterates.
const JAVA_LOCATION_25: &str = "Java 25 location"; // app.settings.java-installations.location.title (:2750)
const JAVA_LOCATION_21: &str = "Java 21 location"; // the same key, app.settings.java-installations.location.title (:2750), second version
const JAVA_LOCATION_17: &str = "Java 17 location"; // the same key, app.settings.java-installations.location.title (:2750), third version
const JAVA_LOCATION_8: &str = "Java 8 location"; // the same key, app.settings.java-installations.location.title (:2750), fourth version
const JAVA_PATH_PLACEHOLDER: &str = "/path/to/java"; // app.java-selector.path.placeholder (:992), the field inside each row's selector (JavaSelector.vue:9, :114-117)
const MEMORY_TITLE: &str = "Memory allocation"; // app.settings.default-instance-options.memory-allocation.title (:1694)
const MEMORY_DESCRIPTION: &str = "Maximum memory available to each instance."; // app.settings.default-instance-options.memory-allocation.description (:1691)
const MEMORY_MIN_VALUE: &str = "512 MB"; // the slider's own `min-label` (java-and-memory-settings/index.vue:90); its `:min` is 512 and its `:step` 64 (index.vue:85, :87)
const JAVA_ARGS_TITLE: &str = "Java arguments"; // app.settings.default-instance-options.java-arguments.title (:1688)
const JAVA_ARGS_DESCRIPTION: &str = "Arguments passed to Java when launching an instance."; // app.settings.default-instance-options.java-arguments.description (:1682)
const JAVA_ARGS_PLACEHOLDER: &str = "Enter Java arguments..."; // app.settings.default-instance-options.java-arguments.placeholder (:1685)
const ENV_VARS_TITLE: &str = "Environment variables"; // app.settings.default-instance-options.environment-variables.title (:1664)
const ENV_VARS_DESCRIPTION: &str = "Environment variables set when launching an instance."; // app.settings.default-instance-options.environment-variables.description (:1658)
const ENV_VARS_PLACEHOLDER: &str = "Enter environment variables..."; // app.settings.default-instance-options.environment-variables.placeholder (:1661)

// --- Window (settings/instance-defaults/window-settings.vue) ---
const WINDOW_FULLSCREEN_LABEL: &str = "Fullscreen"; // app.settings.default-instance-options.fullscreen.title (:1670)
const WINDOW_FULLSCREEN_DESCRIPTION: &str = "Start instances in fullscreen by updating their options.txt file."; // app.settings.default-instance-options.fullscreen.description (:1667)
const WINDOW_WIDTH_LABEL: &str = "Width"; // app.settings.default-instance-options.width.title (:1721)
const WINDOW_WIDTH_DESCRIPTION: &str = "The width of the game window when launched."; // app.settings.default-instance-options.width.description (:1715)
const WINDOW_WIDTH_PLACEHOLDER: &str = "Enter width..."; // app.settings.default-instance-options.width.placeholder (:1718)
const WINDOW_HEIGHT_LABEL: &str = "Height"; // app.settings.default-instance-options.height.title (:1679)
const WINDOW_HEIGHT_DESCRIPTION: &str = "The height of the game window when launched."; // app.settings.default-instance-options.height.description (:1673)
const WINDOW_HEIGHT_PLACEHOLDER: &str = "Enter height..."; // app.settings.default-instance-options.height.placeholder (:1676)

// --- Launch hooks (settings/instance-defaults/launch-hooks-settings.vue) ---
const PRE_LAUNCH_LABEL: &str = "Pre-launch hook"; // app.settings.default-instance-options.pre-launch-hook.title (:1712)
const PRE_LAUNCH_DESCRIPTION: &str = "Runs before the instance starts."; // app.settings.default-instance-options.pre-launch-hook.description (:1706)
const PRE_LAUNCH_PLACEHOLDER: &str = "Enter pre-launch command..."; // app.settings.default-instance-options.pre-launch-hook.placeholder (:1709)
const WRAPPER_LABEL: &str = "Wrapper hook"; // app.settings.default-instance-options.wrapper-hook.title (:1730)
const WRAPPER_DESCRIPTION: &str = "Command used to wrap the Minecraft launch process."; // app.settings.default-instance-options.wrapper-hook.description (:1724)
const WRAPPER_PLACEHOLDER: &str = "Enter wrapper command..."; // app.settings.default-instance-options.wrapper-hook.placeholder (:1727)
const POST_EXIT_LABEL: &str = "Post-exit hook"; // app.settings.default-instance-options.post-exit-hook.title (:1703)
const POST_EXIT_DESCRIPTION: &str = "Runs after the game closes."; // app.settings.default-instance-options.post-exit-hook.description (:1697)
const POST_EXIT_PLACEHOLDER: &str = "Enter post-exit command..."; // app.settings.default-instance-options.post-exit-hook.placeholder (:1700)
const HOOK_VARIABLES_INTRO: &str = "Hooks run in the working directory of the instance, with the following variables:"; // instance.settings.tabs.hooks.variables.description (:4226)

// --- Syncing (settings/syncing/syncing-settings.vue) ---
const SYNC_GAME_OPTIONS_LABEL: &str = "Sync game options"; // app.settings.synced-options.game-settings (:2981)
const SYNC_GAME_OPTIONS_DESCRIPTION: &str = "Use the same game options across your instances."; // app.settings.synced-options.game-settings.description (:2996)
const SYNC_SERVERS_LABEL: &str = "Sync multiplayer servers"; // app.settings.synced-options.multiplayer-servers (:3065)
const SYNC_SERVERS_DESCRIPTION: &str = "Use the same multiplayer servers across your instances."; // app.settings.synced-options.multiplayer-servers.description (:3077)
const SYNC_RESOURCE_PACKS_LABEL: &str = "Sync resource packs"; // app.settings.synced-options.resource-packs (:3116)
const SYNC_RESOURCE_PACKS_DESCRIPTION: &str = "Use the same resource packs across your instances."; // app.settings.synced-options.resource-packs.description (:3119)
const SYNC_DATA_PACKS_LABEL: &str = "Sync data packs"; // app.settings.synced-options.data-packs (:2966)
// The catalogue has no full stop on this one; it is copied as it is.
const SYNC_DATA_PACKS_DESCRIPTION: &str = "Use the same datapacks across your instances"; // app.settings.synced-options.data-packs.description (:2969)
const SYNC_COMMAND_HISTORY_LABEL: &str = "Sync command history"; // app.settings.synced-options.command-history (:2951)
const SYNC_COMMAND_HISTORY_DESCRIPTION: &str = "Use the same command history across your instances."; // app.settings.synced-options.command-history.description (:2954)
const SYNC_HOTBARS_LABEL: &str = "Sync saved creative hotbars"; // app.settings.synced-options.creative-hotbars (:2960)
const SYNC_HOTBARS_DESCRIPTION: &str = "Use the same saved creative hotbars across your instances."; // app.settings.synced-options.creative-hotbars.description (:2963)

// --- Resources (settings/system/resource-management-settings/index.vue, content-storage.vue) ---
const STORE_TITLE: &str = "Content storage"; // app.settings.resource-management.store.title (:2879)
const STORE_DESCRIPTION: &str = "Save space by reusing game installs, mods and packs across your instances."; // app.settings.resource-management.store.description (:2855)
const STORE_REPAIR_LABEL: &str = "Verify and repair"; // app.settings.resource-management.store.repair (:2864)
const STORE_CLEAR_LABEL: &str = "Clear unused"; // app.settings.resource-management.store.clear (:2843)
const STORE_CACHE_LIMIT_LABEL: &str = "Keep unused downloads"; // app.settings.resource-management.store.cache-limit.label (:2834)
const STORE_CACHE_LIMIT_DESCRIPTION: &str = "Keep up to this much unused content for future installs. Keeping more can save download time."; // app.settings.resource-management.store.cache-limit.description (:2831)
const STORE_CACHE_LIMIT_UNIT: &str = "GiB"; // app.settings.resource-management.store.cache-limit.unit (:2837); the number before it defaults to 5 (`content-storage.vue:60`)
const APP_DIRECTORY_LABEL: &str = "App directory"; // app.settings.resource-management.app-directory.title (:2804)
const APP_DIRECTORY_DESCRIPTION: &str = "Where Modrinth App stores instances and other files. Changes take effect after restarting the app."; // app.settings.resource-management.app-directory.description (:2798)
const APP_DIRECTORY_BROWSE: &str = "Browse for an app directory"; // app.settings.resource-management.app-directory.browse (:2795), the folder picker's label and tooltip
const MAX_DOWNLOADS_LABEL: &str = "Maximum concurrent downloads"; // app.settings.resource-management.maximum-concurrent-downloads.title (:2819)
const MAX_DOWNLOADS_DESCRIPTION: &str = "Number of files the app can download at once. Lower this if downloads are unreliable on your connection. Requires an app restart."; // app.settings.resource-management.maximum-concurrent-downloads.description (:2816)
const MAX_DOWNLOADS_VALUE: &str = "1"; // its slider's `:min="1"` (resource-management-settings/index.vue:116), drawn at the floor; its `:max` is 10 (index.vue:117)
const MAX_WRITES_LABEL: &str = "Maximum concurrent writes"; // app.settings.resource-management.maximum-concurrent-writes.title (:2825)
const MAX_WRITES_DESCRIPTION: &str = "Number of files the app can write to disk at once. Lower this if you frequently encounter I/O errors. Requires an app restart."; // app.settings.resource-management.maximum-concurrent-writes.description (:2822)
const MAX_WRITES_VALUE: &str = "1"; // its slider's `:min="1"` (resource-management-settings/index.vue:132), drawn at the floor; its `:max` is 50 (index.vue:133)

// --- Troubleshooting (settings/system/troubleshooting-settings.vue) ---
const APP_CACHE_TITLE: &str = "App cache"; // app.settings.resource-management.app-cache.title (:2783)
const APP_CACHE_DESCRIPTION: &str = "Clear cached data and download it again from Modrinth. The app may load more slowly until the cache is rebuilt."; // app.settings.resource-management.app-cache.description (:2780)
const CLEAR_CACHE_LABEL: &str = "Clear cache"; // app.settings.troubleshooting.clear-cache (:3161)
const BACKUPS_TITLE: &str = "App database backups"; // app.settings.resource-management.app-database-backups.title (:2792)
const BACKUPS_DESCRIPTION: &str = "Backups of important app data are stored here in case you need to recover them later."; // app.settings.resource-management.app-database-backups.description (:2786)
const BACKUPS_OPEN_LABEL: &str = "Open backups folder"; // app.settings.resource-management.app-database-backups.open-folder (:2789)
const DEBUG_INFO_TITLE: &str = "Debug info"; // app.settings.resource-management.debug-info.title (:2813)
const DEBUG_INFO_DESCRIPTION: &str = "Export app logs and diagnostic data to a ZIP file to share with Modrinth support."; // app.settings.resource-management.debug-info.description (:2807)
const DEBUG_INFO_EXPORT_LABEL: &str = "Export debug info"; // app.settings.resource-management.debug-info.export (:2810)
const REPORT_AD_TITLE: &str = "Report an ad"; // app.settings.troubleshooting.report-ad.title (:3182)
const REPORT_AD_DESCRIPTION: &str = "Seen an inappropriate or offensive ad? Copy its details while it's showing and send them to <support>Modrinth Support</support>. We'll review it and remove it."; // app.settings.troubleshooting.report-ad.description (:3173); the `<support>` span is a link to support.modrinth.com (troubleshooting-settings.vue:299-309)
const REPORT_AD_COPY_LABEL: &str = "Copy ad details"; // app.settings.troubleshooting.report-ad.copy (:3170)
const COPY_DETAILS_LABEL: &str = "Always show copy details"; // app.settings.resource-management.always-show-copy-details.title (:2774)
const COPY_DETAILS_DESCRIPTION: &str = "Show the Copy details action while an install is queued or running. It is always available for failed or interrupted installs."; // app.settings.resource-management.always-show-copy-details.description (:2771)

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

/// The Behavior tab, in the source's order: the sync row (`behavior-settings.vue:241-266`),
/// then `Startup and navigation` with three switches (`:268-312`), `Home and content` with
/// three (`:314-349`) and `Confirmations` with two (`:351-383`). The two rules are the
/// `mt-8 border-t` sections (`:314`, `:351`); the first section's own `border-b` is not
/// drawn, as on the Features tab.
///
/// The switch states are the app's own starting values: `use-app-settings.ts:45-53` holds
/// `refocusOnGameClose: false`, `compactInstanceCards: false`, `showPlayTime: true`,
/// `warnOnUnknownModpacks: true`, `skipNonEssentialWarnings: false`,
/// `hideNametagSkinsPage: false` and `toggleSidebar: false`, and the sync ref starts off
/// (`:19`). `hide_on_process_start` -- the `Minimize app` switch (`:282`) -- has no mirror in
/// that file; its default lives in the Rust settings store, which this checkout does not
/// carry, so it is drawn off with the rest.
///
/// `// owed:` the signed-out tooltip on the sync switch
/// (`app.behavior-settings.sync-across-devices.signed-out-tooltip`, :221) needs a tooltip
/// this pass has no renderer for.
const BEHAVIOR_BLOCKS: &[Block] = &[
    Block::Heading {
        text: BEHAVIOR_SYNC_LABEL,
        description: Some(BEHAVIOR_SYNC_DESCRIPTION),
        big: false,
        // Signed out, the source draws this off and disabled (`:257-263`).
        control: Some(Control::Switch { on: false }),
    },
    Block::Heading {
        text: BEHAVIOR_STARTUP_HEADING,
        description: None,
        big: true,
        control: None,
    },
    Block::Row {
        label: MINIMIZE_APP_LABEL,
        description: Some(MINIMIZE_APP_DESCRIPTION),
        control: Control::Switch { on: false },
    },
    Block::Row {
        label: REFOCUS_LABEL,
        description: Some(REFOCUS_DESCRIPTION),
        control: Control::Switch { on: false },
    },
    Block::Row {
        label: HIDE_SIDEBAR_LABEL,
        description: Some(HIDE_SIDEBAR_DESCRIPTION),
        control: Control::Switch { on: false },
    },
    Block::Rule,
    Block::Heading {
        text: BEHAVIOR_CONTENT_HEADING,
        description: None,
        big: true,
        control: None,
    },
    Block::Row {
        label: COMPACT_MODE_LABEL,
        description: Some(COMPACT_MODE_DESCRIPTION),
        control: Control::Switch { on: false },
    },
    Block::Row {
        label: PLAY_TIME_LABEL,
        description: Some(PLAY_TIME_DESCRIPTION),
        control: Control::Switch { on: true },
    },
    Block::Row {
        label: HIDE_NAMETAG_LABEL,
        description: Some(HIDE_NAMETAG_DESCRIPTION),
        control: Control::Switch { on: false },
    },
    Block::Rule,
    Block::Heading {
        text: BEHAVIOR_CONFIRMATIONS_HEADING,
        description: None,
        big: true,
        control: None,
    },
    Block::Row {
        label: UNKNOWN_PACK_LABEL,
        description: Some(UNKNOWN_PACK_DESCRIPTION),
        control: Control::Switch { on: true },
    },
    Block::Row {
        label: SKIP_WARNINGS_LABEL,
        description: Some(SKIP_WARNINGS_DESCRIPTION),
        control: Control::Switch { on: false },
    },
];

/// The Language tab, in the source's order: the `Language` heading
/// (`language-settings/index.vue:3-5`), the translation warning (`:7-9`) and the description
/// (`:11-22`), then the locale list (`:24-31`).
///
/// `// owed:` the reference draws a search field over a radio list with one
/// `CheckCircleButton` per locale, each with its flag, its translated name and its coverage
/// percentage (`language-settings-selector.vue:221-297`). There is no list block and no
/// locale data here, so the list is drawn as one select showing the current locale and the
/// search field as a select plate holding its placeholder. The warning is an `Admonition`
/// (`:7`), drawn here as the heading's muted line. Two strings belong to the list and come
/// with it: `settings.language.languages.search.no-results` (ui :6347) and
/// `settings.language.coverage.label` (ui :6332).
const LANGUAGE_BLOCKS: &[Block] = &[
    Block::Heading {
        text: LANGUAGE_TITLE,
        description: Some(LANGUAGE_WARNING),
        big: true,
        control: None,
    },
    Block::Heading {
        text: LANGUAGE_CATEGORY,
        description: Some(LANGUAGE_DESCRIPTION),
        big: false,
        control: Some(Control::Select {
            value: LANGUAGE_CURRENT,
        }),
    },
    Block::Standalone(Control::Select {
        value: LANGUAGE_SEARCH_PLACEHOLDER,
    }),
];

/// The Feature flags tab: the search field (`feature-flag-settings.vue:49-55`), then one row
/// per entry in `DEFAULT_FEATURE_FLAGS` (`use-app-settings.ts:3-14`) in its declaration
/// order. Each switch is the flag's own default, so only `Pride Fundraiser` starts on.
///
/// `// owed:` every row also carries a quiet `Reset to default` button (`:63-69`), and the
/// list is filtered with Fuse (`:20-32`) with `No flags found matching` for an empty result
/// (`:77-79`, a template literal). A row holds one control here, and the search plate is
/// drawn rather than searched: both are owed.
const FEATURE_FLAG_BLOCKS: &[Block] = &[
    Block::Standalone(Control::Select {
        value: FLAGS_SEARCH_PLACEHOLDER,
    }),
    Block::Row {
        label: FLAG_PROJECT_BACKGROUND,
        description: None,
        control: Control::Switch { on: false },
    },
    Block::Row {
        label: FLAG_PAGE_PATH,
        description: None,
        control: Control::Switch { on: false },
    },
    Block::Row {
        label: FLAG_SERVER_PROJECT_QA,
        description: None,
        control: Control::Switch { on: false },
    },
    Block::Row {
        label: FLAG_VERSION_ENVIRONMENT,
        description: None,
        control: Control::Switch { on: false },
    },
    Block::Row {
        label: FLAG_SERVER_RAM_BYTES,
        description: None,
        control: Control::Switch { on: false },
    },
    Block::Row {
        label: FLAG_APP_CONTROLS,
        description: None,
        control: Control::Switch { on: false },
    },
    Block::Row {
        label: FLAG_SYNC_UPDATE_MODAL,
        description: None,
        control: Control::Switch { on: false },
    },
    Block::Row {
        label: FLAG_PRIDE_FUNDRAISER,
        description: None,
        control: Control::Switch { on: true },
    },
    Block::Row {
        label: FLAG_I18N_DEBUG,
        description: None,
        control: Control::Switch { on: false },
    },
    Block::Row {
        label: FLAG_LOCALHOST_SIGN_IN,
        description: None,
        control: Control::Switch { on: false },
    },
];

/// The Profile tab's signed-in body (`AccountProfileSettings.vue:25-125`): the lead paragraph
/// (`:26-41`), the profile picture section with its upload and remove buttons (`:49-86`), then
/// username (`:88-110`) and bio (`:112-123`). The paragraph has no block of its own -- it is
/// not a heading -- so it is drawn as the first heading's muted line, and the `hr` under it
/// (`:43-47`) is not drawn.
///
/// `// owed:` both fields are an `Input` and a `Textarea` holding the account's own username
/// and bio, with a `39`- and a `160`-character counter (`:93-106`, `:116-119`); there is no
/// text field here, so each is drawn as a select plate carrying the field's own name where
/// the value would be. Three states are owed with them: the `Reset` button, which appears
/// only while a change is pending (`button.reset`, ui :287, `:75-83`); the signed-out
/// `EmptyState` that replaces the whole body (`settings.profile.sign-in-required.title` and
/// `.description`, ui :6395, :6392); and the lead paragraph's two links, drawn as their
/// literal markup.
const PROFILE_BLOCKS: &[Block] = &[
    Block::Heading {
        text: PROFILE_PICTURE_LABEL,
        description: Some(PROFILE_DESCRIPTION),
        big: false,
        control: None,
    },
    Block::Standalone(Control::Button {
        label: PROFILE_UPLOAD_LABEL,
        glyph: Some(Glyph::Upload),
    }),
    Block::Standalone(Control::Button {
        label: PROFILE_REMOVE_LABEL,
        glyph: Some(Glyph::Trash),
    }),
    Block::Row {
        label: PROFILE_USERNAME_LABEL,
        description: Some(PROFILE_USERNAME_DESCRIPTION),
        control: Control::Select {
            value: PROFILE_USERNAME_LABEL,
        },
    },
    Block::Row {
        label: PROFILE_BIO_LABEL,
        description: Some(PROFILE_BIO_DESCRIPTION),
        control: Control::Select { value: PROFILE_BIO_LABEL },
    },
];

/// The Social tab's signed-in body (`AccountSocialSettings.vue:25-202`): friend requests
/// (`:26-44`), shared instance invites (`:46-63`) and hosting access invites (`:65-82`) --
/// each a heading, its description and a group of chips -- then blocked users (`:85-102`).
///
/// `// owed:` the three settings are `Chips` groups, a choice of `everyone`, `mutual` and
/// `none` or `everyone`, `friends` and `none` (`:31-40`, `:257-258`), drawn here as a select
/// showing the start, `everyone` (`:254-256`). The blocked-users table (`:100-201`) is a row
/// per blocked user with an `Unblock` action (`settings.social.blocked-users.unblock`,
/// ui :6434), and the three restriction bullets (`:93-97`) are list items; the first is owed
/// for its data and the second for want of a list block. The signed-out state
/// (`settings.social.sign-in-required.title` / `.description`, ui :6491, :6488) is owed too.
const SOCIAL_BLOCKS: &[Block] = &[
    Block::Heading {
        text: SOCIAL_FRIEND_REQUESTS_LABEL,
        description: Some(SOCIAL_FRIEND_REQUESTS_DESCRIPTION),
        big: false,
        control: Some(Control::Select { value: SOCIAL_EVERYONE }),
    },
    Block::Heading {
        text: SOCIAL_SHARED_INSTANCES_LABEL,
        description: Some(SOCIAL_SHARED_INSTANCES_DESCRIPTION),
        big: false,
        control: Some(Control::Select { value: SOCIAL_EVERYONE }),
    },
    Block::Heading {
        text: SOCIAL_HOSTING_LABEL,
        description: Some(SOCIAL_HOSTING_DESCRIPTION),
        big: false,
        control: Some(Control::Select { value: SOCIAL_EVERYONE }),
    },
    Block::Heading {
        text: SOCIAL_BLOCKED_LABEL,
        description: Some(SOCIAL_BLOCKED_DESCRIPTION),
        big: false,
        control: None,
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

/// The Java and memory tab (`java-and-memory-settings/index.vue:66-150`): `Java
/// installations` (`:68-73`) with one row per version in `java-installations.vue:34`'s
/// `[25, 21, 17, 8]`, then the `border-t` group (`:74`) holding memory allocation
/// (`:76-106`), Java arguments (`:107-123`) and environment variables (`:124-140`).
///
/// `// owed:` memory allocation is a `Slider` from 512 MB to the machine's memory in 64 MB
/// steps (`:80-93`) and the two argument fields are `Input`s (`:111-119`, `:128-136`); with no
/// slider and no text field, each is drawn as a select plate -- the memory one showing the
/// slider's own `512 MB` min-label, the fields their placeholders. Each Java row's selector is
/// an `Input` with `Detect` and `Browse` buttons (`JavaSelector.vue:9`, `:75-87`); the row
/// draws only its path placeholder.
const JAVA_AND_MEMORY_BLOCKS: &[Block] = &[
    Block::Heading {
        text: JAVA_INSTALLATIONS_HEADING,
        description: None,
        big: true,
        control: None,
    },
    Block::Row {
        label: JAVA_LOCATION_25,
        description: None,
        control: Control::Select {
            value: JAVA_PATH_PLACEHOLDER,
        },
    },
    Block::Row {
        label: JAVA_LOCATION_21,
        description: None,
        control: Control::Select {
            value: JAVA_PATH_PLACEHOLDER,
        },
    },
    Block::Row {
        label: JAVA_LOCATION_17,
        description: None,
        control: Control::Select {
            value: JAVA_PATH_PLACEHOLDER,
        },
    },
    Block::Row {
        label: JAVA_LOCATION_8,
        description: None,
        control: Control::Select {
            value: JAVA_PATH_PLACEHOLDER,
        },
    },
    Block::Rule,
    Block::Heading {
        text: MEMORY_TITLE,
        description: Some(MEMORY_DESCRIPTION),
        big: false,
        control: None,
    },
    Block::Standalone(Control::Select {
        value: MEMORY_MIN_VALUE,
    }),
    Block::Heading {
        text: JAVA_ARGS_TITLE,
        description: Some(JAVA_ARGS_DESCRIPTION),
        big: false,
        control: None,
    },
    Block::Standalone(Control::Select {
        value: JAVA_ARGS_PLACEHOLDER,
    }),
    Block::Heading {
        text: ENV_VARS_TITLE,
        description: Some(ENV_VARS_DESCRIPTION),
        big: false,
        control: None,
    },
    Block::Standalone(Control::Select {
        value: ENV_VARS_PLACEHOLDER,
    }),
];

/// The Window tab (`window-settings.vue:60-125`): fullscreen (`:63-77`), then the width
/// (`:78-96`) and height (`:97-115`) fields, which the source disables while fullscreen is on
/// (`:91`, `:110`).
///
/// `// owed:` the two fields are `Input type="number"`s, drawn here as select plates holding
/// their placeholders, and neither shows the disabled state the fullscreen switch puts them
/// in. `force_fullscreen`'s default is not in this checkout -- it lives in the Rust settings
/// store -- so the switch is drawn off.
const WINDOW_BLOCKS: &[Block] = &[
    Block::Row {
        label: WINDOW_FULLSCREEN_LABEL,
        description: Some(WINDOW_FULLSCREEN_DESCRIPTION),
        control: Control::Switch { on: false },
    },
    Block::Row {
        label: WINDOW_WIDTH_LABEL,
        description: Some(WINDOW_WIDTH_DESCRIPTION),
        control: Control::Select {
            value: WINDOW_WIDTH_PLACEHOLDER,
        },
    },
    Block::Row {
        label: WINDOW_HEIGHT_LABEL,
        description: Some(WINDOW_HEIGHT_DESCRIPTION),
        control: Control::Select {
            value: WINDOW_HEIGHT_PLACEHOLDER,
        },
    },
];

/// The Launch hooks tab (`launch-hooks-settings.vue:82-156`): pre-launch (`:85-101`), wrapper
/// (`:102-118`) and post-exit (`:119-135`) hooks, then the variable list (`:136-146`).
///
/// `// owed:` the three hooks are `Input type="text"`s, drawn here as select plates holding
/// their placeholders. The six `$INST_*` lines (`:138-145`,
/// `instance.settings.tabs.hooks.variables.*.description`, :4229-4244) are `<li>` items with
/// no list block to draw them in, so only the line that introduces them is ported.
const LAUNCH_HOOKS_BLOCKS: &[Block] = &[
    Block::Heading {
        text: PRE_LAUNCH_LABEL,
        description: Some(PRE_LAUNCH_DESCRIPTION),
        big: false,
        control: None,
    },
    Block::Standalone(Control::Select {
        value: PRE_LAUNCH_PLACEHOLDER,
    }),
    Block::Heading {
        text: WRAPPER_LABEL,
        description: Some(WRAPPER_DESCRIPTION),
        big: false,
        control: None,
    },
    Block::Standalone(Control::Select {
        value: WRAPPER_PLACEHOLDER,
    }),
    Block::Heading {
        text: POST_EXIT_LABEL,
        description: Some(POST_EXIT_DESCRIPTION),
        big: false,
        control: None,
    },
    Block::Standalone(Control::Select {
        value: POST_EXIT_PLACEHOLDER,
    }),
    Block::Heading {
        text: HOOK_VARIABLES_INTRO,
        description: None,
        big: false,
        control: None,
    },
];

/// The Syncing tab (`syncing-settings.vue:563-635`): one row per entry in `globalRows`
/// (`:184-225`), in its order -- game options, multiplayer servers, resource packs, data
/// packs, command history and creative hotbars -- each a label, a description and a toggle.
/// All six start off: `defaultGlobalOptions` (`:229-237`).
///
/// `// owed:` five of the six rows also carry a circular edit `IconButton` (`:601-621`) whose
/// label is `commonMessages.editButton` (`button.edit`, ui :215) or, for game options,
/// `app.settings.synced-options.game-settings.button` (:2984). A row holds one control here,
/// so the switch is drawn and the edit button is owed, with the tooltips that say why it is
/// disabled (`:376-398`) and the four modals it opens.
const SYNCING_BLOCKS: &[Block] = &[
    Block::Row {
        label: SYNC_GAME_OPTIONS_LABEL,
        description: Some(SYNC_GAME_OPTIONS_DESCRIPTION),
        control: Control::Switch { on: false },
    },
    Block::Row {
        label: SYNC_SERVERS_LABEL,
        description: Some(SYNC_SERVERS_DESCRIPTION),
        control: Control::Switch { on: false },
    },
    Block::Row {
        label: SYNC_RESOURCE_PACKS_LABEL,
        description: Some(SYNC_RESOURCE_PACKS_DESCRIPTION),
        control: Control::Switch { on: false },
    },
    Block::Row {
        label: SYNC_DATA_PACKS_LABEL,
        description: Some(SYNC_DATA_PACKS_DESCRIPTION),
        control: Control::Switch { on: false },
    },
    Block::Row {
        label: SYNC_COMMAND_HISTORY_LABEL,
        description: Some(SYNC_COMMAND_HISTORY_DESCRIPTION),
        control: Control::Switch { on: false },
    },
    Block::Row {
        label: SYNC_HOTBARS_LABEL,
        description: Some(SYNC_HOTBARS_DESCRIPTION),
        control: Control::Switch { on: false },
    },
];

/// The Resources tab: `content-storage.vue`'s section (`:261-419`) first, then the app
/// directory (`index.vue:82-107`), the download limit (`:109-123`) and the write limit
/// (`:125-139`), which is the order `index.vue:80-140` declares.
///
/// `// owed:` the storage section's usage bar and its `Unique` / `Shared` / `Orphaned`
/// breakdown (`:283-321`) are measurements rather than rows; the cache limit is a number
/// field with the `GiB` unit trailing it (`:402-416`, starting at 5 GiB per `:60`), drawn here
/// as a plate showing the unit; `Verify and repair` and `Clear unused` carry tooltips
/// (`:324-355`) a button here has no room for; the app directory is an `Input` with a
/// folder-picker `IconButton` (`:86-103`), drawn as the picker's own label with the field
/// itself owed; and both limits are `Slider`s (`:113-119`, `:129-135`), drawn as a plate at
/// each slider's floor.
const RESOURCES_BLOCKS: &[Block] = &[
    Block::Heading {
        text: STORE_TITLE,
        description: Some(STORE_DESCRIPTION),
        big: false,
        control: None,
    },
    Block::Standalone(Control::Button {
        label: STORE_REPAIR_LABEL,
        glyph: Some(Glyph::ShieldCheck),
    }),
    Block::Standalone(Control::Button {
        label: STORE_CLEAR_LABEL,
        glyph: Some(Glyph::Trash),
    }),
    Block::Row {
        label: STORE_CACHE_LIMIT_LABEL,
        description: Some(STORE_CACHE_LIMIT_DESCRIPTION),
        control: Control::Select {
            value: STORE_CACHE_LIMIT_UNIT,
        },
    },
    Block::Rule,
    Block::Row {
        label: APP_DIRECTORY_LABEL,
        description: Some(APP_DIRECTORY_DESCRIPTION),
        control: Control::Button {
            label: APP_DIRECTORY_BROWSE,
            glyph: Some(Glyph::FolderSearch),
        },
    },
    Block::Heading {
        text: MAX_DOWNLOADS_LABEL,
        description: Some(MAX_DOWNLOADS_DESCRIPTION),
        big: false,
        control: None,
    },
    Block::Standalone(Control::Select {
        value: MAX_DOWNLOADS_VALUE,
    }),
    Block::Heading {
        text: MAX_WRITES_LABEL,
        description: Some(MAX_WRITES_DESCRIPTION),
        big: false,
        control: None,
    },
    Block::Standalone(Control::Select {
        value: MAX_WRITES_VALUE,
    }),
];

/// The Troubleshooting tab (`troubleshooting-settings.vue:221-330`), in its order: app cache
/// (`:233-243`), app database backups (`:244-255`), debug info (`:256-278`), report an ad
/// (`:279-311`) and the always-show-copy-details row (`:312-328`).
///
/// `// owed:` `Clear cache` asks for confirmation first unless non-essential warnings are
/// skipped (`:166-173`, `:224-232`), and `Copy ad details` turns into `Copied`
/// (`app.settings.troubleshooting.report-ad.copied`, :3167) for three seconds (`:209-212`);
/// neither state is drawn, and a button here has no tooltip for the ad-details flow. The
/// report-an-ad description's `<support>` span is a link to support.modrinth.com
/// (`:299-309`) and is drawn as its literal markup.
const TROUBLESHOOTING_BLOCKS: &[Block] = &[
    Block::Heading {
        text: APP_CACHE_TITLE,
        description: Some(APP_CACHE_DESCRIPTION),
        big: false,
        control: None,
    },
    Block::Standalone(Control::Button {
        label: CLEAR_CACHE_LABEL,
        glyph: Some(Glyph::Trash),
    }),
    Block::Heading {
        text: BACKUPS_TITLE,
        description: Some(BACKUPS_DESCRIPTION),
        big: false,
        control: None,
    },
    Block::Standalone(Control::Button {
        label: BACKUPS_OPEN_LABEL,
        glyph: Some(Glyph::FolderOpen),
    }),
    Block::Heading {
        text: DEBUG_INFO_TITLE,
        description: Some(DEBUG_INFO_DESCRIPTION),
        big: false,
        control: None,
    },
    Block::Standalone(Control::Button {
        label: DEBUG_INFO_EXPORT_LABEL,
        glyph: Some(Glyph::Download),
    }),
    Block::Heading {
        text: REPORT_AD_TITLE,
        description: Some(REPORT_AD_DESCRIPTION),
        big: false,
        control: None,
    },
    Block::Standalone(Control::Button {
        label: REPORT_AD_COPY_LABEL,
        glyph: Some(Glyph::Copy),
    }),
    Block::Row {
        label: COPY_DETAILS_LABEL,
        description: Some(COPY_DETAILS_DESCRIPTION),
        // `use-app-settings.ts:37` starts `alwaysShowCopyDetails` off.
        control: Control::Switch { on: false },
    },
];

/// The blocks for one tab, in the source's order. Every tab carries its rows now; the
/// `// owed:` note above each table names the rows and controls this pass still cannot draw.
fn blocks(tab: Tab) -> &'static [Block] {
    match tab {
        Tab::Appearance => APPEARANCE_BLOCKS,
        Tab::Features => FEATURES_BLOCKS,
        Tab::Behavior => BEHAVIOR_BLOCKS,
        Tab::Language => LANGUAGE_BLOCKS,
        Tab::FeatureFlags => FEATURE_FLAG_BLOCKS,
        Tab::Profile => PROFILE_BLOCKS,
        Tab::Social => SOCIAL_BLOCKS,
        Tab::Privacy => PRIVACY_BLOCKS,
        Tab::JavaAndMemory => JAVA_AND_MEMORY_BLOCKS,
        Tab::Window => WINDOW_BLOCKS,
        Tab::LaunchHooks => LAUNCH_HOOKS_BLOCKS,
        Tab::Syncing => SYNCING_BLOCKS,
        Tab::Resources => RESOURCES_BLOCKS,
        Tab::Troubleshooting => TROUBLESHOOTING_BLOCKS,
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
        let body_height = draw_body(painter, dialog, theme, state.tab, state.scroll);
    state.last_body_height = body_height;

    // The wheel, over the content pane, moves the body. Clamped both ways: above zero is the
    // top of the tab, and the bottom is the last row sitting at the pane's lower edge.
    let content = content_rect(dialog);
    let pane = content.height() - 2.0 * CONTENT_PADDING;
    let limit = (body_height - pane).max(0.0);
    let wheel = ui.ctx().input(|i| i.smooth_scroll_delta.y);
    if wheel != 0.0 && ui.rect_contains_pointer(content) {
        state.scroll = (state.scroll - wheel).clamp(0.0, limit);
    }
    state.scroll = state.scroll.min(limit);
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

/// The height the rail wants at the reference's own pitch.
///
/// Used only to decide whether it has to be compressed: the rows are the source's 40-point
/// rows and 4-point gaps, the headings its category line, and the gaps between categories its
/// `mt-3`.
fn rail_needed_height() -> f32 {
    let mut height = 0.0;
    let mut previous: Option<Tab> = None;
    for (index, tab) in Tab::ALL.into_iter().enumerate() {
        let starts_category = previous.map_or(true, |last| last.category() != tab.category());
        if starts_category {
            if index > 0 {
                height += CATEGORY_TOP_GAP;
            }
            height += CATEGORY_LINE + CATEGORY_PADDING;
        }
        height += RAIL_ROW_HEIGHT + RAIL_ROW_GAP;
        previous = Some(tab);
    }
    height
}

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

    // The rail has fourteen rows in four categories, which does not fit the pane at 900
    // points of window: the last three tabs used to be laid out past the dialog's edge and
    // skipped as targets, so Syncing, Resources and Troubleshooting could not be reached at
    // all. The rail is compressed to fit instead -- the reference's own size when it fits, a
    // proportionally tighter pitch when it does not -- because a settings rail that hides
    // tabs is a settings rail with unreachable settings.
    let available = rail.height() - 2.0 * RAIL_PADDING;
    let needed = rail_needed_height();
    let scale = if needed > available {
        (available / needed).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let row_height = RAIL_ROW_HEIGHT * scale;
    let row_gap = RAIL_ROW_GAP * scale;
    let category_gap = CATEGORY_TOP_GAP * scale;
    let category_line = (CATEGORY_LINE + CATEGORY_PADDING) * scale;

    let mut y = rail.top() + RAIL_PADDING;
    let mut previous: Option<Tab> = None;
    for (index, tab) in Tab::ALL.into_iter().enumerate() {
        let starts_category = previous.map_or(true, |last| last.category() != tab.category());
        let heading = if starts_category {
            // `mt-3` above a category that is not the first child (`TabbedModal.vue:180`).
            if index > 0 {
                y += category_gap;
            }
            let rect = Rect::from_min_size(
                pos2(rail.left() + RAIL_PADDING, y),
                vec2(width, category_line),
            );
            y = rect.bottom();
            Some(rect)
        } else {
            None
        };

        let row = Rect::from_min_size(
            pos2(rail.left() + RAIL_PADDING, y),
            vec2(width, row_height),
        );
        rows[index] = RailRow { tab, heading, row };
        y = row.bottom() + row_gap;
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
/// Draw the tab's body, scrolled by `scroll`, and return its total height.
///
/// The height is returned so the caller can clamp the offset against the pane: a scroll bar
/// that let a reader go past the last row by a screenful is as wrong as no scrolling at all.
fn draw_body(
    painter: &egui::Painter,
    dialog: Rect,
    theme: &theme::Theme,
    tab: Tab,
    scroll: f32,
) -> f32 {
    let content = content_rect(dialog);
    painter.rect_filled(content, PANE_RADIUS, theme.SURFACE_2);
    let body = painter.with_clip_rect(content);

    let left = content.left() + CONTENT_PADDING;
    let right = content.right() - CONTENT_PADDING;
    let mut y = content.top() + CONTENT_PADDING - scroll;

    let blocks = blocks(tab);
    if blocks.is_empty() {
        // No tab takes this path now -- every table in `blocks` holds at least one block --
        // but the body still falls back to the tab's own heading if one ever empties.
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
        return y + scroll - content.top();
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

    // The height the caller needs: how far below the pane's top the last row fell, with the
    // scroll offset added back so the number does not move as the reader scrolls.
    y + scroll - content.top()
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

    /// The rail keeps the reference's pitch when it fits and tightens it when it does not, and
    /// either way every tab is inside the rail and on one pitch within its category.
    ///
    /// The absolute numbers the earlier version of this test asserted were the *uncompressed*
    /// layout — which is exactly the layout that put three tabs past the dialog's edge and made
    /// them unselectable. What has to hold is the relationship, not the constant.
    #[test]
    fn the_rail_fits_its_pane_and_keeps_one_pitch_per_category() {
        for screen_size in [vec2(1440.0, 900.0), vec2(1600.0, 1200.0), vec2(1280.0, 800.0)] {
            let dialog = dialog_rect(Rect::from_min_size(pos2(0.0, 0.0), screen_size));
            let rail = rail_rect(dialog);
            let rows = rail_rows(dialog);
            assert_eq!(rows.len(), TAB_COUNT);

            // Every row starts and ends inside the rail: a row outside it is drawn past the
            // dialog and skipped as a target, which is the bug this layout was changed for.
            for row in rows {
                assert!(
                    row.row.top() >= rail.top() - 0.001
                        && row.row.bottom() <= rail.bottom() + 0.001,
                    "{} falls outside the rail at {screen_size:?}",
                    row.tab.label()
                );
            }

            // One positive pitch per step, and a heading wherever the category changes.
            let mut crossings = 0;
            for pair in rows.windows(2) {
                let pitch = pair[1].row.top() - pair[0].row.top();
                assert!(pitch > 0.0, "rows advance downward");
                if pair[0].tab.category() == pair[1].tab.category() {
                    assert!(pair[1].heading.is_none(), "a category is headed once");
                } else {
                    crossings += 1;
                    assert!(pair[1].heading.is_some(), "a new category is headed");
                }
            }
            assert_eq!(crossings, 3, "Display, Account, Instance defaults, System");
        }
    }

    /// The rail is compressed only as far as it must be, and never below a readable row.
    ///
    /// The earlier version of this test asserted that a tall window keeps the reference's
    /// 40-point rows — which cannot happen: the dialog's body is capped at `DIALOG_MAX_HEIGHT`
    /// (640) and fourteen rows at the source's pitch need about 730, so the rail is always a
    /// little tighter than the reference. What matters is that the tightening is small and
    /// bounded, so a row is still a target a reader can hit.
    #[test]
    fn the_rail_is_compressed_only_as_far_as_it_must_be() {
        let dialog = dialog_rect(screen());
        let available = rail_rect(dialog).height() - 2.0 * RAIL_PADDING;
        let needed = rail_needed_height();
        assert!(
            needed > available,
            "fourteen rows at the source pitch are taller than the pane: {needed} > {available}"
        );
        let scale = available / needed;
        assert!(scale > 0.75, "the rail is not squeezed beyond use: {scale}");

        let rows = rail_rows(dialog);
        for row in rows {
            assert!(
                row.row.height() >= 30.0,
                "{} is {} points tall, below a comfortable target",
                row.tab.label(),
                row.row.height()
            );
        }
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

    /// Every tab carries a body. Before this pass eleven tabs stopped at their heading; a tab
    /// that regresses to that shows up here.
    #[test]
    fn every_tab_has_a_body() {
        for tab in Tab::ALL {
            assert!(!blocks(tab).is_empty(), "{} has no blocks", tab.label());
        }
    }

    /// The block count each table holds, taken from the `.vue` files those tables cite. A
    /// block is one heading, row, standalone control or rule, so these are not the "settings
    /// rows" a tab holds -- a tab with three rules counts three more. A ported row that goes
    /// missing moves its tab's number and fails here.
    #[test]
    fn every_tab_carries_the_blocks_this_pass_ported() {
        let ported: [(Tab, usize); TAB_COUNT] = [
            (Tab::Appearance, 6),
            (Tab::Features, 13),
            (Tab::Behavior, 14),
            (Tab::Language, 3),
            (Tab::FeatureFlags, 11),
            (Tab::Profile, 5),
            (Tab::Social, 4),
            (Tab::Privacy, 4),
            (Tab::JavaAndMemory, 12),
            (Tab::Window, 3),
            (Tab::LaunchHooks, 7),
            (Tab::Syncing, 6),
            (Tab::Resources, 10),
            (Tab::Troubleshooting, 9),
        ];

        for (tab, count) in ported {
            assert_eq!(blocks(tab).len(), count, "{} blocks", tab.label());
        }
        // Every tab is in the list above, and every entry is a tab in the rail.
        for tab in Tab::ALL {
            assert!(ported.iter().any(|(listed, _)| *listed == tab), "{} is listed", tab.label());
        }
        assert_eq!(ported.len(), TAB_COUNT);
    }

    /// No ported string is empty: a heading says something, a description that is there says
    /// something, and every select and button carries its value or label.
    #[test]
    fn every_ported_string_says_something() {
        for tab in Tab::ALL {
            for block in blocks(tab) {
                match *block {
                    Block::Heading {
                        text,
                        description,
                        control,
                        ..
                    } => {
                        assert!(!text.is_empty(), "{} has an empty heading", tab.label());
                        if let Some(description) = description {
                            assert!(
                                !description.is_empty(),
                                "{} has an empty heading description",
                                tab.label()
                            );
                        }
                        if let Some(control) = control {
                            assert_control_is_lettered(tab, control);
                        }
                    }
                    Block::Row {
                        label,
                        description,
                        control,
                    } => {
                        assert!(!label.is_empty(), "{} has an empty row label", tab.label());
                        if let Some(description) = description {
                            assert!(
                                !description.is_empty(),
                                "{} has an empty row description",
                                tab.label()
                            );
                        }
                        assert_control_is_lettered(tab, control);
                    }
                    Block::Standalone(control) => assert_control_is_lettered(tab, control),
                    Block::Rule => {}
                }
            }
        }
    }

    /// A select shows its value and a button its label; a switch carries no text of its own.
    fn assert_control_is_lettered(tab: Tab, control: Control) {
        let text = match control {
            Control::Switch { .. } => None,
            Control::Select { value } => Some(value),
            Control::Button { label, .. } => Some(label),
        };
        if let Some(text) = text {
            assert!(!text.is_empty(), "{} has an unlettered control", tab.label());
        }
    }
}
