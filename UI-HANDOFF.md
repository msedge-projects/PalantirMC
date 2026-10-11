# UI hand-off — start the selected model here

## Paused, with a new product decision to read first (2026-10-11)

Frontend work is paused by the owner. The desktop crate and its assets are on
disk uncommitted and unpushed.

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
