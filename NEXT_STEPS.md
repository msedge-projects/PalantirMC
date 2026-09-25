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

Two runs carry stages 0 and 1, both all five jobs green:

| Run | Commit | What it proved |
| --- | --- | --- |
| 36020421890 | `2361cc3` | The prune builds and tests where it matters, with three workspace members gone |
| 36022495155 | `28d9d4d` | The design system regenerates byte-identically, quoted from the log: `theme generation is byte-identical` |
| 36027116004 | `f8c77e8` | Same for the icon set: `icon generation is byte-identical` |

Worth remembering when reading the run list: `99fe67f` shows as *cancelled* rather
than green, because `ci.yml` sets `concurrency: cancel-in-progress` and the next
push superseded it. A cancelled run is not a passing one, and it is not a failing
one either.

**Stages 2, 3 and the engine's slices were pushed to a runner that could not
start.** Ten runs at the time of writing, one per commit from `82232f9` onwards --
`36038333030`, `36033443993`, `36143868395`, `36152873678`, `36156018946`,
`36158909795`, `36159099493`, `36162312983`, `36162426595`, `36165962973`, and one
more per push -- died in three to six seconds with zero steps and the same message
-- `recent account payments have failed or your spending limit needs to be
increased` -- which is a billing state and not a verdict on the tree; none of the
jobs was scheduled, so none could report the expected `test result: ok`. The tree
was therefore measured on the machine it was written on, with the same commands
CI runs, and the transcripts are in
[`GATES.md`](GATES.md) beside the gates they evidence. **This is a weaker receipt
than a green run and it is recorded as one**: the local run is the same compiler
and the same flags, but it is not a clean checkout and it is not the authority
`AGENTS.md` names. The moment billing is restored the same push should be re-run
with `workflow_dispatch` and watched to green, and until then nothing here should
be read as "CI passed".

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

Backend suites as run in this session: `palantir-core` 168 plus 8, `palantir-gui`
6, `palantir-loader` 31, `palantir-net` 205, with 12 live tests ignored by design --
976 in the workspace, and the desktop crate's 554 are in it. Which binary each
number belongs to is written out in [`GATES.md`](GATES.md), because a bare list of
numbers is how the earlier version of this paragraph managed to mislabel three of
them.

## The plan, and where each stage stands

| Stage | What it is | State |
| --- | --- | --- |
| 0 | Prune what nothing references, and reorganize the documents | **Done** |
| 1 | The generated design system: `tools/gen_theme.py` compiles the reference's CSS custom properties, Tailwind's default theme and the component transition blocks into a `theme_gen.rs` the shell paints from, plus a motion table; `tools/gen_icons.py` compiles the 313 vendored SVGs into strokeable geometry | **Done** |
| 2 | The shell rebuilt on the reference's own information architecture: rail, head, page pane, right panel, a `Route` tree with children, Settings as a modal | **Done**: the `Route` tree, the tween engine, the icon widget, the copy, the colour theme and the shell itself are in and run under `--shell`. The old chrome is still what runs by default, which is the plan's own decision -- it is switched over when the new shell can launch an instance |
| 3 | Pages, in the reference's order: instance pages first, then project, Home, Discover's six tabs, Skins, Screenshots, Servers, User | **In progress**: all eight page modules are in and the pane draws them instead of the placeholder. What is not real yet is anything a service answers -- see "What stage 3 has landed so far" |
| 4 | The backend engine: one pooled client, a scheduler, resumable and cancellable downloads, one TTL'd metadata store, a hash-keyed content store, Modrinth's metadata | **In progress**: one client with one ceiling, one retry policy, cancellable and resumable transfers, a work queue where every job reports, a metadata cache that revalidates instead of re-downloading, a content store where a file that is already here is never fetched twice, Mojang's piston metadata read directly and checked against its own digests, and Modrinth's API on the same cache and ceiling are all in and gated (G66-G74). What is not in is the seam that lets a page use any of it, and the desktop call sites that still hold their own client -- see "What stage 4 has landed so far" |
| 5 | Instances in our own format, with importers for the popular launchers | Not started |

