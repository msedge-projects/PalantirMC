# Module 05 — The per-instance surface

**Scope.** Everything reachable under `/instance/:id`: the sub-navigation bar, the six tab
bodies (Content, Files, Screenshots, Worlds, Logs, Share), the instance page header, and the
instance Settings modal. Covers how each tab builds its list, what each action does to disk,
and exactly which global settings an instance view overrides versus inherits.

**Source pin.** `modrinth/code` @ `v0.21.9` (`6ebd75b68dd386cd4d74eee1cf2e96d945c937db`).
All paths below are relative to that repo root. Vue SFCs are cited by template/script line.

---

## Second observation pass (2026-10-14) — this surface is now OBSERVED

The first pass could not create an instance (OQ-004), so the whole tab bar and the
instance Settings modal were `SOURCE`-only. A second pass fixed the environment
(OQ-012) and created a real instance ("Fabric 26.3", game 26.3, Fabric 0.19.5)
with **no account**, then captured the surface. Everything in this section is
`OBSERVED`; the source analysis above still stands and now has pixels behind it.

**Tab bar as actually rendered.** `Content · Files · Worlds · Logs · Share`
`[S-0099 S-0101 S-0102 S-0103 S-0104]`. Note there is **no `Screenshots` tab in
this build's bar** — the five tabs above are what a fresh Fabric instance shows.
That is the `Features` setting working as designed (module 06): the set of tabs is
data-driven, so a screenshot of "the tab bar" is only ever a screenshot of *one*
feature configuration. The source lists six possible tab bodies; five render by
default.

**INST-CONTENT — the installed-content list** `[S-0117]`. Once content exists the
empty state is replaced by a table: columns **Project / Version / Actions**, one
row per installed project, each row carrying the project icon + name + author,
the resolved version string and the jar filename, and three per-row actions — an
enable/disable toggle (on by default), an update/reinstall arrow, and a delete
trash — plus a kebab. Controls above the table: `Upload files`, `Browse content`,
a `Search N projects` field, a name sort, `All | Mods` filter chips, `+ Filter`,
and `Refresh`.
**Purpose:** the instance's own inventory, and the place a user repairs or prunes
it. **Operation:** the toggle writes the item's disabled state (the jar stays on
disk); the arrow reinstalls/updates that one item; the trash removes it.
**Persistence:** per-instance, on disk under the instance's `mods/` etc.

**INST-FILES — the file browser** `[S-0101]`. A breadcrumb (starting at a home
glyph), a `Search files` field, `Refresh`, and a `+` add button over a table:
**Name (sortable) / Size / Created / Modified / Actions**. Rows observed: `config`,
`crash-reports`, `datapacks`, `mods`, `resourcepacks`, `saves`, `screenshots`,
`shaderpacks` — i.e. the instance directory itself, rendered as a navigable tree
with per-row actions. This is the honest "open folder, but inside the app"
surface. **Purpose:** inspect and edit instance files without leaving the launcher.

**INST-WORLDS** `[S-0102]`. Empty state: "No servers or worlds added" /
"Add a server or browse to get started", with `Add server` and `Browse servers`.
Finding: **Worlds and servers share one tab** in this build — a saved world and a
joined server are one list.

**INST-LOGS** `[S-0103]`. A `Search logs` field, a `Live Log` view selector, filter
chips `All | Error | Warn | Info`, an `Expand` affordance, and an ASCII-art empty
state: "Start your instance in the top right to start receiving live logs!". The
empty state is itself a call to action (module 10's pattern) and confirms the log
view is a *stream*, not a file dump.

**INST-SHARE** `[S-0104]`. Empty state "No friends invited" / "You can share this
instance with your friends!" + `Invite friends`. Account-gated in effect (there is
no one to invite without a Modrinth account), so documented here as seen.

**INST-SETTINGS — the instance Settings modal** `[S-0105 S-0106 S-0107]`. Title
breadcrumb "<instance> > Settings", left nav **General · Installation · Sync
overrides**:
- **General** `[S-0105]`: `Name` field, `Icon` (+ the same Upload/Randomize/
  Customize affordances as creation), `Duplicate instance` with the helper
  "Creates a copy of this instance, including worlds, configs, mods, etc.",
  `Update channel` = **Release (default) | Beta | Alpha** with "Only release
  versions will be shown as available updates.", and `Delete instance` (red) with
  the consequence sentence: "Permanently deletes an instance from your device,
  including your worlds, configs, and all installed content. Be careful, as once
  you delete a instance there is no way to recover it."
- **Installation** `[S-0106]`: read-only **Installation info** (Platform `Fabric`,
  Game version `26.3`, Fabric version `0.19.5`), a warning — "We don't recommend
  editing your installation settings after installing content… it may cause
  issues." — and `Repair` with "Reinstalls Minecraft dependencies and checks for
  corruption. This may resolve issues if your game is not launching due to
  launcher-related errors."
- **Sync overrides** `[S-0107]`: the intro "Enable an override to keep a synced
  setting separate for this instance.", a `Manage synced settings` button, and
  **five independent toggles, all default off** — *Unsync game settings*
  ("Keep this instance's options.txt separate from the synced copy."), *Unsync
  multiplayer servers*, *Unsync resource packs*, *Unsync command history*, *Unsync
  saved creative hotbars*. Each carries a small edit affordance.

**Purpose/rationale of the instance Settings split.** Global settings are global;
an instance view only exists to answer "how is *this* one different", so the modal
offers exactly three things — identity (General), install facts + repair
(Installation), and the list of things that *could* be pulled out of global sync
(Sync overrides). The toggles are phrased as the exception ("Unsync…"), which is
the correct default: shared-by-default, opt out per instance.

**Supersedes these `[ORCH-PENDING]` markers above:** the tab-bar rendering, the
Files/Worlds/Logs/Share bodies, the instance Settings General/Installation/Sync
overrides pages, the per-instance override toggles, and Repair. **Still
`[ORCH-PENDING]`:** `INST-17` export-world's rendered affordance (no world existed
to export), and tab-bar overflow at very narrow widths (the window floors at
1100 px; see OQ-002).

---

## Coverage table

| ID | label | tab | defaults | persistence key | evidence | status |
| --- | --- | --- | --- | --- | --- | --- |
| INST-1 | Instance sub-navigation (NavTabs) | nav | — | none (route-driven) | `apps/app-frontend/src/pages/instance/layout.vue:513` | documented |
| INST-2 | Content | tab | always first | route `/instance/:id` | `layout.vue:515`, `routes.js:107` | documented |
| INST-3 | Files | tab | `showFilesTabInInstances=true` | app setting `show_files_tab_in_instances` | `layout.vue:522`, `use-app-settings.ts:54` | documented |
| INST-4 | Screenshots | tab | `showScreenshotsTabInInstances=false` | app setting `show_screenshots_tab_in_instances` | `layout.vue:530`, `use-app-settings.ts:56` | documented |
| INST-5 | Worlds | tab | `showWorldsTabInInstances=true` | app setting `show_worlds_tab_in_instances` | `layout.vue:538`, `use-app-settings.ts:55` | documented |
| INST-6 | Logs | tab | always shown | route `/instance/:id/logs`, `renderMode: 'fixed'` | `layout.vue:544`, `routes.js:126` | documented |
| INST-7 | Share | tab | conditional (see item) | route `/instance/:id/share` | `layout.vue:548`, `layout.vue:497` | documented |
| INST-8 | Content list + project-type filter pills | content | sort `alphabetical-asc` | `sessionStorage: content-sort:<instanceId>` | `packages/ui/src/layouts/shared/content-tab/layout.vue:197` | documented |
| INST-9 | Enable / disable toggle (content) | content toggle | derived from file suffix | file renamed `<name>.disabled` | `content/index.vue:813`, `paths.rs:99` | documented |
| INST-10 | "Update all" | content button | shown when ≥1 outdated | n/a (action) | `content-tab/layout.vue:1375` | documented |
| INST-11 | "Browse content" / "Upload files" (add content in context) | content button | — | n/a | `content-tab/layout.vue:1449` | documented |
| INST-12 | Per-item overflow menu | content menu | Show file / Copy link / Freeze | per-item `locked` flag | `content/index.vue:1497` | documented |
| INST-13 | Content update surfacing (`has_update` / `update_version_id`) | content | auto, 10 min throttle | SQLite `content_update_checks` | `layout.vue:242`, `update_check.rs:6` | documented |
| INST-14 | Worlds list, search, filters | tab | filters empty | none | `worlds/index.vue:24` | documented |
| INST-15 | Play world / play server | button | — | quick-play args | `worlds/index.vue:92`, `:535` | documented |
| INST-16 | Edit world / delete world | button | — | `level.dat` NAME + folder | `worlds/index.vue:95`, `:510` | documented |
| INST-17 | Export world (to `.zip`) | button | — | writes `.zip` beside world | `worlds/index.vue:23` (uses `WorldItem`) | [ORCH-PENDING] |
| INST-18 | Open folder (world) | button | — | — | `worlds/index.vue:105` | documented |
| INST-19 | Add server / Browse servers | button | — | instance synced servers | `worlds/index.vue:39`, `:43` | documented |
| INST-20 | Desync server | button | server rows only | synced-option off + server removed | `worlds/index.vue:104`, `:504` | documented |
| INST-21 | Screenshots viewer + actions | tab | sort Newest, group custom | `<instance>/screenshots` | `screenshots-page/index.vue:146` | documented |
| INST-22 | Logs: Live Log source | tab entry | always index 0 | n/a | `logs/index.vue:52` | documented |
| INST-23 | Logs: historical log files | tab entry | filtered to `.log`/`.txt` | instance logs dir + crash-reports | `logs/index.vue:50`, `logs.rs:280` | documented |
| INST-24 | Logs: Clear / Delete / Share | button | — | deletes file | `console/layout.vue:43`, `logs/index.vue:136` | documented |
| INST-25 | Logs: crash analysis | banner | runs on mount if not playing | network (`mclogs.insights_v1`) | `logs/index.vue:113` | documented |
| INST-26 | Files browser | tab | — | instance root path | `files/index.vue:65` | documented |
| INST-27 | Files search | field | empty | none | `file-search.ts:9` | documented |
| INST-28 | Files: read-only enforcement / path safety | constraint | — | — | `files.rs:57` (app-lib) | documented |
| INST-29 | Share: invite friends / invite link | tab | — | server-side invite codes | `share/index.vue:4` | documented |
| INST-30 | Share: publish (share instance content) | button | owner only | server-side publish state | `share/index.vue:44` | documented |
| INST-31 | Settings modal: General (name, icon, update channel, duplicate, delete) | dialog tab 0 | name ≤80 chars | instance row `name` / `icon_path` | `general-settings.vue:46` | documented |
| INST-32 | Settings modal: Installation (loader / game version / loader version) | dialog tab 1 | read from instance | content-set row | `installation-settings.vue:210` | documented |
| INST-33 | Settings modal: Repair | button | — | reinstall from link | `installation-settings.vue:371` | documented |
| INST-34 | Settings modal: Reinstall / Swap modpack | button | imported packs only | re-import `.mrpack` | `installation-settings.vue:383` | documented |
| INST-35 | Settings modal: Sync overrides (synced options) | dialog tab 2 | all inherited | per-instance `synced_options` | `synced-options-settings.vue:198` | documented |
| INST-36 | Custom memory allocation | field | inherits global until toggled | `launch_overrides.memory` | `java-settings.vue:102` | documented |
| INST-37 | Custom Java installation | field | inherits global JRE pool | `launch_overrides.java_path` | `java-settings.vue:42` | documented |
| INST-38 | Custom Java arguments | field | inherits global | `launch_overrides.extra_launch_args` | `java-settings.vue:92` | documented |
| INST-39 | Custom environment variables | field | inherits global | `launch_overrides.custom_env_vars` | `java-settings.vue:97` | documented |
| INST-40 | Custom window settings (fullscreen / width / height) | field | inherits global | `launch_overrides.force_fullscreen` / `game_resolution` | `window-settings.vue:18` | documented |
| INST-41 | Custom game launch hooks | field | inherits global | `launch_overrides.hooks` | `hooks-settings.vue:181` | documented |
| INST-42 | Settings modal: Sharing (invites, unpublish) | dialog tab 3 | owner only | server-side | `sharing-settings.vue:13` | documented |
| INST-43 | Page header: Play / Stop / Repair / Settings / More | button | — | n/a | `page-header/index.vue:71` | documented |
| INST-44 | Page header overflow: Open folder / Export modpack / Create shortcut / Report | menu | — | n/a | `page-header/index.vue:356` | documented |

---

## INST-1 — Instance sub-navigation

**Type:** tab (navigation bar)
**Nav path:** `/instance/:id/*`
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/layout.vue:59] (renders `<NavTabs :links="tabs" />`), [SOURCE apps/app-frontend/src/pages/instance/layout.vue:513] (`tabs` computed), [SOURCE packages/ui/src/components/base/NavTabs.vue:7] (the `<nav>` element), [SOURCE apps/app-frontend/src/routes.js:90] (parent route `/instance/:id`)

**Purpose:** Gives a launched instance a single home and the six views that operate on it. The
bar is a rounded "pill" segmented control whose active segment is a single absolutely-positioned
slider that animates between tabs (80 ms, with a deliberate 175 ms trailing-edge stagger so the
leading edge of the pill moves first). Tabs are hidden rather than disabled, and the component
renders nothing at all when only one link survives filtering — so a fully-minimal instance shows a
header with no tab bar instead of a one-item pill.

**Operation:** `NavTabs` iterates `filteredLinks` in the order supplied by `layout.vue` and renders
each as a `RouterLink` to `link.href` (`NavTabs.vue:17-38`). Active tab is computed by reverse
scanning the link list and matching `decodeURIComponent(route.path)` against `decodeURIComponent(link.href)`
— exact match first, then a "subpage" match where the path starts with the href and the next
character is `/` (`NavTabs.vue:177-213`). A `ResizeObserver` on the scroll container re-measures the
slider on reflow (`NavTabs.vue:328`). There are no network calls.

