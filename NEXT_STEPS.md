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

- `crates/palantir-gui` lost its `backend.rs` module plus the `Page`/`Route` types,
  the `Error::Json` variant, and its `serde`/`serde_json` dependencies. Its
  `lib.rs` and `model.rs` were rewritten around the leftovers.
- `crates/palantir-desktop/src/theme.rs` lost `ColorTheme::is_active`.
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

- **Microsoft sign-in.** Implemented (device-code flow in `accounts.rs` +
  `palantir-net::auth`, with refresh at launch and the token stored in the
  accounts file). This line said the opposite for several revisions after the
  work landed.
- **Assets and libraries are not downloaded at launch.** Stale in the same way:
  `install::plan` + `install::run` fetch libraries, natives, the asset index and
  its objects, and a managed JRE, and a launch refuses to start when a file is
  missing rather than reporting it. See §19 and §20.
- **CurseForge packs still import overrides only.** A CurseForge `manifest.json`
  lists its files as `projectID`/`fileID` pairs, which only resolve through the
  CurseForge API and that API needs a key at the caller's expense. Modrinth
  `.mrpack` files *are* fetched (§22); the CurseForge half is reported as
  `N entries not installed` instead of pretending.
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
| `7b29170` | Test ✅ · Lint ❌ 16 in `palantir-gui` · gnu ✅ · msvc ✅ |
| `3dee1fc` | **all four ✅** |

**The lint job found a genuine bug, twice.** `palantir-core` and `palantir-gui` deny
`unwrap_used`/`expect_used` crate-wide while their own doc comment states the
rule as "no `unwrap`/`expect` *outside tests*", and `palantir-loader` already
carries the `cfg_attr(test, allow(...))` line that makes the two agree. Neither
crate had ever been run through clippy (the component is not installed for the
local toolchain), so 153 and 16 errors respectively had been sitting there. The
first fix gave the `allow` *before* the `deny` in `palantir-gui`; inner attributes
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

- `crates/palantir-core/examples/dbg_ini.rs` — a scratch debug harness with
  hardcoded `println!`s, superseded by `ini.rs`'s own unit tests.
- `crates/palantir-core/examples/palantir-cli.rs` — a strict subset of
  `crates/palantir-cli/src/main.rs` (4 subcommands against its 8). The example had
  been stale since phase 2.
- `build.log`, `build_dbg.log` — leftover build output, already matched by
  `*.log`.
- `.unlazy/` — an empty scratch directory from a working ledger, now also
  ignored in `.gitignore`.
- `palantir-gui`'s `backend.rs`: a `GuiBackend` trait, an `App` driver and a
  `HeadlessBackend` written for a frontend that had not been chosen yet. iced is
  the frontend and drives the models directly, so this was a second, unreachable
  path. Its `Page`/`Route` navigation types duplicated the real `Page` enum in
  `app.rs`, with fewer pages.
