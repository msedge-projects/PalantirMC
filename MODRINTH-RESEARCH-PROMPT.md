# Modrinth App research mission — copy/paste start prompt

Copy everything from `=== START PROMPT ===` to `=== END PROMPT ===` into a fresh,
long-running agent session. It is self-contained: the agent does not need to ask
the owner anything, and must not. The owner will be asleep.

Written for the PalantirMC 2.0 rewrite. The output of this mission is a research
blueprint under `/PalantirMC/docs/modrinth-research/` that the implementing agent
(your successor) consumes to close the tab/settings/function gap against the
Modrinth App.

=== START PROMPT ===

# Mission: reverse-engineer the Modrinth App into a buildable blueprint

You are the research orchestrator. Your one deliverable is a complete,
evidence-backed blueprint of the **Modrinth App** (codename *Theseus*): every
tab, every settings item, every function — what it is, how it operates, why it
exists, and what it is for — written so that another agent can implement our own
version of it without ever opening the Modrinth App again.

You will run unattended for a long horizon. **The owner is asleep. Never ask
questions. Never pause waiting for input.** Make the reasonable call, record it
in `open-questions.md` with the alternatives you rejected, and continue. The only
acceptable reasons to stop are: the coverage matrix in §8 is complete, or every
degradation path in §10 is exhausted and documented.

This is a **research** task. You do not implement features in our app. You may
write research tooling (screenshot/automation scripts) and documentation; you
may not modify anything under `/PalantirMC/rewrite/` (the implementation
workspace) or `/PalantirMC/crates/` (the legacy tree, which is out of scope and
must not be read for content).

## 0. What you are studying

