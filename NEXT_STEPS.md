# NEXT_STEPS — deferred work

Written 2026-09-13 16:55, while the machine was busy; updated 17:40 after a
compile.

**Steps 1 and 2 have now been run, and one headline claim did not survive
contact.** The compile is green — 463 tests, 0 failures, 0 warnings — and the
artifact is current. What did *not* hold up is the size of the title-bar win: the
message it removes costs ~16 us per move, not a rebuild storm. See §8 for the
measurement and what it does and does not prove.

---

## 1. Run this first — DONE 17:34

Code was deleted by hand, so the compiler is the only thing that can confirm the
tree is still whole:

```
cargo test --workspace --all-targets --locked
```

Result: **exit 0. 463 tests, 0 failures** (4 + 150 + 8 + 200 + 6 + 26 + 69 across
the seven targets), **0 warnings**, finished in 5.3 s of test time. `--locked`
was included on purpose: the hand-trimmed `Cargo.lock` resolved without cargo
needing to touch it, which is what the CI's `--locked` builds depend on.

One real failure was found and fixed, and it was worth having: see §8.1.

What the compiler was checking:

- `crates/prism-gui` lost its `backend.rs` module plus the `Page`/`Route` types,
  the `Error::Json` variant, and its `serde`/`serde_json` dependencies. Its
  `lib.rs` and `model.rs` were rewritten around the leftovers.
- `crates/prism-desktop/src/theme.rs` lost `ColorTheme::is_active`.
- `Cargo.toml`/`Cargo.lock` lost unused dependencies (see §7).

Two checks *were* possible without a compiler and both pass, so the risk is
narrower than "hand-edited Rust":

- every workspace manifest's dependency edges match `Cargo.lock` exactly, so the
  `--locked` builds in CI will not trip over the dependency trims;
- every hand-edited `.rs` file is brace-balanced (string/comment aware) and no
  remaining source file references anything that was removed.

The per-module filters that were listed here are now subsumed: the workspace run
covers every one of them, and `gpu::`, `theme::`, `prefs::`, `scroll::` and the
app's theme tests all passed inside it.

## 2. Then rebuild the artifact — DONE 17:34

```
cargo build --release --locked
cp target/release/PalantirMC.exe dist/
```

Result: **exit 0, finished in 5m05s** (`lto = true` with `codegen-units = 1`
means the final link is most of that). `dist/PalantirMC.exe` is now

| | |
|---|---|
| bytes | 8,132,608 |
| sha256 | `df716b8de8837f74e9cda808ed52ceb3a6a3ec94499c62b34faa7479688d8cf6` |
| built | 2026-09-13 17:34 |

The 16:27 artifact it replaces was a working launcher of the revision *before*
the cleanup and the resource pass. Same byte count, different content — so a
size comparison proves nothing here; the hash is the identity.

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

## 4. Known gaps (missing features, not regressions)

- **Microsoft sign-in.** Not implemented; offline accounts only. The About page
  says so out loud.
- **Assets and libraries are not downloaded at launch.** Launch resolves the
  pack, probes Java and streams output; a fresh instance reports what is missing
  instead of failing silently.
- **Modpack import copies overrides only** — the remote files a `.mrpack` lists
  are not fetched.
- **Per-frame cost while interacting is not addressed.** iced 0.12 repaints the
  whole window per input event and exposes no partial redraw, so a stream of
  mouse events still costs tens of percent of a core on an integrated-GPU
  laptop. Idle is 0%. The only real fix is not reachable from the application
  side.
- **Renderer probe edge cases.** `gpu.rs` pins `ICED_BACKEND=tiny-skia` only when
  no hardware Direct3D 12 or Vulkan adapter is present, and never overrides an
  explicit `ICED_BACKEND`. `ICED_BACKEND` set to a *name iced does not know*
  (e.g. `software`) is still honoured verbatim and lands in iced's fallback,
  which is intentional but is the one path where `gpu::active_backend()`'s
  report can disagree with what iced actually built.

## 5. Delivery: wired up and green

The repository is now `MSedgeMC/PalantirMC` (private), and CI runs on every
push to `master`. All four jobs pass. The Release workflow is registered but has
never run, because that needs a `v*` tag -- see §5.1 for what that would test.

