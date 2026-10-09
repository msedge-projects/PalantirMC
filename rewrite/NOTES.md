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
| 3 | `palantir-loader`: vanilla, Fabric, Forge, NeoForge, Quilt; `.mrpack` and Prism importers | not started (estimate ~30–40 h) |
| 4 | Theme + shell: palette, rail, title bar, page pane, right panel | not started (estimate ~20–30 h) |
| 5 | Instances: create, launch, kill, logs, delete | not started (estimate ~15–25 h) |
| 6 | Discover: search, filters, project page, install | not started (estimate ~15–20 h) |
| 7 | Settings modal, Home, About, error surfaces | not started (estimate ~15–20 h) |
| 8 | Audit green, own licence chosen, notices complete, first Release tagged | not started (estimate ~3–5 h) |

The phase list and its estimates come from the rewrite specification
(`docs/superpowers/specs/2026-10-08-clean-room-own-licence-rewrite.md` §6);
phases 1–3 are the largest block of new work. All hours are estimates.

## Decisions

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

**2026-10-08 — the data root is one tree, shared where the game shares.**
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