**Controls:** `links[]` of `{label, href, shown?, icon?, subpages?, onHover?}` (`NavTabs.vue:86-93`).
`mode` is `'navigation'` here (RouterLink-based); a `'local'` mode exists for in-page segments.
`pageNav` is false on the instance page (the bar is inline), true elsewhere (it becomes a full-width
scroll strip with `-mx-6 -mt-2`). Icons are hidden below the `sm` breakpoint (`hidden sm:block`,
`NavTabs.vue:32`) — narrow windows keep labels only.

**Persistence:** none. The active tab is purely the URL. Note a genuine defect: the component
auto-hides itself when `filteredLinks.length <= 1`, so an instance with only Content + Logs still
renders (2 > 1), but the tab order itself is *not* persisted anywhere.

**Rationale & notes:** Ordering is `Content → Files → Screenshots → Worlds → Logs → Share`, and
it is **not** alphabetical: it is frequency-of-use descending. Content is always index 0 and
unconditional; Logs is the only other always-on tab, deliberately pinned last among the always-on
group because it is the "something went wrong" destination. Share is last because it is the rarest
and most conditional. Files/Screenshots/Worlds are inserted in that order only if their
*global* app setting is on, so the bar can grow and shrink globally but never per instance — see
INST-3/4/5. The icon set is chosen for silhouette distinguishability at 20 px rather than familiarity.

---

## INST-2 — Content

**Type:** tab
**Nav path:** `/instance/:id` (route name `InstanceContent`)
**Evidence:** [SOURCE apps/app-frontend/src/routes.js:107], [SOURCE apps/app-frontend/src/pages/instance/layout.vue:515]

**Purpose:** The default landing view of an instance and the page users spend the most time on —
everything installed into the pack, and every mutation available on it. Also hosts the shared /
modpack "managed content" banner, because that banner's whole job is to explain who owns the
content below it.

**Operation:** The page component is `pages/instance/content/index.vue`. It provides a
`ContentManager` context (`content/index.vue:1600`) that the shared `ContentCardLayout` consumes.
Data comes from `loadInstanceContentData()` which fires `get_content_items` +
`get_linked_modpack_info` in parallel (`helpers/instance-content.ts:27`). The query is cached under
`instanceKeys.content(<id>)` with a 30 s stale time and `networkMode: 'always'`
(`query-options.ts:112-121`). Installing items are optimistically merged into the list so a
just-installed mod appears with an `installing` flag before the query round-trips
(`content/index.vue:271-284`). Every file dropped onto the webview is auto-installed via
`add_project_from_path` and `.mrpack` drops are ignored (`content/index.vue:1767-1777`).

**Controls:** See INST-8..INST-13.

**Persistence:** content state lives in SQLite (`content_rows.rs`), not in `modrinth.json`. View
state only: `sessionStorage['content-sort:<instanceId>']` and
`sessionStorage['content-filters:<instanceId>']`, both keyed on `filterPersistKey = instance.id`
(`content/index.vue:1701`, `content-tab/layout.vue:197`, `content-filtering.ts:64`).

**Rationale & notes:** The route `projects/:type` (`InstanceContentFilter`) exists in
`routes.js:111` and renders the *same* component, but nothing in the app reads `route.params.type`
or navigates to it — **verified dead route** (`grep` for `params.type` / `/projects` across
`apps/app-frontend/src` returns nothing). The per-type narrowing that used to be URL-addressable is
now session-storage filter pills instead (INST-8).

---

## INST-3 — Files

**Type:** tab
**Nav path:** `/instance/:id/files`
**Evidence:** [SOURCE apps/app-frontend/src/routes.js:120], [SOURCE apps/app-frontend/src/pages/instance/layout.vue:522]

**Purpose:** Raw escape hatch into the instance directory for anything the launcher does not model
(configs, `options.txt`, `saves`, shader zips you dropped yourself). Reached for rarely, but
when something is wrong it is the only place to look.

**Operation:** see INST-26..INST-28.

**Controls:** gated on the **global** app setting `show_files_tab_in_instances`.

**Persistence:** `AppSettings.show_files_tab_in_instances` (SQLite settings row, mirrored into the
reactive app-settings store at `App.vue:832`; default `true` at `use-app-settings.ts:54`).

**Rationale & notes:** See INST-26. A per-instance `visible_tabs.files` override exists in the data
model (`InstanceTabVisibility`, `launch.rs:7-20`) and is even editable by
`settings-modal/behavior-settings.vue:97` — but that component is **never imported**
(`grep` for `behavior-settings` across `apps/` + `packages/` matches only the unrelated
global `components/ui/settings/display/BehaviorSettings.vue`). So the per-instance tab-visibility
feature is fully implemented backend-side and entirely unreachable from the UI.

---

## INST-4 — Screenshots

**Type:** tab
**Nav path:** `/instance/:id/screenshots`
**Evidence:** [SOURCE apps/app-frontend/src/routes.js:122], [SOURCE apps/app-frontend/src/pages/instance/layout.vue:530]

**Purpose:** Shows the screenshots captured during play for this instance, with the same rich
viewer (crop/edit, copy, group, export) as the global Screenshots page.

**Operation:** see INST-21.

**Controls:** gated on `show_screenshots_tab_in_instances`, which **defaults to `false`** — this is
the only instance tab that is opt-in.

**Persistence:** `AppSettings.show_screenshots_tab_in_instances` (`use-app-settings.ts:56`,
`settings.rs:31`).

**Rationale & notes:** Defaulting a whole tab off is a strong signal that it was judged lower-value
than the cost of a permanently-present pill. It sits between Files and Worlds because all three are
"assets you produced", ordered by how often you go looking for them.

---

## INST-5 — Worlds

**Type:** tab
**Nav path:** `/instance/:id/worlds`
**Evidence:** [SOURCE apps/app-frontend/src/routes.js:100], [SOURCE apps/app-frontend/src/pages/instance/layout.vue:538]

**Purpose:** A unified launcher for both singleplayer saves on disk and multiplayer servers, so
"play this world" is one click from the instance rather than a multi-step modded-Minecraft launch.

**Operation:** see INST-14..INST-20.

**Controls:** gated on `show_worlds_tab_in_instances`.

**Persistence:** `AppSettings.show_worlds_tab_in_instances` (default `true`,
`use-app-settings.ts:55`).

**Rationale & notes:** See INST-14.

---

## INST-6 — Logs

**Type:** tab
**Nav path:** `/instance/:id/logs`
**Evidence:** [SOURCE apps/app-frontend/src/routes.js:124], [SOURCE apps/app-frontend/src/pages/instance/layout.vue:544]

**Purpose:** The instance's console — live tail while running, plus every retained log file and
crash report, with server-side crash analysis.

**Operation:** see INST-22..INST-25.

**Controls:** always shown; the only per-route layout flag in the whole router.

**Persistence:** route meta only — `meta: { renderMode: 'fixed' }` (`routes.js:126`).

**Rationale & notes:** `renderMode: 'fixed'` is the one route in the app that pins the header + tabs
and gives the tab body its own scroll container. The comment in `layout.vue` explains why: the log
console needs a bounded height to resolve `h-full`. Every other instance tab scrolls with the
viewport so `position: sticky` works. This is a good example of a layout mode chosen per-content-
type rather than globally.

---

## INST-7 — Share

**Type:** tab
**Nav path:** `/instance/:id/share`
**Evidence:** [SOURCE apps/app-frontend/src/routes.js:103], [SOURCE apps/app-frontend/src/pages/instance/layout.vue:548], [SOURCE apps/app-frontend/src/pages/instance/layout.vue:497] (`showShareTab`)

**Purpose:** Multiplayer-by-link. Lets you hand a friend an invite link or a code so they get a
copy of *this exact instance*, content included.

**Operation:** see INST-29..INST-30.

**Controls:** shown only when **all** of: the signed-in user is eligible for shared instances, the
instance is not quarantined, the user's role is not `member`, and the link type is not
`server_project` / `server_project_modpack` (`layout.vue:497-509`). If the user is on the Share route
and the instance becomes quarantined, a watcher force-redirects them back to Content
(`layout.vue:576-586`).

**Persistence:** none local. Server-side invite codes; cached under
`instanceKeys.sharedMembers(<id>)` and `['sharedInstanceInvites', <id>]`.

**Rationale & notes:** Note the inverse of the usual rule: the tab is hidden from **members**
(`role === 'member'`) and shown to **owners**. A member has nothing to share — they can only accept
updates — so giving them a Share tab would be an empty promise. The owner also gets a separate
Sharing settings tab (INST-42) for link-level management, so the two surfaces split as
"invite people" (tab) vs "manage existing invites + unpublish" (settings).

---

## INST-8 — Content list and project-type filter pills

**Type:** field (filter) + list
**Nav path:** `/instance/:id` (session-scoped, not URL)
**Evidence:** [SOURCE packages/ui/src/layouts/shared/content-tab/layout.vue:277] (`useContentFilters` with `showTypeFilters: true`), [SOURCE packages/ui/src/layouts/shared/content-tab/composables/content-filtering.ts:74] (option construction), [SOURCE apps/app-frontend/src/pages/instance/content/index.vue:1701] (`filterPersistKey: instance.value.id`)

**Purpose:** One flat table holding every content type at once (mods, resource packs, shaders,
data packs, plugins). The type split is a *filter*, not separate tabs, because a user debugging a
crash needs to see "this mod and this shaderpack and this datapack" together.

**Operation:** `ContentCardTable` renders `ctx.items` after `applyFilters()`; each row maps
through `mapToTableItem` (`content/index.vue:1661`). Sorting is client-side over the whole list
(`layout.vue:235`). Search is `useContentSearch` over three fields: `project.title`, `owner.name`,
`file_name` (`layout.vue:271`). Metadata filters (author) are powered by
`useContentMetadataFilters` and only render when at least one item has an owner
(`layout.vue:290`, `:307`).

**Controls:**
- **Type pills** — generated dynamically from the types *present in this instance*, ordered by
  descending frequency (`content-filtering.ts:78-89`); labels come from
  `commonProjectTypeCategoryMessages`. An instance with only mods never shows a "Shaders" pill.
- **Status pills** — "Enabled"/"Disabled" are only offered if the instance contains **both**
  states (`content-filtering.ts:67-72`). Selecting one deselects the other
  (`content-filtering.ts:128-140`).
- **Updates / Warnings pills** — explicitly disabled on the instance (`showUpdateFilter: false`,
  `showWarningsFilter: false`, `content-tab/layout.vue:281-282`) because update state is surfaced
  per-row instead.
- **Sort** — 4 modes: `Name (A-Z)` (default), `Name (Z-A)`, `Newest first`, `Oldest first`
  (`layout.vue:208-233`).
- Stale filter selections are auto-pruned when an option disappears (`content-filtering.ts:117`).
- `highlightedItemId` clears search + all filters when set, so a cross-page deep link always
  shows its target (`layout.vue:296`).

**Persistence:** `sessionStorage`. Keys are `content-sort:<instanceId>` and
`content-filters:<instanceId>` — **per instance, session-scoped, not synced and not on disk**.
INFERRED from `useSessionStorage` calls that these vanish on app restart; basis: the composable's
contract, and the fact that the page passes `instance.id` as `filterPersistKey`.

**Rationale & notes:** Putting frequency ordering first means "Mods" leads for a modpack but
"Resource packs" leads for a vanilla pack — the pill row reshapes itself to the instance rather than
presenting a fixed taxonomy the user mostly has to ignore. Enabled/Disabled pills only appear when
meaningful avoids a filter that filters nothing.

---

## INST-9 — Enable / disable toggle

**Type:** toggle
**Nav path:** `/instance/:id` (row switch)
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/content/index.vue:813] (`toggleDisableMod`), [SOURCE packages/app-lib/src/api/instance/files.rs — no; `packages/app-lib/src/state/content_store/domain/paths.rs:99`] (`file_path_on_disk`), [SOURCE packages/app-lib/src/state/instances/commands/content_mutation.rs:854] (`toggle_disable_project`)

**Purpose:** Turn a mod off without uninstalling it — the single most common content operation in
modded Minecraft. Implemented as a **file rename**, not a database flag alone, because loaders read
the directory.

**Operation:**
1. Frontend guards `canMutateContent(mod)` and calls `beginContentOperation(mod)` which locks the
   item against concurrent operations and marks it `installing` (`content/index.vue:618`).
2. Special case first: if the item belongs to a **synced pack** and that pack is enabled for this
   instance, the call is `set_synced_pack_enabled(...)` instead — toggling one member of a shared
   pack is a pack-level operation, not a file rename (`content/index.vue:824-831`).
3. Otherwise `toggle_disable_project(instanceId, file_path, desiredEnabled)` →
   `plugin:instance|instance_toggle_disable_project`.
4. Rust: `InstanceContent::lock` → `ContentChange::Toggle` → `prepare_toggle` → `commit`
   (`content_mutation.rs:860-867`). `commit` upserts the `InstanceFile` row with the new `enabled`
   flag (`content_mutation.rs:723-728`).
5. **Files touched:** the on-disk path toggles between `<name>.jar` and `<name>.jar.disabled`
   (`paths.rs:99-106` — `file_path_on_disk` appends `.disabled` when disabled). The frontend then
   rewrites `file_path`/`file_name`/`enabled` from the returned new path
   (`content/index.vue:832-847`).
6. Shared-instance side effect: if the user owns a shared instance, `reconcileSharedInstancePublishState()`
   re-fetches the publish preview so the owner's pending diff stays accurate
   (`content/index.vue:858-860`).
7. Telemetry: `InstanceProjectDisable` with loader, game version, project id/title/type, `disabled`.

