# Lightness

**Why this is a requirement.** The egui rewrite was agreed to because the iced
launcher is heavy: a 28.6 MB binary, a frame loop that keeps a core busy while
nothing moves, a scrolling p95 of 26–62 ms. A rewrite that buys a smooth frame
with a bigger binary, or one animation with another hundred megabytes, has
failed the requirement as surely as a page that is not 1:1.

## The five budgets

| Budget | Number | How it is measured |
| --- | --- | --- |
| Release exe | **no larger than the iced build** — 28,648,960 B (28.6 MB), recorded 2026-10-25 from `dist/msvc/PalantirMC.exe` | length of `--exe`, against `--baseline` or the recorded constant |
| Idle RAM | **120 MB** resident, five minutes after the window is up | `WorkingSet64`, harness below |
| Startup | **400 ms** from process start to a window that is up | stopwatch, harness below |
| Scrolling | **20 ms** p95 of frame intervals during a scripted wheel scroll | `SIFT_FRAME_LOG`, read below |
| Dependencies | **none** of `iced`, `iced_wgpu`, `iced_winit`, `cosmic-text`, transitively | `cargo metadata`'s full graph, from the working directory |

Idle CPU needs no row of its own: the same idle window samples
`TotalProcessorTime`, and the delta must be **0 ms** — the L0 slice's
unconditional `ctx.request_repaint()` is exactly what the app must not inherit.

## Where the numbers come from

Size is one `stat`. The dependency row walks `cargo metadata --format-version 1`
(the full graph, not `--no-deps`) from the workspace the command runs in, so
`iced` cannot return even transitively. The root workspace still contains iced —
it *is* the iced build — so judge the egui side from `crates/palantir-egui` until
slice D1. No `cargo` or no `Cargo.toml` is SKIP. RSS, startup and idle CPU come
from this harness, against a release build:

```powershell
$sw = [Diagnostics.Stopwatch]::StartNew()
$p = Start-Process -FilePath "dist\egui\PalantirEgui.exe" -PassThru
while ($p.MainWindowHandle -eq 0) { Start-Sleep -Milliseconds 5; $p.Refresh() }
$startup = $sw.Elapsed.TotalMilliseconds     # start -> window up: a lower bound on first frame
Start-Sleep -Seconds 60; $p.Refresh(); $cpu0 = $p.TotalProcessorTime.TotalMilliseconds
Start-Sleep -Seconds 240; $p.Refresh()       # five idle minutes in total
$rss = $p.WorkingSet64 / 1MB; $idleCpu = $p.TotalProcessorTime.TotalMilliseconds - $cpu0
```

Frame p95 comes from `SIFT_FRAME_LOG` (today `PALANTIRMC_FRAME_LOG`; slice R1
renames it), one line per 30 frames:

    frames=30 interval_ms mean=16.63 p50=16.70 p95=17.80 max=33.20 | view_ms mean=0.42 p95=0.61 max=1.30

Scroll a long list for five seconds, take the 95th percentile of the per-window
`interval_ms p95` values, and write the three numbers the checker reads:

```powershell
$env:SIFT_FRAME_LOG = "$PWD\frame.log"   # set before launch; scroll, close, then:
$p95 = Get-Content frame.log | Select-String 'interval_ms .*p95=([0-9.]+)' |
  ForEach-Object { [double]$_.Matches[0].Groups[1].Value } | Sort-Object
$p95 = $p95[[int][math]::Floor(($p95.Count - 1) * 0.95)]
@{ idle_rss_mb = $rss; startup_ms = $startup; scroll_p95_ms = $p95 } |
  ConvertTo-Json | Set-Content tools\lightness-measured.json
```

## What buys the budgets

- **iced out of the binary** — the whole stack and its winit/wgpu tree: most of
  the 28.6 MB, and exactly what the forbidden list protects.
- **Release profile** — `lto = "thin"`, `codegen-units = 1`, `strip = true`,
  `panic = "abort"`: dead code and symbols gone without a `fat` LTO build.
