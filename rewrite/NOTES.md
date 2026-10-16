# NOTES — the clean-room tree's engineering record

Decisions taken while building PalantirMC 2.0, and the state of its phases.
This file records *our* choices with the reason for each; numbers are either
design decisions (written down here) or measurements (with the command that
produced them). Estimates are labelled as estimates until a measurement
replaces them.

## Phases

| Phase | Content | Status |
| --- | --- | --- |
| 0 | Workspace, CI, `tools/licence_audit.py`, notices, licence placeholder | **landed** 2026-10-08: 8 tests green, audit 6/6, self-test catches all 5 planted failures |
| 1 | `palantir-core`: version JSON, rules, libraries, asset index, launch arguments, data-root layout | **landed** 2026-10-08: 60 tests green, round-trips three real version documents + manifest + 2 asset indexes |
| 2 | `palantir-net`: pooled client, scheduler, resumable downloads, metadata cache, content store | **landed** 2026-10-08: 83 offline tests green (mock server) + 2 live proofs: real 1.5.2 synced end to end (9 libraries, 2 natives, 749 assets, every file hash-verified) and a real interrupted jar resumed from its 256 KB mark |
| 3 | `palantir-loader`: vanilla, Fabric, Forge, NeoForge, Quilt; `.mrpack` and Prism importers | **landed** 2026-10-09: inheritance merge, install, Java runtimes, all four importers, launch plans, the Forge processor runner, and the done-when live test -- all five loaders install into a temp root and launch headless Java against their own document's runtime (5/5 green live; processors 6/6 with receipts verified; every game process watched running to its 20 s grace) |
| 4 | Theme + shell: palette, rail, title bar, page pane, right panel | **native UI checkpoint** 2026-10-10: original Field Notes design, navigation, native decorations, adaptive activity notebook, three accents and densities; Linux window smoke verified, Windows acceptance remains |
| 5 | Instances: create, launch, kill, logs, delete | **native UI checkpoint** 2026-10-10, Forge family added 2026-10-12: five detail tabs, create/import/rename/confirmed deletion, content toggling, managed vanilla/Fabric/Quilt/Forge/NeoForge launch and stop/log workers (the Forge family installs from the vendors' own installer jars and runs their processor pipelines with the managed runtime); actual-window file flows verified; real-game desktop acceptance verified 2026-10-15 (`tools/play_live.py`: vanilla 1.5.2 created and played with offline identity in the actual window, the managed runtime's java watched running 20s); Windows interactive acceptance remains |
| 6 | Discover: search, filters, project page, install | **native UI checkpoint** 2026-10-10: live search/type/order/version filters, pagination, native project notes and stopped-collection install selection; actual-window live search/detail/dependency install verified; reviewed local `.mrpack` optional selection is connected; catalogue-to-new-pack download verified 2026-10-15 in the actual window against the live catalogue (`tools/pack_live.py`, 26 pack files landed in a brand-new collection) |
| 7 | Settings modal, Home, About, error surfaces | **native UI checkpoint** 2026-10-10: six preferences panes, persistent accent/density/player/resources, Home/About, account status and modal errors; Linux window flows verified; additional locales landed 2026-10-15 (en/de/fr/ja catalogues with compiler-enforced complete coverage, selectable in the Language pane); live Microsoft native-client acceptance remains |
| 8 | Audit green, own licence chosen, notices complete, first Release tagged | not started (estimate ~3–5 h) |

The phase list and its estimates come from the rewrite specification
(`docs/superpowers/specs/2026-10-08-clean-room-own-licence-rewrite.md` §6);
phases 1–3 are the largest block of new work. All hours are estimates.

## Offline-mode acceptance — Java and Bedrock, 2026-10-16

Asked whether *every* game path had been checked, the answer was no, and the
gap named was offline-mode launching for both editions. What exists and what
was missing:

- **Java offline launch was already implemented** — `LaunchMode::Offline`
  hands the launch the selected offline account's name and derived uuid with
  access token `0` and user type `legacy`, and needs no session
  (`backend.rs::prepare_and_launch`); it is the *Microsoft* arm that refuses
  and never falls back (`a_microsoft_launch_decides_from_the_stored_session_and_never_falls_back`).
  Re-verified live twice on 2026-10-16: the loader suite's vanilla launch (the
  real game installed into a temp root, spawned, watched 20 s) and
  `tools/play_live.py` (the actual window, *Play with offline identity*, 762
  files downloaded, the managed `jre-legacy` java watched on the real 1.5.2
  classpath for 20 s). Both reproducible.
- **Bedrock launching carries no launcher session by design**: `windows.rs::launch`
  is a shell activation of the installed package (`activate_command`), so
  there is no offline *mode* to add — the game owns its Xbox sign-in. What was
  genuinely missing is that the real Windows branch had **no test at all**:
  only the non-Windows stubs were covered, so `#[cfg(windows)]` compiled on CI
  and was never run. Added
  `windows_tests::the_real_enquiry_answers_without_a_store_session`, which runs
  the real, read-only `Get-AppxPackage` enquiry with no Store or Xbox session
  and asserts every package it returns is one this launcher can address. It
  runs in the existing `windows-latest` gates job; a runner with no Bedrock
  answers with an empty list, which is the honest result, so the test asserts
  the enquiry *answered*, never that the game is present. Shell activation is
  deliberately not called in a test — `start` is asynchronous and can raise an
  "open with" prompt.

**Harness requirement discovered:** `tools/play_live.py` sizes its window to
1280×1500, so it needs a *tall* private display — this session's `Xvfb :95` and
`:97` at 1920×2600 satisfy it. On the 1706×848 desktop display the window is
clamped to 820 high and the accent-filled primary buttons stop reading in every
OCR pass (3x/4x at psm 6 and 11), so the harness fails at its first control
while the app itself is fine. Recorded so a future run on a short display is
not mistaken for a product fault.

**Still not automated:** Bedrock's actual *launch* (shell activation of a real
package) needs an entitled Windows machine by hand; only its enquiry is
automated now. The modpack-import paths remain fixture-driven — a real pack is
installed through `pack_live.py`, but a genuine CurseForge/Prism archetype has
not been imported and launched end to end.

## Live acceptance — the real game from the desktop Play button, 2026-10-15

Phase 5's last gap closed the same way as the pack flow: the actual window,
driven for real. `tools/play_live.py` created a vanilla 1.5.2 collection
(`world-1792093584066458666`), pressed *Play with offline identity*, and
watched the game's own process for 20 s -- the same bar as the loader
suite's live launches, but arrived at through the desktop. The first Play
downloaded 762 files (the game jar, its libraries, the pre-1.6 asset
index, and the managed `jre-legacy` runtime) before spawning; the watched
process is that runtime's `java` on the real 1.5.2 classpath.

The first attempt at this acceptance **passed on a false positive**: the
process rule asked for the substring "java" plus this run's data path, and
matched the *observer's own shell* -- a poll command that merely echoed the
word. The rule is now argv0-is-java plus a Minecraft main class token among
the arguments (1.5.2's document names
`net.minecraft.launchwrapper.Launch`; modern ones name
`net.minecraft.client.main.Main`), which no bystander can satisfy. A
passing assertion must name its subject; "something matched" is not
receipt evidence.

## Live acceptance — catalogue-to-new-pack download, 2026-10-15

The work order's last Discover gap closed by driving the actual window
against the live catalogue: `tools/pack_live.py` searched for *Simply
Optimized*, opened the top result's modpack (**Simply Optimized Reloaded**,
NeoForge 26.1.2), chose a release, configured the new collection, reviewed
the downloaded `.mrpack`, and installed it. The acceptance is on disk, not
in the click log: a brand-new collection `world-1792092219875021113` holds
28 files (26 under `game/`) -- the pack's indexed content and its overrides --
while the verified archive stayed in the shared `pack-archives/` cache.

Three findings the driver turned up along the way, all recorded in the
script's own comments:

- the Discover type chips default to **Mods**, so a modpack query under them
  legitimately answers "No matching additions"; choosing *Modpacks* is part
  of the route, not a workaround.
- the accent-filled primary buttons' 14px labels drop out of tesseract's
  psm 3 and psm 11 whole-page passes entirely; a psm 6 pass reads them
  ("Add to a collection..." at x=848 y=321 reads clean at 4x/psm 6).