| File | What it does |
|---|---|
| `workflows/ci.yml` | `test` (workspace, `--all-targets --locked`), `lint` (clippy at deny-by-default lints; rustfmt advisory), `package` (both Windows targets, size + hash guard, DLL-dependency gate, artifact upload) |
| `workflows/release.yml` | Tag `v*` → `guard` (tag must equal the workspace version) → `build` (tests, both targets, DLL gate) → `publish` (`gh release create --verify-tag` with both exes, regenerated `.sha256` sidecars, zips) |
| `scripts/check_exe.py` | Fails a package build whose PE import table names a non-Windows runtime |
| `dependabot.yml` | Weekly grouped cargo + actions updates |

### Why the DLL gate exists, and what it proved

`check_exe.py` parses the PE import table and fails if it names the GNU/MSVC
runtime (`libgcc_s_*`, `libwinpthread-1`, `libstdc++-6`, `vcruntime140`, …).
Ran against the 17:34 build it reports **27 DLLs, all of them Windows' own** —
in particular no `libwinpthread-1.dll` and no `libgcc_s_seh-1.dll`, so the GNU
build is self-contained and needs no `+crt-static`. The negative control (the
same exe with `d3dcompiler_47.dll` renamed in place to `libgcc_s_dw2-1.dll`,
same byte length, which is the exact failure mode) exits 1 with the hint
attached. So the gate can fail, which is the only reason it is worth having.

### What the first CI runs actually found (14:08–14:41)

All four jobs are green as of `3dee1fc`. Getting there took three runs, and the
failures were real rather than configuration:

| Run | Result |
|---|---|
| `eeca48d` | Test ✅ · Lint ❌ 169 `unwrap`/`expect` errors · gnu ✅ · msvc ❌ `VCRUNTIME140.dll` |
| `7b29170` | Test ✅ · Lint ❌ 16 in `prism-gui` · gnu ✅ · msvc ✅ |
| `3dee1fc` | **all four ✅** |

**The lint job found a genuine bug, twice.** `prism-core` and `prism-gui` deny
`unwrap_used`/`expect_used` crate-wide while their own doc comment states the
rule as "no `unwrap`/`expect` *outside tests*", and `prism-loader` already
carries the `cfg_attr(test, allow(...))` line that makes the two agree. Neither
crate had ever been run through clippy (the component is not installed for the
local toolchain), so 153 and 16 errors respectively had been sitting there. The
first fix gave the `allow` *before* the `deny` in `prism-gui`; inner attributes
apply in sequence, so the deny still won -- the second run is what showed the
order mattered.

**The DLL gate found a portability difference between the two targets.** The
MSVC exe imported `VCRUNTIME140.dll`, which arrives with Visual Studio, Office
or a game rather than with Windows, so it was the *less* portable of the two
builds while looking like the more standard one. Both targets now build with
`-C target-feature=+crt-static` for MSVC only, and both pass the gate. Verified
on CI, not just locally: the GNU exe needs 27 DLLs and every one ships with the
OS.

**What did not break** is worth recording, because it was the risk: the test
suite passes on a clean Windows runner from an empty registry with `--locked`,
so the hand-trimmed `Cargo.lock` is genuinely consistent and no test was
silently leaning on this machine's Prism install at `E:/Games/PrismLauncher`.
The GNU job's first-ever mingw install via `choco` also worked.

The lint job's second step (`cargo fmt --check`) is still advisory and will
report a large diff whenever it is first run for real.

### Registration quirk worth knowing

`release.yml` did **not** register on the first push -- no run, no listing, and
`GET /actions/workflows/release.yml` returned 404. The file was not at fault:
the identical bytes with only the `name:` changed registered fine in a throwaway
repo, and a later push to this one registered it without any change. So a
workflow that never appears in the Actions tab may simply have failed to
register, and the fix is to push again rather than to edit the file.

### 5.1 Two things still owed

**The release path has never run.** Its `guard` job's PowerShell was verified
locally against this `Cargo.toml` (`v0.1.0` passes and extracts `0.1.0`;
`v0.2.0` throws; `0.1.0` throws), but `publish` has only been read, not
executed -- the file renaming, the regenerated `.sha256` sidecars and
`gh release create --verify-tag` are all untested. A single `v0.1.0` tag
exercises all three at once, and also publishes, so it wants to be deliberate.