**Controls:** blocked entirely for quarantined instances (tooltip "Content in locked instances cannot
be changed.", `content/index.vue:190-193`) and for `source_kind` items in a member-role shared
instance. Bulk enable/disable runs `Promise.all` over the selection
(`content/index.vue:1613-1628`). Disabling *managed* content on a shared instance triggers a
`ConfirmDisableModal` listing dependents (`content/index.vue:1184-1192`,
`use-managed-content-policy.ts:51`).

**Persistence:** the `InstanceFile.enabled` boolean in SQLite; the on-disk `.disabled` suffix is the
durable truth. No app setting involved.

**Rationale & notes:** Toggle-off is preferred over delete because modded Minecraft is iterative —
you disable a mod to bisect a crash, then re-enable it. Making it a rename means the *loader*
honours the state even if the app is closed, and it degrades gracefully when the instance folder is
moved to another launcher. The `enabled`/`disabled` state is derived from the path suffix everywhere
else in Rust too (`canonical_content_path`, `content_mutation.rs:888`), which keeps the DB and disk
in agreement.

---

## INST-10 — Update all

**Type:** button
**Nav path:** `/instance/:id`
**Evidence:** [SOURCE packages/ui/src/layouts/shared/content-tab/layout.vue:1375] (button render), [SOURCE packages/ui/src/layouts/shared/content-tab/layout.vue:893] (`openUpdateAll`), [SOURCE apps/app-frontend/src/pages/instance/content/index.vue:969] (`bulkUpdateSelections`)

**Purpose:** Bring a whole pack up to date without opening 40 modals.

**Operation:**
- Button only renders when `hasBulkUpdateSupport && hasOutdatedProjects`
  (`content-tab/layout.vue:1424`). Disabled while `isBulkOperating || loadingUpdateAll`.
- **Single-item shortcut:** if exactly one item is outdated *and* `ctx.updateItem` exists, clicking
  "Update all" opens the ordinary single-item updater modal instead of the bulk one
  (`content-tab/layout.vue:899-902`) — deliberately no extra dialog for the trivial case.
- Otherwise `UpdateAllModal` opens immediately (before any network completes) while versions load.
- Recommended target versions are fetched in parallel with per-project version lists, batched 8
  projects at a time (`content-tab/layout.vue:920-939`). Results are written into the query cache
  under `['labrinth','version','v2','summary',id]` and `['labrinth','versions','v2',projectId]`
  with a 5-minute stale time.
- A request-id guard (`updateAllRequestId`) discards results if the user cancels or re-opens.
- On confirm, `bulkUpdateSelections` maps selections to `{project_path, version_id}` and calls
  `install_bulk_update_content(instanceId, updates)` — a **single install job**, not N jobs
  (`content/index.vue:986`, `helpers/install.ts:267`). `bulkUpdatesInBackground: true` tells the
  table the work continues off-screen (`content/index.vue:1645`).

**Controls:** candidate set is filtered by `canChangeContentVersion(item)` — i.e. mutable **and not
frozen** (`content/index.vue:592`, `:978`). Frozen versions are excluded from Update all silently.
Downgrades are allowed (the version list is sorted newest-first, and any version is selectable).

**Persistence:** none for the button. Applied versions are written to the content store by the
install job; the update-check cache row is updated by `check_content_updates.rs`.

**Rationale & notes:** Sorting by `date_published` rather than by version number is a deliberate
choice — it makes "latest" mean "most recently published", which is what mod authors actually mean.
The single-item shortcut and the cache-priming look like micro-optimisations but together they are
the difference between "Update all feels instant" and a multi-second spinner.

---

## INST-11 — Browse content / Upload files (add content in context)

**Type:** button
**Nav path:** `/instance/:id`
**Evidence:** [SOURCE packages/ui/src/layouts/shared/content-tab/layout.vue:1449] and `:1463] (buttons), [SOURCE apps/app-frontend/src/pages/instance/content/index.vue:708] (`handleBrowseContent`), [SOURCE apps/app-frontend/src/pages/instance/content/index.vue:713] (`handleUploadFiles`)

**Purpose:** Two distinct acquisition routes sitting side by side because they serve different
confidence levels: Browse = known-good Modrinth content; Upload = a jar you already have.

**Operation:**
- **Browse content** → `router.push('/browse/<type>', { query: { i: instanceId } })` where `<type>`
  is `resourcepack` for vanilla instances and `mod` otherwise (`content/index.vue:708-711`, and the
  identical logic in `layout.vue:786-795`). The `i` query param is what the Browse page uses to
  know the install target.
- **Upload files** → native multi-file dialog (`open({ multiple: true })`), then for each selected
  file `is_file_on_modrinth(path)` is called. Files **not** recognised trigger an
  `UnknownFileWarningModal` naming the file (`content/index.vue:727-743`). If the modal's
  "don't show again" is ticked, `appSettings.warnOnUnknownModpacks = false` is written to the
  settings store *and* persisted to the Rust settings row (`content/index.vue:799-811`).
- Recognised (or confirmed) files are installed with `add_project_from_path`; results are deduped by
  returned `installedPath` and reported as one notification ("{name} was added" vs "{count} projects
  were added") (`content/index.vue:745-777`).
- Drag-and-drop onto the webview is a third implicit route to the same install
  (`content/index.vue:1767`); `.mrpack` drops are filtered out.

**Controls:** both buttons are disabled for quarantined instances via `disableAddContent` /
`disableAddContentTooltip` (`content/index.vue:1607-1608`). The unknown-file warning itself is
globally suppressible.

**Persistence:** `AppSettings.warn_on_unknown_modpacks` (global, persisted).

**Rationale & notes:** The unknown-file warning is the single most important safety affordance here:
dropping a random `.jar` into a pack is the classic way to get malware. Putting it on *upload*
(and drag-drop) rather than on install-from-browse is correct because browse results are
authenticated Modrinth artifacts.

---

## INST-12 — Per-item overflow menu

**Type:** menu
**Nav path:** `/instance/:id` (row overflow)
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/content/index.vue:1497] (`getOverflowOptions`)

**Purpose:** The rare-but-real operations that must not clutter every row.

**Operation:** built per item and consumed by the row's `TeleportOverflowMenu`:
1. **Synced-pack actions first** (`packActions.overflowOptions(item)`) — e.g. promote/demote or
   open pack settings when the item belongs to a synced pack (`content/index.vue:1498`).
2. **Show file** → `highlightModInInstance(instanceId, file_path)`, which pushes
   `/instance/:id/files?highlight=<path>` and makes the Files tab auto-open that row — a cross-tab
   deep link (`content/index.vue:1500-1505`).
3. **Copy link** → clipboard write of `https://modrinth.com/<project_type>/<slug>`, only when the
   project has a slug (`content/index.vue:1507-1518`).
4. Divider, then **Freeze version** / **Unfreeze version** → `set_project_locked(instanceId, path, frozen)`.
   Frozen items cannot be updated or version-switched (`canChangeContentVersion` checks `!item.locked`,
   `content/index.vue:592`) (`content/index.vue:1520-1530`).

**Controls:** freeze/unfreeze and everything else is hidden when `canMutateContent(item)` is false.
`item.locked` also flips the row's own version-switch button off
(`mapToTableItem.hideSwitchVersion`, `content/index.vue:1697`) and suppresses `hasUpdate`
(`content/index.vue:1698`).

**Persistence:** `InstanceFile.locked` in SQLite, per file, per instance. Not a global setting.

**Rationale & notes:** "Freeze version" is a genuinely well-judged feature name — it conveys
"pin this exact version, updates stop" in one word, and the lock/unlock icon pair makes the toggle
reversible at a glance. It solves the real problem of a pack that must stay reproducible across
other people's updates. Note the consistent convention that mutating actions are only offered when
they would actually succeed, rather than offered-and-disabled.

---

## INST-13 — How an available version update is surfaced

**Type:** field (derived state)
**Nav path:** `/instance/:id`
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/layout.vue:242] (`instanceKeys.contentUpdateCheck` query), [SOURCE packages/app-lib/src/state/instances/content.rs:20] (`ContentItem.has_update` / `update_version_id`), [SOURCE packages/app-lib/src/state/instances/model/update_check.rs:6] (`ContentUpdateCheck`), [SOURCE packages/app-lib/src/state/instances/commands/check_content_updates.rs:44] (`refresh_content_updates`)

**Purpose:** Tell the user, without them asking, which installed content has a newer version that
their update channel permits — the backbone of both the per-row update affordance and Update all.

**Operation:**
- A dedicated query keyed `['instances','content-update-check', <id>]` runs
  `refresh_content_updates(instanceId)` on mount (`layout.vue:242-254`). It is **gated on a
  preceding successful `sync_content_files`** so the file list is current before versions are
  compared, on `!offline`, and on `install_stage === 'installed'`.
- Throttle: `staleTime: 10 * 60_000`, `gcTime: 30 * 60_000`, `retry: false` (`layout.vue:247-250`).
- Rust runs it with `CacheBehaviour::Bypass` (`check_content_updates.rs:51`), so every check
  actually hits the API rather than a cached version list.
- Each checked entry writes a `ContentUpdateCheck { content_entry_id, update_channel,
  update_version_id, checked_at }` row (`update_check.rs:6`). `update_channel` is recorded, so the
  result is channel-scoped.
- `list_content` then surfaces `has_update: bool` + `update_version_id: Option<String>` on every
  `ContentItem` (`content.rs:20-21`). The frontend overwrites `has_update` with the *policy*
  answer (`canUpdateProject(item)`) rather than the raw flag (`content/index.vue:1578`), so a
  frozen or read-only item never shows an update badge even if a newer version exists.
- Watching `update_channel` re-runs `initProjects('must_revalidate')`
  (`content/index.vue:1809-1816`) — changing the channel immediately re-evaluates which versions
  count as updates.

**Controls:** the update channel is Release / Beta / Alpha, a Chips group in Settings → General
(`general-settings.vue:402`). Per-channel copy: release = "Only release versions will be shown as
available updates." (`general-settings.vue:247-258`).

**Persistence:** per-instance `update_channel` (instance row); per-check cache rows in SQLite.

**Rationale & notes:** The two-phase gate (`sync_content_files` must succeed first, then update
check) is a correctness decision that is easy to miss: comparing versions against a stale file
list produces phantom "update available" badges. The 10-minute throttle plus `networkMode: 'always'`
on the content query is a deliberate trade of a little extra network for never showing stale state.

---

## INST-14 — Worlds list, search, and filters

**Type:** tab
**Nav path:** `/instance/:id/worlds`
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/worlds/index.vue:24] (search input), [SOURCE apps/app-frontend/src/pages/instance/worlds/index.vue:624] (`filterOptions`), [SOURCE apps/app-frontend/src/pages/instance/worlds/index.vue:660] (`filteredWorlds`)

**Purpose:** Present every way to play this pack — saves and servers — in one ordered list that can
be searched and filtered, since a large pack accumulates dozens of servers.

**Operation:**
- Worlds load from `instanceWorldsQueryOptions` → `refreshWorlds` (`worlds/index.vue:279-284`).
- On load, each *server* row is pinged via `refreshServers` to fill MOTD / player count / status
  (`worlds/index.vue:306-324`).
- **Dedup:** `dedupedWorlds` collapses duplicate server entries by normalized address; if both a
  managed and an unmanaged entry share an address, the managed one wins
  (`worlds/index.vue:591-622`). This is why a Modrinth-hosted server does not appear twice when it
  is also in the user's server list.
- **Search:** `worldNameMatchesQuery(x.name, searchFilter)` — a fuzzy name match, not a substring
  (`worlds/index.vue:662`).
- **Refresh (the ↻ button):** re-invalidates the worlds query and re-pings every server, with a
  spinner on each server row immediately (`worlds/index.vue:433-462`). The button is disabled while
  a refresh is in flight.
- **Auto-refresh:** an `instance` app event with `event === 'servers_updated'` triggers a full
  refresh — but **capped at 3 refreshes on Linux** to avoid server ping spam
  (`MAX_LINUX_REFRESHES = 3`, `worlds/index.vue:292-296`, `:394-399`).

**Controls:**
- Search placeholder is count-aware: "Search {count} worlds..." (`worlds/index.vue:195-198`),
  clearable, debounced by the Input component.
- Filter pills are **dynamically generated** (`worlds/index.vue:624-650`): "Singleplayer" only if
  both types exist; "Vanilla"/"Modded" only if servers of both `content_kind` values exist;
  "Online"/"Offline" only if some servers are up and some are down. Selecting Singleplayer
  mutually excludes Online/Offline and vice-versa (`updateFilters`, `worlds/index.vue:262-270`).
- A watcher prunes selected filters whose option no longer exists (`worlds/index.vue:652`).
- Empty state: "No servers or worlds added" / "Add a server or browse to get started", with the
  two add buttons inline (`worlds/index.vue:207-214`, `:109-125`).
- Singleplayer rows show the world name, game mode (via `GAME_MODES[world.game_mode]`), and a
  "locked" state; servers show MOTD, players, and status (`worlds/index.vue:87-90`).

**Persistence:** none in the tab itself. Server membership persists via
`multiplayer_servers` synced option + the instance's server list (INST-20).

**Rationale & notes:** Linux is capped at 3 auto-refreshes while Windows/macOS are not — an
explicit, commented concession to the fact that the Linux status poller fires far more often, and
that pinging a server list is externally visible. Making every filter pill conditional on the data
means the row never offers a filter that would return nothing. Rows also carry a
`highlighted` state fed from `route.query.highlight`, the same cross-tab deep-link mechanism as
"Show file" in INST-12.

---

## INST-15 — Play world / play server

**Type:** button
**Nav path:** `/instance/:id/worlds`
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/worlds/index.vue:535] (`joinWorld`), [SOURCE apps/app-frontend/src/pages/instance/worlds/index.vue:299] (`hasServerQuickPlaySupport`)

**Purpose:** Launch the instance directly *into* a specific world or server, rather than launching
to the title screen and navigating manually.

**Operation:**
- Blocked entirely if `instance.quarantined` (`worlds/index.vue:536`).
- **Managed server** (the linked Modrinth server project): `playServerProject(projectId)` — starts a
  remote server instance rather than a local launch (`worlds/index.vue:541-551`).
- **Regular server:** `start_join_server(instanceId, world.address)`.
- **Singleplayer:** `start_join_singleplayer_world(instanceId, world.path)`.
- All paths emit `InstanceStart` with `source: 'WorldsPage'` (servers only — singleplayer play does
  not emit it, `worlds/index.vue:558-560`).
- Capability gating: quick-play args only exist on newer Minecraft protocol versions, computed from
  `get_instance_protocol_version` + the global game-version list
  (`worlds/index.vue:299-304`, `:404-419`). `supportsWorldQuickPlay` / `supportsServerQuickPlay`
  are passed down to the row to hide/disable Play where unsupported.
