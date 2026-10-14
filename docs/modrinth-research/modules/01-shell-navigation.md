# 01 — Shell, chrome and navigation architecture

The permanent furniture of the Modrinth App: the titlebar it draws for itself,
the icon-only left rail, the account menu, the status indicators, and the window
behaviour underneath. Everything here is SOURCE unless tagged otherwise.

**Why this module is first.** The app has no native chrome
(`decorations: false`, `apps/app/tauri.conf.json:87`) and draws every part of its
own shell in one Vue file. `apps/app-frontend/src/App.vue` is the shell: its
template begins at `App.vue:2211`, and the rail, the titlebar, the sidebar, the
notification hosts and every global modal are mounted from it. Any work that
changes what the app *is* rather than what a page *shows* changes this file.

## Second observation pass (2026-10-14) — with an instance and with the network fixed

The first pass captured the shell without an instance and with the `Offline` false
positive active. A second pass (OQ-012) removed that and created instances, which
resolved several shell items the first pass could only mark `[ORCH-PENDING]`:

- **Connectivity chip** `[S-0093]`. With `GIO_USE_NETWORK_MONITOR` making the
  webview report online, the "Offline" chip **disappears entirely** and the titlebar
  shows only the running-instance pill. Confirms NAV-04 from the other direction:
  the chip is bound to `navigator.onLine` and nothing else.
- **Running-instance status pill** `[S-0093 S-0099]`. "No instances running" is the
  idle text; it stays a pill beside a live job chip `[S-0097]`, and the shell
  distinguishes "a task is running" (download chip) from "a game is running" (the
  pill) with two separate indicators.
- **Rail instance switcher** `[S-0116]`. With two instances the rail's switcher
  renders **one icon per instance** (a stack), so the rail is not a fixed eight
  slots — the switcher slot grows with the library. Any static "eight slots" count
  is a snapshot, not an invariant.
- **Task popover** `[S-0125]`. The titlebar download glyph is the job manager
  (module 09); the shell owns this, not any page.
- **Breadcrumb** `[S-0099 S-0119]`. Confirmed rendering "<instance> > <page>"
  ("Fabric 26.3 > Settings", "Discover mods > Sodium", "Fabric 26.3 > Discover
  mods"), which is the mechanism that makes an icon-only rail legible.

A **third pass** (2026-10-14, later; `RUNLOG.md`) closed most of what the second
left, and this time with a **window manager** running on the private display for
the parts that need one:

- **Back / forward** `[S-0127 S-0129]` — confirmed disabled on the first page and
  enabled once you have navigated in.
- **Search entry point** `[S-0144]` — the Browse search focuses and debounces.
- **Account menu** `[S-0142]` — the rail's bottom control is labelled "Sign into
  Modrinth"; activating it opens the Modrinth sign-in modal (module 07).
- **Window controls** `[S-0134]` and **close behaviour** — with openbox present,
  maximize fills the screen and the close control makes the whole app exit: **no
  close-to-tray and no lingering tray process** (verified by process count, not by
  pixels). `NAV-27` native decorations `[S-0149 S-0150]`: with
  `native_decorations = true` the app hides its **own** – ▢ × controls and lets the
  OS draw the frame.
- **The floor** `[S-0147]` — requesting 840×600 clamps to **1100×700**; at the
  floor nothing collapses, which is the whole point of a fixed minimum.

**Still `[ORCH-PENDING]`:** `NAV-28` drag-and-drop targets — canvas-level Xdnd
cannot be synthesised in this environment (no drag source, and `python-xlib` is
not installed), so the targets are documented from `file-drop.ts` only. Also
still pending: back/forward enablement across a deep-link *cold start*, and
breadcrumb overflow with a very deep trail.

**Coordinate model.** The shell is a CSS grid
(`App.vue:2651-2660`): row `status` (the titlebar) then row `nav` (the rail),
with the rail in a narrow `auto` column and an empty `1fr` filler beside it.
Three custom properties carry every measurement: `--top-bar-height: 3rem`,
`--left-bar-width: 4rem`, `--right-bar-width: 300px` (`App.vue:2646-2648`).
`--window-controls-width` is a fourth, set at runtime by `WindowControls.vue`.
The page content is a second, absolutely positioned grid inset by the first two
(`App.vue:2684-2702`). At 1280×800 that puts the rail from y=48 to y=800 at
x=0..64, and the content pane from (64, 48). The observed rail slot centres —
72, 124, 176, 228, 280 — reproduce exactly from a 48 px button, a 4 px gap and
an 8 px rail padding with no top padding (`App.vue:2262`), which confirms the
source order is the observed order and that no slot is conditionally hidden in
the capture.

## Coverage table

| ID | Label | Type | Evidence | Status |
| --- | --- | --- | --- | --- |
| NAV-01 | Titlebar shell / drag region | region | `App.vue:2393` | SOURCE |
| NAV-02 | Brand mark | indicator | `App.vue:2395` | SOURCE |
| NAV-03 | Back / forward | button | `App.vue:2397-2420` | `OBSERVED S-0127, S-0129` + SOURCE |
| NAV-04 | Breadcrumb trail | indicator | `Breadcrumbs.vue:18-56` | `OBSERVED S-0099, S-0119` + SOURCE |
| NAV-05 | Connectivity chip | indicator | `AppActionBar.vue:15-18` | `OBSERVED S-0001, S-0093, S-0151` + SOURCE |
| NAV-06 | Running-instance pill | indicator | `AppActionBar.vue:20-120` | `OBSERVED S-0001, S-0099` + SOURCE |
| NAV-07 | App-update pill | button | `app-update-button/index.vue:3-22` | SOURCE |
| NAV-08 | Download manager bar | indicator | `download-manager/index.vue:41-48` | `OBSERVED S-0097, S-0125` + SOURCE |
| NAV-09 | Sidebar toggle | button | `App.vue:2425-2434` | SOURCE |
| NAV-10 | Window controls | button | `WindowControls.vue:9-33` | `OBSERVED S-0134` (maximize) + SOURCE |
| NAV-11 | Rail — Home | tab | `App.vue:2264-2274` | `OBSERVED S-0093` + SOURCE |
| NAV-12 | Rail — Discover content | tab | `App.vue:2275-2284` | `OBSERVED S-0003, S-0114` + SOURCE |
| NAV-13 | Rail — Skin selector | tab | `App.vue:2285-2291` | `OBSERVED S-0004` + SOURCE |
| NAV-14 | Rail — Screenshots | tab | `App.vue:2292-2298` | `OBSERVED S-0005` + SOURCE |
| NAV-15 | Rail — Hosting | tab | `App.vue:2299-2310` | `OBSERVED S-0006` + SOURCE |
| NAV-16 | Rail — quick instance switcher | region | `QuickInstanceSwitcher.vue:292-360` | `OBSERVED S-0116` + SOURCE |
| NAV-17 | Rail — Create instance | button | `App.vue:2311-2321` | `OBSERVED S-0093, S-0094` + SOURCE |
| NAV-18 | Rail — Settings | button | `App.vue:2322-2327` | `OBSERVED S-0008` + SOURCE |
| NAV-19 | Rail — account menu | button | `App.vue:2328-2391` | `OBSERVED S-0142` (opens the Modrinth sign-in modal) + SOURCE |
| NAV-20 | Search entry points | region | `browse-tab/layout.vue:49-53` | `OBSERVED S-0144` + SOURCE |
| NAV-21 | Shortcut — `N` quick create | shortcut | `WelcomeScreen.vue:50-68` | `OBSERVED S-0094` + SOURCE |
| NAV-22 | Shortcut — macOS menu accelerators | shortcut | `macos/menu.rs:37-46` | SOURCE |
| NAV-23 | Shortcut — menu keyboard model | shortcut | `button-menu.ts:109-182` | SOURCE |
| NAV-24 | Notification surfaces | region | `App.vue:2588-2589` | `OBSERVED S-0015` + SOURCE |
| NAV-25 | Window size and state | region | `tauri.conf.json:73-89` | `OBSERVED` (1280×800 at 1100×700 floor, `S-0147`) + SOURCE |
| NAV-26 | Close behaviour, no tray | region | `main.rs:347-457` | `OBSERVED` (close exits the process; no tray) + SOURCE |
| NAV-27 | Native decorations toggle | region | `App.vue:819-823` | `OBSERVED S-0149, S-0150` + SOURCE |
| NAV-28 | Drag-and-drop targets | region | `file-drop.ts:1-70` | SOURCE + `[ORCH-PENDING]` |
| NAV-29 | Deep links and file open | region | `main.rs:176-195` | SOURCE |

---

## NAV-01 — Titlebar shell / drag region
**Type:** region
**Nav path:** always present; `App.vue:2393`
**Evidence:** [SOURCE apps/app-frontend/src/App.vue:2393]
**Purpose:** The app has no native titlebar (`decorations: false`,
`tauri.conf.json:87`), so it must provide both a window-drag surface and a home
for global controls. The titlebar is a single 48 px (`h-[--top-bar-height]`)
strip across the full window width; it is a sibling of the rail, not a child,
which is why the rail starts below it and the content pane starts at both
offsets. Everything in it is optional chrome that must survive a 1100 px window.
**Operation:** Nothing to click — it is dragged. The strip carries
`data-tauri-drag-region`, as do its left cluster, its right cluster and the
breadcrumb inner strip (`App.vue:2393-2394`, `App.vue:2424`,
`Breadcrumbs.vue:4`, `Breadcrumbs.vue:13`), so dragging empty titlebar moves the
window. Interactive children opt out with `data-tauri-drag-region-exclude`,
which `App.vue:2680-2682` maps to `-webkit-app-region: no-drag`; the window
controls use exactly that attribute (`WindowControls.vue:7`).
`[ORCH-PENDING]` Confirm by dragging from: the gap between the mark and the back
button; directly over the breadcrumb text; and the empty span to the left of the
"Offline" chip. Record whether all three move the window.
**Controls:** Full width of the window, 48 px tall, always `bg-raised`.
**Persistence:** n/a
**Rationale & notes:** The decision to opt individual children out of the drag
region rather than to paint a fixed-width drag strip is the right one — it means
every pixel not claimed by a control is draggable, so there is no dead zone. The
cost is that the breadcrumb trail is *inside* the drag region
(`Breadcrumbs.vue:4`), meaning a breadcrumb you are trying to select by pressing
can start a window drag instead. That is a real, findable conflict between two
features sharing 48 px.

## NAV-02 — Brand mark
**Type:** indicator
**Nav path:** `App.vue:2395`
**Evidence:** [SOURCE apps/app-frontend/src/App.vue:2395]
**Purpose:** Identity, and a fixed left anchor so the eye can find the chrome
again after the breadcrumbs change length on every navigation. It is also the
one element in the titlebar that is guaranteed present on every route.
**Operation:** None. Rendered as a wordmark component with `pointer-events-none`
applied, so it is inert even though it sits inside the drag region — you can
drag the window by grabbing the logo.
**Controls:** n/a
**Persistence:** n/a
**Rationale & notes:** Making it `pointer-events-none` is a deliberate detail
worth copying: without it, the drag region would be broken over the single
largest opaque element in the bar. Note it is *not* a link — there is no
"click the logo to go home" affordance, which is a small missed opportunity given
that it occupies the most recognisable position in the chrome.

## NAV-03 — Back / forward
**Type:** button
**Nav path:** `App.vue:2397-2420`
**Evidence:** [SOURCE apps/app-frontend/src/App.vue:2397]
**Purpose:** Session history inside the app. Needed because navigation is not a
tree — the same `/browse/:projectType` route is re-entered with different
query parameters, and a user who drilled into a filter expects to back out of
it. Reach for it constantly while exploring; almost never while playing.
**Operation:**
1. Press the left chevron to `router.back()`; the right chevron calls
   `router.forward()` (`App.vue:2402`, `App.vue:2414`).
2. Each button is `disabled` unless the corresponding history entry exists
   (`App.vue:2401`, `App.vue:2413`).
3. Availability is derived from the *browser* history state, not from Vue
   Router: `historyState?.back != null` / `historyState?.forward != null`
   (`App.vue:225-229`), refreshed in `router.afterEach` (`App.vue:983`).
4. Disabled chevrons are dimmed to 20 % opacity (`App.vue:2406`, `App.vue:2418`).
`[ORCH-PENDING]` Check whether the buttons enable when arriving via a deep link
or a second-instance launch — those paths push history outside a normal click
and `App.vue:983` may never run for them.
**Controls:** Two fixed-size buttons, 28×28 px (`!h-7 !min-w-7 !w-7`,
`App.vue:2400`), outlined, 4 px apart, sitting 8 px to the right of the mark.
**Persistence:** n/a
**Rationale & notes:** Two things stand out. First, the size mismatch: these are
28 px tall while every rail button is 48×48 (`NavButton.vue:13`) and every
window control is 34 px tall by widened hit area. The navigation controls — the
most frequently pressed chrome in the app — are the smallest targets in it.
Second, reading `window.history.state` rather than Router's own
`router.options.history.state` is fragile in principle, though it happens to
work because Vue Router's HTML5 history writes the same `back`/`forward` keys.
The dimming is applied twice (disabled attribute *and* opacity), which is
belt-and-braces for a control the user is meant to stop noticing.

## NAV-04 — Breadcrumb trail
**Type:** indicator
**Nav path:** `App.vue:2422` → `apps/app-frontend/src/components/ui/Breadcrumbs.vue`
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/Breadcrumbs.vue:18]
**Purpose:** Answers "where am I, and what is above me?" in one line, and turns
every ancestor into a jump target. This matters more here than on the web
because the app has two independent navigation axes: the left rail selects a
*top-level destination*, while the actual view is a deep route with query
parameters (`/browse/modpack?i=<instance>&sid=<server>`), so the rail alone
cannot describe where you are. Reached for constantly, read constantly.
**Operation:**
1. Pages declare breadcrumbs imperatively. A page calls `useRootBreadcrumb()` to
   replace the whole trail, or `useBreadcrumb()` to push onto it
   (`providers/breadcrumbs.ts:193-208`).
