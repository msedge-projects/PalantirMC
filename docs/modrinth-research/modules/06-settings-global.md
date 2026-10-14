# 06 — Global settings (Modrinth App v0.21.9)

Source: `modrinth/code` @ `6ebd75b68dd386cd4d74eee1cf2e96d945c937db` (tag `v0.21.9`).
All label text below is the **authoritative English string**, read out of the locale
JSON (`apps/app-frontend/src/locales/en-US/index.json` for app strings,
`packages/ui/src/locales/en-US/index.json` for shared strings). Where a label is
only in the Vue `defineMessages` block I cite that file too.

## Second observation pass (2026-10-14)

Two things this pass adds:
- **Per-instance overrides are now observed, not just sourced.** Module 05's second
  pass captured the instance Settings → Sync overrides page `[S-0107]`: five
  independent `Unsync …` toggles (game settings / multiplayer servers / resource
  packs / command history / saved creative hotbars), **all default off**, each with
  its own explanation, plus `Manage synced settings`. That is the concrete answer to
  "which global settings can an instance opt out of", and it lives in the instance
  modal, not here.
- **The connectivity question this module's Privacy/Behavior sections touch is
  resolved at the environment level.** The "Offline" indicator is not a setting and
  not a user-visible state — it is the `navigator.onLine` bug; see OQ-004 and
  OQ-012. No settings row controls it.

**Headline facts, up front, because they change everything downstream:**

1. **There is no settings JSON file.** At v0.21.9 settings live in a **SQLite
   table `settings` (single row, `id = 0`)** inside the app database
   [SOURCE packages/app-lib/migrations/20240711194701_init.sql:1],
   [SOURCE packages/app-lib/src/state/settings.rs:115]. The Rust struct is
   `Settings` [SOURCE packages/app-lib/src/state/settings.rs:10] and it is
   read/written whole-object through two Tauri commands,
   `settings_get` / `settings_set`
   [SOURCE apps/app/src/api/settings.rs:59],
   [SOURCE apps/app/src/api/settings.rs:67],
   [SOURCE packages/app-lib/src/api/settings.rs:64],
   [SOURCE packages/app-lib/src/api/settings.rs:81].
   The frontend never patches a field: it fetches the whole object, spreads the
   change over it, and writes the whole object back
   [SOURCE apps/app-frontend/src/helpers/settings.ts:130],
   [SOURCE apps/app-frontend/src/helpers/settings.ts:135]. Two tabs doing this at
   once can clobber each other; they defend with a `scope: { id: 'app-settings' }`
   mutation scope [SOURCE apps/app-frontend/src/components/ui/settings/display/BehaviorSettings.vue:165]
   and a re-read-before-write [SOURCE apps/app-frontend/src/components/ui/settings/display/BehaviorSettings.vue:182].

2. **There is no "Profiles" concept in the settings UI.** The word "profile" was
   renamed to **instance** across the product (the old `profiles` table was
   migrated to `instances` and dropped
   [SOURCE packages/app-lib/migrations/20260611120000_instances-content-foundation.sql:1],
   [SOURCE packages/app-lib/migrations/20260619120000_finish-instance-profile-migration.sql]).
   The word survives only in account settings, where "Profile" is the
   *Modrinth account* profile (username/bio/avatar) [SOURCE packages/ui/src/layouts/wrapped/settings/AccountProfileSettings.vue:49].