- Errors route to `handleSevereError` and reset the row's optimistic "starting" state
  (`worlds/index.vue:529`).
- When the running process ends, every **locked** singleplayer world is re-read after a 1 s delay
  so its metadata reflects the final save (`worlds/index.vue:565-580`).

**Controls:** per-row Play/Stop; a second Stop button appears once launched
(`worlds/index.vue:93`).

**Persistence:** none.

**Rationale & notes:** Quick-play support being *detected per instance* rather than assumed means
the same UI works across the whole 1.7→1.21 range without version-sniffing in the UI. The
`startingInstance` flag exists so the row can show a spinner before the process actually appears in
the process list — the process query is polled, so there is a real window where "launched" has been
requested but not observed.

---

## INST-16 — Edit world / delete world

**Type:** button
**Nav path:** `/instance/:id/worlds`
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/worlds/index.vue:95] (row handler), [SOURCE apps/app-frontend/src/pages/instance/worlds/index.vue:510] (`editWorld`), [SOURCE apps/app-frontend/src/pages/instance/worlds/index.vue:524] (`deleteWorld`)

**Purpose:** Rename a world (including removing its custom icon) or permanently delete a save.

**Operation:**
- Edit opens `EditWorldModal` with `(path, name, removeIcon)`; on submit the local array is mutated
  and re-sorted, or — if the world is not found locally — an error plus a full refresh
  (`worlds/index.vue:510-522`).
- Delete opens `ConfirmRemoveWorldModal`, then `delete_world(instanceId, world.path)` and removes
  the row from the local list (`worlds/index.vue:524-527`, `:702-715`).
- Managed servers are exempt from both: `@edit` returns `undefined` and `@delete` is a no-op when
  `isManagedServerWorld(world)` (`worlds/index.vue:95-103`).

**Controls:** the confirm modal is mandatory for delete.

**Persistence:** world name lives in the save's own metadata (edit modal owns the write); deletion
removes the world folder.

**Rationale & notes:** The optimistic-local-update-then-refresh-on-miss pattern appears three times
here (edit server, edit world, remove server) and is a reasonable response to a Rust API that has no
per-entity mutation endpoint — the list is cheap to refetch.

---

## INST-17 — Export world

**Type:** button
**Nav path:** `/instance/:id/worlds`
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/worlds/index.vue:23] (`WorldItem` list render; the export affordance lives inside `apps/app-frontend/src/components/ui/world/WorldItem.vue`)

**Purpose:** Back up or share a single save without packaging the whole instance.

**Operation:** [ORCH-PENDING] — the export affordance is rendered inside the shared `WorldItem`
component (`apps/app-frontend/src/components/ui/world/WorldItem.vue`), which is outside the
per-instance page tree this slice owns. The orchestrator must open an instance → Worlds, hover a
singleplayer row, and confirm: (a) the exact label of the action, (b) whether it writes a `.zip`
and where, and (c) whether it is offered for server rows.

**Controls:** [ORCH-PENDING]

**Persistence:** [ORCH-PENDING]

**Rationale & notes:** Leaving this one open is deliberate — the evidence for the control set does
not live in the files I was scoped to, and guessing would violate the clean-room rule.

---

## INST-18 — Open folder (world)

**Type:** button
**Nav path:** `/instance/:id/worlds`
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/worlds/index.vue:105] (`@open-folder="(world: SingleplayerWorld) => showWorldInFolder(instance.id, world.path)"`)

**Purpose:** Reveal the world folder in the OS file manager — for manual backup, manual edits, or
diagnosing "the launcher says the save is corrupt".

**Operation:** `showWorldInFolder(instanceId, world.path)` opens the platform file manager with the
save selected. Singleplayer only (the handler is typed `SingleplayerWorld`).

**Controls:** row overflow, singleplayer rows only.

**Persistence:** none.

**Rationale & notes:** Offering this on the *world* rather than only on the instance folder is a
good granularity choice — instance-level "Open folder" (INST-44) puts you at the instance root and
makes you hunt for `saves/<world>`.

---

## INST-19 — Add server / Browse servers

**Type:** button
**Nav path:** `/instance/:id/worlds`
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/worlds/index.vue:39] (`addServerModal.show()`), [SOURCE apps/app-frontend/src/pages/instance/worlds/index.vue:43] (`instancePage.browseServers`), [SOURCE apps/app-frontend/src/pages/instance/layout.vue:797] (`browseServers` → `/browse/server?i=<id>&from=worlds`)

**Purpose:** Get a server into the list — either by typing an address, or by discovering one from
the directory.

**Operation:**
- **Add server** → `AddServerModal`; on submit the server is pushed into the local array, the list
  re-sorted, and the new address immediately pinged. `AddServerModal` takes a `start` boolean so
  "add and join" is a single submit (`worlds/index.vue:3-13`, `:464-468`).
- **Browse servers** → `/browse/server?i=<instanceId>&from=worlds`. The `from=worlds` param tells the
  Browse page to return here rather than to the instance root.

**Controls:** the brand-coloured "Browse servers" is the primary action; "Add server" is `outlined`,
de-emphasised.

**Persistence:** the server address lands in the instance's server list (via the modal's own
backend call); membership additionally depends on the `multiplayer_servers` synced option (INST-20).

**Rationale & notes:** "Add and join in one submit" removes a real friction step — the most common
intent when adding a server is to play it immediately.

---

## INST-20 — Desync server

**Type:** button
**Nav path:** `/instance/:id/worlds`
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/worlds/index.vue:104] (`@desync`), [SOURCE apps/app-frontend/src/pages/instance/worlds/index.vue:504] (`confirmDesyncServer`), [SOURCE apps/app-frontend/src/pages/instance/worlds/index.vue:485] (`removeServer` with scope)

**Purpose:** Remove a server that came from a synced source (a shared instance or a synced pack)
without touching the source, so this instance stops tracking it.

**Operation:**
- `DesyncServerModal` → `desync_server(instanceId, server.server_id, mode)` then a full refresh
  (`worlds/index.vue:504-508`).