**A diagnostic repo is still standing.** `MSedgeMC/actions-probe` was created
while isolating the `release.yml` registration failure and could not be deleted
-- the `gh` token has no `delete_repo` scope. It is private and empty of
anything but the probe files; delete it from the repository settings, or after
`gh auth refresh -h github.com -s delete_repo`.

## 6. Cleanup performed in this pass

Deleted, because nothing referenced them and they would otherwise sit in the
repo forever:

- `crates/prism-core/examples/dbg_ini.rs` — a scratch debug harness with
  hardcoded `println!`s, superseded by `ini.rs`'s own unit tests.
- `crates/prism-core/examples/prism-cli.rs` — a strict subset of
  `crates/prism-cli/src/main.rs` (4 subcommands against its 8). The example had
  been stale since phase 2.
- `build.log`, `build_dbg.log` — leftover build output, already matched by
  `*.log`.
- `.unlazy/` — an empty scratch directory from a working ledger, now also
  ignored in `.gitignore`.
- `prism-gui`'s `backend.rs`: a `GuiBackend` trait, an `App` driver and a
  `HeadlessBackend` written for a frontend that had not been chosen yet. iced is
  the frontend and drives the models directly, so this was a second, unreachable
  path. Its `Page`/`Route` navigation types duplicated the real `Page` enum in
  `app.rs`, with fewer pages.
- Unused dependencies: `anyhow` in `prism-desktop`, `anyhow` in `prism-core`'s
  dev-dependencies (it existed for the deleted examples), and `serde` +
  `serde_json` in `prism-gui` (only the removed `AppSnapshot` JSON round-trip
  used them). `Cargo.lock` was updated to match in the same pass.
- `theme.rs`'s `ColorTheme::is_active` — defined, never called.

Kept deliberately:

- `assets/brand/palantirmc.png` (699 KB) — not embedded in the binary, and it is
  the only master of the logo. `logo512.png` and `icon256.png` are crops of it.
- `target/` — deleting it would turn the next build into a full dependency
  rebuild, which is the opposite of what a busy machine wants.

## 7. Verification state carried over

From the working ledger that was folded into this file (it recorded a hash-bound
evidence fingerprint per gate; only the substance survives):

| gate | subject | state |
|---|---|---|
| G1–G3 | desktop typecheck, desktop tests, workspace tests | met (before this pass) |
| G4 | GitHub Actions remains the release-build path | met |
| G6 | explicit `ICED_BACKEND` is honoured, not clobbered | met |
| G7 | renderer default justified by measured resource use | met |
| G5 | native window behaviour and scrolling reviewed on the running binary | met, pre-cleanup |
| G8 | renderer chosen from what the machine offers | **met 17:34** (`gpu::` 11/11), and re-confirmed live 17:40: this machine resolves to `tiny-skia` |
| G9 | colour themes differ, System follows the OS, ids round-trip | **met 17:34** (`theme::`) |
| G10 | launcher prefs stored apart from Prism's config, survive a damaged file | **met 17:34** (`prefs::` 5/5) |
| G11 | wheel eases to a target, clamps, adopts a dragged scrollbar, idles frame-free | **met 17:34** (`scroll::`); the *idle* half independently measured at 0.0 ms/1.5 s (§8) |
| G12 | choosing a theme applies it and records it | **met 17:34** (`app::`) |
| G13 | the shell is genuinely idle when untouched | **met 17:34**, 0.0 ms CPU over 1.5 s on both revisions |
| G14 | the title-bar gate removes the per-move message cost | **met, but the claim is scaled down** — 31.2 -> 15.6 us per move, not a rebuild storm (§8) |

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
The bar was wrong, and the artwork is why. `logo512.png` is a soft radial glow,
not a solid disc — measured directly, **exactly 2 of its 262,144 pixels reach
alpha 255 and only 102 clear 240**. Averaging that spike into a 2x2 window is
*expected* to land just under it, so a fixed threshold there measures where the
brightest pixel fell on the sampling grid, not whether the shrink preserved the
mark.

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