3. **There are four distinct settings scopes**, not one. See
   [Scopes](#scopes) below. This is the single most important structural fact
   for a reimplementation.

4. **There is no update-channel setting, no bandwidth limit, no proxy setting,
   no font-size/density setting, no accent-colour setting, no game-window-title
   setting and no close-to-tray setting anywhere in v0.21.9.** Each is proven
   absent below rather than merely omitted.

---

## Scopes

| Scope | Where it lives | Synced? | Reset story |
| --- | --- | --- | --- |
| **Global app** | SQLite `settings` table, one row | no (device-local) | some tabs discard unsaved edits back to last-saved; some tabs write immediately and have no reset at all |
| **Instance override** | `instance_launch_overrides.overrides` JSONB per instance [SOURCE packages/app-lib/migrations/20260611120000_instances-content-foundation.sql:72] | no | per-instance modal |
| **Synced options** | `sync_feature_settings` + `synced_*` tables [SOURCE packages/app-lib/migrations/20260819120000_instance-synced-options.sql:3] | no, but shared across instances | toggle per feature, choose a "source instance" on enable |
| **Account preferences** | Modrinth API, `UserPreferences` v3 [SOURCE packages/ui/src/providers/user-preferences.ts:14] | **yes**, across devices | discard unsaved edits |

The three "…across devices" toggles (theme / behavior / features) gate the third
row: when on, a change is `PATCH`ed to the account and then pulled back down into
the local DB at startup [SOURCE apps/app-frontend/src/App.vue:1235].

---

## Coverage table

`ID | label | section | default | persistence key | evidence | status`

| ID | Label | Section | Default | Persistence key | Evidence | Status |
| --- | --- | --- | --- | --- | --- | --- |
| SET-1 | Color theme | Display ▸ Appearance | `dark` | `settings.theme` (TEXT `'dark'`) | SOURCE packages/app-lib/migrations/20240711194701_init.sql:7 | documented |
| SET-2 | Sync theme across devices | Display ▸ Appearance | `true` | `settings.sync_theme_across_devices` | SOURCE packages/app-lib/migrations/20260818120000_sync-preferences-settings.sql:1 | documented |
| SET-3 | Advanced rendering | Display ▸ Appearance | `true` | `settings.advanced_rendering` | SOURCE packages/app-lib/migrations/20240711194701_init.sql:10 | documented |
| SET-4 | System window frame | Display ▸ Appearance | `false` | `settings.native_decorations` | SOURCE packages/app-lib/migrations/20240711194701_init.sql:11 | documented |
| SET-5 | Sync features across devices | Display ▸ Features | `true` | `settings.sync_features_across_devices` | SOURCE packages/app-lib/migrations/20260902140000_sync-features-setting.sql:1 | documented |
| SET-6 | Show Worlds tab in instances | Display ▸ Features | `true` | `settings.show_worlds_tab_in_instances` | SOURCE packages/app-lib/migrations/20260902130000_promote-instance-navigation-settings.sql:5 | documented |
| SET-7 | Show Files tab in instances | Display ▸ Features | `true` | `settings.show_files_tab_in_instances` | SOURCE packages/app-lib/migrations/20260902130000_promote-instance-navigation-settings.sql:2 | documented |
| SET-8 | Show Screenshots tab in instances | Display ▸ Features | `false` | `settings.show_screenshots_tab_in_instances` | SOURCE packages/app-lib/migrations/20260902130000_promote-instance-navigation-settings.sql:8 | documented |
| SET-9 | Show all screenshots in sidebar | Display ▸ Features | `true` (fresh install) | `sync_feature_settings.screenshots` — **not** the `settings` table | SOURCE packages/app-lib/migrations/20260819120000_instance-synced-options.sql:29 | documented |
| SET-10 | Show skin selector in sidebar | Display ▸ Features | `true` | `settings.show_skin_selector_in_sidebar` | SOURCE packages/app-lib/migrations/20260902130000_promote-instance-navigation-settings.sql:11 | documented |
| SET-11 | Quick instances in sidebar | Display ▸ Features | unset ⇒ falls back to max (20) | `localStorage['modrinth-quick-instance-count']` | SOURCE apps/app-frontend/src/composables/use-quick-instance-limit.ts:3 | documented |
| SET-12 | Show Jump in section | Display ▸ Features | `true` | `settings.show_jump_in` | SOURCE packages/app-lib/migrations/20260918120000_promote-behavior-settings.sql:17 | documented |
| SET-13 | Sync behavior across devices | Display ▸ Behavior | `true` | `settings.sync_behavior_across_devices` | SOURCE packages/app-lib/migrations/20260818120000_sync-preferences-settings.sql:4 | documented |
| SET-14 | Minimize app | Display ▸ Behavior | `false` | `settings.hide_on_process_start` | SOURCE packages/app-lib/migrations/20240711194701_init.sql:28 | documented |
| SET-15 | Refocus app when Minecraft closes | Display ▸ Behavior | `false` | `settings.refocus_on_game_close` | SOURCE packages/app-lib/migrations/20260918120000_promote-behavior-settings.sql:1 | documented |
| SET-16 | Hide right sidebar | Display ▸ Behavior | `false` | `settings.toggle_sidebar` | SOURCE packages/app-lib/migrations/20241222013857_feature-flags.sql:1 | documented |
| SET-17 | Compact mode | Display ▸ Behavior | `false` | `settings.compact_instance_cards` | SOURCE packages/app-lib/migrations/20260918120000_promote-behavior-settings.sql:3 | documented |
| SET-18 | Show play time | Display ▸ Behavior | `true` | `settings.show_play_time` | SOURCE packages/app-lib/migrations/20260918120000_promote-behavior-settings.sql:8 | documented |
| SET-19 | Hide nametag | Display ▸ Behavior | `false` | `settings.hide_nametag_skins_page` | SOURCE packages/app-lib/migrations/20250514181748_skin_nametag_setting.sql:1 | documented |
| SET-20 | Warn me before installing unknown modpacks | Display ▸ Behavior | `true` | `settings.warn_on_unknown_modpacks` | SOURCE packages/app-lib/migrations/20260918120000_promote-behavior-settings.sql:11 | documented |
| SET-21 | Skip non-essential warnings | Display ▸ Behavior | `false` | `settings.skip_non_essential_warnings` | SOURCE packages/app-lib/migrations/20260918120000_promote-behavior-settings.sql:14 | documented |
| SET-22 | Language | Display ▸ Language | `en-US` | `settings.locale` | SOURCE packages/app-lib/migrations/20251229145719_add_locale_setting.sql:2 | documented |
| SET-23 | Feature flags (10 rows) | Display ▸ Feature flags (dev only) | per-flag, table below | `settings.feature_flags` JSONB | SOURCE apps/app-frontend/src/composables/use-app-settings.ts:3 | documented |
| SET-24 | Profile picture | Account ▸ Profile | none | account API `avatar_url` | SOURCE packages/ui/src/layouts/wrapped/settings/AccountProfileSettings.vue:327 | documented |
| SET-25 | Username | Account ▸ Profile | current | account API `username` | SOURCE packages/ui/src/layouts/wrapped/settings/AccountProfileSettings.vue:331 | documented |
| SET-26 | Bio | Account ▸ Profile | current | account API `bio` | SOURCE packages/ui/src/layouts/wrapped/settings/AccountProfileSettings.vue:336 | documented |
| SET-27 | Friend requests | Account ▸ Social | `everyone` | account preferences `social.friend_privacy` | SOURCE packages/ui/src/layouts/wrapped/settings/AccountSocialSettings.vue:270 | documented |
| SET-28 | Shared instance invites | Account ▸ Social | `everyone` | account preferences `social.shared_instances_privacy` | SOURCE packages/ui/src/layouts/wrapped/settings/AccountSocialSettings.vue:271 | documented |
| SET-29 | Hosting access invites | Account ▸ Social | `everyone` | account preferences `social.hosting_access_privacy` | SOURCE packages/ui/src/layouts/wrapped/settings/AccountSocialSettings.vue:272 | documented |
| SET-30 | Blocked users | Account ▸ Social | empty | account API `blocked_users` v3 | SOURCE packages/ui/src/layouts/wrapped/settings/AccountSocialSettings.vue:500 | documented |
| SET-31 | Manage preferences (ads consent) | Account ▸ Privacy | n/a | third-party consent store | SOURCE apps/app-frontend/src/components/ui/settings/account/PrivacySettings.vue:32 | documented |
| SET-32 | Telemetry | Account ▸ Privacy | `true` | `settings.telemetry` | SOURCE packages/app-lib/migrations/20240711194701_init.sql:13 | documented |
| SET-33 | Discord Rich Presence | Account ▸ Privacy | `true` | `settings.discord_rpc` | SOURCE packages/app-lib/migrations/20240711194701_init.sql:14 | documented |
| SET-34 | Sync game options | Instances ▸ Synced settings | `true` fresh / `false` upgraded | `sync_feature_settings.game_options` | SOURCE packages/app-lib/migrations/20260902120000_enable-synced-options-for-fresh-installs.sql:1 | documented |
| SET-35 | Sync multiplayer servers | Instances ▸ Synced settings | `true` fresh / `false` upgraded | `sync_feature_settings.multiplayer_servers` | SOURCE packages/app-lib/migrations/20260902120000_enable-synced-options-for-fresh-installs.sql:2 | documented |
| SET-36 | Sync resource packs | Instances ▸ Synced settings | `false` | `sync_feature_settings.resource_packs` | SOURCE packages/app-lib/migrations/20260903120000_synced-packs.sql:1 | documented |
| SET-37 | Sync command history | Instances ▸ Synced settings | `true` fresh / `false` upgraded | `sync_feature_settings.command_history` | SOURCE packages/app-lib/migrations/20260902120000_enable-synced-options-for-fresh-installs.sql:3 | documented |
| SET-38 | Sync saved creative hotbars | Instances ▸ Synced settings | `true` fresh / `false` upgraded | `sync_feature_settings.creative_hotbars` | SOURCE packages/app-lib/migrations/20260902120000_enable-synced-options-for-fresh-installs.sql:4 | documented |
| SET-39 | *(row exists but is hidden)* Sync data packs | Instances ▸ Synced settings | n/a — **not rendered** | `sync_feature_settings.data_packs` | SOURCE apps/app-frontend/src/helpers/instance.ts:304 | documented |
| SET-40 | Edit game settings | Instances ▸ Synced settings | n/a (button) | `synced_game_option_preferences` + `synced_game_option_values` | SOURCE packages/app-lib/migrations/20260827120000_game-options-sync.sql:10 | documented |
| SET-41 | Fullscreen | Instances ▸ Synced settings ▸ Window | `false` | `settings.mc_force_fullscreen` | SOURCE packages/app-lib/migrations/20240711194701_init.sql:24 | documented |
| SET-42 | Width | Instances ▸ Synced settings ▸ Window | `854` | `settings.mc_game_resolution_x` | SOURCE packages/app-lib/migrations/20240711194701_init.sql:25 | documented |
| SET-43 | Height | Instances ▸ Synced settings ▸ Window | `480` | `settings.mc_game_resolution_y` | SOURCE packages/app-lib/migrations/20240711194701_init.sql:26 | documented |
| SET-44 | Memory allocation | Instances ▸ Synced settings ▸ Java and memory | 2048 MB, rewritten at first launch to 2/4/6 GB by RAM | `settings.mc_memory_max` | SOURCE packages/app-lib/src/api/jre.rs:401 | documented |
| SET-45 | Java arguments | Instances ▸ Synced settings ▸ Java and memory | `[]` | `settings.extra_launch_args` JSONB | SOURCE packages/app-lib/migrations/20240711194701_init.sql:20 | documented |
| SET-46 | Environment variables | Instances ▸ Synced settings ▸ Java and memory | `[]` | `settings.custom_env_vars` JSONB | SOURCE packages/app-lib/migrations/20240711194701_init.sql:22 | documented |
| SET-47 | Pre-launch hook | Instances ▸ Synced settings ▸ Launch hooks | `null` | `settings.hook_pre_launch` | SOURCE packages/app-lib/migrations/20240711194701_init.sql:30 | documented |
| SET-48 | Wrapper hook | Instances ▸ Synced settings ▸ Launch hooks | `null` | `settings.hook_wrapper` | SOURCE packages/app-lib/migrations/20240711194701_init.sql:31 | documented |
| SET-49 | Post-exit hook | Instances ▸ Synced settings ▸ Launch hooks | `null` | `settings.hook_post_exit` | SOURCE packages/app-lib/migrations/20240711194701_init.sql:32 | documented |
| SET-50 | Java 25 location | Instances ▸ Java installations | unset | `java_versions.path` where `major_version = 25` | SOURCE apps/app-frontend/src/components/ui/settings/instances/JavaSettings.vue:34 | documented |
| SET-51 | Java 21 location | Instances ▸ Java installations | unset | `java_versions.path` where `major_version = 21` | SOURCE apps/app-frontend/src/components/ui/settings/instances/JavaSettings.vue:34 | documented |
| SET-52 | Java 17 location | Instances ▸ Java installations | unset | `java_versions.path` where `major_version = 17` | SOURCE apps/app-frontend/src/components/ui/settings/instances/JavaSettings.vue:34 | documented |
| SET-53 | Java 8 location | Instances ▸ Java installations | unset | `java_versions.path` where `major_version = 8` | SOURCE apps/app-frontend/src/components/ui/settings/instances/JavaSettings.vue:34 | documented |
| SET-54 | Content storage (usage readout) | Instances ▸ Resource management | n/a (info) | derived from content-store tables | SOURCE apps/app-frontend/src/components/ui/settings/instances/ContentStorageSettings.vue:26 | documented |
| SET-55 | Keep unused downloads | Instances ▸ Resource management | `5` GiB | `app_metadata['store_cache_limit_bytes']` | SOURCE packages/app-lib/src/state/content_store/commands/cache.rs:141 | documented |
| SET-56 | Clear unused | Instances ▸ Resource management | n/a (button) | n/a | SOURCE apps/app-frontend/src/components/ui/settings/instances/ContentStorageSettings.vue:125 | documented |
| SET-57 | Verify and repair | Instances ▸ Resource management | n/a (button) | n/a | SOURCE apps/app-frontend/src/components/ui/settings/instances/ContentStorageSettings.vue:133 | documented |
| SET-58 | App directory | Instances ▸ Resource management | OS app-data dir | `settings.custom_dir` | SOURCE packages/app-lib/migrations/20240711194701_init.sql:34 | documented |
| SET-59 | Always show copy details | Instances ▸ Resource management | `false` | `settings.always_show_copy_details` | SOURCE packages/app-lib/migrations/20260918120000_promote-behavior-settings.sql:20 | documented |
| SET-60 | Purge cache | Instances ▸ Resource management | n/a (button) | n/a | SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:75 | documented |
| SET-61 | Maximum concurrent downloads | Instances ▸ Resource management | `10` | `settings.max_concurrent_downloads` | SOURCE packages/app-lib/migrations/20240711194701_init.sql:4 | documented |
| SET-62 | Maximum concurrent writes | Instances ▸ Resource management | `10` | `settings.max_concurrent_writes` | SOURCE packages/app-lib/migrations/20240711194701_init.sql:5 | documented |
| SET-63 | App database backups → Open backups folder | Instances ▸ Resource management | n/a (button) | n/a | SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:119 | documented |
| SET-64 | Debug info → Export debug info | Instances ▸ Resource management | n/a (button) | n/a | SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:40 | documented |
| SET-65 | Modrinth App {version} / OS | Modal footer | n/a (info) | n/a | SOURCE apps/app-frontend/src/components/ui/modal/AppSettingsModal.vue:313 | documented |
| SET-66 | Downloading v{version} + progress bar | Modal footer | n/a (info) | n/a | SOURCE apps/app-frontend/src/components/ui/modal/AppSettingsModal.vue:309 | `[ORCH-PENDING]` |
| SET-67 | Toggle developer mode | Modal footer | `false` | `settings.developer_mode` | SOURCE apps/app-frontend/src/components/ui/modal/AppSettingsModal.vue:321 | documented |

---

## Structure of the settings surface

**Entry point.** A gear button at the bottom of the icon-only left rail, above
the account avatar [SOURCE apps/app-frontend/src/App.vue:2322]. It opens a
**modal dialog**, not a route — there is no `/settings` path anywhere in
`apps/app-frontend/src/pages/` and no entry in the router table. This matches the
orchestrator's own sweep (RUNLOG 2026-10-14T12:58Z).

**Shape.** A `TabbedModal` with a left tab rail, width `min(928px, calc(95vw - 10rem))`
[SOURCE apps/app-frontend/src/components/ui/modal/AppSettingsModal.vue:331].
Eleven tabs in three groups [SOURCE apps/app-frontend/src/components/ui/modal/AppSettingsModal.vue:80]:

| Group | Tab | Label id |
| --- | --- | --- |
| Display | Appearance | `app.settings.tabs.appearance` → "Appearance" |
| Display | Features | `app.settings.tabs.features` → "Features" |
| Display | Behavior | `app.settings.tabs.behavior` → "Behavior" |
| Display | Language | `app.settings.tabs.language` → "Language", badged **Beta** [SOURCE apps/app-frontend/src/components/ui/modal/AppSettingsModal.vue:116] |
| Display | Feature flags | `settings.feature-flags.title` → "Feature flags", **developer-only** [SOURCE apps/app-frontend/src/components/ui/modal/AppSettingsModal.vue:123] |
| Account | Profile | `settings.profile.navigation-title` → "Profile" |
| Account | Social | `settings.social.title` → "Social" |
| Account | Privacy | `app.settings.tabs.privacy` → "Privacy" |
| Instances | Synced settings | `app.settings.tabs.synced-options` → "Synced settings" |
| Instances | Java installations | `app.settings.tabs.java-installations` → "Java installations" |
| Instances | Resource management | `app.settings.tabs.resource-management` → "Resource management" |

Group headers are `settings.sidebar.label.display` → "Display",
`settings.sidebar.label.account` → "Account",
`app.settings.sidebar.label.instances` → "Instances"
[SOURCE apps/app-frontend/src/components/ui/modal/AppSettingsModal.vue:65].

The Feature flags tab is filtered out entirely when developer mode is off
[SOURCE apps/app-frontend/src/components/ui/modal/AppSettingsModal.vue:175]. It is
also reachable from the account overflow menu in the rail, but only shown when
dev mode is on [SOURCE apps/app-frontend/src/App.vue:1567].

**Two save models, and the difference is inconsistent.** Appearance, Features,
Behavior, Language, Profile, Social and the Game-settings editor use
`useSavable` + an "You have unsaved changes" floating action bar with **Reset**
and **Save** [SOURCE apps/app-frontend/src/components/ui/modal/AppSettingsModal.vue:341],
[SOURCE packages/ui/src/utils/savable.ts:5]. Privacy, Feature flags, Resource
management, Java installations and the Synced-settings toggles write on change
with no bar at all. That means in the same modal, one tab's typo survives for
minutes and another commits the instant you click it.

**What "Reset" actually does — verified, not guessed.** `useSavable.reset()`
assigns `cloneDeep(data())`, i.e. it re-reads the *persisted* state and throws
away unsaved edits [SOURCE packages/ui/src/utils/savable.ts:44]. **It never
restores a factory default.** A setting you moved away from its original value
stays where you put it. Tabs with no floating action bar have no reset at all.
This is the single most important UX bug class to avoid copying.

---

## SET-1 — Color theme
**Type:** select (visual swatch grid, radio semantics)
**Section:** Display ▸ Appearance, first section, heading "Color theme"
[SOURCE packages/ui/src/layouts/shared/appearance-settings/messages.ts:4]
**Nav path:** left rail gear → Appearance
**Evidence:** [SOURCE packages/ui/src/layouts/shared/appearance-settings/components/appearance-settings-theme-selector.vue:20]
[SOURCE apps/app-frontend/src/composables/use-theme.ts:4]
[SOURCE packages/app-lib/src/state/settings.rs:443]
**Purpose:** The single most-used setting in the app, chosen within seconds of
first launch and then never again. It also has a second-order job: choosing a
*dark* variant is remembered separately from the light one, so "Sync with system"
can pick the right dark shade at sunset.
**Operation:** Selecting a swatch writes `<html class="{theme}-mode">` immediately
[SOURCE apps/app-frontend/src/composables/use-theme.ts:98]. On Save it PATCHes the
account preference `appearance` when SET-2 is on and a user is signed in
[SOURCE apps/app-frontend/src/components/ui/settings/display/AppearanceSettings.vue:49]
and writes `settings.theme` locally regardless.
**Controls:** Five values, exact strings from
[SOURCE apps/app-frontend/src/composables/use-theme.ts:4]:
`dark`, `light`, `oled`, `retro`, `system`.
Labels [SOURCE packages/ui/src/layouts/shared/appearance-settings/components/appearance-settings-theme-selector.vue:20]:
`system` = **"Sync with system"**, `light` = **"Light"**, `dark` = **"Dark"**,
`oled` = **"OLED"**, `retro` = **"Retro"**.
**Default:** `dark` — both the SQL column default [SOURCE packages/app-lib/migrations/20240711194701_init.sql:7]
and the Rust parser's fallback for an unrecognised string
[SOURCE packages/app-lib/src/state/settings.rs:469].
`retro` is **hidden unless developer mode is on** or the user has already chosen
it [SOURCE apps/app-frontend/src/components/ui/settings/display/AppearanceSettings.vue:73].
No reset-to-default; Reset reverts to the last saved theme.
**Persistence:** `settings.theme` (TEXT, `'dark'|'light'|'oled'|'retro'|'system'`) in
the SQLite `settings` table. Device-local. Syncs only if SET-2 is on. Mirrored
into `localStorage['modrinth-theme']` for the pre-DB first paint
[SOURCE apps/app-frontend/src/composables/use-theme.ts:12], and the dark variant
into `localStorage['modrinth-preferred-dark-theme']`
[SOURCE apps/app-frontend/src/composables/use-theme.ts:13].
**Rationale & notes:** "Sync with system" rather than "Auto" is the right call —
it names the mechanism instead of the outcome. Two real problems. First, the
label for the *dark-preference* sub-choice is a sun/moon icon with only a
tooltip ("Preferred light theme" / "Preferred dark theme")
[SOURCE packages/ui/src/layouts/shared/appearance-settings/components/appearance-settings-theme-selector.vue:91]:
an icon-only control with no text label is a poor choice in a settings grid.
Second, `retro` being developer-gated but still persisting means a user can be
locked into a theme whose swatch has vanished from the grid — the only escape is
developer mode. Do not copy that.

## SET-2 — Sync theme across devices
**Type:** toggle
**Section:** Display ▸ Appearance, directly under the theme grid
[SOURCE packages/ui/src/layouts/shared/appearance-settings/layout.vue:54]
**Nav path:** left rail gear → Appearance
**Evidence:** [SOURCE packages/ui/src/layouts/shared/appearance-settings/messages.ts:12]
[SOURCE apps/app-frontend/src/composables/use-app-settings.ts:55]
**Purpose:** Lets someone who owns a desktop and a laptop have one theme without
re-picking it. Reached for only by people with more than one machine.
**Operation:** On Save, if enabled **and** signed in, PATCHes
`appearance: { auto: true }` or `{ auto: false, theme }`
[SOURCE apps/app-frontend/src/components/ui/settings/display/AppearanceSettings.vue:49].
On startup a watcher re-applies account preferences into local settings
[SOURCE apps/app-frontend/src/App.vue:1255].
**Controls:** on/off. Disabled, forced to false, when signed out, with tooltip
"Sign into Modrinth to sync theme"
[SOURCE packages/ui/src/layouts/shared/appearance-settings/messages.ts:21].
**Default:** `true` [SOURCE packages/app-lib/migrations/20260818120000_sync-preferences-settings.sql:1].
**Persistence:** `settings.sync_theme_across_devices`, INTEGER NOT NULL DEFAULT TRUE.
**Rationale & notes:** This is the app's first cross-device setting and it sets
the pattern for SET-5 and SET-13 — three independent toggles for three
independent categories. That is defensible (theme is cosmetic, behaviour is not)
but it means a user who wants "everything follows me" has to find three toggles
in three different tabs. Grouping all three under one "Sync across devices"
heading, as the per-tab wording already half-suggests, would be better. Note also
the wording tells you how to opt out ("Turn this off to keep a separate theme on
this device") which is genuinely good and rare.

## SET-3 — Advanced rendering
**Type:** toggle
**Section:** Display ▸ Appearance, bottom group with SET-4
[SOURCE packages/ui/src/layouts/shared/appearance-settings/layout.vue:102]
**Nav path:** left rail gear → Appearance
**Evidence:** [SOURCE packages/ui/src/layouts/shared/appearance-settings/messages.ts:65]
[SOURCE apps/app-frontend/src/App.vue:219]
**Purpose:** Turns on backdrop blur and similar expensive CSS. Reached for by
users on software rendering, integrated GPUs, remote desktops, or old hardware —
i.e. exactly the users least able to work around a stuttering UI.
**Operation:** Sets a `disable-advanced-rendering` class on the app root
[SOURCE apps/app-frontend/src/App.vue:2219] which disables backdrop filters, and
`noblur` on modal behaviour [SOURCE apps/app-frontend/src/App.vue:390]. No
restart needed.
**Controls:** on/off. Label "Advanced rendering", description
"Enables advanced rendering such as blur effects that may cause performance
issues without hardware-accelerated rendering."
**Default:** **`true`** in the SQL column [SOURCE packages/app-lib/migrations/20240711194701_init.sql:10]
and `true` in the reactive store [SOURCE apps/app-frontend/src/composables/use-theme.ts:54].
**Persistence:** `settings.advanced_rendering`.
**Rationale & notes:** Defaulting the expensive thing *on* is a deliberate bet
that most machines have a GPU. The description names the failure mode
("without hardware-accelerated rendering") but never says how to check whether
you have one, and never says which app chrome is affected. For a reimplementation:
either default it off, or auto-detect and only expose the toggle when the app is
actually on software rendering.

## SET-4 — System window frame
**Type:** toggle
**Section:** Display ▸ Appearance, bottom group
[SOURCE packages/ui/src/layouts/shared/appearance-settings/layout.vue:165]
**Nav path:** left rail gear → Appearance
**Evidence:** [SOURCE packages/ui/src/layouts/shared/appearance-settings/messages.ts:99]
[SOURCE apps/app-frontend/src/App.vue:818]
**Purpose:** Restores the OS title bar and traffic-light/maximise buttons, for
users who want native window snapping, screen readers' window handling, or just
the look of every other app on their desktop. The app ships with
`"decorations": false` and a custom title bar
[SOURCE apps/app/tauri.conf.json — `decorations`/`hiddenTitle`, quoted in RUNLOG
2026-10-14T12:56Z], so this is the escape hatch from that decision.
**Operation:** Written to settings and applied to the window **on next launch**:
`getCurrentWindow().setDecorations(native_decorations)` on startup for non-macOS
[SOURCE apps/app-frontend/src/App.vue:819]. The label says so: "Requires an app
restart."
**Controls:** on/off. **Hidden on macOS** — the provider passes `undefined` when
`platform() === 'macos'` [SOURCE apps/app-frontend/src/components/ui/settings/display/AppearanceSettings.vue:149].
**Default:** `false` [SOURCE packages/app-lib/migrations/20240711194701_init.sql:11].
**Persistence:** `settings.native_decorations`.
**Rationale & notes:** The label is "System window frame", not "Use native window
decorations" — plainer, and it does not leak the implementation word "decorations".
Good. The restart requirement is honestly disclosed. What it does not do is
promote the toggle: it sits under Appearance, indistinguishable from a colour
preference, even though it changes the app's whole shape and needs a restart.

## SET-5 — Sync features across devices
**Type:** toggle
**Section:** Display ▸ Features, its own bordered section above everything else
[SOURCE apps/app-frontend/src/components/ui/settings/display/FeaturesSettings.vue:252]
**Nav path:** left rail gear → Features
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/display/FeaturesSettings.vue:42]
**Purpose:** Lets a user's sidebar/navigation layout follow them between a
laptop and a desktop. Same audience as SET-2.
**Operation:** PATCHes `behavior.{show_jump_in, show_files_tab_in_instances,
show_worlds_tab_in_instances, show_screenshots_tab_in_instances,
show_all_screenshots, show_skin_selector_in_sidebar, quick_instance_count}`
[SOURCE apps/app-frontend/src/components/ui/settings/display/FeaturesSettings.vue:157].
Note this writes the *features* payload into the server's **`behavior`** namespace
— appearance, behavior and features are not three server namespaces, one is.
**Controls:** on/off; disabled when signed out with tooltip "Sign into a Modrinth
account to sync settings."
**Default:** `true` [SOURCE packages/app-lib/migrations/20260902140000_sync-features-setting.sql:1].
**Persistence:** `settings.sync_features_across_devices`.
**Rationale & notes:** See SET-2. The migration also back-fills the new column
from the old behavior-sync value [SOURCE packages/app-lib/migrations/20260902140000_sync-features-setting.sql:5],
which is a nice touch — a new sync toggle inherits the old one's state rather
than silently resetting.

## SET-6 — Show Worlds tab in instances
**Type:** toggle
**Section:** Display ▸ Features ▸ "Instances"
[SOURCE apps/app-frontend/src/components/ui/settings/display/FeaturesSettings.vue:55]
**Nav path:** left rail gear → Features
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/display/FeaturesSettings.vue:59]
**Purpose:** Hides a whole tab in every instance's page for users who never open
a saved world. Reached for by pack-focused users; the tabs cost vertical space in
every single instance view.
**Operation:** `settings.show_worlds_tab_in_instances` plus the reactive mirror
used by the instance page.
**Controls:** on/off, no description text under the heading (only an
`aria-label`) — unlike the rows in the Sidebar group.
**Default:** `true` [SOURCE packages/app-lib/migrations/20260902130000_promote-instance-navigation-settings.sql:5].
**Persistence:** `settings.show_worlds_tab_in_instances`, CHECK IN (0,1).
**Rationale & notes:** This setting was **demoted from a feature flag to a real
setting on 2026-09-02**, and the migration migrates any user who had already
toggled the old flag, preserving both value and name
[SOURCE packages/app-lib/migrations/20260902130000_promote-instance-navigation-settings.sql:16].
That promotion ladder — flag → default-off setting → default-on setting → user
toggle — is how this app retires flags without breaking anyone. Worth copying.

## SET-7 — Show Files tab in instances
**Type:** toggle
**Section:** Display ▸ Features ▸ "Instances"
**Nav path:** left rail gear → Features
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/display/FeaturesSettings.vue:63]
**Purpose:** As SET-6, for the file browser tab. Advanced/power users keep it;
most never touch it.
**Operation:** As SET-6.
**Controls:** on/off.
**Default:** `true` [SOURCE packages/app-lib/migrations/20260902130000_promote-instance-navigation-settings.sql:2].
**Persistence:** `settings.show_files_tab_in_instances`, CHECK IN (0,1).
**Rationale & notes:** The row order is Worlds, Files, Screenshots — source order
[SOURCE apps/app-frontend/src/components/ui/settings/display/FeaturesSettings.vue:283].
It is **not** the order the migration added them (Files, Worlds, Screenshots,
Skin selector), and not alphabetical. So there is a deliberate human ordering
here that does not survive being read off the schema. Capture it from the UI,
not from the DB.

## SET-8 — Show Screenshots tab in instances
**Type:** toggle
**Section:** Display ▸ Features ▸ "Instances"
**Nav path:** left rail gear → Features
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/display/FeaturesSettings.vue:67]
**Purpose:** Same as SET-6/7 for a newer feature, so nobody is forced to have a
tab for a feature they do not use.
**Operation:** As SET-6.
**Controls:** on/off.
**Default:** **`false`** — the only one of the three instance tabs to ship off
[SOURCE packages/app-lib/migrations/20260902130000_promote-instance-navigation-settings.sql:17],
[SOURCE apps/app-frontend/src/composables/use-app-settings.ts:56].
**Persistence:** `settings.show_screenshots_tab_in_instances`, CHECK IN (0,1).
**Rationale & notes:** Defaults track feature maturity: Files and Worlds on
because they are old, Screenshots off because it is new. This is good, correct
practice, and it is exactly why "default" is not a constant across a settings
table — a reimplementation must not assume one.

