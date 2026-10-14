# Module 04 — Library / instance list

Reverse-engineering notes for the Modrinth App (Theseus) **v0.21.9**, tag `v0.21.9`,
commit `6ebd75b68dd386cd4d74eee1cf2e96d945c937db`.

Scope: the Home tab's instance list — the surface itself, grouping/sorting/filtering,
the play control, every context-menu action, pinning, duplication, deletion, editing,
export, folder access, the import-from-other-launchers flow, the add-instance dialog,
the empty-library and first-run states, and offline behaviour.

Everything is derived from the pinned source. Rows marked `[ORCH-PENDING]` need a
visual capture to confirm; everything else is `[SOURCE …]`.

---

## Coverage table

| ID | label | type | evidence | status |
| --- | --- | --- | --- | --- |
| LIB-1 | The library surface (Home tab) | indicator | `apps/app-frontend/src/pages/Index.vue:91-105` | confirmed |
| LIB-2 | Instance card — what it shows | indicator | `apps/app-frontend/src/components/ui/library/instance-group/instance-card-view.vue:39-93` | confirmed |
| LIB-3 | Play / Stop / Repair button and its states | button | `apps/app-frontend/src/components/ui/library/instance-group/instance-card.vue:306-361` | confirmed |
| LIB-4 | Loading / installing overlay | indicator | `…/instance-card.vue:292-305` | confirmed |
| LIB-5 | Selection checkbox, shift-range select, multi-select | toggle | `…/instance-card.vue:363-388`, `…/library/index.vue:190-236` | confirmed |
| LIB-6 | Instance context menu — complete list | menu | `apps/app-frontend/src/components/ui/library/use-library.ts:1264-1379` | confirmed |
| LIB-7 | Group context menu | menu | `…/library/instance-group/index.vue:326-360` | confirmed |
| LIB-8 | Group header hover actions | button | `…/library/instance-group/group-action-buttons.vue:51-100` | confirmed |
| LIB-9 | Drag & drop between groups | button | `apps/app-frontend/src/components/ui/library/use-library.ts:590-783` | confirmed |
| LIB-10 | Drag-gather and Alt-drag "duplicate into group" | toggle | `…/library/instance-group/instance-group-dnd.vue:1-69`, `use-library.ts:685-704` | confirmed |
| LIB-11 | Create group | dialog | `…/library/library-toolbar/new-group-modal.vue:1-90` | confirmed |
| LIB-12 | Rename group (inline) | field | `…/library/instance-group/index.vue:394-422` | confirmed |
| LIB-13 | Delete group + confirmation | dialog | `…/library/instance-group/index.vue:304-324`, `:670-692` | confirmed |
| LIB-14 | "Add instances to group" modal | dialog | `apps/app-frontend/src/components/ui/library/group-instances-modal.vue:1-90` | confirmed |
| LIB-15 | Group reordering (drag + arrows) | button | `…/library/index.vue:98-178`, `use-library.ts:1107-1175` | confirmed |
| LIB-16 | Grouping modes | button | `use-library.ts:69-75`, `429-566` | confirmed |
| LIB-17 | Sorting options | button | `use-library.ts:59-67`, `434-467` | confirmed |
| LIB-18 | Filtering | toggle | `…/library/library-toolbar/filter-menu.vue:28-56` | confirmed |
| LIB-19 | Library search | field | `…/library/library-toolbar/index.vue:32-39`, `view-state.ts:3` | confirmed |
| LIB-20 | Multi-select floating action bar | indicator | `…/library/LibrarySelectionActionBar.vue:1-59` | confirmed |
| LIB-21 | Deletion confirmation | dialog | `apps/app-frontend/src/components/ui/modal/ConfirmDeleteInstanceModal.vue:1-186` | confirmed |
| LIB-22 | Duplication | button | `use-library.ts:1184-1186`, `apps/app-frontend/src/helpers/install.ts:242-246` | confirmed |
| LIB-23 | Export modpack — **not** in the library | dialog | `apps/app-frontend/src/pages/instance/layout.vue:7`, `:52` | confirmed |
| LIB-24 | Open folder / Copy path | button | `use-library.ts:1336-1347` | confirmed |
| LIB-25 | Icon editing submenu | menu | `use-library.ts:1234-1262` | confirmed |
| LIB-26 | "Add content" shortcut | button | `use-library.ts:1314-1320`, `…/instance-card.vue:231-237` | confirmed |
| LIB-27 | Pinning (Favorites) | toggle | `use-library.ts:1297-1312`, `apps/app-frontend/src/helpers/instance-groups.ts:3` | confirmed |
| LIB-28 | Import from another launcher — supported list and conversion | dialog | `packages/app-lib/src/api/pack/import/mod.rs:35-43`, `apps/app-frontend/src/providers/setup/instance-import.ts:15` | confirmed |
| LIB-29 | Add-instance dialog (creation flow) | dialog | `packages/ui/src/components/flows/creation-flow-modal/components/SetupTypeStage.vue:51-107` | confirmed |
| LIB-30 | Empty library and first-run call to action | dialog | `apps/app-frontend/src/components/ui/WelcomeScreen.vue:1-138` | confirmed |
| LIB-31 | Offline behaviour of the library | indicator | `apps/app-frontend/src/pages/Index.vue:56-62`, `App.vue:465-471` | confirmed |
| LIB-32 | Quick-instance switcher (left rail) | indicator | `apps/app-frontend/src/components/ui/QuickInstanceSwitcher.vue:1-330` | confirmed |
| LIB-33 | Library page background context menu | menu | `apps/app-frontend/src/pages/Index.vue:70-88` | confirmed |
| LIB-34 | Compact card mode setting | toggle | `apps/app-frontend/src/components/ui/library/instance-group/instance-card-view.vue:24-33` | confirmed |
| LIB-35 | Virtualised grid + scroll restoration | indicator | `…/library/instance-group/index.vue:143-162`, `…/library/index.vue:62-75` | confirmed |

---

## LIB-1 — The library surface (Home tab)
**Type:** indicator
**Nav path:** `/` (route name `Home`)
**Evidence:** [SOURCE `apps/app-frontend/src/routes.js:8-12`]; [SOURCE `apps/app-frontend/src/pages/Index.vue:91-105`]; [SOURCE `apps/app-frontend/src/components/ui/library/index.vue:265-406`]
**Purpose:** The Home tab is a composite: an optional `RecentWorldsList` strip (only when `showJumpIn` is on and there is at least one instance), then the library, then a background context menu. The library is one `LibrarySection` fed an `instances` array from a single vue-query (`instanceListQueryOptions`). The page renders either the `WelcomeScreen` (if the app has never created an instance) or the library.
The library body is a vertical stack of **collapsible group sections** (`InstanceGroup`), each with a header row (disclosure chevron, inline-editable name or the fixed label, a count `TagItem`, and hover-revealed action buttons) and an absolutely-positioned card grid inside an `Accordion`.
The whole section is a drop target for drag-and-drop (`InstanceGroupDnd` wraps it) and carries `data-library-page-background`, which is how LIB-33 knows a right-click landed on the page background rather than a card.
**Operation:** Instance list load is **suspense-gated on `hasCreatedInstance`** — on a genuinely fresh profile the query is not awaited at all (`Index.vue:58-62`), which is what makes first launch show the welcome screen rather than an empty library.
**Controls:** Section-level collapse, group reorder handle, group actions, per-card actions, selection.
**Persistence:** Search text and scroll position live in a module-level reactive singleton (`view-state.ts`), not in the URL; filters, sort, grouping, collapsed sections and the ungrouped-group position live in `localStorage`.
**Rationale & notes:** The library is the app's home, so it carries no chrome of its own — the only heading is `Library`. Because the grid is absolutely positioned and windowed (LIB-35), sections are stable scroll anchors rather than reflowing as cards mount.

---

## LIB-2 — Instance card — what it shows
**Type:** indicator
**Nav path:** `/instance/<id>` (whole card navigates)
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/library/instance-group/instance-card-view.vue:39-93`]; [SOURCE `…/instance-card.vue:263-291`]; [SOURCE `apps/app-frontend/src/components/ui/library/instance-group/index.vue:123-138`]
**Purpose:** A card, not a row. It shows exactly four things:

| Element | Data | Decision it supports |
| --- | --- | --- |
| Icon (avatar) | `instance.icon_path`, tinted by instance ID | *Which one is this?* Recognition again. Local raster files are thumbnailed client-side (96 px compact / 384 px normal, cache-busted by `instance.modified`); SVG and remote icons go through `getInstanceIconUrl`. |
| Name | `instance.name`, truncated with a truncated-tooltip | *Is this the one?* |
| Subtitle | `{loader} {gameVersion}`, capitalised | *Is this the right environment?* Loader + version is the whole compatibility contract for an instance, and it is what makes a duplicate obvious. |
| Hover overlay button | Play / Stop / Repair | see LIB-03 |

The selection checkbox (LIB-05) overlays the top-right corner and is hidden until card hover or selection.

Two layouts, driven by the `compactInstanceCards` setting (LIB-34): normal is a tall square card with the icon filling the width and the text below; compact is a 40 px row-ish card with a 40 px icon and the text beside it. The grid column count is computed from the container width — `minWidth = 15rem` compact, `8rem` when the window is under 1280 px, `10rem` otherwise.
**Operation:** Click navigates to `/instance/<id>`; Enter does the same; Space toggles selection (only while a selection is active). `mouseenter` re-checks running processes so the play button's state is never stale on arrival.
**Controls:** As above.
**Persistence:** None on the card itself.
**Rationale & notes:** The card deliberately shows **no** play-time, last-played, update-available badge or server-player count. `GameInstance` carries `last_played`, `recent_time_played`, `submitted_time_played`, `install_stage`, `quarantined` and `link`, and the library surfaces none of them — the sort menu uses them, the card does not. That is a real restraint: the card answers "what is this?" and "can I start it?", nothing else.

---

## LIB-3 — Play / Stop / Repair button and its states
**Type:** button
**Nav path:** n/a (starts/stops the game in place)
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/library/instance-group/instance-card.vue:306-361`]; [SOURCE `…/instance-card.vue:172-237`]; [SOURCE `apps/app-frontend/src/locales/en-US/index.json` → `app.library.instance.play`, `.stop`, `.repair`, `.loading`, `.installing`]
**Purpose:** One circular icon button overlaid on the icon, bottom-right in normal mode / centred in compact mode. **It is hidden at rest** (`opacity-0`, `scale-75`) and revealed on card hover or focus-within, so a grid of twenty idle instances is visually quiet. Four states, in this priority order:

