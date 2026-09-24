# Next steps

The state of this launcher, and what is being done to it. Short on purpose: the
engineering record moved to [`NOTES.md`](NOTES.md), the plan is
[`docs/superpowers/specs/2026-09-24-modrinth-native-rewrite.md`](docs/superpowers/specs/2026-09-24-modrinth-native-rewrite.md),
the numbers the shell is held to are [`REFERENCE.md`](REFERENCE.md), the evidence
that a build met them is [`GATES.md`](GATES.md), and the rules for working in
this tree are [`AGENTS.md`](AGENTS.md).

This file used to be 2231 lines carrying all of that plus a section-by-section
history of a port that is being replaced. The history that is still true was
lifted into `NOTES.md` **with its old section numbers** (source comments cite
them); the rest is in git history, where the map below says which is which.

## What this product is now

A **native Rust implementation of the Modrinth App**, drawn in iced. The design
reference is the Modrinth App's own source, vendored read-only at
`vendor/modrinth-app/` and pinned in its `UPSTREAM.md`. Prism Launcher is no
longer a reference for anything: not its on-disk formats, not its meta API, not
its page layout, not its icons.

## Where it stands

The run that carried stages 0 and 1 is **36022495155** on `28d9d4d`: all five
jobs green, including the design-system regeneration step added in the same
commit, whose log line is `theme generation is byte-identical`. The commit before
it, `99fe67f`, had its own run cancelled by the next push rather than failed --
`concurrency: cancel-in-progress` is on, so a verdict only survives for the
newest commit, which is worth remembering when reading the run list.

Stage 0 is done. Three workspace members left because nothing reaches them
(`crates/nbt`, `crates/schema`, `crates/palantir-cli`), and with them the
PandoraLauncher notice they were the only reason for: `Cargo.lock` went from 553
packages to 510, 447 lines of it, with the closure of `anyhow` and of both
adopted crates.

The workspace is now four crates plus the shell:

| Crate | What it is |
| --- | --- |
| `palantir-core` | Minecraft's own formats (version JSON, libraries, rules, asset index, launch arguments) and the data-root layout. Still Prism-shaped in `ini`/`settings`/`pack`/`instance`; those go with the importer. |
| `palantir-net` | Auth, downloads, metadata, the Modrinth API client. |
| `palantir-loader` | Forge, Fabric, NeoForge, Quilt, and modpack archives. |
| `palantir-gui` | The view-model the shell reads. Goes with `app.rs`. |
| `palantir-desktop` | The window: shell, pages, engine glue, platform code. |

Backend suites as run in this session: `palantir-core` 168, `palantir-gui` 8,
`palantir-loader` 6, `palantir-net` 31 plus 104, with 7 live tests ignored by
design. The desktop crate's own 411 are re-run by CI, which is the authority for
the full workspace run.

## The plan, and where each stage stands

| Stage | What it is | State |
| --- | --- | --- |
| 0 | Prune what nothing references, and reorganize the documents | **Done** |
| 1 | The generated design system: `tools/gen_theme.py` compiles the reference's CSS custom properties, Tailwind's default theme and the component transition blocks into a `theme_gen.rs` the shell paints from, plus a motion table and an icon set | **Generator done; icons next** |
| 2 | The shell rebuilt on the reference's own information architecture: rail, head, page pane, right panel, a `Route` tree with children, Settings as a modal | Not started |
| 3 | Pages, in the reference's order: instance pages first, then project, Home, Discover's six tabs, Skins, Screenshots, Servers, User | Not started |
| 4 | The backend engine: one pooled client, a scheduler, resumable and cancellable downloads, one TTL'd metadata store, Modrinth's metadata | Not started |
| 5 | Instances in our own format, with importers for the popular launchers | Not started |

Stages 1-5 land on a `rewrite-modrinth-native` branch with a draft PR, so CI
sees every commit while `master` keeps building a launcher that runs. Only
stage 0 goes to `master` directly, because it removes nothing that is still
used.

## What stage 1 found, and what it leaves open

