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
  loader identities, and Fabric/Quilt release pinning; rename/delete dialogs.
- Filesystem source browser, read-only import inspection and confirmation, plus
  `.mrpack` metadata/optional-file review (archive execution explicitly unavailable).
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
- Vanilla, Fabric, and Quilt collection creation and launch are connected.
  Forge/NeoForge descriptors can be saved and inspected but have a clear
  loader-setup-required action instead of pretending launch is available. Plain game-directory and
  Prism/MultiMC directory imports copy worlds/content/configuration and preserve
  supported loader pins, without copying launcher authentication files.
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

A compatible dependency-free mod/resource pack/shader release can be installed
with SHA-1 and size verification. Existing files are not silently overwritten.
Dependency-bearing releases and modpacks are explicitly refused, not partially
installed. Catalogue descriptions are their authors' live content, not launcher
interface copy or artwork.

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

## Remaining functionality / release acceptance

- Microsoft device-code backend/client registration and authenticated play;
  frontend states and controls exist, but no tokens or codes are fabricated.
- Forge/NeoForge desktop installer coordination.
- `.mrpack` archive execution and dependency coordination. Metadata/optional
  selection review is implemented; it does not claim to execute archives.
- Additional independently authored translations.
- Full Windows execution, real-game desktop acceptance, and packaged distribution.
- Final product licence, legal design/trademark review, and Release.

The UI coverage is not a claim that those backend features are complete.
Original artwork may itself be copyrightable; no claim of legal clearance or
non-copyrightability is made. Provenance is recorded in `THIRD_PARTY_NOTICES.md`.