| State | Icon | Colour | Tooltip / aria-label | Visible when |
| --- | --- | --- | --- | --- |
| **Stop** | stop-circle | red, `colored` | `Stop` | `playing === true` |
| **Repair** | download | brand, `colored` | `Repair` | not loading, not installing, no active selection, `install_stage !== 'installed'`, not quarantined |
| **Play** | play | brand, `colored` | `Play` | not loading, not installing, no active selection, not quarantined |
| *(none)* | — | — | — | loading / installing / selection active / quarantined |

So the button is a **two-state tri-state**: it becomes *Repair* when the instance's `install_stage` is anything other than `installed`, which is how a freshly-imported, broken, or partially-installed instance surfaces a recovery action without a separate UI.

**Operations:**
- **Play** → `run(instanceId)`; errors go through `handleSevereError` (a fatal-crash reporter), not the ordinary toast. Analytics `InstanceStart` carries `loader`, `game_version` and a `source` discriminator (`InstanceCard` / `InstanceGridContextMenu` / `QuickInstanceSwitcher`) so every play entry point is attributable.
- **Stop** → `kill(instanceId)`; sets `playing = false` optimistically *before* the call.
- **Repair** → branches: if `install_stage !== 'pack_installed'` **and** the instance is linked to a Modrinth or server modpack, `install_pack_to_existing_instance({type: 'fromVersionId', …})`; otherwise `install_existing_instance(instanceId, force = false)` — i.e. a plain re-install of Minecraft + loader without touching mod content.
- **Play is blocked** when `instance.quarantined` is true. There is no UI affordance explaining quarantine on the card; the tooltip is absent and the button simply disappears.
**Controls:** One button; hover to reveal; disabled by absence, not by `disabled`.
**Persistence:** None.
**Rationale & notes:** Three choices stand out. Hiding the control at rest keeps the grid calm. Routing launch failures through a *severe* error handler rather than a toast is correct — a crash on launch is a support case, not a warning. And routing errors through `handleSevereError` while `kill` uses the ordinary `handleError` draws a deliberate line between "the game broke" and "we could not stop it".

---

## LIB-4 — Loading / installing overlay
**Type:** indicator
**Nav path:** n/a
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/library/instance-group/instance-card.vue:292-305`]; [SOURCE `…/instance-card.vue:76-101`]
**Purpose:** A translucent `bg-surface-1 opacity-30` scrim over the icon plus a centred `SpinnerIcon`, shown when the card is busy. Two distinct tooltips share the spinner:
- `Instance is loading...` when `modLoading` — the process is starting, or a `launched` event has fired but `playing` has not yet been confirmed.
- `Installing...` when `instance.install_stage.includes('installing')` — a Rust-side install/update job is running.

There is a deliberate **100 ms delay** (`LOADING_INDICATOR_DELAY_MS = 100`) before the overlay appears, so a fast launch does not flash a spinner. State comes from two sources: `useAppEvent('process', …)` for live transitions and a `get_by_instance_id()` poll on mount and on every `mouseenter`.
**Controls:** None.
**Persistence:** None.
**Rationale & notes:** The `modLoading` computation is subtle and load-bearing: `currentEvent === 'launched' && !playing` treats a launched-but-not-confirmed process as still loading. Without that, the card would show a Play button during the window where Minecraft has actually started but the process list has not yet caught up — the exact window in which a double-click produces two copies of the game.

---

## LIB-5 — Selection checkbox, shift-range select, multi-select
**Type:** toggle
**Nav path:** n/a
**Evidence:** [SOURCE `…/instance-card.vue:363-388`]; [SOURCE `apps/app-frontend/src/components/ui/library/index.vue:190-262`]; [SOURCE `apps/app-frontend/src/components/ui/library/use-library.ts:956-989`]
**Purpose:** Multi-select exists so bulk destructive and bulk organisational operations are possible without a separate "enter selection mode" flag: a checkbox appears in each card's top-right on hover, and once **anything** is selected the whole library switches into selection mode (`isLibraryInstanceSelectionActive`), which suppresses the Play/Repair buttons and turns a plain card click into a toggle.
Interaction model, fully specified:
- Click the checkbox → toggle that instance.
- Shift-click (or Shift + Space) → **range select** from the anchor to the target across the flattened, displayed instance list (groups in display order), replacing the selection. Shift-clicking inside an already-selected range **deselects** the range.
- `Escape` → clears the selection (bound at the window level, `use-library.ts:981-989`).
- Changing the grouping mode clears the selection (`use-library.ts:960`).
- Deactivating the library component (e.g. navigating to another tab) clears it (`onDeactivated`, `library/index.vue:186`).
- Selection is keyed by `(groupId, instanceId)`, not instance ID alone — so an instance present in three groups is three independently selectable cards, and moving it between groups correctly moves the selection with it.
- The anchor card gets a purple outline while Shift is held, as a preview of what a range click will capture.
**Controls:** The checkbox is a real `<button>` with `aria-pressed` and per-instance labels `Select {name}` / `Deselect {name}`; the card itself switches its `aria-label` between `Open {name}` and the select/deselect pair.
**Persistence:** In-memory `Ref<Map<string, …>>` only.
**Rationale & notes:** The `(groupId, instanceId)` key is the non-obvious part and the correct one: in `Custom group` mode a single instance renders once per group it belongs to, so an instance-ID key would make multi-group membership indistinguishable from duplication. Shift-range across a *flattened* list (groups in render order) is what users expect from a file manager, and the grouping-agnostic flattening is why it works without extra machinery.

---

## LIB-6 — Instance context menu — complete list
**Type:** menu
**Nav path:** right-click any instance card
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/library/use-library.ts:1264-1379`]
**Purpose:** Opened via `contextmenu.prevent.stop` on the card, dispatched through a single shared `ContextMenu` instance labelled `Instance actions`. The full item list, in order, with exact availability conditions:

| # | Label | Icon | Shown when | Disabled when | Action |
| --- | --- | --- | --- | --- | --- |
| 1 | `Stop` | stop-circle | `item.playing` | — | `item.stop(null, 'InstanceGridContextMenu')` |
| 2 | `Play` | play | not playing **and** not quarantined | `installing` | `item.play(null, 'InstanceGridContextMenu')` |
| 3 | `Add to favorites` / `Remove from favorites` | star | always (label flips) | — | `edit(instanceId, { group_ids })` toggling `group:favorites` |
| — | *divider* | | always | | |
| 4 | `Add content` | plus | `!quarantined && !link && !installing` | — | router push to `/browse/mod` or `/browse/datapack` with `?i=<id>` |
| — | *divider* | | only when 4 is shown | | |
| 5 | `View instance` | eye | always | `installing` | `/instance/<id>` |
| 6 | `Duplicate instance` | clipboard | always | `install_stage !== 'installed'` | `install_duplicate_instance(id)` |
| 7 | `Open folder` | folder-open | always | — | `showInstanceInFolder(id)` |
| 8 | `Copy path` | clipboard | always | — | `navigator.clipboard.writeText(instance.id)` |
| 9 | `Edit icon` | edit | always | — | **submenu** (see LIB-25) |
| 10 | `Remove from group` | minus | grouping is `Custom group` **and** the card's group is neither `None` nor `Favorites` | — | drop that group ID from `group_ids` |
| — | *divider* | | always | | |
| 11 | `Delete` | trash | always | — | opens the confirmation modal (LIB-21) |

The `remove_from_favorites` item has a custom slot rendering a **filled** star in the default text colour, so the current state of an item is visible in the menu itself rather than only in its label.

Conditional-divider handling means the menu never shows a dangling divider: the divider after item 4 is bound to the same `shown` condition as item 4.

**Things that are conspicuously *absent* from this menu** — and are the most likely source of a "our app is missing something" report:
- **Export** is **not** here. It lives only on the instance page header (LIB-23).
- **Rename** is **not** here. Renaming is inside the instance's Settings → General modal, not the library.
- **Edit instance settings / Java / hooks** are not here — the card click and `View instance` both route to the instance page.
- **Update** (content update, modpack update) is not here — it is on the instance's Content tab.
- **Repair** is not in the menu even though it is on the card button.
- **Create shortcut** is not here; it is an instance-page header action.
- **Stop all / Play all** batch actions do not exist; multi-select offers only *group* and *delete* operations (LIB-20).

**Operation:** The menu needs a live handle on each card, so the parent registers card component instances by ID (`setInstanceComponent`) and passes the card's exposed API (`play`, `stop`, `seeInstance`, `openFolder`, `addContent`, `instance`, `playing`) into the menu builder. That is why the play state in the menu is always accurate even though it is captured at open time.
**Controls:** As tabulated.
**Persistence:** Items 3 and 10 write `group_ids` through the `edit` command (SQLite). Others are ephemeral or filesystem operations.
**Rationale & notes:** The menu is *organise and destroy*, with one exception (`Add content`). Everything about running and configuring the instance lives behind `View instance`. `Duplicate instance` being disabled unless `install_stage === 'installed'` is a deliberate guard: duplicating a half-installed instance would copy a broken state and the copy would need repair too.

---

## LIB-7 — Group context menu
**Type:** menu
**Nav path:** right-click anywhere in a group section that is not a card
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/library/instance-group/index.vue:326-360`]; [SOURCE `apps/app-frontend/src/locales/en-US/index.json` → `app.library.group.*`]
**Purpose:** A separate `ContextMenu` labelled `Group actions`, bound to the group container's `contextmenu.prevent.stop`. The list:

| # | Label | Icon | Shown when | Action |
| --- | --- | --- | --- | --- |
| 1 | `New instance` | plus | always | opens the add-instance dialog (LIB-29) |
| 2 | `Add to group` | square-plus | custom groups only | opens the add-instances modal (LIB-14) |
| 3 | `Edit group name` | edit | custom groups only | starts inline editing |
| — | *divider* | | custom groups only | |
| 4 | `Delete group` | trash | custom groups only | confirmation if non-empty, else immediate (LIB-13) |

"Custom group" means: grouping mode is `Custom group` **and** the section is neither the synthetic `Ungrouped` section nor `Favorites`. Items 2–4 are therefore absent on `Ungrouped` and `Favorites` — which cannot be renamed, have instances added to, or be deleted.

There is a small but careful detail here: opening this menu sets `groupContextOpen = true`, which **disables the header's disclosure toggle** so that the same right-click cannot both open a menu and collapse the section. The close event resets the flag.
**Controls:** As tabulated.
**Persistence:** `New instance` → creates an instance; `Delete group` → deletes the group row (memberships become empty, LIB-13).
**Rationale & notes:** Putting `New instance` at the top of the group menu is a context signal: right-clicking the "Fabric" group and choosing "New instance" tells the app nothing about the loader, but the menu advertises that instance creation is available from anywhere in the library, not just the toolbar.

---

## LIB-8 — Group header hover actions
**Type:** button
**Nav path:** hover the group header
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/library/instance-group/group-action-buttons.vue:51-100`]; [SOURCE `apps/app-frontend/src/components/ui/library/instance-group/index.vue:579-591`]
**Purpose:** A hover-revealed cluster of four quiet icon buttons at the right of the group header, present when the section is a custom group **or** `Ungrouped` (for `Ungrouped`, delete and edit are hidden):