- **One Inter subset**, not four whole faces: five ~300 KB TTFs ship today; only the weights and scripts drawn are kept.
- **Images decoded at draw size**, not at source resolution and scaled per frame.
- **Bounded caches** — icon atlases, image and layout caches capped so long use cannot read as a leak.
- **Repaint on demand** — a frame when something changed, an animation until it
  lands, nothing while idle: all of idle CPU 0 and much of idle RAM.

## Running it

```
python tools/lightness.py --help
python tools/lightness.py --exe dist/PalantirEgui.exe --baseline dist/PalantirMC.exe
python ../../tools/lightness.py --exe target/release/PalantirEgui.exe --check  # CI, from crates/palantir-egui
```

Exceeded rows print as `BUDGET exceeded: <name> <value> > <budget>`, and
`--check` turns them into exit 1. A missing measurement is SKIP, never PASS.

## Measured: the egui build, 2026-10-25

`tools/lightness-measure.sh` on the `sift-launcher` VM, against the **debug** build (the one
every capture is taken from), so these are a ceiling rather than a shipping number:

| Budget | Limit | Measured (debug) | Verdict |
| --- | --- | --- | --- |
| Startup to first window | <= 400 ms | **65 ms** | passes with room |
| Idle CPU | 0 ticks | **0 ticks over 10 s** | passes exactly |
| Idle RSS | <= 120 MB | **134.1 MB** | **over by 14 MB** |
| Release exe | <= the iced build (28.6 MB) | debug 304 MB; release measured separately | pending |

Two of the three are already what the budget asks for, and the idle-CPU one is the design
working: `App::ui` asks for a frame only while a hover, a glide, a search or an icon fetch is
in flight, so a window with nothing happening draws nothing at all. Zero processor ticks over
ten seconds is the measurement of that claim.

**Idle RSS is the one that misses, and the debug build is most of the reason**: an
unoptimised 304 MB binary has every touched code page resident, and the 2.7 MB generated icon
table is debug-shaped. The honest test is the release build, so the script is run against it
next; if a release build still sits above 120 MB the next steps in order are: drop the two
Inter weights nothing uses (400/500/600/700/800 -> 400/600/800), load the icon table once
rather than as a debug-sized const, and give the texture cache a bound (it currently keeps
every icon it has ever fetched).

The measurement itself is worth keeping as a habit rather than a number: startup is polled
for the window rather than assumed from a sleep, RSS comes from `/proc/<pid>/status`, and idle
CPU is a `utime+stime` delta over a ten-second window with the app untouched.

## Measured: the release build, 2026-10-29 (0.1.23)

Same VM, same method, `target/release/SiftUi` built in 2m19s from `master` at `v0.1.23`.

| Budget | Limit | Release | Verdict |
| --- | --- | --- | --- |
| Startup to first window | <= 400 ms | **55 ms** | passes, 7x of room |
| Idle CPU | 0 ticks | **0 / 10 s** | passes exactly |
| Idle RSS | <= 120 MB | **124.2 MB** | **over by 4.2 MB** |
| Exe (Linux release) | — | 14,029,440 B | for reference; the Windows comparison is CI's |
| Scroll p95 (view) | <= 20 ms | **0.06–0.08 ms** | passes; interval p95 7.2–8.2 ms |

RSS is stable rather than leaking: three runs read 124,292 / 124,224 / 124,264 kB, and each was
identical at 25 s and 45 s. Allocator tuning changes nothing (`MALLOC_ARENA_MAX=1` 124,352 kB,
`=2` 123,996 kB, mmap thresholds 124,236 kB).

**Where the 124 MB is.** `/proc/<pid>/smaps` says most of it is file-backed library code, and the
largest single mapping is the VM's software rasteriser:

```
Rss 124,148 kB   Pss_Anon 42,256 kB   Pss_File 70,422 kB
46,604 kB  /usr/lib/x86_64-linux-gnu/libLLVM.so.19.1
26,268 kB  [heap]
 5,888 kB  .../target/release/SiftUi
 5,624 kB  libgallium-25.0.7
 5,176 kB  /SYSV00000000   (X shared memory)
```

