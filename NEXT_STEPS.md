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
start.** Fifty-four runs on this branch at the time of writing, one per push from
`82232f9` onwards -- `36038333030` the first, `36331380300` the latest -- died in
three to six seconds with zero steps and the same message -- `recent account
payments have failed or your spending limit needs to be increased` -- which is a
billing state and not a verdict on the tree; none of the jobs was scheduled, so
none could report the expected `test result: ok`. One push left nothing at all to
read: `21abf30`, which the next push superseded inside ninety seconds, because
`ci.yml` sets `concurrency: cancel-in-progress` and a superseded push leaves no
run object behind. The tree
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

The workspace is now three crates plus the shell:

| Crate | What it is |
| --- | --- |
| `palantir-core` | Minecraft's own formats (version JSON, libraries, rules, asset index, launch arguments) and the data-root layout. Still Prism-shaped in `ini`/`settings`/`pack`/`instance`; those go with the importer. |
| `palantir-net` | Auth, downloads, metadata, the Modrinth API client. |
| `palantir-loader` | Forge, Fabric, NeoForge, Quilt, and modpack archives. |
| `palantir-desktop` | The window: shell, pages, engine glue, platform code, and `model.rs` -- the view-model that used to be `palantir-gui`, moved here with the crate retired. |

Backend suites as run against this tree: `palantir-core` 168 plus 8,
`palantir-loader` 31, `palantir-net` 213, with 13 live tests ignored by design --
879 in the workspace, of which the desktop crate's 455 plus its 4 native tests
are 459. Which binary each number belongs to is written out in
[`GATES.md`](GATES.md), because a bare list of numbers is how the earlier version
of this paragraph managed to mislabel three of them. The same numbers are on a
page as well as in a table: `python tools/progress.py --dashboard` writes the
stage cards, the whole gate ledger and the code sizes to `.scratch/progress.html`,
and `python tools/dashboard.py --check` is what keeps it carrying every gate
(G92).

## The plan, and where each stage stands