- multi-word phrases must be matched within one OCR pass's reading order;
  merging passes first interleaves duplicate tokens and breaks every phrase.

## UI continuation — minimum-size defect pass, 2026-10-15

The work order's ship-set is in (passes recorded below and in the root
`UI-HANDOFF.md`); this pass was defect-only, driven by actual-window captures
at the supported minimum sizes (840×600, 1024×576, 1280×576, and the
1024/1280/1600-wide sweep). Four defects, each fixed at one site with its
measurement in the comment there:

- The library card's action cluster squeezed the name column into one row of
  pills — long names wrapped to five lines at the minimum window size. The
  cluster now stacks through `App::actions` at narrow effective widths
  (width / scale < 1100), and the name keeps its column.
- The instance header's Actions/Play cluster had the same squeeze beside the
  title; at 840×600, 1024×576 and 1280×576 the captures showed the row
  collapsing into the tab picker, which then rendered as an unlabelled
  Overview control where the cluster should be. Same helper, same threshold.
- The sidebar lockup wrapped its tagline mid-phrase at the compact width, and
  iced's scrollbar drew over the wordmark and the buttons' right edges (iced
  draws a scrollable's bar over its content and reserves no space for it).
  The mark/gap/tagline take tighter sizes below 1100 effective, and the
  sidebar's padding now lives *inside* the scrollable, so the bar draws over
  padding instead of the lockup.
- The same missing gutter let a panel body's bar cover card borders;
  `panel_shell` gives its rows, notice and body one 14px gutter against the
  10px bar, so right edges match with or without it.

Verification: `tools/ui_smoke.py` actual-window runs at the probed sizes
(OCR over settled states, fuzzy within two character edits) and the gallery's
offscreen per-screen rendering. Capture evidence is this-machine-only, in
`/tmp/minfix/` (transient). No game was launched; real-game desktop
acceptance and Windows interactive acceptance remain unclaimed.

## UI continuation — research-driven settings slices, 2026-10-14 (second pass)

The research agent's blueprint landed under `docs/modrinth-research/` and its
`roadmap.md` was consumed as the work order (its "ship 0, 1, 3, 4, 9" rule).
All five shipped: 0, 1 and 3 in the first record below, 4 and 9 in the second.

- **Slice 0 — settings store.** `Preferences` gains a schema `version`
  (0 = pre-schema file) and `model::migrate` on load: older files declare the
  current version (every missing field arrives through serde defaults), a file
  from a *newer* build is refused by name instead of guessed at. Round-trip
  test sets every field away from default and compares struct equality.
  Deviation from the roadmap's letter, recorded: the store stays in
  `palantir-desktop::model` rather than moving to `palantir-core` — app settings
  in the engine crate would drag UI concerns into the engine; the acceptance
  criteria (one defaults home, additive fields, round-trip, asserted version)
  all hold where it lives.
- **Slice 1 — Features section.** A `Features` settings pane with five
  toggles that hide instance tabs (Overview/Content/Worlds/Logs/Manage).
  `InstanceTab::shown` is the single predicate behind the strip, the narrow
  picker, and the selection guard, so a hidden tab cannot surface through any
  of them; hiding the active tab falls back to the first visible one, and
  hiding all five renders an honest empty state pointing at Preferences.
  Default-on everywhere. Tabs not built yet (files, screenshots) are named in
  the struct comment under the same `show_*` convention — deliberately no dead
  toggles for surfaces that do not exist.
- **Slice 3 — launch args / env / hooks.** New `palantir-loader::hooks`:
  `command_words`/`quote_words` (shell-quoter pair; the reference's v1
  whitespace-split quoting trap cannot occur because parsing is real),
  `env_pairs` (split at the *first* `=`, order preserved), `Hooks`
  (pre-launch / wrapper / post-exit) and `LaunchCustom`. Launch wiring:
  pre-launch runs to completion and a failure stops the launch by name; the
  wrapper becomes the program with java folded after it (`gamemoderun java …`);
  env + extra args apply in stored order after the plan's own arguments; the
  post-exit hook fires on clean exit, crash, and monitoring failure alike,
  with failures reported as events. All hooks execute directly — no shell ever
  sees them. `hooks_offline.rs` pins the acceptance criteria with mock
  processes: argv order, `=` in env values, a hook path containing a space
  (the quoting regression), wrapper folding, and visible hook failures.

The launch-settings dialog gained the customs fields (args, env, three hooks)
and the app now issues `text_input::focus("dialog-first")` for the two
per-collection dialogs exactly as create/rename get — without it, typed input
had nowhere to land (found by the smoke, not by a test).

The third pass of this slice set closed the two that remained:

- **Slice 4 — Java installation management.** Preferences' Java & player pane
  gains a "Java installations" roster; each entry carries the version line its
  probe reported plus the path. "Add a Java executable…" opens a path dialog
  and sends `Request::JavaProbe`: the candidate is asked for its version before
  anything joins the list, and a refusal names the path ("…is gone") and
  leaves the roster untouched — the smoke's mock runtime proves both
  directions on a real window. "Use for launches" sets the launcher's runtime
  (the active entry reads "In use"); "Remove from list" drops the entry and
  clears the selection when it *was* that runtime, and never deletes files on
  disk — the roster is a list of paths, not a package manager. Acceptance
  criteria, each checked: an added runtime appears and drives launches and a
  removed one is no longer selectable
  (`java_roster_probes_a_runtime_and_removes_it_again`); a stale collection
  override naming a removed runtime refuses the launch *by name* before
  anything happens (`model::ensure_java`, called from launch prep in
  `backend.rs`) with a message that says where to go next; per-instance
  override beats the launcher's runtime and clearing it restores the global
  (`model::effective_java`, pinned in the model tests).
- **Slice 9 — instance actions and the destructive inventory.** The reference
  serves its set as a context menu; the roadmap's own rule is that every action
  needs a keyboard-reachable path, "not just a right-click" — so the set is a
  dialog of plain buttons: "Actions" on the library card and the instance
  header opens `Dialog::InstanceActions` with Open collection, Play, Rename,
  Duplicate, Back up or export a world…, Open game folder, and Delete
  collection…. Every entry is Tab-and-Enter reachable and nothing hides behind
  a right-click only. Deviation from the roadmap's letter, recorded at the
  owner's decision: "export" wires to the world backup/export surface (the
  collection opens on its Worlds tab) rather than an instance exporter — the
  engine has no whole-collection export and one was not built for this slice;
  the menu says so in place. Delete keeps the confirmation that names what
  goes away and whose receipt lands in the open dialog; while a session or
  task runs, the entry becomes "Stop current work before deleting", matching
  the Manage tab's rule. `the_actions_menu_is_the_complete_keyboard_path_for_one_collection`
  pins the whole set, its order, and that delete goes through its confirmation
  first.

Three defects this pass's extended smoke exposed, all fixed:

1. **The probe receipt dumped the user out of Preferences.** The Java-add
   dialog had *replaced* `Dialog::Preferences` in the single dialog slot, so
   `JavaProbed` clearing the slot landed the user on the page behind — the
   smoke's `click("Behaviour")` after an add failed for this reason. The
   receipt now returns to Preferences on the Java tab (an add is a step of
   that visit, not a way out of it); a probe answer for a dialog that has
   since closed is ignored.
2. **Immediate saves could roll back on close.** Roster and account changes
   write to disk at once, but `saved_preferences` advanced only on the dialog
   save, so closing Preferences reverted the visible state to a baseline that
   predates the add. `save_preferences_now` now keeps the exact snapshot sent
   as `pending_preferences`, and the write's receipt promotes it to the
   confirmed baseline; a revert goes to that snapshot, never to an older one.
3. **The save receipt buried the action's own.** The worker's generic
   "Preferences saved." replaced "Java runtime added to the list." in the
   single note slot the moment the write landed. The save now carries the
   receipt it must leave on screen (`pending_note`); the generic confirmation
   is only the fallback for a plain dialog save.