2. The trail lives in a stack behind a `shallowRef`
   (`providers/breadcrumbs.ts:80`); `activate` splices an entry in after its
   parent, `resetTo` truncates to a single entry
   (`providers/breadcrumbs.ts:99-117`).
3. Each entry carries `id`, `label`, optional `to`, and optional `visual`
   (`providers/breadcrumbs.ts:26-32`). `id` is watched with `flush: 'sync'` and
   re-activates the entry when it changes (`providers/breadcrumbs.ts:185-190`),
   which is how a lazy-loaded label ("Loading…") swaps for a real one in place.
4. All but the last crumb render as `RouterLink`; the last is a `span` with
   `aria-current="page"` and `select-none` (`Breadcrumbs.vue:20-29`).
5. When the trail is wider than its container, a `ResizeObserver` detects the
   overflow (`Breadcrumbs.vue:81-90`, `116-121`), a mask fades both ends
   (`Breadcrumbs.vue:133-141`), and hovering runs a 10 s scroll animation
   (`Breadcrumbs.vue:92-112`, `143-158`).
`[ORCH-PENDING]` Trigger the overflow with a deep instance + server + browse
chain (`?i=` and `?sid=` together) and record whether the auto-scroll actually
runs or is skipped, and whether the faded edges still accept clicks.
**Controls:** Up to N crumbs; typical shapes are registered in exactly nine
places: `pages/Index.vue:48`, `pages/Screenshots.vue:15`, `pages/Skins.vue:60`,
`pages/Servers.vue:16`, `pages/instance/layout.vue:347`,
`pages/hosting/manage/Index.vue:123`, `pages/project/Index.vue:395`,
`pages/project/Version.vue:154`, `pages/User.vue:173`, plus the conditional
synthesised roots in `pages/Browse.vue:230-249`. A crumb's visual is either a
20 px circular image or a 20 px icon (`Breadcrumbs.vue:31-48`).
**Persistence:** n/a — the stack is in memory and resets on navigation to a page
that calls `useRootBreadcrumb`.
**Rationale & notes:** This is the strongest idea in the shell and the one most
worth copying. `Browse.vue:230-247` is the proof: it *rewrites the root* to be
the instance or the server when either is in the query string, so
`MyInstance › Discover mods` is a correct description of a screen whose URL still
begins `/browse`. A declarative route-name breadcrumb could not express that.
The hover-to-scroll overflow treatment is a genuinely thoughtful answer to "the
titlebar has 48 px and a user name can be forty characters". The weakness is
that the trail is the only part of the shell that can grow, and it is growing
into the drag region.

