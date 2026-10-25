# Sift Launcher: the egui rewrite, and the rename

Written 2026-10-25, at the owner's instruction: *"we need egui cuz i want to do custom
gui by user too in the upcoming features or dynamic elements"*, *"change all the
branding even the repo"*, with a new mark in
`crates/palantir-desktop/assets/brand/sift-logo-source.png`.

This is a program, not a patch. It is written down so that every later session knows
what it is inside of, what order things happen in, and what "done" means for each
piece.

---

## 0. Why egui, in one paragraph with numbers

The frame log measured both stacks on one machine, one 1280×720 window, 300 cards:

| | iced (during a scroll) | egui + fastframe-scroll (continuous) |
| --- | --- | --- |
| interval p50 | 16.1 ms | 16.7 ms |
| interval p95 | 26–62 ms | **17.6–18.6 ms** |
| worst frame | 111 ms | 18.6 ms |
| work per frame | 0.13–0.27 ms (view build) | 3–5 ms (whole immediate-mode pass) |

egui does *more* work per frame and holds a **3× tighter tail**, so the cost was never
the widgets — it is what each toolkit does between the widget and the present. But
smoothness is the smaller half of the reason. The larger half is the owner's actual
goal: **a GUI the user can shape**. In an immediate-mode toolkit the interface is a
function of state, so a user-authored layout is data that gets rendered; in a retained
widget tree with typed messages it is surgery on a tree you did not build. iced was
fighting the next feature, not just this session's scrollbar.

---

## 1. What "custom GUI by user" is going to require

Designed now, because it decides how pages get ported — not something bolted on after.

**A registry, and layouts as data.** Every page is rendered from a layout document (a
list of sections) through a registry of named renderers (`search`, `cards`,
`instance-list`, `hero`, `news`, `stat-tiles`, …). Default layouts ship with the app;
a user's copy in the data directory replaces them. An unknown section name renders as
a labelled placeholder and logs once — never a panic, never a blank window.

**Theme as data.** The tokens (colours, radii, spacing, fonts) already come from the
reference's CSS through `tools/gen_theme.py`. In egui they become a `theme.json` the
user may edit, hot-reloaded: the file is watched, the watcher calls
`ctx.request_repaint()`, and the frame after it is drawn with the new tokens.

**Declarative, not executable, in this era.** Layouts and themes are data. There is no
scripting language and no user-supplied code in v1 — a plugin lane (WASM components
behind a capability boundary) is explicitly a *later* decision, and the registry is
the seam it would plug into.

**Everything keeps working without a config.** No layout file means the shipped
default; no theme file means the generated one. Customisation is additive, which is
what makes it safe to ship.

---

## 2. The rename: PalantirMC → Sift Launcher

Do it **once, early, before the port**, so every later commit is written in the new
name and nothing has to be renamed twice. Every surface, with the consequence of
getting it wrong:

| Surface | Today | Becomes | Notes |
| --- | --- | --- | --- |
| GitHub repo | `msedge-projects/PalantirMC` | `msedge-projects/sift-launcher` | GitHub redirects old URLs; local remotes and any badges must be updated in the same session |
| Crates | `palantir-core`, `-net`, `-loader`, `-desktop` | `sift-core`, `-net`, `-loader`, `-ui` | mechanical `sed` of `palantir_`/`palantir-` across ~45 files + `Cargo.lock`; leaving them is the half-branding to avoid |
| Binary | `PalantirMC.exe` | `Sift Launcher.exe` | the installer already renames it to this |
| Product name | `palantir_core::PRODUCT_NAME` | `"Sift Launcher"` | travels into the launch script; one constant, not a second literal |
| Window/app id | `PalantirMC` | `Sift Launcher` | taskbar grouping and the single-instance mutex |
| Data dirs | `%APPDATA%\PalantirMC`, `%LOCALAPPDATA%\PalantirMC` | `…\Sift` | **needs a migration** (see §5 Q2): instances, caches, screenshots live here |
| Prefs file | `palantirmc-desktop.json` | `sift.json` | read the old one once, write the new one |
| Env vars | `PALANTIRMC_FRAME_LOG`, `PALANTIRMC_HOME`, `PALANTIRMC_CURSEFORGE_API_KEY`, `PALANTIRMC_SHOT` | `SIFT_*` | keep reading the old names for one release so scripts and the frame-log workflow keep working |
| Log/cache/pid names | `palantirmc.log`, `palantir.pid` | `sift.log`, `sift.pid` | |
| CI artifacts | `palantirmc-x86_64-…`, `palantir-egui-slice` | `sift-x86_64-…`, `sift-ui-slice` | release workflow names included |
| Brand art | `palantirmc.png`, `logo512.png`, `icon256.png`, `app.ico` | from `sift-logo-source.png` | `tools/gen_brand.py` must be rewritten: the old recipe derives alpha from a black backdrop, and the new source is *already* RGBA (2048×2048) |
| Docs | README, HANDOFF, installer text | new name | `NOTES.md` keeps its history as history, with one rename note at the top of the section |

