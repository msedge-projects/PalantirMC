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
| 2 | `palantir-net`: pooled client, scheduler, resumable downloads, metadata cache, content store | not started (estimate ~15–25 h) |
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