## NAV-05 — Connectivity chip
**Type:** indicator
**Nav path:** `AppActionBar.vue:15-18`
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/AppActionBar.vue:15]
**Purpose:** Tells the user the app has lost the network before they act on
stale data. Appears only when disconnected; sits left of the running-instance
pill in the titlebar's right cluster.
**Operation:** Reactive only — initialised from `!navigator.onLine` and flipped
by the window `online`/`offline` events (`AppActionBar.vue:283-294`, and
identically at `App.vue:465-471`). Not clickable and not a toggle.
`[ORCH-PENDING]` Already characterised in `open-questions.md` OQ-004 as a false
positive inside a Tauri webview; see also NAV-28 in this module for the
consequence for drag-and-drop.
**Controls:** A small unplug glyph plus the word "Offline"; whole group is
`v-if`-gated on `offline`.
**Persistence:** n/a
**Rationale & notes:** This is a defect the orchestrator already logged, and it
is worth restating here because of what it does to the shell's credibility: it
sits directly beside a working "No instances running" pill, so the two look like
the same class of assertion and a user who trusts one has to distrust the other.
Deriving connectivity from a browser-level API inside a native shell is the root
cause. Our rewrite must source this from the same layer that performs the
requests.

## NAV-06 — Running-instance pill
**Type:** indicator
**Nav path:** `AppActionBar.vue:20-120`
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/AppActionBar.vue:20]
**Purpose:** The launcher is a background process: instances run while the user
keeps using the app. The pill is the always-visible proof that Minecraft is
running, and it is also the fastest control surface for the three things a user
does while a game is running — see it, stop it, read its logs.
**Operation:**
1. Idle: a grey dot plus "No instances running" (`AppActionBar.vue:116-119`).
2. Running: a pulsing-dot glyph, the **primary** instance's name as a router
   link to `/instance/<id>` (`AppActionBar.vue:26-32`), then stop and
   view-logs buttons (`AppActionBar.vue:101-114`).
3. If more than one instance is running, a disclosure chevron opens a popper
   listing every process, each with make-primary, stop and view-logs
   (`AppActionBar.vue:33-99`).
4. State refreshes on the backend `process` app event (`AppActionBar.vue:296-298`),
   not on a timer.
`[ORCH-PENDING]` Launch two instances at once and confirm the popper, the star
on the primary, and that stopping from the popper leaves the pill correct.
**Controls:** Exactly one primary instance is the focus of the pill
(`AppActionBar.vue:276-278`); the choice is local and resets to the first
returned process.
**Persistence:** None for the primary choice; running processes come from the
backend (`helpers/process` `get_all`).
**Rationale & notes:** This is the best-designed element in the titlebar: it
degrades from a status noun to a status *plus controls* without changing
footprint much, and it never opens a modal for something as reversible as
stopping. The weakness is that with several instances running the pill can
present only one name, so the disclosure chevron — an unlabelled 24 px control
— becomes load-bearing. Also worth noting: `messages.viewLogs` sends the user to
a *route* (`AppActionBar.vue:315-321`) rather than opening a log tail, which is
the correct choice for a webview and the wrong one if you wanted logs while the
game was being played.

## NAV-07 — App-update pill
**Type:** button
**Nav path:** `app-update-button/index.vue:3-22`
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/app-update-button/index.vue:3]
**Purpose:** A desktop app that never tells you it updated will not be updated.
This pill is the single point where an available app update becomes actionable
in the shell rather than in a modal.
**Operation:**
1. Visible only when an update is known **and** either it is already downloaded
   or the network is metered (`app-update-button/index.vue:68-70`).
2. Label cycles "Update" → "Downloading update {n}%" → "Reload to update"
   (`app-update-button/index.vue:72-82`).
3. Click downloads, or installs if already downloaded
   (`app-update-button/index.vue:106-116`); installing triggers a restart
   through `setRestartAfterPendingUpdate` and a forced window close
   (`App.vue:2096-2109`).
`[ORCH-PENDING]` Confirm the empty state: on a normal, unmetered, up-to-date
install the pill should be absent entirely, leaving no trace that updates exist.
**Controls:** Outlined brand button, 34 px tall, `aria-busy` while downloading.
**Persistence:** `settings.pending_update_toast_for_version` carries the
"you updated, here is the changelog" toast across restarts
(`App.vue:923-930`; field declared at `packages/app-lib/src/state/settings.rs:82`).
**Rationale & notes:** Gating on *metered* rather than on *available* is a
deliberate politeness: it never spends the user's bandwidth silently, but it
also means the pill is absent in the common case (unmetered + updating
automatically), so the update path is invisible to most users. A small,
under-emphasised "what version am I on" affordance does exist — the Settings
modal footer — but nothing surfaces it from the shell.

## NAV-08 — Download manager bar
**Type:** indicator
**Nav path:** `download-manager/index.vue:76-120`, inside `AppActionBar.vue:3`
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/download-manager/index.vue:41]
**Purpose:** Installs take minutes and the user will navigate away from the page
that started them. The bar is a single persistent, shell-level commitment that
transfers never silently stop because the page was closed.
**Operation:**
1. Renders only when `shown || selectedJob || activeJobs || attentionJobs ||
   completedJobs` is non-empty (`download-manager/index.vue:41-48`); with no
   transfers it is removed from the DOM entirely.
2. A `FloatingMenu` trigger bar opens a panel listing active, attention and
   completed jobs with retry / cancel / pause / dismiss / clear
   (`download-manager/index.vue:101-118`).
3. `Esc` inside the panel closes it and returns focus to the bar
   (`download-manager/index.vue:116`).
4. State lives in a separate `PopupNotification` of `type: 'download'`
   fed by the backend `loading` event (`AppActionBar.vue:503-505`).
`[ORCH-PENDING]` Start a long install, navigate away, and confirm the bar
survives; then check the idle case — does the bar vanish or shrink to a circle?
**Controls:** Width animates between an idle circle (`2rem + 2px`) and a capped
expanded form (`min(24rem, 40 % of window width)`, `download-manager-bar.vue:139-159`);
it is reduced to a 34 px pill carrying only an icon, a job count badge and an
attention glyph when no job is selected (`download-manager-bar.vue:154`).
Appearance changes by state: green complete, orange failed/interrupted, plain
running or paused (`download-manager-bar.vue:49-72`).
**Persistence:** In-memory only. Jobs survive page navigation because the shell
outlives the page, not because anything is written down.
**Rationale & notes:** The animation budget here is remarkable — measured text
widths, a spring resize, a pulse on the count badge, a scaleX compression when
it collapses — and all of it is disabled under
`prefers-reduced-motion` (`download-manager-bar.vue:47`,
`415-419`). That is the right call and it is consistently applied across this
component family. The design weakness is discoverability: a transfer that has
finished and been dismissed looks identical to one that never existed, and the
completed-job state occupies no persistent home.

## NAV-09 — Sidebar toggle
**Type:** button
**Nav path:** `App.vue:2425-2434`
**Evidence:** [SOURCE apps/app-frontend/src/App.vue:2425]
**Purpose:** The 300 px right sidebar carries accounts, friends, news and an
advert. Hiding it reclaims a third of the width for content. It is a session
comfort toggle, not a preference — but note the coupling below.
**Operation:**
1. Click flips `sidebarToggled` (`App.vue:2431`); the glyph rotates 180° when
   hidden (`App.vue:2430`).
2. Rendered **only** when `!forceSidebar && appSettings.toggleSidebar`
   (`App.vue:2426`). `forceSidebar` is true on `/browse`, `/project` and `/user`
   (`App.vue:266-271`), so on those routes the sidebar is mandatory and the
   toggle disappears entirely.
3. The final visible state is `sidebarToggled || forceSidebar`
   (`App.vue:272`).
`[ORCH-PENDING]` Confirm the chevron is genuinely absent (not merely disabled)
on `/browse/modpack`, and that the content pane's right inset returns to 0 there.
**Controls:** A single arrow glyph that rotates; `type` switches between `base`
and `quiet` to signal state (`App.vue:2427`).
**Persistence:** None of its own. Its *visibility* is driven by
`settings.toggle_sidebar` (default `false`,
`composables/use-app-settings.ts:53`), so on a default install the user never
sees this control at all until they enable "Toggle sidebar" in Settings →
Appearance.
**Rationale & notes:** This is the shell's worst interaction. A control that
only exists once you have found and enabled it in Settings is not a control. The
`toggle_sidebar` setting is really a "may the user hide the sidebar" capability
flag that was wired to a toggle's *visibility* rather than to its default state
— the natural reading is the reverse. The `forceSidebar` override for
`/browse`, `/project` and `/user` is itself worth questioning: those are exactly
the pages whose sidebar content (search filters, project metadata) is load-bearing,
so the override is defensible, but it means the toggle is a two-state control in
one third of the app.

