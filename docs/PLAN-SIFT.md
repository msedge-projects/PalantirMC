# Sift Launcher: the egui rewrite, 1:1 with Modrinth

**Plan v2, 2026-10-25.** Supersedes v1. What changed, at the owner's instruction:

- **Dynamic Island is out of the plan.** It was an example of why immediate mode
  mattered, not a deliverable; it moves to the appendix and nothing waits on it.
- **Every page must be 1:1 with the Modrinth app** — animations, transitions, UX,
  icons, elements, tabs — and each one is **verified by looking at both side by side**
  on the VM before it counts as done.
- **Lighter**: the rewrite has size, memory, startup and frame budgets, measured, not
  hoped for.

---

## 1. What "1:1" is going to mean, precisely

Two toolkits cannot produce identical pixels — different text rasterisers — so "1:1"
is defined on five axes, each with a tolerance and each checked per page:

| Axis | Definition | Tolerance | How it is checked |
| --- | --- | --- | --- |
| **Layout** | every element's box at 1440×900 against Modrinth's | ±2 px on x, y, w, h | geometry dump from our side, rectangles read off Modrinth's screenshot |
| **Colour** | the tokens, not the pixels: backgrounds, borders, text, accents | exact RGB, ±1/255 alpha | sampling the same point on both captures |
| **Type** | family, size, weight, line height, letter spacing | exact from the reference CSS | computed style vs our constants |
| **Icons** | same glyph, same stroke weight, same optical size, same alignment | 1 px | the 313-glyph set generated from the reference's SVGs |
| **Behaviour** | hover, active, disabled, focus, tab order, transition durations and easing, scroll step | duration ±16 ms; states exact | driving both apps with the same scripted input and comparing frames |

A page is done when its gate `G-PARITY-<page>` holds all five rows with evidence
attached. A page that "looks right" but has a 140 ms hover where the reference has 150
is not done — the timings are in the reference's CSS and are cheap to match exactly.

## 2. The comparison loop: how each page is verified

The requirement is to *look* at both, so the loop is built to make that cheap and repeatable.

1. **Both apps on `sift-launcher`**, headless at 1440×900 under `Xvfb`: the Modrinth app
   (Linux build, pinned to the vendored `8966b5e` tree's release if one matches, else the
   version we vendored) and Sift Launcher built for Linux. Same window size, same
   display scale (`GDK_SCALE=1`), same font set.
2. **`tools/parity/capture.py`** drives both with one script (`xdotool`): open a route,
   wait for idle, capture; hover a named element, capture; click a tab, capture the
   transition mid-flight and after. Output: `parity/<page>/<state>-modrinth.png` and
   `-sift.png`, plus a `report.txt` of sampled colours and measured rectangles.
3. **`tools/parity/side_by_side.py`** composes them into one image per state, halves
   labelled, plus a difference strip. **I open that image and list every difference** in
   a table in `NOTES.md` — this is the "look at both" step, and it is done by eye as well
   as by `compare -metric AE`.
4. Fix, re-capture, until the difference table is empty. The final pair goes in the
   commit's evidence line.

**Known limits, stated up front.** Modrinth's UI is a webview (Tauri/WebKit) and ours is
native; a browser lays out text differently from any Rust toolkit, so line breaks are the
one thing that will not always match. The mitigation is the reference's own numbers:
where Modrinth's CSS fixes a height, a width or a line count, we set it; where it does
not, the tolerance in §1 applies. Pages behind a login (`user`) are captured with a
throwaway account; instance pages need a profile — we create one through each app rather
than hand-writing JSON, so both describe the same instance.

## 3. Lighter: budgets, measured before and enforced after

**Phase L0 measures the baseline first** — an unmeasured budget is a wish. Baselines
come from the iced build we ship today and the egui slice:

