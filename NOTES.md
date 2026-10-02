# Notes: what was measured, and what it cost

The engineering record for this launcher: the things that were learned by
measuring rather than by reading, kept because they are true of any launcher
built this way and not of the port they were learned on.

**Every section keeps the number it had in `NEXT_STEPS.md`.** The gaps
(1, 2, 5-7, 11-15, 17, 18, 24-26) are deliberate: those sections described the
Prism-shaped formats, the port's page-by-page history, or verification state
that has since been superseded, and they were dropped when the rewrite began.
Numbers are kept because source comments across `crates/` cite them --
`// see NEXT_STEPS.md 15` and its like -- and renumbering would silently break
every one of those references. `NEXT_STEPS.md` says where each dropped section
went.

The measurements the shell is held to live in `REFERENCE.md`, the evidence that
a build met them in `GATES.md`, and the contract for working in this tree in
`AGENTS.md`. This file is the fourth thing: why the numbers are what they are.

---

## 3. What has never been looked at on a real display

The code below exists and typechecked before this pass, but no run of the current
tree has exercised it end to end. Everything here is a five-second visual check:

- **Rail tooltips** — hovering the left rail icons should show a label
  (`Screenshots`, `Discover content`, …).
- **Discover page** — five Modrinth tabs (Mods / Resource packs / Data packs /
  Shaders / Modpacks), search box, sort and view selectors, `Install` buttons
  that download and land the file in `mods/`, `resourcepacks/`, `datapacks/` or
  `shaderpacks/` for the selected instance.
- **Screenshots page** — empty state ("No screenshots yet") with no instance
  selected, real thumbnails when an instance has any.
- **Settings dialog** (rail gear) — sectioned nav, Appearance pane with the four
  colour-theme cards (Dark / Light / OLED / Sync with system), and the version
  footer. Picking a theme must repaint the whole shell, and it must still be in
  force after a restart (`prefs.rs` stores it separately from Prism's config).
- **Smooth scrolling** — the wheel must glide to a stop rather than jumping 60 px
  per notch, on a long page (Discover, Logs).
- **Native window** — drag the title bar, drag all four edges and corners to
  resize, double-click the title bar to maximize/restore, and the caption buttons.


## 8. Resource profile: what the shell spent, and what changed

This pass optimised CPU/RAM against the actual iced 0.12 sources in the cargo
registry rather than against intuition, because two earlier rounds of this were
wrong in opposite directions. The facts that drove every decision, each verified
by reading the pinned crate:

1. **A message is what makes the shell rebuild.** `iced_winit-0.12.2`
   (`application.rs`, the `AboutToWait` arm) dispatches the frame's events
   through `UserInterface::update` and only goes on to rebuild when that produced
   a message or reported the interface `Outdated`:
   `if !messages.is_empty() || matches!(interface_state, State::Outdated)`. Each
   message then runs `application.update` and `build_user_interface` — which calls
   `view()` — and a command carrying a widget operation (`scroll_to` is one)
   builds *another* user interface in `run_command`. So a message costs one to
   two full `view()` builds, and a pointer move that publishes nothing costs
   none: it only repaints the widget tree the shell already holds, which
   `iced_tiny_skia` then skips outright if the primitives compare equal.
2. **An unchanged frame is free.** `iced_tiny_skia-0.12.1`'s `present` compares
   this frame's primitives with the last frame's and returns immediately when
   the damage list is empty — no raster, no blit.
3. **A changed frame is not.** When anything is damaged, softbuffer presents the
   **entire** buffer: there is no partial present, so every changed frame pays a
   full-window blit (3.5 MB at 1257x708) plus the raster of the damaged regions.
4. **`adjust_clip_mask` memsets the whole window-sized mask, and `Primitive::Clip`
   calls it twice** (enter and restore) — `iced_tiny_skia/src/backend.rs` lines
   746/757 and 942. At ~900 KB per call that is a per-clip cost, and it is why
   clip count matters. (The reported iced issue #3368 is a nastier variant of
   this in 0.14, where text runs that overflow their clip each re-clear; 0.12.1
   does not call it from `draw_text`.)
5. **The renderer's image cache is bounded per frame** (`raster.rs`
   `Cache::trim` retains only handles hit in the last frame), so thumbnails that
   scroll off screen release their decoded copy. It keeps them in premultiplied
   `u32`, i.e. a second 4 bytes per pixel alongside our handle's RGBA.
6. **`Scrollable::scroll_to` does not notify `on_scroll`** — that callback only
   fires from real scroll input — so a tween frame costs one rebuild, not two.

### What that meant, and what was changed

| Change | Why | Effect |
|---|---|---|
| `app.rs` `grabbable(...)` now takes `armed` and attaches `on_move` only while a press is armed | Fact 1: an always-on `on_move` published a message per pointer move over the title bar for a handler that could do nothing until a drag was armed | Measured at 2.0x the per-move cost removed (31.2 -> 15.6 us, §8) — real, idempotent, and much smaller than this table first claimed; drag behaviour is bit-for-bit identical |
| `scroll.rs` `EASE` 0.28 → 0.45 | Every tween frame is a message, hence a rebuild + draw + full blit. 0.28 needed ~15 frames for one notch and up to 22 for a flick | ~8 frames for a notch, 11-12 for a flick: about half the frames per gesture, monotonic deceleration preserved |
| `screenshots.rs`: thumbnails sized by `thumbnail_side(tile_width, scale)` instead of a fixed 360 | The grid draws 268 logical px; 360 was 1.8x the pixels, and memory is quadratic in this number | ~9 MB less on the page at 100% (our 7.8 MB + renderer copy instead of 14 MB + 4.4 MB), and the sampler is 1:1 instead of downscaling |
| `brand.rs`: the logo is decoded once at 256px instead of handing the renderer the 512px original | It is drawn at 112px at its largest | ~0.75 MB less cached pixels, and a smaller sampling source |
| `native.rs`: added `system_scale_factor()` | So the thumbnail size can follow the display instead of a fixed guess | Prevents the fix above from being soft on a 125%/150%-scaled display |

Two deliberate non-changes: `hero_tile` keeps its 28px blur shadow (removing the
glow would be a visible downgrade for one small widget on one page), and the log
page keeps its 500-row window (scrollback is a real feature, and the cost only
lands while that page is open).

### 8.1 The one test that failed, and what it caught

`brand::tests::logo_is_decoded_at_the_drawn_size_not_the_original` failed on the
first run of the new tree: *"most opaque pixel was 239"* against a bar of 240.
The bar was wrong, and the artwork is why. The `logo512.png` of that run was a
soft radial glow, not a solid disc — measured directly, **exactly 2 of its
262,144 pixels reach alpha 255 and only 102 clear 240**. Averaging that spike
into a 2x2 window is *expected* to land just under it, so a fixed threshold
there measures where the brightest pixel fell on the sampling grid, not whether
the shrink preserved the mark.

Both numbers are the artwork of that run. The mark was replaced on 2026-10-01,
and its successor — solid strokes rather than a glow — measures **1,423 pixels
at alpha 255** in the same 512 tile. The bar stayed relative rather than going
back to a fixed threshold, which is the point of it: the relative bar is the
claim that holds for either mark.

The test now measures the result against the source instead, which is the claim
that actually matters:

- **the glow is not dimmed** — the shrunk peak must be >= 90% of the source peak;
- **the ink is not lost** — total alpha across the image must be conserved to
  within 5% of `source_total / 4`. That is the invariant a bad resample breaks:
  dimming every pixel keeps the shape while washing the mark out, and clipping
  brightens it. The band absorbs the filter's fractional edge windows and its
  per-pixel integer truncation.

Both pass. The useful part is that the first version of this test could not have
caught either failure — it would have passed on a washed-out logo.

### What the acceptance run found (17:34–17:40)

Method, because the machine was in use and the physical cursor was off limits:
events were **posted** into the window with `PostMessageW` rather than injected
with `SendInput`. `winit` 0.29 reads a mouse-move's position out of the message's
`lParam` instead of calling `GetCursorPos`, so the app sees a pointer sweeping
the bar while the user's cursor never moves. The window is parked beyond the
desktop with `SWP_NOACTIVATE` and never focused. CPU comes from
`GetProcessTimes` (100 ns, so it is a subtraction rather than a sample) and
memory from `GetProcessMemoryInfo`. Scripts: `ab_titlebar.py` and
`calibrate_bar.py` in `%TEMP%\palwin`.

| Measurement | Result |
|---|---|
| idle CPU, 1.5 s untouched | **0.0 ms** — both builds |
| working set, 4 real instances | **30–36 MB**, peak equal to resident |
| moves along the title bar, no press armed | **15.6 us each** |
| moves along the bar *with* a press armed | **31.2 us each** (2.0x) |
| adapter wgpu sees | `Microsoft Basic Render Driver \| Direct3D 12 \| hardware=false`, `Intel(R) HD Graphics 4400 \| OpenGL \| hardware=true` |
| renderer decision | **`tiny-skia`**; `accelerated: false` |

**The headline claim was overstated and is withdrawn.** The fix is real — the
armed path is exactly twice the unarmed path, and that doubling is the message
renderer's `view()` rebuild the gate removes — but the absolute size is
**microseconds, not milliseconds**. At a generous 1000 move events/s the whole
path is ~3% of one core. "An unbounded stream of full view rebuilds" was wrong:
full view construction in this shell costs tens of microseconds, because a
message only rebuilds the widget tree and the damage tracker then finds an
identical frame and skips the raster and the blit entirely. The change is still
worth keeping (it deletes work nobody asked for, and drag behaviour is
unchanged), but it was never the reason the machine felt heavy.

**The A/B against the previous binary came back flat** (15.6 vs 26.0 us on the
same sweep, new build nominally *slower*, which is run-order noise in a
single-threaded measurement) and is therefore not evidence either way. It is
recorded as a negative result rather than a win.

**What the instrument cannot see, and why that matters.** Posted messages do not
make Windows re-evaluate `WM_NCHITTEST`, do not generate `WM_SETCURSOR`, and
do not move the real pointer across the caption buttons or the resize edges. All
three happen under genuine cursor motion, and the shim's per-move work lives in
`WM_NCHITTEST`. So every number above is a **lower bound** for real pointer
motion. In particular it does *not* contradict the older measurement recorded in
`gpu.rs` (213% of a core on the wgpu path against 69% on tiny-skia, 464 MB of
commit against 12 MB) — that one used real injected motion, this one cannot.
The two are not comparable, and the renderer conclusion still rests on the older,
stronger measurement.

**What this does answer.** Does the shell use the GPU? On *this* machine, no —
and deliberately. The only hardware adapter is reachable over OpenGL through a
2013 Intel driver, which measured ~3x the CPU and ~38x the committed memory of
rasterising on the CPU, so `gpu.rs` declines it and pins `tiny-skia`. On any
machine with a real Direct3D 12 or Vulkan adapter, iced's own GPU path is used
unchanged. `ICED_BACKEND=wgpu` forces the GPU path back for anyone who wants to
compare on their own hardware.

That the rasteriser really is the live renderer is corroborated by the memory
figure rather than assumed from the test alone: the older measurement put the
two paths at **121 MB resident on wgpu against 22 MB on tiny-skia**, and these
builds sit at **30–36 MB**. A build on the OpenGL path would not fit in that
envelope. (`select_renderer` sets the variable before iced builds its compositor,
and `forced_backend` is unit tested, but nothing inside the process can observe
which compositor won — so an independent physical measurement is the honest way
to close that loop.)

#### Still unmeasured

1. **Screenshots page memory** (~9 MB expected saving) — needs the page opened
   with screenshots present; neither script navigates.
2. **Scroll feel** — a human eye, not a number: one notch should glide and stop
   in ~130 ms.
3. **The real-input comparison** for the title bar, which is the only way to
   settle whether genuine pointer motion costs what the older measurement says.
   `SendInput` would do it but moves the user's actual cursor, so it needs an
   explicit go-ahead from someone who is not using the machine.

---

One retraction is worth keeping, because it was written into the code as a
comment and it cost two rounds to establish:

> A default launch loading `d3d12.dll` does **not** prove GPU compositing —
> `d3d12.dll` loads for WARP too, so that check once certified the slower
> backend as the faster one. The renderer conclusion was re-derived from
> measured resource use instead: with an identical stream of injected mouse
> moves, the wgpu path cost ~3× the CPU (213% of one core against 69%) and held
> ~38× the committed memory (464 MB against 12 MB). Idle is 0% either way.
> The machine measured is an Intel HD Graphics 4400 whose driver exposes no
> hardware Direct3D 12 adapter, which is why `gpu.rs` treats a hardware adapter
> on the legacy OpenGL backend as *not* qualifying.


## 10. Verifying a downloaded build

`tools/launch_check.py` does the part of verification CI cannot: it launches the
downloaded exe off-screen without activating it, proves the UI thread is
pumping (`SendMessageTimeout(WM_NULL, SMTO_ABORTIFHUNG)`), and samples CPU and
working set across a series of windows rather than one.