## NAV-10 — Window controls
**Type:** button
**Nav path:** `WindowControls.vue:3-34`
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/WindowControls.vue:9]
**Purpose:** Because the window is undecorated, the app owes the user minimise,
maximise and close. They are also the only place the app can hand its own
titlebar real estate back to the platform.
**Operation:**
1. Minimize → `getCurrentWindow().minimize()` (`WindowControls.vue:13`);
   maximize → `toggleMaximize()` (`WindowControls.vue:21`), glyph swapping
   between "maximise" and "restore" on resize (`WindowControls.vue:22-24`,
   `96-98`); close → `handleClose` (`WindowControls.vue:110-113`).
2. Close first persists window state, then closes (`WindowControls.vue:111-112`,
   mirrored at `App.vue:948-951`).
3. The whole cluster is excluded from the drag region via
   `data-tauri-drag-region-exclude` (`WindowControls.vue:7`).
4. A `ResizeObserver` writes the cluster's measured width into
   `--window-controls-width` on the document element
   (`WindowControls.vue:64-82`), which the titlebar consumes as
   `padding-right` (`App.vue:2675`). This is how the shell avoids laying content
   under the buttons without hard-coding their size.
`[ORCH-PENDING]` Measure the real hit rectangles — the CSS widens each button by
about 9 px vertically and 6–9 px horizontally via a `::before` pseudo-element
(`WindowControls.vue:116-124`), which is invisible in a screenshot. Confirm the
close button turns red on hover/focus and that the cluster never overlaps the
breadcrumb at 1100 px.
**Controls:** Three quiet icon buttons; close turns red on hover and focus
(`WindowControls.vue:29`). Labels are **hard-coded English strings**, not i18n
keys — "Minimize window", "Toggle maximize window", "Close window"
(`WindowControls.vue:11`, `19`, `28`) — so these three control names are the only
chrome in the app that a non-English user cannot read.
**Persistence:** Position, size, maximised state (and on Windows, decorations)
persist via `tauri-plugin-window-state` into `app-window-state.json`
(`main.rs:242-256`).
**Rationale & notes:** The `--window-controls-width` round-trip is the correct
architectural answer to a custom titlebar and is worth copying verbatim: measure
the platform-specific cluster, publish it as a CSS variable, let layout consume
it. Mac is handled by *not* drawing the cluster at all
(`WindowControls.vue:58-62`) and instead padding the titlebar's left edge by
5 rem to clear the traffic lights (`App.vue:2869-2873`, toggled at
`App.vue:505-507`). The hard-coded English labels are a straightforward miss —
every other string in the shell is a `defineMessages` entry.

## NAV-11 — Rail — Home
**Type:** tab
**Nav path:** `/` (`routes.js:8-12`) → `pages/Index.vue`
**Evidence:** [SOURCE apps/app-frontend/src/App.vue:2264]
**Purpose:** The app's front door: the library, the onboarding checklist, and the
recents list. It is the destination you return to when you have lost track of
where you were. Used on every session start and constantly thereafter.
**Operation:** Click, or press the `N` shortcut (NAV-21). Active state is exact
(`route.path === '/'`); it also shows a *subpage* highlight when you are in a
browse/project view scoped to an instance (`route.query.i`, `App.vue:2268-2271`).
**Controls:** One slot, always present, no condition. Tooltip
`app.nav.home` = "Home" (`App.vue:645-648`).
**Persistence:** n/a
**Rationale & notes:** The glyph is a solid right-pointing play triangle with a
half-pixel left optical nudge (`ml-0.5`, `App.vue:2273`) — it reads as "launch"
rather than "home", which is deliberate for a launcher but is a genuine
disambiguation cost: "Home" and "Play" are the two most confusable actions in the
product, and they are separated only by hover text. The subpage highlight
(`NavButton.vue:90-92`, a filled button background rather than the selected
circle) is the one place the rail encodes a second level of state, and it works
well.

## NAV-12 — Rail — Discover content
**Type:** tab
**Nav path:** `/browse/modpack` (`routes.js:50-54`) → `pages/Browse.vue`
**Evidence:** [SOURCE apps/app-frontend/src/App.vue:2275]
**Purpose:** The content marketplace, entered at the modpacks tab because that is
what most people want. It is also the entry point for instance-scoped browsing —
`/browse/mod?i=<instance>` is how you add a mod to one specific game — which is
why it carries both a primary and a subpage highlight.
**Operation:** Click. Primary when `/browse*` and neither `i` nor `sid` is in the
query; subpage when `/project*` with neither (`App.vue:2278-2281`). The project
type is a route parameter, so switching types is a route push, not a tab state.
**Controls:** One slot, always present. Tooltip `label.discover-content` =
"Discover content" (`packages/ui/src/utils/common-messages.ts:119-122`).
**Persistence:** Filters and search are persisted into the URL query
(`persistentQueryParams: ['i','ai','shi','sid','wid','from']`,
`pages/Browse.vue:1157`), which is what makes a filtered browse view shareable.
**Rationale & notes:** Putting a compass glyph here is the clearest mapping in
the rail — the tool *is* discovery, and it is the only destination where the
metaphor and the function coincide. The subpage highlighting of `/project/*` is
the interesting part: project pages are reached from browse, from a user profile,
and from a deep link, and lighting up "Discover" for all three teaches the user
that a project page is a mode of browsing rather than a separate place. That is
good IA. The failure is that `/user/*` gets no such treatment even though a user
profile is reached from the same places.

## NAV-13 — Rail — Skin selector
**Type:** tab
**Nav path:** `/skins` (`routes.js:55-59`) → `pages/Skins.vue`
**Evidence:** [SOURCE apps/app-frontend/src/App.vue:2285]
**Purpose:** A profile/cosmetics surface: change the skin your Minecraft account
wears. Reached occasionally — typically before launching — not per session.
**Operation:** Click. No active/subpage highlighting at all, so the rail shows
nothing to indicate you are on it beyond hover.
**Controls:** **Conditionally present**: `v-if="appSettings.showSkinSelectorInSidebar"`
(`App.vue:2286`), i.e. `settings.show_skin_selector_in_sidebar`, which defaults
to `true` (`composables/use-app-settings.ts:57`) and can be synced from the
account's behaviour preferences (`App.vue:1305-1320`). Tooltip
`app.skins.title` = "Skin selector" (`apps/app-frontend/src/utils/app-messages.ts:3-6`).
**Persistence:** The visibility toggle, above. The skin selection itself is a
Minecraft-account property, not app state.
**Rationale & notes:** Answering the brief directly: **no, Skins is not always
present.** It is one of only two rail slots that can disappear, and it is the
only one whose visibility is a plain user preference — the kind of preference
that belongs in a menu, not in the permanent navigation. The glyph is a garment
silhouette, which is more literal than the rail's other icons and reads well but
does not say "Minecraft cosmetics". Its total lack of active-state styling is
the clearest gap in the rail: every other destination tells you where you are,
this one does not.

## NAV-14 — Rail — Screenshots
**Type:** tab
**Nav path:** `/screenshots` (`routes.js:60-64`) → `pages/Screenshots.vue`
**Evidence:** [SOURCE apps/app-frontend/src/App.vue:2292]
**Purpose:** A single gallery of every screenshot the app has captured across
instances. Low frequency — mostly used to delete or share, or to check a capture
happened.
**Operation:** Click. No active/subpage highlighting.
**Controls:** **Conditionally present**, and this is the most surprising
condition in the shell: `v-if="globalSyncedOptionsQuery.data.value?.screenshots"`
(`App.vue:2293`). That is `GlobalSyncedOptions.screenshots` — whether the
*screenshot-folder syncing* capability is enabled globally for synced instances
(`packages/app-lib/src/api/instance/synced_options/orchestration.rs:55-65`),
which is itself account-synced via `behavior.show_all_screenshots`
(`App.vue:1325-1338`). Tooltip `app.nav.screenshots` = "Screenshots"
(`App.vue:653-656`).
**Persistence:** `set_global_synced_option('screenshots', …)`, global across
instances, and account-synced.
**Rationale & notes:** **No, Screenshots is not always present**, and this
condition is almost certainly a bug in the information architecture rather than a
decision. The rail item's visibility is tied to an *instance-synchronisation*
capability, so a user who has never enabled screenshot syncing has no way to
reach the screenshot gallery at all — even if screenshots exist on disk. The
converse is also true: a user who *has* enabled syncing sees a gallery entry in
their navigation for a reason they did not intend. Two unrelated settings
("show the screenshots page" and "sync screenshots between instances") are
conflated into one `v-if`. This is the single clearest IA defect in the shell.