## SET-9 — Show all screenshots in sidebar
**Type:** toggle
**Section:** Display ▸ Features ▸ "Sidebar"
[SOURCE apps/app-frontend/src/components/ui/settings/display/FeaturesSettings.vue:330]
**Nav path:** left rail gear → Features
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/display/FeaturesSettings.vue:75]
[SOURCE apps/app-frontend/src/composables/use-app-settings.ts:57]
**Purpose:** Adds a rail button that shows screenshots from every instance.
Reached for by screenshot-heavy players.
**Operation:** This is the **odd one out**: it does not live in the `settings`
table at all. It calls `set_global_synced_option('screenshots', value)`
[SOURCE apps/app-frontend/src/components/ui/settings/display/FeaturesSettings.vue:186],
writing to `sync_feature_settings` [SOURCE packages/app-lib/migrations/20260819120000_instance-synced-options.sql:3].
It is presented under "Features ▸ Sidebar" but is technically one of the seven
*synced options* that SET-34…SET-38 also live in — and it is the only synced
option shown in the Features tab rather than the Synced settings tab.
**Controls:** on/off, with description.
**Default:** `globally_enabled = 1` for `screenshots` on every install path —
both the seed insert [SOURCE packages/app-lib/migrations/20260819120000_instance-synced-options.sql:29]
and the upgrade path [SOURCE packages/app-lib/migrations/20260819120000_instance-synced-options.sql:53].
**Persistence:** `sync_feature_settings.screenshots`. Not in `settings`, not synced
to the account by default (only under SET-5).
**Rationale & notes:** A setting whose label says "sidebar" living in a table
called "sync_feature_settings" is a genuine modelling mismatch, and it is the
user's problem too: a user who turns it off in Features will not find it in
Synced settings to turn it back on. Keep one store per concept.

## SET-10 — Show skin selector in sidebar
**Type:** toggle
**Section:** Display ▸ Features ▸ "Sidebar"
**Nav path:** left rail gear → Features
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/display/FeaturesSettings.vue:84]
[SOURCE apps/app-frontend/src/App.vue:2336]
**Purpose:** Adds the skin-selector rail button. Skinned players turn it on;
everyone else leaves the rail shorter.
**Operation:** `settings.show_skin_selector_in_sidebar`; the rail button is
`v-if`'d on it [SOURCE apps/app-frontend/src/App.vue:2335].
**Controls:** on/off, with description.
**Default:** `true` [SOURCE packages/app-lib/migrations/20260902130000_promote-instance-navigation-settings.sql:18].
**Persistence:** `settings.show_skin_selector_in_sidebar`, CHECK IN (0,1).
**Rationale & notes:** Defaults to on even though SET-6/7/8 are about hiding
*tabs* — the reasoning is that a rail button is cheap and discoverable, an
instance tab is expensive and permanent. Defensible; just note it is not
consistent, and the user is never told why.

## SET-11 — Quick instances in sidebar
**Type:** number (slider + numeric input)
**Section:** Display ▸ Features ▸ "Sidebar", last row
[SOURCE apps/app-frontend/src/components/ui/settings/display/FeaturesSettings.vue:360]
**Nav path:** left rail gear → Features
**Evidence:** [SOURCE apps/app-frontend/src/composables/use-quick-instance-limit.ts:3]
[SOURCE apps/app-frontend/src/components/ui/settings/display/FeaturesSettings.vue:92]
**Purpose:** Caps how many recent instances the rail lists, so a 40-instance
library doesn't push everything else off the rail. Reached for by heavy users
once the rail gets long.
**Operation:** Stored in **`localStorage`**, not the DB
[SOURCE apps/app-frontend/src/composables/use-quick-instance-limit.ts:20], and
pushed to the account under SET-5 as `behavior.quick_instance_count`.
**Controls:** integer **0 … 20** (`QUICK_INSTANCE_LIMIT_MAX = 20`), step 1.
Description: "Choose the maximum number of recent instances shown in the left
sidebar. You can also adjust this by dragging the divider."
**Default:** **unset**. `normalizeLimit` maps `null` (or anything ≥ 20) back to
`null` [SOURCE apps/app-frontend/src/composables/use-quick-instance-limit.ts:7],
and the settings UI shows `QUICK_INSTANCE_LIMIT_MAX` when the stored value is
`null` [SOURCE apps/app-frontend/src/components/ui/settings/display/FeaturesSettings.vue:141] —
so the slider *displays* 20 but the stored value is "no cap", which are
different things. Also draggable in-place via a divider in the rail, which writes
the same key.
**Persistence:** `localStorage['modrinth-quick-instance-count']`; `null` means the
key is absent.
**Rationale & notes:** Two independent problems. (a) The displayed default (20)
is not the stored default (unbounded) — a user who saves the tab without moving
the slider writes `20` and thereby *imposes* a cap they never chose.
(b) `normalizeLimit` discards any value ≥ 20, so a future raise of
`QUICK_INSTANCE_LIMIT_MAX` would silently reset existing users. Do not encode
"max value" as "unset" in one helper.

## SET-12 — Show Jump in section
**Type:** toggle
**Section:** Display ▸ Features ▸ "Play page" (its own group)
[SOURCE apps/app-frontend/src/components/ui/settings/display/FeaturesSettings.vue:379]
**Nav path:** left rail gear → Features
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/display/FeaturesSettings.vue:105]
[SOURCE apps/app-frontend/src/composables/use-app-settings.ts:36]
**Purpose:** Controls a "Jump in" block of recently-played worlds and instances
at the top of the Play page. The only setting in the tab with a whole section to
itself, which over-sells it.
**Operation:** `settings.show_jump_in` (value mirrored into the reactive store as
`showJumpIn`).
**Controls:** on/off, with description "Show recently played worlds and instances
at the top of the Play page."
**Default:** `true` [SOURCE packages/app-lib/migrations/20260918120000_promote-behavior-settings.sql:17].
Previously a feature flag named `worlds_in_home`; the migration inverts nothing
here but does rename it [SOURCE packages/app-lib/migrations/20260918120000_promote-behavior-settings.sql:50].
**Persistence:** `settings.show_jump_in`.
**Rationale & notes:** The label changed from the flag's honest `worlds_in_home`
to a vaguer "Jump in section" — a regression in clarity that came with promotion
to a real setting. Note also that SET-12 is in **Features** while the migration
that created it is named `promote-behavior-settings`; the taxonomy and the
migration history have drifted apart.

## SET-13 — Sync behavior across devices
**Type:** toggle
**Section:** Display ▸ Behavior, own bordered section at the very top
[SOURCE apps/app-frontend/src/components/ui/settings/display/BehaviorSettings.vue:241]
**Nav path:** left rail gear → Behavior
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/display/BehaviorSettings.vue:33]
**Purpose:** Same audience as SET-2/SET-5. Distinguishes "how the app behaves"
(which people want consistent) from "what the app shows" (which some want local).
**Operation:** PATCHes `behavior.{minimize_app, refocus_on_game_close,
hide_right_sidebar, compact_instance_cards, show_play_time, hide_nametag,
warn_on_unknown_modpacks, skip_non_essential_warnings}`
[SOURCE apps/app-frontend/src/components/ui/settings/display/BehaviorSettings.vue:168].
**Controls:** on/off; disabled when signed out, tooltip "Sign into a Modrinth
account to sync settings."
**Default:** `true` [SOURCE packages/app-lib/migrations/20260818120000_sync-preferences-settings.sql:4].
**Persistence:** `settings.sync_behavior_across_devices`.
**Rationale & notes:** See SET-2's note about three toggles in three tabs. The
**description is the best in the whole settings surface** — "Use these behavior
settings everywhere you're signed in. Turn this off to keep separate settings on
this device." It says what it does, who it affects, and how to opt out. Every
other sync toggle should be written that way.

## SET-14 — Minimize app
**Type:** toggle
**Section:** Display ▸ Behavior ▸ "Startup and navigation", first row
[SOURCE apps/app-frontend/src/components/ui/settings/display/BehaviorSettings.vue:273]
**Nav path:** left rail gear → Behavior
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/display/BehaviorSettings.vue:58]
[SOURCE packages/app-lib/src/launcher/mod.rs:1253]
**Purpose:** Gets the launcher out of the way when a game starts. Reached for by
anyone who launches several instances and does not want the launcher stealing
focus mid-session.
**Operation:** In Rust, after a process is spawned, if the setting is on, calls
`window.minimize()` [SOURCE packages/app-lib/src/launcher/mod.rs:1253].
**Controls:** on/off, with description "Minimize Modrinth App when Minecraft
starts."
**Default:** `false` [SOURCE packages/app-lib/migrations/20240711194701_init.sql:28].
**Persistence:** `settings.hide_on_process_start` — note the persisted name still
says "process", the label says "Minecraft". A name/label mismatch that survived
several renames.
**Rationale & notes:** The label says "Minimize app" inside a tab already called
Behavior, next to "Refocus app when Minecraft closes" which explicitly says
"Minecraft". Inconsistent naming for a matched pair of settings — the two should
read as a pair. Also: on macOS `minimize()` puts the window in the Dock rather
than hiding it; there is no dock-hide option.

## SET-15 — Refocus app when Minecraft closes
**Type:** toggle
**Section:** Display ▸ Behavior ▸ "Startup and navigation", second row
[SOURCE apps/app-frontend/src/components/ui/settings/display/BehaviorSettings.vue:285]
**Nav path:** left rail gear → Behavior
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/display/BehaviorSettings.vue:66]
[SOURCE packages/app-lib/src/state/process.rs:994]
**Purpose:** The companion to SET-14, and the one that actually matters: without
it, a user who minimises the launcher (SET-14) never gets it back. Reached for
immediately after enabling SET-14.
**Operation:** On process exit, Rust calls `window.unminimize()` then
`window.set_focus()` [SOURCE packages/app-lib/src/state/process.rs:994].
**Controls:** on/off, with description "Bring Modrinth App to the foreground when
Minecraft exits."
**Default:** `false` [SOURCE packages/app-lib/migrations/20260918120000_promote-behavior-settings.sql:1].
**Persistence:** `settings.refocus_on_game_close`.
**Rationale & notes:** This is a **pairing bug**: SET-14 and SET-15 are two halves
of one behaviour and are independent booleans, so the combination "minimise on
launch, never come back" is reachable and is a support ticket waiting to happen.
The descriptions don't cross-reference. A reimplementation should make these one
setting with two fields, or make SET-15 default-on whenever SET-14 is on.

## SET-16 — Hide right sidebar
**Type:** toggle
**Section:** Display ▸ Behavior ▸ "Startup and navigation", third row
[SOURCE apps/app-frontend/src/components/ui/settings/display/BehaviorSettings.vue:302]
**Nav path:** left rail gear → Behavior
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/display/BehaviorSettings.vue:82]
[SOURCE packages/app-lib/migrations/20241222013857_feature-flags.sql:1]
**Purpose:** Collapses the right-hand context sidebar (News, Getting started,
friends) and adds a button to bring it back. Reached for on small screens or by
people who want the content pane wider.
**Operation:** `settings.toggle_sidebar`; the persisted name is the **inverse** of
the label — `toggle` vs `hide`.
**Controls:** on/off, description "Hide the right sidebar by default and add a
button to show or hide it."
**Default:** `false` (sidebar shown) [SOURCE packages/app-lib/migrations/20241222013857_feature-flags.sql:1].
**Persistence:** `settings.toggle_sidebar`.
**Rationale & notes:** The label/description pair is good: it says both the effect
and the affordance ("add a button"). The **key name is the opposite polarity of
the label**, which is exactly the kind of thing that produces "why did my sidebar
disappear" bugs after a refactor. Rename the key or the label; never both.

## SET-17 — Compact mode
**Type:** toggle
**Section:** Display ▸ Behavior ▸ "Home and content", first row
[SOURCE apps/app-frontend/src/components/ui/settings/display/BehaviorSettings.vue:319]
**Nav path:** left rail gear → Behavior
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/display/BehaviorSettings.vue:90]
[SOURCE packages/app-lib/migrations/20260918120000_promote-behavior-settings.sql:5]
**Purpose:** Switches library instance cards from a grid to a dense row list, so
more instances fit on one screen. Reached for by anyone with more than ~8
instances.
**Operation:** `settings.compact_instance_cards`.
**Controls:** on/off, description "Display library instances in a compact row
layout."
**Default:** `false` [SOURCE packages/app-lib/migrations/20260918120000_promote-behavior-settings.sql:5].
**Persistence:** `settings.compact_instance_cards`.
**Rationale & notes:** "Compact mode" is a name borrowed from email clients where
it usually also means reduced chrome everywhere; here it only affects instance
cards. The description rescues it, but the label alone would mislead. Prefer
"Compact instance list".

## SET-18 — Show play time
**Type:** toggle
**Section:** Display ▸ Behavior ▸ "Home and content", second row
[SOURCE apps/app-frontend/src/components/ui/settings/display/BehaviorSettings.vue:329]
**Nav path:** left rail gear → Behavior
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/display/BehaviorSettings.vue:98]
[SOURCE packages/app-lib/migrations/20260918120000_promote-behavior-settings.sql:8]
**Purpose:** Shows accumulated play time per instance. Reached for by players who
track hours, and by nobody else — which is why it defaults on (it is also a
nice screenshot).
**Operation:** `settings.show_play_time`.
**Controls:** on/off, description "Show how long you've played each instance."
**Default:** `true` [SOURCE packages/app-lib/migrations/20260918120000_promote-behavior-settings.sql:8].
The pre-migration flag was named `show_instance_play_time` and the migration maps
it explicitly [SOURCE packages/app-lib/migrations/20260918120000_promote-behavior-settings.sql:47].
**Persistence:** `settings.show_play_time`.
**Rationale & notes:** Nothing to fault. The one genuinely nice touch in this
tab is that the migration enumerates **every** renamed flag with an explicit
`COALESCE(json_extract(feature_flags, '$.old_name'), <new default>)` and then
`json_remove`s the old keys [SOURCE packages/app-lib/migrations/20260918120000_promote-behavior-settings.sql:59]
— so the JSON never accumulates dead flags. That is the migration discipline to
copy.

## SET-19 — Hide nametag
**Type:** toggle
**Section:** Display ▸ Behavior ▸ "Home and content", third row
[SOURCE apps/app-frontend/src/components/ui/settings/display/BehaviorSettings.vue:339]
**Nav path:** left rail gear → Behavior
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/display/BehaviorSettings.vue:106]
[SOURCE packages/app-lib/migrations/20250514181748_skin_nametag_setting.sql:1]
**Purpose:** Removes the username overlay above the avatar preview on the skin
selector, for taking clean screenshots of a skin. A pure edge case — and the
setting is named and placed as if it were not.
**Operation:** `settings.hide_nametag_skins_page` (persisted name mentions
"skins page"; the label does not mention skins either — only the description
does).
**Controls:** on/off, description "Hide your username above the player preview on
the Skin selector page."
**Default:** `false` [SOURCE packages/app-lib/migrations/20250514181748_skin_nametag_setting.sql:1].
Note this migration is the only one that spells its check as
`CHECK (hide_nametag_skins_page IN (0, 1))` instead of relying on the INTEGER
column — belt and braces.
**Persistence:** `settings.hide_nametag_skins_page`.
**Rationale & notes:** A screenshot-only setting shipped as a top-level Behavior
toggle. It should live next to the skin selector, or in an "Advanced" group. Its
default of `false` with a positive-phrased label ("Hide nametag") means every
reader has to invert it mentally; "Show nametag" would be kinder. It is also the
only row in "Home and content" that has nothing to do with home or content —
the grouping is thematic, not functional.

## SET-20 — Warn me before installing unknown modpacks
**Type:** toggle
**Section:** Display ▸ Behavior ▸ "Confirmations", first row
[SOURCE apps/app-frontend/src/components/ui/settings/display/BehaviorSettings.vue:356]
**Nav path:** left rail gear → Behavior
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/display/BehaviorSettings.vue:114]
[SOURCE apps/app-frontend/src/components/ui/install_flow/UnknownPackWarningModal.vue:53]
**Purpose:** Stops you from double-clicking a `.mrpack` you downloaded from
somewhere else without noticing you did. Reached for by anyone who sideloads
packs.
**Operation:** `settings.warn_on_unknown_modpacks`. The warning modal is shown on
install of a non-Modrinth-hosted pack, and **accepting the warning sets the
setting to `false` permanently** [SOURCE apps/app-frontend/src/components/ui/install_flow/UnknownPackWarningModal.vue:53].
**Controls:** on/off, description "Show a safety warning before installing a
Modrinth Pack (.mrpack) that isn't hosted on Modrinth."
**Default:** `true` [SOURCE packages/app-lib/migrations/20260918120000_promote-behavior-settings.sql:11].
The migration derives it from the **negated** old flag `skip_unknown_pack_warning`
[SOURCE packages/app-lib/migrations/20260918120000_promote-behavior-settings.sql:48].
**Persistence:** `settings.warn_on_unknown_modpacks`.
**Rationale & notes:** Two things to copy: the migration correctly inverts the old
negative flag while renaming it, and the description names the exact file
extension. One thing not to copy: **clicking through the warning silently and
permanently turns the safety feature off**. A "don't ask again" checkbox would
be explicit; a side effect of the confirm button is not. This is the sharpest
UX defect in the settings surface.

## SET-21 — Skip non-essential warnings
**Type:** toggle
**Section:** Display ▸ Behavior ▸ "Confirmations", second row
[SOURCE apps/app-frontend/src/components/ui/settings/display/BehaviorSettings.vue:371]
**Nav path:** left rail gear → Behavior
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/display/BehaviorSettings.vue:123]
[SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:172]
**Purpose:** A master "stop asking me" for the whole app — duplicate installs,
ordinary deletions, bulk updates, unlinking, repairs. Reached for by heavy,
deliberate users who find confirmations friction. It is a **power-user** setting
wearing plain clothes.
**Operation:** Consulted at each confirmation site; it is checked *before* the
confirm modal is even created, e.g. "Purge cache" runs immediately when it is on
[SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:171].
The flag is *not* synced to the account: the server payload in
`BehaviorSettings.vue` includes it, but the startup pull-back in
`apps/app-frontend/src/App.vue:1266` also applies it — worth noting it is one of
the few behaviour settings that round-trips.
**Controls:** on/off. The description is the longest in the app and is genuinely
load-bearing: "Skip confirmations for low-risk actions such as duplicate
installs, normal content deletion, bulk updates, unlinking, and repairs.
Warnings for dangerous actions are always shown."
**Default:** `false` [SOURCE packages/app-lib/migrations/20260918120000_promote-behavior-settings.sql:14].
**Persistence:** `settings.skip_non_essential_warnings`.
**Rationale & notes:** The wording is a model: name the risk classes both ways
("…such as X, Y, Z" **and** "Warnings for dangerous actions are always shown"),
so the user knows the boundary of what they are giving up. It is the only setting
in the app that explains its own limits. The label, though, is the weakest thing
here — "non-essential" is undefined on its face. If we ship this, label it
"Safe actions won't ask for confirmation".