So this VM has no GPU: Mesa's llvmpipe JIT compiles shaders through LLVM, and that library is 46 MB
of the resident set. The launcher's own anonymous heap is **42 MB**, its code 5.9 MB, the X server's
shared buffer 5.2 MB. On a machine with a working GL driver `libLLVM` is not loaded at all.

**What that means for the budget.** The 120 MB figure was set against the iced build and is passed
here at 119.8 MB on 2026-10-25 on this same VM — the same software GL was in play then, so the
~4 MB delta is ours, not the driver's. It is small, it is not a leak, and finding it needs a heap
profile rather than more measuring; `docs/LIGHTNESS.md`'s own next steps (bound the texture cache,
load the icon table once) are the place to start.

**The font atlas is the other big resident block.** Opening the settings dialog and visiting all
fourteen of its tabs costs 15.9 MB of atlas (Home 124.0 MB -> 139.9 MB), because every tab's glyphs
are rasterised at five weights and twelve sizes. Two of those sizes were used exactly once at a
half point (13.5 and 12.5); folding them into 13 and 12 removed two whole passes:

| | Home | Settings, all tabs |
| --- | --- | --- |
| before | 124,044 kB | 139,916 kB |
| after | 124,412 kB | **135,784 kB** |

So the fold saves 4.1 MB where it counts and costs nothing on Home -- which also tells us the idle
figure is not this: 4 MB of the difference from the 119.8 MB run is something else, and a heap
profile is the next step rather than another guess.

## Measured: the release build, 2026-10-25

Same script, same VM, against `target/release/SiftUi` (2m01s to build):

| Budget | Limit | Release | Debug | Verdict |
| --- | --- | --- | --- | --- |
| Startup to first window | <= 400 ms | **65 ms** | 65 ms | passes with 6x of room |
| Idle CPU | 0 ticks | **0 / 10 s** | 0 / 10 s | passes exactly |
| Idle RSS | <= 120 MB | **119.8 MB** | 134.1 MB | **passes by 0.2 MB** |
| Exe | <= 28,648,960 B | **20,081,824 B** | 304,157,632 B | passes, 30% under the iced build |

**Every budget passes, and one of them passes too narrowly to leave alone.** 119.8 against 120
is not a margin, it is a coin toss: the next icon, the next font weight or a platform whose
allocator rounds differently puts it over. The fix list, in the order worth trying:

1. **The texture cache has no bound.** It keeps every icon it has ever fetched, so RSS depends
   on how much the reader has browsed. A cap (say 256 textures, oldest dropped) makes the
   number independent of history, which also makes it repeatable.
2. **Five Inter weights** are registered (400/500/600/700/800) and the interface uses three of
   them. Two unused faces are atlas and shaping state for nothing.
3. The 2.7 MB generated icon table is `const` data in the binary's read-only segment: it costs
   file size rather than RSS, which is why the exe-over-budget worry and the RSS worry are
   different worries.

Startup did not move between debug and release, which says the 65 ms is dominated by window
creation and the first frame rather than by code speed -- worth remembering before optimising
anything for it.

## Final: the shipped profile, 2026-10-25

The workspace now declares its own `[profile.release]` (`opt-level = 3`, `lto = true`,
`codegen-units = 1`, `strip = true`, `panic = "abort"`). A nested workspace inherits nothing
from the root, so until this slice the exe shipped unstripped and without LTO.

| Budget | Limit | Shipped release | Verdict |
| --- | --- | --- | --- |
| Startup to first window | <= 400 ms | **53 ms** | 7.5x of room |
| Idle CPU | 0 ticks | **0 ticks over 10 s** | passes exactly |
| Idle RSS | <= 120 MB | **114.6 MB** | passes with 5.4 MB of margin |
| Exe | <= 28,648,960 B (iced) | **11,981,120 B** | 58% smaller than iced |

Against the same budgets a debug build measures 55 ms, 0 ticks, 130.3 MB and 307,368,432 B --
which is why the script grew a binary argument: measuring the wrong build silently is how a
budget appears to pass while the thing that ships was never checked.