| Stage | What it is | State |
| --- | --- | --- |
| 0 | Prune what nothing references, and reorganize the documents | **Done** |
| 1 | The generated design system: `tools/gen_theme.py` compiles the reference's CSS custom properties, Tailwind's default theme and the component transition blocks into a `theme_gen.rs` the shell paints from, plus a motion table; `tools/gen_icons.py` compiles the 313 vendored SVGs into strokeable geometry | **Done** |
| 2 | The shell rebuilt on the reference's own information architecture: rail, head, page pane, right panel, a `Route` tree with children, Settings as a modal | **Done**: the `Route` tree, the tween engine, the icon widget, the copy, the colour theme and the shell itself are in, it can launch an instance (G81), and **it is what a plain run gets**: the shell it replaces asks for itself with `--classic` (G82). Its right panel draws its first section now (G83); the rest of the panel's sections are service answers and are named in "What stage 3 has landed so far" |
| 3 | Pages, in the reference's order: instance pages first, then project, Home, Discover's six tabs, Skins, Screenshots, Servers, User | **In progress**: all eight page modules are in and the pane draws them instead of the placeholder; their controls tween their hover off the shell's own interaction clock (G76), Settings offers the reference's colour themes (G77), and the right panel's first section -- *Playing as* and its accounts card -- is drawn (G83). Two pages ask the engine now: Discover's search (G75) and the project page's own document, team and version list (G96), and that page's Install button is real: it installs the version that matches an instance the reader picks (G97), and for a pack it makes the instance itself (G98). Home is the welcome screen on a first run (G103), and the Skins page is the account's own: it draws the skins and capes Minecraft says it owns, read from that service (G104), and puts any of them on -- or takes a cape off -- with the same token a launch holds (G106), leaving one measurement of the reference's own arm-style test unclaimed (G105). The right panel draws four of its five sections now -- the getting-started checklist and the friends sentence a reader with no Modrinth session sees (G102), Modrinth's news feed, four articles and the link to the rest, with the opener those links needed (G101). The profile page is real too, off Modrinth's own *published* API rather than the internal user service the reference reaches it through: the header's facts and the projects list are read, filtered and drawn (G108), with collections and organizations left named as absent because they need the sign-in. What is not real yet is Servers with an instance's hosting half and the panel's fundraiser banner and the friends list's signed-in half -- all of them behind a Modrinth sign-in this launcher does not have, which G105 measured; those two documents have since been checked against the live service rather than its fixtures, and the check turned up the rule that makes Servers measurable (G110) -- see "What stage 3 has landed so far" |
| 4 | The backend engine: one pooled client, a scheduler, resumable and cancellable downloads, one TTL'd metadata store, a hash-keyed content store, Modrinth's metadata | **Done**: one client with one ceiling, one retry policy, cancellable and resumable transfers, a work queue where every job reports, a metadata cache that revalidates instead of re-downloading, a content store where a file that is already here is never fetched twice, Mojang's piston metadata read directly and checked against its own digests, and Modrinth's API on the same cache and ceiling are all in and gated (G66-G74). Discover's search is the first page served by it (G75); and the *launch* is on it too -- every library, asset object and Java runtime it fetches goes over the engine's own queue, resume, digest check and ceiling (G91), as do a modpack's own file list (G93), an installed project's own file (G97) and a pack's own archive (G98); the panel's news feed is a document on the same cache (G101); and the metadata a *launch* resolves through is the publishers' own: Fabric's and Quilt's launch profiles, read per game version over the same cache and client (G94), and Minecraft's own version file, read from piston and translated into the shape this launcher's model resolves (G95); and the two Forge-shaped loaders install from their own jars too -- each build's launch profile read out of its installer and translated the same way (G99), with the installer's own processors run at install time over the same queue, resume and digest check (G100) |
| 5 | Instances in our own format, with importers for the popular launchers | **In progress**: an instance can be created from the library or the rail's `+` and the reader lands in it (G78), for any version Mojang publishes rather than only the current one -- the dialog's picker lists them, searchable, with the snapshots behind its own footer (G80); the welcome screen's import button lists what the other launchers on this machine hold and brings one in (G79); and Play launches: the page reports it, the shell builds the run from the launcher's own files, the worker installs, signs in and spawns the game, its facts come back as `LaunchEvent`s, and the header follows the run from *Starting* to *Stop* and back (G81); the reference's custom-setup step draws its own modloader chips and the loader-version row, and what they choose is written into the instance's pack profile (G84-G85); a run is watchable from *any* page through the action bar's chip, its level and its stop control (G86); and the shell this one replaces is **deleted** -- `app.rs`, its glyphs, its settings page and the carved Prism art -- with Windows' own frame handling and the `--shot` capture it owned now this shell's (G88), and nothing the deleted shell was the last caller of was left behind (G89). **Done**: the launch surface closed the stage -- the bar watches several runs at once through a popover over every one of them, and the download manager's job list is every job rather than the run's own (G90) |

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

