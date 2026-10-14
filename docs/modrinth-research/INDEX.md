# INDEX — Modrinth App research blueprint

Read this first. It is the map: the app's navigation tree as actually observed,
the master coverage matrix, and how to read the evidence.

- **Subject:** Modrinth App 0.21.9 (codename *Theseus*), Tauri + Vue 3.
  Pinned to source tag `v0.21.9`, commit `6ebd75b6`. Full provenance and
  licences in [`sources.md`](sources.md).
- **Purpose:** so an implementing agent can close the tab/settings/function gap
  in our rewrite *without ever opening the Modrinth App again*.
- **Direction:** Modrinth's **UX, not its UI**. These docs describe behaviour,
  information architecture, flow and design intent. They never propose pixel
  copies, and no upstream code or asset was taken (see `sources.md`).
- **State:** two passes. The first covered the shell, Browse and settings; the
  second unlocked and observed the instance / install / launch slice (see
  `RUNLOG.md`, OQ-012). 116 screenshots, `S-0001`…`S-0126`.

---

## How to read this tree

| File | What it is | Who wrote it |
| --- | --- | --- |
| `BLUEPRINT.md` | The merged single-document blueprint. **Start here.** | orchestrator |
| `INDEX.md` | This file: nav tree, coverage matrix, reading guide. | orchestrator |
| `modules/01…10` | Per-surface deep dives, one file per surface. | workers + orchestrator |
| `data-model.md` | What the app persists on disk, and where. | worker |
| `api-surfaces.md` | Every API endpoint the app calls, and its parameters. | worker |
| `gap-analysis.md` | Every Modrinth capability mapped onto **our** rewrite crates. | orchestrator |
| `roadmap.md` | The gaps turned into ordered, shippable slices with acceptance criteria. | orchestrator |
| `open-questions.md` | Every judgement call made without the owner, and what was rejected. | all |
| `sources.md` | Versions, commit, SHA-256, licences, branding rules. | orchestrator |
| `MANIFEST.md` | Every screenshot, what it shows, who references it. | orchestrator |
| `RUNLOG.md` | Append-only: one line per action, with timestamps. | orchestrator |
| `FINAL-REPORT.md` | ≤2 pages; the summary the owner reads first. | orchestrator |

### Evidence tags — every claim in every file carries one

| Tag | Means | Must cite |
| --- | --- | --- |
| `OBSERVED` | Seen in the running app. | a screenshot id, e.g. `[S-0031]` |
| `SOURCE` | Read in upstream code. | `path:line` in `modrinth/code` |
| `SOURCE-web` | From docs / changelog / help. | URL + retrieval date |
| `INFERRED` | Reasoned from the above. | the basis, in the same sentence |
| `[ORCH-PENDING]` | Needs the running app; the orchestrator captures it. | what to look at |

A guess is never stated as fact. Where evidence kinds conflict, both are
recorded and the trusted one is named — our standing rule is that **source beats
pixels for defaults and behaviour, pixels beat source for current layout**.

---

## Navigation tree — as observed

Captured by hovering each rail slot, reading the tooltip the app itself draws,
then clicking that slot. Nothing here was inferred from a coordinate. The left
rail is **icon-only**; the tooltip is what names each destination.

### Top-level shell

```
Modrinth App  (window: custom chrome, decorations:false, titleBarStyle:Overlay)
│
├── Titlebar  [OBSERVED S-0001…S-0009]
│   ├── Modrinth logo (branding — not ours to copy)
│   ├── ‹ ›          back / forward
│   ├── ◷ <title>    current-route breadcrumb ("Home", "Discover modpacks", …)
│   ├── ⚯ "Offline"  connectivity chip  ← false positive, see OQ-004
│   ├── ● "No instances running"   running-instance status pill
│   ├── (account avatar)           account menu
│   └── – ▢ ×       minimise / maximise / close
│
├── Left navigation rail  (icon-only, top→bottom)
│   ├── ▷            Home                      "/"                     [S-0002]
│   ├── ◴            Discover content          "/browse/:projectType"  [S-0003]
│   ├── ☺(shirt)     Skin selector             "/skins"                [S-0004]
│   ├── ▤            Screenshots               "/screenshots"          [S-0005]
│   ├── ☰            Hosting / Servers         "/hosting/manage/"      [S-0006]
│   ├── ───────────  divider
│   ├── +            Create instance           (dialog)                [S-0007]
│   ├── ⚙            Settings                  (modal dialog)          [S-0008]
│   └── ⤶            leave / sign out          (unlabelled, §OQ-006)
│
├── Content region  (routes to the selected tab)
│
└── Right sidebar  (contextual: onboarding checklist + News feed + promos)
    └── "Getting started" — dismissible checklist: Create first instance /
        Sign in to Minecraft / Sign in to Modrinth
```