- Unused dependencies: `anyhow` in `palantir-desktop`, `anyhow` in `palantir-core`'s
  dev-dependencies (it existed for the deleted examples), and `serde` +
  `serde_json` in `palantir-gui` (only the removed `AppSnapshot` JSON round-trip
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

## 11. What the reference client's own pixels measured

Read off a `PrintWindow` capture of the installed Modrinth App, not from
screenshots of it and not from its stylesheet -- the stylesheet was used to find
*which* token to look for, and the capture is what fixed the value. Those numbers
are now baked into `tools/panel_gate.py` as its reference, and `GATES.md` records
what each one asserts.

Its capture is 1088x612 for a 1364x881 window, so it is DPI-virtualised at ~0.79;
our own captures come back at ~1.0 and ~0.81 depending on how the window is
launched. That is why the checker locates every boundary structurally instead of
hardcoding coordinates -- a fixed offset that is inside a panel's padding at one
scale lands on the card's border at another, which is a mistake this cost two
revisions to stop making.

| What | Reference | Where it comes from |
|---|---|---|
| panel background, top | `#182524` | `.app-sidebar`'s `--brand-gradient-bg` |
| panel background, bottom | `#131a1a` | the same, measured as a straight ramp |
| a card in the panel | `#2a3633` | `--brand-gradient-button` |
| a row inside a card | `#3a4341` | `ditto`, one step lighter |
| section divider | `#303e38` | `--brand-gradient-border` |
| page pane | `#16181c` | `--color-bg` |
| chrome (rail, bar, panel) | `#27292e` | `--color-bg-raised` |
| pane's top-left corner | 20px cut | `.app-contents`'s `--radius-xl` |

The panel's wash is a two-stop vertical ramp and nothing else: sampled down its
empty gutter it runs `#172321` at the top to `#141b1b` at the bottom, and the
midpoint predicts the measured middle within one level. It is not flat and it is
not a colour -- which is exactly what a single `background` value cannot express,
and why the panel had read as a lighter grey strip beside the page rather than as
the page tinted.

## 12. Still not matching the reference, and not claimed by any gate

Recorded so the difference between "verified" and "finished" stays visible. None
of these is asserted by `GATES.md`, and the first two are visible in the capture.

1. **The panel's scrollbar.** Ours is drawn in the outer ~10px of the panel where
   the reference's `v-overlay-scrollbars` fades one over the content, so our
   panel's right edge carries a light strip theirs does not. Visible in
   `.scratch/pal-final.png` at the right edge.
2. **The panel's sections are separate cards, not divided sections.** The
   reference places its sections on the panel's own surface and separates them
   with a 1px `--brand-gradient-border` rule; we keep each in a bordered card of
   its own, which reads as more boxes than the reference has.
3. **The panel's left hairline.** `.app-sidebar` has `border-l-[1px]`, which we do
   not draw. iced paints a container's border on all four edges, so this needs its
   own 1px column the way the rail's hairline already is one.
4. **The checklist does not open and close.** The reference wraps it in an
   Accordion: `grid-template-rows: 0fr -> 1fr` over 0.3s ease-in-out, with the
   chevron rotating 180 over `transition-transform duration-300`. We always show
   the rows. Animating a height in iced means animating a `Length`, which is a
   tween plus a subscription -- the machinery `scroll.rs` already has for the
   glide, but not yet wired to this.
5. **The status bar** is ours alone; the reference has none. Left in place because
   it carries the instance count and readiness the reference puts nowhere.
6. **Hover and press feel.** Omorphia's buttons are `hover:brightness(1.25)` in
   dark and `active:scale(0.95)`; ours interpolate between palette colours
   instead. Same intent, not the same arithmetic, and not measurable from a
   still capture either way.

## 13. The Settings dialog: what it now is, and what it does not do yet

The reference's dialog is three groups and **eleven** tabs — Display (Appearance,
Features, Behavior, Language, Feature flags), Account (Profile, Social, Privacy),
Instances (Synced settings, Java installations, Resource management). All eleven
are now panes in `crates/palantir-desktop/src/settings.rs`, in that order, with the
group headings printed once per run of tabs, and `Feature flags` hidden until
developer mode is on (six presses on the version in the footer, as the reference
does). The section list is a scrollable column of real buttons, the pane is a
second independent scrollable at `min(65vh, 600px)`, and both scroll through
`scroll.rs` so a wheel over them glides rather than jumps.

**Six switches are wired, and the rest say so.**

| Switch | What it changes |
|---|---|
| Show Worlds tab | rail entry; hiding it while on the page also leaves the page |
| Show Screenshots tab | rail entry; same fallback |
| Minimize app | `window::minimize` — but only once a launch has actually started |
| Hide right sidebar | the panel is not drawn at all |
| Compact mode | cards lose their metadata chips and tighten up |
| Show play time | the playtime chip is drawn on library and sidebar cards |

Every other switch is drawn **disabled with the reason under it** rather than as a
control that silently forgets. That is a deliberate trade: the dialog names
**twenty** switches and this launcher can honour six of them, and a switch that
moves while nothing happens is indistinguishable from a working one until the
user notices. Fourteen rows therefore carry their own explanation, which is the
list of what is left to wire.

Two tests hold that line, and one of them was written because the line had
already been crossed:

1. `every_flag_names_a_field_that_carries_its_value` writes each flag's field
   name into a one-key prefs file and reads it back through the flag, so a
   typo — or a field renamed and missed — cannot pass.
