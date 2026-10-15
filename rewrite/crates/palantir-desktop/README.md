# PalantirMC native desktop

Independently authored frontend for the rewrite, using registry iced 0.12.1
with software rendering. No legacy frontend source, assets, string catalogues,
patches, or generators are used as implementation material.

From `rewrite/`:

```bash
cargo run -p palantir-desktop --bin PalantirMC
```

The data root defaults to `palantir_core::paths::DataRoot::platform_default()`.
`PALANTIR_DATA_DIR=/absolute/path` overrides it for isolated development.
Native system fonts are used; no external font download is required.

## Connected world and local-content flows (2026-10-13)

The former placeholder reviews now execute through `local.rs` and the existing
worker channel. Both editions offer a complete world copy: `.zip` for Java,
`.mcworld` for Bedrock, including settings and pack references. Backup and export
use the same complete-copy format. Review hashes every file and shows paths,
counts, sizes and destination; confirmation rechecks the plan, requires the
player to acknowledge closing the game, and refuses existing outputs. The output
parent must already exist. Copies are streamed and archive contents verified
against review hashes; changed worlds cause the new output to be removed.
Worlds are bounded to 32 GiB, 100,000 files and 40 directory levels.

Java local imports copy a `.jar` mod or `.zip` resource/shader pack into the
selected collection without overwriting existing files. Bedrock imports extract
one manifest-defined pack under its UUID in the detected content directory.
The review lists archive-relative destinations and expanded sizes. Unsafe,
linked, duplicate/case-conflicting, Windows-reserved and multi-pack inputs are
refused. An optional world selection reviews that world's resource/behaviour
pack list; activation preserves unrelated entries. Without that selection no
world lists change. Skin packs can be installed but not activated per world.
The launcher does not certify local-file compatibility or trust.

Bedrock inspection now uses the world engine's titles and versions, with an
explicit fallback when metadata cannot be read. The reader also accepts actual
Bedrock little-endian NBT with its eight-byte header. Database bytes remain
opaque. Bedrock detection is refreshed before review and confirmation, not taken
from an arbitrary UI-provided data path. No ownership checks are bypassed.

Verification: 310 workspace tests passed, then the final Bedrock title-filter
repair passed all 72 desktop tests (311 passing tests across the unchanged
workspace suites; 56 Bedrock, 7 live ignored).
zero-warning workspace Clippy, formatting, development build, licence audit 6/6
and audit self-test. Actual Linux desktop acceptance produced a Java world zip
with each entry byte-matched to its unchanged source, refused an existing output,
and imported local resource packs with byte-exact matches. The final edited
binary repeated the local import at 840×600; scrolling, F1 help and Tab/Enter
activation were exercised. Evidence is under `/tmp/palantir-ui-connected/`.
The final title-filter repair was also exercised in the actual gallery window:
a readable-title match shows its world without an empty warning; an unmatched
query shows the empty state. Gallery samples are explicitly not live game data.
Bedrock extraction/export/activation use real temporary-file tests, not Windows
installed-game evidence. Concurrent external filesystem replacement and game
writes cannot be made crash-atomic by this review: close the game first. A pack
may remain installed if its subsequent activation fails; the error explicitly
asks the player to enable it in-game. No commit, push or publication occurred.

## Dependency-aware installation review (2026-10-12)

Explore's final install review now resolves the selected release and every
required dependency on a worker thread before enabling confirmation. It lists
exact destination filenames, SHA-1 hashes, supplied sizes, download totals,
and hash-verified existing files. Optional dependencies are not installed.
Loading, resolution failure/retry, changed selection, and disconnected-worker
states never enable installation. Review does not download game files.

Confirmation sends the reviewed `ContentPlan` to the worker. The worker resolves
again and refuses changed release/dependency/destination state before downloads.
Existing regular files are reused only when their hash and supplied size match;
different files, links, and duplicate destination names are refused. New files
are staged and verified; placement uses exclusive creates instead of overwriting
renames, and a failed placement rolls back files created by that operation.
This is operation-level rollback, not a crash-atomic filesystem transaction.
An interrupted transfer may retain resumable partial download data.

