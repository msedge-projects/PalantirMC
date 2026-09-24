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
| 1 | The generated design system: `tools/gen_theme.py` compiles the reference's CSS custom properties, Tailwind's default theme and the component transition blocks into a `theme_gen.rs` the shell paints from, plus a motion table; `tools/gen_icons.py` compiles the 313 vendored SVGs into strokeable geometry | **Done** |
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

### The icon set, and the four mistakes it caught

`tools/gen_icons.py` compiles all 313 SVGs into `icons_gen.rs`: 1105 elements,
5177 commands, as `Cmd` data with one small interpreter rather than 313 closures.
Arcs (82 absolute and 589 relative), quadratics and the smooth-curve forms are
resolved to cubics; `<g transform>` and inline `style` are applied; the whole set
is refused rather than guessed at when something is not understood. Nine tests in
the generated file pass, and the whole desktop suite is 430.

Writing it found four things that a plausible-looking generator would have got
wrong silently, each of which is now an assertion:

1. **The stroke width is scaled by the transform.** `loader.svg` declares
   `stroke-width="23"` inside `matrix(.08671 0 0 .0867 -49.8 -56)` — that is how a
   24-unit icon is stroked at 2. Transforming the geometry but not the width draws
   the shape correctly eleven times too heavy, which renders, and looks like an
   icon.
2. **`fill`, `stroke` and `stroke-width` are inherited from `<svg>`.** The
   reference declares them on the root element, where CSS inherits them into every
   shape. A reader that looks only at the shapes sees `x.svg` as an unremarkable
   path and strokes it — drawing the outline of an outline. Nine icons are filled,
   not five, and the list is asserted.
3. **`opacity` is part of the drawing.** The spinner's ring is a quarter opaque,
   and a generator that accepted the attribute without applying it would draw a
   solid ring, which reads as a finished circle.
4. **A shape can be filled *and* stroked.** `images.svg` draws its circle both
   ways, so it becomes two elements with one geometry; a boolean flag meaning
   "both" would have made the painter guess.

The winding rule was the open question and it is now closed rather than hedged:
the reference fills with even-odd, the toolkit fills with non-zero, and the two
differ only for a filled path whose subpaths wind the same way. Six filled
subpaths have more than one contour, four wind oppositely (so the rules agree) and
two do not declare even-odd at all (so non-zero is what draws them upstream).
`winding risk 0` is therefore a computed answer, and the generated test fails if
a future icon makes it false.

What is still unproven is that they *look* right: the gate is structural, and no
number here says an arc came out on the correct side. Comparing rendered icons
against the reference's own window is part of stage 2's visual gate, and it is the
one thing this stage cannot do on its own.

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
