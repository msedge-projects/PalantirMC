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

Inter is now the shell's typeface: 400/500/600/700/800, loaded through
`Settings::fonts` so the first frame is already Inter. `default_font` is weight
500 because that is what Modrinth sets body text at.

**Superseded (4 Oct, `0f78a06`).** Those five faces were our own subset, ~292 KB,
cut by `tools/make_fonts.py` from the release Modrinth's own stylesheet pins. They
are now the reference's own published bytes -- the five files on Modrinth's CDN,
unwrapped from WOFF by that tool -- 1,497 KB for all five and a binary 6.2%
larger, for 2,505 codepoints and 2,548 glyphs against the subset's 505.
Cyrillic, Greek and Latin Extended Additional go from nothing to 254, 121 and 256
codepoints. See "The fonts are the reference's own faces, and the version string
that accused them" below, which is the finding that sent the subset: the subset was
never the fault, because `Version 3.019;git-0a5106e0b` **is** Inter 3.19 and its
`hmtx` advances already matched the reference's for every codepoint both sides
cover, in all five weights, with zero differences.

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

### Our own licence, and what is still to remove (2026-10-08)

The owner asked for this launcher to be their own product rather than a GPL one,
and took legal advice; the instruction that came back was to remove the GPL offer.
`Cargo.toml` is `LicenseRef-Proprietary`, `LICENSE` is the owner's terms,
`README.md`, `THIRD_PARTY_NOTICES.md` and `vendor/modrinth-app/UPSTREAM.md` were
changed in the same commit, and this section is the part that matters more than
any of them: **the licence field is metadata, and the obligation is not.**

GPL-3.0 attaches to what came from the Modrinth App, and that material is still
in the tree while its replacement is written. The list, and what removes each:

| Still here | Where | Removing it |
| --- | --- | --- |
| 1,858 vendored files | `vendor/modrinth-app/` | `rm -rf vendor/modrinth-app`. `UPSTREAM.md` already promises nothing in the build reads it, so its absence changes no test and no artifact |
| 3,846 interface sentences | `text_gen.rs`, generated | write our own English table and point `gen_text.py` at it |
| 189 tokens in four modes | `theme_gen.rs`, `theme_tokens.rs`, generated | choose our own design system -- the owner's instruction is a matte black ground and an orange accent, so the palette is a decision rather than a compile |
| the icon set | `icons_gen.rs`, generated from their copy | 339 of 445 are proved Lucide geometry and re-source from Lucide's own releases under ISC (see the next section); the rest are drawn here |
| 12 PNGs | `crates/palantir-desktop/assets/hosting/` | our own art for the Servers page's empty state |
| 279 lines citing their source | 17 files under `crates/palantir-desktop/src/` | rewritten against a spec of our own |
| the port's own record | `REFERENCE.md`, `GATES.md`, `README.md` | retired with the gates that mirror the reference |

Until that list is empty, the GPL-3.0 terms apply to the listed material and to
the combined work -- which is why this section states it instead of leaving it to
a licence file that no longer claims it.

The route that ends the list is written down as a prompt in
`docs/superpowers/specs/2026-10-08-clean-room-own-licence-rewrite.md`: a fresh
repository, written from behaviour and public formats, with no file taken from
this one, and an audit that fails on GPL text, an unlisted asset or a citation to
another project's source. The theme it carries is the owner's: matte black
`#0B0B0C` for the ground, orange `#FF7A1A` as the one accent, black labels on
orange, no pure black and no pure white anywhere.

### The icon set is Lucide's, and 339 of 445 files can prove which release they came from

The licence question above turns on whose expression is in the tree, so it was
measured rather than assumed. `tools/lucide_source.py` reads every icon in the
vendored set -- 445 files: 313 at the top level, 102 tag categories and 30 tag
loaders, of which four are refused by the generator and could not be drawn anyway
-- and asks which published `lucide-static` release holds the same drawing.

**It does not compare text.** The first pass did, and under-counted badly: it
found 216 matches against a single release. `check.svg` is the case that shows
why -- theirs is `<path d="M20 6L9 17l-5-5" />` and Lucide 0.562.0's is
`<path d="M20 6 9 17l-5-5" />`, which is one implicit `lineto` written out in full
against one left implicit, and the same drawing either way. Their build
re-serialises what it takes, so a string comparison measures formatting. Reading
both files through `gen_icons.py`'s own parser -- the one the build trusts -- and
comparing the geometry that comes out is what the tool does instead.

What that finds, over a ladder of 60 releases sampled from the registry:

* **339 icons are that release's drawing.** 169 of them resolve to `0.100.0`,
the oldest release the ladder samples, so the honest reading of those is "at least
that old" rather than "taken from 0.100.0".
* **102 are in none of the 60.** Of those, 29 are the loader marks -- Forge,
Fabric, OptiFine, Paper and their like, which are other projects' logos and were
never Modrinth's to license in the first place -- and the rest are drawings made
for the reference: `affiliate`, `client`, `dropdown`, `gap`, `omorphia`,
`page-round`, `spinner`, `unknown`, `updated` and the like, none of which exist in
Lucide under any name.
* **Four are refused** by the generator by name, and are absent from the tree
anyway.

The distinction decides what can be re-sourced and what has to be drawn: the
Lucide-derived majority is ISC -- use, copy, modify and distribute for any purpose
with or without fee, provided the notice travels -- which is why the clean-room
prompt tells the new tree to take its icons from Lucide's own package and carry
Lucide's `LICENSE` verbatim.

### The settings dialog's remaining six panes

The dialog was ported with two panes and now carries the eight of the reference's own
`AppSettingsModal.vue` list that this launcher can draw: Features, Behavior, Privacy,
Synced settings, Java installations and Resource management join Appearance and
Language. The three the list has and this does not are Feature flags
(`developerOnly: true`) and Profile and Social, which are about a Modrinth account.
Each pane is drawn from its own component -- `FeaturesSettings.vue`,
`BehaviorSettings.vue`, `PrivacySettings.vue`, `instances-synced-settings/index.vue`,
`JavaSettings.vue`, `ResourceManagementSettings.vue` -- at the reference's own
spacing: `mt-6` (24) and `mt-8` (32) between sections, `mt-4` (16) between rows,
`gap-2.5` (10) inside one, and `text-lg`/`text-xl` headings on the 28-pixel line
Tailwind gives both.

Four things are drawn differently, and each is recorded here rather than only in the
drawing.