## NAV-15 — Rail — Hosting / Servers
**Type:** tab
**Nav path:** `/hosting/manage` (`routes.js:13-49`) → `pages/Servers.vue`
**Evidence:** [SOURCE apps/app-frontend/src/App.vue:2299]
**Purpose:** The paid hosted-server product: list your servers, manage content,
files, backups and access. Reached on a schedule (check a server, restart it,
read a log), not per session.
**Operation:** Click. Primary highlight on `/hosting/manage` exactly
(`App.vue:2302`); subpage highlight when inside a specific server, or when in a
browse/project view scoped to a server via `?sid=` (`App.vue:2303-2307`).
**Controls:** One slot, **always present — there is no entitlement gate**. Tooltip
`app.nav.modrinth-hosting` = "Modrinth Hosting" (`App.vue:649-652`).
**Persistence:** n/a
**Rationale & notes:** Answering the brief: **Hosting is always present**, with
no feature flag and no subscription check in the rail at all. That is a
deliberate and defensible choice for a paid product — you cannot gate a
destination behind a paywall the user may already have bought, and showing it to
non-subscribers is how an upsell surface gets to exist at all (the sidebar
carries the "Upgrade to Modrinth+" link at `App.vue:2574-2583`). The glyph is a
stack of horizontal slabs, which is the most literal icon in the rail. It is also
the only slot whose highlight has to reason about three different route shapes at
once (`App.vue:2302-2307`), which is the visible cost of a page that is reachable
both directly and as a context for browsing.

## NAV-16 — Rail — quick instance switcher
**Type:** region
**Nav path:** injected into the rail between Hosting and Create instance
(`App.vue:2311-2321`)
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/QuickInstanceSwitcher.vue:293]
**Purpose:** A per-instance strip — one 48 px avatar per recently used instance,
each a direct link — placed in the rail because instances are the app's primary
object and the deepest level of its hierarchy. This is what turns the rail from
a seven-item menu into a two-level navigator.
**Operation:**
1. Instances are sorted by the later of `last_played` and `created`, then by name
   (`QuickInstanceSwitcher.vue:42-59`).
2. How many appear is `min(userLimit, min(containerHeight-derived max,
   instanceCount, QUICK_INSTANCE_LIMIT_MAX))`
   (`QuickInstanceSwitcher.vue:63-69`).
3. The count is set by **dragging a divider** up or down, or with arrow keys
   when it is focused (`QuickInstanceSwitcher.vue:329-345`, `96-105`). It
   overshoots with a red flash to signal the limit (`QuickInstanceSwitcher.vue:120-129`).
4. Right-click an avatar for a context menu: Play / Stop, View instance, Open
   folder (`QuickInstanceSwitcher.vue:255-289`).
5. Instances still installing are dimmed and carry a spinner
   (`QuickInstanceSwitcher.vue:316-324`).
`[ORCH-PENDING]` Create six or more instances and verify the divider drag
resizes the strip; check whether the chosen count survives a restart.
**Controls:** A 1 px horizontal rule that is the drag handle, present only when
at least one instance exists (`QuickInstanceSwitcher.vue:296-300`,
`329-356`); the whole block is the `flex-1` spacer that pushes Settings and the
account button to the bottom of the rail.
**Persistence:** The visible count is `useQuickInstanceLimit`, which reads
`behavior.quick_instance_count` from the account's synced behaviour preferences
(`App.vue:1321-1323`), so it follows the account across devices.
**Rationale & notes:** This is the most original idea in the rail and the one
most worth stealing: it puts a *second level* of navigation in the same
48 px column without any hierarchical indentation, because the divider makes the
boundary between "destinations" and "objects" explicit and draggable. The
drag-resize is a genuinely good affordance — it hands the user a scrollbar for
their own navigation, with keyboard support and a red overshoot state. Its cost
is that the rail's information density varies with account size, and a rail with
fourteen avatars looks nothing like a rail with one. The `flex-1` wrapper is also
load-bearing for layout: it is what pushes Settings to y≈716 and the account to
y≈768 at the default window size.

## NAV-17 — Rail — Create instance
**Type:** button
**Nav path:** opens `CreationFlowModal` (`App.vue:2313-2319`) — a modal, not a route
**Evidence:** [SOURCE apps/app-frontend/src/App.vue:2313]
**Purpose:** The app's primary creation verb, promoted out of the sidebar
(onboarding checklist, welcome screen) to a rail slot because it is the one
thing a new user must be able to find. Used at first run and then whenever a new
game is wanted.
**Operation:**
1. Click, or press `N` while the Home page is mounted (`App.vue:2315-2316`).
2. It is `disabled` when offline (`App.vue:2316`) — creation needs the catalogue.
`[ORCH-PENDING]` The rail button's disabled styling while offline: the glyph
should visibly grey, and the tooltip still reads "Create new instance". Confirm
whether a disabled-looking button still announces itself.
**Controls:** One slot, always present, rendered inside the switcher's footer so
it sits directly below the instance list with the drag handle between them
(`QuickInstanceSwitcher.vue:357-359`). Tooltip `app.nav.create-new-instance` =
"Create new instance" (`App.vue:657-660`).
**Persistence:** n/a
**Rationale & notes:** A creation action living in the *navigation* rail is a
deliberate IA choice: it makes "new" a first-class destination rather than a
button on a page. It is also the one slot that does not navigate — it opens a
modal — so the rail is not purely a router, which slightly undercuts the mental
model the rail otherwise teaches. The offline disable is correct behaviour
expressed in the wrong place: `navigator.onLine` inside a Tauri webview is
unreliable (NAV-05), so this control can be disabled while the network is fine.

## NAV-18 — Rail — Settings
**Type:** button
**Nav path:** opens `AppSettingsModal` (`App.vue:2322-2327`)
**Evidence:** [SOURCE apps/app-frontend/src/App.vue:2322]
**Purpose:** Entry to every preference, an update action, and the
developer-mode hatch. Used rarely — a handful of times per user lifetime
permanently, plus whatever the onboarding pushes people toward.
**Operation:**
1. Click, or the five-click logo tap in the modal footer while it is already
   open (`AppSettingsModal.vue:299-306`, `366-377`).
2. Opens a `TabbedModal` (`AppSettingsModal.vue:328-335`) whose tabs are grouped
   into three sidebar categories — Display, Account, Instances — with 11 tabs
   total (`AppSettingsModal.vue:80-173`).
3. `FeatureFlagSettings` is `developerOnly` and hidden unless
   `settings.developer_mode` (`AppSettingsModal.vue:118-124`, `175-177`).
`[ORCH-PENDING]` Open Settings and record whether the modal traps focus, whether
`Esc` closes it, and whether the unsaved-changes bar appears on any tab.
**Controls:** Always present. Tooltip `label.settings` = "Settings"
(`packages/ui/src/utils/common-messages.ts:331-334`).
**Persistence:** Everything inside; `developer_mode` flips the tab list and can
also be reached from the account menu (`App.vue:1567-1573`).
**Rationale & notes:** **Settings is a modal, not a tab, and that is
justifiable.** A route would be simpler and would give it a shareable URL and a
history entry — but it would also put a permanent destination into a rail whose
entire value is being short and about *playing*. Worse, a settings route is
exactly the kind of thing users leave open in a background tab; as a modal it
cannot accumulate. The cost is real though: the dialog has no URL, so its state
is not linkable, browser-back does not dismiss it, and it has to reimplement
things a route gets for free. `TabbedModal` mitigates this well — it supports
`:before-hide` and `:before-tab-change` guards
(`AppSettingsModal.vue:332-333`) which block navigation when there are unsaved
changes, exactly the protection a route would have needed a `beforeRouteLeave`
for. One genuine defect: the sidebar-toggle bug (NAV-09) means the Discover,
Profile and Project pages permanently reserve a right-hand region whose content
the user may have wanted gone.

## NAV-19 — Rail — account menu
**Type:** button
**Nav path:** bottom of the left rail, `App.vue:2328-2391`
**Evidence:** [SOURCE apps/app-frontend/src/App.vue:2328]
**Purpose:** Identity and session management in one place: view your profile,
upgrade, add friends, switch accounts, sign out. It is also the app's only
sign-in affordance, which is why it has four visual states rather than one.
**Operation:**
1. **Loading** (`credentials === undefined`): a disabled, non-interactive
   spinner; a tooltip still names it "Loading profile…"
   (`App.vue:2328-2338`, label from `App.vue:1457-1461`).