- `removeServer(server, scope)` handles two scopes (`worlds/index.vue:485-502`):
  - `scope === 'here'` and the server's `source === 'user_synced'` → `set_synced_option(instanceId,
    'multiplayer_servers', false)` first, so the sync stops propagating, *then*
    `remove_server_from_instance(instanceId, server.index)`.
  - otherwise → `remove_server_from_instance` only.
- Always followed by invalidating `instanceKeys.all` and `['instance-synced-options']` and a full
  refresh.

**Controls:** only rendered for server rows; the modal picks the mode.

**Persistence:** `multiplayer_servers` per-instance synced flag + the instance's server list entry.

**Rationale & notes:** The two-scope model ("here" vs "all") is a good answer to a real ambiguity:
deleting a synced server from every instance is a different, more destructive action than removing
your copy. Doing the sync-flag change **before** the removal (in that order) avoids a window where
the server could be re-added by the next sync.

---

## INST-21 — Screenshots viewer and actions

**Type:** tab
**Nav path:** `/instance/:id/screenshots`
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/screenshots/index.vue:10] (`<ScreenshotsPage :instance-id="..." />`), [SOURCE apps/app-frontend/src/components/ui/screenshots-page/index.vue:146] (`isGlobal`), [SOURCE packages/app-lib/src/api/instance/screenshots/operations.rs:513] (`source_screenshots_dir`)

**Purpose:** Reuse the full global Screenshots experience, scoped to one instance.

**Operation:** the instance route is a 3-line wrapper that passes `instancePage.instanceId` into the
shared `ScreenshotsPage` (`screenshots/index.vue:9-11`). The same component with no `instanceId`
powers the global `/screenshots` page; `isGlobal` switches the query and the persisted view-state
suffix (`screenshots-page/index.vue:146-147`). Instance mode uses
`instanceScreenshotsQueryOptions(instanceId)` (`screenshots-page/index.vue:324`).

**Data model:** `InstanceScreenshot[]` plus `ScreenshotGroup[]`
(`{id, title, screenshots, dropInstanceId?, customGroupId?}`, `screenshots-page/index.vue:93-99`).
Groups can be backed by an instance (`instance:<id>`, line 464) or a custom user-created group.
Multi-select drives a `FloatingActionBar` with **Export ZIP**, **New group**, **Remove from group**,
and **Delete** (`screenshots-page/index.vue:286-309`). Per-card actions: **Copy image**
(clipboard, with a 2 s "Copied" confirmation reset, `screenshots-page/index.vue:1260-1272`),
**Edit screenshot** (opens `ImageViewerEditor`), and an overflow **more** menu
(`card.vue:253-272`) containing **Show in folder**, **Go to instance**, and **Delete**
(labels at `screenshots-page/index.vue:273-283`). Drag-and-drop between groups and onto instance
group targets is supported (`screenshots-page/index.vue:1281+`).

**Controls:** Search; Sort by `Newest` / `Oldest` / `Name`; Group by `Custom group` / `Instance` /
`Date` / `No grouping` (`screenshots-page/index.vue:247-253`, toolbar at `toolbar.vue:26-31`).
Date grouping buckets into Today / Yesterday / This week / This month (`screenshots-page/index.vue:254-257`).
Default ZIP filename: `Modrinth screenshots.zip` globally, `{instance} screenshots.zip` when scoped
(`screenshots-page/index.vue:298-306`).

**Persistence:** view state in `localStorage` (via `useStorage`) with the key suffix
`instance` vs `global` (`screenshots-page/index.vue:147`). Group names capped by
`MAX_INSTANCE_GROUP_NAME_LENGTH` (`screenshots-page/index.vue:70`). Image files live at
`<instances_dir>/<instance path>/screenshots/` — `canonicalize`d, with an explicit
`ensure_directory_is_not_symlink` check (`operations.rs:513-536`).

**Rationale & notes:** The single-component/single-prop design means the instance tab is guaranteed
to stay feature-identical to the global page — no drift is possible. Local (not session) storage is
chosen because screenshot view preferences are the kind of thing users expect to survive restarts.
The symlink guard on the screenshots directory is a defence against a crafted instance folder
redirecting writes outside the instances root.

---

## INST-22 — Logs: Live Log source

**Type:** tab entry
**Nav path:** `/instance/:id/logs`
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/logs/index.vue:52] (`{ name: 'Live Log', live: true }`), [SOURCE apps/app-frontend/src/pages/instance/logs/index.vue:194] (`useAppEvent('log', …)`)

**Purpose:** Show the running (or just-finished) game output as it streams.

**Operation:**
- The live source is **always index 0** of the log-source combobox and is synthetic — it is not a
  file (`logs/index.vue:52`, `:87`).
- Lines arrive via the Tauri event bus, not polling: `useAppEvent('log', payload)` handles
  `type === 'log4j'` (structured events) and `type === 'legacy'` (raw stdout/stderr strings)
  (`logs/index.vue:194-202`). The `useInstanceConsole` composable buffers them into `liveConsole`.
- On `process` `launched`: the live console is cleared, the historical list is invalidated and
  refetched, and the selection resets to Live Log (`logs/index.vue:204-211`).
- On `process` `finished`: historical list refetched and crash analysis re-run
  (`logs/index.vue:212-217`).
- While the instance is playing, the historical entry named `latest.log` is hidden from the source
  list (`filteredLogs`, `logs/index.vue:89-93`) because it would duplicate the live stream.
- Initial hydration is a suspense-awaited query (`consoleHydrationQuery`,
  `logs/index.vue:38-48`) so the terminal is populated before first paint.
- **Rendering:** `ConsolePageLayout` → `BaseTerminal` (xterm). Lines are appended incrementally by
  tracking `lastWrittenIndex` and only writing the delta; a full `rewriteTerminal` only happens on
  a source switch, a filter change, a shrink, or a load-state transition
  (`console/layout.vue:343-386`). Searches debounce at 200 ms (`console/layout.vue:388-393`).

**Controls:** Search box ("Search logs"), a log-source combobox (220 px wide), level filter pills
(`ConsoleFilterPills`, with `debug`/`trace` pills only appearing if those levels are actually
present — `console/layout.vue:167-175`), Clear (live only), Fullscreen (Esc to exit,
`console/layout.vue:308-312`), Share, Delete (historical only).

**Persistence:** none for the live buffer; it is in-memory in Rust and cleared on launch.

**Rationale & notes:** Event-driven tailing rather than cursor polling is the right call for a
Tauri app — no wakeups, no race between "read" and "write", and the redraw strategy means a
10k-line crash dump does not re-render 10k lines on every keystroke in the search box. Hiding
`latest.log` while running is a small but thoughtful de-duplication.

---

## INST-23 — Logs: historical log files and crash reports

**Type:** tab entry (list)
**Nav path:** `/instance/:id/logs`
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/logs/index.vue:50] (`buildLogList`), [SOURCE packages/app-lib/src/api/logs.rs:270] (`get_logs_from_type`), [SOURCE packages/app-lib/src/api/logs.rs:26] (`enum LogType`)

**Purpose:** Keep access to previous sessions' output and to crash reports, because the useful
evidence for "it crashed last Tuesday" is almost never the current run.

**Operation:**
- `getHistoricalLogs` → `plugin:logs|logs_get_logs` returns logs from **both** folders in one list
  (`logs.rs:319`, scanning `InfoLog` then `CrashReport`).
- Frontend filter (`logs/index.vue:50-67`) excludes `latest_stdout.log`, `latest_stdout`,
  `launcher_log.txt`, empty outputs, and anything not ending `.log`/`.txt`. The first three are
  excluded because they duplicate the live stream.
- Selecting a source: cached content is used if present, else `get_output_by_filename(instanceId,
  log_type, filename)` is fetched, cleared into `historicalConsole` via `addLegacyLog`
  (`logs/index.vue:167-186`).
- **Data model:** `Logs { log_type: LogType, filename, age: u64, output: Option<CensoredString> }`
  (`logs.rs:18-24`); `LogType` is `InfoLog | CrashReport` (`logs.rs:26-30`). Sorted by age
  descending, then filename (`logs.rs:340`).
- **Storage:** `state.directories.instance_logs_dir(&instance_path)` for InfoLog and
  `state.directories.crash_reports_dir(&instance_path)` for CrashReport (`logs.rs:280-284`).
- **Censoring:** every returned output is passed through `CensoredString::censor`, which replaces
  the OS username with `{COMPUTER_USERNAME}` and every known credential (access token, offline
  profile name/UUID in both simple and hyphenated forms) with `{MINECRAFT_ACCESS_TOKEN}`,
  `{MINECRAFT_USERNAME}`, `{MINECRAFT_UUID}` (`logs.rs:44-71`).
- **Compaction:** runs of ≥20 identical consecutive lines are collapsed into a summary
  (`LOG_COMPACTION_THRESHOLD = 20`, `logs.rs:32`, `:123-131`) — this is what keeps a
  log-spamming mod from producing an unreadable console.

**Controls:** source combobox; selection resets to Live Log after delete.

**Persistence:** on disk only.

**Rationale & notes:** Censoring *before* the data crosses the IPC boundary (not at upload time)
means the Share button cannot leak a token even if someone adds a new consumer. Using the offline
profile for the replacement guarantees no Mojang API request is made on the hot path of displaying
a log. Log compaction at 20 is aggressive but correct for the common mod-spam case.

---

## INST-24 — Logs: Clear / Delete / Share

**Type:** button
**Nav path:** `/instance/:id/logs`
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/logs/index.vue:136] (`deleteSelectedLog`), [SOURCE apps/app-frontend/src/packages → packages/ui/src/layouts/shared/console/layout.vue:43] (`ConsoleActionButtons`), [SOURCE packages/ui/src/layouts/shared/console/layout.vue:437] (`handleShare`)

**Purpose:** Clear the live view, delete a retained log file, or upload the (filtered) view to
mclo.gs to paste into a bug report.

**Operation:**
- **Clear** — only shown for the live source (`showClear: isLiveSource`,
  `console/layout.vue:44`). Resets the terminal, zeroes `lastWrittenIndex`, and calls
  `clearLive()` which invokes `plugin:logs|logs_clear_live_log_buffer` — i.e. it clears the **Rust
  ring buffer**, not just the terminal (`logs/index.vue:152-155`, `helpers/logs.js:72`). A no-op for
  historical sources.
- **Delete** — hidden for the live source (`showDelete`, `console/layout.vue:258`). Opens a
  `NewModal` header "Delete log file", `fade="danger"`, containing an Admonition "This is
  irreversible" (`console/layout.vue:77-95`). On confirm: `delete_logs_by_filename(instanceId,
  log_type, filename)` → `plugin:logs|logs_delete_logs_by_filename` → the file is removed from disk;
  the list is rebuilt and the selection resets to Live Log (`logs/index.vue:136-144`).
- **Delete is blocked on `latest.log` while the instance is running** — `deleteDisabled` returns true
  and the tooltip is the hardcoded string "Cannot delete latest.log while the instance is running"
  (`logs/index.vue:130-134`, `:158`).
- **Share** — builds content from the lines *currently visible after filters*:
  `lines.filter(predicate).map(l => l.text).join('\n')`, uploads via
  `client.mclogs.logs_v1.create(content)`, and shows the returned URL in a Share modal
  (`console/layout.vue:437-458`). Disabled when `instancePage.offline`
  (`logs/index.vue:159`).

**Controls:** the delete confirm modal; the filter-aware share.

**Persistence:** delete removes the file; clear empties an in-memory ring buffer.

**Rationale & notes:** Sharing the *filtered* view rather than the whole buffer is a genuinely good
call — a user who filtered to `ERROR` wants to share exactly that. The `latest.log` guard exists
because deleting the file Minecraft has open on Windows fails at the OS level anyway; blocking it
with an explanation beats a raw IO error. Note the tooltip is a **hardcoded English string**, not a
message ID — an i18n inconsistency.

---

## INST-25 — Logs: crash analysis

**Type:** banner (admonition)
**Nav path:** `/instance/:id/logs`
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/logs/index.vue:113] (`analyseForCrash`), [SOURCE packages/ui/src/layouts/shared/console/layout.vue:8] (`CollapsibleAdmonition type="critical"`), [SOURCE packages/ui/src/layouts/shared/console/layout.vue:139] (`crashHeader`)

**Purpose:** Turn a wall of stack traces into "here are 2 problems detected" with actionable fixes.

**Operation:**
- `analyseForCrash()` joins the live console lines with `\n` and calls
  `client.mclogs.insights_v1.analyse(content)`. Results are shown only if
  `data.analysis?.problems?.length > 0` (`logs/index.vue:113-126`). Wrapped in a silent `catch` —
  best-effort by design.
- Triggers: on mount **if the instance is not playing** (`logs/index.vue:190-192`), and on
  `process` `finished` (`logs/index.vue:216`).
- Rendered as a dismissible `CollapsibleAdmonition` of type `critical`, header
  "{count} problem{s} detected", each item's `title` = problem `message` and `descriptions` =
  that problem's `solutions[].message` (`console/layout.vue:8-15`, `:139-151`).
- Dismissal sets `crashAnalysis = null` (`logs/index.vue:162-164`).

**Controls:** dismissible; hidden entirely in fullscreen mode.

**Persistence:** none — the analysis is not stored.

**Rationale & notes:** Firing on `finished` rather than only on mount is what makes this feel
intelligent: you press Play, it crashes, and the diagnosis is already on screen when you come back
to Logs. Analysing on mount when not playing covers the "left it crashed overnight" case. Running
this silently on every Logs tab open is acceptable because the endpoint is cheap and failures are
invisible.

---

## INST-26 — Files browser

**Type:** tab
**Nav path:** `/instance/:id/files`
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/files/index.vue:65] (`listDirectory`), [SOURCE apps/app-frontend/src/pages/instance/files/index.vue:305] (`provideFileManager`), [SOURCE packages/app-lib/src/api/instance/files.rs:96] (`list_instance_files`)

**Purpose:** Full read/write access to the instance directory, for everything the launcher does not
model.

**Operation:** every capability is a Tauri invoke against the `files` plugin
(`apps/app/src/api/files.rs:10-20`):
- `file_list` — directory listing; `file_read` / `file_read_as_blob` — read; `file_write` — write
  (with a `createOnly` flag for "new file"); `file_create_directory`; `file_rename` (used for both
  rename **and** drag-move); `file_delete` (with a `recursive` flag); `file_save_as`; and
  `file_extract_zip` with an `overrideConflicts` + `dryRun` pair (`files/index.vue:135-287`).
- Upload: files are read into memory as `ArrayBuffer` and written whole — **there is no streaming
  upload**; progress is simulated per-file (`uploadState`, `files/index.vue:228-270`).
- Live refresh on the `instance` `synced` event (`files/index.vue:289`).
- The instance root comes from `get_full_path(instanceId)`, cached with `staleTime: Infinity`
  (`files/index.vue:46-53`).

**Controls:** breadcrumb navigation with a Home root; New file / New folder / Upload file /
Upload from .zip file / Upload from .zip URL / Install CurseForge pack (navbar labels at
`files-tab/components/FileNavbar.vue:268-292`); per-row rename, move, delete, "Find in file",
"Share to mclo.gs", and Show-in-folder; a text editor for small text files; refresh button
(`files/index.vue:339` sets `show-refresh-button`).

**Persistence:** none. All state is on disk in the instance directory.

**Rationale & notes:** `file_rename` doubling as the drag-move primitive keeps the plugin surface
small. The `dryRun` on zip extraction is a real UX win: the UI can list the conflicting files
before writing anything (`files.rs:93-103`).

---

## INST-27 — Files search

**Type:** field
**Nav path:** `/instance/:id/files`
**Evidence:** [SOURCE packages/ui/src/layouts/shared/files-tab/composables/file-search.ts:9]

**Purpose:** Find a file by name in a directory listing that is otherwise sorted and browsable.

**Operation:** `useFileSearch` filters the *current directory's* items by
`item.name.toLowerCase().includes(query)` (`file-search.ts:9-14`). There is no recursion and no
server-side involvement.

**Controls:** clearable search input, placeholder "Search files".

**Persistence:** none (local `ref`).

**Rationale & notes:** This is the weakest part of the Files tab: search is **current-directory
only**, so finding `saves/<world>/level.dat` from the instance root requires clicking through.
Breadcrumbs make that tolerable, but it is a real limitation worth noting rather than defending.

---

## INST-28 — Files: read-only enforcement and path safety

**Type:** constraint (backend-enforced)
**Nav path:** `/instance/:id/files`
**Evidence:** [SOURCE packages/app-lib/src/api/instance/files.rs:30] (`resolve`), [SOURCE packages/app-lib/src/state/content_store/domain/paths.rs:21] (`validate_relative`), [SOURCE apps/app-frontend/src/pages/instance/files/index.vue:69] (`isReadOnly`)

**Purpose:** Make it structurally impossible to corrupt an instance from the Files tab, and
impossible to escape the instance directory.

**Operation — all of this is enforced in Rust, not in the UI:**
1. `normalized(path)` strips a leading `/` (`files.rs:26-28`).
2. **`validate_relative`** rejects any path that is empty, contains `\` or `:`, has an empty /
   `.` / `..` segment, has a segment ending in `.` or a space, or has any non-`Normal` `Path`
   component (`paths.rs:21-39`). This is what blocks `../` traversal and Windows drive letters.
3. **Empty path (the instance root) cannot be written** — "The instance directory cannot be changed
   in the Files tab" — and the root itself must not be a symlink (`files.rs:44-56`).
4. **`mods/` is hard-blocked for writes** — "The mods folder is read-only in Files. Manage mods from
   the Content tab" (`files.rs:58-67`). This is the single most important rule: it prevents the
   Files tab from desynchronising the content database.
5. **Managed content is read-only** — if the target path matches a DB-bound instance file (by
   `relative_path`, resolved content path, or prefix for a directory), writes are refused with
   "Managed content is read-only in Files. Use the Content tab" (`files.rs:68-82`).
6. **Symlinks are refused for both read and write**, at the root (`files.rs:50`) and at any
   destination (`files.rs:88-92`), with "Files cannot access a symbolic link or its target".
7. Zip extraction re-validates the archive path and **every entry target** through
   `validate_instance_file_write`, and rejects absolute or backslash-containing entry names
   ("Invalid archive path") — classic zip-slip defence (`apps/app/src/api/files.rs:72-91`).
8. `write_instance_file` takes both an instance-content lock and a files lock before writing
   (`files.rs:188-189`), and `create_only` refuses to clobber an existing file
   ("A file already exists at this path", `files.rs:194-196`).
9. The **UI mirrors** this: `isReadOnly(path)` returns true for `mods/**` or any row the backend
   flagged `readOnly`, and the reason string is "Manage your installed content via the Content tab"
   (`files/index.vue:69-75`, `:307`).

**Controls:** the read-only tooltip is a single global message; the Rust error strings are more
specific per rule.

**Persistence:** none — this is policy, not state.

**Rationale & notes:** The `read_only` flag is computed server-side per listing entry by *attempting
a write-resolution* (`resolve(..., true).is_err()`, `files.rs:117-118`), so the UI can never
disagree with the backend about what is writable. Rule 5's prefix check means entering a directory
that contains managed content marks the whole directory read-only, which is the correct conservative
choice. Denying symlinks outright (rather than resolving them) is a deliberate sandboxing decision.

---

## INST-29 — Share: invite friends / invite link

**Type:** tab
**Nav path:** `/instance/:id/share`
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/share/index.vue:4] (`InvitePlayersModal`), [SOURCE apps/app-frontend/src/pages/instance/share/use-shared-instance-invite-link.ts] (link lifecycle)

**Purpose:** Get other people into this instance, either by account (friends picker / user search)
or by link/code (for people without an account relationship).

**Operation:**
- The tab renders one of four mutually exclusive states: `unableToConnect` empty state, a members
  table, an "unavailable" empty state, an "actions locked" empty state (wrong/signed-out account),
  or the "no friends invited" empty state with the Invite button (`share/index.vue:50-127`).
- **Invite modal** combines a friends list, a user search, and a shareable link in one dialog
  (`share/index.vue:4-27`). `useSharedInstanceInviteCandidates` supplies friends + search;
  `useSharedInstanceInviteLink` owns the link's expiry and max-uses.
- The link's `maxUses` is **capped at the remaining slot count** — `remainingUserSlots =
  SHARED_INSTANCE_USER_LIMIT - members.length`, and `linkMaxUsesLimit` is passed to the modal
  (`share/index.vue:12`, `:219-222`). A new invite link is generated when you edit its settings.
- Invite is disabled while members are mid-mutation or a link update is pending
  (`can-invite`, `share/index.vue:15-19`).
- Account-required flows raise `ModrinthAccountRequiredModal` (`share/index.vue:3`).

**Controls:** the invite button is `colored`/`brand`/`lg` in the empty state; the members table is
the primary view once there are members. Invite copy strings mention the instance name
(`shareModalHeader` with `{name}`).

**Persistence:** server-side. Invite codes live at `['sharedInstanceInvites', <instanceId>]` with
`staleTime: Infinity`, `refetchOnMount: 'always'`, `retry: false` (`sharing-settings.vue:125-141`).
Member rows at `instanceKeys.sharedMembers(<id>)`.

**Rationale & notes:** The four-state empty-state design is the most considered part of this tab:
network down, loading, shared instance gone, wrong account, and "nobody yet" are genuinely
different problems and collapsing them into one "No friends invited" would be a bug factory.
Capping link max-uses at remaining slots prevents the classic over-invite link that outlives the
seat limit.

---

## INST-30 — Share: publish

**Type:** button
**Nav path:** `/instance/:id/share`
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/share/index.vue:44] (`SharedInstancePublishModal`), [SOURCE apps/app-frontend/src/pages/instance/share/index.vue:194] (`publishState`)

**Purpose:** Push the owner's local content changes out to everyone who has the shared instance —
i.e. "publish" is the write side of the shared-instance model, as opposed to the tab-level "accept
update" that members get.

**Operation:** `SharedInstancePublishModal` is driven by `:instance` and emits `state-change` into
`publishState: 'idle' | 'reviewing' | 'publishing'` (`share/index.vue:44-48`, `:194`). Publish
reconciles against `get_shared_instance_publish_preview(instanceId)`
(`content/index.vue:596-602`), which is also re-fetched after every local content mutation so the
pending diff stays accurate.

**Controls:** modal-driven; state is surfaced back to the page.

**Persistence:** server-side publish state; local mirror is the `linkedModpackInfo` query.

**Rationale & notes:** Keeping the preview reconciled after *every* toggle/delete/update
(`reconcileSharedInstancePublishState`, called from `toggleDisableMod` and `setManagedContentEnabled`)
means the owner never sees a stale "3 changes pending" count. Silent failure is deliberate: the
reconcile is wrapped in a `debug`-logged catch so a preview failure cannot break the actual content
mutation that triggered it.

---

## INST-31 — Settings → General (name, icon, update channel, duplicate, delete)

**Type:** dialog (tab 0 of 4)
**Nav path:** modal opened from the header gear or the page context menu
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/components/settings-modal/index.vue:82] (tab list), [SOURCE apps/app-frontend/src/pages/instance/components/settings-modal/general-settings.vue:167] (name patch)

**Purpose:** Identity and lifecycle of the instance itself.

**Operation:**
- **Name** — an `Input` with `maxlength=80`, saved on every keystroke via a deep watcher calling
  `edit(id, { name: title.trim().substring(0, 80) })` (`general-settings.vue:167-178`).
- **Icon** — a 108 px `Avatar` with a hover overlay; the overflow offers **Select icon** (native file
  dialog filtered to `png, jpeg, svg, webp, gif, jpg`), **Create an icon** (opens `IconEditorModal`,
  a generator producing `InstanceIconConfig`), and **Remove icon** (shown only when an icon exists)
  (`general-settings.vue:307-363`).
- **Update channel** — a `Chips` group of Release / Beta / Alpha, persisted immediately, with
  optimistic rollback on failure (`general-settings.vue:104-117`). Changing it invalidates
  `['linkedModpackInfo', instanceId]`.
- **Duplicate** — `install_duplicate_instance(id)`; description "Creates a copy of this instance,
  including worlds, configs, mods, etc."; disabled while installing
  (`general-settings.vue:65-71`, `:379-397`).
- **Delete instance** — red `colored` button behind `ConfirmDeleteInstanceModal`; description warns
  "Permanently deletes an instance from your device, including your worlds, configs, and all
  installed content. Be careful, as once you delete a instance there is no way to recover it."
  (sic — "a instance") (`general-settings.vue:275-291`, `:415-438`).

**Controls:** name 0–80 chars, trimmed, falling back to `'Instance'` if empty; icon formats as above;
release channel ∈ {release, beta, alpha}.

**Persistence:** instance row fields `name`, `icon_path`, `icon_config`, `update_channel`
(`edit_instance.rs:18`, `:24`, `:30`, `:31`). `icon_path` and `icon_config` are `double_option`
(`serde_with::rust::double_option`) so `null` can *clear* a value while absence means "don't touch"
(`edit_instance.rs:19-30`).

**Rationale & notes:** Typing the name with no Save button is the right call for a launcher — a
rename is frequent and instantly reversible. Delete is placed last in the tab, coloured red, and
fronted with the most explicit copy in the whole settings modal. The duplicated-glyph typo
("a instance") is a genuine upstream copy bug worth recording.

---

## INST-32 — Settings → Installation (loader, game version, loader version)

**Type:** dialog (tab 1 of 4)
**Nav path:** modal
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/components/settings-modal/installation-settings.vue:210] (`installationInfo`), [SOURCE apps/app-frontend/src/pages/instance/components/settings-modal/installation-settings.vue:343] (`save`), [SOURCE apps/app-frontend/src/pages/instance/components/settings-modal/installation-settings.vue:263] (`resolveGameVersions`)

**Purpose:** Read — and, for unlocked instances, change — which Minecraft version and which loader
build the instance runs, and keep the on-disk install in sync with that choice.

**Operation:**
- **Read-only display rows** (`installationInfo`): "Platform" (`formatLoaderLabel(loader)`),
  "Minecraft version", and "{loader} version" — the last row is only added when the loader is not
  `vanilla` **and** `loader_version` is set (`installation-settings.vue:210-230`).
- **Metadata preloads** at modal mount: `get_loader_versions` for `fabric`, `forge`, `quilt`, `neo`
  and `get_game_versions`, all under `['instance-settings', ...]` (`settings-modal/index.vue:127-150`).
- **Game-version resolution** filters the global version list against the loader manifest's
  `gameVersions`, and hides snapshots unless the picker offers them
  (`installation-settings.vue:263-281`).
- **Loader-version resolution** walks `gameVersions → versionGroup → versionGroups[].loaders`, with
  a `${modrinth.gameVersion}` placeholder fallback
  (`installation-settings.vue:283-320`).
- **Save** patches `{loader, game_version}` (+ `loader_version` when not vanilla) via `edit`, then
  `afterSave` runs `install_existing_instance(id, false)` — a full (non-forced) reinstall — and
  emits `InstanceRepair` telemetry (`installation-settings.vue:343-369`).
- **Modpack block:** if linked, shows the modpack card with version + link, and version actions
  (`onModpackVersionConfirm` → `update_managed_modrinth_version`) gated on
  `showModpackVersionActions` (`installation-settings.vue:452`, `:475`).
- **Busy gating:** the entire tab disables when `quarantined`, `install_stage !== 'installed'`, or
  during repair/reinstall/unlink/offline, with the message "Installation settings are unavailable
  while this instance is locked." (`installation-settings.vue:135-146`).

**Controls:** loaders limited to those whose `supported_project_types` include `modpack`, plus
`vanilla`, with `vanilla` sorted first (`installation-settings.vue:63-69`). Changing loader resets
the loader-version list.

**Persistence:** `content_set` rows — `game_version`, `loader`, `loader_version`, `protocol_version`,
`source_kind` (`AppliedContentSetPatch`, `edit_instance.rs:88-106`). The edit is applied to the
*content set*, not the instance row, which is why a content-set edit also marks a shared instance
stale (`edit_instance.rs:128-134`).

**Rationale & notes:** Separating "what this instance runs" (content set) from "how it launches"
(launch overrides, INST-36..41) is the key architectural decision in this settings modal: changing
the Minecraft version must reinstall, changing the memory slider must not. The `vanilla`-first sort
in the loader list reflects that most vanilla users are not modders. The silent `debug` logger
(`useDebugLogger('AppInstallationSettings')`) with detailed call-site instrumentation throughout is a
strong signal of how hard the loader-version manifest resolution was to get right.

---

## INST-33 — Settings → Repair

**Type:** button
**Nav path:** modal → Installation tab
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/components/settings-modal/installation-settings.vue:371] (`repair`), [SOURCE packages/app-lib/src/install/runner.rs:157] (`install_existing_instance`), [SOURCE apps/app-frontend/src/pages/instance/layout.vue:520] (header Repair)

**Purpose:** Re-materialise a broken install (missing/corrupt files, failed migration) without
deleting worlds, configs, or manual edits.

**Operation:**
- **Settings Repair button:** `install_existing_instance(instanceId, /* force */ true)`
  (`installation-settings.vue:371-381`). The `force` flag is the whole difference from the
  post-version-change reinstall.
- **Header Repair button:** appears only when `install_stage !== 'installed'` (`page-header/index.vue:109-119`),
  and does **more** — for a modpack instance in the `pack_installed` stage it calls
  `install_pack_to_existing_instance(..., { type: 'fromVersionId', ... })` re-resolving the linked
  modpack version; otherwise it falls back to `install_existing_instance(id, false)`
  (`layout.vue:520-535`).
- `install_existing_instance` starts an async `InstallJob` (`runner.rs:157-163`); progress is tracked
  via `useAppEvent`/job snapshots and the button shows a spinner while `repairing` is true.
- Both emit `InstanceRepair` telemetry with loader + game version.

**Controls:** disabled while `repairing`, and by the tab-wide busy gate (INST-32).

**Persistence:** the install job rewrites the instance's content files; the job record itself is in
the install-job store.

**Rationale & notes:** Two different "Repair" behaviours under one label is a mild inconsistency —
the header's version is a *resume/finish install* (it re-downloads the modpack from its version),
the settings' version is a *forced repair*. Both are correct for their context but the wording is
identical.

---

## INST-34 — Settings → Reinstall modpack / Swap modpack / Unlink modpack

**Type:** button
**Nav path:** modal → Installation tab
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/components/settings-modal/installation-settings.vue:383] (`reinstallModpack`), [SOURCE apps/app-frontend/src/pages/instance/components/settings-modal/installation-settings.vue:406] (`swapModpack`), [SOURCE apps/app-frontend/src/pages/instance/components/settings-modal/installation-settings.vue:423] (`unlinkModpack`)

**Purpose:** Three distinct recovery paths for pack instances — re-fetch the same modpack version,
replace it with a different `.mrpack` file, or cut the link entirely so the instance is free-form.

**Operation:**
- **Reinstall modpack:**
  - *Modrinth-linked pack:* `update_repair_modrinth(instanceId)` →
    `plugin:instance|instance_repair_managed_modrinth` (`helpers/instance.ts:553`).
  - *Imported pack (`link.type === 'imported_modpack'`):* opens the native file picker
    (`filePicker.pickModpackFile`), then `install_pack_to_existing_instance(id, { type: 'fromFile',
    path })` and awaits the job with `wait_for_install_job(appEvents, job.job_id)`
    (`installation-settings.vue:191-203`, `:388-390`).
  - Telemetry `InstanceRepair` only fires if it actually ran.
- **Swap modpack:** same picker path but runs unconditionally for **any** linked pack —
  `installLocalModpackFromPicker()` (`installation-settings.vue:406-421`).
- **Unlink modpack:** `edit(id, { link: null })`, then invalidate `['linkedModpackInfo', id]` and
  emit `unlinked` so the page header refreshes (`installation-settings.vue:423-433`). The Content
  tab has its own `unpairInstance` doing the same thing plus `localImportedModpackUnlinked` so the
  local modpack banner disappears (`content/index.vue:1447-1456`).
- **Unlink shared instance** (the shared-instance variant) additionally invalidates
  `instanceKeys.sharedMembers` and is gated behind `canUnlinkSharedInstance`
  (`installation-settings.vue:148-162`). It is rendered by
  `SharedInstanceInstallationSettingsControls` (`shared-instance-installation-settings-controls.vue:13`).

**Controls:** both buttons show a spinner while `reinstalling`; the shared-instance unlink flow opens
a warning modal that also offers an **inline backup** ("The version being replaced will be backed up
before anything changes.", `shared-instance-installation-settings-controls.vue:57-61`), and
cancelling the modal cancels the backup (`on-hide="() => backupCreator?.cancelBackup()"`, `:51`).

**Persistence:** `instance_link` row (upsert/delete via `instance_rows::upsert_instance_link`,
`edit_instance.rs:193-196`); content files rewritten by the install job.

**Rationale & notes:** Treating *reinstall the same pack*, *swap to a different pack*, and *unlink*
as three buttons instead of one overloaded "reinstall" is a good call — they have very different
blast radii. Backing up to a modal you can still cancel from is excellent friction design for an
operation that discards a pack's intended composition.

---

## INST-35 — Settings → Sync overrides (the override mechanism)

**Type:** dialog (tab 2 of 4, labelled **Sync overrides**)
**Nav path:** modal
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/components/settings-modal/index.vue:99] (tab definition, label "Sync overrides"), [SOURCE apps/app-frontend/src/pages/instance/components/settings-modal/synced-options-settings.vue:198] (`rows`), [SOURCE apps/app-frontend/src/pages/instance/components/settings-modal/synced-options-settings.vue:299] (`excluded`)

**Purpose:** Opt one instance out of each globally-synced category so it keeps its own copy.
This is the *second, distinct* override system in the settings modal (the first being launch
overrides, INST-36..41) and the two must not be confused.

**Operation:**
- `rows` enumerates exactly six per-instance overrides, **in this display order**:
  `game_options`, `multiplayer_servers`, `resource_packs`, `data_packs`, `command_history`,
  `creative_hotbars` (`synced-options-settings.vue:198-233`). Each is filtered by
  `isSyncedOptionAvailable(option)` before rendering.
- Each row is a `Toggle` whose `model-value` is `excluded(option)` — i.e. the toggle reads
  **"am I excluded from the global sync?"**, not "am I synced?" (`synced-options-settings.vue:299-307`,
  template `:658-671`). This inversion is deliberate: the safe/expected state (follow the global)
  is the toggle's "off".
- Turning the toggle calls `setExcluded(option, nextExcluded)` → `set_synced_option(instanceId,
  option, enabled, resolution)` (`synced-options-settings.vue:454-485`). The mutation is optimistic
  with a rollback on error, patching both `instanceKeys.detail(id)` and `instanceKeys.list()`
  (`synced-options-settings.vue:376-429`).
- **Creative hotbars are special:** because both sides may have diverged, the toggle first calls
  `get_synced_option_join_preview(instanceId, 'creative_hotbars')`. If the action is
  `requires_resolution`, a conflict modal asks "{instance} and your synced version have different
  creative hotbars. Choose which one to use across your instances." with **Use synced** /
  **Overwrite others**, plus "The version being replaced will be backed up before anything changes."
  (`synced-options-settings.vue:164-184`, `:512-561`).
- **Global gating:** if `global_options[option] === false` (syncing turned off in app settings
  entirely) the row is disabled with a per-option message ("Resource pack syncing is turned off in
  app settings.", etc. — `globalDisabledMessages`, `:189-196`). The global kill-switch always wins.
- `capabilities[].disabled_reason` from the overview query is surfaced as the same tooltip
  (`:322-327`).
- **Row-specific edit buttons** open the matching modal, and are themselves disabled when the
  override is off:
  - `game_options` → `GameSettingsModal`; tooltip "Turn off this override to edit synced packs"
  - `multiplayer_servers` → `SyncedServersModal`; disabled with "You haven't synced any servers yet"
  - `command_history` → `CommandHistoryModal`; disabled with "Choose a sync source before editing this setting."
  - `resource_packs` / `data_packs` → `SyncedPacksModal` (`'resourcepack'` / `'datapack'`); disabled with "Turn off this override to edit synced packs."
  (`synced-options-settings.vue:588-657`, `:329-341`).
- Below the synced rows, separated by `<hr>`, the modal **also** embeds Window Settings (INST-40),
  Java Settings (INST-36..39) and Hooks (INST-41) (`synced-options-settings.vue:678-686`) — i.e.
  the "Sync overrides" tab is really "everything about how this instance differs from your defaults".
- A "Manage synced settings" button closes this modal and opens the global app settings on the
  synced-options section (`synced-options-settings.vue:343-349`, `:567-570`).

**Controls:** one toggle per row; global availability can disable a row.

**Persistence:** per-instance booleans in `instance.synced_options[<option>]`, written by
`set_synced_option`. Overview/capabilities read from `['instance-synced-options', instanceId]`.
**Does it sync?** Yes — that is the point. The flag is per-instance and local; the *data* it
governs is synced globally.

**Rationale & notes:** The polarity inversion (`excluded`) is the single most confusing thing in
this tab and is worth flagging for the successor: every other toggle in the modal means "use my
value", this one means "abandon my value for the global one". Because all six rows share a
`Toggle` + description + optional edit button layout, the tab scans as a single repeating unit,
which mitigates it. Bundling Window/Java/Hooks under a tab called "Sync overrides" is a naming
mismatch — those are launch overrides, not sync overrides — but grouping all per-instance
divergence in one place is defensible and arguably better than a fifth tab.

---

## INST-36 — Custom memory allocation

**Type:** field (+ toggle)
**Nav path:** modal → Sync overrides tab
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/components/settings-modal/java-settings.vue:102] (`overrideMemorySettings`), [SOURCE apps/app-frontend/src/pages/instance/components/settings-modal/java-settings.vue:124] (`editInstanceObject`), [SOURCE packages/app-lib/src/api/instance/run.rs:142] (resolution)

**Purpose:** Give one instance more (or less) RAM than the global setting, without affecting the
others.

**Operation:**
- Default state: `overrideMemorySettings = !!instance.memory`; the displayed value is
  `instance.memory ?? globalSettings.memory` (`java-settings.vue:102-103`).
- Toggling **off** copies the global value back into the slider so the user sees what they will
  inherit, then writes `memory: null` (`java-settings.vue:118-122`, `:134`).
- Toggling **on** writes the current slider value.
- A deep watcher on `[overrideMemorySettings, memory]` writes on every change — no Save button
  (`java-settings.vue:138-153`).
- Effective value at launch: `context.launch_overrides.memory.unwrap_or(settings.memory)`
  (`run.rs:142`).

**Controls:**
- Slider: `min=512` MB, `max=maxMemory` (system RAM, `Math.floor(maxMemoryQuery/1024)`,
  `useMemorySlider.js:8`), `step=64`, snap points at 2048 MB and every doubling after it
  (`useMemorySlider.js:10-20`), `snapRange=512`, `unit="MB"`, min label "512 MB", max label
  `"<x.x> GB"` (`java-settings.vue:222-235`).
- Disabled (and the whole section dimmed to `opacity-50`) when the override is off
  (`java-settings.vue:226`, `:258`).
- If the max-memory query errors, the slider is replaced by a **Refresh** button; while loading, a
  pulsing skeleton bar with `role="status"` (`java-settings.vue:236-244`).
- No client-side numeric validation — the Rust side is the backstop.

**Persistence:** `launch_overrides.memory` in SQLite; `null` means inherit. **Does not sync.**

**Rationale & notes:** Power-of-two snap points starting at 2048 MB encode the actual convention
(2 GB / 4 GB / 8 GB) and prevent typos like "3500". Showing the inherited global value while the
toggle is off — rather than blanking the fields — is the key UX decision: the user always sees what
they *would* get, so toggling is an informed act. The `java.exe` → `javaw.exe` substitution on
Windows happens in the same patch (INST-37).

---

## INST-37 — Custom Java installation

**Type:** field (+ toggle)
**Nav path:** modal → Sync overrides tab
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/components/settings-modal/java-settings.vue:42] (`overrideJavaInstall`), [SOURCE packages/app-lib/src/launcher/mod.rs:133] (`get_java_version_from_launch_context`), [SOURCE packages/app-lib/src/launcher/mod.rs:318] (`resolve_java_for_launch` fallback)

**Purpose:** Run one instance on a specific JDK — required for old modpacks (Java 8) alongside a
modern pack (Java 21).

**Operation:**
- Default: `overrideJavaInstall = !!instance.java_path`; the field shows
  `instance.java_path ?? optimalJava.path`, where `optimalJava` comes from
  `get_optimal_jre_key(instanceId)` — the JRE the launcher *would* pick
  (`java-settings.vue:40-46`).
- **Live validation:** a watcher on the path runs `testJavaInstallation(newPath, null, false)` once on
  first change (immediately) and debounced thereafter; the adjacent button then renders
  ✓ (green), ✗ (red), a spinner, or a ↻ depending on state, and the button itself is recoloured
  (`java-settings.vue:56-75`, `:276-322`).
- **Detect** opens `JavaDetectionModal` (scans for installed JDKs); **Browse** opens a native
  directory dialog (`java-settings.vue:81-90`, `:325-333`).
- Saved value has `java.exe` replaced by `javaw.exe` before persisting — so on Windows the game
  launches without a console window (`java-settings.vue:126-129`).
- **Resolution chain at launch** (`launcher/mod.rs:133-156` and `:314-324`):
  1. `launch_overrides.java_path` → `check_jre` on it; if it validates, use it.
  2. Otherwise fall back to `JavaVersion::get(key)` where `key` is the Minecraft version's required
     major version (default 8) — i.e. the globally installed JRE for that major version.
  3. Otherwise `auto_install_java(key)` — **download and install it**, then `upsert` it into the
     local JRE table so step 2 succeeds next time.
- A broken override path therefore degrades gracefully to auto-detect rather than failing the
  launch.

**Controls:** free-text path input (placeholder "/path/to/java"), Detect, Browse, Test. Disabled
when the override toggle is off.

**Persistence:** `launch_overrides.java_path` (`String?`, `null` = inherit). **Does not sync.**

**Rationale & notes:** Showing the *optimal* (auto-selected) JRE in the field while the override is
off is the same "show what you'd inherit" pattern as INST-36, and it makes the Detect button's
purpose obvious. Live validation with a green/red button is better than a separate "Test" flow
because the failure mode (wrong JDK major version) is otherwise only discoverable at launch. The
`javaw.exe` rewrite is a small but genuinely user-visible quality touch on Windows.

---

## INST-38 — Custom Java arguments

**Type:** field (+ toggle)
**Nav path:** modal → Sync overrides tab
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/components/settings-modal/java-settings.vue:92] (`overrideJavaArgs`), [SOURCE packages/app-lib/src/api/instance/run.rs:114] (resolution)

**Purpose:** Add JVM flags for one instance (e.g. `-XX:+UseG1GC` tuning, or a memory-related flag).

**Operation:**
- Default: `overrideJavaArgs = (instance.extra_launch_args?.length ?? 0) > 0`; the field shows
  `(instance.extra_launch_args ?? globalSettings.extra_launch_args).join(' ')`
  (`java-settings.vue:92-95`).
- Toggling off restores the global string (`java-settings.vue:106-110`).
- Persisted by splitting on `/\s+/` after trimming and filtering empties
  (`java-settings.vue:130-132`) — **so arguments containing quoted spaces cannot be expressed**.
- Resolution at launch: `launch_overrides.extra_launch_args.clone().unwrap_or(settings.extra_launch_args)`
  (`run.rs:114-118`). These are also what `$INST_JAVA_ARGS` expands to for hooks
  (`run.rs:174-179`).

**Controls:** single-line text input, placeholder "Enter Java arguments...", disabled when the
override is off (section dimmed).

**Persistence:** `launch_overrides.extra_launch_args` (`Vec<String>?`, `null` = inherit).
**Does not sync.**

**Rationale & notes:** Displaying as one space-joined string but storing as a `Vec<String>` is the
standard launcher compromise. The naive whitespace split means `-Dfoo="a b"` will be mangled —
worth recording as a limitation of the reconstruction.

---

## INST-39 — Custom environment variables

**Type:** field (+ toggle)
**Nav path:** modal → Sync overrides tab
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/components/settings-modal/java-settings.vue:97] (`overrideEnvVars`), [SOURCE apps/app-frontend/src/helpers/settings.ts] (`parseEnvVars` / `serializeEnvVars`), [SOURCE packages/app-lib/src/api/instance/run.rs:128] (resolution)

**Purpose:** Set per-instance env vars for the game process (e.g. `JAVA_TOOL_OPTIONS`, a proxy, a
modded-loader flag).

**Operation:**
- Default: `overrideEnvVars = (instance.custom_env_vars?.length ?? 0) > 0`; the field shows
  `serializeEnvVars(instance.custom_env_vars ?? globalSettings.custom_env_vars)`
  (`java-settings.vue:97-100`).
- Toggling off restores the serialized global value (`java-settings.vue:112-116`).
- Persisted via `parseEnvVars(text) -> Vec<(String, String)>`
  (`java-settings.vue:133`) — i.e. round-tripped through the same `KEY=VALUE`-per-line serializer
  used by the global settings page, so the two are formatted identically.
- Resolution at launch: `launch_overrides.custom_env_vars.clone().unwrap_or(settings.custom_env_vars)`
  (`run.rs:128-132`). The resulting pairs are **replaced**, not merged, with the global list — an
  override does not add to the globals, it substitutes for them. Note `run.rs:185-187`: when hooks
  are active, the env args used are `hook_environment.injected_envs()` rather than the raw list,
  so hook-injected variables take precedence.

**Controls:** single-line text input, placeholder "Enter environmental variables...", disabled when
the override is off.

**Persistence:** `launch_overrides.custom_env_vars` (`Vec<(String,String)>?`, `null` = inherit).
**Does not sync.**

**Rationale & notes:** Reusing the global serializer for the per-instance field is a small but real
quality decision — the two forms stay learnable from each other. The replace-not-merge semantics
should be documented in the successor's blueprint, because users often expect merge.

---

## INST-40 — Custom window settings (fullscreen / width / height)

**Type:** field (+ toggle)
**Nav path:** modal → Sync overrides tab
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/components/settings-modal/window-settings.vue:18] (`overrideWindowSettings`), [SOURCE apps/app-frontend/src/pages/instance/components/settings-modal/window-settings.vue:35] (`editInstanceObject`), [SOURCE packages/app-lib/src/api/instance/run.rs:143] (resolution)

**Purpose:** Launch one instance at a different window size or in fullscreen.

**Operation:**
- Default: `overrideWindowSettings = !!instance.game_resolution || !!instance.force_fullscreen`;
  fields show `instance.game_resolution ?? globalSettings.game_resolution` and
  `instance.force_fullscreen ?? globalSettings.force_fullscreen`
  (`window-settings.vue:18-26`).
- **Mutual exclusion:** when fullscreen is on, `game_resolution` is written as `null` — the two
  settings are not simultaneously persisted (`window-settings.vue:42-45`). Width/height inputs are
  `disabled` while fullscreen is on (`window-settings.vue:137`, `:156`).
- Toggling off writes `{force_fullscreen: null, game_resolution: null}` (`window-settings.vue:36-40`).
- Resolution at launch: `launch_overrides.game_resolution.unwrap_or(settings.game_resolution)`
  (`run.rs:143-146`).
- Fullscreen is described as being applied "using options.txt" (`window-settings.vue:69-72`) —
  INFERRED from that copy that the fullscreen flag is written into the instance's `options.txt`
  rather than passed as a JVM arg; basis: the user-facing description string in the source.

**Controls:** width/height `type="number"` inputs (placeholders "Enter width..." / "Enter
height..."), no min/max/validation in the UI, disabled when the override is off or fullscreen is on.
The whole section dims to `opacity-50` when the override is off.

**Persistence:** `launch_overrides.force_fullscreen` (`bool?`) and `launch_overrides.game_resolution`
(`WindowSize?`), both `null` = inherit. **Does not sync.**

**Rationale & notes:** Forcing width/height to `null` when fullscreen is enabled (rather than
persisting a stale resolution) means toggling fullscreen off and re-enabling the override starts
from a clean value. The "using options.txt" hint is a genuinely useful disclosure — it tells the
user *why* the change may not take effect if they edited `options.txt` by hand, and it dovetails
with the `game_options` synced option (INST-35) which owns that file.

---

## INST-41 — Custom game launch hooks

**Type:** field (+ toggle)
**Nav path:** modal → Sync overrides tab
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/components/settings-modal/hooks-settings.vue:181] (`hasCustomHooks`), [SOURCE packages/app-lib/src/api/instance/run.rs:106] (pre-launch resolution), [SOURCE packages/app-lib/src/api/instance/run.rs:189] (execution)

**Purpose:** Run system commands before and after the game for one instance — the standard way to
start a local proxy/LCraft server, or clean up after a session.

**Operation:**
- Default: `overrideHooks = !!hooks.pre_launch || !!hooks.wrapper || !!hooks.post_exit`; the fields
  show the instance hooks if any are set, else the global ones
  (`hooks-settings.vue:181-191`).
- Toggling off writes all three to `''` (`hooks-settings.vue:203-215`) — note this writes **empty
  strings**, not `null`, because the Rust type is `Hooks` (non-optional) with `Option<String>`
  fields. Empty strings are then filtered out at launch by `.filter(|hook| !hook_command.is_empty())`.
- Resolution at launch (`run.rs:106-140`): each of the three independently does
  `launch_overrides.hooks.X.or(settings.hooks.X)` then `.filter(!is_empty)`. **They are resolved
  per-hook, not as a group** — so an instance that sets only `pre_launch` still inherits the global
  `wrapper` and `post_exit`.
- If any hook is non-empty, the launcher canonicalizes the instance directory and builds a
  `HookEnvironment` exposing `$INST_NAME`, `$INST_ID`, `$INST_DIR`, `$INST_MC_DIR`, `$INST_JAVA`,
  `$INST_JAVA_ARGS` (`run.rs:147-184`; the variable list is documented to the user verbatim at
  `hooks-settings.vue:359-369`).
- Pre-launch runs via `shlex::split` + `Command::spawn().wait()` with `current_dir` = instance dir
  and the env args applied; **a non-zero exit aborts the launch** with "Non-zero exit code for
  pre-launch hook: {code}" (`run.rs:189-221`).
- `wrapper` is expanded through the hook environment and becomes the launcher command
  (`run.rs:224-230`); `post_exit` runs after the process exits.

**Controls:** three text inputs — **Pre-launch** ("Ran before the instance is launched."),
**Wrapper** ("Wrapper command for launching Minecraft."), **Post-exit** ("Ran after the game
closes.") — plus a bulleted list of the six expansion variables. All disabled when the override is
off (section dimmed).

**Persistence:** `launch_overrides.hooks` (`Hooks`, non-optional object).
**Does not sync.**

**Rationale & notes:** Per-hook fallback is the right granularity — a user who only needs a
pre-launch command for a server-join instance does not lose the global post-exit cleanup. Making a
failing pre-launch hook a hard error (rather than a warning) is the correct call: users script
dependency startup here, and continuing with the game down would be worse. The variable list is
surfaced in the UI rather than hidden in docs, which is the difference between a usable feature and
an obscure one.

---

## INST-42 — Settings → Sharing (active invites, unpublish)

**Type:** dialog (tab 3 of 4)
**Nav path:** modal
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/components/settings-modal/index.vue:107] (tab with `shown: role === 'owner' && !quarantined`), [SOURCE apps/app-frontend/src/pages/instance/components/settings-modal/sharing-settings.vue:13] (invites table), [SOURCE apps/app-frontend/src/pages/instance/components/settings-modal/sharing-settings.vue:193] (`unpublishSharedInstance`)

**Purpose:** Owner-side administration of the shared instance: see outstanding invite links and
terminate the whole share.

**Operation:**
- The tab is **only present** when `instance.shared_instance?.role === 'owner'` and the instance is
  not quarantined (`settings-modal/index.vue:114`). The tab list is recomputed reactively, so the
  tab disappears if ownership changes while the modal is open.
- **Active invites table** columns: `Invite link` (a `CopyCode` of `{siteUrl}/share/{id}`),
  `Uses` (`row.uses / row.maxUses`), `Expires` (relative time, absolute on hover), `Actions`
  (`sharing-settings.vue:13-67`, `:164-186`). `min-width: 36rem`, actions right-aligned.
- Empty state: "There are no active invites." with a spinner while loading
  (`sharing-settings.vue:14-25`, `:232-235`).
- **Revoke invite** → `ConfirmRevokeSharedInstanceInviteModal` → `revoke_shared_instance_invite`;
  the row is removed optimistically from `['sharedInstanceInvites', instanceId]` on success
  (`sharing-settings.vue:144-154`, `:188-191`).
- **Unpublish** → `unpublish_shared_instance(instanceId)`, then clears `sharedMembers` and
  invalidates `linkedModpackInfo`, and calls `onUnlinked()` so the page header refreshes
  (`sharing-settings.vue:193-205`). It is rendered via
  `SharedInstanceInstallationSettingsControls` with `can-unpublish`, which opens a warning modal
  before confirming (`shared-instance-installation-settings-controls.vue:2-12`, `:27-45`).
- Disabled while `install_stage !== 'installed'`, while a revoke/unpublish is in flight, or offline
  (`sharing-settings.vue:156-162`).

**Controls:** revoke per row; unpublish once, orange-coloured with a confirmation modal.

**Persistence:** server-side; cached at `['sharedInstanceInvites', instanceId]` and
`instanceKeys.sharedMembers(instanceId)`.

**Rationale & notes:** Splitting "invite people" (the Share tab) from "manage the invites you
already made" (Settings → Sharing) mirrors the common distinction between the act and its
administration. Showing `uses / maxUses` and a relative expiry is exactly the information needed to
decide whether a link is still useful; a bare list of opaque codes would not be actionable. Revoke
is an `IconButton` with a destructive hover colour rather than a text button, keeping the table
visually quiet.

---

## INST-43 — Page header: Play / Stop / Repair / Settings

**Type:** button
**Nav path:** `/instance/:id` (always visible above the tabs)
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/components/page-header/index.vue:71] (`#actions`), [SOURCE apps/app-frontend/src/pages/instance/layout.vue:540] (`tabs` block follows header)