Home's first state is the reference's welcome screen (G103), and it is the whole
page rather than a card in an empty library: the hero's icon, *Welcome to
Modrinth* at `text-2xl font-semibold` over *Ready to start playing?*, the create
button with the quick-create hint under it, and *Escaping another launcher?* with
the import button at the foot. Three things are this launcher's, each written down
where it is drawn: the hero's icon is this launcher's own art (the vendored
`assets/` has `branding/` and `external/` and no `welcome/`, so the reference's
`modrinth-social-icon.png` is not in this tree), the dot pattern behind the hero is
not drawn (iced 0.12 has no overlay widget, so a pattern could only be above or
below the hero rather than under it), and neither button is disabled when the
machine is offline (this launcher has no online signal anywhere, and what an
offline reader meets instead is the flow's own failure). The hint is also a fix:
the table keeps `Press <shortcut>N</shortcut> to quick create an instance`
verbatim, so the sentence is split by `text::tagged` and the slot is the
reference's own `kbd` chip -- until now the first reader of the first run saw the
tags themselves. And the key the hint names is real: `n` opens the creation flow,
listened for only while the welcome screen is up, which is how the reference's
`event.target` guard is kept by a launcher with no focus signal.

The Skins page is real for the account a launch would sign in as (G104), and its
source is Minecraft's own service rather than any part of the reference that is in
this tree: `pages/Skins.vue` draws through a Tauri plugin whose Rust is not
vendored, and what that plugin wraps is
`api.minecraftservices.com/minecraft/profile` -- one document that names the
account and lists the skins and capes it owns, with the `ACTIVE` one marked.
`palantir_net::MicrosoftAuth::skins` reads it, in the crate that already signs this
launcher in; which account that is comes from the shell's own selection, through the
request seam the project page opened (G96) -- `Ask::Skins` carries a round and
nothing else, because a page has never seen an account file or a token. The texture
of the skin in force comes over the engine's own pool, and `crate::skin` cuts it
into the front view the page draws: arithmetic over the format's own layout, because
the reference's 3D model is a renderer this launcher does not have. Three departures
are named in that module -- no rotation or lighting, no second layer (hat, jacket,
sleeves, trousers), and four-pixel arms for both variants -- and two more in the
page: the account's skins are listed by their *variant* because Minecraft's document
names each one by its id and nothing else, and the sections above them are still
Modrinth's bundles, whose skins are not here. The half that changes anything landed
next as G106, and the seam it widened is the same one: a page's *write* is the same
kind of value `Ask::Skins` is -- `Ask::Wear` carries a round and one
`palantir_net::SkinChange`, and the account and token stay the shell's, because a
page that could make the request would have to hold both. The page refuses a second
press while the first is in flight (the rows draw their Apply unusable), a failure
comes back as a sentence in the slot every other failure goes, and a success comes
back as *silence plus a reload* rather than as invented copy: what changed is the
document, so the document is what is read again.

The **User profile page** is real (G108), and it is the one page of stage 3 whose
data turned out to be reachable without a Modrinth session. G105 measured that the
reference draws it through `plugin:users|get_user_profile`, which wraps Labrinth's
*internal* v3 user service -- but Modrinth's **published** v2 API answers the same
account: `GET /v2/user/{id or username}` returns the id, the username, a nullable
display name, an avatar URL, a bio and a creation date, and
`GET /v2/user/{id}/projects` returns the projects that account owns, keyed by `id`
where a search hit keys the same field as `project_id`. Both are now read
(`ModrinthApi::user`, `ModrinthApi::user_projects`), the page draws the header the
reference's own computed properties produce -- the project count, the **sum** of
the projects' downloads, and the join date, all arithmetic over the one list the
service gives, because the API publishes no total -- and the list under it is the
projects themselves, filtered by the type its address names. `Store::user` is three
requests for that one answer (the profile by the name the address spells, the
projects by the id only that document carries, the avatar over the engine's pool),
and the avatar's failure is deliberately not the page's: a machine with no
connection still has a name, a bio and a list, which is the split
`skin::Appearance::of` already makes for a doll. Two things the page found are
worth keeping: `name` is **nullable** in the live document (`user/modrinth`
answers `"name": null`), which `#[serde(default)]` does not cover and
`modrinth::null_as_empty` now does; and the strip of filters is the reference's
*third* order -- `PROJECT_TYPE_ORDER` puts mods first and modpacks fifth, unlike
Discover's tabs -- whose links the reference spells plural (`/user/x/mods`), so
the address grammar now reads both spellings while `collections`, the fourth link,
is still refused rather than guessed at. What is *not* here is every part of that
page that needs the session G105 found missing: collections and organizations, the
reader's-own empty sentence (which needs an account to compare the profile's id
against), and the header's Edit and overflow actions. The page's one picture is the
avatar; its rows carry every word the service publishes, and no icon.

`pages/project.rs` is the second page to *ask* rather than draw a shape (G96),
and the request seam now has two users rather than one: a page describes what to
ask for out of `update` or `opening`, the shell carries it to the engine off the
frame thread, and the answer arrives as a message of that page's own. Modrinth
splits one project three ways -- the project document names a team but no person,
the author is a member list, and the versions are a list endpoint of their own --
and the page is one `Load`, so `Store::project` makes the three requests and
answers with the one thing the page draws. The translation lives in the page
(`Project::from_api`), where `Hit::from_api`'s does: what a page *is* belongs to
the page that draws it. A team that cannot be read is the one failure that does
not fail the page: the author is a caption under the title, and a project with no
caption is still a project.

The Install button is real now (G97), and the rule behind it is a measurement
rather than a preference. Modrinth's version list answers publish-date
descending, so the version to install is the *first* one that matches the
instance -- not the newest release, which is the reading that looks safer and is
wrong: the two rules differ on 9 of Sodium's 41 game versions and 298 of
fabric-api's 389, because a project publishes betas ahead of the release for the
same game version. A mod matches its instance's loader (a mod published as a data
pack as well is the second pass), everything else matches the game version
alone, and a version with no file at all is skipped rather than installed as
nothing. The file lands in the folder its kind names -- `mods/`, `plugins/`,
`resourcepacks/`, `datapacks/`, `shaderpacks/` -- fetched over the same `Wire`
the launch uses, so a mod and the launch that will load it are one connection
pool and one ceiling apart, and a file already here under the digest the API
published costs no request at all: that is what the store's test asserts by
pressing twice and counting three requests both times. A pack is refused by name
rather than by silence, because one of those becomes an instance of its own, and
that -- not the mod-shaped install -- was what stage 3 owed until the pack case
landed (G98), which is the other half of this paragraph.