## SET-22 — Language
**Type:** select (searchable list with translation-coverage percentages)
**Section:** Display ▸ Language (badged **Beta**)
[SOURCE apps/app-frontend/src/components/ui/modal/AppSettingsModal.vue:108]
**Nav path:** left rail gear → Language
**Evidence:** [SOURCE packages/ui/src/layouts/wrapped/settings/language-settings/index.vue:1]
[SOURCE packages/ui/src/composables/i18n.ts:38]
[SOURCE packages/app-lib/migrations/20251229145719_add_locale_setting.sql:2]
**Purpose:** App and website localisation. Reached for by every non-English
speaker, once, at first launch — the single most consequential setting for the
people who need it.
**Operation:** `setLocale` swaps `vue-i18n`'s active locale and lazily fetches
the two JSON bundles (app + shared UI)
[SOURCE apps/app-frontend/src/i18n.config.ts:45]. On Save it PATCHes
`localization.locale` to the account, then persists the value locally via the
app's `persistLocale` prop [SOURCE apps/app-frontend/src/components/ui/settings/display/LanguageSettings.vue:30].
**Controls:** **32 locales** — the full list is in `LOCALES`
[SOURCE packages/ui/src/composables/i18n.ts:38]: `cs-CZ`, `da-DK`, `de-CH`,
`de-DE`, `en-US`, `es-419`, `es-ES`, `fi-FI`, `fil-PH`, `fr-FR`, `he-IL`,
`hu-HU`, `id-ID`, `it-IT`, `ja-JP`, `ko-KR`, `ms-MY`, `nl-NL`, `no-NO`, `pl-PL`,
`pt-BR`, `pt-PT`, `ro-RO`, `ru-RU`, `sr-CS`, `sv-SE`, `th-TH`, `tr-TR`, `uk-UA`,
`vi-VN`, `zh-CN`, `zh-TW`. **`ar-SA` is commented out** with the note "Commented
out as it's RTL - will enable when we have better RTL support"
[SOURCE packages/ui/src/composables/i18n.ts:39] — so 33 locale folders ship and 32
are selectable. Each row shows a flag (from `flagcdn.com`), the language's own
name, the English name, and a "% supported" figure from a generated coverage
table. An unknown tag silently falls back to `en-US`
[SOURCE apps/app-frontend/src/i18n.config.ts:47].
The tab opens with a warning admonition
[SOURCE packages/ui/src/utils/common-messages.ts:1278] and a Crowdin credit.
**Default:** `en-US` [SOURCE packages/app-lib/migrations/20251229145719_add_locale_setting.sql:2].
Reset restores the last *saved* locale [SOURCE packages/ui/src/utils/savable.ts:44];
leaving the tab with unsaved changes also reverts the live locale
[SOURCE packages/ui/src/layouts/wrapped/settings/language-settings/index.vue:107].
**Persistence:** `settings.locale` (TEXT NOT NULL DEFAULT 'en-US').
Device-local. Also synced to the account as `localization.locale`, and applied
from the account at startup **regardless of any toggle** — language is the one
preference that is always synced
[SOURCE apps/app-frontend/src/App.vue:1251].
**Rationale & notes:** Three genuinely good decisions here: (a) showing
translation *coverage* next to each language, so a user knows in advance that
choosing Polish gets them 62% Polish; (b) a plain warning that untranslated
strings fall back to English instead of pretending to be complete; (c) a Beta
badge, which is honest — the RTL language is literally disabled for lack of
layout support. The one flaw: the language is synced unconditionally while
everything else needs a toggle, which is inconsistent in a way a user cannot
reason about.

## SET-23 — Feature flags
**Type:** toggle ×10 (plus a search box and a per-row reset)
**Section:** Display ▸ Feature flags — **developer mode only**
[SOURCE apps/app-frontend/src/components/ui/modal/AppSettingsModal.vue:118]
**Nav path:** left rail gear → Feature flags (only present once developer mode is
on), or account overflow menu → Feature flags
[SOURCE apps/app-frontend/src/App.vue:1567]
**Evidence:** [SOURCE apps/app-frontend/src/composables/use-app-settings.ts:3]
[SOURCE packages/app-lib/src/state/settings.rs:96]
[SOURCE apps/app-frontend/src/components/ui/settings/display/FeatureFlagSettings.vue:47]
**Purpose:** Ship half-built features to staff and power users without a
release. Reached for by Modrinth employees and bug reporters; hidden from
everyone else.
**Operation:** Rows are written straight through to `settings.feature_flags`
(JSONB) on every change, with no save step
[SOURCE apps/app-frontend/src/components/ui/settings/display/FeatureFlagSettings.vue:39].
Flags are read at startup with `Object.assign(appSettings.featureFlags, feature_flags)`
[SOURCE apps/app-frontend/src/App.vue:809] and consumed lazily via
`getFeatureFlag`, which falls back to the hard-coded default
[SOURCE apps/app-frontend/src/composables/use-app-settings.ts:31].
**Controls:** ten flags. Defaults and what each gates:

| Flag key (label as rendered, `_`→space) | Default | Gates |
| --- | --- | --- |
| `project_background` | `false` | Project hero background image [SOURCE apps/app-frontend/src/pages/project/Index.vue:58] |
| `page_path` | `false` | Debug pill showing `route.fullPath` in the viewport [SOURCE apps/app-frontend/src/App.vue:2465] |
| `server_project_qa` | `false` | Server project QA surfaces; no consumer found in this tag |
| `show_version_environment_column` | `false` | Environment column on a project's Versions table [SOURCE apps/app-frontend/src/pages/project/Versions.vue:8] |
| `server_ram_as_bytes_always_on` | `false` | Force RAM shown in bytes for servers [SOURCE apps/app-frontend/src/App.vue:384] |
| `always_show_app_controls` | `false` | Always-visible app window controls [SOURCE apps/app-frontend/src/components/ui/WindowControls.vue:56] |
| `show_sync_instances_update_modal` | `false` | The sync-instances update modal [SOURCE apps/app-frontend/src/App.vue:1210] |
| `pride_fundraiser` | **`true`** | Seasonal Pride banner, additionally date-gated by `PRIDE_FUNDRAISER_END_DATE` [SOURCE apps/app-frontend/src/App.vue:280] |
| `i18n_debug` | `false` | Overlay marking untranslated strings [SOURCE apps/app-frontend/src/plugins/i18n-debug.ts:18] |
| `localhost_sign_in` | `false` | **Security-relevant**: points the OAuth start URL at `http://localhost:3000/auth/sign-in` [SOURCE packages/app-lib/src/api/mr_auth.rs:20] |

Reset: a per-row "Reset to default" button, disabled when the flag already
equals its default [SOURCE apps/app-frontend/src/components/ui/settings/display/FeatureFlagSettings.vue:63].
**This is the only Reset in the whole settings surface that restores a factory
default** — which is the correct behaviour and shows the authors know the
difference. Search uses Fuse.js at threshold 0.4
[SOURCE apps/app-frontend/src/components/ui/settings/display/FeatureFlagSettings.vue:20].
Note the toggles all share `id="advanced-rendering"` [SOURCE apps/app-frontend/src/components/ui/settings/display/FeatureFlagSettings.vue:71]
— a copy-paste accessibility bug: ten rows, one DOM id.
**Persistence:** `settings.feature_flags` JSONB, default `'{}'`
[SOURCE packages/app-lib/migrations/20241222013857_feature-flags.sql:2].
**Rationale & notes:** The promotion ladder (see SET-6) is visible in the data:
four of these ten used to be flags and are now first-class settings. `pride_fundraiser`
defaulting to `true` in a *flag* is the smell that motivates the ladder — a
seasonal campaign should not live in a list of developer toggles.
Two real hazards: `localhost_sign_in` is a flag that redirects an OAuth flow to
plain HTTP on localhost, which in the wrong hands is a token-interception
primitive; and the Rust enum has drifted from the frontend map — the Rust
`FeatureFlag` has `ServersInApp` which the frontend never sends
[SOURCE packages/app-lib/src/state/settings.rs:103], and the frontend sends
`show_version_environment_column` which the Rust enum does not accept. Because
deserialisation failure is swallowed with `.ok()` and then `unwrap_or_default()`
[SOURCE packages/app-lib/src/state/settings.rs:204], an unknown key silently
**wipes the entire flag map**. That is a real, current bug, and it is exactly
what the "promote or delete, never leave stale" discipline prevents.

## SET-24 — Profile picture
**Type:** button (file picker + remove + reset)
**Section:** Account ▸ Profile, "Profile picture"
[SOURCE packages/ui/src/layouts/wrapped/settings/AccountProfileSettings.vue:50]
**Nav path:** left rail gear → Profile
**Evidence:** [SOURCE packages/ui/src/layouts/wrapped/settings/AccountProfileSettings.vue:54]
**Purpose:** Public identity on Modrinth. Reached for once, then rarely.
**Operation:** `change_user_avatar` uploads to the account
[SOURCE apps/app-frontend/src/components/ui/settings/account/ProfileSettings.vue:57];
`delete_user_avatar` removes it. Save is explicit (floating action bar).
**Controls:** file types `image/png, image/jpeg, image/gif, image/webp`
[SOURCE packages/ui/src/layouts/wrapped/settings/AccountProfileSettings.vue:60];
**max size 524288 bytes (512 KiB)**
[SOURCE packages/ui/src/layouts/wrapped/settings/AccountProfileSettings.vue:58].
Buttons: "Upload image" (FileButton), "Remove image" (shown only when an avatar
exists and no deletion is pending), "Reset" (shown only when there is a pending
change).
**Default:** whatever the account already has.
**Persistence:** account-side only — **nothing in the app's settings DB**. This
is the clearest example of the account-settings scope.
**Rationale & notes:** The three-button affordance (upload / remove / reset) is
the right shape for a staged edit with an undo. The 512 KiB cap is not stated
anywhere in the UI; a `FileButton` that silently rejects a 2 MB PNG is a poor
experience. Show the limit.

## SET-25 — Username
**Type:** text
**Section:** Account ▸ Profile
[SOURCE packages/ui/src/layouts/wrapped/settings/AccountProfileSettings.vue:88]
**Nav path:** left rail gear → Profile
**Evidence:** [SOURCE packages/ui/src/layouts/wrapped/settings/AccountProfileSettings.vue:93]
**Purpose:** Public identity. Rarely changed; changed once at most.
**Operation:** `PATCH`ed via `patch_user` with only the changed field
[SOURCE apps/app-frontend/src/components/ui/settings/account/ProfileSettings.vue:50].
**Controls:** hard limit **39 characters** — the input is marked `:error` above
39 [SOURCE packages/ui/src/layouts/wrapped/settings/AccountProfileSettings.vue:97]
and a live `n/39` counter appears from **30** characters
[SOURCE packages/ui/src/layouts/wrapped/settings/AccountProfileSettings.vue:100],
turning red past 39. Description: "A unique case-insensitive name to identify
your profile."
**Default:** the account's current username.
**Persistence:** account-side (`username` field of the v2 user object).
**Rationale & notes:** The "warn early, cap hard" pattern (amber at 30, red at 39)
is genuinely well done and worth copying. The description explaining
"case-insensitive" saves a support question.

## SET-26 — Bio
**Type:** text (textarea)
**Section:** Account ▸ Profile
[SOURCE packages/ui/src/layouts/wrapped/settings/AccountProfileSettings.vue:112]
**Nav path:** left rail gear → Profile
**Evidence:** [SOURCE packages/ui/src/layouts/wrapped/settings/AccountProfileSettings.vue:116]
**Purpose:** Public profile blurb. Rarely changed.
**Operation:** `PATCH`ed with `bio` when changed
[SOURCE packages/ui/src/layouts/wrapped/settings/AccountProfileSettings.vue:281].
**Controls:** limit **160 characters**, live counter always visible, `:error`
and red counter above 160
[SOURCE packages/ui/src/layouts/wrapped/settings/AccountProfileSettings.vue:116].
**Default:** the account's current bio.
**Persistence:** account-side.
**Rationale & notes:** The whole Account ▸ Profile tab is a public profile editor
that happens to live in the app's settings modal. Two links at the top explain
that the profile is public and link to it and to the API docs
[SOURCE packages/ui/src/layouts/wrapped/settings/AccountProfileSettings.vue:26].
That disclosure is a small, correct piece of consent UX that most apps skip.

## SET-27 — Friend requests
**Type:** select (segmented chips)
**Section:** Account ▸ Social, first group
[SOURCE packages/ui/src/layouts/wrapped/settings/AccountSocialSettings.vue:26]
**Nav path:** left rail gear → Social
**Evidence:** [SOURCE packages/ui/src/layouts/wrapped/settings/AccountSocialSettings.vue:254]
**Purpose:** Controls who can send you a friend request. Privacy-relevant, so it
lives in Social rather than Privacy — arguably a mis-file.
**Operation:** `updatePreferences({ social: { friend_privacy } })`, explicit save
[SOURCE packages/ui/src/layouts/wrapped/settings/AccountSocialSettings.vue:340].
**Controls:** three values, all three shown: **`everyone` → "Everyone"**,
**`mutual` → "Friends of friends"**, **`none` → "No one"**
[SOURCE packages/ui/src/layouts/wrapped/settings/AccountSocialSettings.vue:257].
Description: "Control who can send you friend requests on Modrinth."
**Default:** `everyone` — the frontend's local fallback
[SOURCE packages/ui/src/layouts/wrapped/settings/AccountSocialSettings.vue:254].
The server-side default is not in this repo; `[ORCH-PENDING]` if you need it
authoritative.
**Persistence:** account preferences, **always synced**; no opt-in toggle exists.
**Rationale & notes:** The middle option is named "Friends of friends" while its
value is `mutual` and the type is `FriendPrivacy`. "Mutual friends" is the phrase
users know; "friends of friends" invites a reading of a two-hop graph that it
is not. Always-synced with no local override is right for privacy: a device you
share should not silently widen it.

## SET-28 — Shared instance invites
**Type:** select (segmented chips)
**Section:** Account ▸ Social, second group
[SOURCE packages/ui/src/layouts/wrapped/settings/AccountSocialSettings.vue:46]
**Nav path:** left rail gear → Social
**Evidence:** [SOURCE packages/ui/src/layouts/wrapped/settings/AccountSocialSettings.vue:258]
**Purpose:** Who may invite you to a shared (collaborative) instance.
**Operation:** `updatePreferences({ social: { shared_instances_privacy } })`.
**Controls:** **`everyone` → "Everyone"**, **`friends` → "Friends"**,
**`none` → "No one"** [SOURCE packages/ui/src/layouts/wrapped/settings/AccountSocialSettings.vue:258].
Note this option set differs from SET-27 (`friends` instead of `mutual`) even
though the component reuses one `formatInteractionSource`.
**Default:** `everyone` [SOURCE packages/ui/src/layouts/wrapped/settings/AccountSocialSettings.vue:255].
**Persistence:** account preferences.
**Rationale & notes:** Good that the invite audience is a strict subset of what
friends mean. The invisible inconsistency — `mutual` here vs `friends` there —
is handled by a shared formatter so the UI is consistent, which is the right
call.

## SET-29 — Hosting access invites
**Type:** select (segmented chips)
**Section:** Account ▸ Social, third group
[SOURCE packages/ui/src/layouts/wrapped/settings/AccountSocialSettings.vue:65]
**Nav path:** left rail gear → Social
**Evidence:** [SOURCE packages/ui/src/layouts/wrapped/settings/AccountSocialSettings.vue:69]
**Purpose:** Who may invite you to administer a Modrinth Hosting server. The most
privileged of the three, and it gets the same visual weight as the other two.
**Operation:** `updatePreferences({ social: { hosting_access_privacy } })`.
**Controls:** **`everyone`**, **`friends`**, **`none`** — the same array as SET-28
[SOURCE packages/ui/src/layouts/wrapped/settings/AccountSocialSettings.vue:71].
Description: "Control who can invite you to manage a Modrinth Hosting server."
**Default:** `everyone` [SOURCE packages/ui/src/layouts/wrapped/settings/AccountSocialSettings.vue:256].
**Persistence:** account preferences.
**Rationale & notes:** The default of `everyone` for the most privileged action,
presented identically to the least, is the one thing in this tab I'd change. At
minimum this should default to `friends`, and at best it should be visually
separated as "admin access".

## SET-30 — Blocked users
**Type:** info list + button
**Section:** Account ▸ Social, second section, below a rule
[SOURCE packages/ui/src/layouts/wrapped/settings/AccountSocialSettings.vue:85]
**Nav path:** left rail gear → Social
**Evidence:** [SOURCE packages/ui/src/layouts/wrapped/settings/AccountSocialSettings.vue:100]
**Purpose:** Reverse a block. Reached for after a fall-out, rarely.
**Operation:** `GET blocked_users` v3 then a bulk `users_v2.getMultiple` to
hydrate rows [SOURCE apps/app-frontend/src/components/ui/settings/account/SocialSettings.vue:37];
"unblock" calls `unblock_user` and optimistically removes the row.
**Controls:** two columns, "User" and "Actions"; one action per row, "unblock".
Empty state: "You haven't blocked anyone." Scroll-fade affordances on a
max-height list.
**Default:** empty.
**Persistence:** account-side; nothing local.
**Rationale & notes:** The list explains the three consequences of blocking with
a bulleted list — send friend requests, invite to shared instances, invite to
manage hosting [SOURCE packages/ui/src/layouts/wrapped/settings/AccountSocialSettings.vue:93]
— which is far better than "blocked users" with no explanation. There is **no way
to block from this screen**; blocking happens elsewhere in the app, so the tab is
asymmetric. Worth adding.