**Purpose:** The primary verbs of the instance, always reachable regardless of tab.

**Operation:** the primary button is a **single slot with mutually exclusive states**, which is
worth documenting as a pattern (`page-header/index.vue:73-145`):
1. `playing` → red **Stop**, label becomes "Stopping…" while `stopping`.
2. `isInstalling` (`installing | pack_installing | minecraft_installing`, `page-header/index.vue:313`)
   → disabled button reading "Installing…".
3. `quarantined` → disabled **Play** with tooltip "This instance has been locked".
4. `install_stage !== 'installed'` → **Repair** with a download icon (see INST-33 for behaviour).
5. `!loading && isServerInstance` → a `SplitButton` whose default action is Play (launch the
   instance locally) and whose single menu item is **Launch instance**
   (`page-header/index.vue:120-131`, `:348-355`) — i.e. the primary click is `play-server` and the
   dropdown is the local launch.
6. `!loading` → brand **Play**.
7. otherwise → disabled "Starting…".

Behind `startInstance` there are two interception layers before a launch actually happens
(`layout.vue:536-580`): (a) a shared-instance update preview is fetched when the user is a member,
online and unlocked — if an update is available, `SharedInstanceUpdateModal` opens and the launch is
deferred to its "accept and play" callback; (b) `UpdateToPlayModal` opens if `hasUpdate` is set
(pack content changed on the server). Only if neither intercepts does `launchInstance` call `run(id)`.
A `checkingSharedInstanceLaunch` guard prevents double-clicks, and `loading` guards re-entry.
Launch failures route to `handleSevereError` with the instance id.