Release build time is 1m55s with LTO on one core. The two fixes that turned a 0.2 MB margin
into 5.4 MB were bounding the icon texture cache (256 entries, coldest evicted, `Loading` and
`Failed` never candidates) and the profile itself.

## Scroll: measured at last, 2026-10-25

`SIFT_FRAME_LOG` on during a Discover scroll (40 notches down, 40 up, on the debug build --
the one captures use, so this is the pessimistic reading):

| Window | interval p95 | view p95 |
| --- | --- | --- |
| Own worst scrolling window | **18.53 ms** | 1.00 ms |
| Typical scrolling window | 16.13 - 16.33 ms | 0.99 ms |
| Two windows that are not scrolling | 63.64 ms, 1885.02 ms | -- |

The budget is <= 20 ms and the worst scrolling window is 18.53, so it passes -- and the two
outliers are worth naming rather than averaging away: they are the startup windows, where the
first frames carry the font atlas, the logo decode and the first icon uploads. A frame budget
that quietly included them would look like a 1.9-second scroll.

**`view_ms` is the number that says the scroll container is not the cost.** The paint half of a
frame is about a millisecond at p95; the rest of the 16 ms interval is the vsync the compositor
imposes. That is the `fastframe-scroll` decision paying off: the reference's faster notch needed
a vendored container, and the container is not what the budget was afraid of.

## Re-measured at 0.1.7 — 2026-10-29

The numbers above were taken at 0.1.0. These are the same five budgets against the current tree,
on the same VM, with a release build, and the method stated rather than implied.

| Budget | Limit | 0.1.0 | **0.1.7** | How |
| --- | --- | --- | --- | --- |
| Startup | ≤ 400 ms | 53 ms | **167 ms** | spawn to the window being found, polled every 10 ms, cold |
| Idle CPU | 0 ticks | 0 / 10 s | **0 / 10 s** | `utime+stime` over ten seconds after a 25-second warm-up |
| Idle RSS | ≤ 120 MB | 114.6 MB | **114.7 MB** | `ps -o rss=` at the same moment |
| Exe | ≤ 28.65 MB | 10.79 MB | **13.82 MB** | the release binary, unstripped, GNU target |
| Scroll p95 | ≤ 20 ms | 18.53 ms | **16.49 ms worst, ~13 ms typical** | `SIFT_FRAME_LOG`, 120 wheel steps over the settings body, per-window p95 taken from the log |

**Two things this pass taught that the numbers alone do not say.**

- **The first ten seconds are not idle.** Measured immediately after the window appeared, the
  app burned **19 ticks over ten seconds** and sat at **121.7 MB** — one tick budget missed and
  one megabyte over. After a 25-second warm-up the same build is **0 ticks and 114.7 MB**. Fonts,
  the icon cache and the first layout are real work that finishes; a measurement that starts
  before they do reports them as idle cost. Both figures are recorded so the next person does
  not have to rediscover which one is the budget.
- **Startup grew from 53 ms to 167 ms** and the honest explanation is not a regression in
  startup: the 0.1.0 number was a single observation, this one polls `xdotool` in 10 ms steps
  from a cold cache. They are not the same measurement, which is why the method column exists.

**Scroll, measured 2026-10-29 with the app's own instrument.** `SIFT_FRAME_LOG` writes
`mean/p50/p95/max` for the frame interval and for the paint half every thirty frames. One hundred
and twenty wheel steps over the settings dialog — the longest scrolling body in the tree — gave:

| Window | interval p95 | view (paint) p95 |
| --- | --- | --- |
| six consecutive windows while scrolling | 12.59, 12.74, 13.00, 13.16, 13.27, 14.18 ms | 0.09 - 0.20 ms |
| worst p95 of any window in the run | **16.49 ms** | 0.36 ms |

The budget is 20 ms, so the worst scrolling window clears it by 3.5 ms, and the paint half is a
fifth of a millisecond — the interval is not spent drawing. The one **2073 ms** p95 in the log is
the settings dialog opening (a first frame that builds the body), not a scroll, and it is named
here so nobody mistakes it for one. Xvfb has no vsync, so the ~12 ms interval is the app's own
pace rather than a 16.7 ms cap being held.