| Button | Tooltip | Shown on | Enabled when |
| --- | --- | --- | --- |
| ↑ arrow | `Move group up` | always | not the first group in order |
| ↓ arrow | `Move group down` | always | not the last group in order |
| pencil | `Edit group name` | custom groups only | — |
| square-plus | `Add to group` | custom groups only | — |
| trash | `Delete group` | custom groups only, not `Ungrouped` | not mid-delete |

So `Ungrouped` gets only the two move arrows — it is a real, positionable section, but not renameable or deletable. `Favorites` gets none of these buttons at all (it is pinned to the top by sort order).
**Controls:** Click to act; the container is marked `instance-group-reorder-ignore` so it does not interfere with the header's drag handle.
**Persistence:** Moves write `set_group_order` (SQLite `display_order`); delete writes `delete_group`; edit opens inline editing.
**Rationale & notes:** Duplicating the group menu as hover buttons is the "both audiences" pattern: the context menu is for discovery, the hover cluster is for routine use. Arrow-disabling at the boundaries is correct — there is no wrap-around, and the buttons say so rather than silently doing nothing.

---

## LIB-9 — Drag & drop between groups
**Type:** button
**Nav path:** drag a card onto another group section
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/library/use-library.ts:590-783`]; [SOURCE `apps/app-frontend/src/components/ui/library/instance-group/instance-group-dnd.vue:1-240`]; [SOURCE `apps/app-frontend/src/components/ui/library/instance-group/index.vue:223-230`, `:508-515`]
**Purpose:** Moving instances between groups by dragging, implemented with `@dnd-kit`. Availability is conditional: the card's drag handle is disabled unless grouping mode is `Custom group` (`instance-card.vue:112-121`) and the group sections are drop targets only in the same mode (`instance-group/index.vue:223-230`).
Rules:
- Dragging a card that is **part of the current selection** drags the whole selection (primary card first), otherwise just that card.
- **Move** (default): the card leaves its source group(s) and joins the target. Dropping on `Ungrouped` removes the source memberships.
- **Add** (Alt held at any point during the drag): the card is **copied** into the target without leaving its source. The status popover reads `Duplicate to group`, or `Duplicate {n} instances to group` for multi-select.
- A drop target is only highlighted (dashed outline) when the drop *would change something* — `getInstanceGroupDropState` returns `{alreadyInGroup, canDrop, operation}` and `canDrop` is false when every dragged instance is already in the target (`use-library.ts:643-683`). Hovering a non-valid target with Alt held and nothing to change shows `Already in this group`.
- Collapsed groups auto-expand after a **750 ms** hover (`GROUP_HOVER_OPEN_DELAY`) — a deliberate dwell so you do not accidentally expand a section while passing over it.
- An optimistic `pendingInstanceGroupIds` map updates the UI before the write lands; it is rolled back on failure and cleared when the backend list confirms the new memberships.
- Dropping onto the **section background** (not a card) commits to that group; the group's `useDroppable` region is the whole section.

**Operation:** One batched `set_group_memberships([{instance_id, group_ids}, …])` write for the whole drag, then the selection is cleared.
**Controls:** Drag; hold Alt to duplicate instead of move.
**Persistence:** `instance_group_memberships` in SQLite. The `Favorites` pseudo-group is just another group ID in `group_ids`.
**Rationale & notes:** The Alt-to-duplicate modifier is borrowed from file managers and is well chosen here, because "add to group" and "move to group" are the same gesture with opposite data consequences. Gating all of it behind `Custom group` mode is important: in `Loader` / `Game version` / `Instance type` grouping the sections are derived views, so dropping into one would be meaningless — refusing the gesture is more honest than pretending to persist a grouping that would vanish on the next sort.

---

## LIB-10 — Drag-gather and Alt-drag "duplicate into group"
**Type:** toggle
**Nav path:** drag several unselected cards over a group
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/library/instance-group/instance-group-dnd.vue:56-69`]; [SOURCE `apps/app-frontend/src/components/ui/library/instance-group/use-instance-drag-gather.ts:16-28`]; [SOURCE `apps/app-frontend/src/components/ui/library/use-library.ts:685-704`, `:785-797`]
**Purpose:** A multi-select accelerator for LIB-9. Without selecting first, dragging a card over another already-grouped card gathers more cards into the drag after `gatherDuration = 500` ms of dwell, producing a stack preview (a `DragGather` badge with the growing count) and a drag preview layer. It is a "toss a pile in" gesture.
The Alt-key listeners are global and symmetric: `keydown Alt` sets `isAddingInstanceToGroup = true`, `keyup Alt` clears it, and `window blur` clears it too — the blur handler matters because releasing Alt while the window is unfocused (alt-tabbing away mid-drag) would otherwise leave the drag stuck in duplicate mode.
**Operation:** Hovering a foreign card for 500 ms while dragging adds it to the drag payload.
**Controls:** Drag + dwell; Alt toggles move/duplicate.
**Persistence:** None of its own; it feeds LIB-9's single write.
**Rationale & notes:** The dwell-threshold gather is a power-user affordance that costs nothing to learn (you just notice the badge) and removes the need to shift-click five cards before dragging them. The `blur` handler is a small correctness detail that most implementations miss.

---

## LIB-11 — Create group
**Type:** dialog
**Nav path:** toolbar `New group`, the selection action bar's `New group`, or a group context menu
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/library/library-toolbar/new-group-modal.vue:1-90`]; [SOURCE `apps/app-frontend/src/components/ui/library/use-library.ts:799-813`, `:991-1019`]
**Purpose:** A modal titled `Create group` with two stacked regions: a `Group name` input at the top, and below a divider a scrollable instance picker with its own `Search instance` box. Each row shows the instance icon, name and — if it is already grouped — its first group's name as a `TagItem` (falling back to `Unknown group` for a dangling ID). Rows are `Add` / `Added` toggles. Instances are ordered **grouped-last** (ungrouped first, alphabetical) so the ones most likely to want grouping are at the top.
The name input is pre-filled with a non-colliding default: `Group {n}` where `n` starts at `groupNames.size + 1` and increments until it does not collide with an existing name. The input auto-selects on click and is capped at `MAX_INSTANCE_GROUP_NAME_LENGTH = 256`.
Creation creates the group and **applies the ticked instances in the same operation** (one `create_group` + one `set_group_memberships`), so there is no "created an empty group, now go fill it" intermediate state.
**Controls:** name (max 256, pre-filled, selected); search; per-instance Add/Added.
**Persistence:** `instance_groups` table + memberships.
**Rationale & notes:** The `Group {n}` default with collision avoidance is a small touch that avoids every new group being called `Group 1`. Pre-selecting the group name for typing means the user who wants a name never has to clear it first.

---

## LIB-12 — Rename group (inline)
**Type:** field
**Nav path:** group header pencil, group context menu, or double-click the name
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/library/instance-group/index.vue:394-422`, `:546-558`]; [SOURCE `apps/app-frontend/src/components/ui/library/use-library.ts:1074-1090`]; [SOURCE `apps/app-frontend/src/locales/en-US/index.json` → `app.library.group.name-*`, `app.library.group.rename-failed`]
**Purpose:** Inline editing in the header via `InlineEditableText` in `activation-mode="manual"`, max width 24 rem, edit label `Rename`, `maxlength = 256`. There is no separate rename dialog. Validation is four-layered and each failure raises an error notification titled `Unable to rename group`:

| Rule | Message |
| --- | --- |
| trimmed name empty | `Group names cannot be empty.` |
| longer than 256 | `Group names cannot be longer than {maxLength} characters.` |
| case-insensitively `none` | `"None" is reserved and cannot be used as a group name.` |

The reserved-word rule exists because "None" is the label of the synthetic ungrouped section; allowing a user group called "None" would produce two indistinguishable sections. The validation runs client-side before the write; the Rust `rename_group` returns the updated group and the composable swaps it into the local list.
**Operation:** Click the name (or the pencil / `Edit group name` menu item) → type → commit.
**Controls:** Free text, 256 max, three validation rules.
**Persistence:** `instance_groups.name`.
**Rationale & notes:** `isUngrouped` and `isFavorites` sections render a plain `<span>` instead of the editable component, so renaming is structurally impossible for them rather than merely blocked. Validation failures surface as a notification with the specific reason rather than a generic error.

---

## LIB-13 — Delete group and its confirmation
**Type:** dialog
**Nav path:** group header trash (or context menu `Delete group`)
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/library/instance-group/index.vue:304-324`, `:670-692`]; [SOURCE `apps/app-frontend/src/components/ui/library/use-library.ts:1062-1072`]
**Purpose:** Deleting a group **never deletes instances**. The confirmation modal is titled `Delete group` with `fade="danger"`, 500 px wide, and a single line: **`Instances in this group will be ungrouped.`** Actions are `Cancel` and a red `Delete group`.
The confirmation is **conditional on emptiness**: `requestGroupDeletion` checks `instanceGroup.instances.length > 0` and skips the modal entirely for an empty group (`index.vue:316-324`). So the dialog exists to answer "what happens to my instances?", not to ask "are you sure?" — for an empty group there is nothing at stake.
**Operation:** `delete_group(groupId)` → on failure, `refreshGroups()` re-syncs and the modal stays open; on success the modal hides and the local list drops the group.
**Controls:** Cancel / Delete group (disabled while the delete is in flight).
**Persistence:** `DELETE` on `instance_groups`; the join table drops the memberships.
**Rationale & notes:** Only **custom** groups can be deleted — `Ungrouped` and `Favorites` are excluded at the call site as well as by button visibility. The "ungrouped" framing is the correct mental model: the group is a label, not a container, and the copy says so in one sentence.

---

## LIB-14 — "Add instances to group" modal
**Type:** dialog
**Nav path:** group header square-plus, or group context menu `Add to group`
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/library/group-instances-modal.vue:1-90`]; [SOURCE `apps/app-frontend/src/components/ui/library/use-library.ts:819-942`]
**Purpose:** The bulk-membership editor, titled `Add instances to "{groupName}"` (with `Ungrouped` substituted for the `group:none` pseudo-group). Same shape as the create-group picker — `Search instance` box, scrollable list of icon + name + `Add`/`Added` toggle — but it opens **pre-ticked with the group's current membership**, so it is a true two-way editor rather than an "add" action. Rows are alphabetical. A `Clear all` control in the header of the *create-group* dialog has no counterpart here; instead the footer is `Cancel` / `Save changes`.
The save computes a minimal diff (`saveGroupInstances` compares desired vs current membership per instance) and issues **one** `set_group_memberships` write for only the changed rows. When the target is `Ungrouped`, selecting an instance *removes* it from all groups, and already-ungrouped rows are non-deselectable.
After the write the composable carefully repairs the current selection so that removing a card's group does not silently drop the user's multi-selection: selections whose group no longer applies are deleted, and if that leaves an instance with no valid selection a replacement selection is synthesised against the instance's first remaining group.
**Controls:** Search; per-instance Add/Added toggle; Save changes (disabled while saving).
**Persistence:** `set_group_memberships`.
**Rationale & notes:** The selection-repair logic is the interesting part — it treats the library selection as a *view* of groups rather than as independent state, so a bulk membership edit cannot leave the user with an invisible-but-selected card. Most implementations would not bother, and the result would be an invisible selection.

