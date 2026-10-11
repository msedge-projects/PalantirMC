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
| 5 | Instances: create, launch, kill, logs, delete | **native UI checkpoint** 2026-10-10: five detail tabs, create/import/rename/confirmed deletion, content toggling, managed vanilla/Fabric/Quilt launch and stop/log workers; actual-window file flows verified, real-game desktop acceptance and Forge-family coordination remain |
| 6 | Discover: search, filters, project page, install | **native UI checkpoint** 2026-10-10: live search/type/order/version filters, pagination, native project notes and stopped-collection install selection; actual-window live search/detail/eligibility verified; dependency/modpack resolution remains |
| 7 | Settings modal, Home, About, error surfaces | **native UI checkpoint** 2026-10-10: six preferences panes, persistent accent/density/player/resources, Home/About, account status and modal errors; Linux window flows verified; additional locales and authenticated accounts remain |
| 8 | Audit green, own licence chosen, notices complete, first Release tagged | not started (estimate ~3–5 h) |

The phase list and its estimates come from the rewrite specification
(`docs/superpowers/specs/2026-10-08-clean-room-own-licence-rewrite.md` §6);
phases 1–3 are the largest block of new work. All hours are estimates.

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