2. `every_wired_flag_is_read_outside_this_module` scans every source file except
   `settings.rs` for `prefs.<field>` and fails if a flag marked wired is read
   nowhere else. **Four flags were marked wired in the revision before this one
   while nothing read them**; the switch slid, the value reached the disk, and
   the window never changed. That is the bug this test exists for.

**Not done in this pass.** The panes that describe an account (Profile, Social,
Synced settings) draw what the reference draws but act on nothing, because there
is no Modrinth account to act through. `Privacy`'s telemetry and Discord RPC
switches are drawn disabled: neither has an endpoint or a socket behind it. The
`Resource management` pane's "Purge cache" and `Java installations`' detection
follow the shell's existing behaviour rather than the reference's exact wording,
and neither has been compared against the live app on a real display yet.

## 14. Driving the shell for a capture: what works, and what does not

Capturing a *page* is easy — `tools/winshot.py --launch ... --park` renders the
window off-screen through `PrintWindow` and never touches the desk. Capturing
the *ninth* settings tab is not, because getting there means clicking, and this
shell's input does not accept injected input. Measured, not assumed:

| Delivery | Result |
|---|---|
| `PostMessage(WM_MOUSEMOVE/WM_LBUTTONDOWN/WM_LBUTTONUP)` | no change at all |
| `SendMessage` of the same | no change at all |
| `PostMessage(WM_KEYDOWN/WM_KEYUP)`, and the same sent | no change at all |
| real `SetCursorPos` + `mouse_event` | works |

Two controls keep that table honest. `PrintWindow` is live rather than a cached
frame — resizing the parked window re-renders it and the capture changes — and
the click itself is checked by reading the rail rather than by eye: the selected
entry is the only one with an accent plate behind it, and after every injected
click Home still had it.

So `winshot.py` grew a `--script` session (`click`, `msgclick`, `sendclick`,
`key`, `sendkey`, `shot`, `activate`, `resize`, `wait`) so the probes are
repeatable, and `--park`, which is now applied the moment the window is found
rather than after `--settle` — six seconds of the app over somebody's work is
six seconds of exactly what parking is for. The finding is the point: a future
session should not re-derive it. A screenshot walk of every page and pane needs
either the real pointer or a capture path inside the app that sets its own
state, and nothing in between will do it.

## 15. The rename: PalantirMC all the way down

The workspace no longer carries another project's name anywhere of its own:

| Before | After |
|---|---|
| `crates/prism-core` (package `prism-core`, lib `prism_core`) | `crates/palantir-core` |
| `crates/prism-net` / `-loader` / `-gui` / `-cli` / `-desktop` | `crates/palantir-*` |
| `PrismPaths` | `PalantirPaths` |
| `PrismApp` | `PalantirApp` |
| `PrismVersion` | `PalantirVersion` |
| `MicrosoftOAuth::prism_client_id` / `with_prism_client_id` | `::public_client_id` / `with_public_client_id` |
| `.prism-test-data/`, `prism-cli-test-` | `.palantir-test-data/`, `palantir-cli-test-` |

Two things this pass fixed rather than renamed:

1. **The launch script wrote the wrong launcher's name into the game.**
   `windowTitle` was a hardcoded `"Prism Launcher: {instance}"`, and the desktop
   shell and the CLI both passed `"Prism Launcher"` as the `launcherBrand` — so
   the Minecraft window's own title bar, and the crash report, credited a
   launcher that did not start the game. The name now lives in one place
   ([`palantir_core::PRODUCT_NAME`]), `windowTitle` takes it from the same
   argument as `launcherBrand`, and `brand::APP_NAME` is that constant rather
   than a second literal.
2. The crate descriptions said "Prism-compatible launcher core". They now name
   the product; compatibility is a property of the *formats*, documented where
   the formats are read.

**What was deliberately kept, and why.** References to *Prism Launcher the
project* are not branding, they are the record of where this launcher's data
comes from, and three kinds cannot be renamed without breaking something real:

* `prismlauncher.cfg`, `%APPDATA%\PrismLauncher`, `~/Library/Application
  Support/PrismLauncher` — these are the files and folders the launcher reads and
  writes. Renaming the strings would stop it finding an existing install.
* `meta.prismlauncher.org/v1` — the metadata service the version catalog,
  libraries and assets are fetched from. It is their endpoint.