| Budget | Baseline (iced, today) | Target | Enforced by |
| --- | --- | --- | --- |
| Release exe (msvc, static CRT) | measured in L0 from the CI artifact | **≤ baseline**, ideally −25% | `tools/lightness.py` in CI, hard fail |
| Idle RAM (5 min idle, 4 tabs' worth of state) | measured in L0 | **≤ 120 MB** | harness on the VM, recorded per slice |
| Idle CPU | egui redraws continuously unless told not to | **0% when nothing animates** | repaint-on-demand everywhere; the frame log must show *no* frames while idle |
| Startup to first frame | measured in L0 | **≤ 400 ms** | harness on the VM |
| Frame p95 while scrolling | 26–62 ms | **≤ 20 ms** | `PALANTIRMC_FRAME_LOG` (→ `SIFT_FRAME_LOG`), every UI slice |
| Dependencies | iced + cosmic-text + … | iced and its tree gone | `cargo tree` diff in the slice that deletes iced |

Where "lighter" comes from, concretely: iced's whole stack out of the binary; release
profile with `lto = "thin"`, `codegen-units = 1`, `strip = true`, `panic = "abort"`;
subset Inter to the weights and scripts we actually use instead of shipping four full
faces; images decoded at the size they are drawn; caches bounded and shared; and **no
continuous redraw** — the app asks for a frame when something changed, and while
something animates it stops asking when the animation lands.

## 4. The slices

Order is deliberate: the harness first, because every later slice is judged through it;
the rename first among product work, so nothing is renamed twice.

| # | Slice | Done when |
| --- | --- | --- |
| **L0** | Parity harness + lightness baselines: `tools/parity/*`, `tools/lightness.py`, Modrinth app and our Linux build installed on `sift-launcher`, captures working | one page captured side by side end to end; baselines recorded in `NOTES.md` |
| **R1** | Rename: repo `SiftLauncher`, crates `sift-core/-net/-loader/-desktop/-ui`, `PRODUCT_NAME`, window id, env vars (`SIFT_*`, old read once), prefs file, log/pid names, CI artifacts, **fresh data dirs** (`%APPDATA%\SiftLauncher`, no migration) | both workspaces build; `palantir` survives only in `NOTES.md` history and `vendor/` |
| **R2** | Brand: `gen_brand.py` rewritten for the RGBA source → `logo512.png`, `icon256.png`, `app.ico`, `installer/sift.ico`; About page; installer text | `--check` passes; the new mark in title bar, hero and taskbar |
| **A1** | egui chrome 1:1: window, title bar, rail, panel, the nine scroll regions | rail/head/panel pass their parity gates; p95 ≤ 20 ms |
| **A2** | Motion 1:1: hover/press/focus states and **the reference's durations**, fastframe scroll step, tab transitions | transition-duration gate per component (±16 ms); hover states pixel-compared |
| **B1–B3** | `sift-store`: `store.rs` (4,021) + `prefs.rs` (1,034) out of the UI crate, 119 call sites across 18 modules inverted | both stacks on it; iced still builds; tests green |
| **C1–C10** | Pages, each with its own side-by-side verification: `discover`, `home`, `project`, `instance` (+7 tabs), `skins`, `screenshots`, `servers`, `user`, `bedrock`, settings/dialogs/toasts | `G-PARITY-<page>` green with evidence, plus frame and memory numbers |
| **D1** | Delete iced: crate gone, one workspace, one CI pipeline, gate ledgers merged | `iced` absent from `crates/`; one `cargo test --workspace`; exe within the L0 budget |
| **E1** | Release as **Sift Launcher**: installer, signing (owed), release workflow retargeted | a tagged release that installs and starts on a clean Windows |

**No iced release in between** (owner's decision). The first thing that ships is the
complete egui launcher.

## 5. Cost, and the risks that are real

- **23,727** lines of pages + **16,552** of shell, ~1,400 tests today. Phase C is the bulk:
  ten pages, each with a five-axis parity gate, and the reference's numbers re-derived
  rather than copied because the existing gates (G58–G171) were measured through iced's
  text metrics.
- **MSRV 1.74 → 1.98**; the 1.74 compat test goes.
- **The store extraction is the real engineering**: 18 modules, 119 call sites, and it
  must land while the iced build still passes.
- **Two workspaces until D1** — `iced_winit` pins `web-sys =0.3.67`, eframe needs
  `^0.3.103`; one lock cannot hold both.
- **The webview gap** (§2) is the honest limit on "1:1": equal boxes, colours, type
  metrics, icons and timings; not identical text rasterisation.
- **Rough 25–35 sessions.** C is most of it; L0 and R1–R2 are days, B is a week of
  sessions, E depends on code signing (still owed).

## Appendix: things the owner mentioned and this plan does not build

- **Dynamic Island** — an example of why immediate mode mattered, explicitly not to be
  built now. The design is kept in `NOTES.md` for the day it is wanted.
- **User-authored layouts / theme files** — cheap to add once the renderer registry
  exists, not part of this plan.
- **A plugin lane** — a separate project with its own sandbox decision.

---

## Open decisions before L0

1. **Repo id**: `SiftLauncher` (no spaces possible in a GitHub name). Confirm the
   spelling, or say `sift-launcher`.
2. **Modrinth reference version** for the comparisons: the vendored `8966b5e`, or the
   current release (v0.22.0) installed fresh on the VM? *Recommendation: match the
   vendored tree*, because the CSS and SVGs we generate from are that tree's, and a
   newer app would be compared against numbers we do not have.
3. **Release profile**: `panic = "abort"` costs us `cargo test` panics-as-unwinds in
   release only — fine — but it also means no backtrace recovery in the shipped app.
   Accept, or keep unwinding at a slightly larger size?
