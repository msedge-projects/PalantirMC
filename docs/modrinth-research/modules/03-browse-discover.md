# Module 03 — Browse / Discover

Reverse-engineering notes for the Modrinth App (Theseus) **v0.21.9**, tag `v0.21.9`,
commit `6ebd75b68dd386cd4d74eee1cf2e96d945c937db`.

Scope: the Discover surface — project-type navigation, search, facets, sorting,
pagination, the project card, the project detail page and its child routes, the
install flow reachable from Browse, update flows reachable from Browse, and every
empty/error/offline state.

Everything is derived from the pinned source. Rows marked `[ORCH-PENDING]` need a
visual capture to confirm; everything else is `[SOURCE …]`.

---

## Coverage table

| ID | label | type | evidence | status |
| --- | --- | --- | --- | --- |
| BR-1 | Project-type tabs | tab | `apps/app-frontend/src/routes.js:51`, `apps/app-frontend/src/pages/Browse.vue:713` | confirmed |
| BR-2 | Project-type visibility rules (instance / server / worlds context) | indicator | `apps/app-frontend/src/pages/Browse.vue:668` | confirmed |
| BR-3 | Search input | field | `packages/ui/src/layouts/shared/browse-tab/layout.vue:147` | confirmed |
| BR-4 | Search debounce + stale-response guard | indicator | `packages/ui/src/layouts/shared/browse-tab/composables/use-browse-search.ts:301` | confirmed |
| BR-5 | "Sort by:" combobox | button | `packages/ui/src/utils/search.ts:389` | confirmed |
| BR-6 | "View:" per-page combobox | button | `packages/ui/src/utils/search.ts:387`, `layout.vue:197` | confirmed |
| BR-7 | Pagination (page + offset) | button | `packages/ui/src/utils/search.ts:916`, `use-browse-search.ts:195` | confirmed |
| BR-8 | Category facets | toggle | `packages/ui/src/utils/search.ts:420` | confirmed |
| BR-9 | Loader / Platform facets | toggle | `packages/ui/src/utils/search.ts:551` | confirmed |
| BR-10 | Game-version facet | toggle | `packages/ui/src/utils/search.ts:513` | confirmed |
| BR-11 | Environment facet | toggle | `packages/ui/src/utils/search.ts:473` | confirmed |
| BR-12 | License facet (open source) | toggle | `packages/ui/src/utils/search.ts:689` | confirmed |
| BR-13 | Advanced-exclusions facet (disclosures) | toggle | `packages/ui/src/utils/search.ts:729` | confirmed |
| BR-14 | "Depends on" / "Included content" project filter | field | `packages/ui/src/utils/search.ts:458` | confirmed |
| BR-15 | Project-ID filter (hidden, `display: 'none'`) | toggle | `packages/ui/src/utils/search.ts:713` | confirmed |
| BR-16 | Provided (locked) filters + "Sync with instance" | toggle | `packages/ui/src/pages/Browse.vue:1143`, `sidebar.vue:288` | confirmed |
| BR-17 | "Hide content already installed" toggle | toggle | `packages/ui/src/layouts/shared/browse-tab/sidebar.vue:185` | confirmed |
| BR-18 | Filter chip strip above results | indicator | `packages/ui/src/components/search/SearchFilterControl.vue:1` | confirmed |
| BR-19 | Advanced-exclusions persistence + link override | indicator | `packages/ui/src/utils/advanced-filter-preferences.ts:6` | confirmed |
| BR-20 | Project card (list layout) — metadata and intent | indicator | `packages/ui/src/components/project/card/ProjectCard.vue:100` | confirmed |
| BR-21 | Project card (grid layout) — banner / colour | indicator | `packages/ui/src/components/project/card/ProjectCard.vue:11` | confirmed |
| BR-22 | Card action button (Install / Add to an instance / Installed) | button | `apps/app-frontend/src/pages/Browse.vue:997` | confirmed |
| BR-23 | Project-type header (breadcrumb + install-context header) | indicator | `apps/app-frontend/src/pages/Browse.vue:87`, `layout.vue:130` | confirmed |
| BR-24 | Project detail page header | indicator | `packages/ui/src/components/project/ProjectPageHeader.vue:1` | confirmed |
| BR-25 | Description tab | tab | `apps/app-frontend/src/pages/project/Description.vue:1` | confirmed |
| BR-26 | Versions tab | tab | `apps/app-frontend/src/pages/project/Versions.vue:1` | confirmed |
| BR-27 | Version tab (single version) | tab | `apps/app-frontend/src/pages/project/Version.vue:1` | confirmed |
| BR-28 | Gallery tab | tab | `apps/app-frontend/src/pages/project/Gallery.vue:1` | confirmed |
| BR-29 | Project right-hand sidebar | indicator | `apps/app-frontend/src/pages/project/Index.vue:3` | confirmed |
| BR-30 | Install flow — instance picker | dialog | `packages/ui/src/layouts/shared/content-tab/components/modals/ContentInstallModal.vue:1` | confirmed |
| BR-31 | Install flow — create-instance sub-tab | dialog | `packages/ui/src/layouts/shared/content-tab/components/modals/ContentInstallModal.vue:149` | confirmed |
| BR-32 | Install flow — version/loader resolution | indicator | `packages/ui/src/layouts/shared/browse-tab/composables/install-logic.ts:549` | confirmed |
| BR-33 | Install flow — dependency resolution (required only) | indicator | `packages/modrinth-content-management/src/install/mod.rs:122` | confirmed |
| BR-34 | Install flow — incompatibility warning | dialog | `apps/app-frontend/src/providers/content-install.ts:69` | confirmed |
| BR-35 | Install flow — modpack install + duplicate warning | dialog | `apps/app-frontend/src/components/ui/modal/ModpackAlreadyInstalledModal.vue:1` | confirmed |
| BR-36 | Install flow — progress feedback | indicator | `apps/app-frontend/src/providers/content-install.ts:217` | confirmed |
| BR-37 | Update flow — Switch version (from Browse) | button | `apps/app-frontend/src/pages/project/Versions.vue:12` | confirmed |
| BR-38 | Server-context browse + selection queue | indicator | `apps/app-frontend/src/pages/Browse.vue:905` | confirmed |
| BR-39 | Server browse facets (region / language / status / type) | toggle | `packages/ui/src/utils/server-search.ts:147` | confirmed |
| BR-40 | Empty / no-results / offline / error states | indicator | `packages/ui/src/layouts/shared/browse-tab/layout.vue:259` | confirmed |
| BR-41 | Photosensitivity warning modal | dialog | `packages/ui/src/components/modal/PhotosensitivityWarningModal.vue` (referenced) | `[ORCH-PENDING]` |
| BR-42 | Context menu on a project card (right-click) | menu | `apps/app-frontend/src/composables/browse/use-app-server-browse.ts` | `[ORCH-PENDING]` |

---

## BR-1 — Project-type tabs
**Type:** tab
**Nav path:** `/browse/:projectType` (route name `Discover content`)
**Evidence:** [SOURCE `apps/app-frontend/src/routes.js:50-54`]; [SOURCE `apps/app-frontend/src/pages/Browse.vue:713-735`]; [SOURCE `packages/ui/src/layouts/shared/browse-tab/layout.vue:141-145`]
**Purpose:** The Discover surface is one route parameter deep, not six routes. The page reads `route.params.projectType` into a ref once (`Browse.vue:618`) and watches it for later changes (`Browse.vue:632-648`). The selector is rendered as a pill-style `NavTabs` bar above the search box, not a dropdown — because the type is the single most important axis of the page and must stay visible while the user scrolls a long result list. Tab links carry `:replace="true"` in the app variant, so switching type replaces the history entry rather than stacking ten `/browse/...` entries. The tab list is built from `selectableProjectTypes`, in this fixed UI order: **Modpacks, Mods, Resource Packs, Data Packs, Shaders, Servers**. Only one tab renders if `filteredLinks.length > 1` (`NavTabs.vue:2`). A legacy redirect route (`routes.js:71-76`) maps `/mod|plugin|datapack|resourcepack|shader|modpack/:id/...` onto `/project/:id/...`.
**Operation:** Click a tab → `router.replace` → route param watcher sets `projectType` → the search composable resets `query`, resets sort to `relevance` (`use-browse-search.ts:423-436`) and re-runs the search. **No network on tab change beyond the search request itself.**
**Controls:** Six tabs in fixed order. Labels are `app.browse.project-type.{modpacks|mods|resource-packs|data-packs|shaders|servers}` → `Modpacks`, `Mods`, `Resource Packs`, `Data Packs`, `Shaders`, `Servers`.
**Persistence:** Tab choice lives in the URL path. Search state (query, filters, sort, page, per-page) is mirrored into the URL query string by `createPageParams()` (`utils/search.ts:1051-1092`) and read back on mount by `readQueryParams()` (`utils/search.ts:937-1049`), so a Browse URL is shareable and survives reload.
**Rationale & notes:** Route-param-per-type means every filter combination is a distinct, linkable URL — the same URLs work on modrinth.com. "Modpacks" leads because a modpack is the heaviest decision; "Servers" trails because it is only meaningful without an instance in context.

---