`tools/gen_theme.py` reads the reference's own stylesheets and writes
`crates/palantir-desktop/src/theme_gen.rs` — 172 tokens (144 colours, 11 lengths,
2 bare numbers, 1 curve, 14 raw), 39 parsed transitions, 64 that are written as a
shorthand this tool will not guess at, and 24 `@keyframes` names. `--check`
regenerates and compares byte for byte, and it was proved to fail on a one-byte
edit before it was trusted. The ten tests in the generated file assert the values
that matter: the surfaces per theme, the accent followed through `var()`
indirection to a different rung in each theme, the two hover directions, the
radii at a 16px root, and `--ease-out-expo`.

Two things it found on its first run, which are decisions for stage 2 rather than
bugs:

1. **The stylesheet and the running app disagree about the dark accent.** The
   sheet says `--color-brand` is `green-500`, `#1bd96a`; the app as installed
   measures `#00da75` on the call-to-action, the logo and the active rail icon
   (`theme.rs` recorded the measurement, `REFERENCE.md` has the samples). One of
   the two is right and the reference's own window is the arbiter — so this is a
   page-gate question once a page draws that colour, not a transcription question.
2. **The light accent is a rung below `--color-brand` on purpose.** Light
   `--color-brand` is `green-600`, which is 2.71:1 on the light card and misses the
   3:1 floor for a UI component; the shell took `green-700`. That is a deliberate
   departure from the sheet with its arithmetic written down, and the rewrite has
   to keep or replace it knowingly.

The generator also settled a third thing by refusing to guess: `transition: color,
background-color 125ms ease-in-out` is *two* transitions with different timings,
so the 64 shorthands like it are emitted verbatim with their source file rather
than parsed into a plausible-looking wrong number.

Still open in stage 1: the icon set. 313 SVGs are vendored and nothing renders
them yet — the shell's icons are still PNGs carved from another launcher's binary,
which the rewrite removes. Measured before it is written, so the next session
starts from the problem itself rather than from an estimate:

| | |
| --- | --- |
| Icons | 313 files; 6 viewBoxes among them, 305 at the same `0 0 24 24` |
| Elements | 797 `<path>`, 106 `<circle>`, 114 `<line>`, 55 `<rect>`, 25 `<polyline>`, 5 `<polygon>`, 2 `<ellipse>`, 15 `<g>` |
| Path commands | 82 `A` and 589 `a` — **arcs are not optional**, plus C/S/Q/T and their relative forms |
| Rendering | 310 declare `stroke`, 302 a `stroke-width`, 300 each a linecap and linejoin |

The plan is to parse the geometry in Python and emit iced `canvas` builder calls
stroked at the reference's own width, caps and joins — the icons are outlines, so
this reproduces them rather than approximating a filled bitmap, and it keeps them
tintable from a token the way the current vector chrome is. The risk is stated
rather than deferred: a subtly wrong arc conversion is exactly the kind of
plausible-but-wrong value this whole change exists to stop, so the gate for it has
to compare rendered output against the reference rather than assert that every
number is finite.

## Where the old sections went

`NOTES.md` keeps these, under their original numbers: 3 (never looked at on a
real display), 8 (resource profile), 9 (scrolling), 10 (verifying a downloaded
build), 16 (why an instance could not launch), 19 (the asset objects that were
never downloaded), 20 (slower than the line), 21 (the launch bar), 22 (the audit
against what a launcher must do), 23 (driving the reference client), 27 (the
interaction rule), 28 (the vocabulary and the type-scale gap), 29 (the tween),
30 (the dead-code pass).

Dropped, because they described code this rewrite deletes: 1, 2, 5, 6, 7 (the
first delivery pass), 11 (the reference's pixels -- superseded by
`REFERENCE.md`), 12, 13 (the port's own mismatches and its Settings page), 14
(both of them: driving the old shell, and the loading page), 15 (the rename),
17, 18 (adopting PandoraLauncher's engine), 24, 25 (the Screenshots and Home
ports), 26 (the head and the rail's plate).

## Deliberately not done in stage 0

`palantir-gui` and the Prism-shaped modules in `palantir-core` stay until the
shell and the importer that replace them exist, because `app.rs` calls them
today. Removing them now would mean either a launcher that does not build or a
half-migrated one that does not open the instances it already has.