* The instance icons and the licence: `assets/ATTRIBUTION` records that the art
  is carved from `prismlauncher.exe`, (c) Prism Launcher contributors,
  GPL-3.0-only, and the About page says the same. Attribution is a condition of
  shipping the art, not a brand.

The compatibility tests keep names like
`instance_cfg_reproduces_prism_bytes_from_the_same_settings`, because that is
what they assert: our output matches a file that launcher wrote.

**Not changed on purpose: the data root.** The launcher still discovers and
uses Prism's data root (`%APPDATA%\PrismLauncher` or an existing portable dir),
which is what makes an instance created in either launcher open in the other.
Moving it to a PalantirMC-named directory would be a one-line default and a
broken promise for anyone with instances already there; it needs a migration
story first.

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

## 17. Adopting PandoraLauncher's engine

Decision taken 14 Sep: stop re-deriving the launcher engine and take
[PandoraLauncher](https://github.com/Moulberry/PandoraLauncher)'s instead. Two
facts made that the cheap path rather than a gamble. It is **Rust** — a
workspace of eleven crates, `nbt` and `schema` up through `auth`, `bridge`,
`command` and `backend` — so its code is of a kind this tree can host; and it is
**MIT, Copyright (c) 2025 Moulberry**, which is GPL-3.0-compatible and needs
only that the notice travel with the code. It now does:
`THIRD_PARTY_NOTICES.md`, `licenses/PandoraLauncher-LICENSE.txt`, and a doc
comment at the top of each adopted crate saying where it came from. There is
nothing to hide and no reason to hide it — the licence makes the reuse lawful,
and the notice is one file.

### 17.1 What is in the workspace now

* `crates/nbt` — 2,455 lines, the NBT reader/writer (decode, encode, SNBT).
* `crates/schema` — 3,062 lines, the wire types: version manifests, asset
  indexes, Java runtime components, loader manifests, instance and content
  records.

These two are the leaves — neither depends on another Pandora crate — so they
came first and `backend`/`auth`/`bridge` build on them. Both are workspace
members like any of ours: no separate vendor tree, the same commands, the same
editor. Their manifests spell dependency versions out (upstream inherits them
from its own workspace root) and keep `edition = "2024"`, which upstream's
let-chains require and which needs a toolchain of 1.85 or newer — this machine
has 1.98. The code itself is byte-identical apart from `_name` on the non-unix
arm of `get_shared_library_path_for_name` (its argument is unused there), with
upstream's style lints allowed at the crate root so a later upstream change can
be **merged rather than re-derived**. `clippy::correctness` is deliberately not
among the allows.

Nothing in `palantir-*` imports either crate yet, so the shipped exe is
unchanged in size and behaviour as of this step.

### 17.2 The order of the rest

`auth` (needs `schema`, `oauth2`, async `reqwest`) → `bridge`, `command`, `t`,
`ftree` → `backend` (34 files, ~19.7k lines; brings `tokio`, `rusqlite`,
`rayon`, `image`, `zip`, `tar`, `runas` and the `windows` crate). Each arrives
the same way: copy, spell the manifest out, verify, record any edit in the
notice. The clone used for this is a depth-1 checkout kept outside the tree, so
further crates and future upstream merges come from the same source.

Two mechanical facts to expect when `backend` lands. It is async throughout
while `palantir-*` is blocking by design, so the seam between them is a real
piece of work rather than a rename. And its instance model is Pandora's, not
Prism's, while this launcher reads and writes `instance.cfg` / `mmc-pack.json`;
that mapping is the part with no upstream code to copy.

`cargo fmt` is advisory in CI and upstream formats at `max_width = 120` against
this repo's default 100, so a future `cargo fmt --all` would report on the
adopted crates. Their formatting is left as upstream wrote it on purpose.

### 17.3 The blocker that is not code

Pandora's auth hardcodes its own Azure application id
(`e5226706-5096-431d-9516-ae48fe263401`). Signing in under an app registration
we do not own would tie every user's login to someone else's tenant and to
someone else's ability to revoke it — telling detail: upstream carries a
`force_client_id` override for exactly this. So adopting their auth means
adopting its **flow** (authorization code + PKCE against a loopback listener,
`XboxLive.signin` + `XboxLive.offline_access`, XBL → XSTS → `login_with_xbox`,
and expiry corrected by the clock skew the service reports) under an id **we**
register. Ours is currently `palantir_net::DEFAULT_MICROSOFT_CLIENT_ID` —
Prism's public id — which is the likeliest single reason sign-in fails at all,
and which no amount of copied code fixes.

### 17.4 Verification of this step

`cargo test --workspace --all-targets --locked` green (0 + 4 + 167 + 8 + 336 +
6 + 27 + 99, the six live tests ignored as designed) and `cargo clippy
--workspace --all-targets --locked -- -D clippy::correctness` exits 0 with the
workspace warning count **unchanged at 51** — the two adopted crates contribute
none, because their own warnings were triaged into the crate-root allows rather
than left to accumulate.

## 18. The first artifact built from the Java work, and where it is

`60829d1` pushed the Java-runtime work and the two adopted crates together, and
run [`34839292132`](https://github.com/MSedgeMC/PalantirMC/actions/runs/34839292132)
is the first run to cover either. **All five jobs green** — `Test workspace`,
`Lint`, `Live services`, `Build exe (x86_64-pc-windows-msvc)`,
`Build exe (x86_64-pc-windows-gnu)` — so the live tests pass in CI against the
real metadata service, which is the only place the new `net.minecraft.java`
parsing has ever been asked a question a fixture did not write.

| Artifact | bytes | sha256 |
|---|---|---|
| `PalantirMC.exe` (msvc, recommended) | 8,417,280 | `ee6cc365937499b6343a7622636c5c3b68e20f20a7829b096cec32bb681ad633` |
| `PalantirMC.exe` (gnu) | 8,660,480 | `cc890fa59872b05e9635004eb9569bb56369fc9226abc2b15849f1d6faef155f` |

Both downloaded from the run, both hashes matched the sidecars the runner wrote,
and both pass `check_exe.py` (msvc imports 18 DLLs, gnu 27, all of them
Windows'). `tools/launch_check.py` then ran them off-screen:

| | msvc | gnu |
|---|---|---|
| window appears | 0.20 s | 0.30 s |
| idle windows | 0.0 ms ×5, one 15.6 ms | 0.0 ms ×6 |
| working set | 29.2 MB (peak 33.9) | 30.0 MB (peak 34.7) |
| UI thread | responsive, 0 ms | responsive, 0 ms |

That matches §10's shape, including the single stray window, which is the
recorded transient rather than a target difference.

**Where the files are.** `dist/PalantirMC.exe` is now the msvc build from this
run — the byte count differs from the 8,441,856 that was there, so it is a
different binary and the sidecar beside it is the hash of the new one.
`dist/PalantirMC-gnu-60829d1.exe` is the gnu build, and both raw artifact
directories are under `dist/ci-60829d1/`. None of it is committed: `dist/` is
ignored, and CI's upload is the copy that expires — 14 days, so a build worth
keeping wants a tag.

### The one thing left unverified by this run

`java_runtime`'s *download* half. The live test checks the metadata parses and
that a published digest is the digest its manifest URL serves; nothing in CI
fetches a runtime's few hundred files, because that is a few hundred megabytes
and a filesystem. So the code path that installs a JRE is exercised only by the
fixture-backed tests in `java_runtime.rs`. A machine with no Java at an accepted
major is the way to close it, and it is the same shape of gap as §16.5's real
game launch.

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

## 24. The Screenshots page, copied off the reference's window

The port after §23's shell pass, and the first one done *per page* rather than
across the shell, because that is the shape the work has: 400-500 element-states
for the whole app, captured in sessions and ported with the reference's own
numbers in hand.

**The pass.** `tools/refwalk.py` drove the installed Modrinth App to the
Screenshots page and captured everything it has: the page, its empty state, the
title bar, every hover, and — new — its motion. The measurement tools grew two
verbs for that last part: `shot` OCRs each frame and so costs about two seconds,
which makes it useless for a 300ms transition, so `burst NAME N MS` captures raw
frames at an interval and `mark` captures one without OCR. (The `click` and
`hover` verbs' optional settle argument used to be unreachable — the guard read
three tokens while the body read a fourth — which is what a burst after a click
needs.)

**What the page turned out to be.** Nothing but a centred cluster: a 216x113
illustration, a 24px bold white heading, a 16px tertiary subtext, the gaps
between them 54px and 35px of ink, the whole block centred 20px below the column's
middle and 11px of scrollbar gutter left of it. No heading of its own — the page's
name is in the title bar, which is a shell change that came with this page — no
rule, no button, no drop target.

**Three corrections to the previous pass.** The bar is *one* chrome bar across
the whole window with a 1px `#42444a` rule under it, not three per-column headers;
the page's top-left corner is a 16px radius, not a cut of unspecified size; and
the two "unidentified dim hollow glyphs" beside the wordmark are **back and
forward**, 30px outlined circles with a filled triangle in each, dim because the
reference's rail navigation does not push history. They are recorded, not ported:
this shell's pages are flat, so a history stack would be a new feature wearing
another launcher's chrome.

**What was not copied, and why.** The illustration's *artwork*: the box, the
palette (`#1d1f23` fill, `#34363c` outline — which are this palette's rail and
input surfaces, so the artwork follows the color theme) and the gaps are the
reference's, the drawing is ours. The Refresh chip: the reference has no control
on this page, and removing ours is what makes the page one cluster; the rescan it
drove still runs on entering the page, which is when the reference's page reloads
too. And the populated grid, because the reference's own data root has no
instances, so that state has never been seen.

**The motion result is a negative one, and worth having.** Twenty-six frames
starting at the click show the Screenshots page fully drawn in the *first* frame:
no fade, no slide, and no hover response anywhere on the page. An earlier session
read 1-level differences between captures as a fade; they are ClearType's colour
fringing, which is also why no gate may compare glyph bitmaps between the two
clients. The animated surface in the reference is its right-panel promos
(x 997..1279, y 485..710), which are Modrinth's own and are not ours to draw —
but they are why a whole-window diff of the reference is not evidence.

**The gates.** `tools/page_gate.py` is new and is the page-content oracle, where
`panel_gate.py` judges the shell: the page colour, that the column holds exactly
one cluster, the illustration's 216x113 box and its two colours, the heading's
ink colour and box, the subtext's, both gaps, and the centring including the
gutter. It passes on the reference's own capture and fails on the build it
replaces — the replaced build had an in-page heading, a rule and a Refresh chip,
so its column holds one band where the reference's holds three, and its page
background is the old inset panel's `#34363c` rather than the page's `#16181c`.

**What the runner confirmed.** All five jobs of the run that built this page are
green ([35456763423](https://github.com/MSedgeMC/PalantirMC/actions/runs/35456763423)),
the exe it produced is hash-checked against its own sidecar (`e424a9d6…7bb0`) and
staged as `dist/PalantirMC.exe`, `dist/PalantirMC-msvc-d23ed32.exe` and
`dist/PalantirMC-gnu-d23ed32.exe`, and all nine page gates were then run against a
capture of that exe with the Screenshots page open: **9 met, 0 unmet**. Its
measurements against the reference's, side by side: the illustration 213x108
against 216x113 (the port's own artwork, five rows short of the box because its
back frame starts 6px in), both texts 23 and 16 ink rows and the same two colours
to within a level, the ink gaps 53 and 37 against 54 and 35, and the block's centre
x 514.0 / y 390.0 against 513.0 / 387.5.

**The gap constants were the open question, and the capture closed it.** They are
box-to-box (47px and 7px) rather than ink-to-ink, because a text widget's box
starts above its cap; if iced's boxes carried the font's line gap as well as its
ascent and descender, the heading and subtext would have landed a few pixels below
where the reference draws them. They landed within a pixel and two. So the model
the port was built on holds, and the next page can use it without a capture to
check first.

**Verifying the port found two faults in the gate, not in the page.** Both were
instrument errors that only a capture of *this* shell could expose, and both are
fixed in the tool rather than worked around in the expectations: the panel's left
edge is not the strongest vertical boundary on that side (this shell puts a
scrollbar and then a resize grip beyond it), so the gate was measuring the panel as
part of the page column and merging the page's three content bands into one; and
the page column here does not run to the window's bottom, because the status strip
takes the last 22px, so the column now ends where the page colour ends. The first
attempt at the capture failed for a third reason worth recording: click the rail
and a tooltip stays under the pointer, and a tooltip is the *shell's* popup painted
over the page -- the capture for the gate parks the pointer on the page instead.