---

## LIB-15 — Group reordering (drag + arrows)
**Type:** button
**Nav path:** drag a group header by its blank area, or use the ↑/↓ header buttons
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/library/index.vue:98-178`]; [SOURCE `apps/app-frontend/src/components/ui/library/use-library.ts:1107-1175`]; [SOURCE `apps/app-frontend/src/locales/en-US/index.json` → `app.library.group.move-up`, `.move-down`]
**Purpose:** Groups are user-orderable, and there are two ways to do it: drag the group header (`vuedraggable`, 250 ms animation, swap threshold 0.75, force-fallback, drag handle = the header row, filter excludes inputs/textareas/contenteditables and `.instance-group-reorder-ignore`) or the ↑/↓ buttons (LIB-08). Both call the same `reorderGroups`.
Details:
- `Favorites` is **pinned first** and is excluded from the draggable list entirely (`visibleReorderableGroups` filters it out).
- The synthetic `Ungrouped` group participates in the ordering, and its slot is persisted separately as `ungroupedGroupPosition` — so "Ungrouped" can sit between custom groups rather than always last.
- On drag end, the new order is reconciled against the previous order and `reorderGroups` returns early (no write) if nothing actually changed.
- A 200 ms FLIP animation on the remaining groups smooths the reorder; it is skipped entirely when `prefers-reduced-motion: reduce` is set.
- During a drag a global `html.instance-group-reordering` class forces `user-select: none` and `cursor: grabbing` so the browser does not select text.
- `moveGroup` is disabled while `reorderingGroups` is true, so rapid clicking cannot interleave writes.
**Operation:** One `set_group_order([customGroupIds])` write when the custom-group order changed; `ungroupedGroupPosition` is written to localStorage regardless (it is client-only state).
**Controls:** Drag the header; ↑/↓ buttons.
**Persistence:** `display_order` in SQLite for custom groups; `localStorage['Instances-grid-display-state'].ungroupedGroupPosition` for the Ungrouped slot.
**Rationale & notes:** Splitting the two orderings — durable DB order for real groups, localStorage for the synthetic slot — is pragmatic and correct: there is no row to carry an order for `Ungrouped`. The `prefers-reduced-motion` guard and the global user-select lock are both details most implementations skip.

---

## LIB-16 — Grouping modes
**Type:** button
**Nav path:** toolbar `Group by` combobox
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/library/use-library.ts:69-75`, `:429-566`]; [SOURCE `apps/app-frontend/src/components/ui/library/library-toolbar/sort-menu.vue:77-90`]; [SOURCE `apps/app-frontend/src/locales/en-US/index.json` → `app.library.group-by.*`]
**Purpose:** Five modes, in UI order, in one combobox prefixed by a grid icon:

| Mode | Label | How groups are derived | Group ordering |
| --- | --- | --- | --- |
| `Group` | `Custom group` (default) | the user's groups; instances with no group go to `Ungrouped` | `Favorites` first, then the user's stored order |
| `Instance type` | `Instance type` | `Custom` (no `link`), `Modpack` (has a `link`, not a server), `Server` (has a `link` whose project has `minecraft_server`) | alphabetical |
| `Loader` | `Loader` | one section per `instance.loader` | fixed priority: **fabric, forge, neoforge**, then alphabetical |
| `Game version` | `Game version` | one section per `instance.game_version` | descending, numeric-aware (`1.21` before `1.9`) |
| `None` | `No grouping` | a single section | n/a |

Two of these deserve attention. **Instance type** requires an extra network call: it batch-fetches every linked instance's project v3 record to learn whether it is a server project (`watchEffect` + `get_project_v3_many`, `use-library.ts:353-382`), so this classification is *not* available purely from local data — an offline library will misclassify servers as modpacks. **Loader** has a hand-authored priority (`libraryLoaderPriority = { fabric: 0, forge: 1, neoforge: 2 }`) which exists because alphabetical would put `neoforge` between `forge` and `quilt` and bury the three that matter.

An instance belonging to multiple custom groups is rendered **once per group** in `Custom group` mode — that is the origin of the `(groupId, instanceId)` selection key (LIB-05).
**Controls:** Single-select combobox, `dropdown-min-width: 160px`, `show-icon-in-selected: false`.
**Persistence:** `localStorage['Instances-grid-display-state'].group`, default `'Group'`. An unrecognised stored value is reset to `'Group'` on load. Changing the mode clears the multi-selection.
**Rationale & notes:** The **Loader** priority list is the clearest piece of product thinking in the toolbar: it encodes that ~95% of installs are one of three loaders, and it is invisible to users who do not care. Likewise `Game version` sorting descending-numerically avoids the classic `1.10` before `1.9` bug.

---

## LIB-17 — Sorting options
**Type:** button
**Nav path:** toolbar `Sort by` combobox
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/library/use-library.ts:59-67`, `:434-467`]; [SOURCE `apps/app-frontend/src/components/ui/library/library-toolbar/sort-menu.vue:16-58`]
**Purpose:** Seven options, in UI order, with the exact comparator for each:

| # | Label | Comparator | Meaning |
| --- | --- | --- | --- |
| 1 | `Name` | `a.name.localeCompare(b.name)` | alphabetical |
| 2 | `Last played` **(default)** | `dayjs(b.last_played ?? 0).diff(dayjs(a.last_played ?? 0))` | most-recently-played first; never-played instances fall to the bottom |
| 3 | `Hours played` | `b.recent_time_played + b.submitted_time_played` − same for `a` | total play time |
| 4 | `Date created` | `dayjs(b.created).diff(dayjs(a.created))` | newest first |
| 5 | `Date modified` | `dayjs(b.modified).diff(dayjs(a.modified))` | recently-touched first |
| 6 | `Loader` | localised loader name `localeCompare` | alphabetical by *display* name, not raw id |
| 7 | `Game version` | numeric-aware `localeCompare` | `1.21` before `1.9` |

Sort applies **within** each group, not across groups. The one exception is `Name`, which also sorts the **group sections themselves** alphabetically — so choosing `Name` produces a globally alphabetical library including section names.

`Last played` being the default is the load-bearing decision: the most common reason to open the launcher is to resume something, and it makes the first card after launch the instance you were last playing. The `QuickInstanceSwitcher` in the left rail (LIB-32) sorts by the same key with a `last_played ?? created` fallback, so the rail and the library agree.
An unrecognised persisted sort value is reset to `'Last played'` on load.
**Controls:** Single-select combobox, prefixed by an up/down-arrow icon whose `aria-label` is `Sort by`.
**Persistence:** `localStorage['Instances-grid-display-state'].sortBy`, default `'Last played'`. Any change resets to page/group top via the `onFilterChange`-equivalent path.
**Rationale & notes:** `Hours played` is a **lifetime** metric (recent + submitted), not a rolling window, and the label does not say so — a reasonable assumption for a launcher but worth stating plainly. The `Loader` sort using the localised display name is the right choice for a user-facing list.

---

## LIB-18 — Filtering
**Type:** toggle
**Nav path:** toolbar `Filter by` control
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/library/library-toolbar/filter-menu.vue:28-56`]; [SOURCE `apps/app-frontend/src/components/ui/library/use-library.ts:414-427`]
**Purpose:** A single `DropdownFilterBar` labelled `Filter by`, with `Add filter` and `Clear filters` affordances, checkboxes on the right, and `apply-immediately` (no apply button). Three categories, all **multi-select**, all AND-ed together:

| Category | Options | Source | Option ordering |
| --- | --- | --- | --- |
| `Instance type` | `Modpack`, `Server`, `Custom` | fixed list | fixed |
| `Game version` | the distinct `instance.game_version` values **present in this library** | derived, `searchable: true` | descending, numeric-aware |
| `Loader` | the distinct `instance.loader` values **present in this library** | derived | alphabetical by display name |

The two derived categories are computed from the library's own contents, not from a global tag list. This is a deliberate difference from Browse, where facets come from the whole catalogue: in a library of four instances, a 100-entry Minecraft version list would be noise. It also means the options change as instances are added or removed.
Filtering composes with search and grouping: `filteredInstances` applies the three facet checks, and the group builder then applies the search substring and the sort.
`hasActiveFilters` (any non-empty facet selection) suppresses the "show empty groups" behaviour, so a filtered library never displays empty sections.
**Controls:** Multi-select per category; `Clear filters`; instant apply.
**Persistence:** `localStorage['Instances-grid-filters']` = `{ instanceType: [], gameVersion: [], loader: [] }`, with `mergeDefaults: true` so a schema addition does not wipe existing selections.
**Rationale & notes:** Deriving the option sets from the library is the single best decision in this toolbar — it makes the filter control a direct answer to "what do I actually have?" Empty-group suppression under an active filter is the other one: showing an empty "Forge" section because everything in it was filtered out reads as a bug.

---