Stages 1-5 land on a `rewrite-modrinth-native` branch with a draft PR, so CI
sees every commit while `master` keeps building a launcher that runs. Only
stage 0 goes to `master` directly, because it removes nothing that is still
used.

## What stage 1 found, and what it leaves open

`tools/gen_theme.py` reads the reference's own stylesheets and writes
`crates/palantir-desktop/src/theme_gen.rs` — 172 tokens (144 colours, 11 lengths,
2 bare numbers, 1 curve, 14 raw), 37 parsed transitions, 64 that are written as a
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
the generated file pass, and the whole desktop suite is 461.

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

## What stage 2 landed

On `rewrite-modrinth-native`. The two pieces below are logic rather than chrome,
which is why they go first: each is testable without a window, and each is a
thing the shell would otherwise invent.

**`route.rs`** is the reference's navigation, from
`app-frontend/src/routes.js`, route for route and name for name -- including the
names with spaces in them (`Discover content`, `Skin selector`), because
the reference's own code navigates by those strings and a rename makes every
later port harder to line up with upstream. It carries what the old shell did not
have at all: instance and project nesting, the six Discover tabs, the legacy
`/mod/:id/:rest*` redirect, and the `?i=`/`?sid=` context that turns Discover into
an install-into-this-instance flow. Three absences are decisions with reasons in
the module: no `Settings` route (it is a modal), no `Accounts` route (Modrinth
accounts are the rail's profile menu, Minecraft accounts are in that modal), and
no top-level Mods or Logs (both are instance tabs).

The rail's own highlight rules came out of it, and two of them are worth knowing
before the shell is drawn. `/browse` is Discover -- unless the address carries
`?i=`, in which case the reference marks *Home* as a subpage, because the page
being browsed for an instance belongs to that instance. And `/instance/:id` marks
nothing at all: exactly three of the rail's eight slots get an `is-primary` or
`is-subpage` predicate in `App.vue`, and none of the three tests for `/instance`.
That looks like an oversight upstream and is reproduced anyway, with a test that
says so; the moment the shell looks wrong is the moment to decide.

**`motion.rs`** is the reference's timing, and it needed a solver a native
toolkit does not have: iced has no `cubic-bezier()`, and a CSS timing function is
not sampled at progress, it is *inverted* for it, because the two control points
are given in x and y while the input is x alone. The algorithm is WebKit's
`UnitBezier` -- eight Newton-Raphson steps with a bisection fallback -- and it is
checked against Chromium's own answers: `tools/curve_samples.html` runs each of
the reference's five curves as a real CSS animation in the engine the reference
ships inside, pauses it at each tenth and reads the computed style back, and the
test asserts the same nine values per curve to 1e-5. A Python cross-check of the
same algorithm over the same samples agreed to 1.3e-6, which is why the tolerance
is where it is.

Durations are citations, not numbers chosen here: `Timing::declared("opacity",
125)` looks the pair up in the generated table and returns `None` if the reference
does not declare it, which is the useful answer. The rail's signature animation is
pinned as `Timing::NAV_PLATE` -- 250ms on `--ease-out-expo`, with the `scale: 0.4`
the plate grows from -- quoting `NavButton.vue`'s
`opacity 0.25s var(--ease-out-expo), scale 0.25s var(--ease-out-expo)`, one of the
64 shorthands the generator keeps verbatim. `Tween` follows one value, retargets
mid-flight the way a browser does (the new leg starts from the value on screen,
not from where the first leg began), and lands *exactly* on its target, because
two controls that should line up are drawn from the same number.

### The fifth mistake, this one in stage 1's own generator

`tools/gen_theme.py` multiplied by 1000 on both branches of its duration parse,
so every transition the reference writes in milliseconds came out a thousand
times too long: `transition: outline-color 150ms ease` was in the table as 150000.
Seven rows were wrong, and because `--check` compares the tool's output with the
tool's output, they agreed with themselves and passed. Two of the seven collided
with rows the table already had once corrected, which is why the parsed count went
39 to 37 rather than 39 to 32.

The fix is the millisecond branch, plus a bound: the reference's longest
transition is 2s (`transform` in `TextLogo.vue`), anything past 2000ms is a
mistake in the tool rather than a fact about the reference, and the tool now keeps
such a row verbatim instead of emitting it. Three assertions hold that line -- in
the tool, in the generated test, and in `motion.rs`'s own table check -- because a
wrong number both files agree on is invisible to a byte comparison.

**`icon.rs` and `shell.rs`** are the chrome those two feed. `icon.rs` is the one
place an icon becomes a canvas: the generated elements carry geometry in the
SVG's own 24-unit box, and something has to scale that box to the pixel size
asked for and centre what is left over -- `fit()` is that arithmetic, and it has
to be handed to the painter as well as to the frame, because iced tessellates a
stroke at the width it was given and does not scale it by the frame's transform.
Getting that wrong draws every 16px icon in the head at two thirds weight, which
renders and looks deliberate.

`shell.rs` is the whole interface: a 64px rail of 48px circular plates at a 52px
pitch, a 48px top bar carrying the logo, history, breadcrumb, the panel toggle
and the three window controls, the page pane at a 20px top-left radius, the 300px
right panel with the reference's two-stop wash behind its content, and Settings as
a modal layer -- the panel is the one place iced 0.12's missing z-order shows,
because a modal has to replace the window's content rather than stack over it.
It is a whole `iced::Application`, so `--shell` runs it today, with the pages of
stage 3 inside it.

**The copy, the colour theme and the native gate** came with the same stage.
`tools/gen_text.py` compiles both of the reference's English locales -- 3846
messages, in `app-frontend` and `ui`, which share no key -- into `text_gen.rs`,
refusing the five ICU constructs the reference does not use rather than
guessing at them, and `text.rs` is the small runtime behind the four it does.
`color_theme.rs` is the reference's own theme list from `use-theme.ts`, with the
dev-mode rule for retro quoted from `AppearanceSettings.vue`, and
`theme.rs` now keeps *which* theme is in force as an index into that list, so a
fifth variant cannot silently mean Dark. `tests/native.rs` is the gate for the
whole rewrite's premise: it fails on a browser or a JavaScript engine anywhere in
the resolved graph. All three are in `GATES.md` as G60 and G61.

### The sixth, seventh and eighth mistakes, all caught by the shell's gates

Three of them, and the first two are the kind a plausible-looking shell would
have shipped:

1. **`--color-button-bg-selected` is not a wash of the accent in light mode.**
   The test asserted every theme's selected rail plate was translucent, which is
   true in dark and OLED (the accent at 25%) and false in the other two: light
   uses opaque `green-600` -- the same value as its own `--color-brand`, so a
   selected button there is a *solid* plate -- and retro uses an opaque `#25421e`
   that is neither the accent nor a dilution of it. The assertion now states the
   value per theme instead of one claim for all four.
2. **A gradient stop's colour can contain spaces.** `rgba(68, 182, 138, 0.175) 0%`
   split on whitespace yields `rgba(68,` as the colour, and the reader returned
   `None` for every wash in the reference -- which looked like "the reference has
   no gradient here" rather than like a parser bug. The split now tracks
   parenthesis depth, and the test asserts all four themes' two stops.
3. **`Tween::at` handed out a zero-length timing.** A tween created at rest
   carried the placeholder `0ms linear`, so the first `retarget` on it saw a
   duration of nothing and *jumped* to its target: the rail's plate never grew,
   and a missing animation is much harder to notice than a wrong one. The
   resting timing is now named by the caller, and the rail passes
   `Timing::NAV_PLATE`, which is the reference's own 250ms.

One more thing settled by reading iced rather than guessing: iced's `Background`
gradients do render on the wgpu backend, and its angle convention matches CSS's
exactly -- `Radians::to_distance` subtracts a quarter turn before taking the
direction vector and measures y downwards, so iced's 0 faces up, which is CSS's
`0deg`. The reference's wash is `0deg` in all four themes, so it needs no
conversion; anything not axis-aligned would not, because iced measures the
gradient line as `max(|x|·w, |y|·h)` where CSS projects onto both axes, and the
comment in `parse_gradient` says so rather than leaving it to be rediscovered.

## What stage 3 has landed so far

All eight page modules are in and the pane draws them: `pages/home.rs` (the
welcome screen and the library), `discover.rs` (the six project-type tabs, the
search field, the controls row), `project.rs` (the header and its three tabs),
`instance.rs` (six tabs), `skins.rs` (thirteen sections as disclosures),
`screenshots.rs` (every instance's, searchable), `servers.rs` and `user.rs`.
Two modules hold what they are made of rather than repeating it: `page.rs` is the
scaffold every page is built from -- the five states of a request, the four
blocks that answer four of them, and the dismissible notice -- and `ui.rs` is the
widget kit, where every value quotes a class or a rule from the reference
(`.base-card`, `NavTabs`, `Input.vue`'s icon, `Combobox`, `Admonition`, the
reference's button types) with the two readings that are not quotations marked at
the point they are used.

`store.rs` is the seam that makes the pages honest rather than finished. It
answers what the launcher can already answer -- the instance list and each
instance's own folders, from the same readers the old interface uses -- and for
everything that has to come from a service it returns `Load::Failed` carrying a
sentence that *names stage 4*. The reason is that the alternative was to draw an
empty list, and a page that shows "no results" when the request never happened is
the failure mode the whole scaffold exists to prevent.

Three things about the pages are decisions worth keeping:

1. **A page reports navigation, it does not perform it.** Pressing an instance
   card returns `pages::Open::Instance(id)` out of `Screen::update`, and the shell
   is the only caller that acts on it. Without that, a page would have to know
   what else changes when the pane does -- the history, the rail's selection, the
   breadcrumb -- and every page would grow its own copy of that knowledge.
2. **`mouse_area` does not forward events.** iced's own `update` for it never
   visits its content, so a button drawn *inside* one is dead. The instance card
   is therefore pressable in the part of it that is not a control, which draws the
   same picture as the reference's clickable div with stopPropagation inside it
   and is a region a user can actually hit. The same reading is why the old shell
   wraps its controls in `hover::Report` -- that widget returns the content's own
   status untouched, so the control inside keeps its press and its click.
3. **`--surface-3` is `--color-bg-raised` in all four themes.** A card is told
   apart from the page it sits on, not from the bar above it. The gate had
   asserted the opposite, because that reads like a difference; the reference
   does not make one.

What stage 3 does **not** have yet, named rather than implied:

* **The controls do not tween their hover.** The rail's plate does (stage 2's
  clock), and the pages' buttons, tabs and cards change colour on a frame
  boundary rather than over the reference's 150ms. The machinery is already here
  and generic over the message type -- `motion::Tween` for the value,
  `hover::Report` for the crossing, `anim::Interactions` for the clock -- and the
  work is a key per control plus a `Hover { key, over }` message in each page's
  own family. The old shell's gate for this cannot see it: it reads source text
  for iced's `button(…)` and holds every one of them to `hover_button`, and the
  new kit's builder is a different function with the same name. That is the first
  thing the next session should close, because "the buttons snap" is the kind of
  difference a person notices immediately and a test does not.
* **No page's data comes from the network.** Discover's search, project pages,
  Skins, Servers and the hosting half of an instance all say so out loud. The
  control *states* they will be asked with are live, which is the part that makes
  the request a one-line change rather than a page rewrite.
* **The right panel is still the reference's wash and nothing else.** Discover,
  a project and a profile force it on (`App.vue`'s `forceSidebar`), and what it
  draws when it is there is stage 4's, because everything in it is a service's
  answer.
* **Settings is still a placeholder modal.** The colour theme it will offer is
  already the reference's own list, with retro behind dev mode, in
  `color_theme.rs`.

## What stage 4 has landed so far

`crates/palantir-net/src/engine/` is the backend the pages will be served by. It
is eight modules, and each one exists because the launcher has had the
alternative:

| Module | The thing it replaces |
| --- | --- |
| `http` | One `reqwest` client per call, which throws away the connection and both caches every time; one `User-Agent`, one timeout, one process-wide ceiling |
| `limit` | Two different thread counts that meant two different things, and no cap at all on metadata requests made beside them |
| `retry` | A policy per call site, which is a different policy per call site: 408/425/429 and 5xx are worth another attempt, a 4xx is the server saying the client is wrong |
| `cancel` | Nothing: a phase of 1200 files ran to completion, on the calling thread's terms |
| `request` | The seam. `reqwest` cannot be reached from a unit test, so every rule above is stated against a `Fetch` and exercised with a server that can be told what to do |
| `download` | A transfer that always started from zero. Now: resume from the `.part` file's length, verify the digest before the rename, and delete a part file that fails it |
| `schedule` | A bulk downloader that could not be stopped. Now: submitted jobs with ids, one event per job, cancel one or all, and an `Idle` a caller can wait for instead of polling |
| `cache` | A metadata "cache" whose only rule was "if the file is on disk, use it" -- which is a write-once archive, and how a launcher comes to offer a loader build that was published last year |
| `content` | Three trees holding the same jar three times: `libraries/`, `assets/` and a `.minecraft/versions/` copy of what a version asks for. Now every file is named by its own digest, so what is already here is never fetched again |
| `piston` | `meta.prismlauncher.org`, which is Prism's mirror of Mojang's own metadata rewritten into Prism's shape. Now the launcher reads piston directly, and checks every version file against the `sha1` the manifest published for it |
| `modrinth` | The desktop crate's `browse.rs`, which reaches `api.modrinth.com` with a `reqwest` client of its own -- a second connection pool and a second answer to "how many requests is this launcher making". The engine-side client is in and gated (G73-G74); the desktop's call sites still use theirs |

The cache is the one worth reading twice, because it is the only part of this
that changes what a user sees. An entry has an age; inside its TTL the bytes on
disk are the answer and no request is made, past it the validator the service
gave goes back as `If-None-Match`, and a `304` moves the stamp without moving a
byte. Two TTLs, because the two things are not alike: a version *list* changes whenever
somebody publishes a build, so it is believed for half an hour, and a version
*file* describes something already released, so it is believed for a year. The body is stored as the
service sent it, with the age and the validator in a stamp beside it, so the
Mojang and Modrinth layers that come next can read the same directory.

`content` is the other half of the same idea, applied to files rather than
answers: every jar, asset object and mod is stored under its own digest, so a
file that is already here is already here under every name that asks for it. The
launcher has had three trees holding the same jar (`libraries/`, `assets/`, and a
per-version copy), and a modpack that ships a mod the user already installed had
it twice. Three digests are carried because the services publish three -- Mojang a
`sha1` per library and asset, Modrinth a `sha1` and a `sha512` per file, Prism a
`sha256` -- and all three are computed rather than trusted. Nothing is filed until
it verifies, which is what keeps the store from becoming a place where a corrupt
download is cached forever, and it is also what makes an interrupted transfer
resumable: the `.part` sits under the digest's own name, so the next run finds it
without being told what it was doing.

**The live measurements, which are the part of this that is not local.** Three,
all of them `tests/live.rs` tests, `#[ignore]`d by design and run here with
`--ignored`:

1. `meta.prismlauncher.org` sends an `ETag` on
   `/v1/net.minecraft/index.json` and answers `304 Not Modified` to it -- the
   recorded line is `revalidated with "6ab45c1a-6bc5e", no body sent`, so the
   revalidation path is the service's behaviour rather than the double's.
2. A real asset object, read out of the live asset index by the `sha1` Mojang
   names it with, is fetched, verified against that digest, filed in the content
   store, and answered from the disk on the second look with no request.
3. The live piston manifest names a latest release, the `sha1` it publishes for
   that release is the digest of the version file that arrives, and the file
   parses into `net.minecraft.client.main.Main`, more than twenty libraries, a
   downloadable asset index, and no `order` key -- the last of which is the
   difference between Mojang's file and Prism's mirror of it.
4. A typed Modrinth search -- `facets=[["project_type:mod"]]`, percent-encoded --
   is accepted and answered by the real API, the project its first hit names has
   versions with files that publish digests, and the same search a second time
   returns the same response without a request.

They are the only receipts in this document that came from outside this machine.
A fixture would have agreed with the code on all three; the services are the only
things that can disagree.

Three mistakes the engine's own tests caught, all of them invisible by
inspection and all of them the kind that only shows up in front of a user:

1. **The wait before an attempt was one step off.** `delay(n)` means "the wait
   *before* attempt n", so the first retry was immediate and a policy that said
   three attempts made four. Now the call is `delay(tries + 1)` and the test
   counts the requests rather than the successes -- a retry and a single
   successful request are the same thing to a test that only counts bodies.
2. **A resumed download reported itself as a fresh one.** `finish` derived the
   state from the byte count, and a resumed transfer's byte count is the whole
   file. The state is passed in now, because it is something the transfer knows
   and the file does not.
3. **The backoff's arithmetic went through `as_secs_f32() * factor`.** In f32,
   `0.1 * 2.0` is not `0.2`: a "200 ms" wait came out 200000003 ns. It is
   integer nanoseconds now, which is the fix that makes the policy testable at
   all -- a wait that cannot be stated cannot be asserted.
4. **The cache's age had two sources of truth.** A stamp file holds whole
   milliseconds, and `put` returned the full-precision `SystemTime::now()` it had
   just written -- so the age in the caller's hand and the age on disk differed by
   up to a millisecond. Nothing in the launcher cares about that much time; what
   it costs is a test that can compare the two values at all, and "the stamp
   moved" is exactly the assertion the revalidation path needs. One function,
   `confirmed_at`, truncates now and both write it. How it was found matters more
   than the fix: a test written around a 40 ms TTL and an 80 ms sleep failed about
   one run in ten under load, because the assertion before the sleep was about the
   *machine's* scheduling as much as the cache's behaviour. Widening the margin
   would have hidden it. Making the staleness structural, with a TTL of zero,
   turned a flake into a failure that reproduced every time -- and the failure was
the two-sources-of-truth bug underneath.

Two more the tests caught in the scheduler slice, both about ordering rather
than arithmetic: `submit` has to raise the outstanding count *before* the job is
queued, or a burst of submits can race the `Idle` that says the queue is
finished; and `Scheduler::shutdown` must cancel before it drops the queue, so the
jobs still in flight see a cancelled token and report instead of being counted as
work nobody stopped.

What stage 4 does **not** have yet, named rather than implied:

* **The desktop's call sites on the engine.** Both metadata sources now exist
  (`engine::piston` and `engine::modrinth`, gated by G71-G74) and nothing calls
  them: `browse.rs` still reaches Modrinth with its own `reqwest` client and
  `launch.rs` still reads Prism's mirror. That is the one thing in the tree which
  makes "one pooled client" not yet literally true, and it moves when the pages
  do -- the install paths beside it want the scheduler and the content store too,
  which is the slice after next rather than a rename.
* **The seam that lets a page use any of this.** No `Command` or `Subscription`
  in `store.rs` is wired to the engine, so the pages still answer with the
  sentence that names stage 4 -- which is why this section can be this long
  without a single page changing. It is the next slice, and it is the one that
  turns all of the above into something a person can see.

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
