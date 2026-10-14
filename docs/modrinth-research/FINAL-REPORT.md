# FINAL REPORT — Modrinth App research mission

**For the owner, on waking up.** ≤2 pages. Everything else is in
`docs/modrinth-research/`.

## What was studied

| | |
| --- | --- |
| App | **Modrinth App 0.21.9**, released 2026-10-07 |
| Binary | official `.deb` from the project's GitHub releases, SHA-256 `7c9fa6d0…8198e26` |
| Source | `modrinth/code` pinned to tag `v0.21.9`, commit `6ebd75b6` — **the same build as the binary** |
| Method | the app running on a private Xvfb display, driven by observed controls only (OCR → click what was read, never an assumed coordinate), plus source mining |
| Evidence | 116 screenshots (S-0001…S-0126), 10 modules, 4 reference docs, 13 open questions |

Source and binary are pinned to one tag, so the mission's "version drift" branch
never came into play: the UI and the code agree. The in-app footer reads
"Modrinth App 0.21.9 / Linux 24.4.0", matching both.

## The nav tree, as observed

One window, custom chrome, an **icon-only left rail** with five destinations
(Home, Discover content, Skin selector, Screenshots, Hosting) plus Create,
Settings and leave. A **breadcrumb in the titlebar names where you are in words** —
that is what makes an icon-only rail work. **Settings is a modal with its own
left nav, not a tab.** Onboarding is a dismissible checklist in the right sidebar
that follows the user across tabs. Browse is one surface with a project-type
parameter, not six pages. Ten top-level routes in `routes.js`.

## The top ten capabilities our app is missing

Ordered by the mission's rule: what you called out, then daily-use frequency,
then effort.

1. **50 persisted settings with schema-owned defaults** — they live in *one SQLite
   row* (`id = 0`) in `app.db`, with `DEFAULT` clauses and a versioned migration
   ladder. We have 7 settings tabs and no schema-owned defaults.
2. **A `Features` section that toggles instance tabs** — we have no equivalent.
3. **Pre-launch / wrapper / post-exit hooks.**
4. **Extra JVM args and custom environment variables.**
5. **Java installation management** — we have a Java tab, not management.
6. **Per-instance override of global settings**, with the inheritance stated.
7. **A custom data directory with a recoverable move** (`custom_dir` + `prev_custom_dir`).
8. **App self-update, tri-state** (`null` = ask / `true` = auto / `false` = never).
9. **Developer mode.**
10. **A typed feature-flag enum** gating unfinished surfaces.

**Settings is the whole top ten.** That is your complaint confirmed from both
directions, and it is the cheapest thing on this list to fix once the settings
store exists.

## The five most instructive patterns

1. **Settings as one row, defaults in the schema, migrations owned by the
   schema.** "What are all our settings" becomes a question the schema answers.
   They moved the heap default 2 GB → 4 GB with a migration gated on the old
   value still being untouched, so users who chose 2 GB kept it.
2. **A typed error vocabulary** — an error *code* from an enum plus a *phase*,
   resolved to plain sentences, with paths/hashes/sizes persisted so
   "Copy details" emits a near-finished support document. Consistency for free.
3. **Empty states are calls to action, not verdicts.** And destructive actions are
   gated on a review that *names what will happen*, not a yes/no.
4. **Onboarding derived from real state**, so it can show an unfinished step
   honestly instead of lying.
5. **Defaults that depend on the machine**, and shortcuts that are *advertised
   in the UI* next to the action they accelerate.

## The three to refuse

- **Their connectivity indicator is wrong, and it is load-bearing.** It is
  `!navigator.onLine` and reported **Offline while the app was fully online**.
  Worse, the same flag is bound to the "Create an instance" button's
  `:disabled` and to the `N` shortcut's guard — so in that state **the app
  cannot create an instance at all**. Ours must derive connectivity from the
  request layer, never a browser guess, and must never gate an action on one.
- **They flatten a specific error into a generic one** on the way to the UI
  ("not enough space" → "Couldn't save files"), except in one dialog where they
  get it right.
- **Servers in the launcher.** Three feature flags, five routes, two synced
  tables and a ping module. We decline, deliberately.

## What we already do better — do not regress

We have a **narrow layout** (they floor at 1100×700; we run at 840×600).
**In-dialog receipts** on a dedicated event, where they still lean on toasts.
**Byte-exact local operations** with overwrite refusal, verified against actual
bytes.

## What could not be verified, and why

- **Sign-in surfaces** — no account was created or used (mission rule). Documented
  unauthenticated plus `SOURCE`; a 14-gate list of account-gated surfaces is in
  `modules/07`.
- **Post-launch runtime only** — the Play state machine *after* the account gate
  (verifying → playing → crashed), exit handling and post-exit behaviour. The gate
  itself is now observed `[S-0100]`, and everything before it is observed too.
  Running the game needs a licensed account, and no authentication bypass was built
  (OQ-013). This is the one honest residual gap, and it is source-documented.
- **A narrow-window layout** — does not exist; the window's floor is 1100×700.
- **Photosensitivity modal and project-card right-click menu** — still `SOURCE`
  only (`BR-41`, `BR-42`).
- **The 15% re-verification pass — now run** (2026-10-14): 12 claims spot-checked
  independently, 10 pass, 1 partial (line numbers drifted, content correct), and
  **1 corrected error** (the settings count, below). Single-display `OBSERVED`
  claims remain first-hand, now partly replicated.

## The second pass (2026-10-14), in one line

The first pass could not create an instance because the app's missing connectivity
made its own "Offline" guess true and disabled the button. A one-line environment
fix — `GIO_USE_NETWORK_MONITOR` (OQ-012) — made the webview report online, which
unlocked the entire instance / install / launch slice: **28 new screenshots,
`S-0093`…`S-0126`**, and turned the library, instance detail, content-install
pipeline, task manager and launch gate from `SOURCE` into `OBSERVED`.

## Look at these first

`S-0003` (Browse with live data — the whole card/facet/pagination model),
`S-0008` (the Settings modal and its complete section list), `S-0001` (first-run
Home), `S-0093` (the same Home with the offline bug gone), `S-0094` (the
create-instance dialog), `S-0100` (the launch gate — the one account gate),
`S-0107` (per-instance sync overrides) and `S-0125` (the task manager). Then
`modules/06` for settings, `modules/05` for the instance surface and `modules/08`
for error copy.

## Deliverables

`INDEX.md` (nav tree + matrix) · `BLUEPRINT.md` (merged spec) · `modules/01…10` ·
`data-model.md` · `api-surfaces.md` · `gap-analysis.md` · `roadmap.md` ·
`open-questions.md` · `sources.md` · `MANIFEST.md` · `RUNLOG.md` · `tools/`

**If only five roadmap slices ever ship: 0, 1, 3, 4 and 9** — the settings store,
the Features section, args/env/hooks, Java management, and instance context
menus. That is the shortest path from "our app is missing settings and functions"
to not being told so again.

Nothing was committed. Screenshots are internal reference only — never shipped.