## SET-31 — Manage preferences (ads consent)
**Type:** button
**Section:** Account ▸ Privacy, first block, heading "Your privacy and how ads
support Modrinth"
[SOURCE apps/app-frontend/src/components/ui/settings/account/PrivacySettings.vue:76]
**Nav path:** left rail gear → Privacy
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/account/PrivacySettings.vue:56]
**Purpose:** Open the third-party ad-consent UI, for users who want to change
partner-level cookie choices rather than just toggle telemetry.
**Operation:** `open_ads_consent_preferences()` opens a **separate native webview
window** [SOURCE apps/app-frontend/src/helpers/ads.js].
**Controls:** one button, "Manage preferences", preceded by the paragraph "Ads
make Modrinth possible and fund creator payouts. Our partners may store or
access cookies in the app to personalize ads and measure performance. You can opt
out or manage your preferences below."
**Default:** n/a.
**Persistence:** third-party consent store, not Modrinth's.
**Rationale & notes:** The block is **conditionally rendered**: only when
`adConsentAvailable`, which is `credentials !== undefined && !hasPlus`
[SOURCE apps/app-frontend/src/App.vue:372] — so signed-out users and Modrinth Plus
subscribers never see it at all. That is correct (Plus subscribers see no ads)
but it means the Privacy tab looks materially different across accounts with no
explanation. This is a **1×1 native popup window**, not an in-app panel: a real
reimplementation should put the consent choices in the tab itself.

## SET-32 — Telemetry
**Type:** toggle
**Section:** Account ▸ Privacy, second row
[SOURCE apps/app-frontend/src/components/ui/settings/account/PrivacySettings.vue:91]
**Nav path:** left rail gear → Privacy
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/account/PrivacySettings.vue:36]
[SOURCE apps/app-frontend/src/helpers/analytics.ts:98]
**Purpose:** The opt-out the law requires and most users never find. Reached for
once, by a privacy-minded minority.
**Operation:** Writes on change (no save button) and immediately calls
`optInAnalytics()` / `optOutAnalytics()`
[SOURCE apps/app-frontend/src/components/ui/settings/account/PrivacySettings.vue:60].
Opt-in loads `posthog-js` from `https://posthog.modrinth.com` and queues up to
100 events before activation; opt-out calls `posthog.opt_out_capturing()`
[SOURCE apps/app-frontend/src/helpers/analytics.ts:98].
**Controls:** on/off. The `id` attribute is **`opt-out-analytics`** while the
label is positive ("Telemetry") — inverted ids again, like SET-16.
**Default:** **`true`** [SOURCE packages/app-lib/migrations/20240711194701_init.sql:13].
**Persistence:** `settings.telemetry`. Device-local, not synced.
**Rationale & notes:** Three real problems. (1) **Default-on telemetry** in an
app that also sells Plus, with the toggle on a tab three clicks deep — that is
the pattern most privacy regulation is written against. (2) The label says
"Telemetry" but the description says "collects anonymized analytics **and usage
data to … customize your experience**", which is a materially different claim:
personalisation is not anonymous analytics, and the label does not carry it.
(3) Opting out stops new capture but nothing here clears the already-queued
buffer or the `localStorage` persistence PostHog was initialised with. Also note
there is a **second, separate** telemetry switch inside Minecraft itself
(`extra_telemetry`, `telemetryOptInExtra` in the game-options catalogue) with no
relationship to this one.

## SET-33 — Discord Rich Presence
**Type:** toggle
**Section:** Account ▸ Privacy, third row
[SOURCE apps/app-frontend/src/components/ui/settings/account/PrivacySettings.vue:103]
**Nav path:** left rail gear → Privacy
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/account/PrivacySettings.vue:45]
[SOURCE packages/app-lib/src/state/discord.rs:52]
**Purpose:** Stops the launcher advertising what you are playing to everyone on
your Discord friends list. Privacy-relevant, correctly filed.
**Operation:** Toggled **live** in Rust: every `set_activity` re-reads the setting
and calls `clear_activity` when it is off
[SOURCE packages/app-lib/src/state/discord.rs:52]. The description nonetheless
says "Requires an app restart."
**Controls:** on/off, description "Show Modrinth App as your current activity on
Discord. This does not affect Rich Presence added to instances by mods. Requires
an app restart."
**Default:** **`true`** [SOURCE packages/app-lib/migrations/20240711194701_init.sql:14].
**Persistence:** `settings.discord_rpc`. Device-local.
**Rationale & notes:** **Default-on**, for something that broadcasts your activity
to a third party. That is the wrong default for this class of setting, full stop.
Second, the description is *wrong*: it promises a restart that the code does not
need — the Rust path re-reads the flag on every activity update
[SOURCE packages/app-lib/src/state/discord.rs:131], so turning it off takes effect
immediately. Telling users to restart for a setting that already applied is the
kind of small untruth that teaches people to distrust the rest of the text. Third,
the `id` is `disable-discord-rpc` while the label is positive. The disambiguating
sentence about mod-added Rich Presence is, however, exactly right and worth
keeping.

## SET-34 — Sync game options
**Type:** toggle (+ a sync-source chooser on enable, + an edit button)
**Section:** Instances ▸ Synced settings, first row
[SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/index.vue:191]
**Nav path:** left rail gear → Synced settings
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/index.vue:65]
[SOURCE packages/app-lib/migrations/20260827120000_game-options-sync.sql:1]
**Purpose:** Lets one set of Minecraft video/audio/control/chat options apply to
every instance, instead of re-tuning each. This is the app's answer to
"settings.txt is per-instance and I have nine instances". Reached for as soon as
someone has more than one instance.
**Operation:** Turning it on opens a **source-instance chooser** — which
instance's options should be the seed
[SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/index.vue:467].
If only one eligible instance exists it applies silently without a dialog. The
edit button opens the game-settings modal (SET-40).
**Controls:** on/off. Edit button tooltip "Enable game settings sync before
choosing individual settings." when off
[SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/index.vue:77].
**Default:** `globally_enabled = 0` on upgrade; the migration raises it to `1`
**only if there are no instances at all** [SOURCE packages/app-lib/migrations/20260902120000_enable-synced-options-for-fresh-installs.sql:1]
— i.e. `true` on a fresh install, `false` on every upgrade.
**Persistence:** `sync_feature_settings.game_options`, plus per-instance opt-outs in
`instance_sync_preferences` and values in `synced_game_option_values`
[SOURCE packages/app-lib/migrations/20260827120000_game-options-sync.sql:28].
**Rationale & notes:** The migration's own comment explains the model — "The user
must choose which instance supplies the initial values before it can be enabled"
[SOURCE packages/app-lib/migrations/20260827120000_game-options-sync.sql:1] — and
the UI enforces it. The fresh-install-vs-upgrade default split is a genuinely
thoughtful migration decision: new users get the modern model, existing users keep
what they had. The "source instance" idea is the good part; it is the same idea as
a workspace seed and it generalises.

## SET-35 — Sync multiplayer servers
**Type:** toggle (+ source chooser + edit button)
**Section:** Instances ▸ Synced settings, second row
[SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/index.vue:197]
**Nav path:** left rail gear → Synced settings
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/index.vue:85]
[SOURCE packages/app-lib/migrations/20260819120000_instance-synced-options.sql:22]
**Purpose:** One multiplayer server list across all instances, so you do not
re-add your friends' servers per pack. Reached for by multiplayer players.
**Operation:** Source chooser on enable; edit opens a servers modal listing
name, address, status, players online, with copy-address
[SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/index.vue:85].
Only instances that can supply a server list are eligible
[SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/index.vue:336].
**Controls:** on/off + edit.
**Default:** `true` fresh / `false` upgraded (same fresh-install rule as SET-34).
**Persistence:** `sync_feature_settings.multiplayer_servers`; data in
`synced_servers`, per-instance local copies in `instance_servers`
[SOURCE packages/app-lib/migrations/20260819120000_instance-synced-options.sql:67].
**Rationale & notes:** The "you can edit them from any instance afterward"
sentence [SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/index.vue:113]
is the right reassurance at the moment of the choice. The per-instance "desynced"
model (a local copy replacing the shared entry, tracked by
`excluded_synced_server_id`) is more machinery than a user expects and the UI
does not explain that desyncing one server in one instance is possible.

## SET-36 — Sync resource packs
**Type:** toggle (+ source chooser + edit button)
**Section:** Instances ▸ Synced settings, third row
[SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/index.vue:203]
**Nav path:** left rail gear → Synced settings
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/index.vue:48]
[SOURCE packages/app-lib/migrations/20260903120000_synced-packs.sql:1]
**Purpose:** One resource-pack list across instances — shaders, HUD packs, icon
packs — instead of per-pack installs. Reached for by anyone who uses one shader
everywhere.
**Operation:** Source chooser on enable; edit opens the synced-packs modal. The
edit button is enabled as soon as the toggle is on, with no "nothing synced yet"
gate for this and SET-37/38 [SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/index.vue:372].
**Controls:** on/off + edit.
**Default:** **`false`** — added by the last synced-packs migration with
`globally_enabled = 0`, and unlike SET-34/35/37/38 that migration ran *after* the
fresh-install migration, so fresh installs get `false` too
[SOURCE packages/app-lib/migrations/20260903120000_synced-packs.sql:1].
**Persistence:** `sync_feature_settings.resource_packs`.
**Rationale & notes:** Newer feature, default off, same pattern — consistent with
the maturity rule seen in SET-8. The ordering inconsistency is real though: the
row is third in the UI but the migration is the newest, so the UI order is
"game options, servers, resource packs, command history, hotbars", which is
neither alphabetical nor chronological.

## SET-37 — Sync command history
**Type:** toggle (+ source chooser + edit button)
**Section:** Instances ▸ Synced settings, fourth row
[SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/index.vue:215]
**Nav path:** left rail gear → Synced settings
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/index.vue:93]
[SOURCE packages/app-lib/migrations/20260819120000_instance-synced-options.sql:23]
**Purpose:** Carry your `/say`, `/tp`, macro history between instances.
Reached for by technical players.
**Operation:** Source chooser on enable; edit opens a command-history modal.
**Controls:** on/off + edit. No description is shown.
**Default:** `true` fresh / `false` upgraded.
**Persistence:** `sync_feature_settings.command_history`.
**Rationale & notes:** This row has **no description text at all**
[SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/index.vue:101]
while its neighbours do — the row reads as unexplained. The edit button is
disabled with "Choose a sync source before editing this setting." when the
option was never initialised, which is correct but the wording implies the user
did something wrong when they did nothing at all.

## SET-38 — Sync saved creative hotbars
**Type:** toggle (+ source chooser)
**Section:** Instances ▸ Synced settings, fifth row
[SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/index.vue:221]
**Nav path:** left rail gear → Synced settings
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/index.vue:101]
[SOURCE packages/app-lib/migrations/20260819120000_instance-synced-options.sql:24]
**Purpose:** Creative-mode hotbar layouts shared across instances. Reached for
only by creative players — a textbook narrow-audience setting.
**Operation:** Source chooser on enable. **No edit button** — the row is the only
one of the six without one.
**Controls:** on/off, with description "Use the same saved creative hotbars across
your instances."
**Default:** `true` fresh / `false` upgraded.
**Persistence:** `sync_feature_settings.creative_hotbars`; values in
`synced_hotbar_state` [SOURCE packages/app-lib/migrations/20260819120000_instance-synced-options.sql:69].
**Rationale & notes:** The absence of an edit button is inconsistent with
SET-35/36/37 which all have one, and it is a genuine usability gap: a user who
syncs hotbars can only edit them from inside an instance. The underlying model is
nice — `instance_sync_checkpoints.variant` distinguishes `default`, `legacy` and
`components` layouts and stores a merge base, so hotbar sync is a three-way merge
rather than a copy.

## SET-39 — Sync data packs (row defined but not rendered)
**Type:** toggle — **not shown**
**Section:** Instances ▸ Synced settings — **the row does not appear**
**Nav path:** none; unreachable at v0.21.9
**Evidence:** [SOURCE apps/app-frontend/src/helpers/instance.ts:304]
**Purpose:** Intended to share datapacks across instances; gated off in this tag.
The label exists — `app.settings.synced-options.data-packs` → "Sync data packs"
[SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/index.vue:56].
**Operation:** `isSyncedOptionAvailable` returns `false` for `data_packs`
[SOURCE apps/app-frontend/src/helpers/instance.ts:304] and the tab filters on it
[SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/index.vue:228].
The Rust side still has the row and the tables.
**Controls:** n/a.
**Default:** `globally_enabled = 0` [SOURCE packages/app-lib/migrations/20260903120000_synced-packs.sql:3].
**Persistence:** `sync_feature_settings.data_packs`.
**Rationale & notes:** The launch condition, its translations and its database
row are all shipped; only one `!==` in a TypeScript helper hides it. This is the
single cleanest example of the flag ladder's terminal state in this codebase —
it should have been deleted rather than hidden. If our app gates anything the
same way, we should either delete it or give it a real "coming soon" affordance.

## SET-40 — Edit game settings
**Type:** button → modal (11 categories, ~280 individual settings)
**Section:** Instances ▸ Synced settings, beside the SET-34 toggle
[SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/index.vue:602]
**Nav path:** left rail gear → Synced settings → pencil icon on the "Sync game
options" row
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/game-settings-modal/index.vue:67]
[SOURCE packages/app-lib/src/api/instance/synced_options/game_options/catalog/mod.rs:161]
**Purpose:** The bulk editor for Minecraft's own `options.txt`, per setting, with
per-setting sync control. This is the feature that makes SET-34 worth turning on.
**Operation:** Opens a `TabbedModal` titled "Game settings". Categories:
**Video**, **Music and sound**, **Controls**, **Chat**, **Accessibility**,
**Skin customization**, **Online**, **Language**, and **Custom settings** for
keys discovered in an instance that Modrinth does not ship a label for
[SOURCE apps/app-frontend/src/components/ui/settings/instances/game-settings-modal/index.vue:150].
Each row has its own sync toggle; "Sync all" / "Unsync all" act per category. The
modal header states the override rule: "Synced options are applied to every
instance and override the values included with a modpack." Search across all
categories; keybind conflict detection across the whole set; a floating action bar
for unsaved changes.
**Controls:** per-setting editors are declared in Rust, not the frontend —
`Boolean`, `UnboundedInteger`, `Integer{min,max,step}`, `UnboundedDecimal`,
`Decimal{min,max,step,unit}`, `Enum(&[&str])`, `Language`, `KeyBinding`
[SOURCE packages/app-lib/src/api/instance/synced_options/game_options/catalog/mod.rs:26].
Each carries a `since`/`until` Minecraft version range, so a setting not present
in a given version's `options.txt` is hidden
[SOURCE packages/app-lib/src/api/instance/synced_options/game_options/catalog/mod.rs:353].
Examples of real entries: `fullscreen` (bool), `gui_scale` (integer),
`graphics` (enum `fancy|fabulous|fast`), `render_distance`, `fov`,
`vsync`, `extra_telemetry`, `share_presence` (enum `all|limited|none`).
**Per-row default sync state is declared per setting** (`default_on` in the Rust
catalogue) — for example `main_hand` and the skin-layer toggles ship `true`
[SOURCE packages/app-lib/src/api/instance/synced_options/game_options/catalog/mod.rs:175].
**Default:** the modal's own default is `enabled` per setting from the catalogue,
not a single global value.
**Persistence:** `synced_game_option_values` (one row per `option_id`, with
`kind = 'vanilla' | 'external'`, codec, revision, provenance) and
`synced_game_option_preferences` (which settings sync, kept separately so
"turning a setting off does not forget its last value")
[SOURCE packages/app-lib/migrations/20260827120000_game-options-sync.sql:32],
[SOURCE packages/app-lib/migrations/20260827120000_game-options-sync.sql:38].
Values are written back into each participating instance's `options.txt`.
**Rationale & notes:** The migration comments are the design, in their own words:
"Stores which settings the user chose to sync. This is separate from the values
so turning a setting off does not forget its last value." That is correct and it
is the kind of comment worth copying. The strongest idea here is **external
keys**: Modrinth discovers settings added by mods in your instances, labels them
if it can find the owning project, and syncs them anyway
[SOURCE apps/app-frontend/src/components/ui/settings/instances/game-settings-modal/index.vue:92].
Almost no launcher does that. The weakness is scale — ~280 settings with a search
box is the only way anyone can navigate it, and the category set mixes
Minecraft's own grouping ("Video", "Chat") with app-specific ones ("Custom
settings"). **This is a whole subsystem and is out of scope for this module**; it
belongs in its own page. The app-level coupling to note: when game-options sync is
enabled, SET-41 `force_fullscreen` is no longer a local setting at all — the Rust
`settings::get`/`set` pair redirects it into the shared value and pushes the change
to every participating instance
[SOURCE packages/app-lib/src/api/settings.rs:68],
[SOURCE packages/app-lib/src/api/settings.rs:94]. One boolean, two stores,
transparent to the user.

## SET-41 — Fullscreen
**Type:** toggle
**Section:** Instances ▸ Synced settings ▸ "Window", first row
[SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/launch-options.vue:258]
**Nav path:** left rail gear → Synced settings (scroll past the sync rows)
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/launch-options.vue:120]
**Purpose:** Start every instance fullscreen. Reached for by people who always
play fullscreen and are tired of editing `options.txt`.
**Operation:** Written to `options.txt`, per the description "Start instances in
fullscreen by updating their options.txt file" — not as a JVM argument. **If
SET-34 (game-options sync) is on, this setting is redirected into the shared
game-options store** and pushed to all instances
[SOURCE packages/app-lib/src/api/settings.rs:68].
Enabling it disables SET-42 and SET-43 (`:disabled="settings.force_fullscreen"`).
**Controls:** on/off.
**Default:** `false` [SOURCE packages/app-lib/migrations/20240711194701_init.sql:24].
This tab writes on **every keystroke/change** with no save button
[SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/launch-options.vue:86].
**Persistence:** `settings.mc_force_fullscreen`.
**Rationale & notes:** The section is titled "Window" but only contains
fullscreen and resolution — nothing about windowing, position, or the launcher
window itself. The silent redirect when game-options sync is on means toggling it
here can rewrite every instance's `options.txt` without saying so. **That is the
single most surprising behaviour in the entire settings surface and it must be
surfaced in the UI, not hidden in Rust.**