## LIB-19 — Library search
**Type:** field
**Nav path:** toolbar search box (no URL parameter)
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/library/library-toolbar/index.vue:32-39`]; [SOURCE `apps/app-frontend/src/components/ui/library/view-state.ts:3-4`]; [SOURCE `apps/app-frontend/src/components/ui/library/use-library.ts:268`, `:430-432`]
**Purpose:** A `clearable` search input with placeholder `Search`, bound to a **module-level singleton** `librarySearch` so its value survives navigating away from the Home tab and back. Matching is a plain case-insensitive substring test on `instance.name` only — names, not loader/version/content.
Searching has three side effects that are not obvious from the control itself:
1. `isSearching` becomes true, which **force-opens every group section** (`Accordion :force-open="isSearching"`) and suppresses the header disclosure toggle — a collapsed section must not hide matches.
2. The header toggle is blocked entirely while searching.
3. Collapse-state writes are ignored while searching (`setSectionCollapsed` early-returns), so a pass-through search cannot destroy the user's accordion state.

There is **no debounce and no network**: the filter is a pure client-side `Array.filter` over an already-loaded instance list, so it is instant by construction.
**Controls:** Free text; clear button.
**Persistence:** Module singleton — **not** `localStorage`, so a search does not survive an app restart. This is the right call: a stale search that silently hides instances at next launch would be harmful.
**Rationale & notes:** The force-open behaviour is the detail that makes search trustworthy. Most list implementations leave collapsed sections collapsed and the user concludes "search is broken".

---

## LIB-20 — Multi-select floating action bar
**Type:** indicator
**Nav path:** appears when ≥1 instance is selected
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/library/LibrarySelectionActionBar.vue:1-59`, `:133-205`]
**Purpose:** A floating bar (hidden when any modal is open) with `{count} selected` in tabular figures, then actions:

| Action | Shown when | Behaviour |
| --- | --- | --- |
| `Clear` | always | clears the selection |
| `New group` | grouping is `Custom group` | creates a group with an auto-generated `Group {n}` name from the selected instances, **drops the selection**, forces grouping mode to `Group`, and auto-opens inline rename on the new group |
| `Remove from group` | ≥1 selected card is actually in the group it was selected from | removes exactly the groups the selections came from, preserving other memberships |
| `Delete` (red, filled on hover) | always | opens the multi-instance delete confirmation (LIB-21) |

Deleting a selection uses `Promise.allSettled` over `remove(instanceId)` per instance, keeps only the successful ids in the selection, and surfaces each individual failure via `handleError` — so one instance failing to delete does not abandon the rest.
All buttons disable while `busy` (deleting, removing from group, or creating a group).
**Controls:** As tabulated.
**Persistence:** None; operates on the in-memory selection.
**Rationale & notes:** This is the clearest statement of the library's bulk-operation scope: **organise and destroy only**. There is no bulk play, bulk stop, bulk repair, bulk duplicate or bulk export. `New group` auto-opening the rename field after creation is a small kindness — the generated name is a placeholder the user is expected to replace immediately.

---

## LIB-21 — Deletion confirmation
**Type:** dialog
**Nav path:** `Delete` in the instance context menu, or the selection bar's red `Delete`
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/modal/ConfirmDeleteInstanceModal.vue:1-186`]; [SOURCE `apps/app-frontend/src/locales/en-US/index.json` → `app.instance.confirm-delete.*`]
**Purpose:** One modal for both single and bulk delete, `fade="danger"`, 500 px. It has **three pluralised string sets**, switched on the instance count:

| Element | 1 instance | N instances |
| --- | --- | --- |
| Header | `Delete instance` | `Delete {count} instances` |
| Admonition header | `This action cannot be undone` | same |
| Admonition body | `All data for your instance will be permanently deleted, including your worlds, configs, and all installed content.` | `All data for these {count} instances will be permanently deleted, including their worlds, configs, and all installed content.` |
| List label | `Instance` | `Instances ({count})` |
| Scrolling list | icon + name + `{Loader} {game_version}` per row, max-height 240 px with fade indicators | same, for each |
| Actions | `Cancel` / `Delete instance` | `Cancel` / `Delete {count} instances` |

The scrolling, fade-capped list is the important part of a bulk confirm: it names exactly what is about to be destroyed rather than saying "are you sure?". There is **no typing-to-confirm**, no backup offer, and no option to keep the instance's worlds — the copy is unambiguous that everything goes, which matches the Rust `remove` semantics.
**Operation:** `Delete` emits to either `deleteInstance` (single, from `use-library.ts:1177-1182`) or `deleteSelectedInstances` (bulk, from the action bar). The backend deletion is the standard `remove` command; the app emits an `instance` `removed` event which the library listens to.
**Controls:** Cancel / Delete (red, with a trash icon).
**Persistence:** Removes the instance directory and its SQLite rows.
**Rationale & notes:** Naming the loss — *worlds, configs, and all installed content* — is far more useful than a generic warning, because those three categories map onto the three things users actually fear losing. The bulk list is capped at 240 px with scroll + gradient fades, which is the right call for a 40-instance selection.

---

## LIB-22 — Duplication
**Type:** button
**Nav path:** instance context menu → `Duplicate instance`
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/library/use-library.ts:1330-1335`, `:1184-1186`]; [SOURCE `apps/app-frontend/src/helpers/install.ts:242-246`]
**Purpose:** A single click, **no dialog**, no name prompt, no options. It calls `install_duplicate_instance(instanceId)` which returns an `InstallJobSnapshot` and proceeds as a background install.
Enabled only when `install_stage === 'installed'` — a guard that prevents cloning a half-installed or broken instance into a second broken instance.
The new instance's name is generated by the backend, so the library shows a transient duplicate of the original's name until the list refreshes; there is no rename step, and the user is expected to rename the copy inside the instance's own settings.
**Operation:** `copy_dotminecraft_with_reporter` walks the source instance's files: managed content is copied through the content store (preserving `project_id`/`version_id` origin, enabled state, and the frozen/locked flag), unmanaged files are plain-copied. Two hard failures are possible and are explicit: **`Stop the source instance before duplicating it`** and **`Restore or repair {relative_path} before duplicating this instance`** / **`Repair {relative_path} before duplicating this instance`**. Symlinks in the source are skipped with a warning rather than followed.
**Controls:** One menu item.
**Persistence:** A new instance directory plus a SQLite row.
**Rationale & notes:** "Duplicate" with no dialog is right for the common case — the overwhelmingly typical reason to duplicate is "I want to try a change", and the source is usually not running. The two blocking errors are good design: they name the exact file that is damaged rather than failing with a generic error.

---

## LIB-23 — Export modpack — **not** in the library
**Type:** dialog
**Nav path:** instance page header only (`/instance/<id>` → header export action)
**Evidence:** [SOURCE `apps/app-frontend/src/pages/instance/layout.vue:7`, `:52`]; [SOURCE `apps/app-frontend/src/components/ui/ExportModal.vue:19-51`]; [SOURCE `apps/app-frontend/src/locales/en-US/index.json` → `app.export-modal.*`]; [SOURCE `apps/app-frontend/src/helpers/instance.ts:560-596`]
**Purpose:** Worth documenting precisely because it is an **absence**. There is no export action anywhere in the library — not on the card, not in the instance context menu (LIB-6), not in the selection action bar (LIB-20). Export exists only on the instance page header, and is itself gated: `<ExportModal v-if="!instance.quarantined" …>` and the header action is `@export="() => !instance?.quarantined && exportModal?.show()"`.
The modal is titled `Export modpack` and offers: `Modpack name` (default `Modpack name` placeholder), `Version number` (default placeholder `1.0.0`), a description field (`Enter modpack description...`), a checkbox list of **override candidates** from `get_pack_export_candidates` (the modpack's own `overrides` tree), and an `Export` button. Success notification: `Export complete` / `{name} was exported successfully.`
**Operation:** `export_instance_mrpack(...)` writes an `.mrpack` file to a user-chosen path.
**Controls:** name, version, description, override selection, destination.
**Persistence:** Writes a file outside the app's data directory.
**Rationale & notes:** The placement is coherent — export is an operation *on* an instance's content configuration, so it belongs with the instance, not with the index of instances. It is also the only import/export round-trip: the `.mrpack` produced here is exactly what `Upload a modpack` in LIB-29 accepts.

---

## LIB-24 — Open folder and Copy path
**Type:** button
**Nav path:** instance context menu
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/library/use-library.ts:1336-1347`]
**Purpose:** Two escape hatches into the filesystem:
- **`Open folder`** (folder-open icon) → `showInstanceInFolder(instanceId)`, which reveals the instance directory in the OS file manager. Available unconditionally, on every instance.
- **`Copy path`** (clipboard icon) → `navigator.clipboard.writeText(instance.id)`. Available unconditionally. Note that it writes the **instance ID**, which is also the instance's directory name, so the copied string is directly usable as a relative path — but it is an ID that the user did not choose and cannot predict, which makes this item much less useful than a "copy absolute path" would be.
Neither has a success toast; neither has an error path beyond `handleError` on `open`.
**Controls:** Two menu items.
**Persistence:** None.
**Rationale & notes:** `Open folder` is the one library action that must exist: users routinely need to reach `saves`, `config`, `resourcepacks` or a `crash-reports` folder, and making them hunt for it in `%APPDATA%` would be a support burden. `Copy path` copying an opaque ID is the weaker of the two — an absolute path, or the instance folder name, would serve better.

---

## LIB-25 — Icon editing submenu
**Type:** menu
**Nav path:** instance context menu → `Edit icon` (submenu)
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/library/use-library.ts:1234-1262`]; [SOURCE `apps/app-frontend/src/components/ui/library/index.vue:394-399`]; [SOURCE `apps/app-frontend/src/locales/en-US/index.json` → `instance.settings.tabs.general.edit-icon.*`]
**Purpose:** Icon management nested one level under `Edit icon`, with **three items whose labels change based on current state**:

| Item | Label when no icon | Label when an icon exists | Shown |
| --- | --- | --- | --- |
| Upload | `Select icon` | `Replace icon` | always |
| Palette | `Create an icon` | `Edit icon` | always |
| Trash | `Remove icon` | `Remove icon` | only when `icon_path` is set |

- **Select / Replace icon** opens a native file dialog filtered to `png, jpeg, svg, webp, gif, jpg`, then `edit_icon(instanceId, path)`. Analytics `InstanceSetIcon`.
- **Create / Edit icon** opens the in-app `IconEditorModal` (a separate editor with its own analytics `InstanceCreateIcon` / `InstanceEditCreatedIcon` / `InstanceSaveCreatedIcon`), seeded with the instance's `icon_config` when one exists. This is a *generated* icon: the instance carries an `icon_config` in addition to `icon_path`, so "create" means "draw me one" and "edit" means "reopen the editor".
- **Remove icon** calls `edit_icon(instanceId, null)`. Analytics `InstanceRemoveIcon`.

The label-switching is the point: a user who has an icon should not be told "Select icon", and a user who has drawn one should not be told "Create an icon".
**Controls:** Native file dialog filter as above; the editor modal.
**Persistence:** `edit_icon` updates `icon_path` (and the icon cache); the editor writes `icon_config`.
**Rationale & notes:** Supporting **two** icon kinds — an uploaded file and a generated config — is unusual and user-friendly. The card's thumbnailer treats them differently (`instance-card-view.vue:20-33`): local raster files are downscaled client-side, while SVGs and remote URLs go through `getInstanceIconUrl` uncached locally, because rendering arbitrary SVG through an image pipeline is a different problem.