**Operational rule from the owner: never drive windows or keystrokes on the
owner's working display.** An earlier smoke debugging pass in this session ran
against the ambient `DISPLAY` (the owner's `:0`) before the rule was set; no
further run did. Every verified run is on a private Xvfb on a display number
the owner is not using — `:101` for the first pass, `:103` (`setsid nohup
Xvfb :103 -screen 0 1920x1080x24 -nolisten tcp`) for the later ones. The
owner's correction to the wording is the rule itself: not "use :101", but
"use a display the owner is not using". It lives in `tools/ui_smoke.py`'s
header, not just in this note.

Verified at the third pass's end: 349 workspace tests pass (0 failed; the
live suites stay ignored), `cargo fmt --all -- --check` and workspace Clippy
`--locked -- -D warnings` clean. The extended `tools/ui_smoke.py` passes twice
consecutively on `:103`: world export byte-verified, overwrite refusal,
keyboard flows, 840×600 byte-exact import, the Window pane (854×480 saved; a
half-filled pair refused by name), Features (Worlds tab hidden — verified
absent from the tab strip with the nav band excluded — and restored), the
launch-settings dialog (2048 MiB override saved, verified from the stored
summary the page then shows, and cleared back to the launcher default),
duplicate-with-name, the Java roster on a real window (a dead path refused by
name, then the mock runtime added under its reported version, selected for
launches, and removed until the empty state returns), and the per-collection
actions menu (opened, its export and delete entries observed, delete's
confirmation naming what goes away before it is closed without deleting).
Driver fixes made along the way: OCR reads a magnified copy of the same
capture with boxes scaled back (14px primary labels were below tesseract's
read size) — 3× garbled short button words ("Actions" came out "ae"), so the
scale is now 4×, and `words()` unions two tesseract passes (default +
sparse) because the two mis-segment different words (the settings rail's
"Features" reads "ean eee" in the default pass alone); one button is clicked
by the half of its label OCR can hold ("and review" for "Resolve and review
files" — "Resolve" persistently reads "Recnive", same precedent as
`click("jar or")`); the click recovery alternates scroll direction inside the
window; `activate(steps)` crosses multi-field dialogs to reach below-fold
primary actions; and the home card is looked for over several captures, with
the miss reporting what was actually seen rather than a bare StopIteration.

## UI continuation — window settings and per-collection launch settings, 2026-10-14

The owner asked for the Modrinth App's missing tabs, settings and functions and
assigned the research pass to a separate agent (its mission brief is
`MODRINTH-RESEARCH-PROMPT.md`; its blueprint will land under
`docs/modrinth-research/`). This continuation closes implementation gaps that
need no research: the settings rows Modrinth has and we did not.