## SET-42 — Width
**Type:** number
**Section:** Instances ▸ Synced settings ▸ "Window"
[SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/launch-options.vue:276]
**Nav path:** as SET-41
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/launch-options.vue:128]
**Purpose:** The game window's initial width. Reached for by anyone with an odd
monitor arrangement.
**Operation:** Guarded — the save watcher refuses to write until **both**
dimensions are integers greater than 0
[SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/launch-options.vue:90].
**Controls:** `type="number"`, placeholder "Enter width...". Disabled when
SET-41 is on. **No min/max/step declared.** Stored as `u16`, so a value above
65535 fails serialisation in Rust.
**Default:** **`854`** [SOURCE packages/app-lib/migrations/20240711194701_init.sql:25]
— Minecraft's own default window width, chosen deliberately.
**Persistence:** `settings.mc_game_resolution_x`.
**Rationale & notes:** Good: the "both dimensions valid or write nothing" guard
prevents a half-typed resolution from being persisted. Bad: no bounds are
declared in the UI even though the type is `u16`, so an absurd value is accepted
by the input and rejected (or truncated) by the backend. A `min`/`max` on the
`Input` costs nothing.

## SET-43 — Height
**Type:** number
**Section:** Instances ▸ Synced settings ▸ "Window"
**Nav path:** as SET-41
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/launch-options.vue:297]
**Purpose:** As SET-42.
**Controls:** as SET-42, placeholder "Enter height..."; disabled when SET-41 is on.
**Default:** **`480`** [SOURCE packages/app-lib/migrations/20240711194701_init.sql:26].
**Persistence:** `settings.mc_game_resolution_y`.
**Rationale & notes:** Same as SET-42. The pair is the only place in the app
where two settings share one validation rule implemented in a `watch` rather than
in the control.

## SET-44 — Memory allocation
**Type:** number (slider + numeric input, snap points)
**Section:** Instances ▸ Synced settings ▸ "Java and memory", first row
[SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/launch-options.vue:325]
**Nav path:** as SET-41
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/launch-options.vue:152]
[SOURCE packages/app-lib/src/api/jre.rs:401]
[SOURCE packages/app-lib/src/state/settings.rs:419]
**Purpose:** The heap ceiling for **each** instance. The most consequential
performance setting in the app: too low and the game crashes with a heap error,
too high and several instances starve the OS. Reached for by anyone running two
instances at once, and by anyone who has read a Forge memory guide.
**Operation:** Written as `settings.memory.maximum` (MB). Also the per-instance
override target.
**Controls:** **min 512 MB, max = total system RAM in MB, step 64 MB**, snap
points at every power-of-two multiple of 2048 up to the max, snap range 512
[SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/launch-options.vue:329],
[SOURCE apps/app-frontend/src/composables/useMemorySlider.js]. Labels: "512 MB"
and "{N} GB". Unit MB. If the RAM query errors, a **Refresh** button replaces the
slider entirely.
**Default:** the column default is `2048` MB
[SOURCE packages/app-lib/migrations/20240711194701_init.sql:23], but settings
migration v2→v3 rewrites **exactly 2048** to a RAM-derived value:
`default_memory_max_mb()` = 2 GB if the machine has < 8 GiB, **4 GB** if
8–23 GiB, **6 GB** if ≥ 24 GiB [SOURCE packages/app-lib/src/api/jre.rs:401],
[SOURCE packages/app-lib/src/state/settings.rs:419]. The migration runs in the
background at startup [SOURCE packages/app-lib/src/state/mod.rs:276].
So a **fresh install** ends up at 2/4/6 GB depending on the machine; a user who
deliberately set 2048 MB gets it changed out from under them, once.
**Persistence:** `settings.mc_memory_max` (INTEGER MB).
**Rationale & notes:** Three things to copy. The **snap points** are the
best-in-class detail in this whole document: raw linear MB is unusable on a
64 GB machine, and marking every power of two lets a user hit their target in one
gesture. The **RAM-derived default** is right in spirit. The **"exactly 2048"
heuristic** is the bug: it cannot distinguish "never set" from "deliberately set
2 GB", so a user who chose 2 GB on a 32 GB machine silently gets 6 GB. A nullable
column plus a real NULL check is the correct shape. Finally, the label "Maximum
memory available to each instance" is precise about the per-instance part,
which most launchers are not.

## SET-45 — Java arguments
**Type:** text
**Section:** Instances ▸ Synced settings ▸ "Java and memory"
[SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/launch-options.vue:357]
**Nav path:** as SET-41
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/launch-options.vue:160]
**Purpose:** Append flags such as `-XX:+UseG1GC` or `-Dfml.ignoreInvalidMinecraftCertificates=true`.
Reached for by modpack authors and by anyone who has read a JVM guide. This is the
most expert-facing field in the settings surface.
**Operation:** Stored as a JSON array; the UI holds a single space-joined string
and splits on `/\s+/` **after trimming, discarding empties**
[SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/launch-options.vue:98].
**Controls:** one free-text field, placeholder "Enter Java arguments...".
**No validation at all** — no flag whitelist, no quoting, no per-argument parse.
A value containing a quoted string with spaces will be split incorrectly and
there is no warning.
**Default:** `[]` (empty JSON array seeded by the init migration
[SOURCE packages/app-lib/migrations/20240711194701_init.sql:41]).
**Persistence:** `settings.extra_launch_args` JSONB.
**Rationale & notes:** Splitting on whitespace with no quoting support means
`-Dsome.prop="a b"` becomes two arguments. The description says only "Arguments
passed to Java when launching an instance" and does not warn that **these are
appended to, not replacing, Modrinth's own required arguments** — a user who puts
`-Xmx8G` here will not get 8 GB (SET-44 wins) and will not be told why. A chip-based
editor that shows the parsed result back would make this field safe.

## SET-46 — Environment variables
**Type:** text
**Section:** Instances ▸ Synced settings ▸ "Java and memory"
[SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/launch-options.vue:375]
**Nav path:** as SET-41
**Evidence:** [SOURCE apps/app-frontend/src/helpers/settings.ts:113]
[SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/launch-options.vue:172]
**Purpose:** Per-instance env vars for the launched process — `JAVA_TOOL_OPTIONS`,
`PLAY_*` auth variables, a mod's expected token. Reached for by server-jar and
modpack users.
**Operation:** Serialised as `KEY=VALUE` pairs joined by spaces
[SOURCE apps/app-frontend/src/helpers/settings.ts:109] and parsed by splitting on
whitespace then taking the **first** `=` as the separator; entries with no `=` or
with `=` at position 0 are silently dropped
[SOURCE apps/app-frontend/src/helpers/settings.ts:113].
**Controls:** one free-text field, placeholder "Enter environment variables...".
No per-pair validation, no duplicate-key detection.
**Default:** `[]` [SOURCE packages/app-lib/migrations/20240711194701_init.sql:41].
**Persistence:** `settings.custom_env_vars` JSONB, an array of `[string, string]`.
**Rationale & notes:** **A value containing a space is silently corrupted**:
`KEY=a b` round-trips as `KEY=a` plus a dropped `b`. This is a genuine data-loss
bug in the round-trip, not just a missing validation. A key/value list editor
would be both safer and quicker to use than a string.

## SET-47 — Pre-launch hook
**Type:** text
**Section:** Instances ▸ Synced settings ▸ "Launch hooks", first row
[SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/launch-options.vue:400]
**Nav path:** as SET-41
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/launch-options.vue:184]
[SOURCE packages/app-lib/src/state/settings.rs:487]
**Purpose:** Run a shell command before the game starts — a Discord webhook, a
RAM-cleaning script, a VPN toggle. Reached for by a small, technically confident
minority.
**Operation:** Shell-split with `shlex` and run in the instance's working
directory. A parse failure surfaces as
`"Invalid post-exit command: {cmd}"` / `"Invalid pre-launch command"`-style
errors [SOURCE packages/app-lib/src/state/process.rs:1005].
**Controls:** free text, placeholder "Enter pre-launch command...". Six
substitution variables are documented **in the settings UI itself**:
`$INST_NAME`, `$INST_ID`, `$INST_DIR`, `$INST_MC_DIR` (an alias for `$INST_DIR`),
`$INST_JAVA`, `$INST_JAVA_ARGS`
[SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/launch-options.vue:220].
**Default:** `null`, stored as an empty string via
`#[serde_as(as = "serde_with::NoneAsEmptyString")]`
[SOURCE packages/app-lib/src/state/settings.rs:488].
**Persistence:** `settings.hook_pre_launch` TEXT NULL.
**Rationale & notes:** Documenting the six variables inline, right under the field,
is genuinely excellent and very few apps do it. The serialisation choice
(`NoneAsEmptyString`) is a migration-safety hack so old empty-string rows
round-trip; it is correct but surprising. Settings migration v1→v2 **re-quotes**
old space-split hook strings with `shlex::Quoter`
[SOURCE packages/app-lib/src/state/settings.rs:395], which is the right way to
handle a format change.

## SET-48 — Wrapper hook
**Type:** text
**Section:** Instances ▸ Synced settings ▸ "Launch hooks", second row
[SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/launch-options.vue:418]
**Nav path:** as SET-41
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/launch-options.vue:196]
**Purpose:** Interpose a command around the whole launch — a profiler, a
container wrapper, a JVM shim. Narrow but real.
**Operation:** The command becomes the launcher, with the real invocation passed
to it. Migration v1→v2 **fully quotes** the old bare-path value
[SOURCE packages/app-lib/src/state/settings.rs:405].
**Controls:** free text, placeholder "Enter wrapper command...". Same six
variables listed once, at the bottom of the section.
**Default:** `null`.
**Persistence:** `settings.hook_wrapper` TEXT NULL.
**Rationale & notes:** Of the three hooks this is the only one whose semantics
are genuinely unclear from the UI — "Command used to wrap the Minecraft launch
process" does not say **what the wrapper receives** (argv? env? stdin?) or
**whether it is waited on**. A user cannot use this correctly without reading
the source. This is the weakest-documented field in the app.

## SET-49 — Post-exit hook
**Type:** text
**Section:** Instances ▸ Synced settings ▸ "Launch hooks", third row
[SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/launch-options.vue:436]
**Nav path:** as SET-41
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/launch-options.vue:208]
[SOURCE packages/app-lib/src/state/process.rs:1004]
**Purpose:** Run something when the game exits — upload a log, post a Discord
message. Reached for by the same minority as SET-47.
**Operation:** Fire-and-forget: the app explicitly does **not** wait for it
[SOURCE packages/app-lib/src/state/process.rs:1004]. The exit status is appended
to the instance's log file first.
**Controls:** free text, placeholder "Enter post-exit command...". Same
variables.
**Default:** `null`.
**Persistence:** `settings.hook_post_exit` TEXT NULL.
**Rationale & notes:** Because it is not awaited, a hook that fails is invisible —
no exit code, no notification. "Runs after the game closes" is accurate but does
not promise delivery, so a user building automation on it will be disappointed.
Saying "runs in the background; failures are not reported" would be honest and
still short.

## SET-50 — Java 25 location
**Type:** text + 3 buttons (test status, install, detect, browse)
**Section:** Instances ▸ Java installations, first block
[SOURCE apps/app-frontend/src/components/ui/settings/instances/JavaSettings.vue:31]
**Nav path:** left rail gear → Java installations
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/JavaSettings.vue:34]
[SOURCE apps/app-frontend/src/components/ui/JavaSelector.vue:20]
[SOURCE packages/app-lib/src/state/java_globals.rs:5]
**Purpose:** Tell the app which Java 25 to use. Reached for by anyone whose
default `java` on PATH is the wrong major version — extremely common on Linux
and on Windows with an Adoptium-first PATH.
**Operation:** Four buttons per row. A **status icon button** that runs
`java -version` and shows green ✓ / red ✗ / a refresh glyph
[SOURCE apps/app-frontend/src/components/ui/JavaSelector.vue:20]. **"Install
recommended"** downloads and verifies a Temurin-style build from Adoptium
[SOURCE apps/app-frontend/src/components/ui/settings/instances/JavaSettings.vue:8]
and becomes **"Already installed"** when the path is set. **"Detect"** opens a
Java-detection modal listing candidates found on the system. **"Browse"** opens a
native file picker and probes the chosen binary
[SOURCE apps/app-frontend/src/components/ui/JavaSelector.vue:210].
On save the path is rewritten `java.exe` → `javaw.exe` on Windows
[SOURCE apps/app-frontend/src/components/ui/settings/instances/JavaSettings.vue:24],
and an empty string becomes `null` (= "use the default").
This tab also **writes on change** with no save button.
**Controls:** path placeholder `/path/to/java`. Exactly four major versions are
offered, hard-coded: **25, 21, 17, 8**
[SOURCE apps/app-frontend/src/components/ui/settings/instances/JavaSettings.vue:34].
The heading is the ICU-parameterised
`app.settings.java-installations.location.title` → "Java {version} location".
**Default:** unset. The `java_versions` table ships empty
[SOURCE packages/app-lib/migrations/20240711194701_init.sql:43]; a row is
filtered out entirely if its path is not a file
[SOURCE packages/app-lib/src/state/java_globals.rs:33].
**Persistence:** `java_versions` table, PK `major_version`, one row per major
version [SOURCE packages/app-lib/migrations/20240711194701_init.sql:43]. **Not
**in the `settings` table, and no account sync.
**Rationale & notes:** The live test button with a colour-coded result is the right
affordance — it turns "did I get the path right?" from a guess into an
observation. The manual Detect flow is a modal I cannot fully characterise from
source. `[ORCH-PENDING]`: the orchestrator should open Java installations on a
machine with **no** JREs registered and capture the four empty rows (placeholder
text, which buttons are enabled, the status icon state) — that is the first-run
state and it is not derivable from reading the component.

## SET-51 — Java 21 location
**Type:** text + buttons — as SET-50
**Section:** Instances ▸ Java installations, second block
[SOURCE apps/app-frontend/src/components/ui/settings/instances/JavaSettings.vue:37]
**Nav path:** as SET-50
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/JavaSettings.vue:34]
**Purpose:** As SET-50; Java 21 is the common target for modern 1.20.5+/1.21
modpacks and is probably the single most-used row here.
**Controls:** heading renders "Java 21 location". Same buttons as SET-50.
**Default:** unset.
**Persistence:** `java_versions` row `major_version = 21`.
**Rationale & notes:** Four hard-coded versions and no "other"/"custom" escape is
the design's main weakness — it will need a code change to add Java 26. A
free-text major-version field, or a dynamically-populated list, would age better.

## SET-52 — Java 17 location
**Type:** text + buttons — as SET-50
**Section:** Instances ▸ Java installations, third block
[SOURCE apps/app-frontend/src/components/ui/settings/instances/JavaSettings.vue:37]
**Nav path:** as SET-50
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/JavaSettings.vue:34]
**Purpose:** As SET-50. Java 17 is the floor for 1.18–1.20 packs.
**Controls:** "Java 17 location". Same buttons.
**Default:** unset.
**Persistence:** `java_versions` row `major_version = 17`.
**Rationale & notes:** As SET-50/SET-51.

## SET-53 — Java 8 location
**Type:** text + buttons — as SET-50
**Section:** Instances ▸ Java installations, fourth block
[SOURCE apps/app-frontend/src/components/ui/settings/instances/JavaSettings.vue:37]
**Nav path:** as SET-50
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/JavaSettings.vue:34]
**Purpose:** As SET-50. Java 8 for 1.8–1.16 Forge packs. This block exists purely
for the legacy tail and is the clearest example of an edge case getting
first-class UI.
**Controls:** "Java 8 location". Same buttons.
**Default:** unset.
**Persistence:** `java_versions` row `major_version = 8`.
**Rationale & notes:** Having four explicit blocks rather than one block with a
version dropdown costs vertical space but removes a click from the most common
path, which is the right trade for a settings list people visit once.

## SET-54 — Content storage
**Type:** info (readout + bar)
**Section:** Instances ▸ Resource management, first block
[SOURCE apps/app-frontend/src/components/ui/settings/instances/ContentStorageSettings.vue:265]
**Nav path:** left rail gear → Resource management
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/ContentStorageSettings.vue:26]
[SOURCE packages/app-lib/src/state/content_store/commands/cache.rs:132]
**Purpose:** Explain where the disk is going. The shared content store is the
app's largest architectural change in this version, and this readout is the only
place its behaviour is visible. Reached for when the disk fills.
**Operation:** Calls `plugin:settings|store_usage` and **artificially delays 1 s**
so the loading state is visible
[SOURCE apps/app-frontend/src/components/ui/settings/instances/ContentStorageSettings.vue:49].
**Controls:** none. Displays total stored size, a three-segment proportional bar,
and three figures: **"Unique"**, **"Shared"**, **"Orphaned"**
[SOURCE apps/app-frontend/src/components/ui/settings/instances/ContentStorageSettings.vue:105],
[SOURCE apps/app-frontend/src/components/ui/settings/instances/ContentStorageSettings.vue:112],
[SOURCE apps/app-frontend/src/components/ui/settings/instances/ContentStorageSettings.vue:117].
Empty state: "Install mods or packs to see your storage usage here."
**Default:** n/a.
**Persistence:** none (read-only).
**Rationale & notes:** "Orphaned" is a good plain-English name for "cache entries
no instance references". Showing three buckets rather than one total is exactly
right — it teaches the deduplication model without a word of explanation. The
`Math.max(0, total - unused - shared)` derivation of the Unique bucket
[SOURCE apps/app-frontend/src/components/ui/settings/instances/ContentStorageSettings.vue:200]
means Unique can never go negative, which is deliberate but means the three
figures need not sum to the total if the backend ever reports an inconsistency.