---

## LIB-26 — "Add content" shortcut
**Type:** button
**Nav path:** instance context menu → `Add content`
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/library/use-library.ts:1314-1320`]; [SOURCE `apps/app-frontend/src/components/ui/library/instance-group/instance-card.vue:231-237`]
**Purpose:** The library's one task-oriented action. It routes to `/browse/mod?i=<instanceId>` — or `/browse/datapack?i=<instanceId>` when the instance loader is `vanilla` — i.e. it opens Browse **already scoped to this instance**, with the instance's game version, loader and environment injected as provided filters (module 03, BR-16).
Availability: `!quarantined && !instance.link && !installing`. That means it appears **only for unmanaged instances** — a modpack or server instance has its content defined by its pack, so browsing for more would produce content the next update deletes. This matches the app's own warning copy: `Adding content may prevent you from joining this server. Any content you add will be removed when the managed server content is updated.`
**Operation:** One router push.
**Controls:** One menu item, context-gated.
**Persistence:** None; the instance ID rides in the URL.
**Rationale & notes:** Putting this in the library context menu rather than requiring "open instance → Content → Browse" is a real shortcut — it is the single most common reason to open the library (add a mod to the instance I play). The gating on `!link` is the interesting design decision, and it is stricter than a user would expect: they cannot add content to an installed modpack at all from here.

---

## LIB-27 — Pinning (Favorites)
**Type:** toggle
**Nav path:** instance context menu → `Add to favorites` / `Remove from favorites`
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/library/use-library.ts:1297-1312`, `:276`]; [SOURCE `apps/app-frontend/src/helpers/instance-groups.ts:3`]; [SOURCE `apps/app-frontend/src/components/ui/library/index.vue:81-94`, `:291-303`]
**Purpose:** There is no pin icon on the card. Pinning is a **special group** with the reserved ID `group:favorites`, toggled by adding or removing that ID from the instance's `group_ids` via the ordinary `edit` command. Consequences in `Custom group` grouping mode:
- A `Favorites` section is always rendered first, pinned above every other group by an explicit sort rule (`if (a.id === FAVORITES_GROUP_ID) return -1`).
- It is **excluded from the draggable list**, so it cannot be reordered, and its header action buttons are not rendered.
- It is shown **only when it contains at least one instance** — an empty Favorites section never appears (`visibleInstanceGroups`, `library/index.vue:81-88`).
- Its membership is independent of other groups: a favourited instance still appears under its other groups as well.
- The `Favorites` section is `selectable`: cards in it can be multi-selected, and selecting across Favorites and a normal group is tracked correctly because of the `(groupId, instanceId)` key.

The menu item's label flips between `Add to favorites` and `Remove from favorites`, and the `remove_from_favorites` variant gets a custom slot rendering a **filled** star — the current state is visible in the menu, not only in the wording.
**Controls:** One toggle, context-menu only.
**Persistence:** `group:favorites` in the instance's `group_ids` (SQLite join table). `FAVORITES_GROUP_ID` is a frontend constant that matches a group row the backend treats specially.
**Rationale & notes:** Implementing pinning as a group rather than a separate boolean is the key decision: it reuses drag-and-drop, selection, the group picker and reordering for free, at the cost of pinning not being available outside `Custom group` mode. A user grouping by `Loader` cannot pin anything, and the menu item still shows — clicking it appears to do nothing visible. That is the one real rough edge in this area.

---

## LIB-28 — Import from another launcher
**Type:** dialog
**Nav path:** Welcome screen `Import from launcher`, or add-instance dialog → `Import instance`
**Evidence:** [SOURCE `packages/app-lib/src/api/pack/import/mod.rs:30-56`]; [SOURCE `apps/app-frontend/src/providers/setup/instance-import.ts:14-30`]; [SOURCE `packages/ui/src/components/flows/creation-flow-modal/components/ImportInstanceStage.vue:1-97`]; [SOURCE `apps/app-frontend/src/providers/setup/creation-modal.ts:106-127`]
**Purpose:** The single highest-friction surface in the app, because it is the migration path off another launcher. **The supported list at v0.21.9 is exactly five launchers, derived from the `ImportLauncherType` enum** (`import/mod.rs:35-43`) and cross-checked against the frontend's detection list (`instance-import.ts:15`):

| Launcher | Enum variant | Default path probe | Instances subfolder | Validity test |
| --- | --- | --- | --- | --- |
| **MultiMC** | `MultiMC` | searched across data dir, `%USERPROFILE%`, Desktop, Downloads, `C:\MultiMC`, `%ProgramFiles%`, `%ProgramFiles(x86)%`, `/Applications/MultiMC.app/Data` — accepted only if `multimc.cfg` exists | read from `multimc.cfg` `[InstanceDir]`, else `instances` | `instance.cfg` parses **and** `mmc-pack.json` parses as `MMCPack` |
| **PrismLauncher** | `PrismLauncher` | `<data_dir>/PrismLauncher` | read from `prismlauncher.cfg` `[InstanceDir]`, else `instances` | same as MultiMC |
| **ATLauncher** | `ATLauncher` | `<data_dir>/ATLauncher` | `instances` | `instance.json` parses as `ATInstance` |
| **GDLauncher** | `GDLauncher` | `<data_dir>/gdlauncher_next` | `instances` | `config.json` parses as `GDLauncherConfig` |
| **Curseforge** | `Curseforge` (note the lowercase `f`, matching the folder name) | `~/curseforge/minecraft` if it exists, else `~/Documents/curseforge/minecraft` | `Instances` | `minecraftinstance.json` parses as `MinecraftInstance` |

**Not supported:** Technic, and any launcher not in the enum. There is **no `TechnicLauncher` variant at all** — worth stating explicitly because Technic is a common target. There is also no `.minecraft`-folder import type.

**The `Unknown` variant is the escape hatch.** Because the enum carries `#[serde(other)] Unknown`, passing any unrecognised launcher name (which is what the frontend does for a manually-added path — `getImportableInstances('Custom', path)`) causes the backend to **try all five types in order** and use the first that yields a non-empty instance list (`mod.rs:79-97`); at import time it retries in the same order to find which type actually contains the named folder (`mod.rs:193-228`), failing with `Could not determine launcher type for the given path` if none does. This is how "Add launcher path" works for a launcher nobody has ever heard of.

**The dialog** (`ImportInstanceStage`) shows: a `Launcher instances` header with `Clear all`; while detecting, `Detecting launcher instances...`; then one collapsible card per launcher (auto-expanded if it has instances) with a tri-state checkbox, the launcher name, and per-instance checkboxes filtered by a `Search instance name` box (searching auto-expands matching launchers and restores the pre-search expansion when cleared). Below the list, `Add launcher path` expands into a folder-browse icon button + a `Path to launcher…` input + `Add`, which registers the path as `Custom ({last path segment})`. If a path yields nothing, an error notification fires: `No instances found` / `No importable instances were found at the specified path.`

**What each conversion does, and what is lost** — this is the part that matters:

- **MultiMC / PrismLauncher** (`mmc.rs`): reads `instance.cfg` (`MMCInstance` — `name`, `iconKey`, `ManagedPack*`, `JavaPath`, `JvmArgs`, `ManagedPackType`) and `mmc-pack.json` (`MMCPack` — components with `uid`, `version`, `dependencyOnly`, `important`, `disabled`, `cachedRequires`, `cachedConflicts`). The launcher is detected as **Modrinth-managed** when `managed_pack_type: modrinth`, in which case `project_id`/`version_id` are carried across and the instance is linked (title `Imported Modrinth Modpack`); Flame- and ATLauncher-managed packs and unrecognised types are imported **unmanaged** instead. Minecraft components are translated into `PackDependency::{Minecraft, FabricLoader, Forge, NeoForge, QuiltLoader}` by **uid prefix** (`net.minecraft`, `net.fabricmc.fabric-loader`, `net.minecraftforge`, `net.neoforged`, `org.quiltmc.quilt-loader`). The `minecraft` folder is used if present, else `.minecraft`; a missing one is a hard error (`Instance is missing Minecraft directory`). The icon is recached from `<base>/icons/<iconKey>`.
  **Lost:** the `ui` component (declared in `MMCPack` but never read), `cachedRequires`/`cachedConflicts`/`important`/`disabled` (parsed into the struct, then ignored), custom `JavaPath`/`JvmArgs` (parsed, then ignored — the app uses its own Java settings), any non-Minecraft-family component (e.g. OptiFine, old Forge `net.minecraftforge:launchwrapper`), and all component-level overrides.
- **ATLauncher** (`atlauncher.rs`): reads `instance.json` (`ATInstance` — `id` = the *Minecraft version id*, `launcher.{name, pack, version, loaderVersion{type, version}}`, `javaVersion`). Loader type is lowercased and parsed as `ModLoader`; an unparseable type is a hard error (`Could not parse mod loader type: "…"`). Loader version is resolved through `get_loader_version_from_profile(game_version, loader, requested)`. **Despite `ATLauncherModrinthProject`/`ATLauncherModrinthVersion` being fully modelled in the struct, no Modrinth linking happens** — `CreatePackDescription` is built with `project_id: None, version_id: None`, so `InstanceLink::ModrinthModpack` is never produced and the instance is always unmanaged. Icon resolution tries `<instance>/instance.png`, then `<base>/configs/images/<packname-lowercased-alphanumeric>.png`.
  **Lost:** the entire Modrinth link (so no "update modpack", no `pack_installed` repair branch); `launcher.pack` identity beyond the icon filename; `javaVersion`; `modrinth_manifest`.
- **GDLauncher** (`gdlauncher.rs`): reads `config.json` (`GDLauncherConfig` — `background`, `loader.{loaderType, loaderVersion, mcVersion, source, sourceName}`). Title comes from `sourceName`, falling back to `GDLauncher-{folder}`. Loader version resolved through the same profile lookup. **Note the `mods` field is commented out in the struct** (`// pub mods: Vec<GDLauncherMod>`) — the mod list is explicitly not parsed.
  **Lost:** the mod list, and therefore per-mod enable/disable state; `loader.source` beyond using `sourceName` as a title; no Modrinth link.
- **Curseforge** (`curseforge.rs`): reads `minecraftinstance.json` (`MinecraftInstance` — `name`, `baseModLoader{name}`, `profileImagePath`, `installedModpack{thumbnailUrl}`, `gameVersion`). **Loader parsing is very narrow**: the loader name is split on `-` and matched only against `["forge", version]` and `["fabric", version, _game_version]`; anything else silently becomes `Vanilla`. Forge/Fabric loader versions are resolved through `get_loader_version_from_profile`. Icon comes from `profileImagePath`, else by **downloading** `installedModpack.thumbnailUrl` and caching it. Title falls back to `Curseforge-{folder}`.
  **Lost:** essentially everything CurseForge-specific — the entire installed-mod list, the project's CurseForge file IDs, per-mod overrides and config. NeoForge, Quilt, and any CF loader name in a different format (`forge-1.20.1-47.3.0` style with extra segments beyond the two handled, or `neoforge-*`) fall through to `Vanilla`. The instance is unmanaged with no path back to the pack.