**Window resolution is now a real function, not a missing row.** Preferences
gain a **Window** pane (width/height, blank = the game's own default) and each
collection's Manage pane gains a **Game window** section with its own override
dialog and a "Use launcher default" clear. Precedence is collection → launcher
→ game default, decided in `model::effective_resolution` so one rule covers
every caller.

Decisions and their reasons:

- A size is **both sides or neither**, 320–16384 px per side. A half-filled
  pair is refused at the field, before persistence: a launch command must never
  carry a size the player did not actually choose.
- **Blank means unset**, not zero. The drafts render unset as blank
  (`draft_side`) and the saved indicator compares *parsed* values, so "0" vs
  "" cannot read as unsaved work.
- The engine already had the format's contract: `--width/--height` sit behind
  the `has_custom_resolution` feature rule and are filled from
  `${resolution_width}`/`${resolution_height}` placeholders. The launch opens
  that gate **only** when a size is set and fills both placeholders
  (`backend.rs` `prepare_and_launch`); with no size, no flag reaches the game.
  That contract is pinned at the engine boundary by
  `palantir-core/tests/launch.rs::custom_resolution_is_gated_and_filled_by_the_launch`
  using the real 26.3 document shape.
- Per-collection state is `Instance.window: Option<WindowOverride>` with
  serde defaults, so every pre-existing `instance.json` and `preferences.json`
  loads unchanged (`model::tests::older_files_gain_window_defaults`). Stored
  values are re-validated at launch: a hand-edited descriptor cannot smuggle an
  impossible size into a command.
- The collection dialog saves through a worker request keyed `window:{id}`;
  the receipt closes exactly that dialog, mirroring the rename flow. Reopening
  the dialog prefills from the stored override; blank drafts save `None`.

**Per-collection launch settings** follow the same shape: Manage gains a
**Launch settings** section with a memory budget override (512–32768 MiB) and
a Java executable override, each blank meaning "follow the launcher". The
precedence rule again lives in the model (`effective_memory`, `effective_java`):
collection → launcher preference → the managed runtime the game's metadata
names. The launch command's `-Xmx` and executable come from those two functions
only. A runtime path is executed directly and never through a shell, so a
control character in a stored path is refused (`validate_java_override`), and
stored overrides are re-validated at launch exactly like window sizes. The
dialog saves through a `runtime:{id}`-keyed request with the same receipt,
prefill and "Use launcher defaults" clear.

Verified: 329 workspace tests pass (0 failed, 7 live-ignored; +9 on the 320
baseline — window slice: 3 model, 1 engine contract, 2 desktop; launch-settings
slice: 2 model, 1 desktop), `cargo fmt --all -- --check` and workspace Clippy
`--locked -- -D warnings` clean, licence audit 6/6. The dialog prefills,
half-filled/invalid refusals, clears, and save-receipt guarding are desktop
tests; precedence and stale-storage validation are model tests; the
`--width/--height` gate is pinned at the engine boundary. The settings
tab-count expectation in `all_required_settings_and_instance_surfaces_are_named`
was updated 6 → 7 for the new pane (the test exists to name every surface, not
to freeze the count).

The actual-window smoke passes against the new binary (two consecutive runs:
world export byte-verified, overwrite refusal, keyboard help/activation,
840×600 byte-exact import), so the slices did not disturb the flows it covers;
the new settings wiring itself is compile-and-test covered, not smoke-covered.
One early smoke run flaked on an OCR miss of the Worlds tab: the driver's
`click` recovery scrolled downward only, which can carry a top control out of
view (and at 840×600 its click point sat below the window). The fallback now
alternates scroll direction inside the window; both runs after that pass.

## UI continuation — reviewed world and local-content operations, 2026-10-13

The owner resumed the selected UI model. Work stays in the rewrite; no legacy
frontend source/assets or new dependency/service was used. The existing Field
Notes palette, iced software renderer, original assets and JSON contracts remain.
New `palantir-desktop/src/local.rs` plus `local_tests.rs` connect world and content
reviews to the worker; `backend.rs`, `main.rs`, `bedrock.rs`, `surfaces.rs` and
`gallery.rs` carry worker/state/view integration. Launch remains
`cargo run -p palantir-desktop --bin PalantirMC` from `rewrite/`.

- Both editions make complete world copies (Java `.zip`, Bedrock `.mcworld`),
  not editable or selective snapshots. Review hashes every file and confirmation
  rechecks; existing outputs and outputs inside the world are refused. Output
  parents must already exist. Archives are streamed, then entry hashes and source
  receipts verified. Limits: 32 GiB, 100,000 files, 40 directory levels.
- Local Java mods/resources/shaders copy to the selected collection. Bedrock
  single-pack imports use the manifest's destination and UUID, extract checked
  paths, and optionally enable resource/behaviour packs for one reviewed world.
  Other activation entries remain unchanged. Missing selection means no world
  list write. Multi-pack containers, links, path escapes, case duplicates and
  Windows-reserved names are refused. File trust/compatibility is not certified.
- Reviews need the player to acknowledge closing the game; actual Java sessions
  and task conflicts are checked by both UI and worker. Bedrock is still not
  tracked, so the acknowledgment cannot prove the game is closed. Detection is
  refreshed before Bedrock review/confirmation rather than trusting UI paths.
- Bedrock world titles now call the engine reader. Verification found its prior
  big-endian-only reader did not handle actual Bedrock's eight-byte header and
  little-endian NBT; that path is fixed and pinned. Export now uses exclusive
  output creation and streams data; activation staging also uses exclusive
  creation so linked/stale temporary files cannot be truncated. Pack manifests
  are bounded to 1 MiB. `palantir-net::download::sha1_reader` reuses the existing
  digest implementation for streaming archive verification.
- The world-options toggle that implied incomplete snapshots was removed: both
  backup/export preserve the complete world. Loader review now displays the
  saved pin instead of accepting a draft pin that Play did not apply. No new
  shipped assets or preferences are introduced.

**Verified on final source:** workspace Clippy `--all-targets --locked --
-D warnings` clean; workspace tests 310 pass, 0 fail, 7 live ignored (71 desktop,
56 Bedrock); formatting and development build pass. Licence audit 6/6 and its
self-test pass. Initial checks caught a header-detection error and a complex
return type; both were repaired, not suppressed, and affected checks rerun.
Actual Linux-window acceptance exported a Java world with every archive entry
byte-matched to its unchanged source, refused an existing output, and imported
resource packs with independently matched bytes. The final binary repeated local
import at 840×600 with scroll/confirmation, F1 help, and Tab/Enter activation.
Captures/test data are `/tmp/palantir-ui-connected/`, not shipped artifacts.

**Final completion repair:** readable Bedrock titles and the empty-state warning
now share the same filter. Previously a title match could appear alongside
"No matching folders" because the warning checked only opaque folder names.
Added a regression and updated explicit gallery sample world metadata. Final
changed-crate Clippy, build and all 72 desktop tests pass; the unaffected suites
retain their recorded results (311 total passing tests, 7 live ignored). Actual
gallery-window interaction verifies title match versus no match without claiming
that sample data is a detected Windows installation.

**Limits:** Windows installed-game execution, Microsoft live credentials and
actual desktop game startup were not exercised. Bedrock export/extraction/
activation are proven with temporary files, not a Windows-owned live world.
Filesystem review is not a crash-atomic transaction and cannot prevent arbitrary
concurrent external replacement; close the game first. Pack activation failure
may leave the pack installed, with explicit instructions to enable it in-game.
Multi-pack `.mcaddon`, Preview discovery, entitlement pre-check, session tracking,
additional languages and final release acceptance remain. No commit, push,
account provisioning, publishing, tag or release occurred; unrelated legacy
changes remain untouched.

## Engine continuation — the five outstanding backend gaps, 2026-10-12

The owner paused frontend work and selected all five non-UI tasks from the
recorded gap list. Nothing here is a new product decision: each item closes a
hole this file already named, and no dependency, service account, asset or
licence choice was added. New code is `palantir-loader/src/forge.rs`,
`palantir-net/src/session.rs`, `palantir-bedrock/src/{worlds,content}.rs` and
their tests; the desktop's `backend.rs`/`model.rs`/UI copy and the rewrite CI
workflow changed to match.

**Forge and NeoForge install and launch for real.** `palantir_loader::forge`
reads the vendor's own Maven metadata (`maven-metadata.xml`) to list releases,
maps a version number back to its game (`1.20.1-47.4.26` -> `1.20.1`; the
NeoForge scheme `20.6.141` -> `1.20.6`, `21.0.167` -> `1.21`), fetches the
installer jar, installs the overlay document it carries, places the profile's
toolchain libraries and returns the processor plan. The desktop resolves the
runtime from the *installed* document and runs the processors before natives
extraction, reporting how many ran and how many were skipped on their receipts.
The old `desktop_supported()` guard is gone from the model and every call site.
A `base` seam (`installer_url_at`, `VendorInstall::base`) exists for the same
reason `profiles::LoaderSources::at` does: without it the installer fetch could
only ever reach the real vendor host, which would have left the whole function
with no offline coverage.

Reading the vendors' real documents rather than trusting the shape the code
was written from changed two things, both now pinned by tests. **Maven's
metadata is in deploy order, and neither vendor's is a release order:** Forge's
live file opens its 1.20.1 run descending (`47.4.5`, `47.4.4`, `47.4.3`, ...)
and thousands of lines later ends it ascending (`47.4.26`), while its 1.21 run
starts at `51.0.33` and descends — so "the last entry is the newest" would have
auto-installed `51.0.2` for a 1.21 collection and got 1.20.1 right by luck. The
list is compared numerically now. **And the game's ids moved to a
year-and-release scheme:** `26.1`, `26.2` and `26.3` are real ids in the version
manifest and `1.26` is not one, which NeoForge's leading pair follows, so its
mapping has a second branch instead of prefixed a `1.` that no longer exists.
Both were caught by checking the live files (recorded here as facts, not
estimates) and the desktop's automatic release choice is the caller that
noticed.

**Authenticated Java launch.** `palantir_net::session` stores a signed-in
session (`StoredSession`): atomic write, mode 0600 from creation on unix, a
symlinked or malformed file refused rather than read, and an expiry that is
decided against the stored instant (a session with no stated lifetime is
expired). The device-code answer now keeps the refresh token the
`offline_access` scope buys (`auth::TokenSet`), and `auth::refresh_session`
renews with it. A review set to the Microsoft account launches with that
session, renews once when it has expired, and **refuses** when it cannot -- it
never quietly falls back to the offline identity, which is the one claim this
half of the feature had to make.

**Bedrock worlds and content, engine side.** `worlds.rs` reads `level.dat` with
its own bounds-checked NBT reader (gzipped or raw, nesting and declared lengths
capped so a hostile file is an error and not a panic), lists worlds read-only
(an unreadable world is still listed, so a player never thinks it vanished) and
writes a byte-exact `.mcworld` copy without ever writing into the world folder.
`content.rs` reads a pack's own `manifest.json` to decide resource/behaviour/
skin (the file name cannot), refuses a fabricated uuid or an ambiguous
`.mcaddon`, and reads/updates a world's `world_resource_packs.json` /
`world_behavior_packs.json` atomically. The desktop does **not** call either yet;
the screens say so.

**`.mrpack` optional files.** `Request::Import` carries the review's selection,
where `None` means "not shown yet" (refused, nothing created) and an explicit
selection -- including an empty one -- installs, so declining every optional
file is a decision rather than an inability to proceed.

**Windows packaging.** The rewrite workflow gains a `package` job: on
`windows-latest`, after the gates, a `--locked` release build of the desktop,
an assertion that the binary exists and is above a size floor, and an artifact
upload. It deliberately does not tag or publish -- the product licence is the
owner's placeholder and phase 8 is owner-gated.

### Verification of this continuation

From `/PalantirMC/rewrite`: `cargo fmt --all -- --check` clean; workspace Clippy
with `--locked -- -D warnings` clean; **299 tests passed, 0 failed, 7 live
ignored**; licence audit **6/6** with all self-test scenarios passing. The two
new loader test files drive the *real* `forge::install` against the shared mock
vendor Maven, asserting the resolved document, the toolchain placement, the
packaged `data` file actually staged out of the jar, and the refusal path; the
desktop tests now move real bytes through `install_pack_archive` over a mock
server (which needed a test-only `Engine::at(root)` rather than the
environment-reading constructor) and prove the optional-file decision and the
hash-mismatch refusal; the net tests drive `refresh_session` over the mock.

Not verified here, and not claimed: a live install from either vendor's real
Maven (the ignored live tests are unchanged and were not run), a live Microsoft
sign-in (it needs an Azure client id only the owner can create), the Windows
AppX path (only `windows-latest` compiles it), and the actual desktop window --
the UI was paused, so no Xvfb smoke run was performed this session and the
new launch/session flows have compile-and-worker coverage only.

Remaining, in the order they now matter: wiring the desktop's Bedrock
world/pack screens to the engine modules above, Java world backup, authenticated
live acceptance with a registered client, Windows interactive acceptance, and
phase 8's licence and first Release (owner-gated). No commit, push, publish or
Release was performed by this continuation.

## UI continuation — dependency-aware file review, 2026-10-12

The next frontend slice connects Explore's required-dependency worker through
an exact-file review rather than deleting its old safety guard outright. Source
changes stay in `crates/palantir-desktop/src/{main,flows,model,backend}.rs`; the
README and UI hand-off carry the current status. No new dependency, framework,
asset, product licence choice, or service account was introduced. Existing
instance/preferences JSON schemas and Java/Bedrock separation remain unchanged.

Contract: `Request::ReviewContent(review_id, instance_id, project, release_id)`
returns `Event::ContentReview(review_id, Result<ContentPlan>)`. The receipt holds
collection/project/release ids, game, loader, collection name, and an ordered
list of destination folder/filename, URL, SHA-1, optional size, and verified-
existing flag. Every review/retry invalidates the prior request id; late results
cannot enable confirmation or reopen closed dialogs. The review screen displays
the exact files, dependency count, known download size, unknown-size disclosure,
and reuse/download state, with loading, retry, changed selection, and worker
failure states. Only a matching completed plan permits confirmation.

`Request::Content` now carries the reviewed plan. The worker resolves again and
compares before downloading anything. Matching regular existing files are
hash/size verified and retained; differing bytes, links, and duplicate destination
names are refused. Staging names include file indices to avoid same-hash staging
collisions. After downloads, destination state is checked again. Placement uses
exclusive file creation and rolls back only this operation's created files if
placement fails. This is not a crash-atomic transaction; interrupted transfers
can keep resumable partial bytes. Hash/size assertions were not weakened, and
the old test forbidding all dependencies was replaced by the requested rule:
unresolved reviews cannot install; resolved dependencies can, unless busy.

Final verification from `/PalantirMC/rewrite`: formatting, locked workspace
check, zero-warning workspace Clippy, desktop build, and **233 workspace tests
passed, 0 failed, 7 live ignored** (49 desktop). Audit **6/6** and all self-test
scenarios passed. New coverage includes required-only lookup, duplicate filenames,
service errors, hash/size reuse, changed receipts, commit rollback without
clobbering existing files, and stale/closed-dialog responses. The initial
borrow-check failure was repaired and every affected check rerun.

Actual-window evidence: live search → Sodium Extra project → 1.20.1 release →
Fabric collection selection → resolved three-file review → confirm. No jars
existed during review; after confirmation Sodium Extra (370701 bytes), Sodium
(971552 bytes), and Fabric API (2137232 bytes) all matched the catalogue's SHA-1
and size. Re-review showed **0 downloads**, and deleting a dependency after
review caused confirmation to refuse the changed plan before downloading; the
isolated file was restored afterward. The final review was exercised at 840×600.
Smoke OCR needed coordinate/settling corrections for filled buttons; corrected
flows passed with assertions intact. Evidence/data stay under
`/tmp/palantir-dependency-ui/`, not shipped assets. Those downloaded mods remain
user-content test data, not bundled code or dependencies of this launcher.

Remaining: archive/optional-file UI execution, Forge-family desktop coordination,
authenticated Java launch/session storage, Bedrock account/content/world features,
and Windows interactive acceptance. This install proof does not claim the mods
were launched in-game. No commit, push, publishing, or Release was performed.

## UI continuation — resumed 2026-10-12 (earlier checkpoint)

The owner explicitly resumed frontend work with the selected model. This slice
keeps the existing native iced/Field Notes stack rather than rebuilding the
shell. The sidebar's Java/Bedrock edition switch opens separate Java collection
and managed Bedrock surfaces. The switch is session-local, defaults to Java,
and leaves existing instance/preferences JSON contracts unchanged. Discovered
Windows packages are not stored as Java instances: an edition switch cannot
feed a Bedrock package through a Java loader or catalogue-content installer.
This deliberately avoids migrating `Instance.loader` to an optional field
before there is a persistent Bedrock descriptor to represent.

New source: `crates/palantir-desktop/src/bedrock.rs` (screen/account guidance and
read-only folder helpers). Changes also touch `main.rs`, `model.rs`, `flows.rs`,
`backend.rs`, the desktop manifest, and the rewrite lockfile. The desktop now
uses the existing workspace `palantir-bedrock` crate; no third-party dependency,
asset, copied material, or service account was added. Existing original artwork
and shared keyboard-focusable controls are reused. The README and root UI
hand-off document the resumption and exact supported behaviour.

Connected actions: detect Release/Preview packages on a worker thread, request
shell activation, inspect existing world/resource/behaviour-pack directory
names, and open allowlisted detected folders. Actions re-detect the Windows
package before activation or folder use. Reads do not create missing folders,
follow linked content directories, parse/change LevelDB, edit pack activation,
or write game files. Preview's data path remains explicitly unknown. UI states
cover loading, no installed copies, platform/service failure, missing data, and
launch busy. Shell activation is reported as a request, never a tracked running
session or a fabricated PID.

Bedrock's account surface separates local launch from Windows/Store/Xbox
sign-in, with explicit entitlement/no-bypass wording. It does not offer a Java
device-code button, claim to switch the OS account, or claim an installed
package proves ownership. Java's sign-in copy was updated to describe the real
verification chain and disclose that Play is still offline; the current worker
verifies ownership/profile but does not retain a playable authenticated session.

Verification, on the final source from `/PalantirMC/rewrite`:
- `cargo fmt --all -- --check`, workspace check with `--locked`, workspace
  Clippy with `-D warnings`, and the development desktop build passed.
- `cargo test --workspace --locked`: **228 passed, 0 failed, 7 live ignored**;
  44 desktop tests including six new edition/state/read-only-file checks.
- Licence audit **6/6**, and all audit self-test scenarios passed.
- Actual final Linux-window flows under Xvfb: create a Java collection, switch
  Java → Bedrock → Java, verify its descriptor bytes are unchanged, inspect the
  real Windows-only error and account guidance, navigate Installed copies,
  and exercise 840×600/Large-density account/Escape/Tab/Enter edition navigation.
  Screenshots/OCR evidence and isolated data remain in `/tmp/palantir-ui-resume/`.
  Initial smoke automation needed display, coordinate, and worker-readiness
  corrections; corrected final window flows passed without changing assertions
  about application behaviour. `xdotool windowclose` destroys the X11 drawable
  rather than requesting a normal close and triggered an upstream winit panic
  during smoke teardown. A separate restart closed through `WM_DELETE_WINDOW`
  exited cleanly with an empty error log; force-destroy teardown is not claimed
  as graceful-close coverage. No game was launched by these window checks.

Limits: this is a local native-UI checkpoint, not Windows interactive acceptance
or proof of Bedrock entitlement/launch. Installed-copy layouts are exercised
through test-only events; live package discovery cannot succeed on Linux.
Bedrock account pre-checks, pack importing, world backup/export, Preview data,
and process tracking remain. The already-landed Java dependency and archive
worker APIs remain guarded off in the UI; optional choices are not sent to the
pack worker yet. Finish those as a focused review/install slice, then authenticated
Java session handling and real Windows acceptance. No push, publication, licence
selection, or Release was performed in this continuation.

## UI continuation — 2026-10-10

**Owner clarification: complete frontend, not only shell checkpoints.** The
follow-on frontend pass added live game-version selection, all five loader
identities with backend availability, loader pinning, source browsing and
read-only import review, `.mrpack` optional-file review, Microsoft device-code
frontend state handling, project description/releases/changelog tabs, release
filtering/load-more, author links, final compatibility/dependency install review,
confirmed addition removal, settings modal with save/reset/revert, remember-page
restoration, diagnostic copying, keyboard-focusable controls/selectors, and
three further original illustrations (`identity.svg`, `downloads.svg`,
`additions.svg`). New frontend modules: `src/controls.rs` and `src/flows.rs`.
No new dependency or developer-service account was introduced.

The frontend acceptance inventory is in the desktop README. Regression tests
include source review without writes, archive optional metadata, stale inspection
responses, save receipts not covering later edits, release compatibility and
required-dependency guards, and backend-driven authentication states. Real
window coverage includes version/loader selection, browser/import review,
settings save/reset/revert, keyboard button activation, responsive modal panes,
content removal confirmation and source preservation, live release/changelog
selection and a reviewed **real Fabric API single-file install** into an isolated
Fabric collection. The installed filename with `+` prompted filename validation
through the safe-local-name boundary rather than identifier validation.
Verification: 154 workspace tests passed (seven ignored live), 26 desktop tests
passed again after final repairs, zero-warning workspace Clippy, formatting/build,
and audit 6/6 plus self-test. Evidence: `/tmp/palantir-full-frontend/`.

Backend-only gaps remain Microsoft authentication/client registration, Forge-
family desktop processors, archive execution/dependency coordination and real-game
Windows acceptance. Corresponding frontend choices/review/unavailable surfaces
exist; no unavailable backend is represented by fake success, no sign-in code is
generated, and pack review does not write files. Authenticated intermediate states
are rendered from test-only events for coverage. These are not live auth proofs.
No release publication, push, CI watch or licence choice occurred.


The selected UI session produced `crates/palantir-desktop/`, a new native
application with its own source and three original SVG files (`portal.svg`,
`landscape.svg`, `collection.svg`). Navigation icon paths are original
source-authored geometry. All asset notices are recorded. No legacy frontend
implementation or assets were reused; unrelated pre-existing legacy formatting
churn remains untouched.

Run from `rewrite/`: `cargo run -p palantir-desktop --bin PalantirMC`.
`PALANTIR_DATA_DIR` selects an isolated development data root. Instance and
preferences schemas and explicit limitations are documented in the crate's
README. English is the only authored catalogue; most page copy is still
source-local, so a complete key extraction is follow-up work.

Checkpoint verification from `rewrite/`: full workspace tests passed
(146 passed, seven intentionally ignored live tests), including 18 desktop
contract/state/file tests; workspace Clippy with `-D warnings`, formatting,
workspace check, and the development desktop build passed. Licence audit
passed 6/6 and its self-test passed. The processor mock initially failed
intermittently with ETXTBSY: sibling test forks inherited writable script
handles before exec closed them. Serializing the fixture's creation-through-
processor-exit lifetime preserves the real spawn and receipt assertions;
50 repeated processor suites and 15 repeated workspace runs passed afterward.

Native interface smoke testing under Xvfb used the real window and isolated
data to create and rename a collection, visit all detail and preferences
tabs, validate/reject invalid memory, save player/memory/accent/density,
copy a game directory, verify source preservation and account-file exclusion,
disable/re-enable a mod, list a copied world, cancel and confirm deletion,
retrieve live catalogue search/project notes, and inspect install eligibility.
Small-window coverage includes 840x600 and Large density. Page navigation
now resets its scroll offset so a previously scrolled notebook cannot hide
a new page's title or controls. Captures, logs, and isolated user data stay
under `/tmp/palantir-ui-final/`, outside shipped source.

This is Linux native-window coverage, not Windows acceptance, a visual/legal
review, proof of real game launch through the desktop, or proof of a completed
live content installation. No CI watch, publish, push, or Release was performed.

Known gaps: Forge/NeoForge application coordination, Microsoft sign-in,
required-dependency and `.mrpack` resolution, optional file selection,
additional languages, full Markdown/link/table rendering, Windows packaging,
and actual real-game desktop acceptance. Directory import supports ordinary
game directories and Prism/MultiMC directories, not archives; supported
loader pins are retained. Unsupported installs fail before writing content.
No demo/mock projects masquerade as live results. The README records persistent
schemas, import resource limits and supported flows.

Next focused slice: verify a real desktop install/launch/stop end-to-end and
finish safe dependency/modpack coordination, then exercise the native UI on
Windows. Product licence and release remain owner-gated. Phase checkpoints
are not claims that every product done-when criterion has been met.

## Desktop worker wiring, 2026-10-11 (historical pre-verification record)

The pending note below records the worker's initial hand-off, not its current
verification state. The later backend session compiled/tested the final wiring
(222 tests, zero-warning Clippy, audit 6/6) and Rewrite CI succeeded on `a2d24b6`.
The 2026-10-12 continuation above rechecks the current workspace. Network/live
limitations and the UI guards noted there still apply.

`crates/palantir-desktop/src/backend.rs` was changed to use the library APIs the
previous commit landed but left unwired, and **nothing in this slice was
compiled or run**: it was written without a terminal, so the workspace has not
yet seen `cargo fmt`, `cargo check`, clippy or the tests.

- **Sign-in.** `Request::Authenticate` runs the real device-code chain
  (`palantir_net::auth`) on its own thread, taking the client id from
  `PALANTIR_MSA_CLIENT_ID`; with none set it still reports the feature
  unavailable and requests no code. `Request::CancelAuthentication` stops the
  wait through a stop flag held on the engine, and a cancelled attempt reports
  no result afterwards. No new `Event` variant was added — the states the UI
  already renders (requesting, device code, verifying, connected, expired,
  failed, cancelled) carry the whole flow — so `main.rs` needed no change.
- **Content.** A release's `required` dependencies are resolved with
  `palantir_loader::dependencies::resolve_plan` and installed with it: planned in
  full before the first fetch, staged and hash-verified, then renamed into place.
  A failure anywhere refuses the whole install and removes what it staged, and
  the completion note names the dependency count.
- **Packs.** A `.mrpack` import source is installed from its own index
  (`mrpack::plan_files` plus the archive's override layers, base-first so the
  client layer's copy is the one that lands). A pack carrying optional client
  files, or a pack file with no download and no override to carry it, is refused
  with a message naming what is missing rather than half-installed.
- **Tests added, all pure:** ownership-versus-network wording, a cancelled
  sign-in reporting nothing, the install note's counts, the release document the
  dependency walk reads, optional-pack detection, override precedence, pack path
  escapes, and hash/URL verification. The network paths cannot run here at all.

## Decisions

**2026-10-11 — the launcher grows a second edition, and its account modes
stop being symmetric.** The owner asked whether this launcher could cover
Bedrock as well as Java and then chose the largest answer: both editions,
each with an offline and an online mode. Java stays the engine already
built (version document, libraries, loaders, Modrinth, offline session
injection) and gains the online half it has been missing. Bedrock enters as
a *managed* second edition: we find the copy Microsoft installed, launch
it, and manage its content and worlds. We do not install it, do not
register versions, and do not ask the player to enable Developer Mode.
That pairing — a dual-edition shell over a managed, not installed, Bedrock
— is the owner's choice and it is the cheap half of the idea: the expensive
Bedrock machinery (package download, extraction, AppX registration,
per-instance isolation) buys version switching the official launcher
already does, while content and world management is the part nothing else
hands the player.

**2026-10-11 — two account modes per edition, and they do not mean the same
thing.** Java's offline mode is ours: an offline identity and a fabricated
session, no account touched — already shipped. Java's online mode will be a
Microsoft device-code flow with an ownership check whose session is
injected into the JVM; that needs an Azure application registration (a
client id for a native/device-code client) which only the owner can create,
so the frontend keeps reporting it unavailable until that id exists.
Bedrock has no injection point at all: entitlement and sign-in are enforced
by Windows/Xbox and by the game itself, which reads the OS Xbox identity
rather than anything a launcher passes. Bedrock's two modes are therefore
the launcher's behaviour, not credentials: *Offline* launches the game and
touches no account state; *Microsoft account* assists the Store/Xbox
sign-in and pre-checks entitlement before launching. The UI must say which
of those it is doing, because a toggle that looks identical and means two
different things is exactly the claim this tree has been careful not to
make.

**2026-10-11 — not doing: a no-account Bedrock launch.** The owner asked
for one; it is refused, and recorded so a later session does not re-propose
it. Two reasons, the first being the owner's own scheduling: with Bedrock
scoped to the *installed* copy we own no package to launch, so the only
route to a check-free start is patching or re-signing Microsoft's files —
the repack-and-re-sign scope declined in the same decision. The second is
the reason this repository exists: that check enforces a paid licence,
online play needs Xbox Live regardless (so the mode could never be
complete), and a circumvention would be the first artefact in this tree the
licence audit could not defend. Nothing was built for it, and nothing
should be.

**2026-10-11 — the Bedrock facts, researched rather than invented.**
LeviLauncher (GPL-3.0) already ships this shape, so its source is off-limits
under the rewrite's own rule and nothing here derives from it. Everything
below comes from XMCL's public Bedrock guide and Microsoft's own
documentation. Version packages come from Microsoft's own delivery CDN, so
we redistribute nothing and the audit's shipped-asset rule is untouched.
Registering a version with Windows AppX requires Developer Mode, which is
why it is out of scope here. Launching a packaged app and receiving its
process id is `IApplicationActivationManager::ActivateApplication`, so a
Bedrock session gets a real pid like a Java one. Bedrock keeps one global
data folder rather than one per instance, and it moved with 1.21.120 from the
UWP location
`%LOCALAPPDATA%\Packages\Microsoft.MinecraftUWP_8wekyb3d8bbwe\LocalState\games\com.mojang`
to the GDK layout `%APPDATA%\Minecraft Bedrock\...\games\com.mojang`, so
both must be probed. Worlds are
a modified LevelDB and are read-only to us: list, parse `level.dat`, back
up, export as `.mcworld`, and never write into the database. Content is
zips (`.mcpack`, `.mcaddon`, `.mcworld`) copied into
`resource_packs`/`behavior_packs`/`minecraftWorlds` plus the per-world
`world_resource_packs.json`/`world_behavior_packs.json` activation files.
Entitlement must be owned; Beta/Preview additionally needs Insider
registration.

**2026-10-11 — delivery shape for the edition work.** One `Edition` axis in
the desktop model — `Instance { edition, game_version, loader:
Option<Loader> }`, loader meaningful only for Java — then a
`palantir-bedrock` crate whose Windows-internal modules are detect, launch,
data-path resolution, worlds, content and account state, each behind a
trait so the pure logic unit-tests without a real package. Order: edition
axis and its tests first, then detect/launch, then data and worlds, then
content, then the account panel, then one Home listing both editions.
`palantir-loader` stays Java-only and its name is now generous in spirit;
renaming it would touch every import for no behavioural gain, so it stays
and this note is the record. **Verification limit, stated rather than
discovered later:** this machine is Linux, so `#[cfg(windows)]` Bedrock code
cannot even be compiled here. Bedrock work is verifiable through trait fakes
on the Linux runner and the real AppX path only on the `windows-latest` CI
job, with interactive launch, sign-in and content flows only on an entitled
Windows machine. Java-side work keeps the coverage it has. No Bedrock code
has been written yet.

**2026-10-11 — frontend work is paused by the owner.** The desktop crate and
its assets sit on disk uncommitted and unpushed, unchanged by the edition
decision above; resume from the desktop README's frontend inventory after
reading the 2026-10-11 entries, not before.

**2026-10-10 — selected UI model assigned phases 4–7.** The owner switched
models and explicitly instructed this session to implement the frontend.
The earlier protection remains for unassigned agents, not this assignment.
The owner delegated framework, colours, original SVG/brand art, and locale
choices. Work stays entirely in the rewrite; no legacy frontend is a base.

**2026-10-10 — frontend choices.** Rust + iced 0.12.1 (MIT), already used
by this repository as a framework, but sourced independently from its
registry package with no legacy code or patches. Native widgets integrate
directly with the Rust engine; no browser/IPC bridge is needed. Use the
software renderer to keep the dependency/build surface modest. Graphite
surfaces, warm copper primary actions, cool slate dividers, generous space,
and an original geometric portal mark distinguish our own design. Native
system fonts avoid shipping font binaries. English-first keyed copy with
an explicit fallback is the locale starting point; do not claim additional
translations exist. Original SVG assets are source files, not transcribed
geometry. Blocking network/install work runs on a dedicated worker and
process supervision uses its own thread; neither blocks the UI loop.

Session pace: generation-first; no CI watch or repeated broad check loops.
Essential local checks at a usable checkpoint protect against cascading
API mistakes. Unverified/unwired flows are recorded as drafts, never as
completed release features. Product licence and Release remain owner-gated.


**2026-10-09 — the live test earned its keep: five facts no offline
fixture carried.** The done-when run (`palantir-loader/tests/live.rs`)
failed against real services five times before going green, each failure
a fact about the format or the launch:

1. An *absolute* `data` value in an install profile (`/data/client.lzma`)
   names a file packaged **inside the installer jar** -- the vendor's
   build path, shipped whole. Planning stages the entry beside the jar
   (the vendor installer does the same into a temp directory that dies
   with it); both Forge and NeoForge binarypatcher runs FileNotFound
   until they did.
2. `${classpath_separator}` is real: the Forge family joins its module
   path (`-p`) with it. Unexpanded, the modules never load, `--add-opens`
   targets a module that does not exist, and the game dies in its
   initializer. It is in the table now.
3. `@jar` in a library name is Maven's *default* extension, so
   `group:artifact:version@jar` and the bare coordinate are one file.
   The merge identity said otherwise, NeoForge's overlay and its game
   both carried slf4j, and the game's own bootstrap refused the
   duplicate jar. The identity treats `@jar` as no annotation, and the
   classpath absorbs repeated entries the way the syncer's jobs do.
4. Pre-2016 documents speak `${auth_session}` and `${game_assets}`, and
   every pre-2018 launch needs the launcher's own `-Djava.library.path`
   beside the classpath it already supplied -- 1.5.2 was unlaunchable
   until both landed in the expansion table and `LaunchPlan::command`.
5. A launch context must carry a parseable player uuid: modern authlib
   parses `--uuid` before the window opens, and the empty default killed
   NeoForge's boot at `Main.main`.

The watch's acceptance rule, now encoded in `live.rs`: the spawned game
either survives its 20-second grace (killed by the test) or dies with
output that names the room -- the display layer -- and never the plan:
a missing class, a refused flag, an unexpanded placeholder, or a main
thread dying of anything else fails the test.

**2026-10-09 — phase 4 is a protected phase.** The theme + shell, and
with it every UI phase after it (5 instances, 6 Discover, 7
settings/Home/About), is not the coding agent's to start. The owner
will hand the UI to a different tool under an explicit instruction;
an agent that reaches for it because it is "next" would generate UI
files nobody asked it for. The protection covers what the frontend
*shows*, not only its code: symbols and icons, visible strings and
copy, palette values — frontend assets belong to the same hand-off.
That includes the theme crate's own values: `palantir-theme` exists
so later crates compile against fixed tokens, but its palette is
frontend-visible material like any other, and "values are editable"
there is the owner's tool's invitation, never an agent's. Recorded
in the phase table and in the `AGENTS.md` standing instructions so a
session restart cannot forget it.

**2026-10-09 — the install profile's receipts decide what runs.** A Forge
processor may not run at all: its `outputs` map is the vendor's own
skip receipt (produced artifact, promised SHA-1), so when the artifacts
already sit at their hashes the processor has run before and is skipped.
The runner enforces the other direction too: a processor that exits 0 but
leaves its promises unkept is a failure named after its jar -- the install
would otherwise fail much later, somewhere else.

**2026-10-09 — the launch plan expands placeholders itself.** The format
leaves `${auth_player_name}`, `${classpath}` and friends in the argument
lists; a plan that hands those to a process is not a plan. `build_launch_plan`
resolves the classpath from the version's own libraries (rules applied,
Maven layout under the library root) and expands every placeholder the
format defines in one table, so the invariant is checkable: once a plan is
built, no `${...}` survives into the command. The caller contributes only
the on-disk locations (`LaunchContext`) and the identity values. Pre-2018
versions carry no JVM list at all -- their single `minecraftArguments`
string is split on whitespace (the format's own separator) and `command()`
supplies the missing `-cp`. Loader profiles are overlays: planned after
`merged_with`, never raw.

**2026-10-08 — phase 1 tests run against real downloaded samples, and the
samples teach.** The fixtures are six public metadata documents (manifest,
three version JSONs across 2013–2026, two asset indexes), fetched 2026-10-08
and listed in `THIRD_PARTY_NOTICES.md` with their origin. They immediately
caught three things no synthetic test would have invented: a `value`-only
argument entry with no `rules` at all, the pre-2014 placeholders
`${auth_session}` and `${game_assets}`, and macOS running an *older* lwjgl
build than everyone else (`2.9.2` vs `2.9.4`) via the rule shapes' interplay.
The round-trip test is value-exact on purpose: a launcher that drops a field
it does not model corrupts a version document the first time it writes one
back.

**2026-10-08 — rule semantics read off the fixtures: last match wins,
default deny.** Real rule lists wanting a platform exclusion open with an
unconditional `{"action": "allow"}` (the lwjgl entries in 1.12.2), which
would be redundant under default-allow; allow-lists like `{"action":
"allow", "os": {"name": "osx"}}` carry no companion rule, which would leak
under default-allow. Both shapes are covered by tests against the fixture.

**2026-10-08 — `versionRange` is `[min, max)`.** Inferred from use: the 26.3
sample tunes the JVM two ways around one boundary (`min: 10.0.17134` for ZGC,
`max: 10.0.17134` for the G1 set), which partitions cleanly only with an
exclusive max. The alternative readings would claim the boundary twice. Swap
it if the specification says otherwise; `version.rs` and `rules.rs` say so at
the type.

**2026-10-08 — TLS is the OS certificate store, not a bundled root list.**
The client is the spec's choice (`reqwest`, blocking); its TLS backend is
rustls over `rustls-native-certs`. The bundled-root-lists feature would have
pulled `webpki-roots`, whose licence is weak share-alike -- the rewrite's
own rule (README: no share-alike anywhere in the tree or the graph) says no,
so the root-list feature is off and the notices say why.

**2026-10-08 — eight transfers in flight, backoff without jitter.**
The concurrency ceiling is global because the point is sockets in flight at
a service that throttles the greedy (8: enough to cover a round trip's idle
time, low enough to read as polite -- the number to lower if throttled).
Retry backoff is 250 ms doubling to 1 s over 3 retries, deterministic on
purpose: one launcher process with few retries needs no jitter, and a fixed
delay is testable.

**2026-10-08 — one fetch per content hash, many names.** An asset index
routinely names the same object twice, and version documents repeat whole
library entries (1.5.2 lists `jinput-platform` verbatim twice); two
workers fetching one hash raced on one part file and one store slot. The
Windows runner ended that race as a 0-byte part file and a failed sync
(Linux happened to survive it), which is where the repetition was found.
Jobs now all deduplicate by identity before scheduling -- by hash when the
metadata has one, by URL when it does not -- one fetch, every layout path
materialized from it. The report counts files (layout paths), so two
names for one hash in the hash-keyed object store are one file, while in
the resources layout the same two names are two.

**2026-10-08 — a resume receipt must be true.** `Transfer.resumed_from` is
the offset the winning attempt *actually* continued from: a server that
answers 200 to a range request replaced the partial, so the receipt says 0,
not the offset that was asked for. Wrong-hash bytes are deleted rather than
kept -- wrong bytes are worse than no bytes -- and the store verifies at the
door even when it already holds the hash, so a caller with wrong bytes
surfaces a bug instead of hiding behind a copy we trust.

**2026-10-08 — a `downloads` block is the complete list of what exists.**
The live end-to-end sync asked the repository for
`lwjgl-platform-2.9.0.jar` and got a 404: that library ships *only*
classifier jars, and its plain jar does not exist at all (checked by hand:
base 404, `natives-linux` 200). Resolution used to derive a base jar
whenever the download record was missing, turning a classifiers-only
library into a phantom file and a guaranteed failure. Now a document that
has `downloads` gets exactly what its records name -- artifact and
classifier alike -- and only name-only documents (older and mod-loader
ones) derive the Maven layout from the coordinate. The phase-1 fixture
suite had pinned the opposite as a comment ("whatever carries natives also
contributes its base jar"); the real 1.12.2 sample contradicts it
(`jinput-platform` and the macOS `lwjgl-platform:2.9.2` are
classifiers-only), so that assertion now checks every resolved jar against
the record that names it, plus the split pinned by name.

**2026-10-09 — an install is a resolve, then a sync; the written document
is the receipt.** Every loader install ends as a vanilla install: the
overlay document (a loader profile) is merged over what it inherits, the
*resolved* document is written to `versions/<id>/<id>.json`, and its needs
go through the same syncer as everything else. From that moment
`build_launch_plan` reads one document and cares nothing about how it got
there -- Forge/NeoForge installer documents enter at the same door.

**2026-10-09 — the Java runtime index is pinned by a content hash, and
the pin is a trap.** Its URL path *is* the hash of its content
(`.../v1/products/java-runtime/<hash>/all.json`), so the constant in
`java.rs` is a pinned pointer that goes stale when Mojang re-issues the
index -- a 404 for it means "update the pin", never "no runtimes". The
first attempt at this phase used a misremembered hash and read an S3 XML
error page as if it were the index; the fixtures now on disk are fetched
from the pin the launcher community records, and the parser refuses a
non-JSON body loudly.

**2026-10-09 — the Forge-family installer is planned before it runs.**
Both vendors' installers carry `install_profile.json` (`spec: 1`) and a
`version.json` overlay; the overlay installs through the ordinary door,
and the profile is a processor pipeline run as headless Java. The module
plans the pipeline -- sides filtered, every token expanded -- before
anything runs: `[coord]` is an artifact's Maven path, `{MARKER}` is the
side's `data` value (bracketed path or `'quoted literal'`), and
`{ROOT}`/`{INSTALLER}`/`{MINECRAFT_JAR}`/`{SIDE}` are the context.
Unknown tokens are an error naming the token: a literal `{PATCHED}` in a
processor argument would write a file called `{PATCHED}` and the failure
would surface somewhere else entirely. The fixtures are the vendors' own
installer documents, and they taught the forms: BINPATCH is a plain
literal, MCP_VERSION a quoted one, and one processor claims two output
artifacts as its skip receipt.

**2026-10-09 — every importer lands on one order form, and an unknown
loader is an error, not vanilla.** The pack formats all describe the
same install -- a game, a loader, files -- so `import.rs` is that shape
and each format is only a translation into it: `.mrpack`, Prism/MultiMC,
CurseForge's `manifest.json` (the CurseForge app, GDLauncher and
ATLauncher all share it), and the vanilla `.minecraft` layout the
official launcher, TLauncher, SKLauncher, Badlion, Legacy, Lunar and
Feather keep around their own roots. Two rules run through all of them:
pack-supplied paths are joined, never trusted (the `.mrpack` spec warns
about exactly this), and a dependency id this launcher does not know is
reported -- when it is the only loader it becomes `LoaderTarget::Unknown`
-- because installing a LiteLoader pack as vanilla is a silent wrong
install whose failure lands somewhere else entirely. CurseForge files
arrive as catalogue ids with no URL or name; they stay ids until the
catalogue answers, rather than being guessed into existence.

**2026-10-09 — the mock HTTP server is one crate, shared.** The
loader tests need the same fake service the transfer tests grew (range
answers, one-shot interruptions, request receipts); it lives in
`test-support`, a dev-dependency of every crate that tests against a
wire. Nothing from it ships.
Layout in `paths.rs`: versions own their jar and metadata, libraries and
asset objects are shared across versions (N versions must not mean N copies
of lwjgl), and everything resumable or regenerable lives under `cache/`.
Windows lands in `%LOCALAPPDATA%` rather than roaming: a game tree is
gigabytes and a roaming profile would carry it across the network at every
sign-in. Every path a metadata document names is joined, never trusted --
absolute paths and `..` are refused.

**2026-10-08 — this tree lives in `rewrite/`, as its own Cargo workspace.**
The rewrite is being built on this machine rather than elsewhere, so the
clean-room claim is protected structurally instead: the new code is a separate
workspace that the old tree's build never sees, no path dependency crosses
between them, and nothing from the old tree is read while writing here. When
the old tree is retired, this directory becomes the repository.

**2026-10-08 — the theme is its own crate, `palantir-theme`.**
The design tokens must be compile-checked and usable by every later crate
before the shell exists (phase 4), and a window-free crate is the only way to
test them at all. One file, `theme.rs`: roles fixed, values editable.

**2026-10-08 — `license = "UNLICENSED"` until phase 8.**
The licence is the last step, not the first: it is chosen only on a built tree
with a green audit, by the owner. The placeholder grants nobody anything.

**2026-10-08 — elevation is an ascending luminance ramp, and the spec's prose lost to its own table.**
The rewrite spec's rule text said "insets are darker", but its value table is
`BG #0B0B0C` → `BG_RAISED #131315` → `BG_INSET #1A1A1D` → `BORDER #26262A`, a
strictly ascending ramp, and the table names the uses — text fields, search
boxes, a hovered tab's plate — where the lighter well is the right design (a
hover plate darker than its bar reads as a hole). The spec fixes the roles and
makes the values editable, so where the two disagree the table is the design
and the rule is restated as what the values do: the window is the floor, every
step up is a lighter fill, hairlines separate. The values are unchanged.

**2026-10-08 — status colours lean away from the accent.**
`ACCENT` owns the orange corner of the wheel, so `WARN` is a gold (yellower
than any amber) and `DANGER` is a red darker than the accent: a warning must
never read as the brand and a destructive button must never read as the
primary action. `SUCCESS` is a mid green at matched luminance.