### Browse / Discover — `/browse/:projectType`  [OBSERVED S-0003]

```
Discover content
├── Project-type tabs:  Modpacks | Mods | Resource Packs | Data Packs | Shaders | Servers
├── Search field            "Search modpacks…"  (placeholder is type-aware)
├── Sort select             "Sort by: Relevance" ▾
├── View select             "View: 20" ▾
├── Pagination              1  2  …  942  ›        (real total observed)
├── Project card            icon · title · "by Author ↗" · description
│                           downloads ⭳ · follows ♡ · category badges
│                           environment badge · loader badge · "+N" overflow
│                           relative date · [+ Install]
└── Facet sidebar (right)
    ├── "Hide already installed" toggle
    └── "Category" — Adventure, Challenging, Combat, Kitchen Sink, …
```

### Settings — a **modal dialog with its own left nav**, not a tab  [OBSERVED S-0008]

```
Settings  (modal; opens over the app, closes with × or Esc)
├── DISPLAY
│   ├── Appearance            colour theme: Dark | Light | OLED | Sync with system
│   │                         + "Sync theme across devices" toggle
│   ├── Features
│   ├── Behavior
│   └── Language                        (marked **Beta**)
├── ACCOUNT
│   ├── Profile
│   ├── Social
│   └── Privacy
├── INSTANCES
│   ├── Synced settings
│   ├── Java installations
│   └── Resource management
└── footer:  "Modrinth App 0.21.9 / Linux 24.4.0"
```

> The in-app footer version matches the studied binary and the pinned tag. That
> closes the mission §10 "app version drift" question: source and UI agree.

### Routes declared by source — `apps/app-frontend/src/routes.js`

Ten top-level route objects. The router is the authoritative inventory; the rail
above is what the user actually sees.

| Route | Name | Seen in UI? |
| --- | --- | --- |
| `/` | Home | yes |
| `/browse/:projectType` | Discover content | yes |
| `/skins` | Skin selector | yes |
| `/screenshots` | Screenshots | yes |
| `/user/:user/:projectType?` | User | reachable by link |
| `/project/:id` + `Description`, `Versions`, `Version/:version`, `Gallery` | Project | yes |
| `/instance/:id` + `content`, `content/projects/:type`, `worlds`, `share`, `files`, `screenshots`, `logs` | Instance | yes |
| `/hosting/manage/` + `Overview`, `Content`, `Files`, `Backups`, `Access` | Servers | yes (out of our scope) |

---

## IA notes — why it is split this way

- **The rail is icons-only and the tooltip is the only label.** It keeps the
  chrome ~56 px wide so the content gets the window, and it forces the rail to
  stay to five stable destinations — anything that changes often belongs in the
  content area. The cost is discoverability, paid back by the breadcrumb in the
  titlebar, which always names where you are in words.
- **Settings is a modal, not a tab.** Settings is not a place you *live*; it is
  a task you complete and leave. A modal preserves the instance list behind it,
  so changing a setting never costs you your place. It also lets settings open
  *from* the instance whose settings you want.
- **Instance sub-tabs are routes, not panels.** `content` is a route even
  without a type suffix, and `/projects/:type` is the same component filtered —
  so every content view is linkable and back/forward work.
- **Onboarding lives in the right sidebar, not on Home.** "Getting started" is a
  dismissible checklist that follows the user around, so the next unfinished
  setup step is always visible without occupying the primary tab.
- **Browse is one surface with a type parameter.** Six project types share one
  search/facet/pagination implementation; the type is a tab that writes a route.

---

## Coverage matrix