- **Common to all five** (`import/mod.rs:368-629`): `finish_import` calls `copy_dotminecraft_with_reporter` then `install_minecraft_for_instance_id_with_reporter(instanceId, force = false)`. The copy walks every file recursively:
  - Managed content paths (mods, resourcepacks, shaders, config…) are **imported into the content store** with `origin: None` — i.e. Modrinth learns the file exists but **not which project/version it came from**, because none of the four formats carries that mapping for unmanaged instances. They are therefore "local files" for the rest of the app's life: no update available, no version switch, no dependency tracking.
  - `.disabled` suffix is preserved as `enabled: false`.
  - A duplicate canonical content path (e.g. both `x.jar` and `x.jar.disabled`) is recorded and **not** imported, to avoid a collision.
  - **Unmanaged symlinks are skipped** with a warning rather than followed.
  - Import refuses to overwrite an existing symlink (`Import cannot overwrite a symbolic link`).
  - Progress is reported per file under the `PreparingInstance` phase.
  - If the import fails at any point, the partially-created instance is **deleted** (`crate::api::instance::remove`) and the error propagates — no half-imported instances are left behind.
  - **Minecraft itself is always re-downloaded and re-installed** after the copy, so a corrupted or hand-edited launcher JAR cache does not carry over.
**Operation:** Detect (auto on modal mount, or manual path) → tick instances → `Create`. Each ticked instance is imported **sequentially** (`for … await import_instance`), so progress is linear and a failure in one does not stop the rest (each is individually `.catch(handleError)`-ed). If exactly one instance was imported, the app navigates to it. Analytics `InstanceCreate { source: 'CreationModalImport' }`.
**Controls:** Per-launcher tri-state checkbox (operates on the *filtered* visible list only, so searching then ticking respects the search); per-instance checkbox; `Clear all`; `Add launcher path`; search box.
**Persistence:** Imported instances are normal instances in SQLite; each is wrapped in an `InstallPhaseDetails::Import { launcher_type, instance_folder }` so the install progress UI can name the source.
**Rationale & notes:** The design is honest about its limits. The only format carrying enough metadata to reconstruct a real Modrinth link is MultiMC/Prism (because those launchers store `ManagedPackID`), and only for Modrinth-managed packs — which is exactly the case the code special-cases and comments on ("Kept separate as we may in the future want to add special handling for modrinth managed packs"). Everything else is imported as files, which is why the resulting instance cannot be updated. The `ATLauncherModrinthProject` / `ATLauncherModrinthVersion` structs are modelled but unused, which suggests the link path was intended and abandoned rather than never considered.

---

## LIB-29 — Add-instance dialog
**Type:** dialog
**Nav path:** toolbar `New instance`, left-rail `+`, group context menu `New instance`, and (server context) the install flow
**Evidence:** [SOURCE `packages/ui/src/components/flows/creation-flow-modal/components/SetupTypeStage.vue:1-108`, `:140-213`]; [SOURCE `packages/ui/src/components/flows/creation-flow-modal/index.vue:28-109`]; [SOURCE `packages/ui/src/components/flows/creation-flow-modal/components/CustomSetupStage.vue:1-213`]; [SOURCE `apps/app-frontend/src/providers/setup/creation-modal.ts:88-192`]
**Purpose:** A multi-stage modal (`CreationFlowModal`, type `instance`). **Stage 1** offers, first, a searchable project combobox under the prompt `Already know what you want to play?` (placeholder `Search mods, modpacks, and more…`, results limited to 10, per-option suffix `Install modpack` with a download icon for modpacks or `Create instance` with an arrow icon for everything else) — searching hits `project_types IN [mod, modpack, resourcepack, shader, datapack]`. Then an `or` divider, then the heading `Choose instance type` and four large option buttons:

| Option | Description |
| --- | --- |
| `Custom setup` | `Start from scratch by picking a loader and game version.` |
| `Start from a mod or modpack` | `Choose a project and we'll use its latest version.` |
| `Upload a modpack` | `Install a modpack from an .mrpack file on your device.` |
| `Import instance` | `Import an instance from Prism, CurseForge, or similar.` |

In the **server** variants of this flow the order changes and `Vanilla Minecraft` (`Classic Minecraft with no mods or plugins.`) replaces `Import instance` — the option set is explicitly flow-dependent.