The pack case is the same seam with one more step, and its rule is the one place
the install does *not* ask the instance anything: a pack carries its own Minecraft
version and its own loaders in its index, so the version to install is simply the
newest released one -- releases, then betas, then alphas, whatever game version
they name. Measured against the live service: Fabulously Optimized has 473
versions, the rule picks `14.1.0`, and the archive's index names Minecraft **26.2**
with `fabric-loader 0.19.5` and 51 files to fetch -- a version no existing
instance would have matched, which is exactly why nothing is matched. What the
install then does is a cache and an unpack: the archive is fetched into
`cache/meta/packs/` under its published digest (so a second press costs a read),
the loader crate writes the instance with the pack's `dependencies` as its
`mmc-pack.json` and its `overrides/` tree at the instance root, and the files the
index lists go over the same `Wire` as everything else, mirrors tried in order.
The reader is left in the instance it made, which is what the reference does.

The panel's third section is the getting-started checklist (G102), and it is the one
section whose data is not a service's answer: `onboarding-checklist/index.vue`
draws three steps whose flags arrive from a Tauri plugin, and two of the three are
facts this launcher already holds -- an instance exists, and an account is signed
in. The third is Modrinth's and is `false`, because this launcher has no Modrinth
sign-in at all; the step is drawn outstanding rather than quietly ticked. The
plugin's own `show_checklist` flag is the one rule in this tree that cannot be
read, and `checklist.rs` writes down both the reference's structure and the
reading this launcher uses instead: the section is up while a step it can *finish*
is outstanding -- the step nobody can finish here does not pin the card down
forever, and its prompt is not lost, because the friends section beside it is
exactly where the reference draws the same sentence for a reader with no Modrinth
session. Landing that corrected G83: *Playing as* was drawn one step early because
the flag that gates it is the checklist's, and it is the checklist's own second
fact now, so a launcher with no account draws the steps that lead to one instead.

`store.rs` is the seam that makes the pages honest rather than finished. It
answers what the launcher can already answer -- the instance list and each
instance's own folders, from the same readers the old interface uses -- and what
it cannot answer it says so about, in a sentence that names the thing that is
missing without naming a stage. The reason is that the alternative was to draw an
empty list, and a page that shows "no results" when the request never happened is
the failure mode the whole scaffold exists to prevent.

It is also, now, where a page reaches the engine. The store owns an optional
`Engine` built over the cache directory the rest of the launcher already uses,
and `Store::search` asks Modrinth through it. The call is *blocking on purpose*
-- one `std::thread` and a oneshot channel, so no page has to hold a runtime or
know what async is -- and what comes back down is an ordinary message, which is
the shape everything else in the interface already has. Discover asks by round
and applies an answer only if it is still the round it asked for, because two
keystrokes are in flight over a channel that does not keep their order.

The right panel is drawn now (G83). `App.vue`'s `app-sidebar` is the reference's
own column under its two-stop wash, with the hairline down its page edge and the
one scroll region the sections stack in; the first section is *Playing as* with
`AccountsCard.vue` under it -- the reference's empty card when this launcher
holds no account, and its accordion when it holds one, naming the account a
launch would sign in as. The card's controls write the same `accounts.json` the
other launcher reads, so an account chosen here is the account that launcher
signs in as too. What the reference draws and this does not is the player heads
(there is no head renderer yet -- the Skins page is a placeholder for the same
reason) and the Microsoft sign-in flow, which the card's two controls say is not
built rather than doing nothing. The section is drawn one step early: the
reference gates it on the onboarding checklist's own `hasLoggedIntoMinecraft`,
and that checklist is one of the sections that are not built.

The panel's second section is Modrinth's news feed (G101). `App.vue` reads
`https://modrinth.com/news/feed/articles.json` -- the one Modrinth document on
this engine that is not the API -- and draws the reference's own heading, the
feed's first four articles as cards and a button to the news page. The shape was
measured rather than assumed: 45 articles, five fields (title, summary,
thumbnail, date, link), everything defaulted so one article without a summary
costs a paragraph rather than the section. The date is drawn the way the
reference's `dateStyle: 'long'` does -- `September 7, 2026`, from a twelve-name
table and the string's own first ten characters rather than a date crate -- and
the feed is believed for the project TTL, not the search TTL, because an article
half an hour old is still the article under the reader's nose.

