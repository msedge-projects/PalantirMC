# BLUEPRINT — the Modrinth App, as a buildable specification

**Modrinth App 0.21.9** (codename *Theseus*), Tauri + Vue 3 desktop launcher.
Studied from the official binary and from source pinned to the same tag, so the
UI and the code agree. Provenance, licences and branding rules: `sources.md`.
Per-surface detail: `modules/01…10`. What is missing in our app:
`gap-analysis.md`. What to build: `roadmap.md`.

**Direction: Modrinth's UX, not its UI.** This document describes behaviour,
information architecture, flow and design intent. It never proposes a pixel
copy, and no upstream code, asset or branding was taken.

**Second observation pass (2026-10-14).** The first pass could not create an
instance, so the instance/install/launch slice was `SOURCE`-only. A one-line
environment fix (`GIO_USE_NETWORK_MONITOR`; OQ-012) made the webview report online
and every one of those surfaces became observable with **no account** — the library,
the create-instance dialog, the content-install pipeline, the instance tabs and
settings, the task manager, and the launch gate (the app's only account gate). See
`S-0093`…`S-0126`, the second-pass sections in `modules/01…10`, and `RUNLOG.md`.

---

## 1 — The shape of the app

One window, custom-drawn chrome, an icon-only left rail, and a content region
that swaps per route. Default 1280×800, **minimum 1100×700** — there is no
narrow layout at all, by design.

```
Titlebar   logo · ‹ › · <route breadcrumb> · [Offline chip — bug, see OQ-012] · "No instances running" · avatar · – ▢ ×
Rail       Home · Discover content · Skin selector · Screenshots · Hosting · ─ · + · <instance switcher> · Settings · leave
Content    the route's page
Sidebar    "Getting started" checklist · News · promo     (contextual, dismissible)
```

**Settings is a modal with its own left nav, not a tab** (`modules/06`).
**Onboarding lives in the sidebar**, following the user across tabs. **Browse is
one surface** with a type parameter, not six pages.

| Route | Name | Purpose |
| --- | --- | --- |
| `/` | Home | empty state when new, library when populated |
| `/browse/:projectType` | Discover content | search, facets, cards, install |
| `/skins` | Skin selector | *out of our scope* |
| `/screenshots` | Screenshots | *out of our scope* |
| `/user/:user/…` | User | author page |
| `/project/:id` | Project | description · versions · version · gallery |
| `/instance/:id` | Instance | content · worlds · share · files · screenshots · logs |
| `/hosting/manage/` | Servers | *out of our scope* |

`SOURCE apps/app-frontend/src/routes.js` — ten top-level routes.

---

## 2 — What it does, surface by surface

### 2.1 Shell and navigation — `modules/01`

Five rail destinations, icon-only, each named by a tooltip on hover — the only
labels they have. The **breadcrumb in the titlebar names where you are in
words**, which is the whole justification for the icon-only rail: 56 px of chrome
in exchange for always-visible orientation.

The rail's destinations have **visibility conditions** — the account slot is a
sign-in glyph when signed out and an avatar menu when signed in, and
Skins/Screenshots/Hosting appear or not by state. That is how the rail stays at
five stable destinations instead of growing.

Window behaviour: custom titlebar (`decorations: false`, `hiddenTitle: true`,
`titleBarStyle: Overlay`), drag region, resize, and a running-instance status
pill in the titlebar rather than in the content.

### 2.2 Home — `modules/02`

An **empty state**, not a dashboard. Day one: "Welcome to Modrinth" + "Create an
instance" + "Press N to quick create an instance" + "Escaping another launcher?"
→ "Import from launcher". Populated: the library replaces it entirely.

The `N` shortcut is **advertised on screen** and **guarded against text fields**.
The import escape hatch is placed *below* the primary CTA and visually quieter —
the app would rather you create than import, but rather you import than leave.

### 2.3 Discover / Browse — `modules/03`

Six project-type tabs over one implementation. Search, five sort orders
(`relevance|downloads|follows|newest|updated`), a view-density select, real
pagination with a real total, and project cards carrying icon, title, author,
description, downloads, follows, category/environment/loader badges and a
relative date. A facet sidebar with "Hide already installed".

**Facets are `string[][]`** — outer array ANDed, inner array ORed — so multi-
select within a facet and AND across facets needs no special casing. `SOURCE
packages/api-client/src/modules/labrinth/types.ts:1059`.

**The install flow:** instance picker → version/loader resolution →
**dependency resolution with required vs optional** → confirmation → progress.
`DependencyType = 'required' | 'optional' | 'incompatible' | 'embedded'`
(`SOURCE types.ts:1518`). Optional ones are asked about; required ones are not.

### 2.4 Library and instances — `modules/04`

Instance list with grouping, sorting, filtering, a play button with several
states, and an **exhaustive context-menu action set**. Import from other
launchers, an add-instance dialog (vanilla / with-loader / from-modpack), and
delete-with-confirmation.

### 2.5 Instance detail — `modules/05`

Sub-tabs that are **routes**, not panels: `content` (default), content filtered
by type, `worlds`, `share`, `files`, `screenshots`, `logs`. Per-item
enable/disable, update-all, add-content in context. Instance settings override
globals; the override mechanism is the thing to mirror.

### 2.6 Settings — `modules/06`

Ten sections, **50 persisted settings** (the `Settings` struct has 50 fields; the
live `settings` table has 54 columns), all in **one SQLite row** (`id = 0`) in
`app.db`. Defaults are `DEFAULT` clauses in the schema; `settings.version` is 3
against `CURRENT_VERSION = 3`, migrated by a hand-written ladder. Key names,
defaults, reset behaviour and the i18n strings behind each row are in the module.

The three decisions to take: **one row, one struct**; **defaults in the schema,
not in code**; **a settings schema that owns its own migrations**.

### 2.7 Accounts — `modules/07`

Two identities, never conflated: a **Minecraft** account (device-code grant,
PKCE, in-app webview) and a **Modrinth** account (implicit OAuth with a **sealed
token** — an ephemeral P-256 key plus nonce, so a loopback listener cannot
harvest it). Multiple accounts per kind, a switcher, lazy refresh, and
**sign-out ≠ remove**.

A 14-gate list of surfaces that need an account is in the module — those are the
ones we document unauthenticated.

### 2.8 Launch and runtime — `modules/08`

The play button has **six renderings and no verifying/downloading state** —
downloading is a global task surface, not a button state. 13 install phases × 6
statuses, unbounded concurrent game processes, hard-kill stop, and **no post-exit
dialog at all**.

The headline is the **error vocabulary**: an error *code* from an enum plus a
*phase*, resolved to plain sentences, with paths/hashes/sizes persisted so
"Copy details" emits a near-finished support document. And the headline defect:
a specific "not enough space" error **flattened to "Couldn't save files"** on the
way to the UI — except in one dialog, where it is named correctly.

### 2.9 Updates and maintenance — `modules/09`

App self-update compiled behind a **cargo feature flag** with a no-op fallback;
a **tri-state** update preference (`null` = ask, `true` = auto, `false` =
never); one-shot self-clearing nudge fields; per-instance content update checks
stored in a table; repair/reinstall; cache management made safe by a disk layout
that separates disposable cache from real state.

### 2.10 Cross-cutting — `modules/10`

Toasts vs dialogs vs inline receipts vs banners, and the rule for choosing.
Confirmation patterns. **Empty states are calls to action, not verdicts.**
Loading belongs on the task, not on the trigger. And the two negatives: a
**connectivity chip that lies** (`!navigator.onLine`) and a **theme value the UI
no longer exposes** (`retro`).

---

## 3 — The data model — `data-model.md`

`~/.local/share/ModrinthApp/` on Linux: `app.db` (SQLite, **54 tables**, 49
migrations), `profiles/`, a **content store** (`store/`, `store_operations`,
`store_blobs`, `store_retained_refs`) with staging, per-session timestamped logs,
and webview-managed cache that is disposable.

The architectural decision worth stealing: **the instance file index is a table,
not a directory walk**, and content is referenced by **project/version id** with
**hash-addressed file fetches**. That is what makes updates, rollback and
content sharing between instances possible. Roughly half the schema is
Modrinth-specific and half is standard launcher practice; the module classifies
each.

---

## 4 — The API — `api-surfaces.md`

Typed client wrapper; parameters are real, not inferred. `GET /search` with
`facets: string[][]`, five sort indexes, `offset`/`limit`. Project, version,
dependency, gallery and license endpoints. Auth for both accounts. Files fetched
**by hash**. A **response cache** (601 rows on a fresh install) separate from the
**content store**. Each entry carries a mirror/skip call for `palantir-net`.

---

## 5 — The five patterns worth adopting

1. **Settings as one row with schema-owned defaults and a versioned migration
   ladder.** It makes "what are all our settings" a question the schema answers,
   and changing a default a data migration rather than a code change.
2. **A typed error vocabulary (code + phase) with a "Copy details" that already
   contains the evidence.** Consistency and debuggability for free.
3. **Empty states as calls to actions, and destructive actions gated on a review
   that names what will happen.** Both are cheaper than they look and both are
   felt immediately.
4. **Onboarding that follows the user and is derived from real state**, so it
   never lies.
5. **Defaults that depend on the machine**, moved for existing users by a
   migration gated on the old value still being untouched (the 2 GB → 4 GB heap).

## 5b — The three patterns worth refusing

1. **A connectivity indicator from a browser-level API.** It reported Offline
   while fully online (`OBSERVED S-0003`). Ours must come from the request layer.
2. **Flattening a specific error into a generic one** on the way to the UI.
3. **Building servers into the launcher.** The reference spends three feature
   flags, five routes, two synced tables and a ping module on it. We decline.

---

## 6 — What we already do better

- **A narrow layout.** They floor at 1100×700; we run at 840×600.
- **In-dialog receipts.** A confirmed operation's outcome renders inside the open
  review dialog, on a dedicated event. The reference still leans on toasts.
- **Byte-exact local operations** with overwrite refusal, verified against actual
  bytes.

---

## 7 — Honest limits

No account was ever created or used, so sign-in surfaces are documented
unauthenticated plus `SOURCE`. The populated library, the install flow with
dependencies, launch/runtime states and genuine offline states could not be
captured for the same reason; those gaps are listed in `MANIFEST.md` and marked
`[ORCH-PENDING]` in the modules rather than filled with invention. No narrow
layout exists to observe. Every screenshot is internal reference only.

**Next:** `gap-analysis.md` for what we are missing, then `roadmap.md` for the
ordered slices — **ship 0, 1, 3, 4 and 9** if only five ever ship.