2. **Signed in**: a `TeleportOverflowMenu` showing the avatar
   (`App.vue:2339-2367`) with options — View profile, Upgrade to Modrinth+ (only
   if not already Plus), Add a friend, Feature flags (only in developer mode),
   a divider, Switch account (submenu), Sign out
   (`App.vue:1544-1591`).
3. **Signed out but accounts remembered**: a branded sign-in glyph opening the
   account *switcher* instead (`App.vue:2368-2388`), listing stored accounts
   with role icons and per-account remove, plus "Add account"
   (`App.vue:1463-1485`).
4. **Signed out, nothing stored**: a plain sign-in button calling
   `requestSignIn()` (`App.vue:2389-2391`).
`[ORCH-PENDING]` The orchestrator's capture lists the account avatar among the
*titlebar* elements; in source it is the last element of the **left rail**
(`App.vue:2328-2391` sits inside the rail container that closes at
`App.vue:2392`, while the titlebar container opens at `App.vue:2393`). Worth a
pixel confirmation at 1280×800 so the two readings are reconciled.
**Controls:** Always the bottom-most rail slot; 32 px circular avatar, or a
32 px glyph. Placement is `right-end` so menus open over the content area.
**Persistence:** `mr_auth` credentials, `getAll`/`setActive`/`removeUser`
(`App.vue:143-144`). Switching account persists the target account's theme
first, then reloads the window (`App.vue:1511-1531`, `1499-1509`).
**Rationale & notes:** Four states for one slot is the price of supporting
multiple accounts, and the design handles it well by keeping the *menu content*
and the *slot glyph* independently derived. Two things are wrong. First, the
four-way shape is not discoverable: the signed-out-with-stored-accounts state is
a sign-in glyph that opens an account *picker*, which a user who has just signed
out will read as "sign in again" and be confused when handed a list. Second,
switching account calls `window.location.reload()` (`App.vue:1504`), which
destroys the single-instance window and every in-flight shell indicator — a full
reload for what is conceptually a re-authentication.

## NAV-20 — Search entry points
**Type:** region
**Nav path:** `/browse/:projectType` — search field in the browse layout header
**Evidence:** [SOURCE packages/ui/src/layouts/shared/browse-tab/layout.vue:49]
**Purpose:** Finding content is the app's most frequent power-user task after
launching. Search lives on the page, not in the shell, so it is contextual and
filterable rather than global.
**Operation:**
1. A search input sits at the top of the browse layout with placeholder
   `browse.search.placeholder` = "Search {projectType}…"
   (`browse-tab/layout.vue:49-53`, `153`).
2. Query text and filters are written into the URL by `useBrowseSearch`, and the
   backend is queried via the v3 search endpoint (`pages/Browse.vue:1076-1141`).
3. Filters come from instance/server context and are shown as locked rather than
   hidden (`pages/Browse.vue:1143-1149`, `1447`).
`[ORCH-PENDING]` There is **no global search and no command palette** — confirmed
by absence: the only keyboard handler in the whole app-frontend that is not
component-scoped is `N` (NAV-21). Confirm by tabbing from the rail: focus goes
straight to content. If we want one, this is the gap.
**Controls:** Free text plus a separate filter-results control and a sort
combobox (`browse-tab/layout.vue:17-21`, `47-53`).
**Persistence:** Entirely in the URL query — the strongest persistence story in
the app, and the one to copy.
**Rationale & notes:** URL-as-state here is exemplary: a filtered search is a
link you can paste, the breadcrumb root rewrites itself to match
(`pages/Browse.vue:230-247`), and the rail highlights correctly because the
subpage predicate reads `route.query.i`/`route.query.sid` (`App.vue:2278-2307`).
The design cost is that discoverability lives entirely on the Discover slot —
there is no search field on Home, in the rail, or behind a shortcut, so a user
who never opens Discover never finds search. A ⌘K-style palette over
`/browse/*` would be the single highest-value addition to this shell.

## NAV-21 — Shortcut — `N` (quick create)
**Type:** shortcut
**Nav path:** `WelcomeScreen.vue:50-68`
**Evidence:** [SOURCE apps/app-frontend/src/components/ui/WelcomeScreen.vue:50]
**Purpose:** Create an instance without leaving the keyboard. The Home page
teaches it inline: "Press N to quick create an instance", rendered as a `<kbd>`
chip (`WelcomeScreen.vue:26-29`, `114-124`). Used at first run and whenever a
new game is wanted from Home.
**Operation:**
1. `keydown` on `window`, only while `WelcomeScreen` is mounted
   (`WelcomeScreen.vue:50-74`).
2. Guarded: ignores `N` when a modifier is held, on key repeat, inside
   `contentEditable`, or in an `INPUT`/`TEXTAREA`/`SELECT`
   (`WelcomeScreen.vue:52-62`).
3. Fires only when not offline (`WelcomeScreen.vue:64-67`).
`[ORCH-PENDING]` This is the **only** global-ish shortcut in the shell, and it is
page-scoped: press `N` on `/browse/modpack` and nothing happens. Confirm, then
decide whether we want it global. There is also **no `Ctrl/⌘` accelerator for
any rail destination**, which is a real gap for a keyboard-first launcher.
**Controls:** Plain letter, no modifier.
**Persistence:** n/a
**Rationale & notes:** The guards are correct and complete — this is the pattern
to copy for any single-key shortcut, and the modifier/exclusion checks are the
part most implementations get wrong. The scope is the problem: the shortcut
lives on a component, so it exists exactly where the onboarding wants it and
nowhere else, which makes it a tutorial rather than a feature. A shell-level
keymap with one obvious reserved letter per rail destination would be a
meaningful improvement.

## NAV-22 — Shortcut — macOS menu accelerators
**Type:** shortcut
**Nav path:** `apps/app/src/macos/menu.rs:37-46`
**Evidence:** [SOURCE apps/app/src/macos/menu.rs:37]
**Purpose:** macOS expects a real application menu. Tauri builds it here, and
the app patches the one gap — WKWebView does not forward the native Undo/Redo
menu items into the webview — by declaring its own and re-emitting them as
events the frontend handles.
**Operation:**
1. `CmdOrCtrl+Z` and `CmdOrCtrl+Shift+Z` are declared as custom menu items with
   explicit IDs (`macos/menu.rs:37-46`).
2. Selecting one emits `edit-menu://undo` / `edit-menu://redo` to the `main`
   window (`macos/menu.rs:71-83`).
3. The frontend listens and replays them as DOM events, falling back to
   `document.execCommand` (`App.vue:532-535`, `537-547`).
4. The full menu is App / File (Close Window) / Edit / View (Fullscreen) /
   Window (Minimise, Maximise, Close) / Help (`macos/menu.rs:16-64`).
`[ORCH-PENDING]` These are the **only** accelerators in the entire product —
there are no `Ctrl+S`, no `Ctrl+Q`, no `Ctrl+W` on Windows or Linux, where no
menu exists at all. Confirm `Cmd+Q` and `Cmd+W` behaviour on macOS (they come
from the platform menu, `macos/menu.rs:33`, `54-59`).
**Controls:** macOS only (`#[cfg(target_os = "macos")]`, `main.rs:154-159`).
**Persistence:** n/a
**Rationale & notes:** The undo/redo bridging is a good, well-scoped piece of
platform work and the comment at `macos/menu.rs:11` explains exactly why it
exists. The asymmetry is the finding: macOS users get a full menu and two
accelerators; Windows and Linux users get none. Our rewrite should ship a
global keymap rather than inherit this split.

