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
| 4 | Theme + shell: palette, rail, title bar, page pane, right panel | 🚫 **protected — do not start**: the owner runs this phase with a different tool; no agent may generate UI code or frontend assets — symbols, icons, visible text, palette values (est. ~20–30 h) |
| 5 | Instances: create, launch, kill, logs, delete | not started (estimate ~15–25 h) |
| 6 | Discover: search, filters, project page, install | not started (estimate ~15–20 h) |
| 7 | Settings modal, Home, About, error surfaces | not started (estimate ~15–20 h) |
| 8 | Audit green, own licence chosen, notices complete, first Release tagged | not started (estimate ~3–5 h) |

The phase list and its estimates come from the rewrite specification
(`docs/superpowers/specs/2026-10-08-clean-room-own-licence-rewrite.md` §6);
phases 1–3 are the largest block of new work. All hours are estimates.

## Decisions

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
Recorded in the phase table and in the `AGENTS.md` standing
instructions so a session restart cannot forget it.

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