**The three sliders are number fields.** The quick-instances limit (`0` to the
reference's own 20), the maximum concurrent downloads (`1` to `10`) and the maximum
concurrent writes (`1` to `50`) are `Slider`s in the reference, and this kit has none
-- the same substitution the instance settings modal's memory section already makes,
for the same reason. What each sets is a number in the preferences file, so a field
edits that number and the reference's own range is clamped where the message lands.

The Java rows fill `Java {version, number} location` through the generated
`text_gen::app_settings_java_installations_location_title`, and each row draws the
path the file holds: the reference's `JavaSelector` is a picker over the JREs it has
found, and this launcher goes looking for none. The pane's first draft also drew the
key rather than the message, which is a placeholder on screen.

**The quick-instances count is a count.** It was a bool, which cannot say "seven".
The preference is `quick_instance_limit: Option<u32>`, and `None` is both the shipped
default and the reference's own spelling of the top of its slider: `normalizeLimit`
in its `use-quick-instance-limit.ts` maps anything at or above the maximum to `null`
rather than to the number. The rail the number governs draws no recent instances yet
(`rail_separator`'s own note), so the row stores a number nothing reads -- that is the
honest half of it, and the rail is the other.

**Synced settings draws the reference's rows unavailable.** The five rows its own
`isSyncedOptionAvailable` leaves in -- `data_packs` is filtered out -- are drawn in
its order with the switch it disables while syncing is unavailable, which is this
launcher's state for good: it shares nothing between instances. The first draft of
this pane drew a sign-in notice from the *Profile* tab's keys instead, which the
reference's own pane never shows. The edit buttons, the sync-source picker and
`<LaunchOptions />` are not drawn: each opens a modal over data this launcher does not
keep, or is a section of its own.

**Database backups cannot be opened.** The folder the reference's button opens
belongs to an app with a database, and this launcher keeps none, so that button is
drawn with its press taken away rather than opening nothing or opening somewhere the
sentence does not name.

Measured after the slice, on this machine: `cargo test --workspace --all-targets
--locked` at **1357 passed / 0 failed / 19 ignored** (179 + 8 + 859 + 4 + 33 + 274),
and `cargo clippy --workspace --all-targets --locked -- -D clippy::correctness` with
nothing to say. The push's own run, `37674634610` on `ef5d668`, is green through all
five jobs with the same workspace rows (`Test workspace` 2m28s, `Lint` 1m21s), the
live suite 19 passed / 0 failed in 160.51s, and both Windows exes staged --
28,087,808 B (msvc) and 24,419,328 B (gnu), which are five times the sizes the rows
above record because the slice before this one traded exe size for frames
(`opt-level` `"z"` to `3`).

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

### The tab strip, which was 32 pixels tall and had no icons on it

`NavTabs.vue` draws a link as `flex flex-row items-center gap-2 px-4 py-2` with
a `size-5` icon in front of a `text-nowrap` label, inside a
`relative flex w-fit rounded-full bg-bg-raised p-1 text-xs sm:text-sm font-bold`
track. Three things were wrong with ours.

The strip was **32 pixels** tall, fixed, with the label centred in the leftover
space and no line height of its own. `py-2` around `text-sm`'s own twenty-pixel
line is 36, and that is the number the reference has.

The tabs had a **two-pixel gap** between them. The reference's nav has none: the
`p-1` is the only space in the strip, so its links touch.

And the strip had **no icons**, because `ui::tabs` took labels and nothing else.
`link.icon` is `v-if`-ed on the reference's link, so an iconless tab is a shape it
has -- and two of its pages use it: the project page's Description/Versions/
Gallery and the project-type strips on Discover and on a profile. The instance
page registers one for each of its six, and the glyphs are the reference's:
`BoxesIcon`, `FolderOpenIcon`, `ImageIcon`, `GlobeIcon`, `TerminalSquareIcon`,
`UserPlusIcon`. The icon's ink is a *different* rule from the label's --
`getIconClasses` gives an inactive icon `text-secondary` where `getLabelClasses`
gives the label `text-contrast` -- and both go to
`text-button-textSelected` when the tab is the one in force.

That reading also moved a tab. `layout.vue`'s `tabs` computed pushes Content,
Files, **Screenshots**, Worlds, Logs, Share: the two settings-gated ones come out
in the order that computed pushes them, which is not the order this crate's enum
declares them in, and our strip had them the other way round. `State::TABS`,
`State::tab_at`, `TAB_KEYS` and `TAB_GLYPHS` are one list now, in the reference's
order, and the strip's test asserts all four together so a tab cannot be moved in
one of them and not the others.

One more of `NavTabs.vue`'s own rules came with it: `v-if="filteredLinks.length >
1"`. A strip of one tab is not a strip, and this draws nothing rather than a pill
around a single word.

### The instance header, which was a card with three loose facts on it

The instance page's header was the one header in this tree still wearing
`ui::card`, and the row under the name was three things this launcher
chose: a pill of the loader and game version, the playtime, and a count of
enabled mods out of all of them. None of that is what
`page-header/index.vue` draws.

`base/page-header/index.vue` is a plain block. Its root is `flex flex-col
gap-2`, and the one child in it sits above a `border-0 border-b border-solid
border-divider` hairline with `pb-4` under it -- a rule under the header, not a
box around it. `border-divider` resolves to `--surface-5` in the light
variable set (`variables.scss` line 102), which is why the rule is
`Ink::Surface5` and not `Surface2`. The row is `flex flex-wrap items-start
gap-4`, so the name's column and the actions both sit at the *top* of it,
and the name's column is `flex min-w-0 flex-1 flex-col justify-center gap-2`.
The title is `text-2xl font-semibold leading-none text-contrast` -- twenty-four
pixels at the semibold face on a line exactly as tall as itself, so `TITLE` is
both the size and the line height.

Under it, `pages/instance/components/page-header/index.vue` fills
`PageHeaderMetadata`, which is `flex min-w-0 flex-wrap items-center
gap-x-[1.625rem] gap-y-2` with a `BulletDivider` on every item. Four arms, in
this order:

* the loader and the game version, over a `TagIcon`;
* the playtime, over a `TimerIcon` -- but only `v-if="showInstancePlayTime
&& playtimeLabel"`, so an instance with no seconds has no such fact at all;
* a `ClockIcon` and the relative age when there is a last-played stamp;
* a `ClockIcon` and *Never played* when there is not.

`show_instance_play_time` defaults to true (`use-app-settings.ts` line 16),
so the playtime is drawn whenever there is any -- which is why the clock
never has to say it twice.

Three of those four were being drawn in this launcher's own words, and they
were being drawn as abbreviations. `loaderLabel` is
`[loaderDisplayName, game_version].filter(Boolean).join(' ')` and
`formatLoaderLabel` is the loader's *name* -- *Fabric*, *Vanilla* -- with no
build on it, so `InstanceCard::loader_label` says *Fabric 1.21.1* where the old
pill said *Fabric 0.19.5 · 1.21.1*. The playtime counts down and says
the largest unit it reaches, spelled out: *3 hours*, not *3h 12m*.
`PageHeaderMetadataTimeItem` joins `useRelativeTime` -- `Intl.RelativeTimeFormat`
over dayjs's thresholds -- to its own `label`, so the age reads *Last played 2
hours ago* against a stamp of zero reading *Never played*; the two arms are one
function now (`instances::last_played_label`). `InstanceCard` carries the
stamp itself (`lastLaunchTime`), which is why it grew a
`last_launch_millis`.

The mods count is not one of the four. The reference's header says what an
instance *is*; what is in it is the Content tab's list, and the row under
the title no longer carries a number that belongs somewhere else.

`ui::metadata_row` draws the row at `PageHeaderMetadata`'s own
measurements: `METADATA_GAP` is 26 (`gap-x-[1.625rem]`), the dot is
`METADATA_DOT` = 6 (`BulletDivider`'s `min-w-1.5`), and the gap is split
ten either side of the dot rather than measured twice -- which is what
`absolute right-full w-[1.625rem]` puts there. The first item has no dot,
because `metadata/index.vue`'s scoped rule hides it on `:first-child` and on
every item a `ResizeObserver` marks `data-page-header-metadata-row-start`.
The icon is 20 (`size-5`), the label 16 at `font-medium leading-none`
in `text-secondary` (`page-header-metadata-item.vue`'s `baseClass`), and
the icon sits eight from it (`gap-2` on `contentBaseClass`).

### The Discover sidebar, which was not there at all

§31 recorded the right panel as one of the items still different, and the
recording was right: on Discover the reference draws a *Hide already installed*
switch and a *Category* list with icons where this launcher draws *Getting
started* and News and nothing else.

Where it goes is `App.vue`, not the page. `Browse.vue`'s last two lines are
`<Teleport v-if="browseRouteActive" to="#sidebar-teleport-target"><BrowseSidebar /></Teleport>`,
and that target is a div in the *middle* of `app-sidebar-scrollable`:

    <OnboardingChecklist ... />
    <div id="sidebar-teleport-target" class="sidebar-teleport-content"></div>
    <div class="sidebar-default-content" ...>

so a page with a sidebar of its own adds a section to the panel rather than
replacing it, and it lands between the checklist and the *Playing as* card.
`Shell::panel` now draws `pages::discover::sidebar` at that point.

`browse-tab/sidebar.vue` is a column of sections, and each one is
`border-0 border-b-[1px] border-[--brand-gradient-border] p-4 last:border-b-0`.
The first is not a filter: `showHideInstalled` puts one `<label class="flex
cursor-pointer items-center justify-between gap-3 text-contrast font-medium">` in
it over a `Toggle small`, which is `ui::switch` at `Toggle.vue`'s own 48x24.
`showHideInstalled` is `projectType === 'modpack' || (isServerContext && !==
'modpack') || !!instance` -- two of the three arms are contexts this shell has
no route into, so the modpack tab is the one that draws it and the others draw
nothing rather than a switch that would hide nothing.

**The switch is a request filter, not a row filter**, which is the part that
decided the shape of the port. `Browse.vue`'s `instanceFilters` pushes `{ type:
'project_id', option: 'project_id:<id>', negative: true }` for every installed
project, and `search.ts` renders that list into one `project_id NOT IN [...\]`
group -- so the count the API answers is the count *after* the hiding. A page
that dropped the rows it did not want would show a list of twenty beside a
count of two hundred.

That needed the one piece of `palantir-net` this tree had never wanted:
facet groups beside the project type's. `facets` is a list of or-groups, so a
second constraint is a second *group* and not a second parameter --
`search_url_parts_with_facets` builds `[["project_type:modpack"],
["project_id NOT IN [\"AANobbMI\"]"]]`, and `Search::with_facets` puts them on
the value the shell already sends. A group goes inside one pair of quotes
exactly as it stands, which is what lets the exclusion syntax carry quotes of
its own; a group is opaque on the way in and on the way out.

The page owns the *flag* and the shell owns the *ids*, which is what the
seam already says: `Asked` carries `hide_installed` beside the query, and
`Shell::without_installed` reads the project id out of each instance's
link file (a few bytes, no request) and completes the facet. An instance with
no link is not in the list -- there is no project id to hide by -- and a
launcher with no instances at all asks the search unchanged, because
`facets=[[]]` is a request for a project that satisfies nothing.

**What is still missing is everything under the switch.** Each remaining
section is a `SearchSidebarFilter` for one filter type -- Category,
Environment, Game version, Loader, License -- and every option in them is
read out of `GET /tags`, which this launcher does not ask for. That is the
next slice, and it needs the tags endpoint rather than another widget.

### The tag list, which is what the filter sections are made of

The first draft of this said `GET /v2/tags` is one document and that the
reference's `get_game_versions`, `get_loaders` and `get_categories` are three
names for three fields of it. **That was wrong, and the live service said so:**
`GET /v2/tags` answers `{"error":"not_found"}`. The three lists are three routes
-- `/v2/tag/category`, `/v2/tag/game_version`, `/v2/tag/loader` -- each a bare
JSON array, which is why the reference's three helpers are three calls. The
reference's `helpers/tags.ts` is not vendored, so this reading came from the
service rather than from the tree, and it is written down here because the
mistake is the kind that a class-list reading makes and a `curl` settles.

What survived the correction is the join: a browse page wants all three at once,
and the filter lists are queries over the three together. `Store::tags` is the
join, and it is three requests. One failure fails the read -- a section with some
of its options missing is a filter that quietly does not filter.

Three of its queries are the ones `search.ts` builds its filter lists out of,
and they are queries rather than filters because the sidebar's *sections* are
made of them:

* `categories_under(project_type, header)` -- `search.ts`'s filter id is
  `category_${project_type}_${header}`, so one section is one such pair;
* `headers(project_type)` -- the same sections, in the order the API first lists
  them. The reference sorts these with `sortedCategories` from
  `@modrinth/utils`, which this tree does not vendor, so the API's own order is
  what is used;
* `loaders_for(project_type)` -- a loader is in the lists its
  `supported_project_types` name, which is why `fabric` (mods) and the modpack
  loaders are rows of different sections;
* `game_versions_of(version_type)` -- a version's type is what puts it under
  *Show all versions*.

On the page side this is `Ask::Tags`, and it is asked **before** the first
search rather than beside it: the shell's `opening` hands back one request at a
time, and the tag lists are the ones whose answer the sidebar cannot draw a
single option without. The reference fetches it on mount too. It is asked once
for the life of the page.

Nothing draws from it yet. `State::tags` is a `Load` of its own so that a store
with no engine draws a section that knows it has nothing rather than one that
drew itself empty.

### The category sections, which are one section per header

With the tag lists in hand the rest of the sidebar is widgets rather than
endpoints, and this is the slice that takes the sections `search.ts` builds out
of it. A `FilterType` id is `category_${project_type}_${header}`, so one
section is one `(project type, header)` pair, and `Tags::headers` and
`Tags::categories_under` are that query.

Each section is `SearchSidebarFilter` at the app variant own sizes: an
`Accordion` whose button is the sidebar `buttonClass` -- `flex flex-col gap-1
px-3 py-3 w-full hover:bg-button-bg` -- around a `flex items-center gap-1
w-full text-contrast` row holding a `text-base` `h3` and a `size-5`
`DropdownIcon` at `ml-auto` that turns over when the section is open. The first
section button gets `pt-4` where the others get twelve from `py-3`, which is
what `[&:first-child>button]:pt-4` is.

The header slot class is `text-base m-0` **without** `font-semibold` here,
where the web variant has it: the app leans on its own heading weight, and a
port that added the weight would be drawing the web page.

The options are `SearchFilterOption` rows -- `flex ... rounded-xl px-2 py-1
text-sm font-semibold` with the label, a 16-pixel `CheckIcon` at `ml-auto`,
`bg-brand-highlight text-contrast` when chosen and transparent with a
`bg-button-bg` under the pointer when not. `bg-brand-highlight` is
`Ink::ColorBrandHighlight`, which is `rgba(27, 217, 106, 0.25)` over the
wash rather than an opaque green.

**The category icon is not drawn.** `getCategoryIcon` hands out an SVG per
category out of `@modrinth/assets`, and this launcher ships no icon set for the
three hundred tags Modrinth publishes. A row with a box where a drawing
belongs would be a worse lie than a row whose label starts eight pixels
further left.

Choosing is a request change, so it asks on the turn it changes and goes
back to the first page. `search.ts` pushes one part per chosen option and
joins the parts with ` AND ` into a **single** facet string, because the
strings inside one `facets` group are alternatives to Modrinth: two chosen
categories have to both hold, and two groups would ask for either. The set is
a `BTreeSet` rather than the order the rows were pressed in, so the same
choice is the same request -- and therefore the same cache entry -- whichever way
round it happened.

Opening a section is not a request. The reference opens every category section
on arrival (`getFilterOpenByDefault` opens any id starting with `category`),
so the state is `collapsed`: the ones the reader has shut, rather than the ones
nobody has touched.

The tag names themselves come from `ui/src/utils/tag-messages.ts`, which is
vendored, so `locale::category_label`, `locale::loader_label` and
`locale::category_header_label` are `formatCategory`, `formatLoader` and
`formatCategoryHeader`: the reference own message for the tag when it
publishes one (`kitchen-sink` is *Kitchen Sink*, `gui` is *GUI*), and
`capitalizeString` when it does not. The lookup is by key name over the
generated table (`Key::name` is the reference key verbatim), so a tag this
launcher has never seen is still a lookup and not a table of its own.

Still missing under these: Environment, Game version, Loader, License, and
the two exclusion lists. Every one of them is a `SearchSidebarFilter` over the
same document.

#### What the reference own capture says about the sidebar

Both clients were on the box at once for this one: Modrinth App v0.21.6 on
`:99` and this launcher on `:98`, both at 1280x720. The numbers below
are read off the reference plate at `/tmp/ref-discover-panel.png`, not off a
class list.

| Thing | Measured on the reference |
| --- | --- |
| Panel left edge | page to x 979, the border at **980**, wash from 981 |
| Panel wash | `(24, 32, 30)` |
| The border | `--brand-gradient-border`, `(33, 50, 40)` at the top down to `(29, 41, 34)` at the bottom |
| Switch section rule | one row at **y 322**, same colour as the left edge |
| Switch row ink | y 286..305, label from x **998** (that is `p-4`) |
| Section heading | ink y 341..355, from x **994** (`px-3`), chevron ink x 1251..1264 |
| First option | ink y 385..400, label ink from x **1021** |
| Option pitch | 33 (`385 -> 418`) |

Three of those changed the drawing:

* **The section rule is `--brand-gradient-border`, not `--divider`.** It
  measures `(32, 48, 40)` and `--surface-5` would be `(66, 68, 74)` -- a
  difference visible at a glance, and the first draft of this drew a
  `--surface-5` rule. It is the same token as the panel own edge, so the
  same `Ink::BrandGradientBorder` and the same one-pixel line.
* **The heading face is semibold, though the class does not say so.** The
  app-variant header slot is `text-base m-0` with no weight class, where the web
  variant has `font-semibold`. On the plate *Category* reads heavier than
  the `font-medium` *Hide already installed* above it, so the capture
  decided it. Where a class list and a plate disagree about a face, the plate
  is the authority.
* **The option label is on `text-sm` own line** -- twenty pixels, not the
  nineteen this toolkit would give a fourteen-pixel label -- which is what
  makes the rows thirty-three pixels apart rather than thirty.

One thing the plate settles about the option rows is the **icon**: each one has
a sixteen-pixel drawing at x 998..1011, and the label starts at 1021 --
sixteen pixels of icon and eight of gap. This launcher draws no icon and
therefore leaves that sixteen pixels empty, putting the label where the
reference has it and the icon where nothing is. That is the smaller lie; the
alternatives were a placeholder box or a label eight pixels out.

The gradient itself is the one part of that rule this toolkit cannot draw: the
reference runs a vertical gradient along the panel edge and one flat colour
sits in the middle of it. The panel own left edge already had that limit and
says so.

A correction to the section above, and one the live service settled.

**There is no `/tags`.** `GET /v2/tags` answers `{"error":"not_found"}`;
the three lists are three routes -- `/v2/tag/category`, `/v2/tag/game_version`,
`/v2/tag/loader` -- each a bare JSON array. The first draft of this said
otherwise, from the shape of the reference three helper names, and a `curl`
took it apart in one line. That is worth recording because it is the second
time in this tree that a reading from names rather than from the wire has been
wrong, and both times the wire was one command away.

**There is also exactly one category section, not several.** The live
`/v2/tag/category` gives one header per project type -- `categories`, ten entries
for `modpack` -- so the reference sidebar shows a single *Category* section
with ten options, which is exactly what its plate shows. The other filter
types `search.ts` declares (`environment`, `game_version`, the loaders,
`license`) are real, and they are the sections still missing here.
### The panel toggle, which is not everywhere

The reference gates its own: `v-if="!forceSidebar && appSettings.toggleSidebar"`.
On Discover, Project and User -- the three routes `App.vue`'s `forceSidebar`
names, and the three `Route::forces_sidebar` names here -- there is no arrow in
the head at all, because the panel is up whatever the reader has toggled. This
shell drew one unconditionally, which put a control on the page whose press
could not move the thing it pointed at. The gate is now
`Shell::panel_toggle_shown`, and a test walks both halves of it.

The other half of that row was the order: `App.vue` puts the `IconButton` before
the `AppActionBar`, and this shell had it after. On Discover, where the toggle
is not drawn at all, the action bar is now the only thing on that side of the
head -- which is what the reference plate shows, and which is why the earlier
capture of the two side by side had an arrow sitting between the chip and the
window controls where the reference has nothing.

The arrow itself now flips: `rotate-180` while the panel is down, which this
draws as the launcher's own `LeftArrow` because iced has no transform on an
icon. The reference's class list says the rotation and not the glyph, and the
two are the same shape.

### The ad block under the panel

The reference stacks it *under* the scroll region, not inside it:
`PromotionWrapper` is a sibling of `app-sidebar-scrollable`, and the scroll
region carries a `pb-12` so the last section is not left under the link. So the
panel here is a column of scroll, link, fade and plate, and the scroll carries
the reserve.

The numbers came off the reference plate rather than off the class list, because
the class list does not say where anything lands:

| Thing | Measured on the reference |
| --- | --- |
| Link ink | x 1035..1227, y 440..456 |
| Link ink colour | `(199, 138, 255)` = dark `--color-purple` |
| Ad art | from y 471, filling to the panel foot |
| Panel width | 300 (`min-w-[300px]` on the image) |

The fade is `height: 5rem` of `--brand-gradient-fade-out-color`, which in dark
is `linear-gradient(to bottom, rgba(24, 30, 31, 0), #171d1e 80%)` -- transparent
at the top and the panel's own darkest wash at four fifths of the way down.
`.app-sidebar.has-plus::after` is `display: none`, so a Plus reader gets a hard
edge instead; this shell has no Plus reading and draws the fade.

**The image itself is not drawn.** It is a remote promotional asset fetched from
Modrinth's CDN, and this launcher has no ad fetch. The plate draws a `bg-bg` box
of the reference's own size where it would go: a box in the right place at the
right height says there is something here without claiming what, which is the
smaller lie of the two. The link above it is real, is drawn in full, and goes to
the reference's own `modrinth.plus?app`.

One gate is a reading rather than a port. `showAd` is `sidebarVisible &&
!hasPlus && credentials !== undefined`, and the third term is the one that
cannot be checked here: this launcher holds no Modrinth credential and never
will, so the reference's own answer for a launcher in this shape is the block
being drawn. The other two are real -- a Plus subscriber gets neither the block
nor the fade, and a panel that is down has no block under it -- and both are
what `promo_shown` reads.
### The welcome screen's art, which is a raster this launcher does not ship

The two welcome screens were captured side by side at 1280x720 -- Modrinth App
on `:99`, this launcher on `:98`, both on the same state, no instances -- and
they agree on everything except the hero's art:

| Thing | Reference | This launcher |
| --- | --- | --- |
| Title ink y | 352..372 | 352..372 |
| Description | y 393..406 | y 393..406 |
| Create button | y 437..473 | y 437..473 |
| Hint row | y 496..510 | y 496..510 |
| Foot prompt | y 626..639 | y 626..639 |
| Import button | y 658..694 | y 658..694 |
| **Hero art** | **x 485..558, y 236..309** | **x 496..548, y 233..318** |

Every line of the page lands where the reference has it. The art does not, and
it is not a matter of size: `WelcomeScreen.vue` draws
`<img :src="modrinthSocialIcon">` at `size-[6.25rem]`, and that asset is
`app-frontend/src/assets/welcome/modrinth-social-icon.png` -- a 512x512 RGBA
raster of Modrinth's spiral mark on its own dark plate. This launcher draws
[`crate::brand::logo_handle`]`, the SVG wordmark mark from the head, which is a
hexagon over three chevrons. They are different drawings, not different sizes
of one drawing: the reference's ink is 73x73 and ours is 52x85, so it is not
even the same aspect.

**It is not copied, and the reason is the asset rather than the pixels.** That
PNG is a third-party binary under Modrinth's own licence, and putting one in
`crates/palantir-desktop/assets/` means a `THIRD_PARTY_NOTICES.md` entry and a
decision about redistributing a brand raster inside a launcher that is not
Modrinth's. The SVG mark this shell already ships is drawn from the vendored
frontend's own SVG and carries no such question, so the art stays a vector and
the plate is left undrawn rather than filled with a box that says "there is
something here" in a shape the reference does not use.

Recorded here because it is the one visible difference on the page and it is a
licence decision rather than a port, which is the kind of thing a later reader
would otherwise try to fix with a `sed`.

### The browse search field is 48 pixels, not 40

`browse-tab/layout.vue`'s `<Input>` carries `size="large"`, and this is the
one search field in the tree that does. It is `h-12`: forty-eight pixels at
`px-4` in a `rounded-[14px]` frame, `bg-surface-4` inside a `border-surface-5`
hairline. Every other one -- the library toolbar's, the screenshots page's, the
creation flow's version and build pickers, the language dialog's -- is
`standard` or unstated, which is why [`crate::ui::search`] draws the `h-10` that
is the majority reading of the component.

A capture of both clients at 1280x720 settles it against the class list:

| Thing | Reference | This launcher, before | After |
| --- | --- | --- | --- |
| Field rows | y 126..173 (48) | y 129..167 (40) | y 126..173 |
| Fill | `(52, 54, 60)` = `--surface-4` | same | same |
| Hairline | `(66, 68, 74)` = `--surface-5` | same | same |
| Sort row | y 174..229 | y 169..232 | y 174..230 |

The colours were already right -- the earlier draft of [`crate::ui::search`]
happens to draw `--surface-4` under a `--surface-5` border too, though it
documents them the other way round -- and only the height was wrong. The page
now calls [`crate::ui::input_sized`] at [`crate::ui::InputSize::Large`], which
is the row of the table the reference's own `InputFrame.vue` defines, rather
than the majority-sized helper. The sort row's own 56 pixels are unchanged.

The test asserts the four numbers and, deliberately, that `Standard` is *not*
48 -- so the assertion is a distinction between two rows of the table rather
than one number checked against itself.

**The other four search fields are not changed**, and the reason is that their
sources are not in the vendored pin: `creation-flow` has no directory under
`app-frontend/src/components/`, so the version and build pickers have no
template here to read a size out of. Guessing forty for them because it is the
commonest would be a second reading from the majority rather than from the
component, which is the mistake this section keeps having to correct.

### The §31 list, brought back in line with the tree

§31's "Still different, off the same two recordings" was written before any of
the captures were possible and is now stale on five of its nine items. What is
left, and what closed:

* ~~**The titlebar has one control too many**~~ -- the launch arrow beside *No
  instances running* is the panel toggle, and the reference gates it behind
  `!forceSidebar`, so on the pages where §31 saw it there is none at all. Gone,
  with the order corrected too (it belongs before the action bar).
* ~~**Discover's sort row has two controls the reference does not draw**~~ -- the
  *Filter results...* button and the *Modpacks* • relevance caption are gone,
  and §31's own reading of the reference's `lg:hidden` wrapper was right.
* ~~**Card art**~~ -- the thumbnails carry their project icons; the empty
  squares §31 saw were the frame before the icon round landed.
* ~~**The right panel carries different sections**~~ -- mostly. *Getting
  started*, *Playing as*, News and the Discover filter sections are in, and the
  ad block under the panel is in with the link, the fade and the plate at the
  measurements above.
* ~~**Home is a library grid**~~ -- heading, both toolbar rows and the tiles are
  in `pages/home.rs`. What is *not* in is the *Jump in* strip, and that is a
  decision rather than an omission: it is behind the `worlds_in_home` feature
  flag, which is off in the reference's defaults, so the reference does not
  draw it either.
* **Loading states.** Still open, and now differently so: the vendored pin has
  **no skeleton component at all** -- `grep -rl skeleton vendor/modrinth-app`
  returns three files, all of them using the word for something else. So §31's
  claim that the reference "draws skeleton result cards with a spinner" cannot
  be checked against this pin, and drawing a skeleton nobody's template asks
  for would be inventing a surface. Named, not built.
* **Spacing and sizes.** §31 called these "eyeballed, not measured". The
  browse field was exactly that: forty pixels where the template says
  forty-eight. The ones measured off the plate since are in §33 and in the
  sections above; the rest of the list is still eyeballed, and the fix for
  each is the 189 transcribed tokens, not a screenshot.
* **Animations.** Still open. Hover tweens (§29) and the scroll glide are in;
  the four §31 named are not, and two of them are attached to surfaces that do
  not exist yet.

### The browse header's shadow, and a three-pixel offset under it

Two things sit between the tab strip and the search field that this launcher
does not draw. Measured at x 700, both clients at 1280x720:

| Row | Reference | This launcher |
| --- | --- | --- |
| 117 | `(52, 54, 60)` -- the strip's own `border-b border-surface-5` | `(22, 24, 28)` |
| 118..125 | `(18, 20, 23)` fading to `(22, 24, 28)` | `(22, 24, 28)` -- flat |
| 126 | `(66, 68, 74)` -- the field's hairline | `(22, 24, 28)` |
| 129 | -- | `(66, 68, 74)` -- the field's hairline |

**The band is a drop shadow, not a fill.** Eight rows of a gradient from
`(18, 20, 23)` up to the page background, immediately under the header's
`border-b`. `browse-tab/layout.vue`'s header carries `sticky top-0 z-20`, and a
`z-20` sticky header that pins under a scrolling page is what casts it. This
shell's tab strip is not sticky at all -- it scrolls with the page -- so the
shadow is a symptom of a missing behaviour rather than a missing decoration,
and drawing the band without the stickiness would put a shadow under a strip
that moves, which is worse than not drawing it.

**The three pixels are the same cause.** The field's hairline is at 126 in the
reference and 129 here. The class list does not account for the difference and
neither reading does: the header's own `mb-4` is sixteen, and the reference's
gap from its border row to the field is nine. Something between the two is
doing arithmetic this tree cannot see -- most likely the `-mx-6 -mt-6` the header
pulls itself by against the page's own inset, which is a negative margin
against a container this shell's padding model does not have. The right fix is
the header's own box model, not a padding number tuned until the gap is nine,
and that is a slice rather than a constant.

Not fixed, and written down with the numbers so the next pass starts from a
measurement rather than from a guess about which padding is wrong.

### The browse header is pinned, and the shadow under it is not decoration

`browse-tab/layout.vue`'s header is `sticky top-0 z-20 -mx-6 -mt-6 mb-4
rounded-tl-[--radius-xl] border-0 border-b border-solid bg-surface-1 px-6 py-4
border-surface-5`. That is a pinned band: it holds its place while the results
scroll under it, and it casts a shadow doing so. This page was one scroll
region, so the strip scrolled away with the results and there was nothing for a
shadow to belong to — which is why the band was *named* in the previous section
rather than drawn.

It is pinned now, in the arrangement `pages::instance` already uses — a pinned
part above, a scroll region below — and the reason is the same one that page
gives: a page that scrolled as a whole cannot report where its list starts.

Measured at x 300, both clients at 1280x720:

| Row | Reference | This launcher |
| --- | --- | --- |
| Band | y 73..116, `(39, 41, 46)` | y 73..116, `(39, 41, 46)` |
| Rule | y 117, `(52, 54, 60)` | y 117, `(52, 54, 60)` |
| Shadow | y 118..125, `(18, 19, 23)` → `(22, 24, 28)` | y 118..125, same, within 1/255 |
| Field hairline | y 126 | y 126 |

Four things were wrong on the way there, and each is a mistake worth naming
because the class list does not flag any of them.

**The band is `--bg-raised`, not `bg-surface-1`.** The header's own
`bg-surface-1` is `(22, 24, 28)` in this theme, which *is* the page background —
drawing it would be drawing the page over itself, which is what the first draft
did and why the band was invisible. The `(39, 41, 46)` a capture measures is the
strip's own `--bg-raised`, the same colour as the head bar. It is also exactly as
wide as the tab track: at x 900, past the last tab, the reference's page is
already `(22, 24, 28)`, so the `-mx-6` full-bleed is not something this port
reproduces.

**The rule is `--surface-4`, not `border-surface-5`.** `(52, 54, 60)` against
the `(66, 68, 74)` that `--surface-5` would give — the same class-list-versus-
plate disagreement that turned the sidebar's section rules green.

**The page's inset goes *above* the band, not inside it.** Padding the strip by
`INSET` put twenty-four rows of page background inside the band instead of
before it, so the band started at 49 and ran twenty-four pixels too far down.

**A downwards fade is a half turn, not a quarter.** iced measures a linear
gradient's angle from the positive x axis and walks it *up* the box, so
`FRAC_PI_2` rendered the eight-row shadow as one flat `(19, 21, 24)`. `PI` gives
the reference's own ramp. This is the same trap as the panel's fade and the same
fix, and the second time is the one that says it should have been checked
against a capture before the first.

The rule and the shadow are two elements rather than one gradient with a stop
placed at 1/9: a hand-placed stop is a fraction of the box's *diagonal*, not of
its height, so it came out as a flat band at the first colour.
## The tab strip is the pill, and nothing else

The section above is wrong, and this is the correction. Every number in its table
is right; what it says those numbers *are* is not.

`browse-tab/layout.vue:141` passes `<NavTabs>` **no `pageNav`**. That prop is the
only thing `NavTabs.vue:3` branches its outer element on:

```html
<div :class="pageNav ? '-mx-6 -mt-2 mb-1 overflow-x-auto px-6 py-2' : 'contents'" ...>
```

With `pageNav` false the outer element is `contents`, so the page shows the
`<nav>` itself and nothing around it — `relative flex w-fit rounded-full
bg-bg-raised p-1`, as wide as its tabs. There is no header element, no band, no
`border-b` hairline and no shadow of its own to draw. The hairline and the
`card-shadow` that do appear on screen are the **pill's own**, from
`card-shadow border border-solid border-surface-4` on the same `<nav>`
(`NavTabs.vue:11`), and `ui::tabs` has drawn both since it was written.

A clean capture settles it. At 1280x720 on `/browse/modpack`:

| | Reference |
| --- | --- |
| Pill (`--bg-raised`) | x 88..723 |
| Pill border (`--surface-4`) | x 88 and x 723 |
| Page background at x 730, x 900 | `(22, 24, 28)` — already the page's own |
| Pill rows | y 72..117 (46: `1 + 4 + 36 + 4 + 1`) |
| Head bar's own hairline | y 48 |
| Search field's top hairline | y 126 |

So the pill is **left-aligned at the page's `INSET` from x=64**, and the raised
surface stops at 723. What this port drew instead was a `Length::Fill` container
painted `--bg-raised` with a one-pixel rule and an eight-row gradient under it:
a 914-pixel band where the reference has a 636-pixel pill, and a pill whose own
height was wrong twice over, because the band and the rule beneath it were
standing in for the pill's border and had both been counted as extra.

The corrected strip is `INSET` to the left, `INSET - 1` above and eight rows
under — the last two measured, not quoted: the head bar's hairline at y=48 is
this page's first row, so `48 + 24 = 72` is where the pill's top border lands and
there are 23 rows of page background above it. The eight rows below are the
pill's own `card-shadow` fading into page background and nothing else; the capture
has the shadow on y 118..120 and plain background from 121, so this draws no ink
there at all.

The lesson is the one about captures, and it is worth stating plainly because it
cost a commit: **the first measurement of this region was taken against a
capture with a transient panel over it.** `/tmp/ref-disc6.png` has an unrelated
raised box at x 65..213, y 106..140 — the filter panel mid-animation, or a hover
card — sitting exactly on top of the band's left end. Every x-extent read off
that plate was taken against a background that was not the page's. The row
numbers survived because a row of the band is the same colour wherever you sample
it; the widths did not, and the widths were the thing that was wrong.

Two things this port still does not draw, recorded rather than faked: the
`-mx-6 -mt-6` full-bleed (the page insets its content by 24 and this insets the
pill by the same 24, so the two agree on the left edge and disagree about the
header reaching the pane's edge), and the reference's `mb-4` under the header,
which this accounts for inside `STRIP_UNDER` because the capture leaves no room
between the shadow and the field.

## The fonts are the reference's own faces, and the version string that accused them

A pixel audit measured every label on the profile and hosting pages one to five
pixels narrower than the reference's and put the cause at the typeface, on the
grounds that the bundled faces report `Version 3.019` in their name table while the
reference loads Inter 3.19. Both halves of that are wrong, and the first is wrong
in a way that is worth more than the fault it was blamed for.

**Inter zero-pads the minor component.** The 3.19 release writes `3.019`, so
`Version 3.019;git-0a5106e0b` **is** Inter 3.19 and that exact string with that
exact git hash is in the upstream v3.19 archive's own `Inter Desktop/*.otf`, in
every file the CDN the reference's stylesheet pins serves, and in the faces this
tree shipped. Reading `3.019` as a different release from `3.19` is a formatting
convention mistaken for a version difference.

**And the advances agree anyway.** The `hmtx` advances of the shipped faces are
identical to the reference's for all 505 codepoints they cover, in all five
weights -- **zero** differences across 2,525 comparisons -- so no width could have
moved, and one typeface cannot produce opposite signs either. It does: body text
at 16px measured 5px *wider* here while bold 14px labels measured 3-7px
*narrower*. Meanwhile the profile title, the stats number and the last date word
already matched the reference exactly, which is what confirms the advances really
are the same. The reference's *Data Packs* separates into nine ink runs and ours
into six, every letter a pixel narrower, which is what drawing the bold labels
smaller looks like rather than what a different face looks like.

**What the swap buys is coverage, and it costs 1.2 MB.** Modrinth's CDN build is
not a subset: 2,505 codepoints and 2,548 glyphs, the same counts as upstream's
desktop faces, and a strict superset of what we shipped -- Cyrillic 0 to 254,
Greek 0 to 121, Latin Extended Additional 0 to 256. The reference draws Cyrillic
and Greek mod text out of this very file and we were falling back to a system face
for it. 292 KB becomes 1,497 KB and the binary grows 6.2%; subsetting it back down
needs fontTools and guessing which 2,000 codepoints to keep is worse than shipping
them. Measured after the swap, **all 21 ink boxes on `/user/FlameFire` are
unchanged to the pixel** and the 1-7px class is exactly where it was -- which is
the measurement that says the typeface was never the cause.

Nothing here has ever contained CJK, Kana, Hangul or box drawing, because Inter has
no such coverage to have: the profile page's Chinese renders from a system
fallback through the cosmic-text patch either way, so there was no CJK coverage to
regress.

The faces now come from the reference's own published bytes rather than from
re-running pyftsubset over the upstream release, which is also the only route that
works on this machine: it has neither pip nor fontTools, and the stylesheet's
woff2 cannot be read by cosmic-text at all. A WOFF can, because a WOFF is a
container rather than a compression, so `tools/make_fonts.py` unwraps it with zlib
alone and asserts the version string, the family, the PostScript name,
`unitsPerEm` and the tables a renderer needs before it writes a face -- a bad
unwrap fails the tool instead of shipping a file that renders nothing. Upstream's
`LICENSE.txt` ships beside the faces, since OFL 1.1 asks for the copyright and
licence records to travel with the font.

The width delta itself is still there, and it is space between glyphs: see the next
section, which is where all five of these labels ended up.

## Space between glyphs, fitted per label rather than declared as a letter spacing

**What is not the cause.** There is no `tracking-*` utility anywhere in the
reference's shipped stylesheet, and its only two `letter-spacing` declarations
are `pre code` and `.code-text`. `NavTabs.vue:9` puts `text-xs sm:text-sm
font-bold` on the `<nav>` the label inherits from, and the label `<span>` at `:35`
and `:57` carries only `tab-color text-nowrap` and a colour.
`word-spacing`, `font-variation-settings`, `font-kerning`, `font-optical-sizing`
and `text-rendering` do not occur at all. The gate test
`a_tab_label_carries_no_letter_spacing_in_the_reference` still passes, and what
is recorded here is the reference's own arithmetic read off its pixels rather than
a letter spacing the stylesheet asks for.

**And it is not one constant either.** Fitting a single extra per gap across both
pages asks **+0.72** a character on the profile tab strip and **-0.15** on the
30-pixel Modrinth Hosting heading -- the same fit run again on the shipped code
comes out at **-0.1075** there, which is the same sign and a tighter number. One
constant cannot be both, so an earlier prototype that fitted a single extra across
both pages was rightly thrown out: it left **0.9 px of rms per glyph**, and the
strip came out 16 pixels of error. A per-glyph layout measured against a *global*
fit is not the same thing as one measured against a *per-label* fit, and the
second is what landed.

**What landed fits each label against its own capture.** Five labels carry a
measured extra; everything else keeps the plain `text` path, so the mechanism
reaches nothing that was not measured. `advance()` includes the extra, so the
width layout is told and the width the glyphs are drawn at cannot disagree. A
sixth joined the table later, and it is the one number in it that was **derived**
rather than fitted -- see the card button's section below.

| label | face | extra/gap | rms | rms at extra 0 | advance | `hmtx` |
| --- | --- | --- | --- | --- | --- | --- |
| *Data Packs* | Inter 700 @ 14 | +0.7351 | 0.43 | 2.26 | 83.15 | 76.54 |
| *Modpacks* | Inter 700 @ 14 | +0.3950 | 0.22 | 0.93 | 74.27 | 71.50 |
| *Collections* | Inter 700 @ 14 | +0.5686 | 0.35 | 1.83 | 83.42 | 77.74 |
| *New server* | Inter 600 @ 16 | +0.1806 | 0.41 | 0.68 | 89.99 | 88.36 |
| *Client and server* | Inter 400 @ 14 | +0.1598 | 0.29 | 0.87 | 115.31 | 112.76 |

**How a fit is measured.** A label's glyph origins on the reference's own capture
are the starts of its ink runs at `coverage > 0.5`, and this tree's shaping
satisfies `ink(i) = round(x0 + sum(advance(label[..i])) + lsb(i))` exactly -- so
adding one unknown `extra` per gap and solving by least squares over the
reference's own origins is what the table holds, and the rms beside it is what the
number is worth.

**Three checks that the fits are the reference's and not ours.** The fitter returns
an extra of **-0.043, +0.062 and +0.023** against a true value of zero when run on
this launcher's own captures, so it is not reading our shaping back. On
`/browse/modpack` the same label sits **278.000** pixels to the right of where it
sits here, all nine glyphs to three decimals, so the extra belongs to the string and
not to the page. And *New server* fits **+0.1806** where its own 150.0-pixel box
asks for **0.1818** -- `ButtonFrame.vue`'s `lg` row is `px-4` twice over, a `size-5`
icon and a `gap-2`, so 60 of chrome, and the reference's box measures 150.0 exactly
between its two rings. That is arithmetic which shares nothing with the fitter
landing on the fitter's number.

**A fit that does not beat leaving the label alone is not fitted.** *All*, the
fourth tab, fits +0.1175 at an rms of 0.043 against 0.098 for no fit at all --
three glyphs and the fitter's own noise, so it is not fitted. Across the button
labels alone the reference's extra runs from +0.18 to +0.38 at one size and one
weight, so there is no multiple of `hmtx` that holds them either.

Sixteen pixels of error across the strip, down to two. Ink widths, before and
after, against the reference: *Data Packs* **75 -> 83** against 82, *Modpacks*
**71 -> 74** against 74, *Collections* **77 -> 82** against 83. Those three are the
residuals, at +-1px of ink, and they are open.

**iced 0.12.3 still cannot be asked for a letter spacing.** `Text`'s setters are
`size`, `line_height`, `font`, `style`, `width`, `height`, the two alignments and
`shaping`, and neither `shaping` nor `Paragraph` carries a spacing. That limit is
still real; what was wrong was treating it as the reason the labels are narrow.

## A box-shadow is five rings of opaque ink, not a container's Shadow

**The pane's inset shadow was not being drawn at all.** `App.vue:2755-2770` puts
two things on `.app-contents::before`: the one-pixel `--surface-5` rule
`pane_rule` already draws, and `box-shadow: 1px 1px 15px rgba(0, 0, 0, 0.1)
inset`. The second was missing and the capture reads it on every route. Measured
off `ref/user-ref.png` at y=300 and x=300, identical on `ref/hosting-clean3.png`,
`ref/project.png`, `ref/discover.png` and the scrolled `ref/project-scroll.png`:

| x | y | value | what |
| --- | --- | --- | --- |
| 64 | 48 | `#42444A` | the rule |
| 65 | 49 | `#15171A` | one |
| 66..71 | 50..55 | `#15171B` | six |
| 72 | 56 | `#16181B` | one |
| 73 | 57 | `#16181C` | `--surface-1` |

So it is three depths of the pane's own background -- **0.058, 0.038 and 0.019** --
and not three inks, which is what makes the reserved gutter's own `#15171B`
(`8f33f1a`) the same three numbers wearing a different name. `0.1` over `#16181c`
is declared; which of the three an eight-pixel slice of a fifteen-pixel blur lands
on is not, so the three are measured.

**Three runs, not one `Shadow`,** because of what `7a29753` found: iced 0.12.3
composites a container's `Shadow` inside its element's own rounded-box coverage in
one quad (`solid.wgsl`: `mix(base_color, shadow_color, (1.0 - radius_alpha) *
shadow_alpha)`), which bands its own fill. A background is one quad with neither in
it, so the pane cannot get the banding the tab strip got -- but three explicit
opaque bands can.

**Two things the capture decides and the class list does not.** The rule crosses
the panel and the shadow stops at it -- `#42444A` at y=48 for x=1000 in the
reference, and `#18211E`, `#18211E`, `#18211F`, `#18221F` across y=49..57 with
nothing else in it -- so the top run is the page column's width and not the pane's.
And the order is the reference's own: the reserved column under the shadow and
under the rule, the rule over the shadow, which is how CSS paints one box's border,
inset shadow and background.

**The avatar card's shadow was recorded as inexpressible, and that was worse than
leaving it out.** `Avatar.vue:299` gives every avatar that is not `.no-shadow` a
`box-shadow: var(--shadow-card)`, which the dark look declares
`rgba(0, 0, 0, 0.25) 0px 2px 4px 0px` (`variables.scss:368`). The recorded reason
was that iced cannot paint behind an `image` widget and that a canvas grown to hold
the blur wants 108 pixels across. Both halves are wrong. The precedent above is
right -- iced does composite a `container`'s `Shadow` inside its own coverage --
and the reach is measured rather than derived, and it is **three** pixels rather
than three sigma. Off `ref/user-ref.png` the profile header's card is a 96 box at
x=88..183, y=72..167, and the shadow reaches five rows below its last row, three
columns either side of its centre row and one row above its first: the offset circle
(the card, moved down two) grown by three, so a **102**-pixel canvas, not 108.

So it is drawn, from **five opaque pre-composited rings** one pixel of reach each,
darkest innermost, at depths **0.155, 0.116, 0.077, 0.039 and 0.018** -- the
middles of the intervals a byte can hold, which is what makes the five bands
reproduce the capture's own bytes exactly rather than nearly. Composited over
`--surface-1` rather than the card's own `--color-button-bg`, because `#34363c`
against `#16181c` is a little over twice the value and the deepest ring would read
`(13,14,17)` where the capture reads `(18,20,23)`. The card's pixels are copied
rather than redrawn, so the fill, the picture, the mask and the outline are not
touched by the shadow step at all.

Against the reference the bands match byte for byte on **2,574 of the 2,848** shadow
pixels at least a pixel clear of the card's antialiased rim, and the other **274**
differ by at most one unit in one channel. Isolated before/after binaries built from
the same tree and differing only in that file are identical on all five routes.

**Growing `Icon::circle`'s canvas instead is worse than drawing nothing,** which is
why the shadow is its own constructor. iced takes the layout box from the caller's
width and height and scales the texture into it, so a 102 canvas in the header's 96
slot draws the disc 90.35 across and lays the five bands down inside the card where
the reference has the picture -- **4,994** differing pixels on `/user/FlameFire` --
and shrinks `/hosting/manage`'s nine avatars from 24 and 36 to 19.2 and 28.8,
**4,328** more.

**The slot stops being the canvas.** Both halves of that matter: the reach is three
pixels *outside* the 96-pixel card, so a slot sized to the canvas reserves 102 and
pushes the card three pixels right -- measurably wrong, with the left-hand rings
vanishing into the page and the card landing at x=91 instead of 88. A container
would be worse, because it crops the reach. So the canvas is layered *over* a
transparent spacer of the card's own size at a negative offset, which is the one
shape that reserves 96 and draws 102; and `Stack` reports its base layer's size,
which is why the spacer is the base and the canvas the layer.

What is left is **forty pixels**: in the reserved gutter's own top eight rows this
reads `#15171A` where the reference reads `#14161A`, because the bands are opaque
and the reference's are a composite of the shadow over a gutter that is itself a
composite. One unit of blue on five columns of eight rows.

## A blend mode is arithmetic, and the first reading of it named the other mode

`ServerListEmpty.vue:26` puts `mix-blend-luminosity` on the feature plate's
texture, and the plate carried a note saying the blend had no expression here and
that what reached the picture was a measured share of the texture's own colour.
That was the wrong reason for the right arithmetic. **A separable blend mode is a
function of two colours**, so it can be computed once per theme and drawn as the
plain colour it works out to -- which is what the plate's precomputed 40x40 picture
already is.

**The mode is `SetLum(Cb, Lum(Cs)`** -- the *source's* luminosity carried onto the
*backdrop's* hue and saturation. The note had it the other way round, which is
`mix-blend-mode: color`, and the reference's own pixels settle which is which: the
texture is navy and the plate the reference draws is a saturated green, so the other
reading puts the plate fifteen steps too high in red and blue. Over the **840**
interior pixels of the three plates that carry neither the glyph nor the rounded
corner, the rms against `ref/hosting-clean3.png` is **1.20** for the mode and
**12.4** for the reading that keeps the source's hue.

**The colour space is sRGB, not linear light,** and that is measured rather than
assumed. The same 840 pixels read:

| reading | rms |
| --- | --- |
| spec weights 0.3/0.59/0.11 on sRGB | **1.20** |
| Rec. 709 weights on sRGB | 1.72 |
| spec weights, linearised | 3.27 |
| Rec. 709 weights, linearised | 4.78 |

A weight sweep puts the optimum on the spec's weights, so `LUMA` carries
0.3/0.59/0.11 and the arithmetic runs on the bytes.

**The blend is worth up to ten steps of green and blue** on the reference's own
pixels, which is not a rounding error: the plate cannot be a plain gradient.

Over the three plates, ours against the reference: **4,688 differing pixels become
4,289**, the mean channel error **3.12 becomes 2.17**, and of the pixels that still
differ the share within two steps goes from **25.5% to 66.5%**. The plate's pad
alone -- **1,296** pixels -- goes from mean 2.39 and rms 2.91 to mean **0.86** and
rms **1.34**, with the pixels differing by more than two falling from 45.5% to
**4.4%**. Every non-glyph row of all three plates is now within two per channel.

**The same measurement found a second, unrelated bug.**
`plate_overlay_pixels` was striding the 38-wide texture window by the plate's 40,
which walks two columns right per row and runs off the end of the window over the
last two rows -- so the texture was being drawn in the wrong place and then not at
all. The stride is `PADDING_BOX` now.

One number in that note was mislabelled and is worth naming because a mislabelled
number reads as a smaller gap than the real one: the "2.60" set beside the rms of
1.20 is not the rms of a plain source-over at the same alpha. It is the rms of the
share of the texture's own colour that this replaced, and a plain source-over at
the same alpha measures **7.57**. Three numbers, correctly named, beat two with one
of them mislabelled.

## A layer that swallowed the pointer, and a scroll_to that raced the command meant to recognise it

**Nothing inside the page pane took pointer input, on any page.** `Shell::pane`
wraps every page in `pages::overlay::Stack` for CSS-like absolute layering, and
`Stack` implemented `layout`, `draw` and `mouse_interaction` but not `on_event`. It
therefore inherited `iced_core-0.12.3/src/widget.rs:115-127`, which returns
`Status::Ignored` without descending, and `operate` was an empty stub for the same
reason. The port rendered correctly and could not be used: on `/user/FlameFire` all
four profile tabs stayed unpressable -- the plated tab stayed `All` in 4 of 4
frames -- hover never painted, and the wheel never moved the list, while the rail
worked because it is outside the stack. Only the rail.

Forwarding is **per layer and declared by the call site**, not "every layer to every
event": `Stack::over` is a picture and is not asked at all, and `Stack::over_control`
puts a live overlay back into the walk. The pane's own furniture -- the reserved
scrollbar gutter, the hosting toast, the inset shadow and the rule -- goes on with
`over` and takes no input, so an overlay cannot become a click shield over the page.
The one live overlay is the panel ad's *Upgrade to Modrinth Plus* link, which
`App.vue:2554-2564` puts at `absolute bottom-[250px] ... z-10` over the sidebar's
own column and which was unreachable by `mouse_interaction` as well. `on_event` walks
the layers last to first and stops at the first `Captured`; `mouse_interaction` walks
the same layers in the same order and takes the first answer that is not `Idle`,
because a hit test that disagrees with delivery is worse than either alone.
`operate` now calls `Operation::container` and descends, which is what lets a focus
or clipboard operation reach anything inside a page.

**The hosting preview stays inert, which is the reference's own reading.**
`ServerListEmptyPreview.vue` is `inert aria-hidden` -- a picture of the invite
dialog -- so the panel, its buttons, the friend rows, the invite link and the toast
beside it are drawn and none of them are hit targets. Clicking the picture of an
*Invite* button moves **352** pixels, all of them the pointer.

**`diff` was the other half of the same defect**, and it is fixed here because
delivering input makes it visible: it built a fresh `Tree` per layer and swapped
them in, so every layer's state was thrown away every frame, and a `mouse_area`
keeps `is_hovered` there. A control therefore lit up under the pointer and never
went out again -- the enter was published and the exit never was.
`tree.diff_children` is iced's own idiom and keeps the state.

Measured on `:95` at 1280x720: four profile tabs switch and switch back, hover
paints on and off a tab and a card, the wheel scrolls the project list, `+ New
server` raises its notice, the preview stays inert, the checklist opens the create
dialog and its chips, rows and version list all respond. Pixels against the
reference: `/hosting/manage` 434,931 -> **434,928**, `/user/FlameFire` 307,840 ->
**307,813**, `/browse/modpack` 412,765 -> **412,758**, `/skins` 388,703,
`/instance` 248,007 -> **248,004**. Against the pre-change captures the same routes
differ by **1,079 / 2,362 / 1,074 / 697 / 799** pixels, all of it one to three
levels on 1px borders from the `diff` fix.

**The second defect was a `scroll_to` still in flight reading as somebody else's
move.** `Region::adopt` told this policy's own number from an outsider's by
comparing the offset a wheel event measured against the offset the last `scroll_to`
carried, with half a pixel of tolerance. That comparison races the command it is
meant to recognise. A `scroll_to` is applied synchronously at the end of the same
`application.update` (`iced_winit-0.12.2/src/application.rs:851-886`), but a wheel
does not read the widget: `Wheel::offset` is `viewport.y - bounds.y` off the
**cached** layout, which iced_winit only rebuilds at the end of its `AboutToWait`
branch (`application.rs:543-551`). `Message::Tick` arrives as a winit `UserEvent`
(`application.rs:366-368`) and joins the message queue without going through the
layout, a wheel goes through `user_interface.update` first
(`application.rs:494-499`), and the queue is then drained in arrival order
(`application.rs:637-650`). So a batch arriving as `[Tick, Wheel]` runs the tick's
`scroll_to` and *then* hands `Glides::wheel` a measurement taken before it -- one
frame of glide behind, tens of pixels, past `SETTLED`, and indistinguishable from a
scrollbar drag.

`adopt` therefore resynced onto it, which resets `ScrollAnim::target` from where the
gesture was going back to a position the reader never asked for, and the frame
answered `scroll_to` with the stale offset as well, so the region jumped backwards.
Every notch accumulated since the gesture began was discarded, and how many survived
depended on how many ticks were interleaved. Three clicks at (500,400) on
`/user/FlameFire` measured **277, 435 and 177** pixels across three runs of the same
binary, five measured **180, 304 and 271**, and ten measured **261, 439 and 449** --
where the arithmetic is 360, 600 and 1,200. This was unreachable until the pointer
was forwarded through the stack, after which the wheel works and this is the next
thing showing.

**The command was never the problem.** `scroll_to` is an `AbsoluteOffset`
(`iced_widget-0.12.3`'s `operation::scrollable::scroll_to`, applied at
`State::scroll_to`) and the command that lands last is the only one the widget ever
sees, so there is nothing to make relative. The relative part is deliberate and
stays: a burst of wheel events is delivered in one batch against one cached layout,
so every event in the burst reports the same position, and accumulating on the target
is the only way two events travel two notches.

So `Region` now records **`swept`**, the range of offsets its own commands have put
the region at since it last measured it, and `adopt` skips a measurement that lies
inside it. `Region::send` is the only writer of `sent` and only ever *widens* the
range, which is what makes a repeated command mean the same thing twice -- re-issuing
the last `scroll_to`, or issuing a second while the first is in flight, can no longer
read as an outsider's move -- and it holds however many commands are outstanding
rather than guessing a depth. A debounce was rejected: it buys determinism with
latency and hides the cause rather than removing it. The cost, recorded on `adopt`,
is a drag landing inside the swept range, which is at most one frame of a glide wide
and exists only while a gesture runs.

The four tests are the deliverable, and they are the reason the bug is not in the
file now: `adopt` reverted to the single-number comparison fails three of them and
passes the fourth, which is the one that keeps a real drag adopted so the other
three cannot be satisfied by dropping the gate. With the range, ten clicks at
(500,400) land on the same pixel in three runs, and one, three, five, seven and ten
clicks land on 120, 360, 600 and the end of the content where the arithmetic puts
them.

## The strip's fourth link needed an address of its own

`/user/FlameFire` drew a strip of four links and made the fourth one a dead end. The
tab was drawn, the collections view under it was drawn, and pressing it changed page
state rather than the address -- so the tab could not be reached by typing and the
address could not be copied out of the shell. The cause was that `Route::User` had
nowhere to put it: its third segment is read through `ProjectType`,
`from_profile_token` refuses `collections` on purpose, and a `?` on that arm made
the whole address `None`.

**The reference has no such gap.** Its strip is four `href`s built by one template,
`layout.vue:779`:

```js
href: `${profilePath}/${projectType}s`,
```

and the string it is fed is `'collection'`, pushed into the same list the project
types go into at `layout.vue:762`:

```js
const types = catalogProjectTypes(projects.value)
if (collections.value.length > 0) types.push('collection')
```

so the tab's href is `/user/FlameFire/collections`, built exactly the way `mods` is.
The route that receives it is the one route for all four, `routes.js:66`
(`/user/:user/:projectType?`), with no pattern on the parameter, and
`parseProjectTypeRouteParam` reads both spellings and hands `'collection'` back
(`ui/src/utils/v3-projects.ts:81-83`). Two facts came out of that and they are the
whole shape of the change: **the segment is optional and *All* is its absent case**,
because `layout.vue:771-773` gives *All* an href with no third segment; and only the
plural is ever written.

So `route.rs` grows `ProfileTab { Projects(ProjectType), Collections }`, beside
`ServerTab`, `ProjectTab` and `InstanceTab`, which is what this codebase already
does with a page's tabs. **It is not a `ProjectType` variant, and that is not a
compromise:** a variant would have to be answered by `target_folder` (which folder
does a collection install into), by `sentence`, by `TABS` and by `ALL`, and the
reference answers none of those because it has no such concept either -- it keeps
`'collection'` as a string only its own filter reads. Pretending otherwise would
corrupt the derivation everywhere else. `Route::User.project_type` becomes
`Option<ProfileTab>` and `None` is *All* in `Route::User`, in `Message::Filter`, in
`Open::User` and in `pages::user::Filter::of` alike. The field keeps the name
`project_type` because `routes.js:66` spells the parameter that, and because
`shell.rs` was not that slice's to edit: giving `ProfileTab` the same
`profile_token()` method `ProjectType` has means `shell.rs:2634` writes
`/user/FlameFire/collections` unchanged.

**The page then got simpler rather than bigger.** `Message::Collections` and
`Message::LeaveCollections` are gone: both existed only because the branch had no
address, and `LeaveCollections` was the workaround for a specific trap -- the page
sat at `/user/{name}` while drawing the collections branch, so pressing *All*
produced the address it was already at and `Shell::go` returned early, leaving the
reader where they were trying to leave. With `/user/{name}/collections` in the
address that cannot happen, so all four tabs are one `Message::Filter` and one
`Open::User`. `State::filter` is the single writer of both `project_type` and
`collections`, so the drawn tab and the parsed tab cannot disagree.

The two assertions this fixes are updated, not deleted, because each was right about
project types and wrong only about the address. `route.rs:997` still asserts that no
`ProjectType` is handed back for `collections` -- that is the reason it never becomes
one, and nothing derived from a type has an arm to answer for. `route.rs:1016`
asserted that `Address::parse("/user/jelly/collections").is_none()`, which *was* the
dead end; it now asserts the route the address resolves to, and that the singular
`/user/x/collection` resolves to the same page.

Measured on `:117` at 1280x720. Clicking *Collections* at (411, 223) moves the
strip's plate from x=92..141 to x=354..462 and draws all four of FlameFire's
collections in the reference's own `updated`-descending order -- *Plugin* (1),
*Sodium* (6), *Masa* (13), *Carpet* (3) -- with no status line and no *Create a
collection* button, which is `canSeeCollectionStatus = isSelf || isStaffViewing` and
`v-if="isSelf"` with no Modrinth session, not omissions. **219,734** pixels change.
Clicking *All* back gives the project list and the plate back to x=92..141,
**219,565** pixels, and pressing *Collections* again is byte-identical to the first
press. **Launching straight at `--page /user/FlameFire/collections` renders the
collections view rather than falling back to Home**, which it did before because
`Shell::opening` parses the address and the parse was `None`.

Pixels, counted as `(|dR|+|dG|+|dB|) > 0`: `/user/FlameFire` 307,792 -> **307,727**,
`/hosting/manage` 434,529 -> **434,529**, `/browse/modpack` 412,823 -> **412,823**,
`/skins` 388,703 -> **388,703**, `/instance` 248,004 -> **248,004**. Four of the
five are bit-identical before and after, which is the answer in its strongest form
because `ui.rs` is shared by all five. The user's 406 are bounded to x=786..853
y=334..349 -- the first card's download line, 14.87M -> 14.88M, the reference's own
October capture reading 14.84M -- and outside that box the before and after frames
are 0 px apart, so the total against the reference went *down* by 65. Five captures
of the new build read 307,727 to the digit and differ from each other by 0, so the
build is deterministic and that band is the network.

## A card's summary gets the column the reference's own grid gives it

The second card's summary ended `—— A` where the reference ends `—— A skyblock`, and
the previous pass was right that the size is not the answer: the shared prefix's word
boundaries agree to the pixel, so both sides are set at the same size and the
difference is **where the `1fr` column stops**. It is derivable from the grid rather
than from the pixels.

`ProjectCard.vue:319-325` gives a card with an actions slot

```text
'icon info actions actions'
'icon info dummy   stats'
'icon tags  tags   stats'
```

over `auto 1fr auto auto` with `gap-x-3`, and `__actions` spans the third column, the
gutter and the fourth. Two spec rules finish it. **css-grid-1 §11.2:** gutters are
fixed-size tracks for the sizing algorithm, so the button spans *three* tracks and 12
of its width is the gutter's. **§11.5:** a spanning item's width is distributed
across the tracks it spans, "insofar as possible" -- and nothing else is in the third
column, the `dummy` area being named and never filled, so it takes the whole
shortfall. Tracks three and four together therefore owe the button's width less one
`gap-x-3`, which is **177** of the 834-pixel content box's 698 that is not the icon,
and the summary's column is **521**.

Two readings off `ref/user-ref.png` agree on that number and neither can be fitted
any other way. The button's left edge is **750** and it is flush right (`ml-auto`),
so the span is 939 - 750 = **189**, the button's own measured width, with no free
space for the margin to eat. And the wrap needs it: card two's first line measures
**520.9** pixels of advance from the column's left edge, so the column is at least
521 -- a margin of **0.1** -- and under 557.5 or *with* would have joined the line.

So the third `auto` track is not zero and the old two-`gap-x-3` room was wrong.
`stats` is now drawn as a row of [the third track, which nothing sizes, then the
stats] with the grid's own `gap-x-3`, and the head row keeps one gap beside it,
which makes the room `gap + max(button, gap + stats)` without a constant anywhere.
Card one is untouched by it -- its 94-pixel button is narrower than its 153-pixel
stats line, so the stats govern and the third track is nothing -- and its *Install*
ring stays at x=846..938 and its stats ink at x=787..938, both exactly where they
were.

**What is left is not in that file.** Our *Install to instance* measures **183** where
the reference's measures **189** -- the label's advance, 136.76 by Inter-600's
`hmtx` against the ~143 the reference paints, which is the letter-spacing question
the next-but-one section takes -- so cards two and three get **528** rather than 521,
and card three's first line now carries **one word more** than the reference's. Both
are named at the call site rather than papered over. This is the one open geometry
residual: card three's summary window is `[520.9, 523.9)` and we sit at 528,
downstream of the six-pixel shortfall in one button's label.

The empty sentences are the other half, and they are a weight rather than a size.
`EmptyState.vue:7` is `<span class="text-2xl font-semibold text-contrast">`, so the
weight is **600** and not the 800 `style::heading()` draws a real heading at; the two
call sites ask for `semibold()` now and the shared helper is untouched. Measured on
`--page /user/FlameFire/resourcepacks`, which is the only address that reaches the
empty state for an account this port can fetch: the ink rows are **282..304** either
way and the sentence is **297** pixels wide against **305**, with **2,366** ink pixels
against **2,881** -- the 7.6 pixels Inter's own advances predict for 800 against 600
at twenty-four.

## The toast was over the gutter, not under it

The hosting page's invite toast measured x=649..969 where the reference measures
x=649..984 with the fill visible to x=979, and **the cause was paint order rather
than geometry**. `ServerListEmptyPreview.vue` holds the toast as a *sibling* of the
`overflow-hidden` panel inside its own 400-wide `relative` root, so the panel never
clipped it and it composited above everything the pane draws. Here it was a layer of
the page, and the pane's reserved scrollbar gutter is a *sibling* of the whole page
-- a ten-pixel strip at x=970..979 laid over it to hide iced's own bar -- so the
strip covered the toast's last ten columns.

Nothing inside a page can be over a layer of the pane, so the page now hands the
layer back: `servers::page_overlay` returns the toast and the place in the page the
reference puts it, and `Shell::page_overlay` puts it into the pane's own stack
between the gutter and the inset shadow. That order is the reference's own:
`.app-contents::before` is `z-index: 30` (`App.vue:2725`) and outranks the toast's
`z-10`, while the page's own in-flow content and the sidebar's `::before` are
outranked by it.

**Keeping the clip took a widget.** The toast's box runs four columns past the pane,
and the clip that used to stop it was iced's `Scrollable` wrapping its content in a
layer at its own bounds (`scrollable.rs:909-918`) -- which the hoist leaves behind.
`Container::clip` is not the answer: it only narrows the `viewport` it hands its
content, and a `container`'s background never reads it. So there is a `Clipped`, a
scissor and nothing else, whose box is stated in the layer's own coordinates because
the caller is what moved the layer. Both halves of that are written down, because
each looked right and clipped nothing for a build: the stack's own bounds are the
window's rather than the page column's, since its base layer is the row holding the
page *and* the panel; and an element the stack has already moved to x=649 reports
649 as its own origin, not the pane's 64.

The two answers that do not work are now at `pane_gutter` rather than in the next
reader's experiments, and a control build settles them: with the strip deleted and
nothing else changed, iced's `#757C84` bar shows down x=970..979 on **four of five**
routes -- **5,078** px over 512 rows on `/hosting/manage`, **2,988** over 299 on
`/user/FlameFire`, **2,518** over 252 on `/skins` and **1,088** over 110 on
`/browse/modpack` -- and on none of them with the strip in place. `/instance` shows
none either way because that page does not scroll. Every one of the **1,098** pixels
the hoist moves is at x=970..979, y=440..551: the toast's own rows and the ten
columns the strip used to own. The other four routes are byte-identical before and
after.

Measured at 1280x720 against the captures the tree already measures against: the
fill now runs 650..979 (contiguous, strict `#1D1F23`), the pane's rule is still
`#42444A` at (868,48), x=90..954 is still `#16181C` for all 865 columns, and the
hosting page's whole-image difference falls from **435,982 to 434,931**.

**What it costs is real and is recorded rather than hidden:** the toast no longer
glides with the page. It is a layer of the pane now, outside the page's scroll
region, and `Glides::anim` -- the only accessor for a region's offset -- was
`#[cfg(test)]` in `scroll.rs`, so `pane()` could not translate it. This page scrolls
48px in all and the toast sits 392px below the pane's top, so it cannot leave the
pane; it is up to **48px** out of place at the bottom of the scroll. That is fixed
later and in the right place -- `Glides` is a field on `Shell`, the view runs on
`&self`, and `ScrollAnim::offset` is already `pub`, so `Glides::offset` answers it
and `Shell::page_overlay` subtracts it -- after which the toast's border rows read
392..503 at the bottom of the scroll where they read 440..551 before, which is what
the reference draws.

## A taller line box lands the baseline lower, not the same

An audit of the profile page found two text widgets whose line height was never set,
so both fell to iced's default `Relative(1.3)` -- **20.8** pixels at sixteen -- while
the reference sets an absolute one per class. The metadata row's words are
`page-header-metadata-item.vue:79`'s `leading-none`, which is `line-height: 1`, so
the line is the label's own sixteen; the platform tag's label is `TagItem.vue:19`'s
`leading-none text-sm` at fourteen, which the kit's own 24-pixel pill settles on its
own -- 1 + py-1(4) + line + py-1(4) + 1 = 24 leaves line **14** -- and the same
utility order that puts a `text-sm leading-none` on 14 puts
`page-header/index.vue:13`'s `text-2xl leading-none` h1 on **24** rather than 32.

Sweeping the other thirteen sites against their own sources moved six more numbers,
all of them unsourced: both empty sentences are `EmptyState.vue:9`'s
`text-2xl font-semibold` heading and not a sixteen-pixel card line, and the four
lines of a collection card that are not its name carry no size class at all --
`layout.vue:281`, `:287`, `:290` and `:299` name none, and the `text-primary` on the
description is this preset's alias for a colour, not a scale -- so they inherit the
body size rather than the `text-sm` the old comment claimed.

**The card summary was left alone, and three readings refute `text-sm` there.**
`ProjectCard.vue:402-404`'s `@apply text-sm` sits inside the `@container (width <
550px)` block and the card measures **868**; Inter's em dash is **1.0000 em** of
advance and of ink, and card two's summary ends in a pair of them across thirty-two
solid pixels; the summary's `6` and `ProjectCardStats`'s `6`, which carries no size
class and so is known to be sixteen, are both **twelve** rows of ink; and those two
digits sit exactly **eighteen** rows apart with identical profiles.

**Two of the three changes move no ink at all, and the third is a half-pixel.** Both
unsited lines were inside a box that centres its content, and a centred box puts
the baseline at `top + H/2 + (A - D) * fs / 2` whatever the line is. The metadata
row is not centred on its own, but it is the header column's last block and the
column is centred against the avatar, so the 0.8 the row gave back moved the column
-- which is why the header summary's line went from 18 to the stylesheet's **18.4**.
That is the CSS line rather than the eighteen those pixels paint to, and the table
in its doc comment is what chose it: of the three values the column can take, 18.4
is the one that leaves the title on the reference's rows, and the summary's single
row is what pays.

Whole page against `ref/user-ref.png`: **308,427** differing pixels before,
**308,222** after. The metadata band is 320 of that; nothing else moved by more than
antialiasing.

**The same rule is why the friend names were a pixel high.** `ServerListEmptyPreview
.vue:54` gives a friend's name `truncate text-base font-medium text-primary` and
nothing else, so it is 16 pixels on Tailwind's 24-pixel line. This drew the 16 and
left iced's own default for the face, `LineHeight::Relative(1.3)`, which is 20.8. It
was expected to be invisible: the avatar beside the name is `size="1.5rem"` and the
group is `items-center`, so a taller line box in the same centred space ought to
split its extra leading evenly. **It does not, because iced puts the baseline
`line_height - descent` below the top of the line box rather than centring the
leading in it**, so the taller box lands the baseline half the difference *lower*.
All eight names were a pixel above the reference's, and a pixel above it in the same
direction every time:

| name | reference | before | after |
| --- | --- | --- | --- |
| Josh | 265.467 | 264.458 | 265.456 |
| Prospector | 309.620 | 308.632 | 309.630 |
| Fetch | 353.010 | 351.997 | 352.995 |
| IMB11 | 396.406 | 395.381 | 396.381 |
| Truman | 441.323 | 440.237 | 441.238 |
| Boris | 485.199 | 484.177 | 485.184 |
| Saya | 529.795 | 528.594 | 529.606 |
| Michael | 572.437 | 571.267 | 572.248 |

`Josh`'s own difference sum against `ref/hosting-clean3.png` halves, from **16,490 to
7,860**, and its max channel delta falls from **159 to 82**. The whole capture moves
**2,271** pixels, all of them inside the eight friend rows.

**The 14-pixel URL under the same rule keeps iced's default, and says why in the
comment:** its row is `h-8 items-center` with nothing taller in it, so nothing
cancels there, and stating `text-sm`'s own 20 moves it to y=647.045 against the
reference's 646.095 where the default sits at 646.132. That is the finding stated as
a rule rather than as a fix: the baseline rule is not a bug, it is how iced lays a
line out, and a line height is a decision that has to be read off the element that
centres it.

## Two icon sizes, and one bare element rule

`ui::CONTROL_ICON` is `size-5` and has to stay **twenty**: `ui::search`'s leading
glyph is `size-5` on both the wrapper and the icon (`Input.vue:12`), and the install
checklist's undone mark is `size-5 shrink-0`
(`onboarding-checklist/index.vue:120`). The profile page's collection card is
different. `layout.vue:282` and `:292` draw `LibraryIcon` and `BoxIcon` with **no
class at all**, so they fall to the bare `svg{width:1em;height:1em}` element rule in
the reference's shipped stylesheet, which the bundle in `/usr/bin/ModrinthApp`
carries unshed of specificity between `.iconified-input svg` and `.chart svg`, and
one em there is the **sixteen** pixels `defaults.scss:17`'s `body` asks for.

The four status-line glyphs are the same case and are the reason the rule is a
constant rather than a comment. `layout.vue:301`, `:305`, `:309` and `:313` draw
`GlobeIcon`, `LinkIcon`, `LockIcon` and `XIcon` unclassed, each in a
`flex items-center gap-1`, and **nothing above them sets a font size either**:
`ProjectCardList`, `SmartClickable`, the card's own div and its
`grid-cols-[auto_1fr]` name none, `text-primary` is a colour
(`tailwind-preset.ts:19-20`), and the only size on the card is the `<h2>`'s
`text-lg`, which is a **sibling** of the icon line rather than an ancestor. So they
are neither `size-4` nor `size-5` by intent. `ui::BARE_ICON` is that number, and
`CONTROL_ICON`'s doc now carries all three use sites, because the shared number is
right and only the third caller was wrong.

**That change could not be photographed, and the reason is worth having.** In the
tree it was written against, the Collections view could not be reached at all:
`pages/overlay.rs`'s `Stack` implemented `Widget` without an `on_event`, so it
inherited `iced_core`'s default of `Status::Ignored` and never descended, and
`Shell::pane` wraps every page in one. Hover, press and the wheel all failed over the
page and all worked in the rail, its sibling in `Shell::render`. So the number was
read off the reference's stylesheet and off the six lines that consume it.

**The tab labels are a weight, and the weight is 700.** The strip on the profile page
is `tabs_with_glyphs`, fed by `TAB_LABEL` and `TAB_LINE` -- not by
`NAV_LABEL_SIZE`, which drives the sidebar. So the sixteen-pixel label an audit was
comparing against `Tabs.vue`'s `text-sm` was never in this path: the profile page
renders `NavTabs`, not `Tabs`, and `NavTabs.vue:9` puts `text-xs sm:text-sm
font-bold` on the `<nav>` that the label inherits from, which at a 1280-wide window is
**fourteen** pixels at weight **700**.

The size was already right. Both captures put the label on eleven ink rows, and the
strip's own **forty-six** pixels -- `2 + 2.75 * 16` -- pin the root `rem` at sixteen,
so `text-sm`'s `.875rem` is fourteen here. The weight was not: the label was drawn in
`heading()`, which is eight hundred.

**The discriminator is the `ll` of *Collections*,** which Inter draws as a bare
vertical stem, so its width reads the weight with nothing else in the way: **2.22px**
in the reference against **2.52px** here, where Inter 700 measures **2.118** and
Inter 800 measures **2.431** at fourteen pixels, and both renderers draw the stem
about a tenth of a pixel fatter than the outline. The ink run counts say the same
thing: eight hundred's stems touch their neighbours and merge *Data Packs* into three
blobs where seven hundred keeps nine letters apart. So ink runs go **3/5/8 to
5/6/9** against the reference's **9/8/11**, the stem goes 2.52 to **2.16** against
2.22, and ink mass on *Data Packs* goes **431 to 393** against the reference's **396**.

**The strip's width gets worse and is left alone on purpose.** The reference's
advances sum to **261**, where Inter 700 at fourteen gives **243.9** -- seventeen
pixels of advance that no fourteen-pixel Inter weight can produce, while this port's
own build tracks the font to within its ink-edge quantisation (246 against Inter
800's 247.60, and 242 against Inter 700's 243.86). What is left there moves glyph
origins without touching outlines, which is what space looks like, and it is local to
the strip: the profile bio measures 174 against 173. That is the finding the next
section takes up, and this is the slice that ruled out size and weight as the cause.

`pages/user.rs` also draws the empty state in `heading()`, where `EmptyState.vue:9`
is `text-2xl font-semibold`. Verified, and not fixed there. Worth recording: that
same empty state is an unframed `flex flex-col items-center` column with a
`h-[200px]` illustration, where ours is a `ui::card` with no illustration.

## One loader tag's message is spelled without the hyphen it is looked up with

`is_loader_tag` asks whether `tag.loader.<tag>` exists, and it answers false for
**`bta-fabric`** -- because `tag-messages.ts:10-11` publishes that tag under the id
`tag.loader.bta-babric`, spelled without the hyphen. So one answer cost that tag its
glyph, its *BTA (Babric)* label and its `--color-platform-bta-fabric` ink all at
once, and the reference draws all three.

The exception is named where the lookup is rather than worked around at the call
site, because the oddity is upstream's and there is exactly one of it: the **thirty**
loader tags all take the tag's own name as the message's prefix bar one. The test
that pinned the misspelling as expected is turned around to pin the fix, so a change
that breaks it again says so.

## A test binary never runs `run_shell`, so it never had the shipped faces in it

The tracking test passed on this box and failed on the Windows runner: *Data Packs*
measured **83.6850** here and **77.8054** there, a difference of **5.87**. The fit was
being applied correctly in both -- the three asserts ahead of it pass on the runner, so
`advance()` and `shape_width()` agree with each other there -- and the width differed
underneath them.

**A test binary never runs `run_shell`,** so iced's global font system inside one holds
only **this machine's installed faces**. This machine has Inter installed. A Windows
runner does not. So the constants in `TRACKED` were pinned to the host rather than to
the five faces in `crate::FONTS`, and the reference column beside them had been given
the same overhang so that the two agreed on this host by construction: **83.15 + 0.530 =
83.680**, 74.27 - 0.618 = 73.652, 83.42 - 0.978 = 82.442, 90.0 + 2.136 = 92.136. A
test built on that pair passes on any machine with Inter installed and fails everywhere
else, which is the whole definition of a gate that measures the machine.

**That also corrects a number this file's own code had got wrong.** The per-label
"basis" between a reference advance and `shape_width` was recorded as **+0.530, -0.618,
-0.978 and +2.136**, with *New server*'s 2.136 explained away as *a trailing `r`
overhangs its advance*. None of it was the glyph. On the shipped faces `shape_width`
matches the `hmtx` sum to **-0.206, +0.001, -0.004 and -0.070** -- a fifth of a pixel,
which is all an ink extent and an advance sum ever differ by. The old four were the
**ambient** face's overhang mistaken for the glyph's, and the explanation was invented
on top of the largest of them rather than read off anything.

`shape_width` now has a `shape_width_in` variant that takes the system to shape
through, and the test builds one out of `crate::FONTS` the way the CJK shaping test
already did. Production keeps the window's system, which `run_shell` fills with those
same five faces, so nothing about a drawn label changed -- only the measurement of it.
The test also holds the shaped width against its own `hmtx` sum to a quarter of a pixel,
which is the assertion that would catch a future table measured off a machine rather
than off the shipped faces.

| label | shaped, shipped faces | reference's own advance | off by | `hmtx` | shaped - `hmtx` |
| --- | --- | --- | --- | --- | --- |
| *Data Packs* | 82.9497 | 83.15 | 0.2003 | 76.54 | -0.206 |
| *Modpacks* | 74.2664 | 74.27 | 0.0036 | 71.50 | +0.001 |
| *Collections* | 83.4218 | 83.42 | 0.0018 | 77.74 | -0.004 |
| *New server* | 89.9152 | 90.0 | 0.0848 | 88.36 | -0.070 |

The four extras are the same four the previous section fitted; the check is now against
the reference's own advances read off its glyph origins rather than against those
advances restated on this crate's ambient basis. **Within 0.21 everywhere, and *Modpacks*
and *Collections* within 0.005.**

**What this does not say.** These four widths are `shape_width` through a font system,
not pixels off a screen. The four reference advances were read off `ref/user-ref.png`
and `ref/hosting-clean3.png` while the reference could still be started, so the
right-hand column is a recorded measurement and the left-hand one is reproducible here
today. That asymmetry is the only kind of evidence left for this page.

## The card's button was six pixels short, and a fill column had been paying for it

The last open defect on the two pages. *Install to instance* on project cards two and
three measured **183** where the reference's measures **189**, and because a card's
summary column is `Length::Fill` the shortfall did not stay in the button: **the column
absorbed it** and came out **528** where the grid's own arithmetic derives **521** (the
section above). Card three's summary therefore carried one word more than the
reference's, at x=216..738 against its 217..711, and its window is `[520.9, 523.9)`.
Cards two and three compensated with `521 + 6 - 1`, the last pixel being this card's own
content box being one wider than the reference's -- a constant naming a bug rather than a
measurement.

**The fix is the reference's own box arithmetic, and nothing is named at the call site.**
`ButtonFrame.vue:31`'s `md` row is
`h-9 gap-1.5 rounded-xl px-2.5 text-base font-semibold leading-5 [&>svg]:size-5`, so the
chrome is `px-2.5` twice over (**20**) plus `gap-1.5` (**6**) plus `size-5` (**20**) =
**46**. The reference's box is **189**, so the label is left **143.0** where Inter-600's
`hmtx` sums to **136.76**: **6.24** over **eighteen** gaps, or **0.3467** a character. The
container sizes itself to whatever `tracked_text` returns, so the button is 189 without a
width being written down.

**That extra is derived, not fitted, and it is recorded as what it is.** The four tab and
tag labels were fitted from the reference's measured glyph origins and then
cross-validated against arithmetic that shares nothing with the fitter -- *New server*
fits **+0.1806** where its own 150.0-pixel box asks for **0.1818**. This one could not
be: **the reference's measured box width and the stylesheet's own chrome** are all that
is left of it, and there are no glyph origins left to check the result against.

| label | how the extra was got | extra/gap | at the box | `hmtx` | check |
| --- | --- | --- | --- | --- | --- |
| *New server* | fitted from glyph origins | +0.1806 | 89.9152 against 90.0 | 88.36 | its box asks 0.1818 |
| *Install to instance* | **derived from its box and `md`'s chrome** | **+0.3467** | 143.0 label in a 189 box | 136.76 | **none: no origins** |

**And it is nearly twice *New server*'s at the same size and weight** -- 0.3467 against
0.1806, both Inter-600 at sixteen because `md` and `lg` are both `text-base` -- which is
the same finding as everywhere else on these pages, and the reason `TRACKED` is a table
rather than a constant: the reference's extra runs from +0.18 to +0.38 across the button
labels alone at one size and one weight.

Measured after: both buttons **189** at x=750..938; card three's first line **217..709**
against the reference's **217..711**, card two's **217..736** against **217..737**. The
`521 + 6 - 1` compensation is gone and the column is the grid's own **521**.

**Every figure here is either arithmetic from the reference's own stylesheet or an ink
box recorded while the reference could still be started.** The 189 and the 217..711 were
measured against a capture that no longer exists, and the 189 is now an *input* to the
derivation rather than an output of it -- so a reader who doubts it has nothing here to
re-measure with, and that is worth more to say than a sixth entry in `TRACKED` would be.

## What none of this can be checked against any more

**The reference capture is gone, and so is the reference.** `/tmp/ref/` holds one
file, `launch.log`, and `/usr/bin/ModrinthApp` is still on disk but panics at
startup:

```text
thread 'main' panicked at tao-0.36.0/src/platform_impl/linux/event_loop.rs:217:53:
Failed to initialize gtk backend!: BoolError { message: "Failed to initialize GTK",
  filename: "gtk-0.18.2/src/rt.rs", function: "gtk::rt::init", line: 141 }
```

So **every whole-image differing-pixel count in the sections above is the last
measurement taken while that app was alive.** They are not re-checkable and nothing
in this tree can make them so. The numbers as each commit last recorded them, counted
as `(|dR|+|dG|+|dB|) > 0`: `/user/FlameFire` against `ref/user-ref.png` **307,727**,
`/hosting/manage` against `ref/hosting-clean3.png` **434,929**, `/browse/modpack`
**412,823**, `/skins` **388,703**, `/instance` **248,004**. Where a commit's own
report gives a number -- a difference sum, an rms, an ink box, a plate's interior --
that number is the record, and it is the one to argue with.

**One geometry residual is open and is written down as such.** The fit residuals on the
fitted labels are +-1px of ink -- *Data Packs* **83** against **82**, *Modpacks* **74**
against **74**, *Collections* **82** against **83** -- and they are still open. Card
three's summary is closed: it used to carry one word more than the reference's because
*Install to instance* measured 183 against 189 and the fill column absorbed the six, and
both buttons are 189 now with the column the grid derives. Neither is claimed closed
beyond what its own section says.

**Nothing measured after `cec5440` is a fresh whole-image diff against the reference.**
That commit is where the reference's own process on `:99` and `/tmp/ref/` died, and it
does not start again. The two slices that followed it are measured against values
recorded while it was alive -- the label fits against the reference's own advances off
`ref/user-ref.png` and `ref/hosting-clean3.png`, and the card button against *Install to
instance* at 189 and boxes at 217..711 and 217..737 -- or derived from its own source, as
`ButtonFrame.vue`'s `md` chrome is. The shaped widths are this tree's own
`shape_width()` through the five faces in `crate::FONTS`, which is reproducible here today;
**no figure from either slice is a whole-image differing-pixel count at all**, so neither
is the receipt that kind of number is.

**And CI is not red.** This section used to say it was, at `da746ab`, and that is the
record of what the red run was rather than of the gate's state now. The failure was
`ui::tests::every_measured_label_carries_the_extra_its_own_capture_gave_it` at `ui.rs:3308`:

```text
`Data Packs` measures 77.8054 and the reference's own capture asks for 83.680
test result: FAILED. 821 passed; 1 failed; 0 ignored
```

**The cause was the font, and the fit was never wrong:** a test binary never runs
`run_shell`, so the global font system the fit was measured through held this machine's
installed faces, which include Inter and which a Windows runner does not have. `1f393ab`
measures through `crate::FONTS` instead, the runner is green on the same test since
(`37199382822`, 1314 passed / 0 failed and 19 ignored), and this machine is too (822
passed / 0 failed in the desktop crate). Nothing was amended or force-pushed to get
there, and the red run stays in G156's `EXPECT` because it is what explains the change.