Run interleaved against both CI artifacts of `6dbc01a`, four launches:

| | gnu | msvc |
|---|---|---|
| window appears | 0.1-0.8 s | 0.1-0.5 s |
| idle windows | 15.6 and 62.5 ms stray | 0.0 ms, all windows, twice |
| working set | 29.6-40.4 MB | 29.3-35.0 MB |
| UI thread | responsive, 0 ms | responsive, 0 ms |

That corroborates §8: no measurable idle CPU, ~30 MB resident, the figure worth
about 1 MB less than the earlier local build. Both hashes matched the sidecars
CI wrote, and both pass `check_exe.py`.

One caveat this raises. A single msvc run showed 296.9 ms over 12 s -- 2.5% of
one core, work in seven of eight consecutive windows -- and looked exactly like
a target difference. It did not reproduce in two later runs of the same build,
and the gnu build produced smaller strays of its own. So it was a transient,
and the series is what makes that visible: a one-window sample would have
recorded 2.5% with no way to know. If it recurs over a longer run it wants a
thread-level sample (ETW or a sampling profiler), not more window arithmetic.


## 9. Scrolling: the measurement, and the fix that is not written yet

Picked up again after the CI work because this is what was in flight. The
number that matters was measured cursor-free on 13 Sep, by forcing the same
work a scrolled frame forces (relayout + full raster + full blit):

> **One full-window frame costs ~68.75 ms of CPU.** That is a 14.5 fps ceiling,
> and the scroll tween publishes a message every 16 ms. It cannot keep up, so
> every extra frame is queued, not drawn sooner -- which is exactly the
> "laggy and not smooth, and burning CPU faking it" report.

Cost is per-pixel, roughly 60-110 ns/px, scaling with what gets repainted; a
scrolled frame repaints the whole content column. Facts that rule things out:

- The renderer is `tiny-skia` (software), decided in `gpu.rs`, and that is the
  *deliberate* choice on this machine: the only hardware adapter is a 2013
  Intel HD 4400 on the legacy OpenGL path, measured at ~3x the CPU and ~38x the
  committed memory of rasterising on the CPU.
- There is exactly one blur in the UI (`hero_tile`'s 28 px glow), so blur is not
  the cost. iced recomputes a per-pixel SDF and builds a new pixmap for it every
  frame, so it is worth watching if the tile ever gets drawn off the Home page.
- `iced_tiny_skia` returns early when damage is empty, but softbuffer presents
  the whole buffer -- there is no partial present. Any changed frame pays the
  full 3.5 MB blit.
- Every `scroll_to` command rebuilds the **entire** interface (a second
  `view()` + layout per frame), confirmed in `iced_winit`'s `AboutToWait` arm.

The design chosen, not yet written into `scroll.rs`: make the tween a function
of **elapsed time** rather than a fixed per-frame easing factor, and pace its
tick to what the machine can actually draw, so the frame count adapts itself
and the timer can never publish faster than frames complete. Step 4 of the
acceptance run in §8 is the one that falsifies it: park the pointer and watch
CPU while one notch glides.

### Correction: that 68.75 ms is a *resize* frame, not a scroll frame

The number above came from resizing the window and timing the app's CPU, because
posted wheel messages are inert at the iced level and real input was not
permitted at the time. A resize is not a scroll, and the difference is specific:
ices's text cache is keyed on the text's *bounds*, and a resize changes the
bounds of nearly every run on the page, so a resize re-shapes the whole page --
while a scroll translates content whose bounds did not change and should hit the
cache. So 68.75 ms is an upper bound for a scrolled frame, and the "60-110 ns/px"
reading inherits the same confound. This is not a retraction of "a frame is
expensive"; it is a correction of *how* expensive, and real input is what has to
settle it.

### What was implemented (13 Sep)

`scroll.rs` no longer counts frames. The tween eases against a **deadline**
(it `DURATION`, 160 ms) and whether to have a tween at all is a function of
**this machine**:

- `tick` measures the interval between its own calls and folds it into a
  per-machine frame cost. The fastest recent frame wins, so one hitch cannot
  demote a fast machine, and recovery upward is capped at a quarter per frame.
- `glide_duration` returns `DURATION` below `SMOOTH_FRAME` (24 ms, ~40 fps) and
  `Duration::ZERO` at or above it. Zero means the wheel's own frame moves the
  offset and **no frame subscription is started at all** -- both cheaper and
  less laggy than an animation drawn at 14 fps, and the case this machine falls
  into once it has been measured.
- `restart()` (used on navigation) keeps the measured cost and discards only the
  page's position, so the machine is not re-learned once per page.

**Superseded (1 Oct, `GATES.md` G140).** The demotion above is gone, and the
frame-by-frame measurement of the two recordings is why. The official client
glides a wheel over two to six frames at ~30 fps; ours arrived in a single
frame, on 14 wheel events out of 14 — and the classifier was itself the cause,
because the frame timer exists only while something moves. The first tick of a
gesture was measured against the last tick of the *previous* one, which is a gap
of seconds, and two such gaps turned the glide off for the rest of the session.
`SMOOTH_FRAME`, `SLOW_FRAMES_TO_DEMOTE`, `frame_cost`, `slow_frames` and
`observe_cost` are removed; `begin` always starts a glide, and the 160 ms
deadline bounds what any machine pays.

`hero_tile` lost its 28 px glow shadow. iced's tiny-skia backend renders a
`Shadow` by computing a per-pixel SDF and building a fresh premultiplied pixmap
from it, uncached, on every frame the page paints -- tens of thousands of `sqrt`
calls and two heap allocations per frame, on a tile that sits on two scrollable
pages. The accent hairline carries the same read for one fill.

Inter is now the shell's typeface: 400/500/600/700/800, subset by
`tools/make_fonts.py` from the release Modrinth's own stylesheet pins, ~292 KB
for all five, loaded through `Settings::fonts` so the first frame is already
Inter. `default_font` is weight 500 because that is what Modrinth sets body text
at.

### Still unmeasured / open

1. **Real wheel input on a CI build**, now that cursor control is permitted.
   This is what decides whether the "resize frame" number above applies to
   scrolls at all.
2. **The Modrinth 1:1 pass.** Tokens are in (palette in `theme.rs`, Inter via
   `Settings::fonts`), but the comparison has been against the extracted
   Omorphia stylesheet, not against the running app side by side.

### The first green run of that work (12dd0ef)

`Test workspace`, `Lint`, `Build exe (msvc)` and `Build exe (gnu)` all passed.
Both artifacts were downloaded, staged into `dist/`, and their sha256 checked
against the sidecars the runner itself wrote -- both MATCH, so the binaries are
byte-for-byte what CI produced. `tools/launch_check.py`, off-screen and
unactivated: up in 0.15 s, 28.8 MB working set, UI thread responsive, and no
idle CPU on the gnu build (the msvc build showed 15.6 ms in two of four windows,
the same bursty transient recorded in §10, not a target difference).

Getting there took three pushes, and CI found three real things, none of which a
local build would have:

1. `&include_bytes!(..)[..]` fails in a `static` on a newer toolchain -- slicing
   needs `std::ops::Index` in const, which is not stable. Local rustc accepted
   it; the runner's did not.
2. My own test asserted the frame-cost rule I had *intended* rather than the one
   I wrote: a single slow frame raised the measured cost, so one page fault
   could have turned the glide off for the session. The rule is now asymmetric on
   purpose -- one fast frame resumes animating, two slow ones in a row stop it --
   because believing a fast machine is slow is the expensive mistake.
3. The light palette's accent did not clear the contrast floor the palette's own
   doc promises: 2.71:1 as text on `#F8F8F8`, and 2.88:1 for white on it. The
   reference ladder has the rung for this (green-700, 3.83:1 and 4.06:1), so the
   accent moved down one and light's `--color-brand` became the hover.

Two of those three were caught before the packaging jobs started, because
`package` depends on `test` -- so a compile or test failure costs about 4 billed
minutes, not the 28 a full run costs. The expensive failure is the one that only
appears in a release build.


## 16. Why an instance this launcher created could not launch

Three defects, all in the path from *create an instance* to *the game starts*,
and all three passed their unit tests because the fixtures supplied exactly the
bytes the code expected. The fix is the code; the reason to read this is the
method, which is now automated in CI.

### 16.1 A component with no version was looked up as `<uid>/.json`

`Instance::create` writes the profile Prism writes: `net.minecraft` pinned, and
an `org.lwjgl3` slot with **no** version. Resolution asked the metadata store for
`org.lwjgl3` at version `""` -- the URL `…/v1/org.lwjgl3/.json`, which does not
exist -- reported the component as unresolvable, and a single unresolvable
component is a hard error, so `prepare_launch` refused to start anything. A
Fabric instance was worse: the loader's own metadata requires
`net.fabricmc.intermediary` with **no version and no component listing it at
all**, and nothing added it.

Both are one rule in Prism (`ComponentUpdateTask`): a component that names no
version takes the version something else requires of it (`equals` outranks
`suggests`), and a component something requires but the profile does not list is
added. Two uids — `net.fabricmc.intermediary`, `org.quiltmc.hashed` — are
mappings that follow the game, and Prism resolves both to the Minecraft version
being launched. All of that now happens in `palantir_core::resolve`, in rounds:
load what brings its own version, then fill what the requirements decide, then
add and resolve what was only required, until nothing changes. What is still not
decidable from the metadata is still an error, and there is a test that holds
that line, so the fills cannot have been bought by inventing a version.

**The compat fixture was hiding it.** `seed_meta_cache` called
`profile.set_version("org.lwjgl3", "3.3.3", true)` before resolving, with the
comment "so offline resolution stays clean" — the test was pinning by hand the
exact version the launcher never pins. That call is gone; the test now asserts
the slot is still versionless before resolving, which is what makes it a
regression test rather than a fixture.

### 16.2 The version-list URL was a layout the service does not serve

`OnlineMetaStore::version_list_url` built `{base}/{uid}.json`. Every uid answers
**404** there: `net.minecraft.json`, `org.lwjgl3.json`,
`net.fabricmc.fabric-loader.json` all 404, while `{uid}/index.json` returns the
list. The desktop's own `catalog.rs` had the right layout with the flat shape as
a fallback, which is why the create dialog could list Minecraft releases and
loader builds while the `MetaStore` implementation could not. Both stores, and
both cache paths, now use `<uid>/index.json` and still *read* a flat file a
cache written earlier holds, so an existing install does not re-fetch every list
it already has.

### 16.3 The loader was installed as a synthesized patch, which deleted it

`instances::create` wrote `patches/<uid>.json` built from a table of main
classes. A patch does not add to a component, it **replaces** the metadata's
version file for that uid — and the synthesized file had a `mainClass`, some
`+traits`, and **no `libraries` at all**. So the loader jar, its ASM stack and
the mappings it needs were never on the classpath, and the game died on the first
missing class. Two of the four entry points were invented rather than copied:
Forge and NeoForge start through `io.github.zekerzhayard.forgewrapper.installer.Main`
in the metadata, while the table said `net.minecraft.launchwrapper.Launch` and
`cpw.mods.bootstraplauncher.BootstrapLauncher`.

Checked against the service, all four loaders are self-sufficient when
registered as a component with a version:

| uid | mainClass | libraries (1.21.1 / 1.12.2) |
|---|---|---|
| `net.fabricmc.fabric-loader` | `…knot.KnotClient` | 7 (ASM, sponge-mixin, the loader) |
| `org.quiltmc.quilt-loader` | `…knot.KnotClient` | metadata-provided, requires intermediary |
| `net.neoforged` | `…forgewrapper.installer.Main` | 24 libs + 40 maven files |
| `net.minecraftforge` | `…forgewrapper.installer.Main` / legacy `launchwrapper.Launch` | 30 / 19 |

`instances::create` therefore registers the component
(`PackProfile::set_version`) and nothing else. `palantir_loader::plan_loader_install`
and its main-class table are deleted rather than deprecated: keeping a function
that produces a broken loader would invite the bug back. `write_patch` stays —
it is the override path, and it is what a user's own patch file should go
through.

### 16.4 The gate: CI, not a laptop

`crates/palantir-net/tests/live.rs` holds five `#[ignore]`d tests that ask the
real services, and a `live` job runs them on `master` (and by hand) with
`-- --ignored --test-threads=1`. They cover the three defects directly:

* a created instance resolves against the live metadata service: severity
  clean, `net.minecraft.client.main.Main`, an asset index with a 40-char sha1,
  the versionless LWJGL slot filled to a `3.x` and its libraries and host
  natives on the classpath, Java 21;
* a Fabric instance gets the loader's own libraries (`fabric-loader`,
  `sponge-mixin`) and the mappings component the loader only *requires*, at the
  game version;
* version lists parse for `net.minecraft`, `net.fabricmc.fabric-loader` and
  `org.lwjgl3`, and release entries carry their `sha256`;
* a fetched version file is cached at the path the **offline** store reads and
  its bytes match the digest the service published;