## NAV-23 — Shortcut — menu keyboard model
**Type:** shortcut
**Nav path:** `packages/ui/src/components/base/buttons/button-menu/button-menu.ts`
**Evidence:** [SOURCE packages/ui/src/components/base/buttons/button-menu/button-menu.ts:109]
**Purpose:** The shell's menus — account menu, instance popper, download panel,
window-control overflow — all share one keyboard model, so the rail is
operable without a mouse even though it has no text labels.
**Operation:** Arrow keys move rows; `Home`/`End` jump; `Enter`/`Space` activate;
`→` opens a submenu (the account menu's "Switch account") and `←` returns;
type-ahead by first character; `Esc` closes.
`[ORCH-PENDING]` Tab order across the rail itself: does it go top→bottom through
the instance strip, then Settings, then the account button? Confirm whether the
rail exposes a landmark so a screen reader can jump to it.
**Controls:** n/a
**Persistence:** n/a
**Rationale & notes:** This is what makes NAV-11…NAV-19 defensible rather than
hostile. The `v-tooltip` directive also injects an `aria-label` onto any element
it annotates (`packages/ui/src/providers/tooltip.ts:287-301`) and will add
`tabindex="0"` if the element is not otherwise focusable
(`tooltip.ts:311-323`), so every icon-only rail button does get an accessible
name from its tooltip — including the account button, whose label changes with
auth state (`App.vue:1457-1461`). Three gaps remain: the rail container is a
plain `<div>` with no `<nav>` landmark (`App.vue:2261-2263`); the tooltip text
is the *only* name, so a screen-reader user meets "Skin selector" with no
context; and tooltips have a 300 ms show / 400 ms hide delay
(`tooltip.ts:34-35`) that keyboard focus bypasses (`tooltip.ts:204-210`), which
is correct but means the two input modes feel deliberately different.

## NAV-24 — Notification surfaces
**Type:** region
**Nav path:** `App.vue:2588-2589`
**Evidence:** [SOURCE apps/app-frontend/src/App.vue:2588]
**Purpose:** Two distinct channels with different urgency. Inline notifications
are transient errors and warnings; popup notifications are persistent
action-required items (ads consent, app update, server invites) that must not
time out.
**Operation:**
1. `<NotificationPanel>` and `<PopupNotificationPanel>` are mounted at the root,
   both told whether the sidebar is visible so they can inset themselves
   (`App.vue:2588-2589`).
2. Backend `notification` events become popups; server invites render as toasts
   with accept/decline (`App.vue:1711-1742`).
3. Live notifications are cleared entirely when the session is invalidated
   (`App.vue:1744-1753`).
`[ORCH-PENDING]` Are popups inset by 300 px when the sidebar is up, and do they
overlay the download-manager bar when both are present? Both are handled in
source (`NotificationPanel.vue:4-9`) but their visual stacking is only knowable
on screen.
**Controls:** Popups carry `autoCloseMs: null` for anything requiring a decision
(`App.vue:737`, `1190`, `1735`).
**Persistence:** Unread server invites are marked read against the API
(`App.vue:1660-1670`).
**Rationale & notes:** The inline/popup split is the right call — a modal-feel
toast for a failed download and a corner card for "accept this server invite"
have genuinely different lifetimes. The shell is doing its job here. One thing
worth stealing verbatim: popups that must be answered set `autoCloseMs: null`
**and** `dismissible: false` (`App.vue:737-738`), so consent dialogs cannot be
swept away by a timer.

## NAV-25 — Window size, resizing and state
**Type:** region
**Nav path:** `apps/app/tauri.conf.json:73-89`
**Evidence:** [SOURCE apps/app/tauri.conf.json:73]
**Purpose:** Fix the window's identity and let the platform handle resize.
**Operation:**
1. Config: 1280×800, minimum 1100×700, `resizable: true`, `visible: false`,
   `zoomHotkeysEnabled: false`, `decorations: false`, `titleBarStyle: Overlay`,
   `hiddenTitle: true` (`tauri.conf.json:74-88`).
2. The window is hidden until the frontend calls `show_window`, which is invoked
   from `onMounted` (`App.vue:1637-1638`; command at `main.rs:53-69`). If showing
   fails, a **native** error dialog is raised before the process panics
   (`main.rs:56-66`).
3. Position/size/maximised persist through `tauri-plugin-window-state` into
   `app-window-state.json`, with the sign-in window denylisted and `VISIBLE`
   deliberately excluded from the flags so a restore cannot un-hide the window
   (`main.rs:242-256`, comment at `main.rs:254`).
4. On Windows a custom plugin issues `SetWindowPos(SWP_FRAMECHANGED)` before the
   state plugin measures the client area, because the window starts undecorated
   (`main.rs:197-240`, comment at `main.rs:204`).
5. Resize listeners update `isMaximized`/`isFullscreen`
   (`App.vue:871-876`; `WindowControls.vue:96-98`), which also toggle the
   `mac-traffic-lights` class (`App.vue:505-507`).
`[ORCH-PENDING]` Already established in `open-questions.md` OQ-002: the window
cannot be resized below 1100×700, so there is no narrow layout to inspect.
Confirm the content pane at exactly 1100×700 — in particular that the right
sidebar and the 300 px reserve still fit beside the 64 px rail.
**Controls:** `minWidth: 1100`, `minHeight: 700`; `zoomHotkeysEnabled: false`
disables Ctrl+`+`/`-`/0.
**Persistence:** `app-window-state.json`; the `signin` window is excluded.
**Rationale & notes:** Two decisions deserve credit. Hiding the window until the
frontend is mounted avoids the white-flash-then-splash-screen double paint, and
**excluding `VISIBLE` from the state plugin's flags** (`main.rs:255`) is a
non-obvious fix — otherwise restoring a previously-visible window would defeat
the hidden-at-startup design. The 1100 px floor is a genuine constraint on any
responsive design: at that width the rail (64 px) + sidebar (300 px) leaves
736 px of content, so the layout has to work at 736 px, not at some smaller
breakpoint that never occurs.

## NAV-26 — Close behaviour and the absent tray
**Type:** region
**Nav path:** `main.rs:347-457`; `WindowControls.vue:110-113`
**Evidence:** [SOURCE apps/app/src/main.rs:347]
**Purpose:** Define what closing means — the app is a launcher that keeps
background work running, so this decision matters.
**Operation:** Close persists window state and calls `getCurrentWindow().close()`
(`WindowControls.vue:110-113`, duplicated at `App.vue:948-951`). On exit,
`ExitRequested` flushes a pending Minecraft skin change
(`main.rs:353-361`); on `Exit`, a pending app update is installed, restarting
only if the user asked (`main.rs:363-432`).
`[ORCH-PENDING]` **There is no close-to-tray and no minimise-to-tray.** The app
quits when closed. Confirm that closing the window terminates the process and
that a running Minecraft child does not keep a window alive.
**Controls:** n/a
**Persistence:** n/a
**Rationale & notes:** The absence of a tray is a **defensible negative** and the
single best IA decision in the shell. The app never runs as a resident
background service — it launches a game, and the game outlives it. A tray icon
would imply "there is more happening here than the window shows", which is
false. Two related mechanisms exist and are *not* close-to-tray:
`hide_on_process_start` minimises the window when Minecraft launches
(`packages/app-lib/src/launcher/mod.rs:1252-1256`, setting at
`packages/app-lib/src/state/settings.rs:63`), and there is no minimise-on-close
setting anywhere. Note also that `core:tray:default` is granted in
`apps/app/capabilities/core.json:14` while `tauri-plugin-tray` is **not** a
dependency (`apps/app/Cargo.toml:32-40`) — a capability granted for a feature
that does not exist. Single-instance is enforced through
`tauri-plugin-single-instance` (`main.rs:176-189`), whose second launch forwards
its argument as a command and focuses the existing window rather than starting a
new one.

## NAV-27 — Native decorations toggle
**Type:** region
**Nav path:** Settings → Appearance; applied at `App.vue:819-823`
**Evidence:** [SOURCE apps/app-frontend/src/App.vue:819]
**Purpose:** Escape hatch. The custom titlebar is the app's identity, but some
users — and some Linux window managers — want the platform frame back.
**Operation:**
1. `settings.native_decorations` (default `false`,
   `composables/use-app-settings.ts:58`) is read at startup and applied with
   `getCurrentWindow().setDecorations(...)` on non-macOS
   (`App.vue:818-823`), and again independently by `WindowControls.vue:87-92`.
2. When decorations are on, `WindowControls` hides itself on Windows and Linux
   (`WindowControls.vue:58-62`) because the OS frame already provides them.
3. A feature flag `always_show_app_controls` forces the custom controls on
   regardless (`WindowControls.vue:56-62`).
`[ORCH-PENDING]` Turn native decorations on and confirm: the custom cluster
disappears, the titlebar padding-right collapses to 0 (because
`--window-controls-width` is set to 0 when the element unmounts,
`WindowControls.vue:69-75`), and the left rail still starts at y=48.
**Controls:** Boolean, syncable across devices via `behavior` preferences.
**Persistence:** `settings.native_decorations`; plus the `DECORATIONS` state flag
on Windows (`main.rs:245-247`).
**Rationale & notes:** Two places independently call `setDecorations`
(`App.vue:821` and `WindowControls.vue:91`) — redundant and a latent race, though
harmless because both read the same persisted value. The design is otherwise
right: the custom chrome is the default and the platform frame is the opt-in,
with macOS correctly excluded since `titleBarStyle: Overlay` already gives it
traffic lights.

## NAV-28 — Drag-and-drop targets
**Type:** region
**Nav path:** `apps/app-frontend/src/providers/setup/file-drop.ts:23-62`
**Evidence:** [SOURCE apps/app-frontend/src/providers/setup/file-drop.ts:23]
**Purpose:** Bring files in from the desktop. In a Tauri v2 app this requires
listening to the **native** webview drag event rather than HTML5 drop events,
because files dragged from outside the webview never produce HTML5 `drop`.
**Operation:**
1. `setupFileDropProvider` registers one global `onDragDropEvent` listener
   (`file-drop.ts:28-54`).
2. It normalises the payload's position from physical to logical pixels by
   dividing by `devicePixelRatio` (`file-drop.ts:10-16`) — otherwise every drop
   target's hit test is wrong on HiDPI displays.
3. Dropped paths are read through the Rust command
   `plugin:files|file_read_dragged_file` and turned into `File` objects
   (`file-drop.ts:18-21`, `56-60`).
4. `useFileDropTarget` hit-tests each candidate against its own element rect, so
   **drop targets are opt-in per component** (`packages/ui/src/composables/file-drop.ts:47-62`).
`[ORCH-PENDING]` The shell itself has **no drop zone** — confirm by dragging a
`.mrpack` over the rail, the titlebar and an empty content area and observing
nothing. The only component that opts in is the instance Files-tab upload
(`packages/ui/src/layouts/shared/files-tab/components/upload/FileUploadDragAndDrop.vue`).
There is a misleading signal here: the instance-creation path reports analytics
source `CreationModalFileDrop` (`App.vue:1774-1776`), which reads as
drag-and-drop but is actually the **app being opened with an `.mrpack` argument**
(`App.vue:1758-1773`).
**Controls:** None at shell level.
**Persistence:** n/a
**Rationale & notes:** The devicePixelRatio normalisation and the
bounding-rect hit test are both the kind of detail that silently breaks on HiDPI
and is invisible on a 1× capture display — worth copying without reading
further. The absence of a shell-level drop target is a missed opportunity
rather than a mistake: the file association already means double-clicking a
`.mrpack` works (`tauri.conf.json:51-56`), but dragging one onto an open window
is the gesture users try first, and nothing happens.

## NAV-29 — Deep links and file open
**Type:** region
**Nav path:** `main.rs:176-195`, `258-305`; `apps/app/src/api/utils.rs:242-250`
**Evidence:** [SOURCE apps/app/src/main.rs:176]
**Purpose:** Make the app a URL target so links in a browser, in Discord, or in
Modrinth's own site land inside the app.
**Operation:**
1. The `modrinth://` scheme is registered (`tauri.conf.json:62-68`) and
   `.mrpack` files are associated with the app (`tauri.conf.json:51-56`).
2. Deep links arrive as `deep-link://new-url` events (`main.rs:264-295`); macOS
   additionally routes `RunEvent::Opened` URLs through the same path
   (`main.rs:433-456`).
3. Every command string is parsed into a typed `CommandPayload`
   (`packages/app-lib/src/api/handler.rs`) and emitted to the frontend as a
   `command` app event, which `App.vue:1755-1808` turns into a navigation or an
   install — including pushing to `/project/<id>` and starting a server install.
4. Auth callbacks are intercepted before parsing so a login deep link never
   becomes a bad project id (`api/utils.rs:262-275`).
`[ORCH-PENDING]` Confirm that a `modrinth://` link arriving while the app is
already running navigates rather than silently failing, and that the window is
focused when it does.
**Controls:** n/a
**Persistence:** n/a
**Rationale & notes:** The payload-typed parser rather than string matching is
the right shape, and intercepting auth links first prevents a whole class of
confusing failures. One gap: the second-instance plugin forwards the argument and
focuses the window (`main.rs:177-189`) but never *shows* a window that was
hidden at startup (`tauri.conf.json:85`), so launching the app a second time
during its initialisation can focus an invisible window.

---

## IA rationale — why the shell is shaped this way

### Why the rail is icons-only

The rail is fixed at `4rem` (`App.vue:2647`) and the content pane is inset by
exactly that (`App.vue:2687`), so the rail costs 5 % of a 1280 px window and
3.6 % of the 1100 px minimum. A labelled rail would cost 180–220 px — between a
fifth and a quarter of the usable width at the minimum size. For a launcher whose
primary job is showing content and controls for a game that is already full
screen, that is not a trade worth making. The design compensates honestly:
tooltips on every slot, `aria-label` injection by the tooltip directive
(`tooltip.ts:287-301`), a full keyboard menu model (NAV-23), and a second
nesting level delivered as instance avatars rather than indentation (NAV-16).
The `collapsed_navigation` setting exists in the schema
(`packages/app-lib/src/state/settings.rs:17`) and is persisted but never read by
any frontend code — evidence that an expanded-rail mode was designed for and
deliberately, or accidentally, not shipped.

### Why Settings is a modal

Covered at NAV-18: a 300 px sidebar already competes for width, a settings
*route* would occupy a permanent rail slot for the least-used destination in the
app, and a modal cannot be left open in a background tab. The real cost is the
loss of URL and history integration, which `TabbedModal` explicitly compensates
for with `before-hide` / `before-tab-change` guards
(`AppSettingsModal.vue:332-333`).

### What the breadcrumb is for

Covered at NAV-04. In one line: **the rail cannot describe where you are**,
because the app's real position is a route *plus* a query context
(`/browse/modpack?i=<instance>`), and the breadcrumb is the only element that
assembles both into a sentence.

### What works

- **The measurement handshake** (`--window-controls-width`,
  `WindowControls.vue:64-82` → `App.vue:2675`) lets a custom titlebar coexist
  with a platform control cluster without hard-coding anything.
- **Drag-region opt-out by attribute** (`App.vue:2680-2682`) means no pixel of
  titlebar is wasted, and the logo is `pointer-events-none` so even the largest
  inert element still drags the window.
- **Two-level navigation in one column** via the draggable instance divider
  (NAV-16) — the most original idea in the app.
- **URL-as-state** on browse (`pages/Browse.vue:1157`), which makes every
  filtered view shareable and every back-step correct.
- **Breadcrumb roots that rewrite themselves** for context
  (`pages/Browse.vue:230-247`), so the trail reads as hierarchy rather than as
  a URL path.
- **Degrading indicators rather than opening modals** for things that are
  frequent and reversible: running instances, downloads, updates.
- **Honest absence of a tray** (NAV-26), so the window never lies about whether
  the app is doing something.
- **Consistent `prefers-reduced-motion` handling** throughout the animated
  status components (e.g. `download-manager-bar.vue:415-419`).

### What does not work

1. **The sidebar toggle is invisible by default** (NAV-09). `toggle_sidebar`
   gates *whether the control renders* rather than *what state it starts in*.
   Until you find it in Settings, the right 300 px is simply always there.
2. **The Screenshots rail item is gated on the wrong setting** (NAV-14).
   `GlobalSyncedOptions.screenshots` is an instance-sync capability, so users
   who have not enabled syncing cannot reach the gallery at all, and users who
   have get a navigation entry they did not ask for.
3. **Connectivity is derived from `navigator.onLine`** inside a native webview
   (NAV-05, NAV-17), producing a chip that can be wrong and a create-instance
   button that can be wrongly disabled — already logged as OQ-004.
4. **Three different i18n mistakes in the chrome**: the sidebar toggle's
   accessible label is `app.navigation.next-image` / "Next image"
   (`App.vue:602`, `2428`), and the three window-control labels are
   hard-coded English (`WindowControls.vue:11`, `19`, `28`).
5. **No global search, no command palette, no rail accelerators** (NAV-20,
   NAV-21). The only accelerators in the product are macOS menu items
   (NAV-22); `N` exists only on the Home page.
6. **The account avatar's location is ambiguous in our own records** — rail
   (source) vs titlebar (orchestrator capture). Flagged at NAV-19 for
   confirmation.
7. **Account switching reloads the whole window** (`App.vue:1504`), tearing down
   shell state that is otherwise carefully preserved.
8. **The rail has no landmark** (`App.vue:2261` is a `<div>`), and the Screenshots
   and Skins slots have no active-state styling at all (NAV-13, NAV-14).
9. **Three window controls, zero drag-and-drop** (NAV-28): dragging a `.mrpack`
   onto an open window — the gesture users try first — does nothing, and the
   analytics label `CreationModalFileDrop` (`App.vue:1775`) makes it look as if
   it does.
10. **`setDecorations` is called from two places** (NAV-27), and a tray
    capability is granted for a tray that does not exist
    (`apps/app/capabilities/core.json:14`).