One row per tab / setting / function. Status is `documented`, `partial`, or
`absent` — and `absent` always carries a reason (usually "requires sign-in —
documented from SOURCE per mission §10").

This table is the mission's definition of done (§8): every row must reach
`documented`, or be explicitly `absent` with a reason.

<!-- COVERAGE-MATRIX: populated as modules land; see modules/*.md for the
     per-item detail behind each row. -->

| ID | Surface | Item | Status | Evidence | Screenshots | Module |
| --- | --- | --- | --- | --- | --- | --- |
| NAV-01 | Shell | Window chrome & custom titlebar | documented | OBSERVED | S-0001 | 01 |
| NAV-02 | Shell | Left navigation rail (5 destinations) | documented | OBSERVED | S-0001,S-0002 | 01 |
| NAV-03 | Shell | Breadcrumb / route title | documented | OBSERVED | S-0002,S-0003 | 01 |
| NAV-04 | Shell | Connectivity chip ("Offline") | documented | OBSERVED + SOURCE | S-0001 | 01,10 |
| NAV-05 | Shell | Running-instance status pill | documented | OBSERVED | S-0001 | 01,08 |
| NAV-06 | Shell | Account menu (avatar) | documented | OBSERVED | S-0001 | 01,07 |
| NAV-07 | Shell | Window min size 1100×700 — no narrow layout | documented | SOURCE | — | 01 |
| NAV-08 | Shell | Right sidebar: "Getting started" checklist | documented | OBSERVED | S-0001 | 01,02 |
| NAV-09 | Shell | Right sidebar: News feed | documented | OBSERVED | S-0001 | 02 |
| NAV-10 | Shell | Leave / sign-out rail button | partial | OBSERVED | S-0009 | 01,07 |
| HOME-01 | Home | Welcome / first-run empty state | documented | OBSERVED | S-0001 | 02 |
| HOME-02 | Home | "Create an instance" CTA + `N` shortcut | documented | OBSERVED | S-0001 | 02 |
| HOME-03 | Home | "Import from launcher" escape hatch | documented | OBSERVED | S-0001 | 02 |
| BR-01 | Browse | Project-type tabs (6) | documented | OBSERVED + SOURCE | S-0003 | 03 |
| BR-02 | Browse | Search field | documented | OBSERVED | S-0003 | 03 |
| BR-03 | Browse | Sort select | documented | OBSERVED | S-0003 | 03 |
| BR-04 | Browse | View-density select | documented | OBSERVED | S-0003 | 03 |
| BR-05 | Browse | Pagination | documented | OBSERVED | S-0003 | 03 |
| BR-06 | Browse | Project card metadata | documented | OBSERVED | S-0003 | 03 |
| BR-07 | Browse | Facet sidebar + "Hide already installed" | documented | OBSERVED | S-0003 | 03 |
| BR-08 | Browse | Install flow (scoped direct install; Install-project picker) | documented | OBSERVED + SOURCE | S-0111,S-0112,S-0122,S-0124 | 03,04 |
| BR-09 | Browse | Project detail tabs (Description/Versions/Gallery) | documented | OBSERVED + SOURCE | S-0119,S-0120,S-0126 | 03 |
| SET-… | Settings | 10 sections, every row | see module 06 | OBSERVED + SOURCE | S-0008… | 06 |
| SKIN-01 | Skins | Skin selector (out of our scope) | documented | OBSERVED | S-0004 | 04 |
| HOST-01 | Hosting | Servers tab (out of our scope) | documented | OBSERVED | S-0006 | 09 |
| SSR-01 | Screenshots | Global screenshots viewer | documented | OBSERVED | S-0005 | 05 |

### Second-pass additions (2026-10-14) — the instance/install/launch slice

Observed after the OQ-012 environment fix; each row is documented per mission §3
in the module named in the last column.

| ID | Surface | Item | Status | Evidence | Screenshots | Module |
| --- | --- | --- | --- | --- | --- | --- |
| HOME-04 | Home | Populated Home: "Jump in" + "Library" | documented | OBSERVED | S-0109,S-0116 | 02,04 |
| CI-01 | Create instance | Dialog + four instance types | documented | OBSERVED | S-0094 | 02,04 |
| CI-02 | Create instance | Custom setup: loader/version/loader-version pickers | documented | OBSERVED | S-0095,S-0096 | 04 |
| CI-03 | Create instance | Install-project dialog + Existing-instance picker | documented | OBSERVED | S-0122,S-0124 | 03,04 |
| LIB-01 | Library | Instance list; Jump-in/Library split; sort/group/filter | documented | OBSERVED | S-0109,S-0116 | 04 |
| LIB-02 | Library | Instance context menu (Open folder / Export modpack / Create shortcut) | documented | OBSERVED | S-0108 | 04,05 |
| INST-01 | Instance | Tab bar: Content / Files / Worlds / Logs / Share | documented | OBSERVED | S-0099…S-0104 | 05 |
| INST-02 | Instance | Files browser (columns, rows, search, +) | documented | OBSERVED | S-0101 | 05 |
| INST-03 | Instance | Worlds + servers empty state | documented | OBSERVED | S-0102 | 05 |
| INST-04 | Instance | Logs: live view, All/Error/Warn/Info, empty state | documented | OBSERVED | S-0103 | 05 |
| INST-05 | Instance | Share / Invite friends | documented | OBSERVED | S-0104 | 05 |
| INST-06 | Instance | Settings → General (name/icon/duplicate/update channel/delete) | documented | OBSERVED | S-0105 | 05,06 |
| INST-07 | Instance | Settings → Installation + Repair | documented | OBSERVED | S-0106 | 05,06,09 |
| INST-08 | Instance | Settings → Sync overrides (5 toggles) | documented | OBSERVED | S-0107 | 05,06 |
| INST-09 | Instance | Content list + per-row enable/update/delete | documented | OBSERVED | S-0117 | 05,09 |
| JOBS-01 | Shell | Task manager ("Tasks" popover, Clear all) | documented | OBSERVED | S-0125 | 01,09 |
| LAUNCH-01 | Launch | "Minecraft required" gate (the only account gate) | documented | OBSERVED | S-0100 | 07,08 |
| LAUNCH-02 | Launch | Post-gate runtime states (verifying→playing→crashed) | absent | SOURCE only — not launched (OQ-013) | — | 08 |


> Settings, Accounts, Updates and Cross-cutting keep their per-item detail in
> their own modules (their tables are hundreds of rows); Browse detail, Library,
> Instance detail, the job manager and the launch gate are now summarised as rows
> here too, because the second pass made them observable. "One row per tab,
> setting and function" is satisfied at the level the successor reads; the
> exhaustive per-row tables live in the modules to avoid two copies drifting apart.

---

## Known limits of this research

Stated up front so nobody reads more into it than is there.

1. **The "Offline" chip was lying, and the mechanism is now known.** The app was
   online throughout (see OQ-004); a second pass found *why* the chip read
   "Offline" — WebKitGTK ← `GNetworkMonitor` ← `GIO_USE_NETWORK_MONITOR` — and
   fixed it without patching the binary (OQ-012). It is gone from every capture
   from `S-0093` on.
2. **No account was ever created or used.** Sign-in surfaces are documented
   unauthenticated (what is visible) plus `SOURCE` (the real state machine),
   per mission §10.
3. **No narrow-window layout exists to observe.** The window's floor is
   1100×700, so "responsive behaviour" is documented as *absent by design*
   (OQ-002).
4. **Only one app instance can run on this host** (OQ-001), so every `OBSERVED`
   claim comes from a single window on one private display. This is a limit on
   parallelism, not on evidence quality — no two captures were ever contended.
5. **Screenshots are internal reference only** — never shipped (`sources.md`).
6. **A 15% re-verification pass has now been run** (2026-10-14; `.md` sample of
   12 claims): 10 pass, 1 partial (line numbers drifted, content correct), 1
   corrected error (the settings count — the `Settings` struct has 50 fields and
   the table 54 columns, not the 46 first reported). See `RUNLOG.md` `[V1]…[V10]`.
   Remaining `OBSERVED` single-capture claims are first-hand but not all
   replicated.
7. **The launch *gate* is observed; post-launch runtime is not.** Play on an
   unauthenticated install shows the "Minecraft required" modal `[S-0100]` — the
   only account gate in the whole app. The Play state machine beyond it is
   `SOURCE` only; no authentication bypass was built (OQ-013).