The section needed something the launcher did not have: an **opener** (`open.rs`),
because two of its three controls are links and a control that does nothing is the
one thing this shell refuses. It is the only place this starts a program that is
neither Minecraft nor Java, so its rules live there: `http` and `https` only, each
followed by `//`, everything else refused with a sentence -- the feed is a
stranger's JSON, and `file:///C:/Windows/System32/cmd.exe` arriving as a string
must not become a program. The command is built per platform by
`command_for(platform, url)` so all three are asserted on whichever machine runs
the tests, and the Windows one carries `start`'s empty title argument, without
which a URL with a space in it opens an empty window named by half the address.
Nothing waits for the browser. The thumbnail is *not* drawn -- nothing in this
launcher fetches a picture yet -- and an empty or failed feed draws nothing at all,
which is the reference's own `v-if="news.length"`.

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

* **Servers and the hosting half of an instance are not real.** They say so out
  loud, and a server project is the one kind the Install button still refuses -- it
  has no folder to land in and it is not a pack, which is the letter of
  `ProjectType::target_folder`'s rule. The control *states* they will be asked with
  are live, which is the part that makes the request a one-line change rather than
  a page rewrite -- and what the project page's slice (G96) turned out to be:
  Modrinth splits one project three ways, and the page was already the one `Load`
  the three fill.

  The **Skins** third of this item is finished, and it took two slices. G104 read
  what *Minecraft* publishes for the signed-in account -- the skins and capes it
  owns, and which of each is in force -- and cut the skin in force into a front
  view it draws. G106 is the writing half: a row's Apply puts that skin or cape on
  the account through Minecraft's own skin service (`POST …/profile/skins`,
  `DELETE …/skins/active`, `PUT` and `DELETE …/capes/active`), with the same game
  token the launch already holds, and the page reloads the document afterwards so
  the check moves to the row that is now in force. What is deliberately *not* built
  is the reference's file upload, which is a dialog and a multipart body, and its
  edit modal, which is where `unequip_skin` is reached from -- the client can take
  a skin off and the control that would ask for it is the modal this page does not
  have. Both are named on the page rather than approximated.

  G105 measured which of what is left is *reachable* and which is not, because
  they are not the same kind of gap and the plan asked for that decision rather
  than for another slice. Two of the four need the app's own Modrinth session,
  which this launcher does not have. Modrinth **Hosting** and the **Servers** page
  are one Labrinth service whose product list the frontend asks for as
  `client.labrinth.billing_internal.getProducts()` -- `_internal`, outside
  Modrinth's published API. The panel's **fundraiser banner** is the same word
  again: `client.labrinth.campaign_internal.getPride26()`. And the friends list's
  **signed-in half** is four `plugin:friends` calls that authenticate through
  `plugin:mr-auth`; `plugin:users`, which is Labrinth's own user service behind
  those pages, is the third of the namespaces involved.

  The surfaces that measurement found *reachable* were the Skins page's writing
  half, which G106 took, and the profile page's published-API half, which G108 took:
  the header's facts and the whole projects list are Modrinth's own v2 documents,
  and only collections, organizations and the reader's-own state stay behind the
  sign-in. What that leaves is a choice rather than a debt, and **G109 measured the
  choice** rather than leaving it as a phrase. Modrinth publishes two ways in: a
  personal access token (`Authorization: mrp_…`, one scope per request, no
  registration) and OAuth2 (`https://modrinth.com/auth/authorize`, then a urlencoded
  exchange at `POST api.modrinth.com/_internal/oauth/token` with a registered
  application's client secret, answering `{access_token, token_type: "Bearer",
  expires_in}` and no refresh token). Two corrections came with it: `_internal` is
  not by itself a mark of unreachability -- the published guide *documents* that
  exchange -- and the Servers half is not Labrinth at all but **Archon**, a second
  host with eleven versioned namespaces, 67 distinct methods, a websocket and an
  SFTP handoff, with `billing_internal.getProducts` only the price list beside it.
  A token would live in this launcher's own file (`PalantirPaths::home`, beside
  `prefs`), not in `accounts.json`, which is Prism's; and a token in a file is a
  plaintext secret, which is part of the cost. Nobody has to re-measure either way  before starting, and the open stage-3 items stand at two gates as before. **G110
  then checked the two documents G108 reads against the live service, field by
  field, with no account**: the same `id`, `username`, `name`, `avatar_url`, `bio`,
  `created` and the projects list keyed `id` come back for an ordinary account as
  they do for the official one (`name` is null on both, so the nullable field is
  not a quirk of one), and the header's summed download count is real arithmetic
  over a real list. Two things came with it: Labrinth's errors are
  `{error, description, details}`, so the engine's `HTTP <status>` currently drops
  the service's own sentence; and **Archon answers nothing at all without
  `X-Panel-Version: 1`** -- 426 for every path until the header is sent, which is a
  rule that lives in the api-client package this tree does not vendor. That package
  (`@modrinth/api-client`, one directory upstream of the vendored `app-frontend` at
  the same commit) is where every Archon path and base URL is written down, which
  makes vendoring it the first step of the Servers item rather than part of the
  sign-in decision. One measurement from G105 is
  still unclaimed and stays named here so it is not lost: `helpers/skins.ts`'
  `determineModelType` decides slim from classic by reading one 2x12 column of the
  arm at (54, 20) and asking whether any pixel in it is opaque -- the exact
  question the cutter in `skin.rs` is already holding the pixels for, and the one
  thing that would let a row say which arm style it is without the document having
  to.
* **The panel's other sections are not built.** The panel is no longer a wash
  (G83): Discover, a project and a profile force it on (`App.vue`'s
  `forceSidebar`), and it draws `app-sidebar`'s scroll region with four of its
  five sections -- the getting-started checklist (G102), *Playing as* with the
  accounts card and the checklist's own gate on it (G83, G102), the friends
  sentence a reader with no Modrinth session sees (G102) and the news feed with
  its four cards and its button to the rest (G101). What is absent rather than
  drawn empty is the fundraiser banner, whose campaign is served by an endpoint
  this launcher has not been given -- and the friends list's *signed-in* half,
  which is Modrinth's authenticated friends API and needs the sign-in flow.

**The interface can be more than English now** (G120), and the number that decided
its shape was measured rather than guessed. `tools/gen_text.py` compiles the
reference's English; `tools/gen_locale.py` compiles the other locales and imports
that tool's ICU parser rather than copying it, because a locale the generator
accepts and a locale the runtime renders have to agree about what a message means.
The corpus is 89,177 leaves across the 33 trees -- 2,700,238 bytes of translated
text, against 33 x 3846 = 126,918 slots, so 70.3% translated -- and shipping all of
it costs 2,878,592 bytes of data, +4,612,948 bytes on the debug artifact and about
+16s on a crate rebuild, so all 33 tables ship. A table is the reference's own
sparse shape, `(position in text_gen::ALL, template)`: every locale's keys are a
subset of English's (checked, and refused otherwise), so the key string is already
in the binary once, and a key a table does not carry falls back to English -- which
is the reference's own `fallbackLocale: 'en-US'`. Two claims came out of the slice
corrected. The plural refusal was aimed at the wrong thing: `zero`, `two`, `few`
and `many` are real arms here (6 locales carry `few`, 5 carry `many`, and `ar-SA`
carries all six), so the generator now compiles a locale with the whole CLDR
category set and reports which arms a language's own rule can never select --
`id-ID`, `ja-JP`, `ko-KR`, `vi-VN`, `zh-CN` and `zh-TW` carry `one` arms that
`Intl.PluralRules` resolves as `other`. And **33 trees is 32 offered languages**:
the reference's own `LOCALES` lists 32 codes and has `ar-SA` commented out as RTL,
so the language setting G121 adds offers the reference's 32 while the table for the
33rd is compiled anyway. One real bug the correctness lint caught: the translations
contain 28 invisible characters (`U+200B`, `U+200E`, `U+200F`, `U+00AD`), which
`clippy::invisible_characters` denies; the shared `escape` now writes them as
`\u{...}`, a faithful round trip that leaves English byte-identical.

Two gaps that were on this list are closed and stay named here so the next reader
knows when: the controls tween their hover off the same clock the rail's plate
uses (G76), and Settings offers the reference's colour themes and keeps the one
taken (G77).

## What stage 4 has landed so far

`crates/palantir-net/src/engine/` is the backend the pages will be served by. It
is twelve modules, and each one exists because the launcher has had the
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
| `piston` | `meta.prismlauncher.org`, which is Prism's mirror of Mojang's own metadata rewritten into Prism's shape. Now the launcher reads piston directly, checks every version file against the `sha1` the manifest published for it, and *translates* the file into the shape this model resolves (G95) -- the work the mirror was doing, with the mirror's own answer as the live measurement |
| `loaders` | `catalog.rs`'s Prism-shaped build lists for the four mod loaders. Now each loader's own service answers -- Fabric's and Quilt's build lists *and* their launch profiles per game version (G94), NeoForge's maven and Forge's promotions for the two that mean anything per game |
| `modrinth` | The desktop crate's `browse.rs`, which reaches `api.modrinth.com` with a `reqwest` client of its own -- a second connection pool and a second answer to "how many requests is this launcher making". Discover's search goes through this module on the engine's cache and ceiling (G73-G75), and the desktop's last client of its own went with the pack installer (G93): `palantir-desktop` depends on `reqwest` no longer |

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

What stage 4 held open, and what closed it. The mirror `resolve` used to read
was never a copy of the publishers' files, so moving a question off it is
moving the *translation* with it: the loaders that publish a launch profile
(G94), Minecraft's own file, whose translation is `palantir-core`'s
`version::mojang` and whose measurement is a live test against the mirror for
the same version (G95), and now the two Forge-shaped loaders, whose
installers' own files replaced the mirror's ForgeWrapper rewrite in both
places it stood -- the launch profile, read out of each build's installer jar
and translated the way the game's file was (G99), and the install itself, with
the installer's own processors run at install time instead of at launch (G100).
The mappings components an *imported* instance lists
(`net.fabricmc.intermediary`, `org.quiltmc.hashed`) stay the mirror's: one
library each, from a maven the publisher serves, which is a URL change rather
than a translation. What still points at the mirror is the resolve-time
routing for those two uids (`PublisherMeta` in the desktop crate): the engine
reads and installs from the publishers now, and flipping that switch is the
desktop track's line to move rather than this stage's.

That line is two lines, not one, and the order between them is not a
preference. `published_loader` answers `None` for `net.minecraftforge` and
`net.neoforged` today, so the mirror resolves them -- and nothing outside
`engine::forge` calls `install` or builds an `InstallCtx` at all, so G100's
processors are landed but *unreached from the interface*. Flipping the routing
with the install still unreached would resolve the installer's own profile for
an instance nothing has patched, which is the one order that breaks a launch;
the install has to run first (with the patched client and its declared digests
as the resume test), and the routing follows it.

G107 measured what the mirror's profile actually is, because the claim above
rested on an inference rather than on a document -- and the document changes
the *reason* while keeping the order. Prism's Forge and NeoForge profiles name
`io.github.zekerzhayard.forgewrapper.installer.Main` as their main class and
ship the loader's own installer plus its tools in a second key,
`mavenFiles`: they are **self-installing** at first launch, which is why the
mirror answers those uids without any launcher-side pre-install. So a Forge
instance is not "a profile whose patched client does not exist"; it is a
profile that asks a wrapper to do the install on the first run. What *this*
launcher does with that key is the finding that matters: `palantir-core`
parses `mavenFiles` (Prism's own semantics -- merged, and deliberately not on
the classpath) and `install::plan` fetches `libraries`, `native_libraries` and
`main_jar`, so the installer jar ForgeWrapper needs is never fetched here.
Whether the wrapper then fails or fetches it itself is not decidable from this
tree; what is decidable is that the flip is the *only* path this launcher can
finish, because the installer's own translated profile needs no wrapper and no
`mavenFiles` at all -- its libraries are the real launcher stack and the patched
client is a product G100's processors already produce and digest-check.

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

The Prism-shaped modules in `palantir-core` stay until the shell and the importer
that replace them exist, because `app.rs` calls them today. Removing them now
would mean either a launcher that does not build or a half-migrated one that does
not open the instances it already has.

`palantir-gui` is no longer one of them. It was the same kind of hold-out and it
turned out to be the one with a floor under it: its only dependant was this crate
and its reason to exist was a CLI that is gone, so it moved in whole as
`crates/palantir-desktop/src/model.rs` and the crate went. Three of its readers
are the ones that mattered -- `instances.rs` loads the list, `launch.rs` resolves
a run through the override gates, and `app.rs` builds a row -- and the fourth is
its own tests, which came with it. What the move also cost is the half of that
model nothing called: four list methods the library's own page supersedes and the
whole write side of the settings model, which belongs to the instance-settings
page stage 3 still owes.

## What stage 5 has landed so far

Six pieces. Five of them replaced a sentence that said the feature was not
built; the sixth (G89) is the delete's other half and added no feature at all:

* **A create, and the pickup behind it** (G78). `store.rs` gained the write side
  -- `create_instance` asks Mojang which version is current when the flow has no
  version of its own, writes the instance through `instances::create`, and the
  shell reads the library again so the new instance is on the page that made it.
* **The import, and the empty case** (G79). The launcher's own `find_importable`
  scan, run when the step opens rather than per frame, drawn as the reference's
  own rows and as its *no instances found* copy when this machine holds none.
* **The version picker** (G80). The list is a request, not a field read: one read
  of Mojang's manifest answers both what versions exist and which one is current,
  the picker filters it the way the reference's combobox does (releases, then
  everything behind the footer's own button, and a substring search over the
  ids), and what the user picks is what the instance is created for.
* **The delete's other half** (G89). G88's delete exposed what only the old
  shell was calling -- 199 dead items, 67 of them in `theme.rs`'s palette and 38
  in `browse.rs` -- and this is that list worked to zero: neither the shipped
  binary nor the test build reports a dead item now. What a test still reads
  stayed, as `#[cfg(test)]` rather than deleted, because it is the record of
  what was measured off the reference; 215 lines of marker went in for those and
  their like, and the slice is +378/-1,842 across 21 files. The one case a hand
  could not prune was `text_gen.rs`: generated output CI checks byte for byte,
  so the allowance for its warnings is `tools/gen_text.py`'s to emit, and it
  does -- the `enum_variant_names` allow, and 53 single-character `push`es. The
  32 tests whose only subject was deleted went with them (903 tests to 871).