## SET-55 — Keep unused downloads
**Type:** number (GiB, step 0.5)
**Section:** Instances ▸ Resource management, inside the Content storage block,
below a rule
[SOURCE apps/app-frontend/src/components/ui/settings/instances/ContentStorageSettings.vue:400]
**Nav path:** as SET-54
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/ContentStorageSettings.vue:241]
[SOURCE packages/app-lib/src/state/content_store/commands/cache.rs:136]
**Purpose:** Set the ceiling on how much unreferenced content the store keeps
"just in case", trading disk for future install speed. Reached for by users with
fast disks installing many packs, and by users near a disk limit.
**Operation:** `plugin:settings|store_set_cache_limit` with `bytes = round(GiB ×
2³⁰)` [SOURCE apps/app-frontend/src/components/ui/settings/instances/ContentStorageSettings.vue:254].
Fires on `change` (blur/Enter), not on input.
**Controls:** `type="number"`, **min 0, step 0.5**, unit suffix "GiB"
[SOURCE apps/app-frontend/src/components/ui/settings/instances/ContentStorageSettings.vue:409].
Client-side validation rejects non-safe-integers and negatives and reverts the
field [SOURCE apps/app-frontend/src/components/ui/settings/instances/ContentStorageSettings.vue:254].
Server-side rejects anything above `i64::MAX` with "The shared content cache limit
is too large" [SOURCE packages/app-lib/src/state/content_store/commands/cache.rs:145].
**Default:** **`5_368_709_120` bytes = 5 GiB**, hard-coded in the fallback when no
row exists [SOURCE packages/app-lib/src/state/content_store/commands/cache.rs:141].
The frontend mirrors that same 5 GiB in two places as a fallback
[SOURCE apps/app-frontend/src/components/ui/settings/instances/ContentStorageSettings.vue:60].
**Reset:** none. There is no way to get back to the default once changed.
**Persistence:** `app_metadata` row `store_cache_limit_bytes` (TEXT), **not** the
`settings` table [SOURCE packages/app-lib/src/state/content_store/commands/cache.rs:151].
**Rationale & notes:** "Keep unused downloads" is a noun phrase doing a verb's job;
**"Cache limit: 5 GiB"** would be clearer. The duplicate hard-coded 5 GiB in
Rust and twice in the frontend is a real fragility — three places to update, and
a disagreement shows the wrong number before the query resolves. Read the
persisted value once and let the backend own the default.

## SET-56 — Clear unused
**Type:** button (destructive, red)
**Section:** Instances ▸ Resource management, Content storage block
[SOURCE apps/app-frontend/src/components/ui/settings/instances/ContentStorageSettings.vue:351]
**Nav path:** as SET-54
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/ContentStorageSettings.vue:218]
[SOURCE packages/app-lib/src/api/settings.rs:14]
**Purpose:** Reclaim disk from orphaned content immediately, rather than waiting
for the cache limit to be reached.
**Operation:** `plugin:settings|store_cleanup` returns the number of bytes freed,
reported inline: "Freed {size} of disk space." or "No unused content to clear."
**Controls:** disabled while busy, while the store is empty, or when
`unused_cache_bytes === 0` [SOURCE apps/app-frontend/src/components/ui/settings/instances/ContentStorageSettings.vue:355].
**Default:** n/a.
**Persistence:** none.
**Rationale & notes:** **No confirmation dialog.** Compare SET-60 ("Purge cache"),
which does confirm. Two destructive disk operations in the same tab with
opposite safety postures is a consistency bug. Also: when instances are running,
the button tooltip does not warn, unlike SET-57 which explicitly says "This will
close any running instances."

## SET-57 — Verify and repair
**Type:** button
**Section:** Instances ▸ Resource management, Content storage block
[SOURCE apps/app-frontend/src/components/ui/settings/instances/ContentStorageSettings.vue:331]
**Nav path:** as SET-54
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/ContentStorageSettings.vue:218]
[SOURCE packages/app-lib/src/api/settings.rs:32]
**Purpose:** The "something is wrong, fix it" button after a crash, a disk error,
or a partial download. Reached for when something is already broken.
**Operation:** Closes the settings modal, then runs a streaming verification that
reports progress over a Tauri IPC channel (throttled to ≥200 ms per update)
[SOURCE apps/app/src/api/settings.rs:35]. **If any instance is running, the
repair kills them all** and waits up to 10 s, erroring with "Timed out closing
running instances" [SOURCE packages/app-lib/src/api/settings.rs:37].
Results render as an Admonition: "Checked {n} file(s) and made {n} repair(s)",
with a collapsible list of issues
[SOURCE apps/app-frontend/src/components/ui/settings/instances/ContentStorageSettings.vue:182].
**Controls:** disabled while busy or when `object_count === 0`. **Changes to an
orange button with the tooltip "This will close any running instances"** when
processes are detected [SOURCE apps/app-frontend/src/components/ui/settings/instances/ContentStorageSettings.vue:332]
— an adaptive warning rather than a static one, which is good practice.
**Default:** n/a.
**Persistence:** writes repair state back into the content store.
**Rationale & notes:** **Killing the user's running games from a settings button**
is a genuinely dangerous action with no confirmation. The adaptive orange state
is the right mitigation and more apps should copy it; a confirm dialog naming
*which* instances will be closed is the missing half. Naming it "Verify and
repair" rather than "Fix" is right — the user can run it just to check.

## SET-58 — App directory
**Type:** text + browse button
**Section:** Instances ▸ Resource management, second block
[SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:204]
**Nav path:** left rail gear → Resource management
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:54]
[SOURCE packages/app-lib/src/state/dirs.rs:231]
**Purpose:** Move instances, downloads and the content store off the system
drive. Reached for by anyone with a small boot disk, by anyone moving to an
external drive, and by Linux users who want the data out of `~/.local/share`.
**Operation:** A native directory picker writes `custom_dir`. The actual move
happens **at next launch**, with a progress indicator titled "Moving launcher
directory", under a process-level lock on both roots
[SOURCE packages/app-lib/src/state/dirs.rs:285]. If the move fails,
`ErrorModal` offers `plugin:settings|cancel_directory_change`, which restores
`prev_custom_dir` as the new `custom_dir`
[SOURCE packages/app-lib/src/api/settings.rs:123].
**Controls:** free-text path with a folder-picker icon button, tooltip "Browse
for an app directory". **No validation** that the path is writable or has space.
An empty string is coerced to `null` before saving
[SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:139].
**Default:** `NULL` = the OS app-data directory
(`$XDG_DATA_HOME/<identifier>` or the platform equivalent, overridable with
`THESEUS_CONFIG_DIR`) [SOURCE packages/app-lib/src/state/dirs.rs:36].
**Persistence:** `settings.custom_dir` TEXT NULL, with `settings.prev_custom_dir`
used as the rollback pointer [SOURCE packages/app-lib/migrations/20240711194701_init.sql:34].
**Rationale & notes:** The split of `settings_dir` (where the database lives,
**always** in the OS app-data dir) from `config_dir` (instances, downloads,
store — moveable) is a good architectural decision, but the UI collapses them
into one field called "App directory" whose description says "Where Modrinth App
stores instances and other files", which does not tell the user that their
**settings and accounts will not move**. That is the single most likely source of
"where did my instances go" for a reimplementation. Be explicit about what moves
and what does not.

## SET-59 — Always show copy details
**Type:** toggle
**Section:** Instances ▸ Resource management, between App directory and App cache
[SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:231]
**Nav path:** as SET-58
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:110]
[SOURCE packages/app-lib/migrations/20260918120000_promote-behavior-settings.sql:19]
**Purpose:** Surface a "Copy details" action during a running install so a user
can paste a bug report without waiting for a failure. Reached for by pack
authors and bug reporters.
**Operation:** Writes on change. Deliberately updates **both** the reactive store
and the DB, because this tab's `watch` serialises a snapshot
[SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:240].
**Controls:** on/off, description "Show the Copy details action while an install
is queued or running. It is always available for failed or interrupted installs."
**Default:** `false` [SOURCE packages/app-lib/migrations/20260918120000_promote-behavior-settings.sql:20].
Previously the `always_show_copy_details` feature flag
[SOURCE packages/app-lib/migrations/20260918120000_promote-behavior-settings.sql:51].
**Persistence:** `settings.always_show_copy_details`.
**Rationale & notes:** The description is precise about the *state* the setting
affects ("while an install is queued or running") and about what is **not**
affected ("always available for failed or interrupted installs"), which is
exactly the information a user needs. This is a debugging affordance shipped in
the main settings tab rather than developer mode, which is a deliberate and
commendable choice — the users who need it are users, not staff.

## SET-60 — Purge cache
**Type:** button (destructive)
**Section:** Instances ▸ Resource management, App cache block
[SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:263]
**Nav path:** as SET-58
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:148]
[SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:171]
**Purpose:** Force-refresh all cached Modrinth metadata when the app is showing
stale or wrong project data. Reached for after a bad cache event, rarely.
**Operation:** Purges exactly 17 named cache types — `project`, `project_v3`,
`version`, `user`, `team`, `organization`, `file`, `loader_manifest`,
`minecraft_manifest`, `categories`, `report_types`, `loaders`, `game_versions`,
`donation_platforms`, `file_hash`, `file_update`, `search_results`,
`search_results_v3` [SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:148].
It does **not** touch the content store (SET-54) or the database.
**Controls:** a `ConfirmModalWrapper` titled "Purge the app cache?" with the
description "The app may load more slowly until the cache is rebuilt" — **but the
confirmation is skipped entirely when SET-21 is on**
[SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:171].
**Default:** n/a.
**Persistence:** clears rows; the setting itself is unchanged.
**Rationale & notes:** "Purge cache" is well-chosen wording — not "Clear all data",
which would frighten people. The confirmation's description is a cost statement
("may load more slowly") rather than a consequence statement; it does not say what
will be re-downloaded. The SET-21 bypass is at least *consistent* with SET-21's
own contract (purge cache is named in SET-21's description as one of the
low-risk actions it skips) — but it means a power user loses the one guardrail
on a destructive operation.

## SET-61 — Maximum concurrent downloads
**Type:** number (slider + numeric input)
**Section:** Instances ▸ Resource management
[SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:275]
**Nav path:** as SET-58
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:92]
[SOURCE packages/app-lib/src/state/mod.rs:344]
**Purpose:** The number of files fetched in parallel. Reached for by users on
flaky or metered connections where 10 parallel requests make everything worse,
and by users on fast links who want more.
**Operation:** Builds the `FetchSemaphore` **once, at startup**
[SOURCE packages/app-lib/src/state/mod.rs:344] — which is why the description says
"Requires an app restart." Changing it mid-session has no effect until restart.
**Controls:** **min 1, max 10, step 1**, slider + numeric input
[SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:279].
Description: "Number of files the app can download at once. Lower this if
downloads are unreliable on your connection. Requires an app restart."
**Default:** **`10`** [SOURCE packages/app-lib/migrations/20240711194701_init.sql:4],
[SOURCE apps/app-frontend/src/composables/use-app-settings.ts:6] — i.e. the slider's
maximum, the same "displayed max is the stored default" problem as SET-11.
**Persistence:** `settings.max_concurrent_downloads`.
**Rationale & notes:** The description gives an **actionable rule** ("lower this if
downloads are unreliable") rather than restating the label — that is what a
performance setting needs. The restart requirement is disclosed. What is missing:
no explanation of the trade-off (more parallelism = faster bulk installs but
worse reliability and more memory), and no "apply now" button despite the
restart being the entire downside.

## SET-62 — Maximum concurrent writes
**Type:** number (slider + numeric input)
**Section:** Instances ▸ Resource management
[SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:291]
**Nav path:** as SET-58
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:101]
[SOURCE packages/app-lib/src/state/mod.rs:346]
**Purpose:** The number of files written to disk in parallel. Reached for by
users on network shares, external drives, or spinning disks who hit I/O errors.
This is a genuinely well-targeted setting: it exists for one problem and the
description names that problem.
**Operation:** Builds the `IoSemaphore` at startup
[SOURCE packages/app-lib/src/state/mod.rs:346]; restart required.
**Controls:** **min 1, max 50, step 1** [SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:295].
Note the range is **1–50**, not 1–10 like SET-61, and the default is 10 —
i.e. the default sits at 20 % of the range.
Description: "Number of files the app can write to disk at once. Lower this if
you frequently encounter I/O errors. Requires an app restart."
**Default:** `10` [SOURCE packages/app-lib/migrations/20240711194701_init.sql:5].
**Persistence:** `settings.max_concurrent_writes`.
**Rationale & notes:** SET-61 and SET-62 are placed adjacently with near-identical
wording, differing only in "download"/"write" and the failure mode — that parallel
structure is a genuine strength and makes the pair easy to reason about. The
inconsistent ranges (10 vs 50) are defensible (disk parallelism is cheap, network
is not) but **are not explained**, and the asymmetric default position is a small
tell that these two were tuned separately. One sentence would fix it.

## SET-63 — App database backups → Open backups folder
**Type:** button
**Section:** Instances ▸ Resource management, last-but-one block
[SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:307]
**Nav path:** as SET-58
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:119]
[SOURCE apps/app-frontend/src/helpers/utils.js:67]
**Purpose:** Find and restore automatic database snapshots. Reached for only
after something has gone wrong — which is exactly when you need the path.
**Operation:** `plugin:utils|show_app_db_backups_folder` opens the folder in the OS
file manager.
**Controls:** one button, "Open backups folder". No retention setting, no
"restore" action, no listing of what is backed up or when.
**Default:** n/a.
**Persistence:** none.
**Rationale & notes:** **The label promises a feature the UI does not have.** The
heading says "Backups of important app data are stored here in case you need to
recover them later" — but there is no way to see the backups, restore one, or
delete them from inside the app; the button just opens a folder and the user must
know what a `.sqlite` file is and how to put it back. If we ship backups, ship a
restore button next to the open-folder button; if we cannot, do not use the word
"recover".

## SET-64 — Debug info → Export debug info
**Type:** button
**Section:** Instances ▸ Resource management, last block
[SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:320]
**Nav path:** as SET-58
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:40]
**Purpose:** Produce a support bundle for a bug report. Reached for by anyone
filing an issue — the highest-value button in the tab for support load.
**Operation:** Builds a ZIP of app logs and diagnostics and offers it via a save
dialog; the button shows a spinner while running and is disabled
[SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:324].
Description: "Export app logs and diagnostic data to a ZIP file to share with
Modrinth support."
**Controls:** one button, "Export debug info".
**Default:** n/a.
**Persistence:** none.
**Rationale & notes:** Exactly right, and exactly where it belongs — in the
settings tab, not behind developer mode, because the people who need it are
users filing bugs. The missing half is disclosure: a support bundle may contain
paths, usernames and instance names, and nothing says so here.

## SET-65 — Modrinth App {version} / OS
**Type:** info
**Section:** Modal footer, bottom-left, next to the developer-mode button
[SOURCE apps/app-frontend/src/components/ui/modal/AppSettingsModal.vue:378]
**Nav path:** left rail gear → (footer of any tab)
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/modal/AppSettingsModal.vue:313]
[SOURCE apps/app-frontend/src/components/ui/modal/AppSettingsModal.vue:317]
**Purpose:** The only place the app reports its own version. Reached for by
support, always as the first question.
**Controls:** reads "Modrinth App {version}" and, below it, the platform and OS
version — with a special-cased "macOS" label and otherwise a capitalised platform
name [SOURCE apps/app-frontend/src/components/ui/modal/AppSettingsModal.vue:383].
**Default:** n/a.
**Persistence:** none (`staleTime: Infinity`, computed once)
[SOURCE apps/app-frontend/src/components/ui/modal/AppSettingsModal.vue:264].
**Rationale & notes:** **The version is not selectable or copyable**, which is
the single most common friction in support ("what version are you on?" → user
squints at a screenshot). One click-to-copy on the version string would cost
nothing. There is **no licence/credits/about page anywhere in the app** — a real
gap for an MIT-licensed project. The orchestrator's sweep already confirmed
"Modrinth App 0.21.9 / Linux 24.4.0" (RUNLOG 2026-10-14T12:58Z), which matches
the binary under study.

## SET-66 — Downloading v{version} + progress bar
**Type:** info
**Section:** Modal footer, above the version block, only while an update downloads
[SOURCE apps/app-frontend/src/components/ui/modal/AppSettingsModal.vue:354]
**Nav path:** left rail gear → (footer, while an update is downloading)
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/modal/AppSettingsModal.vue:309]
[SOURCE apps/app-frontend/src/App.vue:1965]
**Purpose:** Reassure the user that the app is updating itself. Reached for
whenever an update lands, which is rare.
**Operation:** The modal subscribes to the app-wide update-download progress
provider [SOURCE apps/app-frontend/src/components/ui/modal/AppSettingsModal.vue:262].
Updates are checked every **5 minutes**
[SOURCE apps/app-frontend/src/App.vue:2015] and auto-downloaded **only on an
unmetered connection** — `isNetworkMetered()` is checked first, and a metered
connection defers to a notification instead
[SOURCE apps/app-frontend/src/App.vue:2000]. On Linux without a package-manager
update path the app polls `https://launcher-files.modrinth.com/updates.json`
every 5 minutes and only ever **notifies** [SOURCE apps/app-frontend/src/App.vue:2022].
**Controls:** none.
**Default:** n/a.
**Persistence:** `settings.pending_update_toast_for_version` suppresses the
"update available" toast once it has been shown for a version
[SOURCE apps/app-frontend/src/App.vue:923].
**`[ORCH-PENDING]`** — the orchestrator should capture the footer in this state.
Trigger: start an update download (easiest by spoofing a metered connection off
and being on a version with a known successor) and screenshot the modal footer.
Exact wording, bar style and whether the row sits above or below the version block
cannot be derived from source alone because the surrounding conditional is
`progress > 0 && progress < 1`.
**Rationale & notes:** The **metered-connection gate is the best decision in this
whole document.** Silently downloading a 60 MB desktop update on someone's capped
mobile hotspot is a genuinely hostile default, and the app checks
[network-metered detection per platform](one call to
`isNetworkMetered`, implemented for Windows/macOS/Linux)
[SOURCE packages/app-lib/src/util/network.rs:20] before downloading. Copy this
exactly. The copy-paste candidate for our app is: *never auto-download a large
payload until you have established the connection is unmetered.*