- **The reference**: the Modrinth App, the Tauri + Vue desktop launcher.
  - Binary: install the official Linux release from <https://modrinth.com/app>
    (AppImage or .deb). Record the exact version you study and its SHA-256 in
    `sources.md`. If the official site is unreachable, try the GitHub releases
    of `modrinth/code`. Never install from an unofficial mirror.
  - Source: the `modrinth/code` monorepo —
    <https://github.com/modrinth/code>. The app lives under `apps/app*`
    (frontend) and Rust/Tauri backend code; shared libraries under `packages/`.
    Development guide: <https://docs.modrinth.com/contributing/theseus/>.
    Clone it shallow into `/tmp/modrinth-code` (it is your ground truth; it must
    never enter our repository trees).
  - Ancillary sources, cited as `SOURCE-web`: the Modrinth changelog
    (<https://modrinth.com/news/changelog>), docs, and help articles. Use them to
    date-stamp behavior ("since vX.Y this moved to …") — the app's UI moves
    between releases and you must not document a stale layout as current.
- **The consumer**: the PalantirMC 2.0 rewrite at `/PalantirMC/rewrite/` — a
  Rust + iced 0.12 native launcher, our own copper/matte design language, GPL
  clean-room with its own licence audit. The owner's standing direction is
  **Modrinth's UX, not its UI**: we reproduce familiar behavior, information
  architecture and flow design in our own visuals. Your blueprint documents
  behavior and rationale; it does not propose pixel copies.

## 1. Ground rules (non-negotiable)

1. **Never ask the owner anything.** Decide, log in `open-questions.md`, move on.
2. **Clean-room discipline.** The upstream monorepo carries per-package licences
   and copying guidelines that restrict code *and* branding. Therefore:
   - No upstream source code, CSS, asset, icon, image, or branding may be copied
     into `/PalantirMC`. Quoting a short UI label in research docs is fine and
     must be cited; wholesale copy, translation files, and asset files are not.
   - Screenshots are internal reference material for our implementer, not assets
     to ship. Say so in `sources.md`.
   - Record every upstream licence you touch (per package, as stated in the
     monorepo's copying guidelines) in `sources.md`, including branding rules.
3. **Evidence discipline.** Every claim in every document carries one of these
   tags and its citation:
   - `OBSERVED` — you saw it in the running app; cite screenshot IDs (`[S-042]`).
   - `SOURCE` — it comes from upstream code; cite `path:line` in `modrinth/code`.
   - `SOURCE-web` — docs/changelog/help page; cite URL + retrieval date.
   - `INFERRED` — your reasoning on top of the above; state the basis.
   Never state an unverified guess as fact. Conflict between evidence kinds is
   information: record both and which you trust (per our earlier finding, source
   beats pixels for defaults, pixels beat source for current layout).
4. **Do not disturb the owner's environment.** The owner's display (`:0`) has a
   running PalantirMC demo instance — leave it alone. Use a **private Xvfb
   display** for all automation (§4). Never kill processes you did not start.
   Never use, create, or link any real Microsoft/Minecraft account; sign-in
   surfaces are documented unauthenticated + from source (§10).
5. **Write only under `/PalantirMC/docs/modrinth-research/`** (your entire
   deliverable) and `/tmp` (scratch: clone, Xvfb, raw captures before culling).
   Nothing else in the repo is touched. Do not commit anything; the owner
   decides what gets committed.
6. **Honesty.** If you could not verify something, the docs say so. A documented
   gap is acceptable; a fabricated answer is a mission failure.

## 2. Orchestration plan — spawn workers, merge their output

You can launch multiple agents. Use them; this is a long task and parallelism is
the point. One orchestrator (you) plus workers, each owning one surface or one
evidence kind:

- **Phase 0 — recon (orchestrator).** Install the app, clone the source, get it
  running on a private display, take one full-window screenshot per top-level
  tab to bootstrap the coverage matrix (§8). Write `RUNLOG.md` from minute one.
- **Phase 1 — capture campaign (parallel workers, one per surface area).**
  Each worker drives the real app on its own private display (`:101`, `:102`, …
  one display per worker — shared displays contaminate captures), works its
  checklist slice end-to-end (screenshots + module doc together, never "docs
  later"), and returns: its `modules/*.md` file, its screenshot batch, and a
  ≤20-line summary.
- **Phase 2 — source mining (parallel workers, one per code area).** For every
  checklist item Phase 1 could not fully explain (defaults, hidden states, what
  "Reset" resets to, persistence keys, feature flags, API calls), read
  `/tmp/modrinth-code` and fill the gap with `SOURCE` citations. Deliver
  `data-model.md` (what the app persists and where) and `api-surfaces.md`
  (§7) from this phase.
- **Phase 3 — synthesis (orchestrator + one reviewer worker).** Merge into
  `BLUEPRINT.md` (§9), write `gap-analysis.md` and `roadmap.md` (§6), and run a
  skeptical pass: a second worker re-verifies a random 15% of claims against
  the app/source and reports discrepancies in `RUNLOG.md`.
- **Phase 4 — self-audit (orchestrator).** Walk the definition of done (§8) and
  the evidence rules; fix or downgrade every claim that fails. Then write
  `FINAL-REPORT.md`.

Worker brief template (fill it fully — workers know nothing):
"Goal: [checklist slice]. Evidence rules: §1.3 of the mission. App access:
[display number], launch command [..]. You write [exact module file path] and
screenshots S-[range] under docs/modrinth-research/. Cite every claim. Log
unknowns to open-questions.md (append, never overwrite). Return ≤20 lines."

Checkpoint continuously: every worker writes its files to disk as it goes, and
`RUNLOG.md` gets one line per completed action (timestamp, what, evidence IDs).
The task must be recoverable after a crash at any point.

## 3. What to document about every tab, setting, and function

This answers "what tabs are like, how do they operate, why are they what they
are, and what's the use of that". For **every** item in the coverage matrix:

1. **Identity** — exact label, icon description, where it sits (navigation
   path), what category of thing it is (tab / button / toggle / field / menu /
   dialog / status indicator / keyboard shortcut).
2. **Purpose** — one paragraph in plain language: what user problem it solves,
   when a user reaches for it, how often. This is the "why does this exist"
   answer the owner explicitly wants. If the item exists to satisfy an edge case
   (e.g. repair, offline, proxy), say which.
3. **Operation** — what happens step by step when used: immediate visual
   feedback, network calls (§7), files touched, what changes afterwards, what
   the user sees next. Include its loading / empty / disabled / error /
   hover / focus states — capture them.
4. **Controls detail** — for each field/toggle: type, full option list, **default
   value** (from source if not observable), range/units, validation, and exactly
   what "reset" does.
5. **Persistence** — where the setting/state lives (config file + key from
   source, in-memory, per-instance vs global) and whether it syncs anywhere.
6. **Rationale & quality notes** — your assessment of why it is designed this
   way (wording, placement, defaults, grouping) and what makes the UX good or
   bad. Our implementer needs the *design intent*, not just the mechanics.

Record the **information architecture** above the item level too: how the
navigation is grouped, what is global vs per-instance, which things are tabs vs
dialogs vs sidebars vs context menus, and why that split. Map the whole nav tree
in `INDEX.md` as an outline.

## 4. Environment & capture recipe (this host)

- Tools already proven here: `xdotool`, ImageMagick `import`, `tesseract` OCR.
  `rewrite/tools/ui_smoke.py` shows the working approach for driving a real
  window by **observed controls only** (OCR the screenshot, click what you saw —
  never click assumed coordinates). Reuse that pattern in your own scripts under
  `docs/modrinth-research/tools/`.
- Private display recipe (from prior work on this host; adjust the number):
  `systemd-run --unit=xvfbNNN --property=StandardOutput=null --property=StandardError=null /usr/bin/Xvfb :NNN -screen 0 1920x1080x24 -nolisten tcp`
  Notes that will save you an hour: another agent's window on a shared display
  contaminates captures — one display per worker; `cargo`/app paths are not on
  the default PATH inside `systemd-run`; never `nohup`, never kill another
  session's processes; long jobs via `systemd-run` + an `.exit` file you poll.
- The Modrinth App is a webview: prefer keyboard navigation (Tab/arrow/Enter/Esc,
  `/` for search, etc. — *verify* the shortcuts, don't assume) and explicit waits
  after focus changes before capturing.
- Standard capture sizes: **1280×800** primary; re-shoot key screens at
  **840×600** to record responsive/narrow behavior. PNG, one window, no
  desktop clutter in frame. If the app renders under Xvfb with issues (this host
  has seen GTK refuse to initialize under some setups — see §10), fall back per
  §10 rather than burning hours.

## 5. Survey checklist (seed list — extend it; nothing on screen escapes the matrix)

Cover at minimum. Each bullet is a matrix row per §3:

- **Shell & navigation**: window chrome, sidebar/rail, tab list, back/forward,
  search entry point, account menu, notifications, download manager/status,
  window resize/min-size behavior, close-to-tray or not, drag-and-drop targets.
- **Home / landing**: what it shows, when it is empty, its call-to-action, how it
  decides content.
- **Browse / Discover**: project types (modpacks, mods, resource packs, shaders,
  datapacks, mod loaders…), search behavior (debounce, submit, facets), filters
  (categories, loaders, game versions, environments, licenses…), sort options,
  open/closed source filter, pagination/infinite scroll, project cards (what meta
  is shown and why), project detail page (tabs: description, versions, gallery,
  changelog, dependencies…), the install flow (instance picker → version
  resolution → optional/required dependencies → confirmation → progress), and
  update flows.
- **Library / instances list**: instance cards/rows, grouping/sorting/filtering,
  play button states (launching/playing/error), context menus, pinning, duplation,
  deletion confirmation, import (mrpack/CurseForge/ATLauncher/Prism/GDLauncher/
  MultiMC/technic… which exactly, from source), "add instance" dialog (vanilla /
  loader / modpack import paths, version + loader pickers).
- **Instance detail pages**: content tabs (Mods, Resource packs, Shaders,
  Datapacks, …), per-item enable/disable, update-all, add-content in context,
  Worlds (list, play, edit, delete, export?, open folder), Screenshots, Logs
  (live view? copy? export? crash reports?), Files/file browser if any, instance
  settings (Java, memory, resolution, hooks/commands, environment variables,
  per-instance overrides of global settings, game version/loader upgrade, repair,
  "open folder" affordances).
- **Settings (global)** — enumerate every section and every row, e.g.:
  appearance (theme/density/language), Java/JVM (discovery, management, memory,
  args), game defaults (resolution, fullscreen, hooks), downloads (concurrency,
  mirrors? bandwidth), network/proxy, privacy (telemetry, analytics, online
  accounts), notifications, features (note the changelog moved Files/Worlds/
  Screenshots toggles from instance settings into Features — verify current
  placement), updates (app auto-update), advanced (data directory, cache,
  developer mode), about (version, licenses, logs folder). For every row: §3.
- **Accounts**: sign-in flow(s) (Microsoft device code? browser?), account
  switcher, multiple accounts, skins/cape if present in-app, offline behavior,
  what happens on token expiry (from source).
- **Launch & runtime**: launch button state machine (idle → verifying →
  downloading → launching → playing → closed/crashed), what happens on crash,
  concurrent launches, "stop" behavior, post-exit behavior, Java-missing and
  disk-full errors.
- **Updates & maintenance**: app self-update, per-instance content updates,
  loader/game version updates, pack repair/reinstall, cache management.
- **Cross-cutting**: keyboard shortcuts (list them all and where documented),
  accessibility labels/focus order, i18n approach, toasts/notifications/dialogs
  inventory, undo/confirmation patterns, error copy patterns (how the app names
  problems and offers next steps — we want to match its *helpfulness*, in our own
  words), empty-state patterns, loading patterns.

## 6. Gap analysis & roadmap (the point of the whole mission)

`gap-analysis.md` maps every finding onto **our** implementation, the rewrite at
`/PalantirMC/rewrite/`. Read (only) these to ground the mapping:
`crates/palantir-desktop/src/` (`main.rs` shell + browse, `design.rs`,
`controls.rs`, `surfaces.rs`, `gallery.rs`, `flows.rs`, `bedrock.rs`, `local.rs`,
`files.rs`, `model.rs`, `backend.rs`), `crates/palantir-net/src/` (auth, cache,
client, download, scheduler, store, sync), `crates/palantir-loader/src/`
(mrpack, prism, forge, java, launch, installer, profiles, dependencies),
`crates/palantir-bedrock/src/`, `crates/palantir-core/src/`, and
`rewrite/NOTES.md` + `UI-HANDOFF.md` for known gaps.

For each Modrinth capability: **Missing / Partial / Equivalent / Out-of-scope**
(vs our stated scope: no skins, screenshots gallery, servers, hosting, telemetry;
two editions Java+Bedrock managed; Modrinth-UX-not-UI direction), one line of
evidence, and the target rewrite module(s).

`roadmap.md` turns the gaps into ordered, independently shippable slices with
acceptance criteria each ("user-visible behavior X, verified by Y"), prioritized
by (a) what the owner called out as missing — tabs, settings, functions — then
(b) daily-use frequency, then (c) effort. This is the checklist your successor
implements from; make each slice concrete enough to start without re-research.

## 7. API & data mapping (source-mined)

`api-surfaces.md`: for each browse/library/update surface, the Modrinth API
endpoints and parameters it uses (search facets, sort options, loaders/versions
filters, project/version/dependency endpoints, auth endpoints for sign-in),
cited `path:line` in `modrinth/code` (frontend `apps/app*`/`packages/` fetch
layers and the `apps/labrinth` API docs). Our `palantir-net` will mirror the
useful subset — note rate limits/caching hints you find.
`data-model.md`: what the app persists on disk (instance/profile format, settings
file location + keys, cache layout, log layout) with source citations; note which
parts are Modrinth-specific versus standard launcher practice.

## 8. Coverage matrix & definition of done

`INDEX.md` ends with the master table: one row per tab/setting/function, columns
= {ID, surface, status: documented|partial|absent, evidence tags, screenshot IDs,
module file}. **Done** means every row is `documented` or explicitly `absent`
with a reason (e.g. "requires sign-in — documented from SOURCE per §10").

Definition of done for the mission — check each before `FINAL-REPORT.md`:
- [ ] Every seed-checklist item (§5) is in the matrix and documented per §3.
- [ ] The nav tree outline in `INDEX.md` reflects the actually-observed app.
- [ ] Every screenshot referenced by any doc exists; every shipped screenshot is
      referenced (MANIFEST.md reconciles both directions).
- [ ] `gap-analysis.md` covers every Modrinth capability found, each mapped to
      rewrite module(s) with a status.
- [ ] `roadmap.md` slices each carry acceptance criteria and an order.
- [ ] `api-surfaces.md` and `data-model.md` exist and are `SOURCE`-cited.
- [ ] `open-questions.md` lists every unresolved item with what was tried.
- [ ] `sources.md` records app version, source commit, licences, URLs, retrieval
      dates, and the internal-reference-only note for screenshots.
- [ ] Evidence tags pass spot-check (Phase 3's 15% re-verification, logged).
- [ ] `BLUEPRINT.md` reads top-to-bottom as a single coherent document.

## 9. Deliverable layout (exact — the successor agent depends on these names)

```
docs/modrinth-research/
  INDEX.md              # nav-tree outline + coverage matrix + how to read this
  BLUEPRINT.md          # the merged single-document blueprint (built LAST)
  modules/
    01-shell-navigation.md
    02-home.md
    03-browse-discover.md
    04-library-instances.md
    05-instance-detail.md
    06-settings-global.md
    07-accounts-auth.md
    08-launch-runtime.md
    09-updates-maintenance.md
    10-cross-cutting.md   # shortcuts, a11y, i18n, dialogs, toasts, error/empty/loading patterns
  data-model.md
  api-surfaces.md
  gap-analysis.md
  roadmap.md
  open-questions.md
  sources.md
  RUNLOG.md             # append-only, timestamped, one line per action
  FINAL-REPORT.md       # ≤2 pages for the owner waking up
  MANIFEST.md           # screenshot index (see below)
  screenshots/S-0001.png …   # zero-padded, chronological
  tools/                # your capture/OCR/automation scripts
```

Module doc internal structure (all of §3's six facets, with a fixed heading so
the successor can skim):

```
## <item ID> — <label>
**Type:** …  **Nav path:** …  **Evidence:** [OBSERVED S-0xx] [SOURCE apps/app/…:123]
**Purpose:** …          ← why it exists / who uses it / when
**Operation:** …        ← step-by-step, incl. states
**Controls:** …         ← fields, options, defaults, ranges
**Persistence:** …
**Rationale & notes:** …
```

Screenshot rules: `MANIFEST.md` rows = {ID, file, window size, state shown, what
to look at, referenced-by}. Number chronologically; never reuse IDs even for
deleted shots (note deletions). Name states in the manifest, not in filenames.

**Screenshots the implementing agent especially wants** (shoot these even if the
matrix would let you skip): every top-level tab at 1280×800; every settings
section fully scrolled (stitch or consecutive frames); every dialog, wizard, and
context menu open; each flow of §5 as a numbered step sequence (add-instance,
modpack install with its dependency/optional step, content update-all, repair,
launch→playing→crashed, account sign-in states); hover / focus / selected /
disabled variants of the main controls; every empty, loading, and error state
you can provoke (offline mode with `unshare -n` or firewall is a legitimate way
to provoke them); the version/loader pickers open; search with filters open and
applied; the notification/toast patterns; and key screens again at 840×600.

## 10. Degradation protocol (do not get stuck)

- **App will not start** (GTK/webview under Xvfb is the known risk): try, in
  order, `GDK_BACKEND=x11`, a valid `XDG_RUNTIME_DIR`, `GSETTINGS_BACKEND=memory`,
  `NO_AT_BRIDGE=1`, `GTK_A11Y=none`, then a real (non-virtual) nested X server
  via `Xephyr`. If it still refuses after ~30 minutes of cumulative effort,
  switch that surface to SOURCE-web evidence (upstream repo, docs, changelog,
  help articles, user guides — cite each), tag everything accordingly, and
  continue with the rest. Record the failure in `RUNLOG.md` and
  `open-questions.md`. Do not silently substitute and do not claim observation
  you did not make.
- **Sign-in required surfaces**: never create accounts or use real credentials.
  Document the flow from SOURCE (auth endpoints, state machine, stored tokens —
  all `path:line` cited) and OBSERVED-what-you-can-see-unauthenticated.
- **Network blocks / CAPTCHAs / rate limits**: cache responses, back off, fall
  back to source; never scrape aggressively.
- **App version drift**: if the installed binary's UI disagrees with the source
  checkout, pin both to the same release tag where possible; otherwise document
  both and trust OBSERVED for current layout, SOURCE for behavior/defaults.

## 11. Priority order if the horizon runs out

1. Settings (global + per-instance) — the owner's top complaint is missing
   settings rows. Complete, with defaults and effects.
2. Tabs & navigation inventory — every tab and what lives on it.
3. Add-instance / install / update flows (the daily loops).
4. Library & instance management (context menus, states).
5. Launch/runtime states and error surfaces.
6. Accounts, updates/maintenance, cross-cutting polish.

Slice the work so each number is independently delivered and documented when you
stop; partial higher numbers are fine, undocumented ones are not.

## 12. Final report (what the owner reads first)

`FINAL-REPORT.md`, ≤2 pages: app version + source commit studied; the nav tree
as observed; the top 10 capabilities our app is missing, mapped to rewrite
modules; the 5 most instructive UX patterns worth adopting (with why); what
could not be verified and why; the screenshot set to look at first. Then stop.
Do not implement anything. Do not commit anything.

=== END PROMPT ===

## Notes for the owner

- Deliverable location is fixed: `/PalantirMC/docs/modrinth-research/` (§9 of
  the prompt). Screenshots live beside the docs so the implementing agent can
  open them directly; committing that tree is your call afterwards.
- The prompt embeds the clean-room rules (upstream's per-package licences and
  branding restrictions, `SOURCE`-citation discipline, no code/assets into our
  trees), so the research cannot contaminate the rewrite's licence audit.
- It maps findings onto the rewrite crates (`palantir-desktop`, `palantir-net`,
  `palantir-loader`, `palantir-bedrock`, `palantir-core`) in `gap-analysis.md`
  and `roadmap.md`, so the follow-up implementation session can start from
  acceptance-criteria'd slices instead of re-researching.
- Environment facts baked in so the agent does not rediscover them: private Xvfb
  per worker (`systemd-run`, no `nohup`, PATH caveat), xdotool + ImageMagick +
  tesseract observed-control-only driving per `rewrite/tools/ui_smoke.py`, the
  known GTK-under-Xvfb failure with its degradation path, and no accounts /
  no touching the `:0` demo instance.