* Microsoft issues a device code for the client id the launcher ships — the one
  step of sign-in that needs no human and fails when the id, scope or endpoint
  is wrong.

They are deliberately about identity and shape, not version numbers, so they do
not need an edit every Minecraft release. A live test that is skipped when the
network is unavailable is a live test that reports success while proving
nothing, so a failure there is a failure.

### 16.5 Still not proven by any of this

* **A real game launch.** Resolution, the classpath and the cache are checked;
  actually starting the JVM and watching the window appear is not, and that is
  the remaining end-to-end gap. Both halves are in place for it — the install
  plan and the launch script — so a `--verbose` run of the built exe on a real
  machine is the next step, not a new feature.
* **A full install of a modded instance.** `install::plan`/`run` are unit-tested
  against fixtures; nothing downloads a real Forge maven tree. The plan is
  derived from the same resolved profile the live tests validate, so the risk is
  in the downloader rather than the plan.
* **Persisting what resolution decided.** Prism writes the filled versions and
  the added dependency components back into `mmc-pack.json`; this resolves them
  in memory on every launch. Launching is identical either way, and the instance
  page showing `Intermediary` as a component would need the write-back.
* **Old versions.** The rules are exercised against 1.21.1. Legacy profiles lean
  harder on `suggests` (LWJGL 2) and on quirks `org.lwjgl3: 3.1.2` /
  `org.lwjgl: 2.9.1` fallbacks, which are **not** implemented: no defensible
  version is invented, and a legacy pack that pins nothing gets a clear error
  instead.


## 19. The asset objects were never being downloaded, and now an install shows it

Two changes, and the second is why the first could be seen at all.

### The 404

The user's second launch of `dwa` reported, from the instance's own
`logs/launcher.log`:

```
install: installed 0 file(s) (0.0 MB), 101 already present, 5057 failure(s)
files are missing and could not be downloaded — not launching
```

5057 failures, 5057 unique URLs, one per asset object, and **0.0 MB** after
several minutes. `run_assets` derived the disk path and the URL from one helper,
`palantir_core::assets::object_relative_path`, which returns the *storage* layout
(`objects/<xx>/<hash>`) because that is what `assets/` holds on disk and what
Prism writes there. Mojang's resource CDN has no `objects/` segment: it serves
`/<xx>/<hash>`. So every request went to `/objects/<xx>/<hash>` and came back
404 — measured against the live service, 1.38 s for the 404 against 0.031 s for
the real object on a warm connection.

The index is 5057 objects totalling **458 MB**, median object 9 KB: not a
bandwidth problem, a path problem. The nine phases around it were fine — the 101
libraries and the index itself are fetched from URLs the *profile* carries, and
this was the one phase where the URL is derived rather than read.

