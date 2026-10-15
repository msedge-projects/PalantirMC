# UI hand-off — start the selected model here

## Minimum-size defect pass (2026-10-15) — the last work of the work order

After the third pass, actual-window captures at the supported minimum sizes
(840×600, 1024×576, 1280×576, and a 1024/1280/1600-wide sweep) drove one
defect-only pass: the wordmark was clipped in the rail (its tagline wrapped
mid-phrase and iced's scrollbar drew over the lockup), panel pages drew under
the pane/rail boundary where the bar overwrote card borders, and the instance
header's action cluster collapsed into the tab picker at the smallest sizes.
Fixes are `App::actions`' narrow stacking (width / scale < 1100), the
sidebar's padding carried inside its scrollable, and `panel_shell`'s shared
14px gutter against iced's 10px bar — each commented at its site in
`main.rs` / `surfaces.rs`. Evidence captures live in `/tmp/minfix/` on the
working machine (transient); `rewrite/NOTES.md` has the full record.

## UI continuation — the work order's ship-set is complete (2026-10-14, third pass)

The roadmap's "ship 0, 1, 3, 4, 9" set is now fully implemented: this pass
landed the two that remained.

- **Slice 4 — Java installation management.** Preferences → Java & player has
  a **Java installations** roster. "Add a Java executable…" probes the
  candidate for its version before it joins the list (a dead path is refused
  by name and the roster stays untouched); entries show the reported version
  and offer **Use for launches** (the active one reads "In use") and **Remove
  from list** (clears the selection when it was that runtime; never deletes
  files on disk). Precedence is collection override → launcher runtime → the
  managed runtime the game's metadata names (`model::effective_java`), and a
  stale override naming a removed runtime refuses the launch *by name*
  (`model::ensure_java`) with a message pointing at Preferences/Manage.
- **Slice 9 — instance actions and the destructive inventory.** "Actions" on
  the library card and the instance header opens `Dialog::InstanceActions`:
  Open collection, Play, Rename, Duplicate, Back up or export a world…, Open
  game folder, Delete collection…. Every entry is a plain button, so every
  action has a keyboard path — nothing lives behind a right-click only (the
  roadmap's rule; our shape is a dialog, not a context menu). **Owner
  decision, recorded as a deviation:** "export" wires to the existing world
  backup/export surface (the collection opens on its Worlds tab); the engine
  has no whole-collection exporter and none was built for this slice — the
  menu says so in place. Delete keeps its named confirmation and in-dialog
  receipt; while a session or task runs it reads "Stop current work before
  deleting" like Manage's copy.

Three defects the extended smoke exposed and this pass fixed: the Java probe's
receipt used to dump the user out of Preferences (the add dialog had replaced
it in the single dialog slot — it now returns to the Java tab); immediate
roster/account saves could roll back when Preferences closed (the in-flight
snapshot is now the revert target and the confirmed baseline); and the save
receipt buried each action's own receipt in the single note slot (the save now
carries the receipt to leave on screen).

Gates from `/PalantirMC/rewrite`: 349 workspace tests pass (0 failed; live
suites ignored), `cargo fmt --all -- --check` clean, workspace Clippy
`--locked -- -D warnings` clean, licence audit + self-test clean.
`tools/ui_smoke.py` passes twice consecutively on a private Xvfb (`:103` —
never the owner's working display; the rule is in the script's header):
world export byte-verified, overwrite refusal, keyboard flows, 840×600
byte-exact import, Window pane save + half-filled refusal, Features
hide/restore, launch-settings save/clear, duplicate-with-name, the Java
roster (refused probe, mock runtime add/select/remove) and the actions menu
with its named delete confirmation. Driver fixes: OCR reads a 4×
magnification (3× garbled short button words) and unions two tesseract passes
(default + sparse, which mis-segment different words); one button is clicked
by the readable half of its label ("and review" for "Resolve and review
files"); the home card lookup retries across captures and reports what it saw
on a miss. Roadmap slices 5, 6, 10, 11 are the remaining unshipped work
(their own rule made them optional).

## Previous continuation — research-driven settings slices (2026-10-14, second pass)

The research agent's blueprint landed at `docs/modrinth-research/`; its
`roadmap.md` is now the work order. Implemented this pass (its slices 0, 1, 3
— 4 and 9 landed in the pass recorded above):

- **Settings store** with schema `version` + `migrate` on load (older files
  upgrade, newer-refused by name), one defaults home, field-additive serde,
  round-trip test over every field.
- **Features settings pane**: five toggles hiding instance tabs, one predicate
  (`InstanceTab::shown`) behind every appearance of a tab, stale selections
  fall back to the first visible tab, all-hidden renders an honest empty state.
  Default-on; no toggles for surfaces that do not exist yet.
- **Launch args / env / hooks** (`palantir-loader::hooks`): shell-quoted,
  directly-executed commands (the reference's quoting trap cannot occur),
  ordered env pairs split at the first `=`, pre-launch/wrapper/post-exit hooks
  wired into launch (pre-launch failure stops the launch; wrapper folds around
  java; post-exit fires on clean exit, crash, and monitor failure). Pinned by
  `hooks_offline.rs` with mock processes against the roadmap's acceptance
  criteria.
- The per-collection launch-settings dialog gained the customs fields and now
  focuses its first field on open like create/rename do (found by the smoke).

**Operational rule from the owner: never drive windows/keystrokes on the
owner's working display.** Verified runs use a private Xvfb (`:101`); the rule
is written into `tools/ui_smoke.py`'s header. An earlier debugging pass this
session ran on the ambient `:0` before the rule existed; nothing after it did.

Gates from `/PalantirMC/rewrite`: 341 workspace tests pass (0 failed, 7
live-ignored), `cargo fmt --all -- --check` clean, workspace Clippy
`--locked -- -D warnings` clean. `tools/ui_smoke.py` passes twice
consecutively on `:101` covering the world-export/import flows **and** the new
Window pane (incl. half-filled refusal), Features hide/restore, and the
launch-settings dialog save/clear — the last verified from the stored summary
the page shows, not from input pixels. Driver fixes: 3×-magnified OCR with
boxes scaled back (14px labels), alternating scroll recovery, `activate(steps)`
for below-fold primary actions.

## Previous continuation — window + per-collection launch settings (2026-10-14)

The owner launched the unattended Modrinth research agent from
`MODRINTH-RESEARCH-PROMPT.md`; its blueprint will land under
`docs/modrinth-research/`. While it runs, this continuation closed a gap that
needs no research: **window resolution settings**, global and per-collection,
fully wired into launch.

- Preferences gain a seventh pane, **Window** (width/height; blank = the game's
  own default). Collections gain a **Game window** section in Manage with an
  override dialog and "Use launcher default" clear.
- Precedence is collection → launcher → game default
  (`model::effective_resolution`). A size is both sides or neither, 320–16384
  px; a half-filled pair is refused before persistence.
- The launch opens the version format's `has_custom_resolution` gate only when
  a size is set and fills `${resolution_width}`/`${resolution_height}`; with no
  size, no flag reaches the game. Pinned at the engine boundary by
  `palantir-core/tests/launch.rs::custom_resolution_is_gated_and_filled_by_the_launch`.
- Old `instance.json` / `preferences.json` load unchanged (serde defaults,
  tested). Stored overrides are re-validated at launch.
- The dialog saves via a `window:{id}`-keyed worker receipt that closes exactly
  that dialog (rename-flow pattern); reopening prefills from the stored value.
- Collections also gain **Launch settings**: a memory-budget override and a
  Java executable override (blank = follow launcher; precedence collection →
  launcher → managed runtime via `model::effective_memory` / `effective_java`).
  Stored overrides are re-validated at launch; a runtime path with control
  characters is refused (executed directly, never via a shell). Saves through
  a `runtime:{id}`-keyed request with the same receipt/prefill/clear shape.

Gates from `/PalantirMC/rewrite`: 329 workspace tests pass (0 failed, 7
live-ignored; +9 on baseline), `cargo fmt --all -- --check` clean, workspace
Clippy `--locked -- -D warnings` clean, licence audit 6/6 + self-test. One
existing test's tab-count expectation moved 6 → 7 for the new pane.
`tools/ui_smoke.py` passes against the new binary (two consecutive runs): the
actual-window world export, overwrite refusal, keyboard help/activation and
840×600 byte-exact import all still work after these slices. The settings
wiring itself is compile-and-test covered, not smoke-covered. One early smoke
run flaked on an OCR miss of the Worlds tab; the driver's click fallback now
alternates scroll direction (a downward-only recovery could carry a top
control out of view) and both subsequent runs pass. Details in
`rewrite/NOTES.md`.

## Latest UI continuation — Modrinth-style browse UX and repeatable window smoke (2026-10-13)

The owner clarified the direction: **Modrinth's UX, not its UI** — familiar
browse patterns in our own code, our copper/matte palette, our sidebar, and
nothing copied (no Modrinth code, assets, or branding; only the common pattern
of "icon, name, meta, actions" lists). Explore now browses live: typing debounces
into a catalogue search (350 ms via the existing ticker), content types are
immediate filter chips, and results are scannable project cards carrying type,
author and download badges. Buttons are pill-shaped; the palette, sidebar and
copy are unchanged. A unit test pins the debounce contract (no early request;
explicit submit replaces the pending debounce).

A review-dialog gap was fixed: a confirmed operation's receipt used to land in
a banner hidden behind the modal scrim. The outcome now renders inside the open
review dialog (`local::State::outcome`, `Event::LocalApplied` — a dedicated
event so another task finishing under the same instance key can never be
mistaken for the receipt). Covered by a desktop test.

Multi-pack reviewed Bedrock add-on import is connected: folder bundles and
nested `.mcpack` members each follow their own manifest destination; duplicate
pack identities and existing destinations are refused before writing; world
pack-list activation entries are preserved.

**Repeatable actual-window regression** is `rewrite/tools/ui_smoke.py`. It
drives the real binary through xdotool on a private X11 display, observes every
control through screenshot OCR before clicking (no assumed coordinates), and
verifies artifacts rather than pictures: a Java world export whose zip matches
every source byte while the source stays untouched; the overwrite refusal with
the output unchanged; the in-dialog "Saved" receipt; F1 keyboard help and
Tab/Enter keyboard activation (with a beat after Tab — the toolkit moves focus
asynchronously); and a local resource-pack import at 840×600 whose installed
bytes match the source exactly. Window discovery falls back to the title when
`_NET_WM_PID` is unreliable. Run from `rewrite/`:
`python3 tools/ui_smoke.py --binary target/debug/PalantirMC` with xdotool,
ImageMagick `import` and tesseract available. Latest run: **PASS**.

Final checks: 81 desktop tests pass (including the receipt and debounce
tests), workspace Clippy clean, `cargo fmt` applied. The desktop was relaunched
on the owner's display with sample data for interactive review. Not claimed:
Windows Bedrock installed-game execution, live Microsoft credentials, release
publication. Changes remain local.

## Historical continuation — connected local operations (2026-10-13)

The owner resumed UI work. World and local-content placeholders are now connected
through worker-backed file plans in `rewrite/crates/palantir-desktop/src/local.rs`:
Java `.zip` / Bedrock `.mcworld` complete-world copies; local Java mods/resources/
shaders; manifest-directed single Bedrock pack extraction; optional per-world
activation preserving other entries. Reviews show exact paths, hashes and sizes,
refuse stale plans/overwrites/unsafe inputs, and require closing the game before
confirmation. Bedrock inspection reads actual little-endian title metadata;
unreadable metadata does not hide a world. The loader review shows the real saved
pin instead of an editable draft that Play ignored.

Final checks: 310 workspace tests passed; a final title-filter repair then passed
all 72 desktop tests, giving 311 across the unaffected workspace suites (56
Bedrock; 7 live ignored). Zero-warning Clippy, formatting/build, audit 6/6 and
self-test pass. The repaired Bedrock title/empty-state behavior was exercised
through the actual gallery window, separately from installed-game acceptance. Actual Linux UI
created a byte-exact Java world archive, refused overwrite, and imported local
resource packs with independently matched bytes. The final binary repeated an
import at 840×600, including scrolling, F1 and Tab/Enter. Evidence remains under
`/tmp/palantir-ui-connected/`. Full details and limitations are in the desktop
README and `rewrite/NOTES.md`. Windows Bedrock installed-game execution, live
Microsoft credentials and release publication are not claimed. Changes remain
local; unrelated legacy churn is untouched.

## Historical continuation — backend engines, UI paused (2026-10-12)

The owner paused frontend work and selected all five non-UI gaps from the
recorded list. The Forge family now installs from the vendors' own installer
jars and launches (including their headless-Java processor pipelines); a
Microsoft session is stored, renewed once and injected into a real launch, and
refuses rather than falling back to the offline identity; a `.mrpack`'s optional
files are carried from the review into the install; Bedrock gained read-only
world listing, `.mcworld` export, pack planning and per-world activation files
(engine only — the screens do not call them yet); and the rewrite CI gained a
Windows `package` job that builds, checks and uploads the desktop binary without
publishing it.

Gates from `/PalantirMC/rewrite`: 299 workspace tests pass (0 failed, 7 live
ignored), `cargo fmt --all -- --check` clean, workspace Clippy with `--locked --
-D warnings` clean, licence audit 6/6 plus self-test. No window run was made
this continuation, so the changed flows have compile-and-worker coverage rather
than new actual-window evidence. `rewrite/NOTES.md` holds the decisions,
what is verified, and what is still open.

## UI continuation — dependency-aware install review (2026-10-12)

Explore now resolves the exact release and all required dependencies in a
worker-backed file review before confirmation. The user sees destination names,
hashes, download totals, and verified existing files. Confirmation rechecks the
plan and refuses changed metadata or destinations before downloading. A real
Sodium Extra + Sodium + Fabric API install was exercised through the Linux
window, with all three SHA-1/size checks independently verified; re-review reuse,
changed-plan refusal, and 840×600 review were exercised too.

Final checks: 233 workspace tests passed, 0 failed, 7 live ignored (49 desktop),
zero-warning Clippy, format/check/build, audit 6/6 and self-test. Details are in
the desktop README and `rewrite/NOTES.md`. Next focused UI slice: safe archive
import with actual optional-choice hand-off; authenticated launch and Windows
interactive acceptance remain separate. Changes remain local, not published.

## Resumed by the owner (2026-10-12)

The owner reassigned UI work to the selected model. The dual-edition continuation
is implemented under `rewrite/crates/palantir-desktop/`: a session-local
Java/Bedrock switch, managed Bedrock installed-copy screen, read-only folder
inventory/actions, and edition-specific account guidance. See the desktop
README's **Dual-edition continuation** and `rewrite/NOTES.md` for verified scope
and remaining gaps. Java descriptors/preferences remain backwards-compatible;
Bedrock discovery is not persisted as Java collections. This continuation does
not publish a release or claim Windows interactive acceptance.

The 2026-10-11 pause below is historical; this reassignment supersedes it.
The prior UI checkpoint was subsequently committed with the backend work.

The owner has since settled a decision that changes what a resumed session
should build: the launcher is to carry two editions — **Java** (the full engine
plus offline and Microsoft-account modes) and **Bedrock** (managed only: detect,
launch, content and worlds; we do not install it, register versions or require
Developer Mode) — with two account modes per edition, and no licence-check
bypass. The reasoning, the researched Bedrock facts, the refusal and the
delivery order are recorded in `rewrite/NOTES.md` under the 2026-10-11
decisions entries. Read those before resuming UI work.

## Current checkpoint (2026-10-10)

**Latest clarification and frontend pass:** the owner asked for the entire
assets/UI/UX/frontend, not only core pages. Continue from the README's frontend
acceptance inventory. New flows include live game/loader choices, filesystem
source browsing and import/archive review, project release/changelog filtering,
final install review, Microsoft device-code state surfaces, settings modal
save/reset/revert, diagnostic copying, confirmed addition removal, and keyboard-
focusable action/selector controls. Assets now include identity, download and
addition illustrations with recorded provenance. Full checks: 154 tests pass
(26 desktop), zero-warning Clippy, formatting/build and audit 6/6. A real reviewed
Fabric API install was verified through the actual window. Backend-only limits
remain authentication, Forge-family coordination and archive/dependency execution;
their frontend controls/states exist without fake success. Latest evidence:
`/tmp/palantir-full-frontend/`. No push or publication occurred.

The paragraph below records the earlier checkpoint, not the latest test count.

The owner switched models and assigned this session to implement the UI.
The rewrite now has `rewrite/crates/palantir-desktop/`; the earlier statement
below that no frontend exists describes the pre-session baseline only.
Continue from `rewrite/crates/palantir-desktop/README.md` and the **UI
continuation** section of `rewrite/NOTES.md`, not by rebuilding the shell.
The framework/design choice is recorded there. The current native checkpoint
has five collection tabs, six preferences panes, safe directory import,
create/rename/delete dialogs, Explore project/install selection, and an adaptive
activity notebook. Full rewrite checks pass: 146 tests (18 desktop), zero-warning
Clippy, formatting/build, and licence audit 6/6 plus self-test. Actual Linux-window
flows and 840x600/Large-density navigation were verified, including restart
persistence; real-game/Windows acceptance and the documented integration gaps
remain. Evidence is under `/tmp/palantir-ui-final/`. No push, CI watch or release
publication was performed.


## Copy/paste start prompt

Start the PalantirMC 2.0 UI session now. Read `/PalantirMC/UI-HANDOFF.md` and
work only in `/PalantirMC/rewrite/`.

The owner explicitly assigns phases 4–7 to this selected UI model: shell,
Home, instances, Discover, settings, About, and error surfaces, including
visible copy, symbols, icons, original SVGs, illustrations, and branding.
This is the named assignment anticipated by the UI protection rule; it
is not permission for unrelated agents to start the protected work.

Choose the frontend technology, framework, colours, typography, geometry,
icon/brand-art direction, and locale approach yourself. The earlier Rust/iced
and orange-palette preferences are not binding on your frontend choice.
Keep the existing Rust launcher engine and document any bridge it needs.
Do not ask for approval of routine design choices. Record your choices and
one-line reasons in `rewrite/NOTES.md`, then start implementing immediately.

Use the session primarily for code and visible assets, especially SVGs.
Avoid broad research, repeated workspace builds, test/lint/audit loops, CI
monitoring, and idle polling. Defer broad verification to the follow-up
repair session; obey any higher-priority requirements for essential checks.
Do not push, publish, deploy, or release an unverified draft. Never describe
untested generation as tested, working, or release-ready.

Build coherent, connected slices rather than disconnected placeholders:
start with a launchable shell and shared components, then Home, instances,
Discover/project details, settings, About, and errors. Keep network, disk,
Java installation, and process work off the UI thread. Persist code and
assets on disk as you go, and maintain a short continuation record in
`rewrite/NOTES.md` describing decisions, implemented surfaces, missing
backend wiring, known issues, and deferred checks. Do not invent a session
deadline or promise all phases will fit in one session.

## Correct workspace boundary

- Repository root: `/PalantirMC`.
- **Implementation workspace: `/PalantirMC/rewrite`.** It has its own
  `Cargo.toml`, `Cargo.lock`, libraries, notices, and licence audit.
- `rewrite/Cargo.toml` currently lists `palantir-core`, `palantir-net`,
  `palantir-loader`, `palantir-theme`, and `test-support`.
- Before this session there was no frontend/app crate. The new
  `rewrite/crates/palantir-desktop` is now the original preview shell and
  application layer. Use its README and the continuation record for current
  capabilities and limitations; do not confuse a preview with a finished app.
- The top-level `crates/`, `tools/`, `vendor/`, and their existing frontend
  assets belong to the legacy implementation. Do not read, copy, adapt,
  import, or depend on them for the rewrite. Do not use their generated
  icon, locale, theme, or brand data.
- Do not touch `.local/`, `PARITY-HANDOFF.md`, or the existing formatting
  churn in the legacy `crates/` tree. They are not UI-session deliverables.

The previous version of this hand-off incorrectly directed work into the
legacy frontend and described it as the rewrite baseline. Those instructions
are withdrawn. A successful legacy build does not verify the rewrite.

## Read only the relevant starting context

1. This file and `rewrite/AGENTS.md` for the existing context and guard.
2. `rewrite/NOTES.md` for engine progress and your continuation record.
3. `rewrite/Cargo.toml` for the actual workspace and dependencies.
4. `rewrite/README.md` and `rewrite/THIRD_PARTY_NOTICES.md` for provenance.
5. The product specification at
   `docs/superpowers/specs/2026-10-08-clean-room-own-licence-rewrite.md`:
   use the functional scope, not its now-superseded frontend stack/palette.

The owner's latest assignment delegates stack and design decisions and
requests generation-first pacing. It does not override higher-priority
instructions in the selected model's environment or waive final delivery
verification. Report any unavoidable conflict instead of silently ignoring it.

## Existing engine interfaces — not a completed application

Phases 0–3 are recorded as landed in `rewrite/NOTES.md`; this hand-off does
not claim they were reverified in the current session. These are library
building blocks, not proof that every product flow already exists.

| Rewrite source | Integration starting point |
| --- | --- |
| `crates/palantir-core/src/paths.rs` | `DataRoot`, `platform_default()`, instance/descriptor/game paths and shared storage paths |
| `crates/palantir-core/src/version.rs` | Metadata documents and inheritance merge |
| `crates/palantir-net/src/client.rs` | Shared blocking `Http` client |
| `crates/palantir-net/src/scheduler.rs` | Transfer concurrency scheduling |
| `crates/palantir-net/src/sync.rs` | `Syncer::sync_version` and `SyncReport` |
| `crates/palantir-loader/src/install.rs` | `resolve_version` and `install_document` |
| `crates/palantir-loader/src/profiles.rs` | `LoaderKind`, `list_loaders`, `LoaderInstall`, `install_loader` |
| `crates/palantir-loader/src/launch.rs` | `LaunchContext`, `build_launch_plan`, `LaunchPlan::command`, natives extraction and processor execution |
| `crates/palantir-loader/src/import.rs` | `ImportedPack`, `LoaderTarget`, import file/override descriptions |
| `crates/palantir-loader/src/{mrpack,prism,curseforge,vanilla}.rs` | Packaging importers |

Paths in this table are relative to `rewrite/`. Read the signatures you use;
do not invent backend methods. If choosing a non-Rust UI, explicitly define
its Rust bridge/IPC contract inside the rewrite. Do not expose arbitrary shell
execution or unconstrained filesystem access to frontend content.

Instance persistence, lifecycle coordination, progress reporting, Discover
service integration, settings storage, and authentication UI may need new
application-layer work. Record incomplete wiring honestly; do not present
mock data or decorative buttons as live integration.

## Product scope and asset requirements

- Windows desktop launcher with usable window controls, navigation,
  resize behaviour, keyboard access, loading, empty, error, and disabled states.
- Instances: create/import, launch, stop, logs, and confirmed deletion.
- Discover: modpacks, mods, resource packs, shaders; search, filters, ordering,
  project details, and installation into an instance.
- Settings: appearance, language, Java, resources, privacy, behaviour.
- Home, About, and errors that name the problem and offer a useful next step.
- Do not spend this session on skins, screenshots, servers, hosting pages,
  telemetry, or release publication. A framework-required webview is a stack
  decision to document, not a request to add browsing features.

You own the new visual language. Original SVGs, icons, branding, illustrations,
and copy are welcome. No old artwork, source, stylesheets, translations, or
interface text may be reused from another launcher. Third-party assets must
come directly from their permitted upstream source with the licence notice;
Lucide's own ISC-licensed package and appropriately licensed fonts are allowed.
Use a focused asset set rather than generating hundreds of unused icons.

Choose and document source/output paths under `rewrite/` for the selected
stack. Record every new shipped asset's origin and licence in
`rewrite/THIRD_PARTY_NOTICES.md`, including original artwork as original work
under the product's current licence placeholder. The theme crate's current
values are starting material owned by the rewrite, not mandatory colours.
Do not change engine APIs just to make the UI look different.

## Draft status and follow-up verification

The generation session should leave a usable checkpoint as early as possible,
then expand it. Save unresolved checks and issues rather than waiting on CI.
Unverified output remains a draft even if it looks complete.

The follow-up repair session runs checks **from `/PalantirMC/rewrite`**, not
from the legacy repository workspace:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets --locked
cargo test --workspace --locked
python3 tools/licence_audit.py
python3 tools/licence_audit.py --selftest
```

If the chosen frontend is not Rust-only, record its exact install, run,
build, typecheck, and test commands and adapt audit coverage as necessary.
Any new generator needs deterministic output and a `--check` mode. Final
verification must exercise the actual desktop UI and important flows, not
just a compile. The existing audit resolves the rewrite root; it does not
certify assets placed in the legacy tree or automatically cover every new
package manager's dependency graph.

Phase 8 (final product licence and Release) remains owner-gated. Do not
choose the product licence, tag a Release, or publish generated output.

## End-of-session continuation record

Leave the following in `rewrite/NOTES.md`:

- stack/framework and design/locale decisions;
- source paths and launch command;
- UI/SVG/asset inventory and notices;
- working integration versus mock/demo data or unwired controls;
- known errors, deferred verification, and the next focused task.

Do not promise a reduced hour count without a measured completed slice.
Preparation saves repeated orientation; it does not guarantee an entire
frontend or a framework migration will fit in one session.