**Stage 2 — `Custom setup`** is the field list:
1. **Icon** preview with a hover `X`, and `Upload` / `Randomize` / `Customize` buttons. Randomize runs automatically when the stage mounts for an instance flow.
2. **Name** — placeholder `Enter instance name`, or an auto-generated name when empty. Existing names are fetched (`fetchExistingInstanceNames`) and supplied to the flow context.
3. **Loader** — a `Chips` single-select group: `vanilla` first, then the available loaders (default `fabric, neoforge, forge, quilt` for the instance flow; `vanilla, fabric, neoforge, forge, quilt, paper, purpur` for the server flow). When a project is preselected, the loader list is narrowed to that project's `compatibleLoaders`.
4. **Game version** — searchable `Combobox`, filtered by what the chosen loader actually supports (read from the loader's version manifest, so Fabric shows only versions with builds). Default is the newest release; a dropdown footer toggle flips `Show all versions` ⇄ `Hide snapshots`.
5. **Loader version** — a collapsed section that only appears once both loader and game version are chosen. Three mode chips: `Stable` / `Latest` / `Other`; `Stable` auto-selects the newest version flagged stable and auto-disables itself if none is. For `paper`/`purpur` the label changes to `Build number` and the picker is a plain build-number combobox with a channel badge (`ALPHA`/`BETA`) instead of version chips.

**Paths, precisely:**
- **Vanilla / with-loader** → `Custom setup` (or the `Vanilla Minecraft` option in server flows) → `install_create_instance({ name, gameVersion, loader, loaderVersion, iconPath, iconConfig })` → navigate to `/instance/<id>`.
- **From modpack** → either search-select a project (takes the newest version: `versions[0]`, deliberately not the newest *compatible* one) or pick an `.mrpack` file. Both run through the duplicate guard (LIB-35's modal). A file is previewed with `install_get_modpack_preview` first; if it contains unknown files or external (non-Modrinth) files, `UnknownPackWarningModal` fires — `Confirm installation` / `Unknown files warning`, `… contains files that aren't published on Modrinth. We strongly recommend only installing files from sources you trust.`, `Files that aren't published to Modrinth aren't reviewed.`, `Unrecognized files`, `Malware is often distributed through mod files by sharing them on platforms like Discord.`, with `Don't show this warning again` / `Install anyway` / `Dont install` (sic). The "don't show again" writes `warn_on_unknown_modpacks` to Rust settings.
- **From a known project (non-modpack)** → `prepareProjectInstall` + `createProjectInstall` hand off to the content-install provider's **new instance** tab (module 03, BR-31), so the field set is the same minus loader-version.

**Operation:** Every path ends in `handleCreate(config)`, which closes the modal, dispatches, and routes to the created instance. Analytics `InstanceCreate { source }` distinguishes `CreationModal`, `CreationModalModpack`, `CreationModalModpackFile`, `CreationModalImport`, and `ProjectInstallModal`.
**Controls:** As listed; the primary button is disabled while the flow is incomplete.
**Persistence:** Created instances persist normally; the `n` quick-create shortcut and the generated icon name are transient.
**Rationale & notes:** Three choices are worth carrying forward. First, the **project search box is above the type choices**, not below — the fastest path for someone who already knows what they want is typing a name, and the four option buttons are the fallback. Second, **game versions are filtered by the chosen loader's manifest**, which is the difference between offering a user 200 versions of which 3 work and offering 3. Third, the modpack file path previews *before* committing and can refuse, which is the only place in the app where an upload is validated against Modrinth at all.

---

## LIB-30 — Empty library and the first-run call to action
**Type:** dialog
**Nav path:** `/` when no instance has ever been created
**Evidence:** [SOURCE `apps/app-frontend/src/pages/Index.vue:92`]; [SOURCE `apps/app-frontend/src/components/ui/WelcomeScreen.vue:1-138`]; [SOURCE `apps/app-frontend/src/locales/en-US/index.json` → `app.welcome-screen.*`]; [SOURCE `apps/app-frontend/src/providers/onboarding-checklist.ts`]
**Purpose:** A centred first-run screen replacing the library entirely (the library component is not even mounted). Copy:
- Title: `Welcome to Modrinth`
- Subtitle: `Ready to start playing?`
- Primary button: `+ Create an instance` — **disabled while offline**
- Hint beneath: `Press <kbd>N</kbd> to quick create an instance`
- Footer: `Escaping another launcher?` with an `Import from launcher` button — also **disabled while offline**

The `N` shortcut is registered in `onMounted` and removed in `onUnmounted`. It is carefully guarded: ignored on key repeat, with any of Cmd/Ctrl/Alt held, when the target is content-editable, or when the target is an `INPUT`, `TEXTAREA` or `SELECT`; and it is gated on being online. It is scoped to this screen — it is **not** a global shortcut, and the app has **no** global keyboard shortcut system at v0.21.9 (no ⌘K palette, no Ctrl+F).
The trigger is `isReady && !hasCreatedInstance` from the onboarding checklist, not "the instance list is empty". That distinction matters: a user who **deletes their last instance** does not get the welcome screen — they get an empty library with a working toolbar. The welcome screen is strictly a first-run artefact.
**Controls:** `Create an instance`; `Import from launcher`; `N`.
**Persistence:** `hasCreatedInstance` is tracked by the onboarding checklist provider, backed by app settings.
**Rationale & notes:** Two primary paths are offered deliberately — start fresh or migrate — because a launcher that cannot import existing instances has no path to a new user who already has a modpack. Giving them equal visual weight at the bottom of the screen is the honest way to do it. Disabling both buttons offline (rather than hiding them) preserves the layout and tells the user the action exists and *why* it is unavailable.

---

## LIB-31 — Empty-library state (post-first-run)
**Type:** indicator
**Nav path:** `/` with instances present but filtered to nothing
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/library/index.vue:272-277`]; [SOURCE `apps/app-frontend/src/components/ui/library/instance-group/index.vue:651-657`]; [SOURCE `apps/app-frontend/src/locales/en-US/index.json` → `app.library.search.no-results.title`, `app.library.group.empty`]
**Purpose:** Two distinct empty states, and the app is careful not to conflate them:
- **Search returned nothing** → a single line where the list would be: **`No instances match your search.`**
- **A custom group is empty** → an inline hint under the group header: **`Drag and drop to add instances.`**
- **Groups with no members are hidden entirely** when there is no search and no active filter, except for the synthetic `None`/`Ungrouped` section (`visibleInstanceGroups`).

There is **no** "you have no instances yet, create one" state outside the first-run welcome screen. A user who deletes everything lands on a library whose only visible things are the `Library` heading, the toolbar (search, `New group`, `New instance`, sort, group-by, filter) and an empty `Ungrouped` section carrying the `Drag and drop to add instances.` hint — which is a slightly odd instruction when there are no instances to drag.
**Operation:** n/a.
**Controls:** The toolbar remains fully functional.
**Persistence:** n/a.
**Rationale & notes:** Two things are right here and one is wrong. Right: hiding empty custom groups rather than showing a wall of empties, and remembering that state across sessions. Right: forcing groups open during a search. Wrong: the zero-instance state has no call to action. The fix is one button wired to the same `showCreationModal` injection the toolbar already uses.

---

## LIB-32 — Offline behaviour of the library
**Type:** indicator
**Nav path:** `/` with no network
**Evidence:** [SOURCE `apps/app-frontend/src/pages/Index.vue:56-62`]; [SOURCE `apps/app-frontend/src/App.vue:465-471`, `:2312-2320`]; [SOURCE `apps/app-frontend/src/components/ui/library/use-library.ts:353-382`, `:414-432`]; [SOURCE `apps/app-frontend/src/components/ui/library/instance-group/instance-card.vue:178-225`]; [SOURCE `apps/app-frontend/src/providers/setup/creation-modal.ts:47-50`]
**Purpose:** The library is almost entirely a **local-first** surface, because the instance list is SQLite, not the API. What works offline:

| Feature | Offline? | Why |
| --- | --- | --- |
| Instance list and cards | ✅ | local SQLite |
| Search, sort, filter, group, collapse, reorder | ✅ | pure client-side computation |
| **Play** | ✅ | `run()` reads the local instance directory; only asset downloads need the network, and those are separately reported |
| **Stop** | ✅ | process management |
| Group create / rename / delete / reorder / membership | ✅ | local SQLite writes |
| Drag & drop, multi-select, bulk delete | ✅ | as above |
| **Delete** | ✅ | filesystem + SQLite |
| **Duplicate** | ✅ | filesystem copy — `copy_dotminecraft_with_reporter` never touches the network |
| **Open folder**, **Copy path** | ✅ | local |
| **Edit icon** (select / create / edit / remove) | ✅ | local file operations |
| **Add content** | ❌ | routes to Browse, which needs the API (though with `must_revalidate` cache semantics it may still show stale results) |
| **Repair** | ❌ | re-downloads Minecraft + loader |
| **Instance type classification** | ⚠️ degraded | `get_project_v3_many` fails; the catch clears `serverProjectIds`, so every linked instance is classified as `Modpack` and the `Instance type` grouping is wrong |
| **New instance** | ❌ | left-rail `+` is `:disabled="offline"`; the welcome-screen button is too; `N` is suppressed |
| **Import from launcher** | ⚠️ | detection is filesystem-only, so it *would* work, but the welcome-screen button is disabled offline anyway |

There is **no offline banner, no offline state, and no queueing** anywhere in the library. The library simply does not change appearance when the network drops. This is correct design for a local-first surface and is the opposite of Browse, which has an explicit offline state (module 03, BR-40).

The catch is the `navigator.onLine` problem: per OQ-004 in `open-questions.md`, this API reports `false` inside the Tauri webview even when the network is fine. So in practice the app *disables the New-instance button* based on a signal that is wrong, while the library — which does not check — keeps working perfectly. The two halves of the same screen disagree about connectivity.
**Operation:** n/a.
**Controls:** n/a.
**Persistence:** n/a.
**Rationale & notes:** The instance-type misclassification (LIB-16) is the one genuine offline bug in this surface, and it is a consequence of a reasonable design choice — deriving "is this a server?" from the project API rather than from the instance's own `link.type`. The `link` already distinguishes `server_project` and `server_project_modpack` from `modrinth_modpack` (used elsewhere in the app, e.g. the instance-page share tab's `showShareTab`), so the classification could be made locally. **If we implement this, derive instance type from the instance's own link, not from a network round-trip.**

---

## LIB-33 — Quick-instance switcher (left rail)
**Type:** indicator
**Nav path:** n/a (a persistent rail strip below the nav buttons)
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/QuickInstanceSwitcher.vue:1-330`]; [SOURCE `apps/app-frontend/src/App.vue:2312-2320`]
**Purpose:** A second, faster path to the same instances. Instances are sorted by `max(last_played, created)` descending (name as a tiebreak), the first `visibleCount` are shown as 28 px avatar buttons linking to `/instance/<id>`, and a draggable divider below them expands the list (`QUICK_INSTANCE_LIMIT_MAX`, and the visible count itself is user-adjustable with ↑/↓). Instances whose `install_stage !== 'installed'` are dimmed (`brightness-[0.25] scale-[0.85]`) and carry a spinner overlay. A hover tooltip gives the instance name.
Its **context menu** (labelled `Instance actions`) is a deliberately reduced subset of the library's: `Stop` (when running) or `Play` (disabled when quarantined or not installed, with tooltip `This instance has been locked`), a divider, `View instance`, and `Open folder`. **No** favourite, add-content, duplicate, edit-icon, remove-from-group or delete — the rail is for launching, not managing.
The divider itself has tooltips `Drag to show recent instances` / (when at limit) `Drag to show fewer recent instances`.
**Operation:** Click → `/instance/<id>`; context menu as above.
**Controls:** Click, context menu, drag the divider.
**Persistence:** The visible-count limit is persisted via a dedicated composable.
**Rationale & notes:** Putting the *launch* affordance permanently in the chrome, with a *management* menu limited to read-only actions, is a clean separation: the rail is for "resume", the library is for "manage". The reduced menu also means a destructive action can never be one stray right-click away from a rail button.

---

## LIB-34 — Library page background context menu
**Type:** menu
**Nav path:** right-click the empty space of the Home tab
**Evidence:** [SOURCE `apps/app-frontend/src/pages/Index.vue:70-88`]; [SOURCE `apps/app-frontend/src/locales/en-US/index.json` → `app.library.actions.label`, `app.library.context-menu.create-instance`]
**Purpose:** A `ContextMenu` labelled `Library actions` with exactly **one** item: `New instance` (plus icon). It only opens when the right-click landed on an element carrying `data-library-page-background` — the page container or the group-list container — so right-clicking a card or a group header opens *their* menu instead.
**Operation:** `showCreationModal()` — the same injection the toolbar's button uses.
**Controls:** One item.
**Persistence:** None.
**Rationale & notes:** A one-item context menu is a deliberate "fill the dead space" move: right-clicking empty space in a list-based UI should do something. Guarding on `data-library-page-background` is the correct mechanism — the handler bails out rather than relying on z-order, so a card's own menu can never be shadowed by the page menu.

---

## LIB-35 — Virtualised grid and scroll restoration
**Type:** indicator
**Nav path:** n/a
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/library/instance-group/index.vue:123-162`]; [SOURCE `…/instance-group/index.vue:88-116`]; [SOURCE `apps/app-frontend/src/components/ui/library/index.vue:62-75`]; [SOURCE `apps/app-frontend/src/components/ui/library/view-state.ts:4`]
**Purpose:** The library renders **only the cards near the viewport**. `visibleInstances` computes the grid column count from the container width, then a card stride of `cardHeight + gap`, and slices `instanceGroup.instances` to `[first, end)` with a **±300 px overscan**, always additionally including the focused card (for keyboard navigation) and the dragged card (so a drag cannot drop its own source out of the DOM). The grid container has an explicit computed height so the scrollbar is correct even though only a window of children is mounted.
Consequences a re-implementation must match: cards are **absolutely positioned** at computed `left`/`top`; `Enter` and `Tab` navigation is hand-rolled (`handleCardTab` moves focus to the next card's first/last focusable when tabbing past the boundary) because the windowed list has no next sibling to focus; and a `ResizeObserver` measures the grid, debounced by 500 ms on mount (`INSTANCE_GRID_OBSERVER_ACTIVATION_DELAY`) so initial layout does not thrash.
Scroll position is stored in the `libraryScrollTop` singleton on route leave and restored via a double `requestAnimationFrame` after groups load, gated on `animationsReady` — restoring before the cards have mounted would land the user at the wrong offset.
**Controls:** None.
**Persistence:** `libraryScrollTop` is a module singleton, so it survives navigation within a session but not a restart.
**Rationale & notes:** The `focusedInstanceId` escape hatch in the virtualiser is the detail that makes keyboard use work at all: without always rendering the focused card, tabbing off the end of the window would move focus to nothing. Card move/enter animations are gated behind an `animationsReady` flag that flips one frame after restore, which is how the library avoids animating dozens of cards into place on every visit.

---

## Cross-cutting notes for implementers

1. **Manage from the library, launch from the rail.** The left-rail switcher is read-only plus launch/stop; every destructive or organising action lives in the library. Copy that split and neither surface becomes dangerous.
2. **Pinning is a group, not a flag.** `group:favorites` reuses group membership for pinning, which buys drag-and-drop, multi-select and the group picker for free — at the price of pinning being unavailable in the other grouping modes.
3. **Selection is keyed by `(groupId, instanceId)`.** In `Custom group` mode one instance renders once per group; an instance-ID key would make multi-group membership indistinguishable from duplication.
4. **Grouping modes come in a deliberate order with a deliberate default.** `Custom group` is the default because it is the only mode where the arrangement is durable. `Loader` has a hard-coded priority (fabric, forge, neoforge) that encodes the real-world distribution of loaders.
5. **`Last played` is the default sort and the rail uses the same key.** The app is optimised for resume, not browse.
6. **Library options are derived from the library's own contents.** Game-version and loader filter options come from the instances you have, not from the global tag list.
7. **Duplication has no dialog; deletion always does.** And the delete confirmation names the three things that are lost — worlds, configs, installed content.
8. **Export is not in the library.** It is on the instance page, gated on `!quarantined`.
9. **The import list is five launchers and nothing else.** No Technic. MultiMC/Prism import is the only path that can restore a Modrinth link, and only for Modrinth-managed packs.
10. **Offline is nearly a non-event here.** Everything except Repair, Add content and instance-type classification is local. The instance-type classification is the one thing that silently degrades, and it does so because it is derived from the network instead of from the instance's own link — do not make that mistake.