**Order:** repo → crates → constants/paths (+ migration) → assets → docs. Each its own
commit, with the tree building at every step.

---

## 3. The port: five phases, one workspace at the end

**Phase A — foundation (in `crates/palantir-egui`, renamed `sift-ui`).**
Window and custom chrome, the rail, the panel, the nine scroll regions, `fastframe-scroll`,
the frame log (**on by default in debug builds** so every later page is measured without
anyone remembering a flag), theme tokens for egui, the icon pipeline (the SVG path data in
`icons_gen` is portable; only the iced `canvas` wrapper dies), Inter as the font. Deliverable:
the shell chrome with an empty page area, at 60 fps with a p95 under 20 ms on the i3.

**Phase B — the data layer.** `sift-store`: extract what is now `store.rs` (4,021 lines)
and `prefs.rs` (1,034) out of the UI crate. This is the hard engineering of the migration
and it was scoped honestly: the store reaches into **18 modules** (`page`, `pages`, `ui`,
`route`, `color_theme`, `scale`, `instances`, `launch`, `meta`, `mods`, `avatar`, `skin`,
`wire`, …) and 119 call sites name `crate::store`/`crate::prefs`. The inversion is
mechanical but it is the largest single piece; both stacks use the result, and the iced
launcher keeps shipping while it happens.

**Phase C — pages, in this order**, each with its behavioural tests ported and its frame
numbers recorded: `discover` → `home` → `project` → `instance` (+ its settings tabs) →
`skins` → `screenshots` → `servers` → `user` → `bedrock` → settings/dialogs/toasts.
`discover` first because it is the heaviest and the one that gets scrolled.

**Phase D — retire iced.** Delete `palantir-desktop`, drop `iced` from the tree, merge the
two workspaces into one (the `web-sys` pin that forces the split disappears with iced),
collapse CI to one pipeline, and merge the two gate ranges (`G1–G171` iced, the egui ones)
into one ledger with a note per gate that was re-derived rather than ported.

**Phase E — the features the switch was for.** Layout documents, the theme editor, the
widget registry, then (and only then, as its own decision) a plugin lane.

---

## 4. Cost, and the risks that are real

- **Size:** 23,727 lines of pages, 16,552 of shell, plus the theme/icon/text generators;
  ~1,400 tests today, most of them in the crate that dies.
- **MSRV:** 1.74 → **1.98** (egui 0.36 / fastframe are edition 2024). The 1.74 compat test
  goes; CI moves to the pinned stable and stays there.
- **Parity gates do not port, they get re-derived.** G58–G171 were measured through iced's
  text metrics (the 76.54 / 71.50 / 77.74 / 88.36 `hmtx` numbers, the 0.25 px fit). egui
  shapes text with its own stack, so every one of those numbers is re-measured against the
  reference screenshot, not carried over. That is a large part of Phase C's cost and it is
  not optional: a gate that asserts a number from a toolkit we no longer use is a lie.
- **The vendored `cosmic-text`** exists for iced's shaping. egui does its own; either it
  becomes unused (delete it, one fewer vendored crate) or it is wired in deliberately.
  Decide in Phase A, write down which.
- **Two workspaces until Phase D**, forced by `iced_winit`'s `web-sys =0.3.67` versus
  eframe's `^0.3.103`. Every CI job must say which workspace it builds.
- **Timeline, honestly:** A–B on the order of a week of sessions each; C is the bulk —
  assuming one page per session with its gates, weeks; D days; E after that. Nothing in
  this plan lands the whole rewrite in one go, and pretending otherwise would be a
  deadline nobody can hold.

---

## 5. Decisions needed before Phase B starts

1. **Repo name:** `sift-launcher`? (Hyphens are the GitHub convention; old URLs redirect.)
2. **Data migration:** move `%APPDATA%\PalantirMC` → `%APPDATA%\Sift` on first run
   (recommended — instances and caches are the user's), or start clean?
3. **Crate rename to `sift-*`** in the same pass as the repo (recommended), or keep
   `palantir-*` internally and rename only what users see?
4. **"Custom GUI by user"** means layouts and themes as data (§1) — not user-supplied
   code. Confirm, because the second one is a sandbox project of its own.
5. **Releases during the migration:** keep shipping the iced launcher until the egui one
   has all nine routes (recommended), or freeze releases and ship the egui one when it is
   whole?