* **The launch** (G81). The worker has always been finished; what was missing was
  the seam. It now speaks `LaunchEvent` -- facts rather than one shell's messages,
  which is what let a second shell watch a run at all -- and the instance page
  reports `Ask::Play`/`Ask::Stop` rather than performing them, because the data
  root, the account and the settings a launch is for are the shell's. The pages
  follow the run through the store: four states, and the run's own last line.
  What is drawn in the header rather than in a bottom action bar (that surface is
  not built), with no stop-circle glyph on *Stop* (this kit's buttons are
  label-only), and with no gate that has actually run a game on this machine --
  that needs a JVM and a version to install.
* **Several runs at once, and a job list that is more than the run's own** (G90).
  The bar could watch one run from any page (G86); what it could not be is *more
  than one*, which is what the reference's action bar is for. `run:
  Option<ActiveRunData>` and one shared child slot became `runs: Vec<Run>` with a
  `ChildSlot` per run -- a stop is aimed at an instance, so the kill has to reach
  the child that instance's launch put in *its own* slot -- and `launch: Launch`
  became `launches: BTreeMap<String, Launch>` whose selected id the reader
  derives. Every `LaunchEvent` is routed to its run by `run_id`, so a stale event
  cannot write to another run's chip; the popover over the running processes is
  the reference's own `currentProcesses.length > 1` condition, one row each, and
  pressing a row is what makes a process the one the bar is about; and `jobs:
  BTreeMap<String, install::Progress>` means two instances installing at once are
  two rows in the download manager and a count on its chip.

## What stage 5 does not carry over, and why

The stage's own list is done. What follows is what the reference has in the same
places and this launcher does not, and none of it is a slice the plan asked for:

* **The download manager's per-job controls.** Pause, resume, retry, cancel,
  dismiss and copy-details are all in the reference's job row and none is here,
  because a job in this shell is a running launch's own phase: there is nothing
  behind a pause, and the words a stopped phase would answer are the run's, on
  the chip and on the popover row. A job that outlives its run -- the reference's
  completed and *needs attention* sections -- needs a job store that outlives the
  run too, which is the engine's scheduler wearing a UI rather than this
  surface's work.
* **The float.** The reference teleports both the popover and the panel above
  everything; iced 0.12 has no z-order, so both are rows under the head here,
  which is the same wall the version picker and the create dialog hit.
* **A picture of the popover.** It is drawn only when a second run exists, and a
  `--shot` run cannot start two real launches: what the gate has is a test that
  renders the whole surface in that state beside a capture proving the surface a
  plain run draws is unchanged.
