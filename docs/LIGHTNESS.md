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