Verified through the real Linux desktop: Sodium Extra 0.5.9 for Minecraft
1.20.1 installed with Sodium and Fabric API into an isolated Fabric collection;
all three jars matched catalogue SHA-1 and size. Re-review showed three verified
existing files and zero downloads. Removing a file after review caused a
confirmation refusal before download. The scrollable review was exercised at
840×600; captures and test data remain in `/tmp/palantir-dependency-ui/`.
Final workspace checks: 233 tests passed, 0 failed, 7 live ignored; 49 desktop
tests, zero-warning Clippy, format/check/build, audit 6/6 and audit self-test.
No Windows interactive execution or actual game startup is claimed.

## Dual-edition continuation (2026-10-12)

The sidebar now switches between **Java** and **Bedrock**, retaining the Field
Notes design and keyboard-focusable controls. Java collections and persisted
JSON descriptors are unchanged. Bedrock packages are ephemeral Windows discovery
results, not synthetic Java instances: they have no Java version or loader and
cannot receive Java catalogue content. The edition choice is session-local;
startup defaults to Java. The two editions use separate Home/library surfaces.

Bedrock connects the existing `palantir-bedrock` APIs on worker threads:
refresh Release/Preview installations, request game activation, inspect the
existing world/resource/behaviour-pack folder names, and open detected folders.
Each action re-detects the package before using its identity or data path. These
are read-only folder operations; missing directories are not created, linked
roots/content directories are refused, and the world database is never opened.
Preview data remains unavailable until its path is established by the engine.
Loading, no-install, platform error, missing-data, and busy-launch states are
visible. Shell activation is reported as a request, not a running session: no
Bedrock stop button or invented PID appears.

The Bedrock account panel distinguishes local play from Windows/Store/Xbox
sign-in and does not request a Java device code. It explicitly discloses that
package detection is not entitlement verification, local launch does not bypass
licence checks, and the launcher cannot switch or pre-check Bedrock accounts.
Java Microsoft sign-in copy now reflects the implemented verification chain and
states where the session lives: a completed sign-in is stored under the data
root with the user profile's own permissions, and a session review switched to
the Microsoft account launches with it, renewing once when it has expired and
refusing rather than quietly playing as the offline identity.

Checks on this continuation: 44 desktop tests pass, including six new
edition/state/read-only-file tests; the development desktop build passes. Actual
Linux-window coverage exercises Java/Bedrock switching, real non-Windows
backend errors, Bedrock account guidance, keyboard edition activation, and the
840×600 layout. Captures stay in `/tmp/palantir-ui-resume/`. Full workspace/lint
results are recorded in `rewrite/NOTES.md`. Installed-copy cards are regression-
tested with test-only events, not evidence of Windows package execution.

## Frontend completion acceptance

The owner clarified that the assignment is the complete launcher frontend,
not only a verified shell checkpoint. Backend readiness is a separate axis.
The acceptance inventory is:

| Surface | Required frontend interaction |
| --- | --- |
| Shell/assets | Original brand/illustrations/icon roles; hover, pressed, disabled/focus states; native window controls; responsive layout and keyboard navigation |
| Home/activity | Recent collections, create/import/explore shortcuts, current tasks/sessions, inspect/stop/log actions, empty/error states |
| Collections | Search; create with game/loader/version choices; import source inspection/review; overview/content/worlds/logs/manage; rename/delete safeguards |
| Explore | All four content types, filter/reset/search/order/pagination; project description/releases/changelog; release/collection selection; dependency/compatibility review before installation |
| Accounts | Offline identity editing; Microsoft sign-in entry and explicit unavailable/ready states, never invented codes or authenticated success |
| Preferences | Modal with all six panes, working save/reset/revert and validation; persistent supported choices; unavailable capabilities explained |
| About/errors | Version/provenance/notices, diagnostic copying, actionable retry/dismiss and backend availability explanations |

Sources remain under `rewrite/crates/palantir-desktop/`; descriptors/preferences
retain their existing JSON paths and backwards-compatible defaults. Network,
source inspection and filesystem operations stay on worker threads. No demo
content is used as live data, no account registration/publishing is authorized,
and destructive user-data actions require confirmation.

## Delivered UI surfaces

- Home, Collections, Explore, Preferences, and About; native window decorations.
- Graphite/copper Field Notes design, asymmetric wayfinder, original landscape,
  empty-shelf illustration, and source-authored navigation geometry.
- Adaptive activity panel: a right notebook in large windows, inline in small
  windows; selectable accent, interface scale, and visibility.