Alongside: a gear `IconButton` (tooltip "Instance settings") opening the modal
(`page-header/index.vue:147-155`), and a quiet `TeleportOverflowMenu` labelled "More actions".

**Controls:** `Settings` opens the modal with no specific tab; the Content tab's "Manage synced packs"
opens it at **tab index 1** (`content/index.vue:232`, `openSettings(1)`) — INFERRED that index 1 is
Installation, basis: the tab array order in `settings-modal/index.vue:82-116`.

**Persistence:** none for the buttons themselves.

**Rationale & notes:** The seven-state primary button is the clearest example in this slice of
Modrinth using a single high-salience slot rather than several competing buttons. The state order
is meaningful: *running* beats *installing* beats *locked* beats *broken* — so a quarantined
instance mid-install shows "Installing…", not the lock tooltip. Deferring the launch behind a
content-update prompt is a significant behavioural choice: the app never silently launches a
stale pack.

---

## INST-44 — Page header overflow: Open folder / Export modpack / Create shortcut / Report

**Type:** menu
**Nav path:** `/instance/:id`
**Evidence:** [SOURCE apps/app-frontend/src/pages/instance/components/page-header/index.vue:356] (`moreActions`), [SOURCE apps/app-frontend/src/pages/instance/layout.vue:44] (ExportModal), [SOURCE apps/app-frontend/src/pages/instance/layout.vue:520] (`repairInstance`), [SOURCE apps/app-frontend/src/components/ui/ExportModal.vue:27] (export labels)