## SET-67 — Toggle developer mode
**Type:** button (hidden affordance)
**Section:** Modal footer, the Modrinth logo button
[SOURCE apps/app-frontend/src/components/ui/modal/AppSettingsModal.vue:366]
**Nav path:** left rail gear → click the Modrinth logo in the footer **6 times**
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/modal/AppSettingsModal.vue:299]
[SOURCE packages/app-lib/migrations/20240711194701_init.sql:15]
**Purpose:** Reveal the Feature flags tab (SET-23) for staff and bug reporters.
Reached for by anyone told to "turn on developer mode".
**Operation:** `devModeCounter` increments per click; when it exceeds **5**
(i.e. the 6th) the mutation flips `settings.developer_mode`
[SOURCE apps/app-frontend/src/components/ui/modal/AppSettingsModal.vue:299].
Because the tab list is a `computed` over `availableTabs`, gaining developer mode
**inserts a new tab** into the middle of the Display group — the modal re-selects
the current tab by identity to avoid jumping
[SOURCE apps/app-frontend/src/components/ui/modal/AppSettingsModal.vue:285].
The footer then shows "Developer mode enabled." in the brand colour
[SOURCE apps/app-frontend/src/components/ui/modal/AppSettingsModal.vue:362].
**Controls:** no visible label; `aria-label` "Toggle developer mode". **There is no
way to turn it off from the UI** except the same six clicks.
**Default:** `false` [SOURCE packages/app-lib/migrations/20240711194701_init.sql:15].
**Persistence:** `settings.developer_mode`.
**Rationale & notes:** Click-the-logo easter eggs are a genuine usability
anti-pattern: undiscoverable, un-documented, and here **one-directional in
appearance** — a user who enables it cannot tell how to disable it, and the
footer gives no hint. Worse, the counter resets to 0 after firing, so a user who
clicks 5 times, looks away, and comes back has lost their place with no feedback
for the first 5 clicks. If we ship a developer mode, ship a labelled switch in an
Advanced section, visible to anyone.

---

## Settings that exist in the database but have **no UI at v0.21.9**

These are worth listing because a reimplementer reading the schema will find them
and may assume they are user-facing. Each is proven unused by searching the whole
frontend for a read or write.

| Key | Column default | Status |
| --- | --- | --- |
| `default_page` | `'home'` (`DefaultPage::Home`) [SOURCE packages/app-lib/src/state/settings.rs:498] | Parsed and persisted [SOURCE packages/app-lib/src/state/settings.rs:151] but **never read by any UI**. The labels "Home"/"Library" still exist in the locale (`app.appearance-settings.default-landing-page.home`/`.library`) [SOURCE apps/app-frontend/src/components/ui/settings/display/BehaviorSettings.vue:74] with **no control attached**. Vestigial. |
| `collapsed_navigation` | `TRUE` [SOURCE packages/app-lib/migrations/20240711194701_init.sql:9] | Same: persisted, never read [SOURCE packages/app-lib/src/state/settings.rs:152]. Vestigial. |
| `personalized_ads` | `TRUE` [SOURCE packages/app-lib/migrations/20240930001852_disable-personalized-ads.sql:1] | Read by exactly one Rust command, `get_ads_personalization` [SOURCE apps/app/src/api/ads.rs:935], and **written by nothing**. The user-facing control is the external consent manager (SET-31). |
| `skipped_update` | `NULL` [SOURCE packages/app-lib/migrations/20250815164653_updater-settings.sql:1] | **Never read or written** outside the settings round-trip [SOURCE packages/app-lib/src/state/settings.rs:83]. Vestigial — there is no "skip this version". |
| `auto_download_updates` | `NULL` [SOURCE packages/app-lib/migrations/20250815164653_updater-settings.sql:4] | **Never read or written** outside the round-trip. The behaviour it was meant to control is actually driven by metered-detection (SET-66). Vestigial. |
| `migrated` | `FALSE` | Legacy-migration bookkeeping; no UI. |
| `prev_custom_dir` | `NULL` | Rollback pointer for the app-directory move (SET-58). |
| `hide_installed_modpacks` | `FALSE` | Persisted, but its only control is **in-context on the Browse page's filter panel**, not in Settings [SOURCE apps/app-frontend/src/pages/Browse.vue:386]. |
| `advanced_filters_collapsed` | `TRUE` | Persisted; UI state of the Browse filter panel, not in Settings. |
| `dismissed_photosensitivity_filter_warning` | `FALSE` | Persisted; dismisses a warning shown once on the Browse page. |
| `friends_active_collapsed` / `friends_online_collapsed` / `friends_offline_collapsed` / `friends_pending_collapsed` | `FALSE`/`FALSE`/`TRUE`/`TRUE` | Persisted accordion state of the friends panel; **no Settings UI**. |

`[ORCH-PENDING]` — the orchestrator should confirm the *negative*: that no
"Default landing page" row, no "Collapsed navigation" row and no "Personalized
ads" row appear anywhere in the ten settings tabs, and that the `retro` theme
swatch is absent with developer mode off. Source says none of these render; a
screenshot of the full Appearance tab plus a full Behavior tab confirms it.

**The lesson for us:** the `settings` table at v0.21.9 carries at least seven
columns the user can never see or change. Every one of them is a decision the
product made, then stopped telling the user about. If we build a settings store,
we should delete what we stop exposing rather than leaving it in the schema —
otherwise the next person to read the schema cannot tell what the product
actually does.

---

## The `settings` table, complete

One row, `id = 0` [SOURCE packages/app-lib/migrations/20240711194701_init.sql:2].
Ordered as the Rust struct declares them
[SOURCE packages/app-lib/src/state/settings.rs:11].

| Column | Type | Default | Source of default |
| --- | --- | --- | --- |
| `max_concurrent_downloads` | INTEGER | `10` | init.sql:4 |
| `max_concurrent_writes` | INTEGER | `10` | init.sql:5 |
| `theme` | TEXT | `'dark'` | init.sql:7 |
| `locale` | TEXT | `'en-US'` | 20251229145719:2 |
| `default_page` | TEXT | `'home'` | init.sql:8 |
| `collapsed_navigation` | INTEGER | `TRUE` | init.sql:9 |
| `hide_nametag_skins_page` | INTEGER | `0` | 20250514181748:1 |
| `advanced_rendering` | INTEGER | `TRUE` | init.sql:10 |
| `native_decorations` | INTEGER | `FALSE` | init.sql:11 |
| `toggle_sidebar` | INTEGER | `FALSE` | 20241222013857:1 |
| `sync_theme_across_devices` | INTEGER | `TRUE` | 20260818120000:1 |
| `sync_behavior_across_devices` | INTEGER | `TRUE` | 20260818120000:3 |
| `sync_features_across_devices` | INTEGER | `TRUE` | 20260902140000:1 |
| `show_files_tab_in_instances` | INTEGER | `TRUE` | 20260902130000:2 |
| `show_worlds_tab_in_instances` | INTEGER | `TRUE` | 20260902130000:4 |
| `show_screenshots_tab_in_instances` | INTEGER | `FALSE` | 20260902130000:6 |
| `show_skin_selector_in_sidebar` | INTEGER | `TRUE` | 20260902130000:8 |
| `show_jump_in` | INTEGER | `TRUE` | 20260918120000:17 |
| `always_show_copy_details` | INTEGER | `FALSE` | 20260918120000:19 |
| `hide_installed_modpacks` | INTEGER | `FALSE` | 20260918120000:21 |
| `advanced_filters_collapsed` | INTEGER | `TRUE` | 20260918120000:23 |
| `dismissed_photosensitivity_filter_warning` | INTEGER | `FALSE` | 20260918120000:25 |
| `friends_active_collapsed` | INTEGER | `FALSE` | 20260918120000:27 |
| `friends_online_collapsed` | INTEGER | `FALSE` | 20260918120000:29 |
| `friends_offline_collapsed` | INTEGER | `TRUE` | 20260918120000:31 |
| `friends_pending_collapsed` | INTEGER | `TRUE` | 20260918120000:33 |
| `telemetry` | INTEGER | `TRUE` | init.sql:13 |
| `discord_rpc` | INTEGER | `TRUE` | init.sql:14 |
| `personalized_ads` | INTEGER | `TRUE` | 20240930001852:1 |
| `extra_launch_args` | JSONB | `jsonb_array()` | init.sql:20, :41 |
| `custom_env_vars` | JSONB | `jsonb_array()` | init.sql:22, :41 |
| `mc_memory_max` | INTEGER | `2048` | init.sql:23 |
| `mc_force_fullscreen` | INTEGER | `FALSE` | init.sql:24 |
| `mc_game_resolution_x` | INTEGER | `854` | init.sql:25 |
| `mc_game_resolution_y` | INTEGER | `480` | init.sql:26 |
| `hide_on_process_start` | INTEGER | `FALSE` | init.sql:28 |
| `refocus_on_game_close` | INTEGER | `FALSE` | 20260918120000:1 |
| `compact_instance_cards` | INTEGER | `FALSE` | 20260918120000:3 |
| `show_play_time` | INTEGER | `TRUE` | 20260918120000:5 |
| `warn_on_unknown_modpacks` | INTEGER | `TRUE` | 20260918120000:7 |
| `skip_non_essential_warnings` | INTEGER | `FALSE` | 20260918120000:9 |
| `hook_pre_launch` | TEXT NULL | `NULL` | init.sql:30 |
| `hook_wrapper` | TEXT NULL | `NULL` | init.sql:31 |
| `hook_post_exit` | TEXT NULL | `NULL` | init.sql:32 |
| `custom_dir` | TEXT NULL | `NULL` | init.sql:34 |
| `prev_custom_dir` | TEXT NULL | `NULL` | init.sql:35 |
| `migrated` | INTEGER | `FALSE` | init.sql:36 |
| `developer_mode` | INTEGER | `FALSE` | init.sql:15 |
| `feature_flags` | JSONB | `'{}'` | 20241222013857:2 |
| `skipped_update` | TEXT NULL | `NULL` | 20250815164653:1 |
| `pending_update_toast_for_version` | TEXT NULL | `NULL` | 20250815164653:2 |
| `auto_download_updates` | INT NULL | `NULL` | 20250815164653:4 |
| `version` | INTEGER | `1` | 20251002173041:1 |

The struct also carries `#[serde(default = "default_true")]` on seven booleans
[SOURCE packages/app-lib/src/state/settings.rs:24],
[SOURCE packages/app-lib/src/state/settings.rs:26],
[SOURCE packages/app-lib/src/state/settings.rs:28],
[SOURCE packages/app-lib/src/state/settings.rs:29],
[SOURCE packages/app-lib/src/state/settings.rs:32],
[SOURCE packages/app-lib/src/state/settings.rs:35],
[SOURCE packages/app-lib/src/state/settings.rs:41],
[SOURCE packages/app-lib/src/state/settings.rs:44] so a row written by an older
build deserialises without erroring. `Settings::CURRENT_VERSION` is `3`
[SOURCE packages/app-lib/src/state/settings.rs:110] and `perform_migration` knows
exactly two steps: v1→v2 re-quotes the hook strings with `shlex::Quoter`
[SOURCE packages/app-lib/src/state/settings.rs:395] and v2→v3 rewrites the legacy
2048 MB memory default [SOURCE packages/app-lib/src/state/settings.rs:419].

**Not in this table, but stored as settings:**
- `java_versions` — four possible rows [SOURCE packages/app-lib/migrations/20240711194701_init.sql:43]
- `sync_feature_settings` — the seven synced-option switches [SOURCE packages/app-lib/migrations/20260819120000_instance-synced-options.sql:3]
- `app_metadata['store_cache_limit_bytes']` — SET-55 [SOURCE packages/app-lib/src/state/content_store/commands/cache.rs:151]
- `localStorage['modrinth-theme']`, `['modrinth-preferred-dark-theme']`, `['modrinth-quick-instance-count']` — SET-1, SET-2, SET-11

---

## Save-model matrix

| Tab | Model | Unsaved-changes bar | Reset restores |
| --- | --- | --- | --- |
| Appearance | `useSavable` | yes | last saved |
| Features | `useSavable` | yes | last saved |
| Behavior | `useSavable` | yes | last saved |
| Language | `useSavable` | yes | last saved locale |
| Feature flags | write-through `watch` | no | (per-row "Reset to default" only) |
| Profile | `useSavable` (exposed API) | yes | last saved |
| Social | `useSavable` (exposed API) | yes | last saved |
| Privacy | write-through `watch` | no | none |
| Synced settings (toggles) | mutation on toggle | no | none |
| Game settings modal | own editor + action bar | yes | last saved |
| Synced settings (launch options) | `watch` on every change | no | none |
| Java installations | write on change | no | none |
| Resource management | `watch` on every change | no | none |

[SOURCE apps/app-frontend/src/utils/savable.ts used by the first five and by
Profile/Social/Game settings — SOURCE packages/ui/src/utils/savable.ts:5;
the write-through tabs use a `deep: true` `watch` calling `set(settings)` —
SOURCE apps/app-frontend/src/components/ui/settings/account/PrivacySettings.vue:60,
SOURCE apps/app-frontend/src/components/ui/settings/instances/ResourceManagementSettings.vue:134,
SOURCE apps/app-frontend/src/components/ui/settings/instances/instances-synced-settings/launch-options.vue:86]

---

## Defaults that ship **on**, and whether that is right

Worth separating, because "the app ships X on" is a design position, not a fact.

| On by default | Assessment |
| --- | --- |
| `advanced_rendering` (SET-3) | **Debatable.** Expensive on software rendering; assume most users have a GPU. |
| `show_files_tab_in_instances`, `show_worlds_tab_in_instances`, `show_skin_selector_in_sidebar` (SET-6/7/10) | Right — mature features. |
| `show_play_time`, `show_jump_in` (SET-18, SET-12) | Right — cheap and pleasant. |
| `warn_on_unknown_modpacks` (SET-20) | **Right.** Safety on, and the copy-through bug is the only flaw. |
| all three `sync_*_across_devices` (SET-2, SET-5, SET-13) | Reasonable for signed-in users; the toggles are disabled when signed out so the default is harmless. |
| `telemetry` (SET-32) | **Wrong.** Opt-out-by-default analytics in a desktop app that also sells a subscription. |
| `discord_rpc` (SET-33) | **Wrong.** Default-on broadcast of your activity to a third party, and the description's restart warning is false. |
| `command_history`, `multiplayer_servers`, `creative_hotbars`, `game_options` synced options on a fresh install | **Right**, and well-argued by the migration that does it only when no instances exist. |

## Defaults pinned from source vs. genuinely unknown

**Pinned from source (defaults I could prove):** SET-1 through SET-63 and SET-65,
SET-67 — every default listed above, from the migration that declares it.
`SET-44`'s RAM-derived default is pinned to the *code path*, with the caveat that
which branch executes depends on the orchestrator's machine's RAM.

**Not pinned (genuinely unknown, needs observation or an out-of-repo source):**
- SET-27/28/29: the **server-side** default for `friend_privacy`,
  `shared_instances_privacy`, `hosting_access_privacy`. The frontend's local
  fallback is `everyone`; the authoritative default is in `apps/labrinth`, which
  is in the clone — but I did not chase it because it is an API-server concern
  rather than an app-settings concern, and this module documents the app.
- SET-11: whether `QUICK_INSTANCE_LIMIT_MAX` (20) is displayed while the stored
  value is genuinely "uncapped" — the code says both at once, and only the UI
  can show which wins.

---

## What to copy, what not to

**Copy:**
1. **Whole-object read/modify/write with a re-read before the write**
   [SOURCE apps/app-frontend/src/components/ui/settings/display/BehaviorSettings.vue:182],
   plus a named mutation scope so two tabs cannot clobber each other.
2. **Three sync toggles with copy that tells you how to opt out**
   (SET-2/5/13), and **language synced unconditionally** because it is the one
   setting people genuinely cannot work without.
3. **Translation-coverage percentages next to each language** (SET-22).
4. **Power-of-two snap points on the memory slider** (SET-44).
5. **Metered-connection detection before any large auto-download** (SET-66).
6. **Descriptive copy that states both what a setting affects and what it does
   not** — SET-24's description of which image types, SET-33's sentence about
   mod-added Rich Presence, SET-59's "always available for failed installs",
   SET-21's enumeration of what is and is not skipped. This is Modrinth's single
   strongest habit in the settings surface.
7. **A debug-info export in the normal settings tab, not behind developer mode**
   (SET-64).
8. **The flag-promotion ladder** — flag → default-off setting → default-on
   setting → user toggle — and the migration discipline that comes with it
   (explicit `COALESCE(json_extract(...))` per renamed flag, then `json_remove`
   of the old keys) [SOURCE packages/app-lib/migrations/20260918120000_promote-behavior-settings.sql:59].
9. **Adaptive, state-aware destructive buttons** — orange + warning tooltip when
   instances are running (SET-57).
10. **Defaults that track feature maturity** (SET-6/7 on, SET-8/36 off) and the
    fresh-install-vs-upgrade default split for synced options
    [SOURCE packages/app-lib/migrations/20260902120000_enable-synced-options-for-fresh-installs.sql:1].

**Do not copy:**
1. **Default-on telemetry and default-on Discord Rich Presence** (SET-32, SET-33).
2. **Positive label + inverted key and inverted `id`**: SET-16 (`toggle_sidebar` =
   "Hide right sidebar"), SET-32 (`opt-out-analytics`), SET-33
   (`disable-discord-rpc`). One of these two is always going to be wrong.
3. **A confirm dialog that silently changes a setting** (SET-20). Use an explicit
   "don't ask again" checkbox.
4. **A destructive button with no confirmation next to an identical one that
   confirms** (SET-56 vs SET-60).
5. **A settings key that kills the user's running games with no confirm**
   (SET-57).
6. **A label that promises recovery the UI cannot perform** (SET-63).
7. **Silent backend redirection of a visible setting into a different store**
   (SET-41 under SET-34).
8. **Round-tripping structured data through a free-text field**
   (SET-46: `KEY=a b` is silently corrupted).
9. **A "Reset" that restores the last saved value while the UI implies it
   restores the default** — and the corollary, ten feature flags sharing one
   `id="advanced-rendering"` [SOURCE apps/app-frontend/src/components/ui/settings/display/FeatureFlagSettings.vue:71].
10. **A six-click easter egg as the only route to a feature, with no way back**
    (SET-67).
11. **Settings persisted in the schema with no UI** — seven of them at v0.21.9.
    Delete what you stop exposing.
12. **A default derived from a magic number that also means "unset"** — the 2048
    MB memory heuristic (SET-44) and the ≥ max-means-unbounded quick-instance
    rule (SET-11). Both silently change user data.