## BR-2 — Project-type visibility rules (instance / server / worlds context)
**Type:** indicator
**Nav path:** `/browse/:projectType` with `?i=<instanceId>` / `?sid=<serverId>` / `?wid=<worldId>` / `?from=worlds`
**Evidence:** [SOURCE `apps/app-frontend/src/pages/Browse.vue:668-736`]
**Purpose:** Which tabs appear is a function of context, because several combinations are meaningless. With **no instance**: all six show. With an **instance**: *Modpacks* and *Servers* hide (you don't browse modpacks while adding content to a specific instance); *Mods* shows only if the instance loader is not `vanilla`; *Data Packs* shows only if the instance game version is at or after `1.13` (`findIndex` comparison against the `1.13` entry in the game-version tag list) and the instance is not a server instance. From the **worlds** surface (`?from=worlds`) the entire tab strip collapses to a single *Servers* tab. In **setup-server** context only *Modpacks* remains. These rules are cheap to re-implement and expensive to get wrong — a user who sees a Mods tab on a vanilla instance will install a mod that cannot load.
**Operation:** Pure computed. `instance` comes from `?i=` via a suspense query (`Browse.vue:153-167`); `isServerInstance` is derived by fetching the linked project's v3 record and checking `minecraft_server != null` (`Browse.vue:173-175`, `161-167`).
**Controls:** `selectableProjectTypes: Array<{label, href, shown?}>`.
**Persistence:** None beyond the URL.
**Rationale & notes:** Suppressing a tab is quieter than greying it out. `hiddenFilterTypes` exists as an injection point in the browse manager but the app does not use it — the project-type suppression is done by omitting `href` entries rather than by hiding filter groups.

---

## BR-3 — Search input
**Type:** field
**Nav path:** `/browse/<type>?q=<query>`
**Evidence:** [SOURCE `packages/ui/src/layouts/shared/browse-tab/layout.vue:147-161`]; [SOURCE `packages/ui/src/utils/search.ts:919-921`]
**Purpose:** The primary refinement control, placed directly under the type tabs. It is a full-width `Input` (`size="large"`, `icon=SearchIcon`, `autocomplete="off"`, `clearable`) bound to `ctx.query`. The placeholder is interpolated per project type: `Search {projectType}…` where `{projectType}` is the pluralised type name (`Search Mods…`, `Search Modpacks…`). There is **no keyboard shortcut** bound to it anywhere in the app at v0.21.9 — no ⌘K palette exists; the only global letter shortcut is `N` on the welcome screen (see module 04, LIB-30).
**Operation:** Every keystroke mutates `query`; the watcher on `effectiveRequestParams` schedules a **200 ms debounce** before calling `refreshSearch()` (`use-browse-search.ts:301-314`). There is no explicit submit/Enter handler — search is purely incremental. Each run calls `options.search(requestParams)` → `get_search_results_v3(...)` → Tauri `plugin:cache|get_search_results_v3` (HTTP to the search API, cached through the app cache with `staleTime: 30_000` at the vue-query layer, `Browse.vue:1080-1089`).
**Controls:** Free text, empty allowed. Clearing via the `clearable` X calls `ctx.clearSearch()` which empties the query and resets to page 1.
**Persistence:** `q` in the URL query (`createPageParams`, `utils/search.ts:1054-1056`). Changing page resets `currentPage` to 1 via a deep watcher (`use-browse-search.ts:285-299`).
**Rationale & notes:** A stale-response guard (`searchVersion` counter, `use-browse-search.ts:350-353`) drops out-of-order responses — necessary because 200 ms debounce + variable latency makes races likely. `loading` only flips to true when the current result set is empty (`use-browse-search.ts:336-341`), so incremental typing does not blank the list on every keystroke.

---

## BR-4 — Search debounce + stale-response guard
**Type:** indicator
**Nav path:** n/a (behaviour of BR-3)
**Evidence:** [SOURCE `packages/ui/src/layouts/shared/browse-tab/composables/use-browse-search.ts:200-201`]; [SOURCE `…/use-browse-search.ts:301-314`]; [SOURCE `…/use-browse-search.ts:350-353`]
**Purpose:** Two mechanisms that together make incremental search feel safe. `searchDebounceTimer` coalesces rapid input; `searchVersion` is a monotonically increasing token captured at the start of `refreshSearch`, and any response whose token is not the latest is discarded. `clearSearchDebounce()` runs on every params change and on `active` transitions so a route change cannot leave a pending timer pointing at a dead page.
**Operation:** Watcher on `effectiveRequestParams` → `refreshing = true` → `clearSearchDebounce()` → if inactive, bail → `setTimeout(refreshSearch, 200)`.
**Controls:** Fixed 200 ms. Not configurable.
**Persistence:** None.
**Rationale & notes:** `refreshing` (distinct from `loading`) exists so the UI can show a subtle "results are updating" affordance while keeping the previous list visible — a deliberate choice over blanking the grid on every filter toggle.

---

## BR-5 — "Sort by:" combobox
**Type:** button
**Nav path:** `/browse/<type>?s=<index>`
**Evidence:** [SOURCE `packages/ui/src/utils/search.ts:389-397`]; [SOURCE `packages/ui/src/layouts/shared/browse-tab/layout.vue:179-195`]
**Purpose:** Five options, in exact UI order, mapping 1:1 onto the API `index` parameter:

| # | UI label | API `index` | What it means |
| --- | --- | --- | --- |
| 1 | `Relevance` | `relevance` | Default. Search-relevance ranking; the only sensible order once a free-text query exists. |
| 2 | `Downloads` | `downloads` | Lifetime download count, descending — proxy for how widely used a project is. |
| 3 | `Followers` | `follows` | Watcher count — proxy for current interest/trust rather than historic use. |
| 4 | `Date published` | `newest` | Newest first. |
| 5 | `Date updated` | `updated` | Most recently changed first. |

The displayed date on the card flips accordingly: when sorted by `newest` the card shows the **published** date, otherwise the **updated** date (`layout.vue:357-359`), so the date on the card always explains the ordering you are looking at.
**Operation:** Select → `ctx.effectiveCurrentSortType.value = val` → `requestParams` recomputes → debounced search. Changing project type resets the sort to `relevance` (`use-browse-search.ts:427-429`).
**Controls:** Single-select, `never-empty` style combobox, 16 rem wide in the app variant. Default `relevance`.
**Persistence:** `s` in the URL, omitted when `relevance` (`utils/search.ts:1081-1083`). Legacy `s` param is also read on mount.
**Rationale & notes:** Labels are hard-coded English strings in `sortTypes` rather than message descriptors, so they are **not** localised — a real localisation gap visible in the `sortTypes` array at `search.ts:390-394`. Downloads-before-follows is the classic "tried a lot" vs "still watched" ordering pair; both are one click away.

---

## BR-6 — "View:" per-page combobox
**Type:** button
**Nav path:** `/browse/<type>?m=<count>`
**Evidence:** [SOURCE `packages/ui/src/utils/search.ts:387`]; [SOURCE `…/use-browse-search.ts:144-146`]; [SOURCE `…/layout.vue:197-212`]
**Purpose:** Page size. Options are `[5, 10, 15, 20, 50, 100]`; the default is **20**. This exists because Modrinth result sets are short (most niche filters return tens of hits) and a 20-row grid wastes a 1080p window. The control's prefix is the literal string `View:` and its placeholder is `View`.
**Operation:** Select → `maxResults` → `requestParams` `limit=` changes → page resets to 1. If the current value is not in the option list, the composable snaps to the numerically nearest option (`use-browse-search.ts:148-154`).
**Controls:** Single-select; integer options above.
**Persistence:** `m` in the URL, omitted when 20 (`utils/search.ts:1084-1086`).
**Rationale & notes:** The app variant exposes no caller-supplied `maxResultsOptions`, so the list is fixed. The Browse page does **not** expose a list/grid density switch (`ctx.cycleDisplayMode` is exposed by the context but never bound in the app), so the effective layout is always `list`.

---

## BR-7 — Pagination
**Type:** button
**Nav path:** `/browse/<type>?page=<n>`
**Evidence:** [SOURCE `packages/ui/src/utils/search.ts:916-933`]; [SOURCE `…/use-browse-search.ts:195-198`]; [SOURCE `…/layout.vue:228-233`]; [SOURCE `…/layout.vue:416-423`]
**Purpose:** Classic **numbered pagination, not infinite scroll**. `pageCount = ceil(totalHits / maxResults)` (with a floor of 1 when there are zero hits). Two `Pagination` components render — one in the toolbar row and one under the results — so the control is reachable after a long list. The control is hidden entirely when `count <= 1` (`Pagination.vue:2`).
**Operation:** Click a page → `setPage(n)` sets `currentPage`, then scrolls the window to top smoothly (`use-browse-search.ts:408-412`). `offset = (currentPage - 1) * maxResults` is sent to the API **only when page ≠ 1**.
**API parameters:** `limit` (page size) and `offset` (0-based record offset), both on the same `new_filters`/search-v3 request string (`utils/search.ts:917`, `927-930`).
**Controls:** First / previous / numbered window with an ellipsis that turns into a "go to page" number input / next.
**Persistence:** `page` in the URL, omitted when 1. Legacy `o` (offset) and `page` params are both read on mount (`utils/search.ts:992-999`).
**Rationale & notes:** Numbered pagination over infinite scroll is the right call for a browse surface because results are a *set* the user may want to return to (page 3 of 942), and because it gives a stable, shareable URL. The dual placement is a direct concession to list length.

---

## BR-8 — Category facets
**Type:** toggle
**Nav path:** `/browse/<type>?f=categories:<name>` (or `?g=` for the resolutions header)
**Evidence:** [SOURCE `packages/ui/src/utils/search.ts:420-445`]; [SOURCE `…/sidebar.vue:244-303`]; [SOURCE `packages/ui/src/components/search/SearchSidebarFilter.vue:102-191`]
**Purpose:** One collapsible group per category *header* (Modrinth's category taxonomy is two-level: header → category). Headers become filter groups named `category_<projectType>_<header>`, display name from `formatCategoryHeader`, and each category option is `categories:<name>` with `method: 'and'` (AND semantics — selecting *Optimization* **and** *Fabric* means both, not either). The `resolutions` header is the exception: it maps to API param `g` (game-version resolutions) and uses `method: 'or'`.
**Operation:** Rendered in the right sidebar as `SearchSidebarFilter` accordions. Options are single-click to **include**, with a secondary ban-icon button (hover-revealed) to **exclude**. In the app variant, category groups and `environment` and `license` default to **open**; everything else defaults closed (`sidebar.vue:121-123`).
**Controls:**
- Multi-select, include + exclude.
- `display: 'all'` — the whole group is listed, no scroll, no "show more".
- Non-searchable within the group.
- Query params: `f` (categories) or `g` (resolutions).
**Persistence:** `f`/`g` URL params, repeated per value. Exclusion is encoded by rewriting `:` to `!=` in the URL value (`utils/search.ts:1192-1201`).
**Rationale & notes:** Categories are the single most-used refinement axis in practice, which is why they default open in the app (they would not on the web). AND semantics for categories plus OR-within-a-resolutions-group is a deliberate taxonomy-specific choice encoded directly in the filter definition.

---

## BR-9 — Loader / Platform facets
**Type:** toggle
**Nav path:** `/browse/mod?g=fabric`
**Evidence:** [SOURCE `packages/ui/src/utils/search.ts:551-688`]; [SOURCE `packages/ui/src/utils/tag-messages.ts:575-577`]
**Purpose:** Five separate filter types, all sharing the `g` query param, one per project type:

| Type | Label | Project types | Display | Default values hoisted |
| --- | --- | --- | --- | --- |
| `mod_loader` | `Loader` | `mod` | expandable | `fabric`, `forge`, `neoforge` |
| `modpack_loader` | `Loader` | `modpack` | all | — |
| `plugin_loader` | `Loader` | `plugin` | expandable | `paper`, `spigot` |
| `plugin_platform` | `Platform` | `plugin` | all | — |
| `shader_loader` | `Loader` | `shader` | expandable | `iris`, `optifine`, `vanilla` |

Options come from the live `/tags/loader` API and are filtered by each loader's `supported_project_types`; `mod_loader` additionally excludes loaders that also support `plugin`/`datapack`, and `plugin_platform` is restricted to the four platform names `bungeecord, waterfall, velocity, geyser` (`utils/search.ts:216`, `565-571`, `651-661`).
**Operation:** Each option maps to `categories:<loader>` with `method: 'or'`, so multiple loaders become `categories IN [...]` in the filter string. "Expandable" means the first N options are shown with a `Show more` / `Show fewer` toggle, and the *default* loaders are floated to the top of the list (`SearchSidebarFilter.vue:305-325`).
**Controls:** Multi-select, include + exclude. Loading a loader on a vanilla instance is possible but the context filter pushes `vanilla` (`Browse.vue:404-408`).
**Persistence:** `g` URL param, repeated. Exclusion via `!=`.
**Rationale & notes:** Loaders are stored as *categories* server-side, which is why they share the category mechanism while presenting under a different label and ordering policy. Hoisting the default loaders is a strong usability decision: Fabric/Forge/NeoForge cover the overwhelming majority of installs, so they are the first three chips.

---

## BR-10 — Game-version facet
**Type:** toggle
**Nav path:** `/browse/mod?v=1.21.4` and `?h=true` for "Show all versions"
**Evidence:** [SOURCE `packages/ui/src/utils/search.ts:513-550`]; [SOURCE `packages/ui/src/components/search/SearchSidebarFilter.vue:211-220`]
**Purpose:** `display: 'scrollable'` (fixed 16 rem scroll panel) with an inline search box (`searchable: true`, placeholder `Search...`). Options come from the `/tags/version` API. Each option maps to `game_versions:<version>` with `method: 'or'`, so selecting several becomes `game_versions IN [...]` — an OR, because a mod that supports 1.21.4 *and* 1.21.5 should match if you asked for either.
Non-release versions (snapshots/alphas/betas) are assigned `toggle_group: 'all_versions'`; the `Show all versions` checkbox (query param `h`) reveals them. Releases are always visible. Ordering of this group is forced: `ordering: 2` when browsing mods and `-1` when browsing shaders (`search.ts:545-549`), so for mods it sits directly under Loader, and for shaders it sinks to the bottom because shader packs rarely pin a game version.
**Operation:** Click to include; ban-icon to exclude (exclusion encoded `game_versions!=x`).
**Controls:** Multi-select, include only (no exclusion — `supports: ['include']`). Searchable. Toggle-group checkbox.
**Persistence:** `v` per value; `h=true` for the toggle group.
**Rationale & notes:** Include-only for game versions is a deliberate simplification: "not 1.21.4" is almost never what a user wants and produces a confusing result set. The loader/game-version pairing (Loader first for mods, Game version last for shaders) encodes an assumed reading order.

---

## BR-11 — Environment facet
**Type:** toggle
**Nav path:** `/browse/mod?e=client`
**Evidence:** [SOURCE `packages/ui/src/utils/search.ts:473-512`]; [SOURCE `…/search.ts:1156-1183`]
**Purpose:** Only available for `mod` and `modpack`. Two options: `Client` (client icon) and `Server` (server icon), `display: 'all'`, include-only. The emitted filter is **not** a single field — the option carries an `environment` value rather than a `field:value` pair, and `getEnvironmentFilterGroups` expands the selection into the underlying environment taxonomy: selecting Client produces `(environment:client_only OR environment:client_only_server_optional OR environment:client_or_server_prefers_both OR environment:client_or_server)`, selecting Server the mirror set, and selecting **both** produces a five-value OR including `client_and_server`. So "Client" really means "works on a client", not "client-only".
**Operation:** Click to toggle include. Defaults to open in the app sidebar.
**Controls:** Multi-select, include only, two options.
**Persistence:** `e` URL param.
**Rationale & notes:** This is the single most consequential facet for "will this break my server", and it is placed among the always-open groups in the app for that reason. Selecting both Client and Server is meaningful (works everywhere) and yields `client_and_server` results too.

---

## BR-12 — License facet (open source)
**Type:** toggle
**Nav path:** `/browse/mod?l=open_source:true`
**Evidence:** [SOURCE `packages/ui/src/utils/search.ts:689-712`]; [SOURCE `apps/app-frontend/src/locales/en-US/index.json` → `search.filter_type.license`, `search.filter_type.license.open_source`]
**Purpose:** Label `License`. At v0.21.9 there is exactly **one** option: `Open source`, value `open_source:true`, `method: 'and'`, include + exclude, `display: 'all'`, not searchable. It applies to every project type except `server`. There is **no per-license selection** (no MIT/Apache/GPL list) and **no "closed source" option** — closed source is the implicit absence of a selection.
**Operation:** Click to include; ban-icon to exclude.
**Controls:** Single option, include + exclude.
**Persistence:** `l` URL param with the literal value `open_source:true`; a legacy boolean form `l=true` is also read on mount (`search.ts:941-955`).
**Rationale & notes:** One binary checkbox rather than a licence picker is a scoping decision — enough to answer "is this auditable?" without implying the app validates licence compatibility. `display: 'all'` + open-by-default in the app keeps it one click away without dominating the sidebar.

---

## BR-13 — Advanced-exclusions facet (disclosures)
**Type:** toggle
**Nav path:** `/browse/mod?a=ai_content` (repeated) / `?a=disclosure_types:ai_content`
**Evidence:** [SOURCE `packages/ui/src/utils/search.ts:226-236`]; [SOURCE `…/search.ts:338-378`]; [SOURCE `…/search.ts:729-753`]; [SOURCE `packages/ui/src/locales/en-US/index.json` → `search.filter_type.advanced`, `search.filter_type.advanced.disclosure.*`]
**Purpose:** A single accordion labelled `Advanced exclusions`, `ordering: -1000` so it always sorts last, `supports: ['exclude']` only, `query_param: 'a'`. Its options are content-safety disclosures, each of which excludes the project rather than including it:

| Option label | Value |
| --- | --- |
| `AI-generated content` | `disclosure_types:ai_content` (with sub-options per AI usage) |
| `Generative AI functionality` | `disclosure_types:ai_functionality` |
| `Advertisements` | `disclosure_types:advertisements` |
| `Photosensitivity triggers` | `disclosure_types:epilepsy_triggers` |
| `External system interactions` | `disclosure_types:system_interactions` |
| `Telemetry` | `disclosure_types:telemetry` (with sub-options per consent level) |
| `Derivative content` | `disclosure_types:derivative_work` |
| `Paid features` | `disclosure_types:paid_features` |
| `Archived` | `disclosure_types:archived` |

`ai_content` and `telemetry` have **expandable sub-options** (`search.filter.dependency_type.*` labels: `Any dependency type`, `Embedded`, `Optional`, `Optional or embedded`, `Required`, `Required or embedded`, `Required or optional` for dependency graphs; and `AI {usage}` sub-labels for AI). On this facet the options carry **no icon rendering in the sidebar list** — the icon map exists (`SparklesIcon`, `BrainCogIcon`, `MegaphoneIcon`, `EyeIcon`, `MonitorCogIcon`, `RadioTowerIcon`, `GitForkIcon`, `CircleDollarSignIcon`, `ArchiveIcon`) and is attached to each option, but `SearchSidebarFilter` renders it as inline SVG at 1rem which the disclosure icons are sized to fit.
Mixed into the same `advanced` facet are **cross-project-type exclusions**: on `mod` you can exclude `plugin` and `datapack` results; on `plugin` exclude `mod` and `datapack`; on `datapack` exclude `mod` and `plugin`. These compile to `all_project_types NOT IN [...]` (`search.ts:901-911`).
**Operation:** Click to exclude. Selected exclusions are echoed as red-banned chips in the filter strip.
**Controls:** Multi-select, exclude only. The facet is hidden entirely if it would have zero options for the current project type (`search.ts:762`).
**Persistence:** `a` URL param; and — uniquely — the selected option IDs are **persisted to localStorage** (see BR-19).
**Rationale & notes:** Making this an exclusion-only, always-last, opt-in facet is the right shape: these are trust signals, not taste signals, so they should never silently reshape results. The photosensitivity exclusion additionally triggers a one-time safety modal (BR-41).

---

## BR-14 — "Depends on" / "Included content" project filter
**Type:** field
**Nav path:** `/browse/mod?dep=<projectId>`
**Evidence:** [SOURCE `packages/ui/src/utils/search.ts:458-472`]; [SOURCE `packages/ui/src/utils/search.ts:137-164`]; [SOURCE `packages/ui/src/components/search/SearchDependsOnFilter.vue`]
**Purpose:** The only **free-text** facet: a project picker that resolves names to IDs and emits `compatible_dependency_project_ids`. Its label is type-dependent — `Depends on` for everything except modpacks, `Included content` for modpacks (`search.ts:461`, locale `search.filter_type.compatible_dependency_project_ids` / `search.filter_type.included_content`). It accepts arbitrary project IDs (`allows_custom_options: 'and'`, `custom_option_field`), so a shared URL can name a project the searcher cannot resolve.
The option string is a **compound encoding** `dependencyTypes:projectId`, e.g. `required,optional:abcd1234`. This matters for mods: a modpack filter means "includes this project", but a mod filter means "depends on this project **by at least one of these dependency types**". The builder expands it to `(required_dependency_project_ids = X OR optional_dependency_project_ids = X OR embedded_dependency_project_ids = X)`, and negates with AND when excluded (`search.ts:781-799`). For modpacks it collapses to the single `compatible_dependency_project_ids` field.
**Operation:** Search-by-name against the projects endpoint; each selection becomes a chip — `Depends on: {name}` / `Includes: {names}` / `Excludes: {names}` in the chip strip (`SearchFilterControl.vue`).
**Controls:** Multi-add, include + exclude. A clear button per dependency filter. Ordered above everything (`ordering: -999`).
**Persistence:** `dep` URL param, repeated.
**Rationale & notes:** This is the feature that answers "what breaks if I remove X?" and "what do I already have that satisfies this?". It is `display: 'depends-on-project'`, a dedicated renderer that also shows a live result count, and it always opens by default (`sidebar.vue:110-112`) because it is the highest-information-per-click control in the sidebar.

---

## BR-15 — Project-ID filter (hidden)
**Type:** toggle
**Nav path:** `/browse/mod?pid=<projectId>`
**Evidence:** [SOURCE `packages/ui/src/utils/search.ts:713-728`]
**Purpose:** An include/exclude-by-ID filter with `display: 'none'` — it renders in neither the sidebar nor the main filter control, and exists only to be populated from the URL. `use-library`'s own browse-side "hide installed" machinery uses it (`Browse.vue:430-432` pushes `{type: 'project_id', option: 'project_id:<id>', negative: true}`).
**Operation:** None visible; URL-driven.
**Controls:** `allows_custom_options: 'and'`, include + exclude, `query_param: 'pid'`.
**Persistence:** URL only.
**Rationale & notes:** Infrastructure, not UI. It is how "hide what I already have" is expressed in the same filter language as everything else rather than as a separate mechanism.

---

## BR-16 — Provided (locked) filters and "Sync with instance"
**Type:** toggle
**Nav path:** `/browse/mod?i=<instanceId>&v=<instanceVersion>&g=<instanceLoader>`
**Evidence:** [SOURCE `apps/app-frontend/src/pages/Browse.vue:400-436`]; [SOURCE `…/Browse.vue:1143-1149`]; [SOURCE `packages/ui/src/layouts/shared/browse-tab/sidebar.vue:288-301`]; [SOURCE `…/SearchSidebarFilter.vue:53-70`]; [SOURCE `…/SearchSidebarFilter.vue:222-238`]
**Purpose:** When Browse is entered from an instance (`?i=`), the instance's own constraints are injected as **provided filters** and are excluded from the filter strip's remove-all, drawn with a padlock icon and tooltip `Provided by the instance`. Three can be injected:
- `game_version` = the instance's game version, unless it is a vanilla shader (in which case `shader_loader: vanilla` is injected instead) or the project type is `resourcepack`.
- `mod_loader` = the instance loader, only for `mod` browse and only for `fabric|forge|quilt|neoforge`.
- `environment: client` when the instance is a server instance.
Each locked facet shows a slot-supplied title and description — `Game version is provided by the instance`, `Loader is provided by the instance`, `Only client-side mods can be added to the server instance` — plus an **Unlock filter** button with the sub-copy `Unlocking this filter may allow you to install incompatible content.`
**Operation:** Clicking **Unlock filter** pushes the filter type onto `overriddenProvidedFilterTypes`, which (a) drops the provided value from the compiled query and (b) marks the facet unlocked so the user can pick their own. A `Sync with instance` button appears on any overridden facet and restores the instance constraint plus clears the facet's own selections.
**Controls:** Per-type lock/unlock. Provided filters carry `negative: true` for the hide-installed pseudo-filter, so they are excluded from install-preference resolution (see BR-32).
**Persistence:** `overriddenProvidedFilterTypes` is in-memory only (a `Ref<string[]>`, `search.ts:404`); the *effect* persists because the resulting URL carries the user's own `v=`/`g=`.
**Rationale & notes:** This is the load-bearing safety feature of instance-scoped Browse. It makes it structurally impossible to install an incompatible mod without two deliberate clicks, and it explains the install affordance rather than hiding it.

---

## BR-17 — "Hide content already installed" toggle
**Type:** toggle
**Nav path:** `/browse/modpack?ai=true` (instance) / `/browse/server?shi=true` (server)
**Evidence:** [SOURCE `apps/app-frontend/src/pages/Browse.vue:1279-1314`]; [SOURCE `packages/ui/src/layouts/shared/browse-tab/sidebar.vue:163-211`]; [SOURCE `apps/app-frontend/src/locales/en-US/index.json` → `label.hide-installed-content`, `app.browse.hide-installed-modpacks`, `app.browse.hide-added-servers`]
**Purpose:** The only toggle group in the sidebar above the facets, shown whenever a context exists. The label is context-dependent: `Hide content already installed` normally, `Hide already installed` for modpacks, `Hide servers already added` from the worlds surface. When on, the set of already-present project IDs is pushed as **negative `project_id` filters**, so it is expressed in the same filter language as everything else and is visible/removable in the chip strip.
**Operation:** Toggle → negative filters injected → debounced re-search. Two adjacent toggles share the panel: `Hide selected content` (server queue) and `Server only`.
**Controls:** Single boolean each. Not persisted server-side; persisted only as `ai=true` / `shi=true` in the URL.
**Rationale & notes:** Separate booleans for installed (`ai`) and selected (`shi`) exist because the server browse lets you build a multi-item install queue, and hiding "already installed" is not the same question as hiding "already picked but not yet installed".

---

## BR-18 — Filter chip strip above results
**Type:** indicator
**Nav path:** reflects the current filter set
**Evidence:** [SOURCE `packages/ui/src/components/search/SearchFilterControl.vue:1-53`]; [SOURCE `…/SearchFilterControl.vue:130-200`]
**Purpose:** A row of removable chips summarising every active filter, placed between the toolbar and the result list. Chip shapes: `Clear all filters` (X-circle, only when >1 filter selected), `Includes: {projects}` / `Depends on: {name}` (modpack vs mod), `Excludes: {projects}` (with a red ban icon), one chip per selected filter option (with a ban icon when negative), and one padlocked chip per provided filter (tooltip `Provided by the instance`). This is the only place the user can see the whole active filter set at once, and the only place provided filters are visible without opening the sidebar.
**Operation:** Click a chip's X (or the whole chip's `action` slot) to remove that filter value; `Clear all filters` empties `selectedFilters` but leaves provided filters intact.
**Controls:** Read + remove, no add.
**Persistence:** Mirrors the URL.
**Rationale & notes:** The sidebar is long and scrolly; the chip strip is the flat summary. Distinguishing *provided* chips with a padlock and a different background is the load-bearing visual here — a locked filter must not look removable.

---

## BR-19 — Advanced-exclusions persistence and link override
**Type:** indicator
**Nav path:** n/a (localStorage)
**Evidence:** [SOURCE `packages/ui/src/utils/advanced-filter-preferences.ts:6-10`]; [SOURCE `packages/ui/src/layouts/shared/browse-tab/composables/use-browse-search.ts:218-283`]; [SOURCE `packages/ui/src/layouts/shared/browse-tab/layout.vue:163-176`]; [SOURCE `packages/ui/src/layouts/shared/browse-tab/components/AdvancedFiltersPersistenceNote.vue`]; [SOURCE `packages/ui/src/locales/en-US/index.json` → `browse.advanced-filters.*`]
**Purpose:** Advanced exclusions (BR-13) are the only filters that survive a session. They are stored in `localStorage` under `modrinth-advanced-exclusion-filters` as a sorted array of option IDs. On entering Browse: if the URL carries **no** search state at all, the saved exclusions are re-applied automatically; if the URL **does** carry filters, they are not, and instead an info Admonition appears above the toolbar — `This link's filters differ from your saved advanced exclusions` — with a single button `Apply saved preferences`. The sidebar also shows a persistent note inside the `Advanced exclusions` group: `Advanced exclusions are saved across sessions. Opening a shared link with filters will override them.`
**Operation:** Changing any advanced option writes through to localStorage immediately (a watcher on the selected IDs merges into the stored set). `Apply saved preferences` replaces the current advanced filters with the saved ones.
**Controls:** Per-option exclude toggles; one "apply saved" button when overridden.
**Persistence:** `localStorage['modrinth-advanced-exclusion-filters']`, global (not per project type — options not valid for the current project type are filtered out of the saved set on read).
**Rationale & notes:** This is a genuinely thoughtful piece of state design. Trust filters are the ones a user wants permanently; but silently letting a shared link silently *remove* your trust settings would be hostile, and silently *keeping* them would make the shared link lie about what it shows. The Admonition resolves both.

---

## BR-20 — Project card (list layout) — metadata and intent
**Type:** indicator
**Nav path:** `/project/<id>` (whole card is the link target)
**Evidence:** [SOURCE `packages/ui/src/components/project/card/ProjectCard.vue:100-187`]; [SOURCE `…/ProjectCard.vue:311-325`]; [SOURCE `…/layout.vue:340-412`]; [SOURCE `packages/ui/src/components/project/card/ProjectCardStats.vue:17-49`]; [SOURCE `…/ProjectCardDate.vue:23-49`]; [SOURCE `…/ProjectCardAuthor.vue:16-38`]
**Purpose:** The app always renders the **list** variant (`effectiveLayout` is `'list'` because `displayMode` is not supplied, `use-browse-search.ts:156-161`). It is a 4-column CSS grid — `icon | info | actions | stats` over `icon | info | … | tags` — that re-flows at container widths 850 px and 550 px, dropping to two columns and shrinking the icon to 64 px. Every element and the decision it supports:

| Element | Data | Decision it helps the user make |
| --- | --- | --- |
| Icon (96 px in list, 100 px declared) | `result.icon_url` | *Do I recognise this project?* Pure recognition — the primary filter before reading any text. |
| Title | `result.name`, 1-line clamp | *What is it?* |
| `by {author}` — organisation name if present, else `author`; links to `/user/<name>` in-app, external `https://modrinth.com/organization/<slug>` when org | `result.organization ?? result.author`, `result.author_id` | *Can I trust it / is it maintained by a team or a person?* Organisation-vs-individual is the single best cheap proxy for durability. |
| Summary, 2-line clamp | `result.summary` | *Is this the thing I want?* The description is the decision input; 2 lines is the budget. |
| Environment tag (mod/modpack only) | `result.project_loader_fields.environment[0]` → `Client` / `Server` / `Client or server` / `Client and server` / `Singleplayer` / `Dedicated server` | *Will this break my server?* Answered before the user clicks. |
| Category / loader tags | `display_categories` + every loader in `result.loaders` (except `mrpack`) + `project_loader_fields.mrpack_loaders` for modpacks | *What is it, and will it load?* Loader badges are the compatibility contract. Deprioritised tags are pushed to the end of the sort so tags matching an active filter do not eat the budget. |
| Download count | `result.downloads`, compact (18.5M) with tooltip `18,570,000 downloads` | *Is this battle-tested?* Historic usage. |
| Follower count | `result.follows`, compact with tooltip `42,000 followers` | *Is this currently maintained/popular?* Differentiates a dead-but-famous project from a live one. |
| Date | `result.date_modified` or `result.date_created` depending on sort, relative (`3 days ago`) with tooltip `Updated {long date}` | *Is it alive?* Coupled to the active sort so the date explains the ordering. |
| Action button | see BR-22 | *Do it now without leaving the list?* |
| Hidden extras | `banner` = `featured_gallery[0]`, `color` → OKLCH gradient, `status` badge | Sorting/visual noise only; not rendered in the list variant's body. |

Overflow tags collapse into a `TagsOverflow` chip. Loader tags can be hidden entirely (`excludeLoaders`) when the user has explicitly chosen a loader filter — the tag then carries no information.
**Operation:** Hover highlights the card (`smart-clickable:highlight-on-hover`); `mouseenter`/`mouseleave` emit to the browse manager so a caller can prefetch. Right-click opens the context menu (BR-42).
**Controls:** None interactive except the card link and the action button. Card link is `/project/<projectId>` plus the originating browse query, including a `b=` back-link (`Browse.vue:1265-1272`).
**Persistence:** None.
**Rationale & notes:** Downloads *and* followers are both shown because they answer different questions — one is historical confidence, the other is current relevance; a project can have high downloads and near-zero followers (abandoned) or vice versa (new and hyped). The organisation-over-author substitution is a deliberate hierarchy: team-maintained reads as more durable than individual-maintained. The `deprioritizedTags` mechanism (BR-20 row 6) is subtle but important: without it, selecting "Fabric" in the sidebar would spend the tag budget on the one tag you already filtered for.

---

## BR-21 — Project card (grid layout) — banner and colour
**Type:** indicator
**Nav path:** n/a (not reachable in the app at v0.21.9)
**Evidence:** [SOURCE `packages/ui/src/components/project/card/ProjectCard.vue:11-99`]; [SOURCE `…/ProjectCard.vue:407-419`]
**Purpose:** The shared UI package ships a `grid` variant that the web frontend uses and the app does not (`displayMode` is never supplied; `ctx.cycleDisplayMode` is never bound). Recorded here because the component is shared and any future density switch would light it up. It renders a 2:1 banner (`featured_gallery[0]`, or a placeholder), the same icon/title/author/summary, an environment chip that **replaces** one tag slot rather than adding one, up to 6 tags, and the stats/date row pinned to the bottom.
**Operation:** n/a in the app.
**Controls:** `maxTags` default 6; `(maxTags + (environment ? 0 : 1))`.
**Persistence:** n/a.
**Rationale & notes:** `bg-project-gradient` derives a two-stop gradient from the project's numeric `color` via `oklch(from … l*0.8 c*0.8 h+15)`, so the banner band is a colour wash rather than a flat fill — deliberately subdued so a grid of them is not garish.

---

## BR-22 — Card action button
**Type:** button
**Nav path:** triggers the install flow (BR-30)
**Evidence:** [SOURCE `apps/app-frontend/src/pages/Browse.vue:997-1053`]; [SOURCE `apps/app-frontend/src/locales/en-US/index.json` → `button.install`, `label.installed`, `app.browse.add-to-an-instance`]
**Purpose:** One button per card, `type: 'outlined'`, `color: 'brand'`, with four label/icon/disabled states:

| State | Label | Icon | Disabled |
| --- | --- | --- | --- |
| Not installed, browsing **with** an instance in context, or the project is a modpack | `Install` | `+` | no |
| Not installed, browsing **without** instance context, non-modpack | `Add to an instance` | `+` | no |
| Installed in the target instance / queue | `Installed` | `✓` | **yes** |
| In flight | `Installing` | spinner (`animate-spin`) | **yes** |

In server context the label set expands to `Selected` (queued, green outline) and `Validating` (queued, plan being resolved). Clicking a queued root project removes it from the queue.
The "why" for each label: `Add to an instance` is the honest label when the destination is unknown — the click opens the instance picker (BR-30). `Installed` is disabled rather than a no-op so the card reads as a state, not a broken button. Modpacks get `Install` even without an instance context because a modpack *creates* an instance rather than joining one.
**Operation:** Non-modpack with an instance → `chooseInstanceInstallVersion` (BR-32) then `installVersion(...)` from the content-install provider. Modpack → `chooseFilterMatchingInstallVersion` then the same. Callback updates the card to `Installed` and reports the installed IDs so the "hide installed" set refreshes.
**Controls:** One primary action; hover tooltip inherits the label.
**Persistence:** None.
**Rationale & notes:** The button is `outlined` rather than filled so a grid of twenty cards is not a wall of solid brand colour. Disabled-on-installed is the right call: the affordance that says "this is done" should not be clickable.

---

## BR-23 — Project-type header: breadcrumbs and install-context header
**Type:** indicator
**Nav path:** n/a
**Evidence:** [SOURCE `apps/app-frontend/src/pages/Browse.vue:87-110`]; [SOURCE `…/Browse.vue:217-249`]; [SOURCE `packages/ui/src/layouts/shared/browse-tab/layout.vue:130-139`]; [SOURCE `packages/ui/src/layouts/shared/browse-tab/header.vue:57-86`]
**Purpose:** Two headers, both essential for orientation when Browse is embedded in a task.
1. **Breadcrumb**, label `Discover {projectType}` (or `Discover servers`), with an icon; parented under `Instance → {name}` when `?i=` is set and under `Servers → {server}` when `?sid=` is set. The breadcrumb id embeds projectType + instance + server + from, so it rebuilds when context changes.
2. **Sticky install header**, only when an install context exists. Sticky at `top: 0` with a bottom border, containing a back button and a metadata row: the heading (`Installing content`, or `Adding server to instance`), `Minecraft {gameVersion}` with a gamepad icon, and the loader name with its loader icon. It exists so the user never loses track of *which instance they are installing into* while scrolling a long result list.
**Operation:** Back button navigates to `backUrl`; if a server install queue has un-installed selections it prompts first via `SelectedProjectsLeaveModal` — `Selected projects not installed yet` / `You have selected {count} to install. Install them now or go back without installing them.` with `Discard` and an install action.
**Controls:** Back, and (server context) Install selected / Discard.
**Persistence:** None.
**Rationale & notes:** The "don't navigate away with pending selections" guard is a good pattern — it is the same reasoning as a dirty-form warning, applied to a transient queue.

---

## BR-24 — Project detail page header
**Type:** indicator
**Nav path:** `/project/<id>`
**Evidence:** [SOURCE `apps/app-frontend/src/pages/project/Index.vue:63-173`]; [SOURCE `packages/ui/src/components/project/ProjectPageHeader.vue:1-56`]; [SOURCE `packages/ui/src/locales/en-US/index.json` → `project.stats.downloads-label`, `project.stats.followers-label`]
**Purpose:** A `PageHeader` with: 96 px icon; status badge (shown when status ≠ `approved`, i.e. draft/processing/unlisted); title; summary; a metadata row of **Downloads** and **Followers** compact counters (each with a full-number tooltip) plus one clickable tag per project category; and the action slot. For server projects the metadata row is replaced by a `ServerDetails` block (online players, verified plays, status).
Categories in the header are clickable and push to `/browse/<type>?f=categories:<name>` — i.e. the header is a one-click "show me more like this" affordance rather than decoration.

**Actions**, in order:
- **Installed** (disabled, `✓`) when already installed in the context instance **and** on the Versions tab.
- **Switch version** (swap icon) when already installed and not on the Versions tab → routes to `/project/<id>/versions?g=<loader>&v=<gameVersion>`.
- **Install** (download icon, `color: brand`, `size: xl`) otherwise. Label morphs: `Installed` → `Validating` → `Installing...` → `Selected` → `Install`.
- **More options** overflow: `Follow` (disabled, tooltip `Coming soon`), `Save` (disabled, `Coming soon`), `Open in Modrinth`, divider, `Report` (red).

For **server projects** the action slot is entirely different: `Stop` (red) or `Play` (brand), an `+` icon button to add the server to an instance, and an overflow of `Open in Modrinth` / `Report`.

**Operation:** `install(null)` → the content-install provider. `Follow`/`Save` are visibly disabled stubs.
**Controls:** Category tags; primary action; overflow menu; right-click on the header opens a context menu (`Install`, `Open in Modrinth`, `Copy link`).
**Persistence:** None.
**Rationale & notes:** Disabling Follow/Save with a `Coming soon` tooltip rather than hiding them is a deliberate choice — it signals direction without shipping a half-feature. The `Switch version` substitution for `Install` when already installed prevents the most common confusion ("why does it install again?").

---

## BR-25 — Description tab
**Type:** tab
**Nav path:** `/project/<id>` (child route `''`, route name `Description`)
**Evidence:** [SOURCE `apps/app-frontend/src/routes.js:83-87`]; [SOURCE `apps/app-frontend/src/pages/project/Description.vue:1-5`]
**Purpose:** Default landing child route. Renders the project's `body` (Markdown) inside a `Card` via the shared `ProjectPageDescription`. Nothing else — no edit affordance, no action. Its entire job is to be the thing you read after the header has told you whether this project is worth reading.
**Operation:** None.
**Controls:** None.
**Persistence:** None.
**Rationale & notes:** The route name `''` with no path means the project detail URL is clean (`/project/<id>`), which is what gets shared. A `Changelog.vue` file exists in the same directory but is an empty stub and is **not** routed (`apps/app-frontend/src/pages/project/Changelog.vue:1-7`) — dead scaffolding.

---

## BR-26 — Versions tab
**Type:** tab
**Nav path:** `/project/<id>/versions`
**Evidence:** [SOURCE `apps/app-frontend/src/routes.js:88-92`]; [SOURCE `apps/app-frontend/src/pages/project/Versions.vue:1-47`]; [SOURCE `packages/ui/src/components/project/ProjectPageVersions.vue:554-609`]; [SOURCE `…/ProjectPageVersions.vue:1-40`]
**Purpose:** The version history table. Hidden entirely for server projects (`shown: projectV3?.minecraft_server == null`, `Index.vue:184`) and hidden for shaders/others where there is nothing to switch.

**Columns:** *(channel indicator)* | **Version** | **Game version** | **Platform** | [**Environment**, behind the `show_version_environment_column` feature flag] | **Published** | **Downloads** | *(actions)*.

- **Channel** — `VersionChannelIndicator` dot (release/beta/alpha/snapshot), clickable to filter the table by that channel.
- **Version** — `version_number` as the link target, with a `Withheld` tag when `files_missing_attribution`.
- **Game version** — grouped version tags (e.g. "1.21 – 1.21.4"), clickable to filter.
- **Platform** — loader tags (`ModLoader` for `minecraft`), clickable to filter; empty with a "No mod loader" indicator for pure-vanilla modpacks.
- **Published / Downloads** — date and per-version download count.
- Each row expands to show the version's files with filename and size, primary file starred.

**Filters** (`VersionFilterControl`): game version, platform, channel, environment — all client-side, all with tooltips `Toggle {filter}`.
**Pagination:** client-side, page size 20, with a `Showing {first} to {last} of {total}` caption.

**Actions per row:**
- Download icon → `Install` this version (tooltip `Install`).
- Swap icon → `Switch to version` when the project is installed at a different version.
- Check icon, disabled → `Already installed` when this row is the installed version.
- External-link button → open this version on modrinth.com.

The Versions tab link carries `?g=<loader>&v=<gameVersion>` derived from the context instance (`Index.vue:440-456`), so the table pre-filters to the versions that matter for the instance you came from.
**Operation:** Row click → `/project/<id>/version/<versionId>`.
**Controls:** Version filters; pagination; per-row install/switch.
**Persistence:** Table filters are local component state; the tab itself is URL-addressable.
**Rationale & notes:** Two things stand out. First, the per-row action is a *verb* not a button: `Install` → `Switch to version` → `Already installed` as a three-way state on the same control, so the user never has to guess what clicking will do. Second, the `g`/`v` pre-filter carried in the link is what makes the "Switch version" round-trip (BR-37) coherent.

---

## BR-27 — Version tab (single version)
**Type:** tab
**Nav path:** `/project/<id>/version/:version`
**Evidence:** [SOURCE `apps/app-frontend/src/routes.js:93-98`]; [SOURCE `apps/app-frontend/src/pages/project/Version.vue:1-79`]; [SOURCE `packages/ui/src/components/version/VersionPage.vue:260-309`]
**Purpose:** Full detail for one version. A `BackToParentLink` (`All versions`) sits above it and registers a breadcrumb `Version {number}`.

**Sections** (from `VersionPage`'s message table):
- **Compatibility** — game versions and loaders for this version.
- **Required content** — required dependencies, each rendered as a `VersionDependencyItem` with project icon, name and version.
- **Optional dependencies** — same, but visually distinguished.
- **Known incompatibilities** — `incompatible` dependency entries.
- **Content** / **Included content** — the modpack's member projects (modpacks only), with a `Search content...` box.
- **Changes** — the changelog; empty state `No changelog was provided.`
- **Files** — the primary and supplementary files.
- **Supplementary resources** — a table of **File**, **Type**, **Size**, each with a `Download in browser` link.
- `No mod loader` indicator for vanilla modpack versions.

Dependencies are **enriched client-side**: the page collects every `project_id` and `version_id` from the raw dependency list and batch-fetches `get_project_many` / `get_version_many` so the dependency rows show real names and links rather than IDs (`Version.vue:191-226`).

**Header actions:** the primary install button (tooltip = the filename and formatted size of the primary file), plus an overflow with `Open in browser` and `Report`.
**Operation:** `install(version.id)` — a specific version, no picker when the tab is reached with `?i=`.
**Controls:** Install/Switch/Installed states; dependency links navigate to the dependency's project or version, preserving the originating query.
**Persistence:** URL.
**Rationale & notes:** The dependency enrichment is the thing that makes this page useful rather than a JSON dump: the single most common support question ("what does this need?") is answered with names, icons and links.

---

## BR-28 — Gallery tab
**Type:** tab
**Nav path:** `/project/<id>/gallery`
**Evidence:** [SOURCE `apps/app-frontend/src/routes.js:99-103`]; [SOURCE `apps/app-frontend/src/pages/project/Gallery.vue:1-88`]; [SOURCE `apps/app-frontend/src/pages/project/Index.vue:186-190`]
**Purpose:** Shown **only when `project.gallery.length > 0`**. A responsive grid of cards (`repeat(auto-fill, minmax(20rem, 1fr))`), each with a 2:1 image, the image's `title`, its `description`, and its upload date in long form with a calendar icon. Clicking an image opens a full-screen `ImageViewerEditor` in `editor="disabled"` mode with left/right navigation and a per-item action button `Open image in new tab` (labelled "Open image in new tab" for a11y).
A synthetic entry titled `__mc_server_banner__` is filtered out (`Gallery.vue:44`, `59-61`) so server banner art does not appear in the user-facing gallery.
**Operation:** Analytics events `GalleryImageExpand`, `GalleryImageNext`, `GalleryImagePrevious` carry `project_id` + `url`.
**Controls:** Expand, navigate, open externally. No editing.
**Persistence:** None.
**Rationale & notes:** Gallery-first is the correct default for mods: the screenshot *is* the product description, and the choice to place it as a sibling tab rather than a sidebar strip keeps the Description tab readable.

---

## BR-29 — Project right-hand sidebar
**Type:** indicator
**Nav path:** `/project/<id>` (teleported into `#sidebar-teleport-target`)
**Evidence:** [SOURCE `apps/app-frontend/src/pages/project/Index.vue:3-47`]; [SOURCE `packages/ui/src/components/project/ProjectSidebarCompatibility.vue`]; [SOURCE `…/ProjectSidebarDetails.vue:24-220`]; [SOURCE `…/ProjectSidebarTags.vue`]; [SOURCE `…/ProjectSidebarCreators.vue`]; [SOURCE `…/ProjectSidebarLinks.vue`]
**Purpose:** Five stacked sections, in order:
1. **Compatibility** (`ProjectSidebarCompatibility`) — game versions and loaders this project supports, with counts. Replaced for server projects by `ProjectSidebarServerInfo` (status, ping, player count, required content, recommended version, modpack loaders).
2. **Links** — the project's source/issue/wiki/social URLs, each opening in a new tab.
3. **Tags** — the full category list, each clickable back into Browse with that category filtered.
4. **Creators** — team members with avatars; org link goes external, user link goes to the in-app `/user/<name>` page.
5. **Details** — a disclosure/attribution stack: photosensitivity triggers, AI-generated content, generative-AI functionality, advertisements, paid features, telemetry (with the consent level and what data is collected), external system interactions, **License** (linking to the licence URL, or opening a modal with the licence text fetched on hover), derivative work (with the source list and a `Show more` disclosure), followers (servers only), and published/created/queued dates.

This is the trust-and-provenance panel. Its purpose is to answer "should I trust this enough to run its code on my machine" without leaving the page.
**Operation:** Every link is an external `target="_blank"` except the in-app user link (`user-link-target: null`).
**Controls:** Category tags (→ Browse), org/user links, licence modal, derivative-work `Show more`.
**Persistence:** None.
**Rationale & notes:** The licence modal is fetched lazily on hover (`enableLicenseFetch`) so the licence text costs nothing until wanted. Putting every disclosure in one section rather than scattering badges means the user has exactly one place to check.

---

## BR-30 — Install flow — instance picker
**Type:** dialog
**Nav path:** opened from a Browse card action or the project header
**Evidence:** [SOURCE `packages/ui/src/layouts/shared/content-tab/components/modals/ContentInstallModal.vue:1-276`]; [SOURCE `apps/app-frontend/src/providers/content-install.ts:427-576`]; [SOURCE `packages/ui/src/locales/en-US/index.json` → `instances.content-install.*`]
**Purpose:** This is the first step of the install flow and the answer to "which instance?". A modal titled `Install project`, up to 560 px, with an optional project banner at the top (icon, title, owner avatar + name, linking back to the project).

Below the banner, an `Instance type` chip group with two tabs: **Existing instance** and **New instance**. The New tab is only offered if the project supports at least one loader (`compatibleLoaders.length > 0`).

**Existing instance tab:**
- A search field (`Search instance`) and an eye toggle that defaults to **Hide unavailable** (i.e. only compatible-and-not-installed instances are listed; clicking shows the rest).
- One row per instance: icon, name (click → navigate to the instance page), and a right-hand button.
- Row states: `Installed` (disabled, check), `Installing...` (disabled, spinner), `Install` (base button) when compatible, and an outlined orange `Install` with a warning triangle when incompatible, with tooltip `This instance uses a different loader or game version than this project supports.`
- Sorting is deliberate: compatible+not-installed first, then installed, then incompatible — so the right answer is at the top.
- Footer left: `{count} compatible {instance|instances}`. Footer right: `Cancel`.
- Empty state: `No compatible instances found`.

**Compatibility is computed server-side**, not guessed: the provider derives every `(game_version, loader)` pair the project has a version for and calls `get_install_candidates(projectId, projectType, targets)`, which returns `{id, name, icon_path, installed, compatible}` per instance (`content-install.ts:594-610`, `545-568`).
**Default tab:** starts on `existing`, and **auto-switches to `new` if no instance is both compatible and not installed** (`content-install.ts:566-568`) — i.e. the modal offers to make one when there is nowhere to put the content.
**Operation:** Click `Install` on a row → `handleInstallToInstance` → `findPreferredVersion` (BR-32) → compatibility re-check → `install_project_with_dependencies` (BR-33) → progress (BR-36).
**Controls:** Tab chips (single-select, never empty); search; hide-unavailable toggle; per-row install.
**Persistence:** None — modal state is transient.
**Rationale & notes:** Two decisions carry this modal. Putting incompatible instances *behind a toggle* rather than hiding them removes the "where did my instance go?" confusion while still allowing a deliberate override. And deriving compatibility from the actual version matrix server-side means the answer is correct even for exotic loader/version combinations the client cannot model.

---

## BR-31 — Install flow — create-instance sub-tab
**Type:** dialog
**Nav path:** same modal, `New instance` tab
**Evidence:** [SOURCE `packages/ui/src/layouts/shared/content-tab/components/modals/ContentInstallModal.vue:149-243`]; [SOURCE `…/ContentInstallModal.vue:526-546`]; [SOURCE `apps/app-frontend/src/providers/content-install.ts:757-818`]
**Purpose:** Creating an instance *and* installing into it in one step, so the user never loses the thing they were installing.

Fields, in order:
1. **Icon** preview with a hover/focus `X` to clear, and three buttons: `Upload` (file picker), `Randomize` (generates an icon; the button shows a spinner while generating), `Customize` (opens the icon editor).
2. **Name** — free text, placeholder `Enter instance name`, default `New instance ({n})` where n = current instance count + 1.
3. **Loader** — a `Chips` single-select group built from the project's `compatibleLoaders`, ordered `vanilla, fabric, quilt, neoforge, forge` then alphabetically for anything else. Default = the caller's `preferredLoader` (the context instance's loader when there is one) else the first compatible loader. `datapack` and `minecraft` loaders are normalised to `vanilla`.
4. **Game version** — searchable `Combobox`, default = `preferredGameVersion` (the context instance's version) else the first release version the project supports. A dropdown footer toggle flips `Show all versions` ⇄ `Hide snapshots`; if the preferred version is a snapshot, snapshots are auto-shown.

The footer has `Cancel` and `Install` (disabled until a name is present).
**Operation:** `Create and install` → `install_create_instance({name, gameVersion, loader, loaderVersion: 'latest', iconPath})` → wait for the install job → `install_project_with_dependencies` on the new instance → navigate to `/instance/<newId>` → close.
**Controls:** As listed. No loader-*version* field here — new instances created from an install always take the newest loader build (`loaderVersion: 'latest'`); that choice is available in the standalone create-instance flow instead (LIB-29).
**Persistence:** None.
**Rationale & notes:** Prefilling from the calling context is the load-bearing detail: browsing from a 1.21.4 Fabric instance and clicking "New instance" produces a 1.21.4 Fabric instance, not a blank slate. That is one less decision at exactly the moment the user has the least attention to spare.

---

## BR-32 — Install flow — version / loader resolution
**Type:** indicator
**Nav path:** n/a
**Evidence:** [SOURCE `packages/ui/src/layouts/shared/browse-tab/composables/install-logic.ts:549-556`]; [SOURCE `…/install-logic.ts:564-596`]; [SOURCE `…/install-logic.ts:797-839`]; [SOURCE `apps/app-frontend/src/pages/Browse.vue:837-862`]; [SOURCE `apps/app-frontend/src/providers/content-install.ts:88-115`]
**Purpose:** Turning "this project + these filters + this instance" into one concrete version ID. Two layers:

**Client layer** (`install-logic.ts`) — used when installing *from Browse*, where the user's active filters express intent:
- Preferences are derived from the effective filters: game versions from `game_version`, loaders from whichever loader facet matches the content type (`getLoaderFilterTypes`). Negative filters are dropped because they are browse-only.
- Candidate order: (1) `merge(selected, target)` — the user's filters win per axis, and any axis they left unconstrained is filled from the instance/server; (2) if that resolves nothing and it differs from the target, the **target alone**.
- `getLatestMatchingInstallVersion` then sorts by `date_published` descending and takes the first version whose `game_versions` and `loaders` match. Loader matching is alias-aware: `paper|purpur|spigot|bukkit` are one family, `neoforge|neo` are one family.
- The resolution source is recorded as `'filtered'` or `'target'` on the plan.
- On total failure the error is `No compatible version found for {versions} / {loaders}.` (or `… for {versions}.` for datapacks).

**Rust layer** (`modrinth_content_management`) — the authoritative resolver used by `install_project_with_dependencies`. `target_preferences()` maps the instance's applied content set to a hard constraint, remapping the loader per content type: datapack → `datapack`, resourcepack → `minecraft`, shader → `iris`, else the instance loader (`apply_content_install.rs:144-160`). `select_newest_matching_version` sorts descending by publish date and tries `selected.merge(target)` first, then `target` alone (`install/mod.rs:240-258`). Matching also accepts a `datapack`-loader version for a mod content type (`install/mod.rs:323-325`).

**Notably absent:** there is no UI in which the user picks *which* version to install when browsing. If filters resolve, the newest matching version is taken silently. The only way to choose is to go to the Versions tab and click the specific row (BR-27).
**Operation:** `get_project` + `get_version_many` (cache, `must_revalidate`) on the client; `CachedEntry::get_project_versions` with `CacheBehaviour::MustRevalidate` in Rust.
**Controls:** None surfaced. The only failure path is an error toast.
**Persistence:** None.
**Rationale & notes:** The two-layer design (client picks, Rust verifies) is deliberate: the client can honour the *browse filters* the user can see and change, while the Rust resolver guarantees the version actually matches the instance. The fallback to target-only means a filter that is too narrow degrades to "the newest version that actually works" rather than refusing outright.

---

## BR-33 — Install flow — required and optional dependency resolution
**Type:** indicator
**Nav path:** n/a
**Evidence:** [SOURCE `packages/modrinth-content-management/src/install/mod.rs:17-38`]; [SOURCE `…/install/mod.rs:111-194`]; [SOURCE `…/install/mod.rs:196-237`]; [SOURCE `…/install/model.rs:41-88`]; [SOURCE `packages/app-lib/src/api/instance/projects.rs:80-158`]; [SOURCE `packages/app-lib/src/state/instances/commands/apply_content_install.rs:205-232`]; [SOURCE `apps/app-frontend/src/providers/content-install.ts:336-381`]
**Purpose:** This is the headline finding of the module, and it is a **negative** one:

> **At v0.21.9 there is no user-facing dependency choice during install. Optional dependencies are never installed, and there is no prompt to decline them.**

The mechanics, from `resolve_content`:
- Only `DependencyType::Required` is walked. The loop body begins `if !matches!(dependency.dependency_type, DependencyType::Required) { continue; }` (`install/mod.rs:122-128`). `Optional`, `Incompatible` and `Embedded` are all skipped without a record.
- Traversal is an explicit stack, depth-first, with a `visited_versions` set guarding against cycles (`install/mod.rs:115-119`).
- A dependency pinned to a `version_id` is fetched directly; if it 404s it is recorded as `SkippedContent { reason: MissingVersion }` and the walk continues (`install/mod.rs:200-214`).
- A dependency specified by `project_id` only gets `select_newest_matching_version` under the same merged/target preference logic. If nothing matches it is recorded as `NoCompatibleVersion` and the walk continues (`install/mod.rs:216-236`).
- A dependency whose project is **already installed in this instance** is recorded as `AlreadyInstalled` and skipped (`install/mod.rs:152-160`) — the instance's already-present version wins.
- A dependency whose project is **already planned in this install**, at the same version (or with no pinned primary), is `DuplicateProject`; at a **different** version it is `ConflictingDependency` — and the *first* plan wins (`install/mod.rs:162-180`). So a version conflict resolves to "whichever was resolved first", silently.
- **Special case:** project `P7dR8mSH` (the Fabric API) required by a mod on a **Quilt** target is skipped as `QuiltFabricApi` — because Quilt ships its own Fabric-API-compatible layer (`install/mod.rs:15`, `347-357`).

The returned `ResolveContentPlan { primary, dependencies, skipped }` is used two ways:
- **Plan phase** (`projects.rs:80-158`): resolves synchronously, returns the plan to the UI, then **spawns** the actual install. This is why the UI can populate its "installing" list instantly from the plan instead of discovering dependencies one download at a time.
- **Install phase** (`apply_content_install.rs:205-232`): the primary is added with `DownloadReason::Standalone` and every dependency with `DownloadReason::Dependency` and its `dependent_on_version_id` set, so the dependency relationship survives into the instance's content store and can power the deletion warning (see LIB-adjacent module on the content tab).

**What the user sees:** `addInstallingItemsForPlan` fetches projects and versions for the primary plus every dependency and inserts a placeholder row per item into the instance's content list, each with the project icon, owner (organisation or team owner) and version number; the placeholders are removed on the `content_install_finished` / `content_install_failed` app events (`content-install.ts:217-405`). So dependencies **appear** — but as an already-decided list, not a question. The callback returns `[primary, ...dependencies]` as `installedProjectIds`, which is what flips every card to `Installed` and feeds the "hide installed" set.

**So, precisely: how does the user choose among dependencies?** They don't, for required dependencies — every required dependency is installed or the walk is aborted, and there is no cancel-and-pick-again. And for optional dependencies there is nothing to choose: they are dropped at the first `continue`. If a user wants an optional dependency, they install it themselves as a separate top-level install, at which point the resolver sees it as `AlreadyInstalled` for anything that wanted it.
**Controls:** None. No picker, no checkbox list, no "skip optional" toggle, no per-dependency version chooser.
**Persistence:** None beyond the resulting instance content.
**Rationale & notes:** This is a meaningful simplification relative to the desktop launchers Modrinth is displacing (CurseForge/ATLauncher show a dependency tree). The justification is visible in the code: the resolver cannot safely offer a choice, because `ConflictingDependency` is resolved by first-wins with no conflict resolution UI. Presenting a choice the engine cannot honour would be worse than not presenting one. The cost is real, though — a mod whose *optional* dependency is actually required in practice will launch broken, and the user gets no signal at install time. If we implement a chooser, we must also implement conflict resolution; those two are one feature, not two.

---

## BR-34 — Install flow — incompatibility warning
**Type:** dialog
**Nav path:** triggered when the chosen version does not match the instance
**Evidence:** [SOURCE `apps/app-frontend/src/providers/content-install.ts:69-73`]; [SOURCE `…/content-install.ts:685-755`]; [SOURCE `apps/app-frontend/src/locales/en-US/index.json` → `app.content-install.no-compatible-versions`]
**Purpose:** The escape hatch for "no version of this project matches your instance". Instead of refusing, the app offers the full version list and lets the user install an incompatible build deliberately.

The modal shows the project icon and name, the target instance's game version and loader, and a message built from the locale string: **`No available versions match {compatibilityLabel}. Select a version to install anyway. Dependencies will not be installed automatically.`** — where `compatibilityLabel` is `{loader} {gameVersion}` for mods/shaders/plugins and just `{gameVersion}` for resource packs and datapacks.

The **Dependencies will not be installed automatically** clause is the key detail: the override path calls `add_project_from_version(instanceId, versionId, 'standalone')` with **no dependency resolution at all**, precisely because the dependency resolver would also fail to find a compatible version.
**Operation:** Pick a version → `add_project_from_version` → success → the callback marks the project installed and the modal closes; failure → `handleError`, the placeholder row is removed, the instance is marked content-install-failed.
**Controls:** A version list; cancel.
**Persistence:** None.
**Rationale & notes:** Stating the dependency consequence inline is exactly right — the user is overriding a safety check, and telling them what they lose is the difference between a deliberate override and a silent corruption. This is the only place in the install flow where the app is explicit about degraded behaviour.

---

## BR-35 — Install flow — modpack install and the duplicate warning
**Type:** dialog
**Nav path:** triggered when a modpack is already installed in some instance
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/modal/ModpackAlreadyInstalledModal.vue:1-80`]; [SOURCE `apps/app-frontend/src/providers/content-install.ts:845-889`]; [SOURCE `…/content-install.ts:983-1009`]; [SOURCE `apps/app-frontend/src/providers/setup/creation-modal.ts:88-108`]
**Purpose:** Installing a modpack **creates a new instance** — there is no "add modpack to existing instance" from Browse; that path exists on the instance side. Before creating it, the app checks whether an instance is already linked to the same `project_id`. If so, and `skipNonEssentialWarnings` is off, a warning modal appears:

- Header: `Modpack already installed`
- Body: `This modpack is already installed in the **{instanceName}** instance. Are you sure you want to duplicate it?`
- Actions: `Cancel` | `Instance` (navigate to the existing one) | `Create` (orange).

Three ways out, and the middle one is the valuable one: jumping to the already-installed copy instead of making a second one. The whole check is suppressed by the `skipNonEssentialWarnings` setting.
**Operation:** `install_create_modpack_instance({type: 'fromVersionId', project_id, version_id, title, icon_url})`. Version selection before that: an explicit `versionId` wins; otherwise, if loader/game-version hints were supplied, `getLatestMatchingInstallVersion`; otherwise `project.versions[project.versions.length - 1]`.
**Controls:** Cancel / Instance / Create.
**Persistence:** The resulting instance's `link.project_id` + `link.version_id`, which is what later powers "update modpack" and the "hide already installed" toggle.
**Rationale & notes:** Making the duplicate detection part of install (rather than only part of creation) means the same guard covers both entry points — the `CreationFlowModal` path and the Browse path share the identical modal.

---

## BR-36 — Install flow — progress feedback
**Type:** indicator
**Nav path:** n/a
**Evidence:** [SOURCE `apps/app-frontend/src/providers/content-install.ts:217-296`]; [SOURCE `…/content-install.ts:298-327`]; [SOURCE `…/content-install.ts:395-405`]; [SOURCE `apps/app-frontend/src/providers/content-install.ts:654-683`]
**Purpose:** Because dependency resolution returns a complete plan up front (BR-33), the progress UI can be **complete from the first frame**: `addInstallingItemsForPlan` inserts one placeholder row per planned project before any download starts, so the user sees the whole batch — including dependencies they did not choose — appear at once in the target instance's content list. Each placeholder is then enriched asynchronously with the owner (organisation fetch, or team fetch to find `is_owner`), so the rows resolve from a bare ID into icon + owner + version.

Removal is event-driven: `instance` events `content_install_finished` / `content_install_failed` (emitted from Rust, `projects.rs:116-153`) strip exactly the planned project IDs from the placeholder map, and bump two revision counters (`installRevisionByInstance`, `installFailureRevisionByInstance`) so the content list re-queries. Failures additionally surface the Rust error message through `handleError`.
**Operation:** Install buttons flip to a spinner + `Installing`; instance content rows appear with placeholders; a background job bar (module: hosting/download-manager) reports bytes.
**Controls:** None.
**Persistence:** None — placeholder state is in-memory `Ref<Map<string, ContentItem[]>>`.
**Rationale & notes:** The placeholder IDs are synthetic (`__installing_{projectId}`) so they cannot collide with real installed files, and the map is keyed by instance so two concurrent installs into different instances do not interleave. Showing dependencies *before* they land is what converts an opaque wait into a reviewable list.

---

## BR-37 — Update flow — Switch version (reachable from Browse)
**Type:** button
**Nav path:** `/project/<id>` → `/project/<id>/versions` → row action
**Evidence:** [SOURCE `apps/app-frontend/src/pages/project/Versions.vue:11-35`]; [SOURCE `apps/app-frontend/src/pages/project/Version.vue:14-38`]; [SOURCE `apps/app-frontend/src/providers/content-install.ts:889-952`]; [SOURCE `packages/app-lib/src/api/instance/projects.rs:172-204`]
**Purpose:** The only "update" reachable from Discover is switching a *project* to a different version. Modpack updates, bulk content updates and `update_project` are **not** reachable from Browse — they live on the instance's content tab (`/instance/<id>`, `ContentUpdaterModal`, `ConfirmBulkUpdateModal`, `ConfirmModpackUpdateModal`).

The browse-reachable flow:
1. Project header shows `Switch version` when the project is already installed in the context instance; clicking routes to the Versions tab, whose link carries `?g=<loader>&v=<gameVersion>` pre-filters the table.
2. Each row's action is `Switch to version` (swap icon) for any row that is not the installed one.
3. `install(version.id)` runs: because an `instanceId` is present and the version is compatible, the provider **removes the existing installed copy** (`remove_project` for every path whose `metadata.project_id` matches) and then installs the new version via `install_project_with_dependencies` — a full dependency re-resolution.

The dedicated `switch_project_version_with_dependencies` command exists (`projects.rs:172-204`) and is the better-behaved path — it replaces in place, reuses the old path when the new filename is unchanged, renames shader `.txt` companion files, and carries over the frozen/locked state. It is wired from the **instance content tab**, not from Browse. Both paths carry the shared-instance-modification and frozen-content guards, which raise e.g. `Shared instance managed content cannot be changed directly.` and `Frozen content cannot change versions. Unfreeze it first.`
**Operation:** As above; `project_background` completion event flips the header to `Installed`.
**Controls:** Install / Switch to version / Already installed per row.
**Persistence:** The instance's installed version ID, which is what the Versions tab compares against to render `Already installed`.
**Rationale & notes:** The asymmetry is worth flagging for our implementation: **the Browse-driven switch is remove-then-install, while the in-app switch is a true in-place switch.** They can differ in file naming, config preservation and dependency resolution. If we expose both, they must share one command, or the two entry points will drift.

---

## BR-38 — Server-context browse and the install selection queue
**Type:** indicator
**Nav path:** `/browse/<type>?sid=<serverId>&wid=<worldId>`
**Evidence:** [SOURCE `apps/app-frontend/src/pages/Browse.vue:461-505`]; [SOURCE `…/Browse.vue:905-992`]; [SOURCE `packages/ui/src/layouts/shared/browse-tab/composables/install-logic.ts:604-622`]; [SOURCE `…/install-logic.ts:712-758`]; [SOURCE `…/install-logic.ts:669-710`]
**Purpose:** When Browse is entered from a Modrinth Hosting server (`?sid=`), the install model changes from "install now" to **build a plan, then apply it**. Card actions become `Selected` (green check, click again to deselect) instead of `Install`, and a floating bar appears: `{count} selected` with `Install {count} projects}`. Leaving the page with selections pending prompts via `SelectedProjectsLeaveModal`.

The queue is **durable**: plans are serialised to `localStorage` under `server-install-queue:{serverId}:{worldId}`, with a token-based lock (`server-install-queue-lock:*`, 15-minute TTL, refreshed every 30 s) and an in-memory flush registry so two app views cannot flush the same queue twice. On flush, every plan's dependencies are re-resolved against the server's existing content (`resolveServerAddonInstallPlans`), duplicates are dropped, and only genuinely-new `{project_id, version_id}` pairs are sent to the server (`install-logic.ts:641-710`).

Server-context provided filters are injected the same way as instance-context: `game_version` from `mc_version`, `mod_loader`/`plugin_loader` from the server's loader (including `paper`/`purpur`), `environment: server` for mods, and both `environment: client` **and** `server` for modpacks.
Runtime filters (`game_version`, `mod_loader`, `plugin_loader`, `plugin_platform`, `datapack_loader`) are stripped before the plan is built, so a user browsing at one game version cannot pin the plan to it.
**Controls:** Per-card select/deselect; `Install {count}` / `Discard` in the floating bar; `Hide installed` / `Hide selected` toggles.
**Persistence:** `localStorage['server-install-queue:{serverId}:{worldId}']`.
**Rationale & notes:** The queue-with-lock pattern is the most sophisticated piece of state management in the whole Browse surface — it exists because a hosting-server plan can be built over minutes and the page can be reloaded, and a naive implementation would double-apply. It is also the only place in the app that uses `navigator.locks` when available with a localStorage-lock fallback.

---

## BR-39 — Server browse facets
**Type:** toggle
**Nav path:** `/browse/server?sct=modpack&sgv=1.21&sr=europe&sl=en&sst=online&sc=<category>`
**Evidence:** [SOURCE `packages/ui/src/utils/server-search.ts:98-119`]; [SOURCE `…/server-search.ts:147-355`]; [SOURCE `packages/ui/src/layouts/shared/browse-tab/sidebar.vue:113-120`]
**Purpose:** The `/browse/server` tab is a completely separate facet set from the project types, driven by `useServerSearch` rather than `useSearch`.

**Sort options** (`SERVER_SORT_TYPES`), in UI order: `Relevance`, `Verified Plays`, `Players`, `Followers`, `Date Published`, `Date Updated`.

**Facets:**
| Facet | Label | Param | Notes |
| --- | --- | --- | --- |
| `server_content_type` | `Type` | `sct` | `Vanilla` / `Modded` (maps to content kind) |
| `server_category_*` | category headers (`Features`, `Gameplay`, `Meta`, `Community`) | `sc` | include + exclude |
| `server_game_version` | `Game version` | `sgv` | scrollable + searchable |
| `server_region` | `Region` | `sr` | 9 regions, sorted by localised name |
| `server_language` | `Language` | `sl` | scrollable + searchable, English first, then the user's locale, then alphabetical |
| `server_status` | `Status` | `sst` | `Online` / `Offline` — **pre-selected to `online`** on mount |
| `advanced` | `Advanced exclusions` | `a` | disclosures compatible with servers |

Server facets compile to field queries (`minecraft_java_server.ping.data EXISTS` for online) with `project_types = minecraft_java_server` pinned. The `Type`, `Game version`, `Language` and `Status` groups default **closed** in the app; the two category groups default open (`sidebar.vue:113-120`).
**Controls:** Multi-select per group; include-only except the category and region groups.
**Persistence:** URL params above.
**Rationale & notes:** Pre-selecting `Status: Online` is a strong default — a user browsing servers wants one they can join, and an offline server is almost never the intent. The eight-locale-aware language ordering is a small but real detail that makes the list scannable.

---

## BR-40 — Empty / no-results / offline / error states
**Type:** indicator
**Nav path:** n/a
**Evidence:** [SOURCE `packages/ui/src/layouts/shared/browse-tab/layout.vue:259-274`]; [SOURCE `packages/ui/src/locales/en-US/index.json` → `browse.offline`, `browse.no-results`, `browse.filter-results`]; [SOURCE `apps/app-frontend/src/pages/project/Index.vue:205`]; [SOURCE `apps/app-frontend/src/pages/Browse.vue:251-262`]
**Purpose:** Four mutually exclusive states in the results region, in priority order:

1. **Loading** — `LoadingIndicator`, shown while `ctx.loading` is true. Critically, `loading` is only set when the current hit list is *empty* (`use-browse-search.ts:336-341`), so it never replaces a visible result set with a spinner.
2. **Offline** — shown when `ctx.offline` is true **and** `totalHits === 0`: **`You are currently offline. Connect to the internet to browse Modrinth!`** A plain-text line, not a modal or a blocking screen. The condition matters: if results are already on screen, dropping the network does not erase them.
3. **No results** — shown when the hit list is empty: **`No results found for your query!`** Centred, plain text. No "clear filters" affordance in this state.
4. **Project load failure** — the project detail page renders a single line: **`Project data could not be loaded.`**

**Offline detection is `ref(!navigator.onLine)`** with `window.addEventListener('offline'|'online')` (`Browse.vue:530-545`). Per OQ-004 in `open-questions.md`, this is known to be a **false positive inside the Tauri webview** — the titlebar chip reads "Offline" while Browse is demonstrably fetching live data. The practical consequence here is that this branch fires spuriously.
**Operation:** n/a.
**Controls:** The only recovery affordances are elsewhere: `Clear all filters` in the chip strip (BR-18) and the `Filter results...` button, which on narrow layouts opens the filter panel as a full-screen sheet with a `Close` icon button (`sidebar.vue:143-161`).
**Persistence:** n/a.
**Rationale & notes:** Two gaps worth carrying forward as negatives. The empty state gives no recovery action — a user who has over-filtered has to find the clear control themselves. And the offline gate keys off a browser API that is wrong in this runtime, so a correct implementation must derive connectivity from the same layer that performs requests.

---

## BR-41 — Photosensitivity warning modal
**Type:** dialog
**Nav path:** shown when the `Photosensitivity triggers` advanced exclusion is first selected
**Evidence:** [SOURCE `packages/ui/src/layouts/shared/browse-tab/sidebar.vue:16`]; [SOURCE `…/sidebar.vue:41-67`]; [SOURCE `packages/ui/src/locales/en-US/index.json` → `search.photosensitivity-warning-modal.*`]
**Purpose:** A one-time safety notice raised the first time a user excludes photosensitivity-triggering content: title `Results are not guaranteed to be safe`, body split across three messages — `We cannot guarantee that all content on Modrinth has been labeled appropriately.` / `Content labels for photosensitivity triggers are self-assigned by the creators who upload their content to Modrinth. These projects have not gone through any safety testing.` / `Using any content on Modrinth is at your own risk. Please stay safe! 💚` — with a `Don't show this again` checkbox.

The trigger logic is precise: it fires only when the option transitions from unselected → selected, it was not already saved, and it has not been dismissed (`sidebar.vue:41-61`). The dialog reference is obtained via `useTemplateRef`, so it is a component instance rather than a route.
**Operation:** Click to acknowledge; optionally check `Don't show this again` → `dismissedPhotosensitivityFilterWarning = true` and the Rust setting `dismissed_photosensitivity_filter_warning` is written.
**Controls:** Dismiss; `Don't show this again` checkbox.
**Persistence:** Rust settings (`packages/app-lib/src/state/settings.rs:44`, `161-162`), global.
**Rationale & notes:** The notice exists because excluding a self-declared label cannot actually protect anyone — the app is being explicit that it is not guaranteeing safety. That is a notably honest framing and is worth mirroring. `[ORCH-PENDING]` — the exact visual treatment of `PhotosensitivityWarningModal` was not read directly; the copy and trigger conditions are from source and the locale file.

---

## BR-42 — Context menu on a project card
**Type:** menu
**Nav path:** right-click a card in Browse or the project header
**Evidence:** [SOURCE `packages/ui/src/layouts/shared/browse-tab/layout.vue:300`]; [SOURCE `apps/app-frontend/src/pages/Index.vue:923-949`]; [SOURCE `apps/app-frontend/src/pages/Browse.vue:1336-1340`]
**Purpose:** A shared `ContextMenu` labelled `Project actions`, opened on `contextmenu.prevent.stop` from every card. In the **project detail page** the full item list is visible in source: `Install` (download icon), a divider, `Open in Modrinth` (globe icon; the slot renders `GlobeIcon … ExternalIcon`), and `Copy link` (clipboard icon → `navigator.clipboard.writeText('https://modrinth.com/<type>/<slug>')`).
In **Browse**, the same `ContextMenu` ref is handed to `useAppServerBrowse({ … handleRightClick })`; the server-type item list lives in `apps/app-frontend/src/composables/browse/use-app-server-browse.ts`, which was not read line-by-line for this module.
**Operation:** Right-click a card or the project header.
**Controls:** Install / Open in Modrinth / Copy link (project page, confirmed); server-type list `[ORCH-PENDING]`.
**Persistence:** None.
**Rationale & notes:** `Copy link` always copies the **modrinth.com** URL, never the in-app route — the correct choice, since the recipient will not have the app. `[ORCH-PENDING]` — the server-browse variant of this menu.

---

## Cross-cutting notes for implementers

1. **Browse is one route with one param, not six routes.** Everything else (filters, sort, page, per-page, query) rides in the URL query string and is written back with `router.replace` after every search completes (`updateUrlParams`, `use-browse-search.ts:381-406`). Preserve persistent context params (`i`, `ai`, `shi`, `sid`, `wid`, `from`) across that rewrite or the install context silently evaporates.
2. **Filter semantics are data, not code.** `FilterType` carries `query_param`, `supports: ['include'|'exclude']`, `display`, `ordering`, `default_values`, `searchable` and `method` per option. The sidebar, the chip strip, the URL codec and the install-preference extractor all read the same descriptors. A hard-coded filter panel will not reproduce this behaviour.
3. **Include vs exclude is per-facet, not global.** Categories, loaders and licence are include+exclude; game version, environment and the dependency facet are include-only; advanced exclusions are exclude-only. Each facet's row component picks its primary action from `supports` and only renders the ban button when both are supported.
4. **`method: 'and'` vs `'or'` is per-option, not per-facet.** Categories AND together within a header; game versions OR together; resolutions OR. Getting this wrong silently changes result counts.
5. **There is no infinite scroll and no density switch in the app**, despite the shared layout supporting both.
6. **Card install is one click deep at most**: instance already in context → install immediately with filters; no context → "Add to an instance" → picker. Modpacks always create an instance.
7. **Optional dependencies are never installed and never offered.** This is the single most important behavioural difference to decide about before implementing a chooser (BR-33).