- Collection Overview, Content, Worlds, Logs, and Manage tabs.
- Create with live game-version browsing, snapshot/history filtering, all five
  loader identities, and release pinning for every loader (Fabric/Quilt's meta
  services and the Forge family's own Maven both answer); rename/delete dialogs.
- Filesystem source browser, read-only import inspection and confirmation, plus
  `.mrpack` metadata/optional-file review whose chosen optional files are carried
  into a real archive install.
- Project Description/Releases/Changelog tabs, release filtering/load-more,
  external author links, compatible-collection selection and final install review.
- Multiple offline accounts: add, select and remove local identities; the active
  one is what Play launches with, saved as soon as it changes, and two rows can
  never share a name because the key derives from it.
- Offline account status and Microsoft device-code frontend states: requesting,
  code/copy/browser, verifying, connected, expired, failed and cancelled. Live
  backend emits unavailable, not fake device codes; intermediate states have
  regression coverage through test-only backend events.
- Modal errors are visible inside dialogs, backgrounds do not receive clicks,
  and Escape closes them. Preferences are a six-pane modal with save/reset/revert.
- All five loaders -- Vanilla, Fabric, Quilt, Forge, NeoForge -- are connected
  for creation and launch. The Forge family installs from the vendor's own
  installer jar on first play: the overlay document, then the profile's
  toolchain libraries, then its processors under the managed runtime, all before
  natives are extracted. Plain game-directory and Prism/MultiMC directory
  imports copy worlds/content/configuration and preserve supported loader pins,
  without copying launcher authentication files.
- Collection filtering, content inventory, mod enable/disable through `.disabled`,
  confirmed content-file removal, saved-world listing, all three content-folder
  actions, refresh, and bounded session logs with copy/clear actions.
- Live catalogue search by kind/order/game version, pagination, project details,
  text-only native project notes, and compatible stopped-collection selection.
- Six preferences panes: Appearance, Language, Java & player, Resources,
  Privacy, Behaviour. Unsupported functionality is explained rather than hidden
  behind inert controls. Deletion confirmation cannot be disabled.
- Empty, loading, busy, service-failure, validation, and worker-disconnection
  states; operation completion cannot close an unrelated dialog.
- Tab/Shift+Tab focus on action buttons, navigation, inputs and selectors;
  Enter/Space action activation and arrow-key selector changes, visible focus
  rings, hover/pressed/disabled styles; Ctrl+1/2/3 navigation, Ctrl+, preferences.
  Page/tab navigation resets scrolling so headers remain reachable after
  reading long project notes or the inline activity notebook.

## Connected engine actions

First play installs game assets and the appropriate managed Java runtime;
subsequent play repairs missing shared files. A custom Java executable overrides
runtime management. Download/install work happens outside the window event loop.
Launch/stop and stdout/stderr logging are connected, explicitly in offline mode.

A compatible mod/resource pack/shader release can be installed with SHA-1 and
size verification, together with every `required` dependency it names, resolved
to a build that matches the collection's own game version and loader. Nothing is
fetched until every destination is decided, so a failure refuses the whole
install instead of leaving part of it, and the completion note says how many
dependencies arrived with the release. Existing files are not silently
overwritten. The `.mrpack` worker installs from its own index: its client file
list is planned and verified, then the archive's override layers are applied over
it, base layer first so a file carried twice ends as the client's copy. Optional
client-file choices are carried from the source review into the import request;
an absent review is refused rather than guessed. A modpack requested as content for an existing collection is still
refused: a pack must not land over a world's own files. Catalogue descriptions
are their authors' live content, not launcher interface copy or artwork.

Microsoft sign-in is implemented and reports itself unavailable unless a client
id is supplied. `PALANTIR_MSA_CLIENT_ID` names a registered native client; the
device-code flow then runs off the window thread (request a code, poll, Xbox
Live user token, XSTS, Minecraft session, profile), and cancelling stops the wait
without a later result being reported. The profile call is also the ownership
check, and that failure is worded as a purchase problem rather than a network
one. With no client id, no code is requested and none is invented.

## Persistent contracts

`instances/<id>/instance.json` has `id`, `name`, `game_version`, `loader`
(`Vanilla`, `Fabric`, `Quilt`, `Forge`, `NeoForge`), nullable `installed_version`, and nullable
`loader_version` (the pinned loader, if imported).
Game data lives in `instances/<id>/game/`; shared downloads retain the existing
engine layout. Path boundaries reject linked instance/game/content directories.
Directory import limits: 32 GiB, 100,000 files, 40 path components; symbolic links
are refused before copying. The source is untouched. An interrupted copy may
leave an explicitly reported recoverable partial collection.

`preferences.json` includes `player_name`, `java`, `memory_mb`, `language`,
`accent` (`Copper`, `Glacier`, `Lavender`), `scale` (`Compact`, `Comfortable`,
`Large`), `show_activity`, `remember_page`, `last_page`, `offline_accounts`
(`id` and `name` rows whose uuid is derived, never stored) and `active_account`;
absent fields use defaults. The selected offline account is the identity Play
launches with — name and derived uuid — and `player_name` is the fallback while
no account is selected. The derivation is the one this launcher already shipped
for its single offline player, so an existing name keeps its identifier. Earlier fields
`confirm_delete` and `reduce_motion` are retained for backwards compatibility,
not exposed as nonfunctional toggles. Remember-page restoration is connected. English is the only authored
locale; transitions are instantaneous. JSON writes stage to `.tmp` and keep a
`.bak` recovery file for Windows rename semantics.

## Verification checkpoint (2026-10-10)

From `rewrite/`, workspace tests pass (154 passed, seven live tests intentionally
ignored), including 26 desktop contract/state/file tests. Later desktop repairs
were rechecked with all 26 desktop tests passing. Formatting, workspace
check, Clippy with `-D warnings`, development build, licence audit 6/6 and audit
self-test pass. Processor fixture concurrency was repaired without weakening its
spawn/receipt assertions; repeated processor and workspace suites passed.

The actual Linux window was exercised under Xvfb with isolated data: creation,
rename, all detail/preferences tabs, invalid-memory feedback, saved preferences,
safe directory import and account exclusion, mod toggling, saved-world listing,
cancelled/confirmed deletion, live Explore search/details and install eligibility.
Restart preserved player, memory, accent and density. Expanded-window testing
also exercised live version selection, Forge availability, source browsing,
import review, Microsoft unavailable state, releases/changelog, reset/revert,
confirmed addition removal, and a real Fabric API install selected through the
final review (SHA-1/size verified by the downloader). Action-button Tab/Enter
activation and the small-window settings modal were exercised. Navigation at
840x600 with Large density was retested after repairing inherited scroll offsets. Logs and
captures are development evidence in `/tmp`, not shipped files.

This is not proof of Windows execution or actual game startup from the desktop.
The verified live single-file content install is not modpack/dependency readiness.
Authentication codes/success are covered with test-only events, not live service
credentials. No push, publication, account provisioning, or Release was performed.

## Backend continuation that changes these screens (2026-10-12)

The owner paused frontend work and had the five recorded backend gaps closed.
Three of them change what the screens above may claim: the Forge family really
installs and launches; a Microsoft session is stored, renewed once and injected,
with a refusal instead of a silent offline fallback when it is missing or
unrefreshable; and a pack's optional files are carried from the review into the
install. Bedrock's world and pack engines landed on the engine side only, so
those two screens now say the engine exists but this screen does not call it.
Verified from this tree's own gates: 299 workspace tests pass (0 failed, 7 live
ignored), formatting and Clippy (`--locked`, `-D warnings`) clean, licence audit
6/6 with self-test. No Xvfb window run was made in this continuation -- the UI
was paused -- so these flow changes carry compile-and-worker coverage and not new
window evidence. Details are in `rewrite/NOTES.md`.

## Remaining functionality / release acceptance

- Microsoft native-client registration. The device-code chain, the stored
  session and the authenticated launch are connected, but with
  `PALANTIR_MSA_CLIENT_ID` unset a build still reports sign-in as unavailable
  rather than inventing a code, and live acceptance needs a client only the
  owner can create.
- Live Windows acceptance of the connected Bedrock world and pack flows;
  multi-pack `.mcaddon` import is currently refused rather than guessed.
- Bedrock entitlement pre-check, account assistance, Preview data discovery,
  and session tracking.
- Additional independently authored translations.
- Full Windows execution, real-game desktop acceptance, and packaged distribution.
- Final product licence, legal design/trademark review, and Release.

The UI coverage is not a claim that those backend features are complete.
Original artwork may itself be copyrightable; no claim of legal clearance or
non-copyrightability is made. Provenance is recorded in `THIRD_PARTY_NOTICES.md`.