**Purpose:** The instance-level operations that are real but not daily.

**Operation:**
- **Open folder** → `showInstanceInFolder(instanceId)` — always present, and the only item that is
  not gated (`page-header/index.vue:357-364`).
- **Export modpack** → `ExportModal`, hidden when `quarantined`; opens a modal with **Modpack name**
  (default = instance name), **Version number** (default "1.0.0") and a description textarea, then
  `export_instance_mrpack(instanceId, path, includedOverrides, excludedOverrides, versionId)`
  producing a `.mrpack` file (`ExportModal.vue:27-45`, `helpers/instance.ts:560-565`). Success is
  reported as "Export complete" / "{name} was exported successfully."
- **Create shortcut** → `createInstanceShortcut(name, id)`; on success a "Shortcut created"
  notification, on failure "Error creating shortcut" (`layout.vue:520-532`, `:210-214`).
- **Report** → only for shared-instance **members**; `tone: 'red'`, preceded by a divider. It
  fetches `install_get_shared_instance_preview(shared.id, name)` and opens
  `SharedInstanceInstallModal.showReport(preview, managerId)`; if the user then chooses to delete,
  `requestInstanceDeletion()` runs (`layout.vue:520`, `:520-536`). This is the abuse/escape path
  for a member stuck in a bad shared instance.
- A page-level **right-click context menu** on the header duplicates most of this plus Play / Stop /
  **Add content** / **Edit** / **Copy path** (`layout.vue:520`, `handleRightClick`). "Copy path" calls
  `get_full_path(id)` and writes to the clipboard.
- **Share (publish)** is *not* in this menu — it is a dedicated tab (INST-30).

**Controls:** Export requires a name and a version number; shortcut creation reports success/failure
as a notification.

**Persistence:** the exported `.mrpack` is written to a user-chosen path; shortcuts are OS artefacts.

**Rationale & notes:** Quarantining the instance removes Export and Create shortcut but *keeps*
Open folder — you can always inspect a locked instance on disk, you just cannot act on it. The
deliberate redundancy between the overflow menu and the right-click context menu means both
mouse-idiom users are served; the right-click menu additionally exposes **Copy path**, which is the
one genuinely novel item and is exactly what a power user needs when reporting a bug.

---

## Cross-cutting findings

### 1. The two override systems are different and easy to confuse

| | Sync overrides (INST-35) | Launch overrides (INST-36..41) |
| --- | --- | --- |
| Tab | "Sync overrides" (top block) | same tab, below `<hr>` |
| Controls | 6 category toggles | 4 field toggles + hooks |
| Data governed | game options, servers, packs, command history, hotbars | Java, memory, args, env, window, hooks |
| Fallback | global *sync source* | global `AppSettings` |
| Stored in | `instance.synced_options` | `instance_launch_overrides` |
| Syncs to cloud | yes (that is the feature) | no |
| Resolution granularity | per category | **per hook** (`run.rs:106-140`) vs per field group elsewhere |

Both are surfaced in one modal tab named "Sync overrides", which accurately describes the first and
misdirects for the rest.

### 2. The override resolution algorithm (enumerated from source)

For every launch-scoped setting, the effective value at launch is:

```
effective(X) = instance.launch_overrides[X]  if  Some
             | global AppSettings.X           otherwise
```
(`run.rs:106-146`), with two refinements:
- **Hooks resolve per sub-field**, not as a group (`run.rs:106`, `:120`, `:134`), and empty strings
  are treated as unset (`.filter(|h| !h.is_empty())`).
- **Java resolves through a three-stage chain**, not a simple override: explicit path → globally
  installed JRE for the Minecraft version's required major version → auto-install of that major
  version (`launcher/mod.rs:133-156`, `:314-331`). A bad explicit path silently degrades to stage 2.

Frontend mirror: each section's toggle is initialised from `!!instance.<field>` and turning it off
copies the global value back into the visible field so the user can see what they are inheriting
(`java-settings.vue:118-122`, `window-settings.vue:28-33`, `hooks-settings.vue:193-201`).

### 3. Things that are implemented but unreachable

- `settings-modal/behavior-settings.vue` — per-instance `visible_tabs` editor. Backing Rust type and
  patch path exist (`launch.rs:7-20`, `edit_instance.rs:85`), but the component is never imported,
  and `layout.vue` reads the *global* app settings instead. Per-instance tab visibility is dead.
- Route `InstanceContentFilter` (`projects/:type`) — no consumer, no navigation target.
- Route-level `meta.renderMode` is only ever `'fixed'` (Logs) — correctly a per-tab opt-in.

### 4. Per-instance persistence summary

| Data | Where | Per-instance? | Syncs? |
| --- | --- | --- | --- |
| name, icon_path, icon_config, update_channel | instance row | yes | no |
| game_version, loader, loader_version, protocol_version | content_set row | yes | no |
| java_path, extra_launch_args, custom_env_vars, memory, force_fullscreen, game_resolution, hooks | `instance_launch_overrides` | yes | no |
| synced_options.* (6 flags) | instance row | yes | yes |
| content enabled/locked flags | `instance_files` | per file | no |
| content update checks | `content_update_checks` (10-min TTL) | per instance | no |
| link (modpack/shared/server) | `instance_link` | yes | no |
| world name / server membership | world files + instance server list | yes | servers via synced option |
| screenshots | `<instance>/screenshots/` + custom groups | yes | screenshots are a sync option |
| content sort/filters | `sessionStorage`, keys `content-sort:<id>` / `content-filters:<id>` | yes | no |
| screenshot sort/group/search | `localStorage`, suffix `instance` vs `global` | yes | no |

---

## Open items requiring the running app

- **INST-17 (Export world)** — `[ORCH-PENDING]`. The affordance lives in
  `apps/app-frontend/src/components/ui/world/WorldItem.vue`, outside this slice's tree. Orchestrator:
  open an instance with at least one singleplayer world → Worlds tab → hover/overflow a world row →
  confirm the action's exact label, whether it writes a `.zip` and to which directory, and whether it
  appears for server rows.
- **Visual layout confirmation** — `[ORCH-PENDING]` for tab-bar overflow behaviour at narrow widths.
  `NavTabs` hides icons below `sm` (`NavTabs.vue:32`) and, when `pageNav` is used, wraps in a
  horizontally scrollable strip (`NavTabs.vue:4`); on the instance page `pageNav` is false, so what
  actually happens when all six tabs exceed the window width could not be determined from source
  alone (the `<nav>` is `w-fit`, so it likely overflows the container without a scroller).
  Orchestrator: resize the window narrow on an instance with all tabs enabled.
- **Whether `update_channel` changes are applied retroactively** — `[ORCH-PENDING]`. Source shows a
  watcher that re-runs `initProjects('must_revalidate')` on channel change
  (`content/index.vue:1809`), and the Rust check records `update_channel` per check
  (`update_check.rs:8`), but whether previously-installed beta versions get flagged when switching
  from Alpha to Release could not be proven statically.