**Why no test saw it.** Every fixture built its expected URL through the same
helper the code built the real one (`install.rs`'s `MapFetcher` keys), so all of
them agreed on a path the server has never had. That is §16's failure mode again
in a new place: the fixture supplied the bytes the code expected.

**The fix** is two functions with two names and their own tests:
`object_relative_path` for the storage layout, `object_cdn_path` for the CDN, and
`install::asset_object_url` as the one place the base URL and the path meet. The
tests that hold it are deliberately not self-consistent — `palantir-core` asserts
the CDN path as the literal Mojang serves, `install.rs` writes the finished URL
out in full, the fixture keys spell the path out rather than calling either
helper, and a new `#[ignore]`d live test in `palantir-net/tests/live.rs` fetches
one real object and checks its SHA-1 against the hash the index named. That last
one is the only test that could have caught this, which is why it exists.

### The bar

The phase was also silent by design: it printed ~50 progress *lines* per phase,
which on a six-minute download is a scrollback nobody reads and a moving number
nobody can see.

So a phase now reports two different kinds of thing through one `install::Reporter`:

* **lines** — few, meaningful, and written to the instance's `launcher.log`:
  the phase opening (`downloading 5057 asset object(s)…`) and a new closing line
  with what it moved and how long it took (`asset objects: done — 452.9 MB in
  91.3 s`).
* **levels** — an `install::Progress` per ~2% of the phase: label, done, total,
  bytes. The window draws it as a bar; the log never sees it, because a log that
grows a line per 2% of a download is a log nobody reads.

The level travels to the window as its own message (`Message::LaunchProgress`)
sent with `try_send` and **dropped when the channel is full**, unlike log lines,
which retry. A level is not an event: the next one supersedes it, and a
downloader waiting on a window that is a frame behind is the wrong trade twice
over. The bar is drawn twice from the one piece of state — in the status strip,
which every page has, and above the console on the Logs page, where the numbers
are spelled out — and the status text tracks the level so the strip cannot sit
on a phase's opening line for the minutes the phase takes.

What is *not* drawn: a bar for a phase with no work (everything already present),
and no bar at all between phases. A per-phase bar that names its phase is honest;
byte-weighting the whole install would need sizes that libraries do not publish
before they are fetched, and would either lie or sit still.

### End to end, measured (20:22)

Then it was done for real: the CI artifact of `892fc6f` (`dist/PalantirMC.exe`,
`fe49bfa1…`), pointed at a scratch data root with `PALANTIRMC_HOME`, holding a
copy of `dwa` and nothing else, driven by a session script. What the app's own
`launcher.log` wrote:

```
install: 101 file(s) to download (127.7 MB), 0 already present
downloading 101 file(s)…
files: done — 101 file(s), 131.4 MB in 28.2 s
downloading 5057 asset object(s)…
```

**The URL holds against the live service.** The asset phase opened and moved:
767 files under the root 9 s after it began, 666 of them objects, about 74
objects a second. That is the phase that used to spend six minutes collecting
5057 404s and end at `0.0 MB`.

**The bar tracks it.** Accent pixels in the window's status strip, read out of
`PrintWindow` captures, against what the phase had done:

| capture | on disk | bar, of its 200 px | phase |
|---|---|---|---|
| before Play | 7 files | nothing drawn | nothing fetching — no bar, as designed |
| t+3 s | 11 files, 1.4 MB | nothing drawn | still planning; no phase has reported |
| t+8 s | 87 files, 46 MB | 144 px = 72% | files phase, ~73 of 101 done |
| t+15 s | 105 files, 122 MB | 186 px = 93% | files phase, ~94 of 101 |
| t+25 s | 105 files, 122 MB | 186 px = 93% | one large library in flight: nothing finished, so nothing advances |
| t+37 s | 767 files, 159 MB | 24 px = 12% | asset phase, ~600 of 5057 objects |

The 12% is 12% of *5057 objects*, not of the libraries before them — the bar
resets with the phase and the status line names which phase it is, which is what
makes a per-phase bar honest and a whole-install one a guess. Nothing is drawn
before the first phase reports or between phases. The strips these rows were
measured from are cropped — one row per capture, `dl-0` before and `dl-1`…`dl-5`
during — into `.scratch/bar-strips.png`.

**What driving it took**, because it cost an hour to find: a session cannot
click the launcher until the launcher is *raised*. `SetCursorPos` + `mouse_event`
delivers the press to whatever is topmost at that point, and on this desktop
that was a full-screen window belonging to another program — so every click went
there and the launcher answered nothing, which is indistinguishable from a dead
button. `winshot.activate` (an Alt tap, then `SetForegroundWindow`) before the
click is the whole fix; the session is `.scratch/drive2.py`. The shell's own
hit-test is not implicated: `WM_NCHITTEST` answers `HTCLIENT` for a card button.

That left two things open, and both are answered now — a later run from the same
window finished the phase and the game launched out of it:

```
asset objects: done — 5057 file(s), 458.2 MB in 81.2 s
install: installed 0 file(s) (458.2 MB), 101 already present, 5057 asset object(s)
using java [java-runtime-epsilon/bin/java.exe] (openjdk version 25.0.1)
[21:26:40] [main/INFO]: Loading Minecraft 26.2 with Fabric Loader 0.19.5
```

Two numbers to read out of that. `458.2 MB in 81.2 s` is 5.6 MB/s, and §20
measures this line at 8.9 MB/s through eight connections — so a third of that
phase was spent not downloading. And `installed 0 file(s) (458.2 MB), 101
already present` is the other half of the trade, and the reason a second launch
is not a second download: the plan carried 458.2 MB of work, the filesystem said
every one of those files was already there, and the launch fetched nothing. The
458.2 MB is what the plan *would* have moved.

The run is also where the four complaints in §20 come from: it started the game
through the console build of Java, having held hundreds of megabytes to get
there.


## 20. Slower than the line, fatter than the files, and wearing a console

None of this was visible from the bar. It came out of the completed install
above — the first one this launcher ever finished — and out of measuring the
real CDN instead of trusting the fixture.

### The command prompt was one window, not two complaints

A JRE ships two launchers for the same JVM: `bin/java.exe`, a console program,
and `bin/javaw.exe`, the same thing with no console. The launcher ran
`java.exe`. A GUI process that starts a console program gets a console window
allocated for it, so what the player sees is a command prompt behind the game —
titled with the `java.exe` path and wearing Java's own icon. With that window
there, the taskbar entry belongs to *it*, not to the game, which is why it reads
as the game launching with someone else's logo and name. One cause, not two.

Both halves of the fix are wanted, because either alone leaves a path that
flashes a console. `java_runtime::launcher_binary` runs `javaw.exe` when it sits
next to the `java.exe` (the *completeness* check still looks for `java.exe`,
which is the file the manifest lists). `launch::hide_console` spawns with
`CREATE_NO_WINDOW`, which covers what the binary choice cannot: a `java.exe`
configured by hand, the two `java -version` probes, and the `cmd /C start` this
launcher uses to open a URL.

### 458 MB moved, and the launcher carried it in memory first

`BlockingHttpFetcher::fetch` read the whole body (`response.bytes()`) and then
copied it again (`.to_vec()`), and `download_bytes` asked for that *before*
writing anything. With eight of those in flight the peak was
`2 x threads x file size`, and these files are not small: the largest library
this index installs is **37.4 MB**, the top five being 37.4, 22.9, 14.5, 8.0 and
2.9 MB. Eight of the big ones at once is **598 MB**; even the asset phase alone
is 172 MB. That is the shape of the complaint — a footprint in the hundreds of
megabytes, moving with the file sizes, seen while the game was starting.

`Fetcher::fetch_to` writes into a sink instead. The default stays
fetch-then-write, so the in-memory test fetchers are unchanged, and
`BlockingHttpFetcher` overrides it to stream the response through `reqwest`'s
`copy_to`. `download_bytes` now writes into a `BufWriter` over the `.part` file
and renames it, so a bulk phase costs one `WRITE_BUFFER` (64 KB) per worker — a
megabyte across sixteen workers, whatever the files weigh.
`download_bytes_streams_the_body_instead_of_holding_it` is the guard: its
fetcher's `fetch` is a hard error, so going back to buffering fails the test
rather than the user's machine.

### 5.6 MB/s on a line that does 8.9

Against the real object CDN, one keep-alive connection per worker, five-second
windows, and then against an unrelated origin to see whose ceiling it was:

| connections | `resources.download.minecraft.net` | `speed.cloudflare.com` |
| --- | --- | --- |
| 1 | 1.55 MB/s | — |
| 8 | 8.86 MB/s | 8.79 MB/s |
| 24 | 10.25 MB/s | — |

Two origins agreeing at eight connections, and tripling to twenty-four worth
16%, says the ceiling is the **line** — about 85 Mbit/s here — and not the CDN.
So `DEFAULT_THREADS` goes 8 → 16: most of that 16%, half the sockets of 24, and
nothing to gain above it.

The rest of the gap was not the connection count but the *assignment*.
`download_many_with_progress` sliced the jobs into one contiguous chunk per
thread and let each worker drain its own, so the phase finished when its slowest
chunk did. Jobs arrive in map-iteration order and these files are wildly
unequal — median 10 KB, mean 93 KB, and the largest 1% holding 46% of the bytes
— so over 400 random orders the heaviest of eight chunks averaged **1.31x** the
average chunk (worst 1.83x). The phase therefore used ~**76%** of the bandwidth
it had; the other quarter was workers idle with unclaimed jobs behind them.
Dealt largest-first into the emptiest chunk those same objects divide into eight
chunks of 57.3 MB, a 1.000x spread.

The fix is smaller than the dealing rule: a shared `AtomicUsize`, so a worker
takes the next job the moment it is free and the queue outlives any one chunk.
`download_many_keeps_every_worker_busy` holds the concurrency with a rendezvous
fetcher that fails — after ten seconds, as an error and not a pass — unless all
four workers are in flight at once.

Together: sixteen threads at about 10 MB/s against a 458 MB index is ~46 s of
transfer instead of the 81 s measured, with a flat footprint instead of a
spiking one.

Worth knowing for a later pass, and a bigger win than everything above: this
machine's `.minecraft` already holds **4227 of those 5057 objects**, so an
install able to adopt an existing store would move **122 MB** instead of 458 MB.
That is a product decision — whose files to trust, and what to say when it uses
them — rather than a tuning one, which is why it is not in this change.


## 21. The launch gets a bar of its own, and stops hiding the window

Two things a launch still did to the person watching it. It minimized the window
the moment Play was pressed, and everything it had to say after the install —
signing in, resolving a version, unpacking a JRE, waiting for the JVM — was a
console line in a scrollback nobody was reading.

### The minimize was a default, not a decision

`minimize_on_launch` shipped `true`, so the window left at exactly the moment a
first install of 458 MB began, which is the one time it is worth watching. It
defaults to `false` now and the switch stays in Behavior for anyone who wants the
desktop back while a game loads. Nothing needed migrating: a preferences file
exists only once a setting has been written, and this one had never been.

### A launch reports as a level too

`install::Progress` gained `starting(label)`: the same shape as a phase report
with a flag saying there is no total to be a fraction of. A bar cannot answer
how far along *signing in* is, and inventing a percentage is the one place this
launcher's progress would lie, so both views answer
`Progress::is_indeterminate()` with a **sliding segment** instead of a fill. That
segment is a canvas widget in `glyphs` beside the icon drawing; `slide_offset` is
the whole rule and it *wraps* rather than clamping, so the segment comes back on
the other side instead of sticking at the end.

Three levels make the launch's bar, each sent from where the launcher actually
reaches it:

| level | sent when | what it replaces |
| --- | --- | --- |
| `preparing '<instance>'` | the worker starts | nothing — the sign-in and resolve block was console-only |
| the install phases | as before | unchanged: they report their own finer levels over the top |
| `starting '<instance>'` | the plan is ready, the JVM about to spawn | `spawning '…'` and `process started, streaming output…` |

### The bar has to end, and that is the careful part

`Message::LaunchStarted` ends it, sent when the game's *own output* says its
window is up: `Backend library:`, `OpenGL Version:`, `OpenGL Renderer:`,
`Sound engine started`, `Created: ` — graphics and sound lines that all run after
the window exists. The exact alternative is to ask Windows whether the child
process owns a visible window; that is FFI in the one path with no fixtures,
where the log states the same fact in a form a test can pin down.

The failure mode was chosen on purpose. A version that renames all five markers
leaves the bar travelling until the process exits — honest for too long — where
matching the loader's bootstrap lines would clear it while the player is still
waiting, which is a bar that lies early. It is a separate message from
`LaunchDone` because it is a third kind of fact: not a line that happened, not a
level that keeps changing, but the end of the waiting.

The tick that moves the segment is the same thread-and-channel shape as the
scroll tween's frames, and it is asked for only while a bar with no total is on
screen — twelve and a half ticks a second, none during a determinate phase and
none once the game is up. The phase lives in the app state rather than being
read from the clock, so the drawing stays a function of state and the tests can
move it.

Still unverified, and only the desktop can say: that the segment visibly
slides, and that a launch now leaves the window where it is. CI compiles and
tests this; it cannot watch either one.


## 22. An audit of "does this launcher do everything a launcher does", and the eight things it found

Reading the code rather than the feature list, because the feature list had gone
stale: §4 still claimed Microsoft sign-in was not implemented a dozen revisions
after it was. Eight real gaps came out of it. Seven are now closed; the eighth
(CurseForge file downloads) cannot be, and says so where a user can read it.

**The worst one first: installing a modpack did nothing.** `ContentType::Modpacks`
answered `"mods"` for its target folder, so `Install` on a pack downloaded the
`.mrpack` into the selected instance's mod folder and reported success. The game
then read a zip as a broken mod jar, and none of the pack — no mods, no configs,
no loader — was present. There was no test for it because the folder mapping
looked like a table lookup. Now the pack *is* the instance: `install_pack`
downloads the archive, imports it (overrides written, loader registered in
`mmc-pack.json`) and populates the files its index lists. `target_folder` returns
`Option<&str>` and answers `None` for a pack, which is what stops the old path
from being reachable again by accident — and a test asserts that `None`.

**Pack files are fetched now, in both directions.** `palantir_loader::plan_pack`
reads either index and returns the remote files (`PackFile`), honouring three
rules that matter: `env.client == "unsupported"` files are skipped (they are the
server's, and installing them is a crash), a `path` that escapes the instance is
*refused and reported* rather than rewritten, and every file is checked against
the `sha1` the pack publishes — a mismatch is deleted, because the next launch
would trust it. The loader still does no network of its own; the fetching lives
in `browse::fetch_pack_files`, which runs through the same thread pool and the
same streaming `.part`→rename path as the install phases. Dropping a `.mrpack`
on the window goes through the same function, off the UI thread, with the same
bar.

**Modrinth dependencies are followed.** `ModrinthProjectVersion` now carries
`dependencies`, and `install_with_dependencies` installs the `required` ones —
resolved with the same game/loader rules as the requested file, transitively, with
a project-id set so two mods needing Fabric API install it once and a cycle in
user-generated metadata terminates. `optional` and `embedded` are deliberately
left alone, and a dependency hosted off Modrinth is named in the status line
rather than guessed at.

**The window size setting reached nothing.** `windowParams` was written into the
launch *script* — a Prism-compat artifact this launcher never reads back — so an
instance set to 1920x1080 opened at 854x480. `--width`/`--height` are Minecraft's
own options (its parser defaults to 854x480, the same numbers), so those are what
the game is now spawned with, and only when the instance overrides its window:
`OverrideWindow` off means "let the game decide". "Maximized" has no argument in
Minecraft, so it is translated into the monitor's work area and the log says so
instead of pretending the window manager did it.

**Two correctness fixes in the install plan.** Asset objects are named after
their own digest, so the download loop now hashes each one it fetched and deletes
a mismatch; before, a corrupted object was trusted forever because the present
path checks size and not content. And native jars carry their library's
`extract.exclude` list to the extractor, so `META-INF/` entries are dropped
instead of being flattened into `natives/MANIFEST.MF` — a file that is not a
native, sitting on the JVM's library path.

**What this did not fix, on purpose:** CurseForge file downloads (no API key),
Forge/NeoForge still have no test coverage beyond the generic path, there is
still one game at a time, and there is no self-update check. Those are named in
§4 rather than left for a user to discover.

New tests, all offline: `plan_pack` for both formats including the skipped-entry
rules, the exclude list in the native extractor, dependency parsing against the
real API shape, the window-argument decision table, a pack fetch that verifies
and drops a tampered file, and the bar's precedence and hand-over in `app`.
Nothing here was watched on a desktop, because none of it is a pixel.


## 23. The reference client, driven with the cursor and measured off its own pixels

The last few passes compared this shell to the Modrinth App through one capture
of its home page. This one went through the app: launched it, pinned its client
to 1280x720 so its captures and ours compare 1:1, **hovered every rail entry to
read its tooltip**, clicked every destination, opened the create dialog and the
settings dialog and backed out with Esc, and typed into the Discover search box.
Nothing was created, installed or deleted in the user's own data; the one
lasting change is the window size it remembers. `REFERENCE.md` is the result,
and `tools/refwalk.py` + `tools/refsample.py` + `tools/refocr.ps1` are how to do
it again.

**Two of the three findings were invisible from a stylesheet.** The app paints
`#00da75`, not the green-500 (`#1bd96a`) its own token ladder names, and its
active rail plate is a 12px-radius square rather than the `rounded-full` its
markup asks for. Both are now pinned by tests and by G9/G10, and both fail on the
build that preceded this one, which is what makes a passing run mean something.

**What the walk cost, and what it could not reach.** Injected mouse and key
messages do not reach a WebView2 window — `NEXT_STEPS.md` §14 recorded the same
for our own iced shell — so every click is the real pointer with the window
raised, and a screenshot session takes over the desk for its duration. The
window also had to be *shown*: launched from a background shell this app creates
its Tauri window with `WS_VISIBLE` unset and leaves it that way, and the first
attempt measured a minimised window's restore rectangle while `PrintWindow` drew
the maximised layout. Two surfaces remain unmeasured and are named in
`REFERENCE.md`: the instance page (unreachable without creating an instance in
the user's own Modrinth data) and the Servers page (which the reference itself
walls behind "Modrinth App update required" at 0.204).

**One tab was deliberately not copied.** The reference's Discover strip has six
tabs to our five, the sixth being Servers. `project_type:server` answers 0 hits
through Modrinth's public search API, so a tab for it could only ever be empty —
and a tab that can never return anything is worse than one that is absent. It is
recorded in `REFERENCE.md` instead of drawn.

**What changed in the shell.** The accent, its two derived states, the
`--color-brand-highlight` plate, the 12px plate radius, the rail's leading
entries and their tooltips, the 8px rail inset, and the Discover strip: modpacks
first and selected by default, 36px pills with a white label in both states, and
"Resource Packs"/"Data Packs" capitalised as the reference sets them. Chrome,
page, raised surface, input and divider were already exact and did not move.

Still open, and unchanged from §12's list: the panel's scrollbar band, the
panel's sections drawn as cards where the reference divides them, its missing
left hairline, the accordion that does not open, and the hover/press arithmetic
that no still capture can measure.


## 27. The interaction, app-wide, because it is the one rule every control shares

The port had been page by page, and the four things a copy can differ in --
pixels, type, style and motion -- were being discovered one surface at a time.
This pass took the one of the four that *every* control shares and read it out of
the reference's own components instead of measuring it off a page: what a hover
does, what a press does, and what a disabled control is.

**The light theme hovered the wrong way, and that is the whole reason it was
worth doing.** `--hover-brightness` is `1.25` in `.dark-mode` and **`0.9` in
`.light-properties`**: dark mode *lights a control up* on hover and light mode
*darkens it*. The shell had a single `const HOVER_BRIGHTNESS: f32 = 1.25` with a
comment admitting the other value existed ("a fact about the reference this
palette does not yet carry over"), so on the light theme every button, tab and row
went brighter under the pointer where the reference's goes darker -- and the three
themes that share dark's factor hid the bug from every capture the gates take,
because a capture is of a control nobody is pointing at.

### What replaced it

Hover and press are now **one filter over a control's whole appearance**, which
is what a CSS filter on an element is:

* `theme::hover_brightness()` reads the factor from the theme in force -- 1.25 for
dark, OLED and retro, 0.9 for light -- so no role has to know which theme it is
drawn in;
* `theme::filtered()` multiplies the fill, the label and the ring together. The
old arithmetic moved the fill only, which is invisible on an `outlined` button:
its hover *is* the label;
* `theme::PRESS_BRIGHTNESS` (0.8, from `classes.scss`'s `.button-base`) replaces
the per-role pressed rungs. The live component's press is `active:scale-[0.97]`
and iced cannot paint a widget smaller than the box it was laid out in, so the
press is a brightness -- the same number the dark palette had already derived by
hand, which is why nothing in dark moves;
* `theme::DISABLED_OPACITY` (0.5) replaces three different fades (0.35 on the
fill, 0.4 on the label) and the invented surface behind a disabled ghost button.
`disabled:opacity-50` is one opacity on the element, and a quiet button with no
fill does not grow one by being disabled.

Three palette rungs died with it: `accent_hover`, `accent_dim` and
`danger_hover` were the rule spelled out per theme, which is exactly how light's
came out pointing the wrong way. A rung cannot follow the theme that produced it;
a filter over the theme's own colour can.

### What holds it

| Gate | What it asserts |
|---|---|
| `hover_goes_the_way_the_theme_says` | dark's hover is brighter than its base, light's is darker, and the factors are 1.25 / 0.9 |
| `hover_moves_the_label_and_the_ring_too` | an outlined control (a fill-less one) still hovers, and a filled one moves all three of its colours by the factor |
| `disabled_buttons_are_dimmed` | fill, label and ring all at 0.5, and a ghost keeps its absent fill |
| `the_reference_still_states_the_factor_this_copy_reads` | reads `vendor/modrinth-app` and asserts the numbers are still in it -- `--hover-brightness: 1.25`/`0.9`, `active:scale-[0.97]`, `disabled:opacity-50`, `duration-150`. It **returns early** rather than failing if the vendored tree is absent, because `UPSTREAM.md` promises that removing it changes no test |

The last one is the interesting one, and it is the shape the rest of the copy
wants: "this is the reference's number" stops being a claim in a comment when the
comment's number is checked against the file it came from, and the copy is
verbatim because `vendor/modrinth-app` is a pinned, hash-checked blob tree rather
than a paraphrase. It skips on a missing tree by design, so the day the reference
moves the test tells us which line to re-read rather than silently passing.

### What the runner confirmed

Commit `b08e9d2`, run
[35593826385](https://github.com/MSedgeMC/PalantirMC/actions/runs/35593826385): all
five jobs green, and the exe it produced is `60f7a6ac43c35ea8e5f5762bec3042cba15e8bd2438e4e247cd367eb87d0de91`
(4,120,864 bytes), hash-checked against the sidecar the runner wrote and staged as
`dist/PalantirMC.exe` with the raw artifact under `dist/ci-b08e9d2/msvc/`. The
Home, Discover and Screenshots page gates were then re-run against captures of
that build (`tools/appshot.py`) and all three pass, which is the static half of
the claim: nothing a gate already measures moved. The hover half cannot be
captured at all -- there is no pointer in an unattended capture -- so it is
asserted by the four unit tests above and by the vendored source they read.

### The token gate, which is what makes the rest of the copy checkable

The four values above were checked by four string comparisons against the
reference's files. That is a fact about four tokens, and the port has hundreds: a
comment saying "this is `--surface-3`" is a claim that nothing reads.

`crates/palantir-desktop/src/reference_tokens.rs` is the answer to that, and it is
the first thing in this repository that treats the vendored tree as an *oracle*
rather than as a shelf. It parses `variables.scss` and `defaults.scss` -- the
`--name: value;` blocks by selector, `var()` chains followed to the end of the
chain, `linear-gradient()` read as its stops -- and compares the result with what
the shell actually holds: the palette's three modes, the radii, the interaction
factors, the type scale.

| Test | What it covers |
|---|---|
| `every_transcribed_token_is_the_references` | 72 transcribed values and 24 declared deviations, across dark, light and OLED -- colours, gradients, and now the five radius constants against `--radius-*` |
| `every_palette_field_is_accounted_for` | the palette's field names read out of `theme.rs`, so the table cannot omit a colour |
| `the_interaction_values_are_the_references` | hover factor per mode, `disabled:opacity-50`, and the two things deliberately not drawn |
| `every_text_size_is_the_reference_or_a_measured_one` | 197 `.size()` call sites against the ladder (10/12/14/16/18/20/24/32/48) and the sizes the reference writes itself |
| `every_font_weight_is_the_references` | the five Inter faces against the reference's `--font-weight-*` and Tailwind's weights |

**The second test is the one that makes the other four mean anything.** A table
can be complete-looking while missing the value that changed; this one reads the
`pub struct Palette` block out of `theme.rs` and fails if any field is not a
token, a stop, or a deviation with its reason written down. Adding a colour to
the palette is therefore a change to two files, which is the correct amount of
friction for a value that is supposed to be a copy.

**A deviation is a first-class answer, not a hole.** The reference's stylesheet
and the reference as installed disagree; `REFERENCE.md` records four of those
from measurement, and the table holds 24 declared deviations (the measured accent
in three modes, the derived brand highlight, the panel's wash and its surfaces,
and the light theme's accent, which moves a rung for contrast). An undeclared
difference fails; a declared one prints in the report with the reason beside it.

#### Two bugs it found in itself, which is the useful part

The first run failed on six OLED tokens it had claimed to be *dark's*. The cause
was in the loader rather than the table: it merged the sheets as light -> dark ->
OLED **into dark as well**, so every dark claim was being read out of the OLED
block. The second was a wrong alias: `--color-divider` is dark's `surface-4` and
not the `surface-5` a hairline here is, and claiming it for `border_strong` was an
assumption the gate refused. A third was a genuine code bug in the comparison
itself -- `find(..).and_then(..)` returns `None` both for a stop that matches and a
stop that is missing, so every *correct* gradient stop was reported as absent.

All three are worth more than the tests they broke, because they are exactly the
failures a hand-written checker produces: taking the wrong block, assuming two
tokens are the same, and writing a comparison whose two failure paths are one.

#### The control

A gate that cannot fail is a checkpoint, so this one was made to fail on purpose:
`palette.surface` moved by one 8-bit level (`#27292e` -> `#26282d`) answers

```
2 of 60 claims disagree with the reference:

palette.surface [Dark]: ours #26282d, reference #27292e
    .../vendor/modrinth-app/assets/styles/variables.scss:237  --surface-3: #27292e

--color-raised-bg [Dark]: ours #26282d, reference #27292e
    .../vendor/modrinth-app/assets/styles/variables.scss:237  --color-raised-bg: var(--surface-3)
```

which is both halves of what the harness is for: the token, the line it lives on,
and the fact that the alias carrying the same value is checked too.

#### What the runner confirmed

Commit `b7cb7a8`, run
[35601175367](https://github.com/MSedgeMC/PalantirMC/actions/runs/35601175367):
all five jobs green, `Test workspace` including the six new tests. The page gates
were not re-run for this commit and did not need to be: the module reads the
reference's sheets and this crate's own sources, and draws nothing -- the
rendered exe is byte-identical in behaviour to the one before it, which the
release build of the run is the artifact of.

#### The skip path, and how not to check it

`UPSTREAM.md` promises that removing the vendored tree changes no test. Five tests
are a strange way to keep that promise if the way to check it is to move the tree:
that was tried, `mv vendor/modrinth-app vendor/.moved-check`, and it left the tree
moved for the length of a timed-out command -- 1857 files showing as deleted in
`git status` while the check ran. Every reader in the module therefore takes the
tree's root as an argument, and `a_tree_that_is_not_checked_out_skips_rather_than_fails`
asserts the skip against a path that is not there. The harness is now the only
thing that needs to know where the tree is.

#### What it does not check, and what its report says

The report (`-- --nocapture`) is the other half of the value: **189 tokens in the
reference's two sheets, 29 held by this shell, 160 not held** -- the ladder rungs
(`--color-red-100` …), the platform colours, the ad colours, the shadows, the
gradient fade-out. That list is what a page pulls from as it ports, and it is
printed rather than asserted because a shell that *doesn't* use a token is not a
failure. Type gets the same treatment: **197 sizes drawn, all 197 on the ladder
or among the reference's own `text-[Npx]` writes** -- the two measurements this
section used to carry (15 for the Settings headings, 28 for the device code)
retired the day the vendored components were read for the headings they actually
contain: the reference's Settings section headings are `text-lg` (18) and its
modal headings `text-xl` (20), so 15 was never a size it states anywhere, and
the device code now stands in the reference's own big-modal slot, `text-3xl`
(32). The headings moved 16 (the ladder's `--font-size-nm`) and the code to 32,
`MEASURED_SIZES` is empty, and the browser-row description that drew 15 draws
14, which is what the reference's own `project-card-summary` (`text-sm`) is.

Not checked, deliberately: the `--shadow-*` tokens (iced draws no box-shadow from
a token), `--gap-*` (the shell's spacing is inline per call site, so there is no
value to compare), the retro mode the reference offers and this shell does not,
and glyph rendering, where the reference's WebView antialiases in colour and no
number in either tree is comparable.

### What this pass did not do, and how "everything" stands

What is **not** carried over, each for a stated reason rather than an oversight:

1. **Transitions.** `ButtonFrame.vue` is `duration-150 ease-out` on six
properties; iced has no transition, so a hover lands in one frame. Drawing it
would mean the `anim.rs` deadline pattern (which exists, for switches and the
scroll glide) attached to *hover* state, i.e. a message per pointer move over
every control -- the per-move cost measured in §8, and the reason that fix was
there in the first place.
2. **The press's `scale-[0.97]`.** A brightness stands in for it.
3. **Per-component hover factors.** Omorphia's buttons share one factor, but its
cards do not: an instance card is `brightness-110`, a world card
`[--hover-brightness:1.25]`, an `InstanceItem` `1.1`. Those are cards, and iced
containers have no hover state -- each one would need a `MouseArea` and a message,
which is a change of shape rather than of value.
4. **The type scale and the surfaces that are still our own.** Nothing in this
pass moved a font size or a colour that a gate already measures.

The inventory the four dimensions have to cover, so this is not mistaken for the
whole job: **pixels** -- the shell, Home, Discover, the loading page and
Screenshots are ported and gated, the instance page and its tab strip, the
create-instance chooser's three cards, the library grid, the mods/worlds/files
screens, the project pages and the eleven Settings panes are not; **type** -- the
reference's ladder is 10/12/14/16/18/20/24/32/48 with heading and title at weight
800 and body at 500, and every size this shell draws was measured off the
reference rather than taken from the ladder, which is right where they agree and
unexplained where they do not (11, 13, 15 and 28 are all drawn here and none is a
rung); **style** -- this pass, plus the four panel gaps in §12; **motion** -- the
switch slide is in, the scroll glide is in, the splash fade is in, and the
accordion in §12.4, the page transitions, the modal in/out and the toast are not.


## 28. The whole vocabulary, copied, and what it says is still missing

§27 ended by saying the cheap way to finish the copy was to turn the vendored
tree into an oracle instead of a bookshelf, and that only four interaction tokens
had been done that way. This is the rest of it: **every token either sheet
declares is now held by the shell**, at the value the reference gives it, with the
file and the line it came from.

### The copy

`tools/gen_tokens.py` reads `variables.scss` and `defaults.scss` at the commit
`UPSTREAM.md` pins and writes `crates/palantir-desktop/src/theme_tokens.rs` -- 821
lines, **189 tokens in each of four modes** (light, dark, OLED, retro), 144 of
them colours. It builds the reference's own cascade from the `@extend` lines
rather than from a hard-coded order (`html` extends `.light-properties`;
`.oled-mode` and `.retro-mode` extend `.dark-mode`), follows `var()` chains to the
end, resolves `rem` at the reference's 16px root, and classifies each value as a
colour, a length, a number, a gradient's stops (`#rrggbbaa@position`, comma
separated) or text. It fails rather than guessing: a block it cannot find, an
`@extend` that moved, or a mode that resolves to fewer than 150 tokens is a
`SystemExit`, not a short file.

`Kind::Text` is not a residue. It is where the reference declares something this
reader cannot answer -- its `hsla()` shadows, its font stacks -- so the shell
holds the token and a claim that needs the value fails loudly instead of quietly
matching half of it.

### Why it is a copy and not a transcription

The sheets are read a **second** time, in Rust, by parsers that share no code with
the generator:

| Test | What it is for |
|---|---|
| `the_generated_vocabulary_is_the_sheets` | re-derives the cascade and compares all **756 rows** keyed by mode and token; a disagreement names the file and line the reference states it on |
| `the_generated_vocabulary_covers_every_declared_token` | collects the declared set by a *line scan* -- a third opinion, so two parsers cannot collide on one blind spot -- and fails on a token the copy is missing or one no sheet declares |
| `the_generated_vocabulary_holds_the_tokens_the_palette_paints` | the values the palette gate checks are *in* the copy, so the palette cannot stop being transcribed from the reference while both gates stay green |

All three skip when the vendored tree is absent, so `UPSTREAM.md`'s "removing it
changes no test" stays true -- and is checked, rather than being tested by moving
1,857 files, which is a thing that was tried once and cost an evening.

The control: changing one byte of one row (`#1bd96a40` -> `#1bd96a41`) fails it
with six rows, each printed with the reference's own line, plus every alias
carrying the same value. What the control caught *before* it was trusted was the
harness's own four bugs, which is the argument for having a second reader at all:
a `u8` formatted with `{}` instead of `{:02x}`, rendering every `#rrggbb` as
decimal digits concatenated (`--surface-1` read back `#252523` where the copy says
`#191917`); chain lookups keyed with the `--` the walk strips, so every `var()`
hop reported a missing token instead of the value at the end of the chain; a
gradient with an unencodable stop classified `Gradient` where the generator calls
it `Text`; and the one real disagreement between the languages --
`rgba(27, 217, 106, 0.7)` is 178.5, Python's `round` is half-to-even and Rust's is
half-away, so the two chose 178 and 179 on the same token. Both now add 0.5 and
truncate.

### The half that is not a cascade

The first cut of this copied the two global sheets, which is the design system.
It was not everything: the reference declares **139 more tokens in 21 files** --
components, pages, `classes.scss`, `global.scss`, `tailwind-utilities.css` -- and
those belong to a *selector*, not to a mode. `--top-bar-height: 3rem` and
`--left-bar-width: 4rem` on `.app-contents` are the shell's own chrome numbers;
the `--os-*` scrollbar knobs a combobox configures are a control's geometry;
`--ease-out-expo: cubic-bezier(0.16, 1, 0.3, 1)` on `:root` is the motion curve
§27 said was missing; and the per-card hover factors (`.instance-item`,
`[--hover-brightness: 1.1]`) are precisely the ones §27 listed as "not carried
over, each for a stated reason". They are all in `SCOPED` now, each row carrying
the rule that sets it.

They are checked the way the mode tables are: the tree is walked a second time in
Rust, each file parsed by a reader that shares no code with the generator, the two
readings compared keyed by file and line; and a third test counts declarations per
file by a line scan, so a walker that skipped a file or invented a row fails
rather than shipping a copy with a hole in it.

Three things that check found, all of them worth the second reader:

1. **An invented token.** `ScrollablePanel.vue` writes
   `transition: opacity 0.1s ease, --_top-fade-height 0.05s linear,` -- and the
   continuation line starts with `--`. The walker read it as a declaration and the
   copy held a token called `--_top-fade-height 0.05s linear,`. The *comparison*
   could not see it, because both readers agreed with each other; the count could,
   and the rule is now stated in both: a declaration is a line that starts with
   `--` **and holds a colon**.
2. **A spelling.** The generator wrote the token with its `--` and the Rust reader
   stripped it, so every one of the 139 rows differed by two characters -- which
   reported as *all* rows wrong, and is what a spelling difference looks like from
   the inside.
3. **Two files that had been invisible**: `classes.scss` (42 declarations, the
   per-component values behind the buttons) and `Avatar.vue`, `ScrollablePanel.vue`,
   `NotificationToast.vue`, `ProjectCard.vue` -- every one of them declaring
   underscore-prefixed names that the first scan's pattern could not match. That
   scan was mine and it was wrong; the walker was right.

### The number the coverage report was understating

The older gate's `-- --nocapture` report said **148 tokens, 26 held, 122 not
held**, and the first number was wrong: it counted the three maps it had built
(`.light-properties`, `.dark-mode`, `.oled-mode`) while the reference's light mode
is also the `html` block -- the gaps, the radii, the ad colours, the ring -- and
`body`'s type ladder. It now counts every name either sheet declares, which is
**189, 29 held, 160 not held**, and it agrees with the vocabulary walk's count
because two readers built it. The 41 tokens the report used to omit are exactly
the kind of thing a porting pass plans against, so the understatement was the
worst possible kind of error in a list whose only job is to be complete.

Merging those blocks into the *maps* was the obvious fix and is the wrong one: a
line number cannot say which sheet it came from, so every failure message would
have named a file the token is not in. Names are merged; lines are not.

### Running it

`python tools/gen_tokens.py` regenerates; nothing in `theme_tokens.rs` is edited
by hand, and the gate's failure message *is* the instruction to re-run it. The
copy is compiled for tests only (`#[cfg(test)]`), because the shell paints from
`theme.rs`; when a page starts consuming a token at runtime, the token moves into
`theme.rs` and this row stays as the receipt.

The copy is already doing the second job for two values: the chrome gate
(`the_chrome_is_the_scoped_copy`) reads `--top-bar-height`, `--left-bar-width`
and `--right-bar-width` out of `SCOPED` and compares them with
`TITLE_BAR_HEIGHT`/`RAIL_WIDTH`/`SIDEBAR_WIDTH`, so the constants that were
taken off a capture are held to the tokens the reference declares; and the
transcription gate reads the radii out of `LIGHT`'s table, because a radius is
modeless and the `html` block is where the reference declares it. Both are the
shape "held" was always meant to take: the shell paints from `theme.rs`/`app.rs`
and the copy is what proves the number.

### What the runner confirmed

Commits `78b69ab` and `3b39f76`, run
[35623738312](https://github.com/MSedgeMC/PalantirMC/actions/runs/35623738312):
all five jobs green, `Test workspace` and `Lint` included, and both exe targets
built. Nothing in this pass draws, so no page gate was re-run: the module reads
the reference's sheets and this crate's own sources, and the exe the run built
behaves as the one before it did.

The other half of the directive is where the work is *checked*: a full
`cargo test --workspace` does not finish inside a working session on this machine
-- it has been killed at the ten-minute mark more than once -- so the loop is the
targeted test locally, then push and `gh run watch` the run that does all of it.
`AGENTS.md` §3 now says so instead of implying that a local workspace run is the
gate.

### What is still not copied, and why each one is a decision

With both halves in, every `--token` the reference declares -- 756 mode rows and
139 scoped rows -- is held by the shell at the value the reference gives it. What
is deliberately *not* copied, unchanged from §27 and worth keeping in one place:

* **The wordmark and the brand art** (the logo's PNGs, the cube in the splash):
  Modrinth's trademarks, not GPL code, and the shell draws its own mark.
* **The reference's own copy**: strings and translations are content, not design.
* **The Tauri shell**: window creation, the tray, updater and OS integration are
  the other framework's shape; this launcher is iced.
* **The Servers tab**: the page is in the tree and cannot return results here.
* **What a structural change would cost**: per-card hover (`brightness-110`) needs
  an iced hover state; `duration-150` transitions need the `anim.rs` pattern on
  hover. §8 measured a message per pointer *move* over every control and rejected
  it, correctly — but a transition does not need one: it needs a message per
  *crossing*, two per visit, which is what §29 builds. The per-card factor itself
  is still not applied (the cards paint the two global `--hover-brightness`
  values), and is recorded in §27.


## 29. The tween: the interaction rule read from the other end

§27 applied the reference's interaction rule — the factors, the direction the
light theme goes, the `opacity-50` — and §28 copied the tokens those are stated
in. What every control still did was *land* on the rule in one frame. The
reference draws all of it as `transition-[background-color,color,box-shadow,
filter,opacity,transform] duration-150 ease-out` (`ButtonFrame.vue`'s base
classes), and this pass is that transition.

### The claim this corrects

§8 measured the cost of driving a hover from messages and rejected it, and §28
quoted that rejection as a decision about `duration-150` transitions: "a message
per pointer move over every control". Half of that is right and half of it is the
wrong question. A pointer *move* arrives hundreds of times a second, so a message
per move is a view rebuild per move — that measurement stands. A pointer
*crossing* arrives twice per visit: once on the way in, once on the way out. The
transition needs the crossing, and the crossing is what this pass added.

### Where a tween can start

The reason this took reading iced's runtime rather than its widgets:
**`iced_winit-0.12`'s `application::update` is the only place a program's
subscriptions are re-tracked, and it runs after a message batch and before the
view.** The frame subscription that advances a tween is gated on
`Interactions::animating()`, so a tween started *by the view* — which is the only
place iced volunteers the pointer's position, as `button::Status::Hovered` — is
started one beat after the subscription that would have carried it. The hover
would paint its first frame and then sit there: no frames requested, no message,
no re-evaluation. A tween started in the view is a tween that never moves.

`MouseArea` publishes enter/leave, and cannot be used around a `button`: it never
hands events to its content, so the button inside goes inert the moment it is
wrapped. Hence `hover::Report`: a wrapper of our own, modelled on `scroll.rs`'s
`WheelGuard` (the delegate-first shape this tree already had), which hands every
event to the control first and returns the control's own status untouched — and
then checks one rectangle, once per crossing, and publishes the change. It also
covers two cases a move-only report misses: a cursor that has *left the window*
(the cursor is unavailable, so a lit control goes out) and a control that appears
or moves under a stationary pointer.

### What is tweened now

* **Every button in the shell — 71 of them.** All of them are built through one
  function, `app::hover_button(key, style, button(...))`, which is where the key,
  the role's style and the pointer report come from the same literal. The call
  sites were rewritten by a script (`65` in `app.rs`, `6` in `settings.rs`) and
  G56 is the check that no new button skips it.
* **The modal's arrival, both ways.** 200ms, which is what `NewModal.vue`
  actually says (`transition: all 0.2s ease-out` on the overlay; `scale: 0.97`
  with `opacity: 0` to `scale: 1` on the dialog body) — the earlier 150 was a
  `duration-150` read off the wrong component. The dialog's scale is drawn as a
  brightness ramp, because iced lays a widget out and then paints it.
* **The switch's knob growth.** `Toggle.vue`'s `group-hover:w-[18px]
  group-hover:h-[18px] group-hover:m-[-1px]` and `group-active:w-[14px]
  group-active:h-[14px] group-active:m-[1px]`: the knob's 16px resting size now
  swells to 18 and shrinks to 14 across the tween instead of appearing there.
* **The structural half of a hover, not only the filter.** A transition in the
  reference moves a control's colours — a fill appearing under a ghost button, a
  rail label going from `text-dim` to `text`, a card picking the raised surface.
  `theme::blend` interpolates the role's two ends on the same clock fraction the
  filter rides, so both halves of the transition arrive together.

### What is still not drawn

* **The press's `scale-[0.97]`.** iced cannot paint a widget 3% smaller inside
  the box it was laid out in; the press stays `classes.scss`'s
  `brightness(0.8)`, unchanged by this pass.
* **The per-card hover factors** (`--hover-brightness: 1.1` on an instance card,
  `brightness-110` on a checklist row): still held in the vocabulary (G51) and
  still not applied — the cards paint the two global factors.
* **The maximize control's hover.** Windows answers that button as non-client, so
  no widget ever sees the pointer; its hover comes from the window procedure and
  its structure changes, but there is nothing to tween against.

### A test that was racing, found while this ran

`theme::tests::button_roles_paint_distinct_primary_and_danger` and
`containers_and_fields_are_themed` compare values read from the theme in force
against the palette read moments later. `choosing_a_theme_changes_what_the_styles
_paint` sets and restores the process-wide theme while holding `THEME_TEST_LOCK`,
but eleven reading tests never took it — so a reader could observe the light or
OLED theme it sets on the way through, and fail on the schedule rather than on
the code. With more tests running beside them, two of them did. Every test that
reads the theme now takes the lock, and the helper's doc says why.

### What the runner confirmed

Local, before this pass was pushed: 408 tests green (the eleven locking tests
are among them), clippy `-D clippy::correctness` clean, and `tools/gen_tokens.py`
still rewrites `theme_tokens.rs` byte-identical. The run that carries this to
`master` is the gate — `package` depends on `test`, so a red there is the record.


## 30. The dead code the compiler was already listing, and what came out

This pass began as a tidiness request and ended by taking the compiler's own list
rather than guessing at one. `cargo check --workspace --all-targets` had been
emitting **eight `dead_code` warnings** in `palantir-desktop`, and the tree
carried six `#[allow(dead_code)]`s — one in the CLI, three in the desktop
crate's `launch.rs`/`accounts.rs`, one each in `reference_tokens.rs` and
`reference_vocabulary.rs`. A warning nobody reads is how a tree grows a second,
worse copy of itself; all of it is gone, and the workspace now typechecks with no
warnings in any target.

The distinction that decided every case is **who calls it**. An item whose only
caller is a test is not dead — it is a test-only item, and the honest fix is
`#[cfg(test)]` (the attribute the compiler's own help suggests) rather than
deleting the fixture or shipping the helper in the binary.

| Item | Only caller | What was done |
|---|---|---|
| `java_major_for` (CLI) | none | deleted, with its `#[allow(dead_code)]`, its comment ("kept here so the CLI surfaces them in future versions") and the `java::JavaVersion` import it was the only user of |
| `MicrosoftState::has_code` | none | deleted |
| `theme::nav_active`, `theme::nav_idle` | none | deleted — superseded by `theme::nav_item(active)`, which is what `settings.rs`'s section list actually calls |
| `theme::tab_button(active)` | `theme.rs`'s style test | deleted; the test now passes `tab_button_at(active, false, false)`, which is what every tab inside a strip passes |
| `AccountsStore::account`, `AccountsStore::load` | `accounts.rs` tests | `#[cfg(test)]` |
| `PalantirApp::shot_path` | `app.rs` test | `#[cfg(test)]` |
| `install::Reporter::lines_only` | `install.rs`, `java_runtime.rs` tests | `#[cfg(test)]` |
| `theme::heading()` | the type-scale gate | `#[cfg(test)]`, and its doc corrected — see below |
| `launch::offline_backend` | the `Sandbox` shell | the `#[allow(dead_code)]` was **stale** — the sandbox shell does reach it — so the attribute went and the function stayed |

Two findings are worth more than the tidying they came with:

1. **`theme::heading()` was a promise written as a description.** Its doc said
   pages draw titles in it; nothing does. The type-scale gate asserts this shell
   holds `--font-weight-heading` (800, the reference's `--font-weight-extrabold`)
   and the assertion is kept, so the helper is now `#[cfg(test)]` with a doc
   saying exactly that: the weight is declared for the gap §28 already names. It
   retires the day the page titles — today `theme::semibold()` (600), every one
   of them — move onto the weight the reference draws them in.
2. **`modal`'s doc comment had been left on `nav_active`.** The paragraph
   explaining why the dialog is a hairline border and not a blurred shadow (the
   tiny-skia cost, the near-black composite) sat above a function about the
   Settings section list. Deleting the pair put it back on the function it
   describes.

Also removed: `serde` from `palantir-loader`'s dependencies. No file in that
crate names it — `serde_json`, which does the real work there, pulls serde
itself — and the lock file lost the one edge with it. `tools/unused_deps.py` is
that scan, kept in the tree so the next refactor can re-run it: the package
name, its hyphen-stripped form and the same with a trailing `-rs` removed,
against the crate's own `src/`, `tests/`, `benches/`, `examples/` and
`build.rs`. It decides nothing — a package's lib name is not always its package
name (`md-5` is used as `md5::`), so a hit is a question to answer — and it is
not wired into CI for that reason: `tools/**` does not start a run at all.

**Not removed, deliberately: the `Sandbox` shell.** `main.rs`'s `State` and
`PalantirApp::sandbox_drain_launch` are unreachable from `main()`, which runs
`App` and the subscriptions. They are kept because §7's original API is a working
synchronous entry point that the tests exercise, and deleting it would take the
honest dry-run path with it. It is *unreachable*, not forgotten; if the next pass
wants it gone, it is one entry point, one drain function and `offline_backend`,
and the tests that call `<State as Sandbox>::update` are the ones to move.

### The naming audit, because the request was "rename everything still saying Prism"

Nothing in the tree names *this* launcher "Prism" any more — §15 is that pass,
and it landed: crates, types, the public client id, the test-data directories,
`windowTitle` and the `launcherBrand` string are all PalantirMC. What is left is
a short list, and every entry is a fact about the world rather than branding, so
renaming one breaks something real:

* `LEGACY_DATA_DIR_NAME = "PrismLauncher"` and `GLOBAL_CONFIG_FILE =
  "prismlauncher.cfg"` (`palantir-core::paths`) — the folder and file this
  launcher reads to find an install that already exists. `§15` records why the
  data root itself did not move: it needs a migration story first.
* `DEFAULT_META_BASE_URL = "https://meta.prismlauncher.org/v1"` — the metadata
  service the catalog, libraries and assets come from. It is their endpoint.
* `"Prism Launcher"` as the **origin** of an imported copy (`instances.rs`,
  `app.rs`'s import dialog) — the label says where the instance came from, which
  is the one thing it must not be vague about.
* The instance icons and the About page's line about them: the art is carved from
  `prismlauncher.exe` (GPL-3.0-only, Prism Launcher contributors) and attribution
  is a condition of shipping it, not a brand.
* Doc comments and compatibility test names (`instance_cfg_reproduces_prism_bytes
  _from_the_same_settings` and friends): they record the launcher whose format
  is being reproduced, which is what makes the claim checkable.

The name that *is* ours now reaches the places that credit someone: `brand::STUDIO`
("Palantir Studios") is drawn in the About hero, the README names the studio and
its copyright, and `[workspace.package] authors` carries it into the crates'
metadata.

### What the runner confirmed

Local, before this pass was pushed: 411 tests green in `palantir-desktop` (the
crate's suite is now warning-free in every target), `cli`/`core`/`gui`/`loader`
green, clippy `--workspace --all-targets -D clippy::correctness` clean, and
`tools/gen_tokens.py` still rewrites `theme_tokens.rs` byte for byte.

[35891316483](https://github.com/MSedgeMC/PalantirMC/actions/runs/35891316483)
then carried it to `master` green in all five jobs: test workspace, lint, live
services, and both Windows exes.

## 31. The two recordings, and the scroll that teleported

The report was two screen recordings of the same window -- this launcher at 20:44
(41.0 s, 2458 frames) and the reference client at 20:41 (86.8 s, 5202 frames),
both 1280x720 at ~60 fps -- with five complaints attached: laggy scrolling, no
animations, wrong spacing, no tabs, "totally different". This is what the frames
were asked, what they answered, and what came out of it. The first complaint is
fixed; the other four are inventoried at the end of this section rather than
answered, because each one is a slice and this was one.

### The method, because "it feels laggy" is not a measurement

1 fps stills from both (86 and 41), then three seconds at full frame rate over
the part of each recording where the wheel is turned -- the reference's at 44 s,
this launcher's at 19 s. Each burst frame became a 320x180 gray PGM, and
`tools/scroll_lag.py` prints the per-transition vertical displacement from a
row-mean profile and a +-40 px cross-correlation, so "gliding" and "teleporting"
stop being adjectives:

```
$ python tools/scroll_lag.py <official burst> <palantir burst>
== official (t = 44 s) ==
moving frames: 25 (14%)  mean |d| while moving: 8.36px  max |d|: 33px
gentle steps (<=6px): 13  large jumps (>=15px): 4
motion runs (consecutive moving frames): [2, 1, 4, 2, 1, 4, 6, 3, 1, 1]
== palantir (t = 19 s) ==
moving frames: 14 (8%)  mean |d| while moving: 8.50px  max |d|: 25px
gentle steps (<=6px): 7  large jumps (>=15px): 2
motion runs (consecutive moving frames): [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1]
```

Same number of transitions, same mean distance, opposite shapes. The reference
moves for two to six frames at a time and the offsets between the frames of a run
are intermediate (`-10, -6, 0, -5, -2, -2, -3, -8, -4, -10`): one flick is one
eased movement. This launcher moves once and then not at all, fourteen times, at
whole offsets (23, 25, 11, 9, 14 px): one wheel event, one teleport, ten to twenty
still frames, and then the next.

### What was wrong, and why the fix is a deletion

The probe that decided whether to animate measured the interval between its own
`tick` calls and demoted the machine when two in a row were 24 ms or slower. Both
rates that matter are slower than that: the reference's display paints a frame in
~33 ms, and this box's software rasteriser was profiled at 69 ms. Once
`frame_cost` was set, `glide_duration` returned `Duration::ZERO`, `begin` moved
the offset inside the wheel's own frame, `animating()` went false -- so no further
frame ran, so nothing was ever re-measured, and the demotion held for the rest of
the session. The gap between two gestures was read as a frame as well: the frame
timer exists only while something moves, so a gesture's first tick was measured
against the previous gesture's last tick, i.e. against however long the reader
spent reading.

Both defects live in the probe, so the probe is gone: `SMOOTH_FRAME`,
`SLOW_FRAMES_TO_DEMOTE`, `frame_cost`, `slow_frames`, `observe_cost` and
`glide_duration` were removed, `begin` always starts a `DURATION` glide, and
`tick` measures nothing. The deadline was always enough -- a machine that draws
two frames in 160 ms gets two steps, not an animation that runs longer -- so what
the demotion bought was nothing and what it cost was every glide after the first.
The four tests that asserted the demotion became four that assert what it
prevented: a gap between gestures is not a frame, a 69 ms machine glides and
lands on the deadline, a late frame ends the gesture instead of extending it, and
a page opened later still glides. G140 is the ledger entry, and the recording's
own numbers are above.

### Still different, off the same two recordings

Every item below is visible in a matched pair of stills; the two pairs read
closely were the Discover page (18 s of ours against 41 s of the reference) and
the reference's Home at 13 s against our Library at 11 s. None of it is fixed
yet, and each is a slice rather than a line:

* **Home is a library grid in the reference and a list here.** The reference's
  Home has a collapsed *Jump in* section (the latest instance as a card, with an
  *Installing...* pill and a kebab menu) above a *Library* heading, a toolbar of
  search + *New group* + *New instance*, a *second* toolbar row (*Last played*,
  *Custom group*, a filter icon, *Add filter*), and instance *tiles* with icon
  art and a selection circle on hover. This launcher draws one flat list row per
  instance with a play button and a *View instance* button.
* **The right panel carries different sections.** On Home the reference shows
  *Getting started*, *Playing as* (the account row with a chevron), a News feed
  and a Modrinth Hosting promo card; on Discover it shows *Getting started*,
  *Hide already installed* as a switch, a *Category* list with icons, and the
  same promo. This launcher shows *Getting started* and News on both.
* **Discover's sort row has two controls the reference does not draw**: a
  *Filter results...* button and a "*Modpacks* • relevance" caption. The
  reference's filtering is the panel's *Category* list.
* **Loading states.** The reference draws skeleton result cards with a spinner
  while a search is in flight; this launcher draws whatever was there before.
* **Card art.** Our Discover thumbnails are empty rounded squares in that frame
  where the reference's carry project art.
* **The titlebar has one control too many**: our green launch arrow beside *No
  instances running*, where the reference has the pill alone.
* **Spacing and sizes are eyeballed, not measured.** Section padding, the
  *Library* heading's size and the toolbar heights all read close but not equal.
  §28's 189 transcribed tokens are the ruler for that pass, not a screenshot.
* **Animations**: the reference also animates its skeletons, the *Jump in*
  chevron, the card hover circle and the *Installing...* spinner. This launcher
  has hover tweens (§29) and, as of this slice, the scroll glide -- and none of
  those four.

What this section does not claim: no capture of this launcher's window was taken
(`PrintWindow` returns a surface without the page in it -- G135), so every item
above is read from the user's own recordings and the vendored reference, and the
spacing item is the one that cannot be settled by eye at all. A fresh pair of
recordings after this change is what shows the glide on screen.

### Where the reference keeps each of those

Every item above is in the vendored pin (`8966b5e2`, per `UPSTREAM.md`), so none
of it needs a newer copy of the reference -- it needs the port. The files, so the
next slice can start from one:

* **Home as a library grid**: `app-frontend/src/pages/Index.vue`, which is
  `ui/WelcomeScreen.vue` until an instance exists and `ui/library/index.vue`
  afterwards. The heading, the toolbar and the tiles are all in that directory:
  `library/library-toolbar/index.vue` for the row, `sort-menu.vue` for *Last
  played* and *Custom group*, `filter-menu.vue` for the filter icon and *Add
  filter*, `library/instance-group/` for a group's own row, and
  `library/LibrarySelectionActionBar.vue` for what a selection does.
* **The *Jump in* strip**: `ui/world/RecentWorldsList.vue`.
* ***Getting started***: `ui/onboarding-checklist/index.vue`.
* ***Playing as*** and the *Upgrade to Modrinth+* banner: `App.vue`'s right
  panel, which is also where the promo card's host is named.
* ***Hide already installed*** and the *Category* list: `pages/Browse.vue`.
* **The loading skeleton**: `ui/library/instance-group/` and the Browse results
  list, both of which draw their placeholders from the same card shape the real
  row uses.

## 32. The move to a machine that can read its own screen

The work in this tree has been done on a Windows box that cannot photograph its own
window: every capture tool in `tools/` reaches `user32` through `ctypes`, and
`PrintWindow` on this shell comes back without the page in it -- G135's own note.
Three gates record what that costs, and each ends with the same sentence about a
machine whose screen can be read settling it in one command: G135 (the shaped text),
G138 (the layout nodes, never read back) and G141 (the dialog after the reservation,
never photographed). The report then found three defects that only exist in a
running window -- a clipped dialog, a panic on a click sequence, a list rebuilt every
frame -- and none of them was visible in the source that was being read instead.

So the tree moves to an Ubuntu 24 VPS, for two reasons: a screen that can be read,
and a machine that can run the reference client beside ours. `TRANSFER.md` is the
runbook; `tools/vps_setup.sh` is the bootstrap; `tools/xshot.py` is the X11 capture
and input path, the counterpart of `winshot.py` (same flags, same `--script` format);
`tools/pack_transfer.py` packs the recordings and the derived frames, which are the
only part of this work that git does not carry.

What was checked before trusting the move, rather than after: the tree is portable by
design (`native.rs`: "everything here is a no-op off Windows", with
`#[cfg(not(windows))]` stubs, and one target-specific dependency in the whole
workspace); the fonts are `include_bytes!`d from `assets/fonts/`, so text cannot be
substituted by the host; `gpu.rs` already falls back to iced's tiny-skia rasteriser,
which is the path this box has been measured on; and `palantir-core::paths` has a
tested `System::Linux` data root. The reference is pinned to **v0.21.6** (2026-09-27)
on purpose: that is the generation four days before the recordings and the same
generation `UPSTREAM.md` pins the source at.

What the move does *not* change: the two recordings stay the timing authority. The
reference under Xvfb renders through software WebKitGTK, so its scroll timing there
says nothing about the recording's, and a VPS has no GPU, so this launcher's absolute
frame costs become that machine's. The Ubuntu box settles layout, spacing, colour,
pixels -- and crashes, because a panic there lands in a file.

Two things are honest limits rather than plans: `tools/xshot.py` has never been run
(it was written where it cannot execute, and the first session there should treat it
as a draft to fix and commit), and the four first jobs are named in `TRANSFER.md`
rather than done -- capture the four surfaces, the crash, the windowed lists, and the
parity inventory of section 31.

## 33. Both clients on one box, read pixel by pixel

`tools/xshot.py` works as written (it was run first for the reference, then for this
launcher, and nothing in it needed changing). The reference runs on `:99` and this
launcher on `:98`, both at 1280x720 on a 1920x1080 Xvfb, so neither window can occlude
the other and a capture of either is a capture of a window nobody has touched. What
follows is what the two screens said when read side by side, not what the source said.

### The installed surface, measured

| what | this launcher | reference |
| --- | --- | --- |
| rail's chrome | `(39, 41, 46)` at x=2, every row | identical |
| page background | `(22, 24, 28)` | `(21, 23, 27)` |
| head rule at y=48 | `(66, 68, 74)` from x=65 right | identical |
| rail button centres | 72, 124, 176, 228, 280, Create ~356, Settings 636, Profile 688 | identical |
| scrim over chrome, y=200 | `(30, 40, 41)` | `(28, 37, 39)` |
| scrim over the page, y=200 | `(24, 35, 35)` | `(22, 31, 32)` |
| scrim's solved alpha | 0.647 at y=200, 0.706 at 350, 0.836 at 600 | 0.647, 0.706, 0.836 |
| settings dialog | 509 wide, no tabs | 928 x 588, centred, 8 tabs |
| welcome call to action | 289 x 41, full brand green | 214 x 40, brand at half opacity |

### The bed was right and the layer under it was missing

The modal's scrim is the reference's own two-stop ramp, and a first capture of it read
as a flat opaque fill: solved from two different backdrops, the dialog's bed gave an
alpha of exactly `1.0` at every row. The stops were not the reason. `render` returned
the modal layer *before* it built the window, so the ramp was compositing over the
window's own clear colour -- the same colour at every row, which is exactly what a
solve from two backdrops reports as an opaque layer. The comment that had been there
said a modal replaces the window "and that is a limitation rather than a choice", and
the limitation was in the code, not the toolkit: iced 0.12 has no stack widget, but a
column of two `Fill` children whose spacing is minus the window's height lays the
second child back at the first one's own top. With the window under it, the scrim now
solves to the reference's alpha at every sampled row and composes within three levels
of the reference's own frame over both backdrops.

### The switch that was tested and not kept

`iced`'s `web-colors` feature packs colours as sRGB and renders to a non-sRGB surface,
which is what a browser does; this app's reference is a browser, so it was worth a
measurement rather than an opinion. Built both ways at the same commit and read at the
same 24 rows over the rail and the page: without it the worst channel difference to
the reference was 5, with it 4, with the same bias in both. It is not kept -- a global
change to how every colour in the app is packed, for a difference inside the sampling
noise of an eight-bit composite, is not a trade this tree should make.

### What the two screens still disagree about

The settings dialog *was* the largest of them and is now ported, measured against the
reference's own capture: `AppSettingsModal.vue`'s `min(928px, 95vw - 10rem)` modal is
928 x 591 at this window (its surface y 66..654, x 177..1102), a 288-wide tab column
behind a 1-pixel `--surface-5` divider at x=488, an 84-pixel header -- `p-6` around its
own row, which is the close `IconButton`'s 36-pixel `h-9` rather than the 32-pixel line
of the title inside it -- a 1-pixel rule at y=150, and a content floor of
`min(65vh, 600px)` (468 here). Every row of both panes lands on the reference's own,
including the theme cards' column transitions, row for row, and the *sync theme across
devices* row with its disabled switch. This port's box is one row shorter -- 590 -- on
purpose: the reference's 591 is a CSS half-pixel when centered in 720, which a browser
snaps to the same top border row (65) and a rasteriser draws as a soft edge; the one
row comes out of the body's bottom padding, and only the dialog's last two rows differ.

The button inventory is the next piece of it. The reference's `ButtonFrame.vue` has
five sizes -- `xs` 28 through `xl` 48, each with its own radius, padding, gap, icon and
label, and `xl` alone in `font-extrabold` -- and the vendored tree's own `Button` and
`IconButton` tags number 612, of which 113 state a size (38 `xl`, 36 `lg`, 22 `sm`,
11 `xs`, 6 `md`); the rest take the frame's own default, `md`. The kit now draws all
five rows (`ui::Size`, `ui::button_sized` and the three builders beside it) and the
table is gated against the component
(`reference_tokens::every_button_size_is_the_frames_own`), but the legacy 40-pixel
14-pixel-label row is still what most call sites draw: each surface moves over as it is
measured. Three things `ButtonFrame.vue` paints that this does not, all recorded rather
than approximated: the `colored` type's outer 1-pixel `color-mix(... 30%, transparent)`
ring, its four soft drop shadows (this backend's blurred-rectangle cost is the note on
`theme::modal`), and the `::before` top-edge highlight over a colored fill. The `base`
and `colored-text` type's `inset 0 0 0 1px var(--surface-5)` *is* drawn -- as the
1-pixel border it is.

### The language pane, which was a grid of chips

The Language tab is the settings dialog's second ported pane, and its first one was
drawn from the wrong component. It was a grid of language chips, wrapped into
rows by `wrap_labels`, because the pane that was read (`LanguageSettings.vue`, a
two-line wrapper) says nothing about how the list is drawn. The list is
`language-settings-selector.vue`, and what it draws is the reference's own page: a
`text-xl` heading, the fallback warning as an `Admonition` with nothing but a body
in it, the description under it with a link inside it, a search field, and then one
`CheckCircleButton` row per language -- full width, 40 pixels, a 1-pixel border
that is `--color-brand` and a `--color-brand-highlight` fill when that language is
the one in force, the name at 16 pixels in the row's `font-semibold`, its own name
in itself beside it at 14, and a coverage percentage at 14, all in front of a
24-pixel check circle.

The reference's own capture measures it: the warning's box is 114 rows (16 of
padding, four 20-pixel `leading-tight` lines, 16 more, two border pixels) over a
`--color-orange-bg` fill on a `--color-orange` border; the search field is 36
pixels -- `InputFrame`'s own `standard` row, `h-9 rounded-xl px-3` over a
`bg-surface-4` fill and a `border-surface-5` hairline; the category heading is
`pt-3 pb-1` over its 16-pixel `font-semibold` name; and the first row's plate
lands on rows 467..506 with the next two at 511..551 and 555..595, which is the
40-pixel row and `gap-1` exactly. Our chips were a 40-pixel row with a 14-pixel
label and no circle, in a grid the reference does not draw at all.

The 114 is `p-4` *inside* a `border border-solid`, which is seventeen rows before
the text on every side rather than sixteen: a browser puts the border outside the
padding, and iced paints a container's border inside its bounds without spending
a row on it, so the seventeen is the padding here. Two rows is the whole distance
between the warning landing on 211..322 and the field landing on 393..428, and
every row under it.

The list is the reference's order and not the offered list's. The selector sorts
what it builds -- `result.sort((a, b) => (b.coverage?.percentage ?? -1) -
(a.coverage?.percentage ?? -1))` -- so the pane opens on the language the
interface ships in, which carries every key and is therefore the first row, and
the capture finds its plate, its check circle and the word *English* in the first
row rather than somewhere below the fold. `crate::locale::offered_by_coverage` is
that comparison, stable on ties the way `Array.prototype.sort` is, and
`OFFERED` itself is untouched: it is still `LOCALES`' own order, which is what the
rows fall back to.

Three things are drawn differently and say so here rather than in the drawing. The
rows search on a Fuse index at a 0.4 threshold over a display name, a translated
name and the locale's own search terms, which is fuzzy matching; this is
case-insensitive containment over the same three strings, because fuzzy matching is
a scoring algorithm and a list of 32 does not need one. The Crowdin link inside the
description is drawn as the words it names rather than as a link, the markup
stripped -- the way the checklist's own link is -- because the reference's press
there opens a translator in a browser. And the 24x16 flag in front of each name
comes from `flagcdn.com`, which this launcher's settings pane does not fetch.
That is 33 pixels and every row shows it: the reference's names start at x=555
inside a row that begins at 513, and this one's start at 522 -- the row's own
eight of `!px-2` and one of border, with the flag's 24 and `gap-2`'s 8 not
drawn. Everything else in the row is the reference's own: the coverage percentage
at x=996..1028 against this one's 997..1029, and the check circle at the row's
right edge in both.

The coverage percentage is a real count rather than a decoration: it is how many of
the 3,846 keys the language's own table carries, which is what the reference's
generated coverage file is a generator writing, and the first port of the pane had
the data and did not draw it.

### The line height that was read backwards

The first port of this dialog came out with no title, a 204-pixel category heading and
an empty content pane, and the cause was one line of iced's API rather than any of the
transcribed numbers: `Text::line_height` takes `impl Into<LineHeight>`, and iced's
`From<f32>` for that type is `LineHeight::Relative` -- a *multiple* of the text's size.
`line_height(18.0)` on a 16-pixel label is a 288-pixel line; `line_height(32.0)` on the
24-pixel title is 768, which pushed the header past the window and left the rows the
capture compares adrift by 100 or more. Five call sites had written the reference's
own pixel numbers into it. `iced::Pixels` is iced's absolute form, every site passes
that now, and a gate scans every source in the crate for a bare number in that call
(`ui::tests::a_line_height_is_an_absolute_length_and_never_a_multiplier`) so the next
one cannot be written.

The same pass found the second half of it: a browser pixel-snaps each box's edges after
laying them out, and an unclassed paragraph in the reference is 16 pixels on
`line-height: 1.15` -- 18.4 -- which paints as 18. This rasteriser draws the fraction,
so the painted integer is what the port passes; the card grid's top border lands on the
capture's own y=241 either way in a browser and only lands there here with the
integer.

### The two buttons that were a control the reference does not draw

The Content tab's row had a *Deselect*/*Edit* button at its right end, switching
between a standard and a quiet frame. The reference has no such control: what
`ContentCardTable.vue` puts in that row is a `Checkbox` with `shrink-0` in front
of the name, and a checkbox is not a button at all -- a bare `button` around a
20x20 `rounded-md` square with a 1-pixel border, `bg-brand border-button-border`
and a 16-pixel `CheckIcon` when it is on, `bg-surface-2 border-surface-5` when it
is off, and a `MinusIcon` where the tick would be for a row that has some of its
children on. `ui::checkbox` is that, including the indeterminate arm and the
`brightness(--hover-brightness)` hover every other control here uses; the press
scale (`checkbox-shadow group-active:scale-95`) is the one thing it cannot draw,
and says so. The row's own two labels went with it: a checkbox is not labelled
*Deselect*.

The project card's two buttons were the reference's controls at the reference's
default size instead of the frame's. `ProjectPageHeader`'s actions slot is a
`<Button type="colored" color="brand" size="xl">` with a `DownloadIcon` in front
of `commonMessages.installButton` -- `button.install`, "Install" -- and beside it
a `TeleportOverflowMenu type="quiet" size="xl"`. So both are `xl` now, the
install says *Install* instead of the library's *Create instance*, and it wears
the download icon the reference gives it. The quiet one keeps its own label: the
reference's menu holds copy-link, report and donate, and this launcher's has one
thing in it, so a button says it and a menu would hide it.

### The breadcrumb, which was one invented string per route

The head's trail was a single string written out per route: `""` on Home,
`Discover mods`, `Servers`, `Servers / srv`, `Profile / jelly`, `Project /
sodium`, `ATM10 / Content`. Four of those are inventions, and the shape is not
the reference's at all.

The reference's `Breadcrumbs.vue` draws a *stack*, and the stack is built by the
pages: `useRootBreadcrumb` for a page that is the root of its section,
`useBreadcrumb` for one that sits inside a root. Every page in this launcher
registers exactly one, so every trail here is one entry long -- which is what the
reference's own trail holds on these routes too, since a project reached from
inside an instance is the only thing that would push a second.

| Route | The reference registers | Drawn |
| --- | --- | --- |
| `/` | `app.navigation.home`, a `PlayIcon` (`Index.vue`) | *Home* + play |
| `/browse/:type` | `app.browse.discover-project-type`, a `CompassIcon` (`Browse.vue`) | *Discover mods* + compass |
| `/skins` | the literal `'Skin selector'`, a `ShirtIcon` | *Skin selector* + shirt |
| `/screenshots` | `app.screenshots.heading`, an `ImageIcon` | *Screenshots* + image |
| `/hosting/manage/` | the literal `'Hosting'`, a `ServerStackIcon` | *Hosting* + server stack |
| `/hosting/manage/:id` | the server's name, a `ServerStackIcon` | the id, the same icon |
| `/user/:user` | the user's name, their avatar | the name, no icon |
| `/project/:id` | the project's title, its icon | the id, no icon |
| `/instance/:id` | the instance's name, its own art | the store's name, no icon |

Three things came out of reading the table rather than the routes. *Servers* is
*Hosting*, which is what the page calls itself. An instance's crumb is the
instance and not the instance and its tab -- the tabs are `NavTabs` under the
head, not crumbs in it, which is why `/instance/ATM10/logs` and
`/instance/ATM10` are the same entry. And the last three have **no icon** here:
the reference's visual for them is a fetched image (an avatar, a project icon, an
instance's own art), and a missing icon is a smaller lie than a person or a box
drawn where a picture belongs.

The trail's own numbers are `Breadcrumbs.vue`'s: a `size-5` visual, `gap-1.5`
inside an entry, a `size-5` `ChevronRightIcon` and `gap-2` between entries,
`text-base font-medium leading-6` on the label -- 16 pixels on a 24-pixel line,
which is why the old 14-pixel label was wrong -- the last entry in
`text-contrast` and every earlier one in `text-primary`, and the trail's own
`pl-4` as the gap in front of it.

Measured against the reference's own head at 1280x720: its crumb's icon ink
runs x 257..274 and its label x 283..381; ours runs 265..284 and 291..388. The
eight pixels are the wordmark, which is this launcher's own and twelve pixels
wider than Modrinth's -- the trail itself is at the reference's own offsets from
the icon inwards.

One reading is worth recording because it is a fact about the running client and
not about the vendored tree: at this pin the reference's Skins crumb reads
*Skins*, where `Skins.vue` registers the literal `'Skin selector'`. The vendored
source is what this port takes its numbers and strings from, so the label stays
*Skin selector* and the difference is written down rather than papered over.
